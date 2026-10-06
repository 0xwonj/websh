//! Sole owner of installed content, mount transitions, backend reads, and listing cache.
use super::RuntimeError;
use super::content_cache::{ContentTextCache, ContentTextCacheKey};
use crate::runtime::{self, RuntimeLoad};
use futures_util::FutureExt;
use leptos::prelude::*;
use std::collections::BTreeMap;
use std::rc::Rc;
use websh_core::domain::{RuntimeMount, VirtualPath};
use websh_core::filesystem::{ContentReadError, GlobalFs, Snapshot};
use websh_core::ports::{LocalBoxFuture, StorageBackendRef};

type TextReadResult = Result<String, ContentReadError>;
type SharedTextRead = futures_util::future::Shared<LocalBoxFuture<'static, TextReadResult>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadStamp {
    pub generation: u64,
    pub revision: u64,
}

#[derive(Clone, Copy)]
pub struct Content {
    historical: bool,
    root_release: RwSignal<Option<Rc<websh_core::publication::VerifiedRelease>>, LocalStorage>,
    pub snapshot: ReadSignal<Rc<Snapshot>, LocalStorage>,
    published: RwSignal<Rc<Snapshot>, LocalStorage>,
    pub mounts: ReadSignal<runtime::MountLoadSet, LocalStorage>,
    mount_state: RwSignal<runtime::MountLoadSet, LocalStorage>,
    backends: StoredValue<BTreeMap<VirtualPath, StorageBackendRef>, LocalStorage>,
    content_text_cache: StoredValue<ContentTextCache, LocalStorage>,
    content_text_inflight: StoredValue<BTreeMap<ContentTextCacheKey, SharedTextRead>, LocalStorage>,
    runtime_generation: RwSignal<u64>,
    root_request_sequence: StoredValue<u64, LocalStorage>,
    mount_cache: StoredValue<runtime::mount_cache::MountCacheRef, LocalStorage>,
}

impl Content {
    pub fn new(load: RuntimeLoad) -> Self {
        let published = RwSignal::new_local(Rc::new(load.snapshot));
        let mount_state = RwSignal::new_local(load.mounts);
        Self {
            historical: runtime::loader::requested_snapshot()
                .is_ok_and(|selection| selection.is_some()),
            root_release: RwSignal::new_local(load.release),
            snapshot: published.read_only(),
            published,
            mounts: mount_state.read_only(),
            mount_state,
            backends: StoredValue::new_local(load.backends),
            content_text_cache: StoredValue::new_local(ContentTextCache::default()),
            content_text_inflight: StoredValue::new_local(BTreeMap::new()),
            runtime_generation: RwSignal::new(0),
            root_request_sequence: StoredValue::new_local(0),
            mount_cache: StoredValue::new_local(Rc::new(
                runtime::mount_cache::BrowserMountCache::default(),
            )),
        }
    }

    pub fn is_historical(&self) -> bool {
        self.historical
    }

    pub fn snapshot_url(&self) -> Option<String> {
        let release = self.release()?;
        let source = self
            .backend_for_mount_root(&VirtualPath::root())?
            .cache_snapshot()?;
        let href = web_sys::window()?.location().href().ok()?;
        let url = web_sys::Url::new(&href).ok()?;
        url.set_search(&format!(
            "?content={}&release={}",
            source.commit,
            release.id()
        ));
        Some(url.href())
    }

    pub fn release(&self) -> Option<Rc<websh_core::publication::VerifiedRelease>> {
        self.root_release.get()
    }

    pub fn home(&self) -> Option<websh_core::publication::HomeProjection> {
        self.root_release
            .with(|release| release.as_ref().map(|r| r.release().home.clone()))
    }

    pub fn issued_at(&self) -> Option<u64> {
        self.root_release
            .with(|release| release.as_ref().map(|r| r.release().issued_at))
    }

    pub fn with_fs<T>(&self, f: impl FnOnce(&GlobalFs) -> T) -> T {
        self.snapshot.with(|snapshot| f(snapshot.fs()))
    }

    pub fn with_fs_untracked<T>(&self, f: impl FnOnce(&GlobalFs) -> T) -> T {
        self.snapshot.with_untracked(|snapshot| f(snapshot.fs()))
    }

    #[cfg(test)]
    fn update_fixture(&self, f: impl FnOnce(&mut GlobalFs)) {
        let mut fs = self.with_fs_untracked(Clone::clone);
        f(&mut fs);
        self.published.set(Rc::new(Snapshot::new(fs).unwrap()));
    }

