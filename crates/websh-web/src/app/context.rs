//! Application-wide reactive context and state accessors.

use std::collections::BTreeMap;
use std::rc::Rc;

use futures_util::FutureExt;
use leptos::prelude::*;

use super::{RuntimeServiceError, TerminalState};
use crate::config::APP_NAME;
use crate::runtime::content_cache::{ContentTextCache, ContentTextCacheKey};
use crate::runtime::{self, RuntimeLoad};
use websh_core::domain::{RuntimeMount, VirtualPath, WalletState, is_runtime_overlay_path};
use websh_core::filesystem::{ContentReadError, GlobalFs, display_path_for};
use websh_core::ports::{LocalBoxFuture, StorageBackendRef};
use websh_core::runtime::RuntimeStateSnapshot;

type TextReadResult = Result<String, ContentReadError>;
type SharedTextRead = futures_util::future::Shared<LocalBoxFuture<'static, TextReadResult>>;

/// Application-wide reactive context.
///
/// This context is provided at the root of the component tree and can be
/// accessed from any child component using `use_context::<AppContext>()`.
///
/// # Architecture
///
/// The URL is the single source of truth for navigation state.
/// AppContext only manages non-navigation state:
/// - **Filesystem**: Virtual filesystem for file operations
/// - **Terminal state**: Command history, output
/// - **Wallet state**: Connection status, address, ENS name
///
/// `Clone + Copy` because every field is a signal handle or a nested
/// signal-container struct — see the module-level convention note.
#[derive(Clone, Copy)]
pub struct AppContext {
    /// Global canonical filesystem tree loaded from mounted content.
    pub global_fs: RwSignal<GlobalFs>,
    /// Current canonical working directory for shell surfaces.
    pub cwd: RwSignal<VirtualPath>,
    /// Wallet connection state.
    pub wallet: RwSignal<WalletState>,
    /// Installed browser wallet event listener handles. Stored so listener
    /// closures are not leaked and setup stays idempotent.
    wallet_event_listeners:
        StoredValue<Option<runtime::wallet::WalletEventListeners>, LocalStorage>,

    /// Current visual palette, mirrored to `html[data-theme]`.
    pub theme: RwSignal<&'static str>,

    /// Terminal state (history, commands).
    pub terminal: TerminalState,

    /// System filesystem with content plus synthetic `/.websh/state`.
    pub system_global_fs: Signal<Rc<GlobalFs>, LocalStorage>,
    /// Backend registry keyed by canonical mount roots.
    backends: StoredValue<BTreeMap<VirtualPath, StorageBackendRef>, LocalStorage>,
    /// Successful backend text reads scoped to the current runtime generation.
    content_text_cache: StoredValue<ContentTextCache, LocalStorage>,
    /// Backend text reads already in flight, keyed like the text cache.
    content_text_inflight: StoredValue<BTreeMap<ContentTextCacheKey, SharedTextRead>, LocalStorage>,
    /// Runtime mount declarations, load status, and scan jobs.
    pub mounts: RwSignal<runtime::MountLoadSet, LocalStorage>,
    /// Runtime generation used to ignore stale background mount scans.
    runtime_generation: RwSignal<u64>,
    root_request_sequence: StoredValue<u64, LocalStorage>,
    mount_cache: StoredValue<runtime::mount_cache::MountCacheRef, LocalStorage>,
    /// Browser-hydrated runtime state rendered under `/.websh/state`.
    pub runtime_state: RwSignal<RuntimeStateSnapshot>,
}

