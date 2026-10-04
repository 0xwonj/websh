//! Concurrent cache restore and live refresh with one publication boundary.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures_util::future::{Either, select};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use super::RuntimeResult;
use super::content::Content;
use super::mount_cache::{CacheRecord, CacheWrite, RESTORE_TIMEOUT_MS};
use super::mounts::{MountScanJob, MountScanResult, RefreshState, SnapshotOrigin};
use crate::platform::time::current_timestamp;

pub async fn refresh_mount(ctx: Content, generation: u64, job: MountScanJob) -> RuntimeResult {
    let descriptor = ctx.mount_cache_descriptor(&job.mount.root);
    let allow_restore = !ctx
        .mounts
        .with_untracked(|mounts| mounts.is_loaded(&job.mount.root));
    let cache_done = Cell::new(!allow_restore || descriptor.is_none());
    let network_error = RefCell::new(None);
    let started = current_timestamp();
    let cache = ctx.mount_cache();

    let restore = async {
        if cache_done.get() {
            return;
        }
        let restored = match select(
            cache.restore(descriptor.clone().expect("cache descriptor")),
            Box::pin(TimeoutFuture::new(RESTORE_TIMEOUT_MS)),
        )
        .await
        {
            Either::Left((snapshot, _)) => snapshot,
            Either::Right(_) => None,
        };
        cache_done.set(true);
        if let Some(snapshot) = restored {
            let refresh = network_error.borrow().as_ref().map_or(
                RefreshState::Running,
                |error: &websh_core::ports::StorageError| RefreshState::Failed(error.to_string()),
            );
            let result = MountScanResult {
                mount: job.mount.clone(),
                backend: job.backend.clone(),
                epoch: job.epoch,
                scan: Ok(snapshot.scan),
            };
            let _ = ctx.apply_mount_snapshot(
                generation,
                result,
                SnapshotOrigin::Cache,
                snapshot.observed_at_ms,
                refresh,
            );
        }
        if let Some(error) = network_error.borrow().clone() {
            let _ = ctx.apply_mount_scan_result(
                generation,
                MountScanResult {
                    mount: job.mount.clone(),
                    backend: job.backend.clone(),
                    epoch: job.epoch,
                    scan: Err(error),
                },
            );
        }
    };
    let network = async {
        let result = super::loader::scan_mount(job.clone()).await;
        if !ctx.mount_attempt_is_current(generation, &job.mount.root, job.epoch) {
            return Ok(());
        }
        match result.scan {
            Ok(scan) => {
                let observed = current_timestamp();
                let record = descriptor.clone().and_then(|descriptor| {
                    CacheRecord::from_scan(descriptor, &scan, started, observed)
                });
                let publication = ctx.apply_mount_snapshot(
                    generation,
                    MountScanResult {
                        mount: job.mount.clone(),
                        backend: job.backend.clone(),
                        epoch: job.epoch,
                        scan: Ok(scan),
                    },
                    SnapshotOrigin::Network,
                    observed,
                    RefreshState::Idle,
                );
                let accepted = match publication {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        *network_error.borrow_mut() =
                            Some(websh_core::ports::StorageError::RemoteRejected {
                                message: error.to_string(),
                            });
                        if cache_done.get() {
                            ctx.fail_attempt(
                                generation,
                                &job.mount.root,
                                job.epoch,
                                error.to_string(),
                            );
                        }
                        return Err(error);
                    }
                };
                if accepted && let Some(record) = record {
                    let root = job.mount.root.clone();
                    let epoch = job.epoch;
                    let cache = cache.clone();
                    spawn_local(async move {
                        cache
                            .persist(CacheWrite {
                                record,
                                is_current: Rc::new(move || {
                                    ctx.mount_attempt_is_current(generation, &root, epoch)
                                }),
                            })
                            .await;
                    });
                }
            }
            Err(error) => {
                let message = error.to_string();
                *network_error.borrow_mut() = Some(error.clone());
                // An early failure waits for a timely cache answer before displaying a cold failure.
                if cache_done.get() {
                    ctx.apply_mount_scan_result(
                        generation,
                        MountScanResult {
                            mount: job.mount.clone(),
                            backend: job.backend.clone(),
                            epoch: job.epoch,
                            scan: Err(error),
                        },
                    )?;
                }
                return Err(super::RuntimeError::RefreshFailed { message });
            }
        }
        Ok(())
    };
    let ((), result) = futures_util::join!(restore, network);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::mount_cache::{CacheDescriptor, CachedSnapshot, MountCache};
    use crate::runtime::{MountLoadSet, MountLoadStatus, RuntimeLoad};
    use wasm_bindgen_test::*;
    use websh_core::domain::{RuntimeMount, VirtualPath};
    use websh_core::filesystem::GlobalFs;
    use websh_core::ports::{
        LocalBoxFuture, ScannedSubtree, StorageBackend, StorageError, StorageResult,
    };
    wasm_bindgen_test_configure!(run_in_browser);

    struct FakeCache {
        delay: u32,
        hit: bool,
        writes: Rc<Cell<u32>>,
    }
    impl MountCache for FakeCache {
        fn restore(&self, _: CacheDescriptor) -> LocalBoxFuture<'_, Option<CachedSnapshot>> {
            Box::pin(async move {
                TimeoutFuture::new(self.delay).await;
                self.hit.then(|| CachedSnapshot {
                    scan: ScannedSubtree::default(),
                    observed_at_ms: 42,
                })
            })
        }
        fn persist(&self, write: CacheWrite) -> LocalBoxFuture<'_, ()> {
            Box::pin(async move {
                if (write.is_current)() {
                    self.writes.set(self.writes.get() + 1);
                }
            })
        }
    }
    #[derive(Clone, Copy)]
    enum ScanOutcome {
        Valid,
        Unavailable,
        ConflictingRoutes,
    }

    struct ScanBackend {
        delay: u32,
        outcome: ScanOutcome,
    }
    impl StorageBackend for ScanBackend {
        fn scan(&self) -> LocalBoxFuture<'_, StorageResult<ScannedSubtree>> {
            Box::pin(async move {
                TimeoutFuture::new(self.delay).await;
                match self.outcome {
                    ScanOutcome::Unavailable => Err(StorageError::Network {
                        message: "offline".into(),
                    }),
                    ScanOutcome::Valid => Ok(ScannedSubtree::default()),
                    ScanOutcome::ConflictingRoutes => Ok(ScannedSubtree {
                        files: ["same.md", "same.html"]
                            .into_iter()
                            .map(|path| websh_core::ports::ScannedFile {
                                path: path.into(),
                                meta: websh_core::domain::NodeMetadata {
                                    kind: websh_core::domain::NodeKind::Page,
                                    ..Default::default()
                                },
                                extensions: Default::default(),
                            })
                            .collect(),
                        directories: vec![],
                    }),
                }
            })
        }
        fn read_text<'a>(&'a self, _: &'a str) -> LocalBoxFuture<'a, StorageResult<String>> {
            Box::pin(async { Ok(String::new()) })
        }
        fn read_bytes<'a>(&'a self, _: &'a str) -> LocalBoxFuture<'a, StorageResult<Vec<u8>>> {
            Box::pin(async { Ok(vec![]) })
        }
    }

    fn setup(
        cache_delay: u32,
        cache_hit: bool,
        network_delay: u32,
        outcome: ScanOutcome,
    ) -> (Owner, Content, MountScanJob, Rc<Cell<u32>>) {
        let owner = Owner::new();
        let writes = Rc::new(Cell::new(0));
        let (ctx, job) = owner.with(|| {
            let ctx = Content::new(super::super::loader::bootstrap_runtime_load());
            let root = VirtualPath::from_absolute("/db").unwrap();
            let mount = RuntimeMount::new(root.clone(), "db");
            let backend = Rc::new(ScanBackend {
                delay: network_delay,
                outcome,
            });
            let mut mounts = MountLoadSet::empty();
            mounts.insert_loading(mount.clone(), backend.clone());
            mounts.set_cache_descriptor(
                &root,
                Some(CacheDescriptor {
                    root: root.to_string(),
                    repo: "owner/repo".into(),
                    reference: "main".into(),
                    prefix: "".into(),
                    manifest_url: "https://example.test/manifest.json".into(),
                    content_url: "https://example.test/".into(),
                }),
            );
            let mut global_fs = GlobalFs::empty();
            global_fs.reserve_mount_point(root.clone()).unwrap();
            ctx.apply_runtime_load(RuntimeLoad {
                snapshot: websh_core::filesystem::Snapshot::new(global_fs).unwrap(),
                backends: [(root, backend.clone() as _)].into(),
                total_files: 0,
                mounts,
            });
            ctx.set_mount_cache(Rc::new(FakeCache {
                delay: cache_delay,
                hit: cache_hit,
                writes: writes.clone(),
            }));
            (
                ctx,
                MountScanJob {
                    mount,
                    backend,
                    epoch: 0,
                },
            )
        });
        (owner, ctx, job, writes)
    }

    #[wasm_bindgen_test(async)]
    async fn timely_cache_survives_early_network_failure() {
        let (_owner, ctx, job, writes) = setup(30, true, 1, ScanOutcome::Unavailable);
        let root = job.mount.root.clone();
        let watch = async {
            TimeoutFuture::new(10).await;
            assert_eq!(ctx.mount_status_for(&root), Some(MountLoadStatus::Loading));
        };
        let (result, ()) =
            futures_util::join!(refresh_mount(ctx, ctx.runtime_generation(), job), watch);
        assert!(result.is_err());
        assert!(matches!(
            ctx.mount_status_for(&root),
            Some(MountLoadStatus::Available {
                origin: SnapshotOrigin::Cache,
                observed_at_ms: 42,
                refresh: RefreshState::Failed(_),
                ..
            })
        ));
        assert_eq!(writes.get(), 0);
    }

    #[wasm_bindgen_test(async)]
    async fn rejected_network_candidate_waits_for_cache_without_publishing_failure_early() {
        let (_owner, ctx, job, writes) = setup(35, true, 1, ScanOutcome::ConflictingRoutes);
        let root = job.mount.root.clone();
        let snapshot = ctx.snapshot.get_untracked();
        let watch = async {
            TimeoutFuture::new(10).await;
            assert_eq!(ctx.mount_status_for(&root), Some(MountLoadStatus::Loading));
            assert!(Rc::ptr_eq(&snapshot, &ctx.snapshot.get_untracked()));
        };
        let (result, ()) =
            futures_util::join!(refresh_mount(ctx, ctx.runtime_generation(), job), watch);
        assert!(result.is_err());
        assert!(matches!(
            ctx.mount_status_for(&root),
            Some(MountLoadStatus::Available {
                origin: SnapshotOrigin::Cache,
                refresh: RefreshState::Failed(_),
                ..
            })
        ));
        assert_eq!(writes.get(), 0);
    }

    #[wasm_bindgen_test(async)]
    async fn fast_network_wins_over_later_cache_and_writes_once() {
        let (_owner, ctx, job, writes) = setup(25, true, 1, ScanOutcome::Valid);
        let root = job.mount.root.clone();
        refresh_mount(ctx, ctx.runtime_generation(), job)
            .await
            .unwrap();
        TimeoutFuture::new(1).await;
        assert!(matches!(
            ctx.mount_status_for(&root),
            Some(MountLoadStatus::Available {
                origin: SnapshotOrigin::Network,
                refresh: RefreshState::Idle,
                ..
            })
        ));
        assert_eq!(ctx.mounts.with_untracked(|m| m.revision(&root)), 1);
        assert_eq!(writes.get(), 1);
    }

    #[wasm_bindgen_test(async)]
    async fn late_cache_cannot_rescue_failed_network_after_restore_deadline() {
        let (_owner, ctx, job, _) = setup(650, true, 1, ScanOutcome::Unavailable);
        let root = job.mount.root.clone();
        assert!(
            refresh_mount(ctx, ctx.runtime_generation(), job)
                .await
                .is_err()
        );
        assert!(matches!(
            ctx.mount_status_for(&root),
            Some(MountLoadStatus::Failed { .. })
        ));
    }

    #[wasm_bindgen_test(async)]
    async fn obsolete_attempt_neither_publishes_nor_persists() {
        let (_owner, ctx, job, writes) = setup(20, true, 30, ScanOutcome::Valid);
        let root = job.mount.root.clone();
        let supersede = async {
            TimeoutFuture::new(1).await;
            ctx.mark_mount_loading(&root).unwrap();
        };
        let (result, ()) =
            futures_util::join!(refresh_mount(ctx, ctx.runtime_generation(), job), supersede);
        result.unwrap();
        assert_eq!(ctx.mount_status_for(&root), Some(MountLoadStatus::Loading));
        assert_eq!(writes.get(), 0);
    }
}
