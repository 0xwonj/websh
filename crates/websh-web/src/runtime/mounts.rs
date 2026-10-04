use std::collections::BTreeMap;

use websh_core::domain::{RuntimeMount, VirtualPath};
use websh_core::ports::{ScannedSubtree, StorageBackendRef, StorageError};

#[derive(Clone, Default)]
pub struct MountLoadSet {
    pub entries: BTreeMap<VirtualPath, MountEntry>,
    pub scan_jobs: Vec<MountScanJob>,
    rejected_entries: Vec<MountEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountEntry {
    pub declared: RuntimeMount,
    /// Request identity changes on refresh; content identity changes only on publication.
    pub epoch: u64,
    pub content_revision: u64,
    pub cache_descriptor: Option<super::mount_cache::CacheDescriptor>,
    pub status: MountLoadStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotOrigin {
    Cache,
    Network,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RefreshState {
    Idle,
    Running,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MountLoadStatus {
    Loading,
    Available {
        total_files: usize,
        observed_at_ms: u64,
        origin: SnapshotOrigin,
        refresh: RefreshState,
    },
    Failed {
        error: String,
    },
}

#[derive(Clone)]
pub struct MountScanJob {
    pub mount: RuntimeMount,
    pub backend: StorageBackendRef,
    pub epoch: u64,
}

pub struct MountScanResult {
    pub mount: RuntimeMount,
    pub backend: StorageBackendRef,
    pub epoch: u64,
    pub scan: Result<ScannedSubtree, StorageError>,
}

impl MountEntry {
    fn new(declared: RuntimeMount, status: MountLoadStatus) -> Self {
        Self {
            declared,
            epoch: 0,
            content_revision: 0,
            cache_descriptor: None,
            status,
        }
    }

    pub fn effective_mount(&self) -> RuntimeMount {
        self.declared.clone()
    }

    pub fn error(&self) -> Option<&str> {
        match &self.status {
            MountLoadStatus::Failed { error } => Some(error),
            MountLoadStatus::Available {
                refresh: RefreshState::Failed(error),
                ..
            } => Some(error),
            _ => None,
        }
    }

    fn fail(&mut self, error: String) {
        match &mut self.status {
            MountLoadStatus::Available { refresh, .. } => *refresh = RefreshState::Failed(error),
            _ => self.status = MountLoadStatus::Failed { error },
        }
    }
}

impl MountLoadSet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn insert_loaded(&mut self, declared: RuntimeMount, total_files: usize) {
        let mut entry = MountEntry::new(
            declared,
            MountLoadStatus::Available {
                total_files,
                observed_at_ms: js_sys::Date::now() as u64,
                origin: SnapshotOrigin::Network,
                refresh: RefreshState::Idle,
            },
        );
        entry.content_revision = 1;
        self.entries.insert(entry.declared.root.clone(), entry);
    }

    pub fn insert_loading(&mut self, declared: RuntimeMount, backend: StorageBackendRef) {
        self.insert_declared_loading(declared.clone());
        self.scan_jobs.push(MountScanJob {
            mount: declared,
            backend,
            epoch: 0,
        });
    }

    pub(crate) fn insert_declared_loading(&mut self, declared: RuntimeMount) {
        self.entries.insert(
            declared.root.clone(),
            MountEntry::new(declared, MountLoadStatus::Loading),
        );
    }

    pub fn insert_failed(&mut self, declared: RuntimeMount, error: impl Into<String>) {
        self.entries.insert(
            declared.root.clone(),
            MountEntry::new(
                declared,
                MountLoadStatus::Failed {
                    error: error.into(),
                },
            ),
        );
    }

    pub fn reject(&mut self, declared: RuntimeMount, error: impl Into<String>) {
        self.rejected_entries.push(MountEntry::new(
            declared,
            MountLoadStatus::Failed {
                error: error.into(),
            },
        ));
    }

    pub fn effective_mounts(&self) -> Vec<RuntimeMount> {
        self.entries
            .values()
            .map(MountEntry::effective_mount)
            .collect()
    }

    pub fn owner(&self, path: &VirtualPath) -> Option<&MountEntry> {
        self.entries
            .iter()
            .filter(|(root, _)| path.starts_with(root))
            .max_by_key(|(root, _)| root.as_str().len())
            .map(|(_, entry)| entry)
    }

    pub fn status(&self, root: &VirtualPath) -> Option<MountLoadStatus> {
        self.entries.get(root).map(|entry| entry.status.clone())
    }

    pub fn is_loaded(&self, root: &VirtualPath) -> bool {
        matches!(self.status(root), Some(MountLoadStatus::Available { .. }))
    }

    pub fn declared(&self, root: &VirtualPath) -> Option<RuntimeMount> {
        self.entries.get(root).map(|entry| entry.declared.clone())
    }

    pub fn failed_entries(&self) -> Vec<MountEntry> {
        self.entries
            .values()
            .chain(self.rejected_entries.iter())
            .filter(|entry| entry.error().is_some())
            .cloned()
            .collect()
    }

    pub fn mark_loading(&mut self, root: &VirtualPath) -> Option<(RuntimeMount, u64)> {
        let entry = self.entries.get_mut(root)?;
        entry.epoch = entry.epoch.saturating_add(1);
        match &mut entry.status {
            MountLoadStatus::Available { refresh, .. } => *refresh = RefreshState::Running,
            _ => entry.status = MountLoadStatus::Loading,
        }
        Some((entry.declared.clone(), entry.epoch))
    }

    pub(crate) fn mark_failed(&mut self, root: &VirtualPath, error: impl Into<String>) -> bool {
        let Some(entry) = self.entries.get_mut(root) else {
            return false;
        };
        entry.fail(error.into());
        true
    }

    pub fn accepts_result(&self, root: &VirtualPath, epoch: u64) -> bool {
        self.entries
            .get(root)
            .is_some_and(|entry| entry.epoch == epoch)
    }

    /// A cache result can fill an empty mount, never replace an available snapshot.
    pub fn accepts_cache(&self, root: &VirtualPath, epoch: u64) -> bool {
        self.entries.get(root).is_some_and(|entry| {
            entry.epoch == epoch && !matches!(entry.status, MountLoadStatus::Available { .. })
        })
    }

    pub fn publish(
        &mut self,
        root: &VirtualPath,
        epoch: u64,
        total_files: usize,
        observed_at_ms: u64,
        origin: SnapshotOrigin,
        refresh: RefreshState,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(root) else {
            return false;
        };
        if entry.epoch != epoch {
            return false;
        }
        entry.content_revision = entry.content_revision.saturating_add(1);
        entry.status = MountLoadStatus::Available {
            total_files,
            observed_at_ms,
            origin,
            refresh,
        };
        true
    }

    pub fn mark_loaded_if_current(
        &mut self,
        root: &VirtualPath,
        epoch: u64,
        total_files: usize,
    ) -> bool {
        self.publish(
            root,
            epoch,
            total_files,
            js_sys::Date::now() as u64,
            SnapshotOrigin::Network,
            RefreshState::Idle,
        )
    }

    pub fn mark_failed_if_current(
        &mut self,
        root: &VirtualPath,
        epoch: u64,
        error: impl Into<String>,
    ) -> bool {
        if !self.accepts_result(root, epoch) {
            return false;
        }
        self.mark_failed(root, error)
    }

    pub fn failed_roots_under(&self, root: &VirtualPath) -> Vec<VirtualPath> {
        self.entries
            .iter()
            .filter(|(candidate, entry)| {
                *candidate != root
                    && candidate.starts_with(root)
                    && matches!(entry.status, MountLoadStatus::Failed { .. })
            })
            .map(|(candidate, _)| candidate.clone())
            .collect()
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use std::rc::Rc;

    use super::*;
    use wasm_bindgen_test::*;
    use websh_core::domain::RuntimeBackendKind;
    use websh_core::ports::{LocalBoxFuture, ScannedSubtree, StorageBackend, StorageResult};

    wasm_bindgen_test_configure!(run_in_browser);

    struct NoopBackend;

    impl StorageBackend for NoopBackend {
        fn backend_type(&self) -> &'static str {
            "noop"
        }

        fn scan(&self) -> LocalBoxFuture<'_, StorageResult<ScannedSubtree>> {
            Box::pin(async { Ok(ScannedSubtree::default()) })
        }

        fn read_text<'a>(
            &'a self,
            _rel_path: &'a str,
        ) -> LocalBoxFuture<'a, StorageResult<String>> {
            Box::pin(async { Ok(String::new()) })
        }

        fn read_bytes<'a>(
            &'a self,
            _rel_path: &'a str,
        ) -> LocalBoxFuture<'a, StorageResult<Vec<u8>>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    fn mount(root: &str) -> RuntimeMount {
        RuntimeMount::new(
            VirtualPath::from_absolute(root).expect("mount root"),
            root.trim_start_matches('/'),
            RuntimeBackendKind::GitHub,
        )
    }

    #[wasm_bindgen_test]
    fn declared_loading_mount_does_not_queue_scan_job() {
        let mut set = MountLoadSet::empty();
        let root = VirtualPath::root();
        set.insert_declared_loading(RuntimeMount::new(
            root.clone(),
            "~",
            RuntimeBackendKind::GitHub,
        ));

        assert!(matches!(set.status(&root), Some(MountLoadStatus::Loading)));
        assert!(set.scan_jobs.is_empty());
    }

    #[wasm_bindgen_test]
    fn loaded_mount_preserves_declaration() {
        let mut set = MountLoadSet::empty();
        set.insert_loading(mount("/db"), Rc::new(NoopBackend));
        let root = VirtualPath::from_absolute("/db").expect("root");
        assert!(set.mark_loaded_if_current(&root, 0, 3));

        let effective = set.effective_mounts();
        assert_eq!(effective[0].root, root);
        assert_eq!(effective[0], mount("/db"));
    }

    #[wasm_bindgen_test]
    fn old_epoch_result_is_stale() {
        let mut set = MountLoadSet::empty();
        let root = VirtualPath::from_absolute("/db").expect("root");
        set.insert_loaded(mount("/db"), 1);

        let (_, epoch) = set.mark_loading(&root).expect("mount should reload");

        assert_eq!(epoch, 1);
        assert!(!set.accepts_result(&root, 0));
        assert!(set.accepts_result(&root, 1));
    }
}