impl AppContext {
    /// Creates a new application context with default state.
    ///
    /// All signals are initialized to their default values:
    /// - Terminal: Empty history
    /// - Wallet: Disconnected
    /// - Filesystem: Empty
    pub fn new() -> Self {
        super::RuntimeServices::install_browser_persistence();
        let initial_load = super::RuntimeServices::bootstrap_runtime_load();
        let global_fs = RwSignal::new(initial_load.global_fs);
        let wallet = RwSignal::new(WalletState::default());
        let wallet_event_listeners = StoredValue::new_local(None);
        let runtime_state = RwSignal::new(super::RuntimeServices::runtime_state_snapshot());
        let system_global_fs = Signal::derive_local(move || {
            Rc::new(global_fs.with(|base| {
                wallet.with(|ws| {
                    runtime_state.with(|rs| websh_core::runtime::build_view_global_fs(base, ws, rs))
                })
            }))
        });

        let backends: StoredValue<BTreeMap<VirtualPath, StorageBackendRef>, LocalStorage> =
            StoredValue::new_local(initial_load.backends);
        let content_text_cache = StoredValue::new_local(ContentTextCache::default());
        let content_text_inflight = StoredValue::new_local(BTreeMap::new());
        let mounts = RwSignal::new_local(initial_load.mounts);
        let runtime_generation = RwSignal::new(0_u64);
        let root_request_sequence = StoredValue::new_local(0_u64);
        let theme = RwSignal::new(crate::render::theme::initial_theme());

        Self {
            // Shared state
            global_fs,
            cwd: RwSignal::new(VirtualPath::root()),
            wallet,
            wallet_event_listeners,

            theme,

            // Terminal state
            terminal: TerminalState::new(),

            // Runtime read state
            system_global_fs,
            backends,
            content_text_cache,
            content_text_inflight,
            mounts,
            runtime_generation,
            root_request_sequence,
            mount_cache: StoredValue::new_local(Rc::new(
                runtime::mount_cache::BrowserMountCache::default(),
            )),
            runtime_state,
        }
    }

    pub fn runtime_mounts_snapshot(&self) -> Vec<RuntimeMount> {
        self.mounts.with(|mounts| mounts.effective_mounts())
    }

    pub fn wallet_event_listeners_installed(&self) -> bool {
        self.wallet_event_listeners
            .with_value(|listeners| listeners.is_some())
    }

    pub fn install_wallet_event_listeners(&self, listeners: runtime::wallet::WalletEventListeners) {
        self.wallet_event_listeners.set_value(Some(listeners));
    }

    pub fn mount_status_for(&self, root: &VirtualPath) -> Option<runtime::MountLoadStatus> {
        self.mounts.with(|mounts| mounts.status(root))
    }

    pub fn mount_is_loaded(&self, root: &VirtualPath) -> bool {
        self.mounts.with(|mounts| mounts.is_loaded(root))
    }

    /// Gets the current prompt string for display.
    ///
    /// Format: `{username}@{app_name}:{path}`
    ///
    /// The username is derived from the wallet state:
    /// - ENS name if available
    /// - Shortened address (0x1234...5678) if connected
    /// - "guest" if disconnected
    pub fn get_prompt(&self, cwd: &VirtualPath) -> String {
        let display_path = display_path_for(cwd);
        let username = self.wallet.get().display_name();
        format!("{}@{}:{}", username, APP_NAME, display_path)
    }

    /// Look up the most specific registered public backend.
    pub fn backend_for_path(&self, path: &VirtualPath) -> Option<StorageBackendRef> {
        let owner = self
            .mounts
            .with_untracked(|mounts| mounts.owner(path).map(|entry| entry.declared.root.clone()))?;
        self.backend_for_mount_root(&owner)
    }

    /// Look up the backend at an exact accepted mount root.
    pub fn backend_for_mount_root(&self, root: &VirtualPath) -> Option<StorageBackendRef> {
        self.backends.with_value(|map| map.get(root).cloned())
    }

    /// Reactive content identity. Refresh progress and failures do not change this value.
    pub fn read_version(&self, path: &VirtualPath) -> (u64, u64) {
        (
            self.runtime_generation.get(),
            self.mounts
                .with(|mounts| mounts.owner(path).map_or(0, |entry| entry.content_revision)),
        )
    }

    pub(crate) fn current_read_version(&self, path: &VirtualPath) -> (u64, u64) {
        (
            self.runtime_generation(),
            self.mounts.with_untracked(|mounts| {
                mounts.owner(path).map_or(0, |entry| entry.content_revision)
            }),
        )
    }

    pub async fn read_text(&self, path: &VirtualPath) -> Result<String, ContentReadError> {
        for _ in 0..2 {
            let version = self.current_read_version(path);
            let result = self.read_text_at_version(path, version).await;
            if self.current_read_version(path) == version {
                return result;
            }
        }
        Err(ContentReadError::Obsolete { path: path.clone() })
    }