    pub fn runtime_mounts_snapshot(&self) -> Vec<RuntimeMount> {
        self.mounts.with(|mounts| mounts.effective_mounts())
    }

    pub fn mount_status_for(&self, root: &VirtualPath) -> Option<runtime::MountLoadStatus> {
        self.mounts.with(|mounts| mounts.status(root))
    }

    pub fn mount_is_loaded(&self, root: &VirtualPath) -> bool {
        self.mounts.with(|mounts| mounts.is_loaded(root))
    }

    /// Look up the backend at an exact accepted mount root.
    pub fn backend_for_mount_root(&self, root: &VirtualPath) -> Option<StorageBackendRef> {
        self.backends.with_value(|map| map.get(root).cloned())
    }

    /// Reactive content identity. Refresh progress and failures do not change this value.
    pub fn read_version(&self, path: &VirtualPath) -> ReadStamp {
        ReadStamp {
            generation: self.mounts.with(|mounts| mounts.generation(path)),
            revision: self.mounts.with(|mounts| mounts.revision(path)),
        }
    }

    pub(crate) fn current_read_version(&self, path: &VirtualPath) -> ReadStamp {
        ReadStamp {
            generation: self.mounts.with_untracked(|mounts| mounts.generation(path)),
            revision: self.mounts.with_untracked(|mounts| mounts.revision(path)),
        }
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

    async fn read_text_at_version(&self, path: &VirtualPath, version: ReadStamp) -> TextReadResult {
        let (mount_root, backend, rel_path) = self.source(path)?;
        let cache_key = ContentTextCacheKey {
            generation: version.generation,
            revision: version.revision,
            rel_path,
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
            let shared = shared_text_read(backend, cache_key.rel_path.clone());
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
            let stamp = self.current_read_version(path);
            let (_, backend, rel_path) = self.source(path)?;
            let result = backend.read_bytes(&rel_path).await.map_err(Into::into);
            if self.current_read_version(path) == stamp {
                return result;
            }
        }
        Err(ContentReadError::Obsolete { path: path.clone() })
    }

    fn source(
        &self,
        path: &VirtualPath,
    ) -> Result<(VirtualPath, StorageBackendRef, String), ContentReadError> {
        let root = self
            .mounts
            .with_untracked(|mounts| mounts.owner(path).map(|entry| entry.declared.root.clone()))
            .ok_or_else(|| ContentReadError::NoBackend { path: path.clone() })?;
        let backend = self
            .backend_for_mount_root(&root)
            .ok_or_else(|| ContentReadError::NoBackend { path: path.clone() })?;
        let rel_path = path
            .strip_prefix(&root)
            .expect("owning mount contains path")
            .to_owned();
        Ok((root, backend, rel_path))
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
    ) -> Result<(RuntimeMount, u64), RuntimeError> {
        let mut marked = None;
        self.mount_state.update(|mounts| {
            marked = mounts.mark_loading(root);
        });
        marked.ok_or_else(|| RuntimeError::MissingDeclaration { root: root.clone() })
    }

    pub(crate) fn mark_mount_failed(
        &self,
        root: &VirtualPath,
        error: impl Into<String>,
    ) -> Result<(), RuntimeError> {
        let error = error.into();
        let mut marked = false;
        self.mount_state.update(|mounts| {
            marked = mounts.mark_failed(root, error.clone());
        });
        if marked {
            Ok(())
        } else {
            Err(RuntimeError::MissingDeclaration { root: root.clone() })
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

    pub fn apply_runtime_load(&self, mut load: RuntimeLoad) -> u64 {
        if let Some(incoming) = &load.release
            && self
                .root_release
                .with_untracked(|old| old.as_ref().is_some_and(|old| old.id() == incoming.id()))
        {
            self.mount_state
                .update(|mounts| mounts.finish_unchanged_root(&load.mounts));
            return self.runtime_generation();
        }
        let retained = self
            .mounts
            .with_untracked(|previous| load.mounts.preserve_loaded(previous));
        if !retained.is_empty() {
            let mut fs = load.snapshot.fs().clone();
            for root in &retained {
                if let Some(entry) = self.with_fs_untracked(|old| old.get_entry(root).cloned()) {
                    fs.replace_subtree(root.clone(), entry);
                }
                if let Some(backend) = self.backend_for_mount_root(root) {
                    load.backends.insert(root.clone(), backend);
                }
            }
            load.snapshot = Snapshot::new(fs).expect("unchanged mounts retain validated routes");
        }
        let generation = self.runtime_generation().saturating_add(1);
        load.mounts.assign_generation(generation, &retained);
        let changed: Vec<_> = self
            .mounts
            .with_untracked(|mounts| mounts.effective_mounts())
            .into_iter()
            .filter(|mount| !retained.contains(&mount.root))
            .collect();
        batch(|| {
            for mount in changed {
                self.evict_text_cache_mount(&mount.root);
            }
            self.backends.set_value(load.backends);
            self.root_release.set(load.release);
            self.runtime_generation.set(generation);
            self.published.set(Rc::new(load.snapshot));
            self.mount_state.set(load.mounts);
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
        self.mounts
            .with_untracked(|mounts| mounts.cache_descriptor(root))
    }

    pub fn mount_attempt_is_current(
        &self,
        generation: u64,
        root: &VirtualPath,
        epoch: u64,
    ) -> bool {
        self.mounts
            .with_untracked(|mounts| mounts.accepts_attempt(generation, root, epoch))
    }

    pub fn apply_mount_scan_result(
        &self,
        generation: u64,
        result: runtime::MountScanResult,
    ) -> Result<bool, RuntimeError> {
        let root = result.mount.root.clone();
        let epoch = result.epoch;
        let outcome = self.apply_mount_snapshot(
            generation,
            result,
            runtime::mounts::SnapshotOrigin::Network,
            crate::platform::time::current_timestamp(),
            runtime::mounts::RefreshState::Idle,
        );
        if let Err(error) = &outcome {
            self.fail_attempt(generation, &root, epoch, error.to_string());
        }
        outcome
    }

    pub(crate) fn fail_attempt(
        &self,
        generation: u64,
        root: &VirtualPath,
        epoch: u64,
        error: String,
    ) {
        if self.mount_attempt_is_current(generation, root, epoch) {
            self.mount_state.update(|mounts| {
                mounts.mark_failed_if_current(root, epoch, error);
            });
        }
    }

    pub fn apply_mount_snapshot(
        &self,
        generation: u64,
        result: runtime::MountScanResult,
        origin: runtime::mounts::SnapshotOrigin,
        observed_at_ms: u64,
        refresh: runtime::mounts::RefreshState,
    ) -> Result<bool, RuntimeError> {
        let root = result.mount.root.clone();
        let label = result.mount.label.clone();
        let epoch = result.epoch;
        if !self
            .mounts
            .with_untracked(|mounts| mounts.accepts_attempt(generation, &root, epoch))
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
                let unchanged = self
                    .backend_for_mount_root(&root)
                    .and_then(|backend| backend.cache_snapshot())
                    .zip(result.backend.cache_snapshot())
                    .is_some_and(|(old, new)| {
                        old.manifest == new.manifest && old.signature == new.signature
                    });
                if unchanged {
                    self.mount_state.update(|mounts| {
                        mounts.refresh_finished(&root, total_files, observed_at_ms, origin, refresh)
                    });
                    return Ok(true);
                }
                let mut global = self.with_fs_untracked(Clone::clone);
                global
                    .replace_scanned_subtree(root.clone(), &scan)
                    .map_err(|source| RuntimeError::ReplaceScannedSubtree { label, source })?;
                let failed_descendants = self
                    .mounts
                    .with_untracked(|mounts| mounts.failed_roots_under(&root));
                for failed_root in failed_descendants {
                    let _ = global.reserve_mount_point(failed_root);
                }
                let snapshot = Snapshot::new(global)
                    .map_err(|source| RuntimeError::InvalidRoutes { source })?;
                batch(|| {
                    self.evict_text_cache_mount(&root);
                    self.backends.update_value(|backends| {
                        backends.insert(root.clone(), result.backend);
                    });
                    self.published.set(Rc::new(snapshot));
                    self.mount_state.update(|mounts| {
                        mounts.publish(&root, epoch, total_files, observed_at_ms, origin, refresh);
                    });
                });
                Ok(true)
            }
            Err(error) => {
                self.mount_state.update(|mounts| {
                    mounts.mark_failed_if_current(&root, epoch, error.to_string());
                });
                Ok(false)
            }
        }
    }
}

fn shared_text_read(backend: StorageBackendRef, rel_path: String) -> SharedTextRead {
    let read: LocalBoxFuture<'static, TextReadResult> =
        Box::pin(async move { backend.read_text(&rel_path).await.map_err(Into::into) });
    read.shared()
}

fn evict_inflight_mount(
    inflight: &mut BTreeMap<ContentTextCacheKey, SharedTextRead>,
    root: &VirtualPath,
) {
    inflight.retain(|key, _| &key.mount_root != root);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gloo_timers::future::TimeoutFuture;
    use leptos::prelude::Owner;
    use std::cell::Cell;
    use std::rc::Rc;
    use wasm_bindgen_test::*;
    use websh_core::domain::{
        AuthoredMetadata, DerivedMetadata, EntryExtensions, NodeKind, NodeMetadata,
    };
    use websh_core::filesystem::MountError;
    use websh_core::ports::{
        LocalBoxFuture, ScannedSubtree, StorageBackend, StorageBackendRef, StorageResult,
    };

    struct CountingBackend {
        reads: Rc<Cell<u32>>,
        text: String,
        delay_ms: u32,
    }

    impl StorageBackend for CountingBackend {
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
        RuntimeMount::new(VirtualPath::root(), "~")
    }

    fn apply_loaded_root_backend(ctx: Content, backend: StorageBackendRef) {
        let mut backends = BTreeMap::new();
        backends.insert(VirtualPath::root(), backend);
        let mut mounts = runtime::MountLoadSet::empty();
        mounts.insert_loaded(root_mount(), 0);

        ctx.apply_runtime_load(RuntimeLoad {
            release: None,
            snapshot: Snapshot::new(GlobalFs::empty()).unwrap(),
            backends,
            total_files: 0,
            mounts,
        });
    }

    fn data_meta() -> NodeMetadata {
        NodeMetadata {
            kind: NodeKind::Data,
            bundle: None,
            authored: AuthoredMetadata::default(),
            derived: DerivedMetadata::default(),
        }
    }

    #[wasm_bindgen_test(async)]
    async fn read_text_caches_backend_results_within_generation() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let path = VirtualPath::from_absolute("/cached.txt").expect("path");

        let ctx = owner.with(|| {
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
            apply_loaded_root_backend(ctx, counting_backend(reads.clone(), "remote", 0));
            ctx.clear_text_cache();
            ctx
        });

        assert_eq!(ctx.read_text(&path).await.unwrap(), "remote");
        assert_eq!(ctx.read_text(&path).await.unwrap(), "remote");
        assert_eq!(reads.get(), 1);
    }

    #[wasm_bindgen_test(async)]
    async fn concurrent_same_generation_read_text_calls_share_one_backend_request() {
        let owner = Owner::new();
        let reads = Rc::new(Cell::new(0));
        let path = VirtualPath::from_absolute("/shared.txt").expect("path");

        let ctx = owner.with(|| {
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
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
                release: None,
                snapshot: Snapshot::new(GlobalFs::empty()).unwrap(),
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
            let mount = RuntimeMount::new(root.clone(), "db");
            let backend = counting_backend(Rc::new(Cell::new(0)), "", 0);

            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
            let mut mounts = runtime::MountLoadSet::empty();
            mounts.insert_loading(mount.clone(), backend.clone());
            ctx.mount_state.set(mounts);
            ctx.update_fixture(|fs| {
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
                RuntimeError::ReplaceScannedSubtree {
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
            let backend = counting_backend(Rc::new(Cell::new(0)), "body", 0);
            apply_loaded_root_backend(ctx, backend.clone());
            let path = VirtualPath::from_absolute("/kept.md").unwrap();
            ctx.update_fixture(|fs| {
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
            assert!(ctx.with_fs_untracked(|fs| fs.get_entry(&path).is_some()));
            assert!(matches!(
                ctx.mount_status_for(&VirtualPath::root()),
                Some(runtime::MountLoadStatus::Available {
                    refresh: runtime::mounts::RefreshState::Failed(_),
                    ..
                })
            ));
            publish_root(ctx, backend);
            assert!(ctx.with_fs_untracked(|fs| fs.get_entry(&path).is_none()));
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

    fn publish_root(ctx: Content, backend: StorageBackendRef) {
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
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
            let ctx = Content::new(runtime::loader::bootstrap_runtime_load());
            apply_loaded_root_backend(ctx, counting_backend(reads.clone(), "wrong source", 0));
            ctx
        });
        let root = VirtualPath::from_absolute("/invalid").unwrap();
        ctx.mount_state.update(|mounts| {
            mounts.insert_failed(RuntimeMount::new(root, "invalid"), "bad declaration")
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
        assert_eq!(reads.get(), 0);
    }
}
