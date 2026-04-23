//! Debounced IDB persistence for ChangeSet. The scheduling logic lives here so
//! it is testable and the reactive layer stays thin.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;

use crate::core::changes::ChangeSet;

use super::idb;

pub const DEBOUNCE_MS: u32 = 300;
pub const GLOBAL_DRAFT_ID: &str = "global";

/// A debounce handle. Call `schedule(changes)` on every mutation; the inner
/// task waits `DEBOUNCE_MS` and persists the latest snapshot. Rapid successive
/// calls reset the timer.
pub struct DraftPersister {
    draft_id: String,
    pending: Rc<RefCell<Option<ChangeSet>>>,
    task_running: Rc<RefCell<bool>>,
}

impl DraftPersister {
    pub fn new(draft_id: impl Into<String>) -> Self {
        Self {
            draft_id: draft_id.into(),
            pending: Rc::new(RefCell::new(None)),
            task_running: Rc::new(RefCell::new(false)),
        }
    }

    pub fn schedule(&self, changes: ChangeSet) {
        *self.pending.borrow_mut() = Some(changes);

        if *self.task_running.borrow() {
            return; // existing task will pick up the newer snapshot
        }
        *self.task_running.borrow_mut() = true;

        let pending = self.pending.clone();
        let running = self.task_running.clone();
        let draft_id = self.draft_id.clone();

        spawn_local(async move {
            TimeoutFuture::new(DEBOUNCE_MS).await;
            let snapshot = pending.borrow_mut().take();
            *running.borrow_mut() = false;

            if let Some(cs) = snapshot {
                match idb::open_db().await {
                    Ok(db) => {
                        if let Err(e) = idb::save_draft(&db, &draft_id, &cs).await {
                            web_sys::console::error_1(&format!("draft persist failed: {e}").into());
                        }
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("idb open failed: {e}").into());
                    }
                }
            }
        });
    }
}