    async fn read_text_at_version(
        &self,
        path: &VirtualPath,
        version: (u64, u64),
    ) -> TextReadResult {
        let fs = self.view_fs_for_path(path);
        if let Some(text) = fs.read_inline_text(path) {
            return Ok(text);
        }
        let mount_root = self
            .mounts
            .with_untracked(|mounts| mounts.owner(path).map(|entry| entry.declared.root.clone()))
            .ok_or_else(|| ContentReadError::NoBackend { path: path.clone() })?;
        // Failed declaration boundaries must never fall through to an ancestor backend.
        if self.backend_for_mount_root(&mount_root).is_none() {
            return Err(ContentReadError::NoBackend { path: path.clone() });
        }
        let cache_key = ContentTextCacheKey {
            generation: version.0,
            revision: version.1,
            rel_path: path
                .strip_prefix(&mount_root)
                .expect("owning mount contains path")
                .to_string(),
            mount_root,
        };
        let mut cached = None;
        self.content_text_cache
            .update_value(|cache| cached = cache.get(&cache_key));
        if let Some(text) = cached {
            return Ok(text);
        }

        let mut read = None;
        self.content_text_inflight.update_value(|inflight| {
            if let Some(existing) = inflight.get(&cache_key) {
                read = Some(existing.clone());
                return;
            }
            let backends = self.backends.with_value(Clone::clone);
            let shared = shared_text_read(fs, backends, path.clone());
            inflight.insert(cache_key.clone(), shared.clone());
            read = Some(shared);
        });
        let read = read.expect("text read installed before await");
        let result = read.clone().await;
        if let Ok(text) = &result
            && self.current_read_version(path) == version
        {
            self.content_text_cache
                .update_value(|cache| cache.insert(cache_key.clone(), text.clone()));
        }
        self.content_text_inflight.update_value(|inflight| {
            // Another waiter may already have removed this request and installed a retry.
            if inflight
                .get(&cache_key)
                .is_some_and(|current| current.ptr_eq(&read))
            {
                inflight.remove(&cache_key);
            }
        });
        result
    }

    pub async fn read_bytes(&self, path: &VirtualPath) -> Result<Vec<u8>, ContentReadError> {
        for _ in 0..2 {
            let version = self.current_read_version(path);
            let fs = self.view_fs_for_path(path);
            if fs.read_inline_text(path).is_none() && self.backend_for_path(path).is_none() {
                return Err(ContentReadError::NoBackend { path: path.clone() });
            }
            let backends = self.backends.with_value(Clone::clone);
            let result = websh_core::filesystem::read_bytes(&fs, &backends, path).await;
            if self.current_read_version(path) == version {
                return result;
            }
        }
        Err(ContentReadError::Obsolete { path: path.clone() })
    }

    pub fn public_read_url(&self, path: &VirtualPath) -> Result<Option<String>, ContentReadError> {
        let fs = self.view_fs_for_path(path);
        if fs.read_inline_text(path).is_none() && self.backend_for_path(path).is_none() {
            return Err(ContentReadError::NoBackend { path: path.clone() });
        }
        let backends = self.backends.with_value(|map| map.clone());
        websh_core::filesystem::public_read_url(&fs, &backends, path)
    }

    fn view_fs_for_path(&self, path: &VirtualPath) -> Rc<GlobalFs> {
        if is_runtime_overlay_path(path) {
            self.system_global_fs.get()
        } else {
            Rc::new(self.global_fs.get())
        }
    }

    pub fn runtime_mount_for_path(&self, path: &VirtualPath) -> Option<RuntimeMount> {
        self.runtime_mounts_snapshot()
            .into_iter()
            .filter(|mount| mount.contains(path))
            .max_by_key(|mount| mount.root.as_str().len())
    }

    pub fn declared_mount_for_root(&self, root: &VirtualPath) -> Option<RuntimeMount> {
        self.mounts.with(|mounts| mounts.declared(root))
    }

    pub fn clear_text_cache(&self) {
        self.content_text_cache
            .update_value(ContentTextCache::clear);
        self.content_text_inflight.update_value(BTreeMap::clear);
    }

    pub fn evict_text_cache_mount(&self, mount_root: &VirtualPath) {
        self.content_text_cache
            .update_value(|cache| cache.evict_mount(mount_root));
        self.content_text_inflight
            .update_value(|inflight| evict_inflight_mount(inflight, mount_root));
    }

    pub fn runtime_generation(&self) -> u64 {
        self.runtime_generation.get_untracked()
    }

    pub fn mark_mount_loading(
        &self,
        root: &VirtualPath,
    ) -> Result<(RuntimeMount, u64), RuntimeServiceError> {
        let mut marked = None;
        self.mounts.update(|mounts| {
            marked = mounts.mark_loading(root);
        });
        marked.ok_or_else(|| RuntimeServiceError::MissingDeclaration { root: root.clone() })
    }

