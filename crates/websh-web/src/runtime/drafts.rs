//! Browser draft persistence service.

use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use websh_core::domain::ChangeSet;
use websh_core::ports::StorageResult;

use super::idb;

const DEBOUNCE_MS: u32 = 300;
const GLOBAL_DRAFT_ID: &str = "global";

thread_local! {
    static GLOBAL_DRAFT_PERSISTER: DraftPersister = DraftPersister::new(GLOBAL_DRAFT_ID);
}

/// Debounced IDB persistence for a browser draft ChangeSet.
struct DraftPersister {
    draft_id: String,
    pending: Rc<RefCell<Option<ChangeSet>>>,
    persisted: Rc<RefCell<ChangeSet>>,
    task_running: Rc<RefCell<bool>>,
}

impl DraftPersister {
    fn new(draft_id: impl Into<String>) -> Self {
        Self {
            draft_id: draft_id.into(),
            pending: Rc::new(RefCell::new(None)),
            persisted: Rc::new(RefCell::new(ChangeSet::new())),
            task_running: Rc::new(RefCell::new(false)),
        }
    }

    fn mark_persisted(&self, changes: ChangeSet) {
        *self.persisted.borrow_mut() = changes;
    }

    fn schedule(&self, changes: ChangeSet) {
        *self.pending.borrow_mut() = Some(changes);

        if *self.task_running.borrow() {
            return;
        }
        *self.task_running.borrow_mut() = true;

        let pending = self.pending.clone();
        let persisted = self.persisted.clone();
        let running = self.task_running.clone();
        let draft_id = self.draft_id.clone();

        spawn_local(async move {
            loop {
                TimeoutFuture::new(DEBOUNCE_MS).await;
                let Some(changes) = pending.borrow_mut().take() else {
                    *running.borrow_mut() = false;
                    break;
                };
                let previous = persisted.borrow().clone();
                match idb::open_db().await {
                    Ok(db) => {
                        match idb::save_draft_delta(&db, &draft_id, &previous, &changes).await {
                            Ok(()) => {
                                *persisted.borrow_mut() = changes;
                            }
                            Err(error) => {
                                web_sys::console::error_1(
                                    &format!("draft persist failed: {error}").into(),
                                );
                            }
                        }
                    }
                    Err(error) => {
                        web_sys::console::error_1(&format!("idb open failed: {error}").into());
                    }
                }
            }
        });
    }
}

pub(crate) fn schedule_global(changes: ChangeSet) {
    GLOBAL_DRAFT_PERSISTER.with(|persister| persister.schedule(changes));
}

pub(crate) async fn hydrate_global() -> StorageResult<ChangeSet> {
    let db = idb::open_db().await?;
    let changes = idb::load_draft(&db, GLOBAL_DRAFT_ID)
        .await?
        .unwrap_or_default();
    GLOBAL_DRAFT_PERSISTER.with(|persister| persister.mark_persisted(changes.clone()));
    Ok(changes)
}