    pub(crate) fn mark_mount_failed(
        &self,
        root: &VirtualPath,
        error: impl Into<String>,
    ) -> Result<(), RuntimeServiceError> {
        let error = error.into();
        let mut marked = false;
        self.mounts.update(|mounts| {
            marked = mounts.mark_failed(root, error.clone());
        });
        if marked {
            Ok(())
        } else {
            Err(RuntimeServiceError::MissingDeclaration { root: root.clone() })
        }
    }

    pub fn begin_root_request(&self) -> u64 {
        let sequence = self.root_request_sequence.get_value().saturating_add(1);
        self.root_request_sequence.set_value(sequence);
        let _ = self.mark_mount_loading(&VirtualPath::root());
        sequence
    }

    pub fn accepts_root_request(&self, sequence: u64) -> bool {
        self.root_request_sequence.get_value() == sequence
    }

    pub fn apply_runtime_load(&self, load: RuntimeLoad) -> u64 {
        let generation = self.runtime_generation().saturating_add(1);
        batch(|| {
            self.clear_text_cache();
            self.backends.set_value(load.backends);
            self.runtime_generation.set(generation);
            self.global_fs.set(load.global_fs);
            self.mounts.set(load.mounts);
        });
        generation
    }

    #[cfg(test)]
    pub(crate) fn set_mount_cache(&self, cache: runtime::mount_cache::MountCacheRef) {
        self.mount_cache.set_value(cache);
    }

    pub fn mount_cache(&self) -> runtime::mount_cache::MountCacheRef {
        self.mount_cache.get_value()
    }

    pub fn mount_cache_descriptor(
        &self,
        root: &VirtualPath,
    ) -> Option<runtime::mount_cache::CacheDescriptor> {
        self.mounts.with_untracked(|mounts| {
            mounts
                .entries
                .get(root)
                .and_then(|entry| entry.cache_descriptor.clone())
        })
    }

    pub fn mount_attempt_is_current(
        &self,
        generation: u64,
        root: &VirtualPath,
        epoch: u64,
    ) -> bool {
        generation == self.runtime_generation()
            && self
                .mounts
                .with_untracked(|mounts| mounts.accepts_result(root, epoch))
    }

    pub fn apply_mount_scan_result(
        &self,
        generation: u64,
        result: runtime::MountScanResult,
    ) -> Result<bool, RuntimeServiceError> {
        self.apply_mount_snapshot(
            generation,
            result,
            runtime::mounts::SnapshotOrigin::Network,
            crate::platform::time::current_timestamp(),
            runtime::mounts::RefreshState::Idle,
        )
    }

    pub fn apply_mount_snapshot(
        &self,
        generation: u64,
        result: runtime::MountScanResult,
        origin: runtime::mounts::SnapshotOrigin,
        observed_at_ms: u64,
        refresh: runtime::mounts::RefreshState,
    ) -> Result<bool, RuntimeServiceError> {
        if generation != self.runtime_generation() {
            return Ok(false);
        }

        let root = result.mount.root.clone();
        let label = result.mount.label.clone();
        let epoch = result.epoch;
        if !self
            .mounts
            .with_untracked(|mounts| mounts.accepts_result(&root, epoch))
        {
            return Ok(false);
        }

        if origin == runtime::mounts::SnapshotOrigin::Cache
            && !self
                .mounts
                .with_untracked(|mounts| mounts.accepts_cache(&root, epoch))
        {
            return Ok(false);
        }

        match result.scan {
            Ok(scan) => {
                let total_files = scan.files.len();
                let mut global = self.global_fs.get_untracked();
                if let Err(error) = global.replace_scanned_subtree(root.clone(), &scan) {
                    let message = format!("mount {label}: {error}");
                    if origin == runtime::mounts::SnapshotOrigin::Network {
                        self.mounts.update(|mounts| {
                            mounts.mark_failed_if_current(&root, epoch, message.clone());
                        });
                    }
                    return Err(RuntimeServiceError::ReplaceScannedSubtree {
                        label,
                        source: error,
                    });
                }
                let failed_descendants = self
                    .mounts
                    .with_untracked(|mounts| mounts.failed_roots_under(&root));
                for failed_root in failed_descendants {
                    let _ = global.reserve_mount_point(failed_root);
                }
                if let Err(source) = websh_core::filesystem::RouteCatalog::from_global_fs(&global) {
                    if origin == runtime::mounts::SnapshotOrigin::Network {
                        self.mounts.update(|mounts| {
                            mounts.mark_failed_if_current(&root, epoch, source.to_string());
                        });
                    }
                    return Err(RuntimeServiceError::InvalidRoutes { source });
                }
                batch(|| {
                    self.evict_text_cache_mount(&root);
                    self.backends.update_value(|backends| {
                        backends.insert(root.clone(), result.backend);
                    });
                    self.global_fs.set(global);
                    self.mounts.update(|mounts| {
                        mounts.publish(&root, epoch, total_files, observed_at_ms, origin, refresh);
                    });
                });
                Ok(true)
            }
            Err(error) => {
                self.mounts.update(|mounts| {
                    mounts.mark_failed_if_current(&root, epoch, error.to_string());
                });
                Ok(false)
            }
        }
    }
}

fn shared_text_read(
    fs: Rc<GlobalFs>,
    backends: BTreeMap<VirtualPath, StorageBackendRef>,
    path: VirtualPath,
) -> SharedTextRead {
    let read: LocalBoxFuture<'static, TextReadResult> =
        Box::pin(async move { websh_core::filesystem::read_text(&fs, &backends, &path).await });
    read.shared()
}

fn evict_inflight_mount(
    inflight: &mut BTreeMap<ContentTextCacheKey, SharedTextRead>,
    mount_root: &VirtualPath,
) {
    let keys = inflight
        .keys()
        .filter(|key| &key.mount_root == mount_root)
        .cloned()
        .collect::<Vec<_>>();
    for key in keys {
        inflight.remove(&key);
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use gloo_timers::future::TimeoutFuture;
    use leptos::prelude::Owner;
    use std::cell::Cell;
    use std::rc::Rc;
    use wasm_bindgen_test::*;
    use websh_core::domain::{EntryExtensions, Fields, NodeKind, NodeMetadata, SCHEMA_VERSION};
    use websh_core::filesystem::MountError;
    use websh_core::ports::{
        LocalBoxFuture, ScannedSubtree, StorageBackend, StorageBackendRef, StorageResult,
    };

    wasm_bindgen_test_configure!(run_in_browser);

    struct CountingBackend {
        reads: Rc<Cell<u32>>,
        text: String,
        delay_ms: u32,
    }

    impl StorageBackend for CountingBackend {
        fn backend_type(&self) -> &'static str {
            "counting"
        }

        fn scan(&self) -> LocalBoxFuture<'_, StorageResult<ScannedSubtree>> {
            Box::pin(async { Ok(ScannedSubtree::default()) })
        }

        fn read_text<'a>(
            &'a self,
            _rel_path: &'a str,
        ) -> LocalBoxFuture<'a, StorageResult<String>> {
            self.reads.set(self.reads.get() + 1);
            let text = self.text.clone();
            let delay_ms = self.delay_ms;
            Box::pin(async move {
                if delay_ms > 0 {
                    TimeoutFuture::new(delay_ms).await;
                }
                Ok(text)
            })
        }

        fn read_bytes<'a>(
            &'a self,
            rel_path: &'a str,
        ) -> LocalBoxFuture<'a, StorageResult<Vec<u8>>> {
            Box::pin(async move { self.read_text(rel_path).await.map(String::into_bytes) })
        }
    }

    fn counting_backend(reads: Rc<Cell<u32>>, text: &str, delay_ms: u32) -> StorageBackendRef {
        Rc::new(CountingBackend {
            reads,
            text: text.to_string(),
            delay_ms,
        })
    }

    fn root_mount() -> RuntimeMount {
        RuntimeMount::new(
            VirtualPath::root(),
            "~",
            websh_core::domain::RuntimeBackendKind::GitHub,
        )
    }

    fn apply_loaded_root_backend(ctx: AppContext, backend: StorageBackendRef) {
        let mut backends = BTreeMap::new();
        backends.insert(VirtualPath::root(), backend);
        let mut mounts = runtime::MountLoadSet::empty();
        mounts.insert_loaded(root_mount(), 0);

        ctx.apply_runtime_load(RuntimeLoad {
            global_fs: GlobalFs::empty(),
            backends,
            total_files: 0,
            mounts,
        });
    }

    fn data_meta() -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Data,
            bundle: None,
            authored: Fields::default(),
            derived: Fields::default(),
        }
    }

    #[wasm_bindgen_test(async)]
    async fn read_text_caches_backend_results_within_generation_and_inline_text_bypasses_cache() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let path = VirtualPath::from_absolute("/cached.txt").expect("path");

        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, counting_backend(reads.clone(), "remote", 0));
            ctx.clear_text_cache();
            ctx
        });

        assert_eq!(ctx.read_text(&path).await.unwrap(), "remote");
        assert_eq!(ctx.read_text(&path).await.unwrap(), "remote");
        assert_eq!(reads.get(), 1);

        ctx.global_fs.update(|fs| {
            fs.upsert_file(
                path.clone(),
                "pending".to_string(),
                data_meta(),
                EntryExtensions::default(),
            );
        });

        assert_eq!(ctx.read_text(&path).await.unwrap(), "pending");
        assert_eq!(reads.get(), 1);
    }

    #[wasm_bindgen_test(async)]
    async fn concurrent_same_generation_read_text_calls_share_one_backend_request() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let path = VirtualPath::from_absolute("/shared.txt").expect("path");

        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, counting_backend(reads.clone(), "shared", 20));
            ctx.clear_text_cache();
            ctx
        });

        let (left, right) = futures_util::join!(ctx.read_text(&path), ctx.read_text(&path));

        assert_eq!(left.unwrap(), "shared");
        assert_eq!(right.unwrap(), "shared");
        assert_eq!(reads.get(), 1);
    }

    #[wasm_bindgen_test(async)]
    async fn generation_change_retries_inflight_text_read_without_caching_stale_result() {
        let owner = Owner::new();
        let stale_reads = Rc::new(Cell::new(0));
        let fresh_reads = Rc::new(Cell::new(0));
        let path = VirtualPath::from_absolute("/generation.txt").expect("path");

        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, counting_backend(stale_reads.clone(), "stale", 50));
            ctx.clear_text_cache();
            ctx
        });

        let fresh_backend = counting_backend(fresh_reads.clone(), "fresh", 0);
        let swap_generation = async move {
            while stale_reads.get() == 0 {
                TimeoutFuture::new(1).await;
            }
            apply_loaded_root_backend(ctx, fresh_backend);
        };

        let (value, ()) = futures_util::join!(ctx.read_text(&path), swap_generation);
        let value = value.expect("read should retry against current generation");
        assert_eq!(value, "fresh");
        assert_eq!(fresh_reads.get(), 1);

        assert_eq!(ctx.read_text(&path).await.unwrap(), "fresh");
        assert_eq!(fresh_reads.get(), 1);
    }

    #[wasm_bindgen_test]
    fn root_mount_status_tracks_root_runtime_loads() {
        let owner = Owner::new();
        owner.with(|| {
            let ctx = AppContext::new();
            let root = VirtualPath::root();

            assert!(matches!(
                ctx.mount_status_for(&root),
                Some(runtime::MountLoadStatus::Loading)
            ));
            assert!(!ctx.mount_is_loaded(&root));

            let root_mount = root_mount();
            apply_loaded_root_backend(ctx, counting_backend(Rc::new(Cell::new(0)), "", 0));

            assert!(ctx.mount_is_loaded(&root));
            assert!(matches!(
                ctx.mount_status_for(&root),
                Some(runtime::MountLoadStatus::Available { .. })
            ));

            let mut failed_mounts = runtime::MountLoadSet::empty();
            failed_mounts.insert_declared_loading(root_mount);
            ctx.apply_runtime_load(RuntimeLoad {
                global_fs: GlobalFs::empty(),
                backends: BTreeMap::new(),
                total_files: 0,
                mounts: failed_mounts,
            });
            ctx.mark_mount_failed(&root, "manifest unavailable")
                .expect("root mount should be declared");

            assert!(matches!(
                ctx.mount_status_for(&root),
                Some(runtime::MountLoadStatus::Failed { error, .. })
                    if error == "manifest unavailable"
            ));
        });
    }

    #[wasm_bindgen_test]
    fn mount_apply_failure_marks_mount_failed() {
        let owner = Owner::new();
        owner.with(|| {
            let root = VirtualPath::from_absolute("/db").expect("root");
            let mount = RuntimeMount::new(
                root.clone(),
                "db",
                websh_core::domain::RuntimeBackendKind::GitHub,
            );
            let backend = counting_backend(Rc::new(Cell::new(0)), "", 0);

            let ctx = AppContext::new();
            let mut mounts = runtime::MountLoadSet::empty();
            mounts.insert_loading(mount.clone(), backend.clone());
            ctx.mounts.set(mounts);
            ctx.global_fs.update(|fs| {
                fs.upsert_file(
                    root.clone(),
                    "not a directory".to_string(),
                    data_meta(),
                    EntryExtensions::default(),
                );
            });

            let result = runtime::MountScanResult {
                mount,
                backend,
                epoch: 0,
                scan: Ok(ScannedSubtree::default()),
            };

            let error = ctx
                .apply_mount_scan_result(ctx.runtime_generation(), result)
                .expect_err("mount apply should fail");
            assert!(matches!(
                error,
                RuntimeServiceError::ReplaceScannedSubtree {
                    source: MountError::MountPointIsFile { .. },
                    ..
                }
            ));
            assert!(matches!(
                ctx.mount_status_for(&root),
                Some(runtime::MountLoadStatus::Failed { .. })
            ));
        });
    }
    #[wasm_bindgen_test]
    fn invalid_refresh_keeps_installed_tree_until_valid_empty_replacement() {
        let owner = Owner::new();
        owner.with(|| {
            let ctx = AppContext::new();
            let backend = counting_backend(Rc::new(Cell::new(0)), "body", 0);
            apply_loaded_root_backend(ctx, backend.clone());
            let path = VirtualPath::from_absolute("/kept.md").unwrap();
            ctx.global_fs.update(|fs| {
                fs.upsert_file(
                    path.clone(),
                    "kept".into(),
                    data_meta(),
                    EntryExtensions::default(),
                )
            });
            let version = ctx.current_read_version(&path);
            let (mount, epoch) = ctx.mark_mount_loading(&VirtualPath::root()).unwrap();
            let scan = ScannedSubtree {
                files: vec![websh_core::ports::ScannedFile {
                    path: "../outside".into(),
                    meta: data_meta(),
                    extensions: EntryExtensions::default(),
                }],
                directories: vec![],
            };
            assert!(
                ctx.apply_mount_scan_result(
                    ctx.runtime_generation(),
                    runtime::MountScanResult {
                        mount,
                        epoch,
                        backend: backend.clone(),
                        scan: Ok(scan)
                    }
                )
                .is_err()
            );
            assert_eq!(ctx.current_read_version(&path), version);
            assert!(
                ctx.global_fs
                    .with_untracked(|fs| fs.get_entry(&path).is_some())
            );
            assert!(matches!(
                ctx.mount_status_for(&VirtualPath::root()),
                Some(runtime::MountLoadStatus::Available {
                    refresh: runtime::mounts::RefreshState::Failed(_),
                    ..
                })
            ));
            publish_root(ctx, backend);
            assert!(
                ctx.global_fs
                    .with_untracked(|fs| fs.get_entry(&path).is_none())
            );
            assert_ne!(ctx.current_read_version(&path), version);
            assert!(matches!(
                ctx.mount_status_for(&VirtualPath::root()),
                Some(runtime::MountLoadStatus::Available {
                    total_files: 0,
                    refresh: runtime::mounts::RefreshState::Idle,
                    ..
                })
            ));
        });
    }

    fn publish_root(ctx: AppContext, backend: StorageBackendRef) {
        let (mount, epoch) = ctx.mark_mount_loading(&VirtualPath::root()).unwrap();
        assert!(
            ctx.apply_mount_scan_result(
                ctx.runtime_generation(),
                runtime::MountScanResult {
                    mount,
                    backend,
                    epoch,
                    scan: Ok(ScannedSubtree::default()),
                }
            )
            .unwrap()
        );
    }

    #[wasm_bindgen_test(async)]
    async fn refresh_start_and_failure_keep_content_revision_and_cached_body() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let backend = counting_backend(reads.clone(), "kept", 0);
        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, backend.clone());
            ctx
        });
        let path = VirtualPath::from_absolute("/kept.txt").unwrap();
        assert_eq!(ctx.read_text(&path).await.unwrap(), "kept");
        let version = ctx.current_read_version(&path);
        let (mount, epoch) = ctx.mark_mount_loading(&VirtualPath::root()).unwrap();
        assert_eq!(ctx.current_read_version(&path), version);
        assert_eq!(ctx.read_text(&path).await.unwrap(), "kept");
        ctx.apply_mount_scan_result(
            ctx.runtime_generation(),
            runtime::MountScanResult {
                mount,
                backend,
                epoch,
                scan: Err(websh_core::ports::StorageError::Network {
                    message: "offline".into(),
                }),
            },
        )
        .unwrap();
        assert_eq!(ctx.current_read_version(&path), version);
        assert_eq!(ctx.read_text(&path).await.unwrap(), "kept");
        assert_eq!(reads.get(), 1);
        assert!(matches!(
            ctx.mount_status_for(&VirtualPath::root()),
            Some(runtime::MountLoadStatus::Available {
                refresh: runtime::mounts::RefreshState::Failed(_),
                ..
            })
        ));
    }

    #[wasm_bindgen_test(async)]
    async fn accepted_refresh_retries_old_reads_against_new_revision() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, counting_backend(reads.clone(), "old", 40));
            ctx
        });
        let path = VirtualPath::from_absolute("/updated.txt").unwrap();
        let generation = ctx.runtime_generation();
        let refresh = async {
            while reads.get() == 0 {
                TimeoutFuture::new(1).await;
            }
            publish_root(ctx, counting_backend(Rc::new(Cell::new(0)), "new", 0));
        };
        let (result, ()) = futures_util::join!(ctx.read_text(&path), refresh);
        assert_eq!(result.unwrap(), "new");
        assert_eq!(ctx.runtime_generation(), generation);
        assert_eq!(ctx.read_text(&path).await.unwrap(), "new");
    }

    #[wasm_bindgen_test(async)]
    async fn accepted_refresh_retries_binary_read_against_current_revision() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, counting_backend(reads.clone(), "old bytes", 30));
            ctx
        });
        let path = VirtualPath::from_absolute("/image.png").unwrap();
        let refresh = async {
            while reads.get() == 0 {
                TimeoutFuture::new(1).await;
            }
            publish_root(ctx, counting_backend(Rc::new(Cell::new(0)), "new bytes", 0));
        };
        let (result, ()) = futures_util::join!(ctx.read_bytes(&path), refresh);
        assert_eq!(result.unwrap(), b"new bytes");
    }

    #[wasm_bindgen_test(async)]
    async fn twice_obsoleted_read_returns_cancellation_instead_of_stale_text() {
        let owner = Owner::new();
        let first = Rc::new(Cell::new(0));
        let second = Rc::new(Cell::new(0));
        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, counting_backend(first.clone(), "old", 25));
            ctx
        });
        let path = VirtualPath::from_absolute("/changing.txt").unwrap();
        let refresh = async {
            while first.get() == 0 {
                TimeoutFuture::new(1).await;
            }
            publish_root(ctx, counting_backend(second.clone(), "middle", 25));
            while second.get() == 0 {
                TimeoutFuture::new(1).await;
            }
            publish_root(ctx, counting_backend(Rc::new(Cell::new(0)), "latest", 0));
        };
        let (result, ()) = futures_util::join!(ctx.read_text(&path), refresh);
        assert!(matches!(result, Err(ContentReadError::Obsolete { .. })));
        assert_eq!(ctx.read_text(&path).await.unwrap(), "latest");
    }

    #[wasm_bindgen_test(async)]
    async fn failed_declaration_never_reads_ancestor_backend() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            apply_loaded_root_backend(ctx, counting_backend(reads.clone(), "wrong source", 0));
            ctx
        });
        let root = VirtualPath::from_absolute("/invalid").unwrap();
        ctx.mounts.update(|mounts| {
            mounts.insert_failed(
                RuntimeMount::new(
                    root,
                    "invalid",
                    websh_core::domain::RuntimeBackendKind::GitHub,
                ),
                "bad declaration",
            )
        });
        let path = VirtualPath::from_absolute("/invalid/file.txt").unwrap();
        assert!(matches!(
            ctx.read_text(&path).await,
            Err(ContentReadError::NoBackend { .. })
        ));
        assert!(matches!(
            ctx.read_bytes(&path).await,
            Err(ContentReadError::NoBackend { .. })
        ));
        assert!(matches!(
            ctx.public_read_url(&path),
            Err(ContentReadError::NoBackend { .. })
        ));
        assert_eq!(reads.get(), 0);
    }
}
