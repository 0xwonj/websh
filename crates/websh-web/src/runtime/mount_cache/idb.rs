//! IndexedDB is optional. Every operation has a deadline, and a timed-out open is closed if it later succeeds.
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::future::IntoFuture;
use std::rc::Rc;

use ::idb::{
    Database, DatabaseEvent, Factory, KeyPath, ObjectStore, ObjectStoreParams, TransactionMode,
};
use futures_util::future::{Either, select};
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use super::*;

const DATABASE: &str = "websh-cache";
const STORE: &str = "mount_snapshots";

#[derive(Clone)]
pub struct BrowserMountCache {
    database_name: &'static str,
    disabled: Rc<Cell<bool>>,
    writes_disabled: Rc<Cell<bool>>,
    running: Rc<RefCell<BTreeSet<String>>>,
    pending: Rc<RefCell<BTreeMap<String, CacheWrite>>>,
}

impl Default for BrowserMountCache {
    fn default() -> Self {
        Self {
            database_name: DATABASE,
            disabled: Rc::default(),
            writes_disabled: Rc::default(),
            running: Rc::default(),
            pending: Rc::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum CacheError {
    #[error("cache unavailable")]
    Unavailable,
    #[error("cache operation timed out")]
    Timeout,
    #[error("cache transaction aborted")]
    Aborted,
    #[error(transparent)]
    Idb(#[from] ::idb::Error),
}

struct Connection(Database);
impl Drop for Connection {
    fn drop(&mut self) {
        self.0.close();
    }
}

struct TransactionGuard(web_sys::IdbTransaction);
impl Drop for TransactionGuard {
    fn drop(&mut self) {
        self.0.set_onabort(None);
        self.0.set_oncomplete(None);
        self.0.set_onerror(None);
    }
}

fn monotonic_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or_else(js_sys::Date::now, |p| p.now())
}

#[derive(Clone, Copy)]
struct Deadline(f64);
impl Deadline {
    fn new(ms: u32) -> Self {
        Self(monotonic_ms() + f64::from(ms))
    }
    fn remaining(self) -> u32 {
        (self.0 - monotonic_ms()).max(0.0).ceil() as u32
    }
}

impl BrowserMountCache {
    async fn open(&self, deadline: Deadline) -> Result<Connection, CacheError> {
        if self.disabled.get() {
            return Err(CacheError::Unavailable);
        }
        let result = self.open_inner(deadline).await;
        if result.is_err() {
            self.disabled.set(true);
        }
        result
    }

    async fn open_inner(&self, deadline: Deadline) -> Result<Connection, CacheError> {
        let mut request = Factory::new()?.open(self.database_name, Some(1))?;
        let disabled = self.disabled.clone();
        request.on_blocked(move |_| disabled.set(true));
        request.on_upgrade_needed(|event| {
            if let Ok(db) = event.database() {
                let mut params = ObjectStoreParams::new();
                params.key_path(Some(KeyPath::new_single("key")));
                // Only the new cache database is ever opened. No legacy schema migration.
                if db.create_object_store(STORE, params).is_err() {
                    db.close();
                }
            }
        });
        let opening = Box::pin(request.into_future());
        let timer = Box::pin(TimeoutFuture::new(deadline.remaining()));
        let mut db = match select(opening, timer).await {
            Either::Left((result, _)) => result?,
            Either::Right((_, pending)) => {
                spawn_local(async move {
                    if let Ok(db) = pending.await {
                        db.close();
                    }
                });
                return Err(CacheError::Timeout);
            }
        };
        if self.disabled.get() || !db.store_names().iter().any(|name| name == STORE) {
            db.close();
            return Err(CacheError::Unavailable);
        }
        db.on_version_change(|event| {
            if let Some(target) = event.target()
                && let Ok(db) = Database::try_from(target)
            {
                db.close();
            }
        });
        Ok(Connection(db))
    }

    async fn read(&self, key: String, deadline: Deadline) -> Result<Option<JsValue>, CacheError> {
        let connection = self.open(deadline).await?;
        let transaction = connection
            .0
            .transaction(&[STORE], TransactionMode::ReadOnly)?;
        let store = transaction.object_store(STORE)?;
        if store.key_path()? != Some(KeyPath::new_single("key")) {
            self.disabled.set(true);
            return Err(CacheError::Unavailable);
        }
        let raw: web_sys::IdbTransaction = store.transaction().into();
        let done = transaction.into_future();
        let operation = Box::pin(async move {
            let _connection = connection;
            let _guard = TransactionGuard(store.transaction().into());
            let result =
                async { Ok::<_, CacheError>(store.get(JsValue::from_str(&key))?.await?) }.await;
            let completion = done.await?;
            if !completion.is_committed() {
                return Err(CacheError::Aborted);
            }
            result
        });
        self.finish(operation, raw, deadline).await
    }

    async fn finish<T: 'static>(
        &self,
        operation: std::pin::Pin<
            Box<impl std::future::Future<Output = Result<T, CacheError>> + 'static>,
        >,
        raw: web_sys::IdbTransaction,
        deadline: Deadline,
    ) -> Result<T, CacheError> {
        match select(
            operation,
            Box::pin(TimeoutFuture::new(deadline.remaining())),
        )
        .await
        {
            Either::Left((result, _)) => result,
            Either::Right((_, pending)) => {
                let _ = raw.abort();
                self.disabled.set(true);
                // Keep callbacks alive until abort settles; late results cannot reach the caller.
                spawn_local(async move {
                    let _ = pending.await;
                });
                Err(CacheError::Timeout)
            }
        }
    }

    async fn transact(
        &self,
        incoming: Option<CacheWrite>,
        quota_eviction: bool,
    ) -> Result<(), CacheError> {
        let deadline = Deadline::new(OPERATION_TIMEOUT_MS);
        let connection = self.open(deadline).await?;
        if incoming.as_ref().is_some_and(|write| !(write.is_current)()) {
            return Ok(());
        }
        let transaction = connection
            .0
            .transaction(&[STORE], TransactionMode::ReadWrite)?;
        let store = transaction.object_store(STORE)?;
        if store.key_path()? != Some(KeyPath::new_single("key")) {
            self.disabled.set(true);
            return Err(CacheError::Unavailable);
        }
        let raw: web_sys::IdbTransaction = store.transaction().into();
        let done = transaction.into_future();
        let operation = Box::pin(async move {
            let _connection = connection;
            let guard = TransactionGuard(store.transaction().into());
            let result = update_store(&store, incoming, quota_eviction).await;
            if result.is_err() {
                let _ = guard.0.abort();
            }
            let completion = done.await?;
            result?;
            if !completion.is_committed() {
                return Err(CacheError::Aborted);
            }
            Ok(())
        });
        self.finish(operation, raw, deadline).await
    }
}

async fn update_store(
    store: &ObjectStore,
    incoming: Option<CacheWrite>,
    quota_eviction: bool,
) -> Result<(), CacheError> {
    // Both reads are queued in this transaction before awaiting, preserving key/value order.
    let keys = store.get_all_keys(None, None)?.into_future();
    let values = store.get_all(None, None)?.into_future();
    let (keys, values) = futures_util::join!(keys, values);
    let now = crate::platform::time::current_timestamp();
    let mut records = BTreeMap::new();
    for (key, value) in keys?.into_iter().zip(values?) {
        let record = serde_wasm_bindgen::from_value::<CacheRecord>(value).ok();
        if let Some(record) = record.filter(|record| {
            key.as_string().as_deref() == Some(&record.key)
                && record.validate(&record.descriptor, now).is_some()
        }) {
            records.insert(record.key.clone(), record);
        } else {
            // Validate the current value inside this transaction; never delete a replacement based on an earlier read.
            store.delete(key)?.await?;
        }
    }
    let mut inserted = None;
    if let Some(write) = incoming
        && (write.is_current)()
        && write
            .record
            .validate(&write.record.descriptor, now)
            .is_some()
    {
        let record = write.record;
        if records
            .get(&record.key)
            .is_none_or(|existing| record.outranks(existing))
        {
            inserted = Some(record.key.clone());
            records.insert(record.key.clone(), record);
        }
    }
    let mut order = records
        .values()
        .map(|r| (r.observed_at_ms, r.key.clone()))
        .collect::<Vec<_>>();
    order.sort();
    let mut bytes: usize = records.values().map(|r| r.manifest_bytes).sum();
    let mut count = records.len();
    for (index, (_, key)) in order.into_iter().enumerate() {
        if count <= MAX_RECORDS && bytes <= MAX_TOTAL_BYTES && !(quota_eviction && index == 0) {
            break;
        }
        if let Some(record) = records.remove(&key) {
            bytes -= record.manifest_bytes;
            count -= 1;
        }
        store.delete(JsValue::from_str(&key))?.await?;
    }
    if let Some(key) = inserted
        && let Some(record) = records.get(&key)
    {
        let value = serde_wasm_bindgen::to_value(record).map_err(|_| CacheError::Unavailable)?;
        store.put(&value, None)?.await?;
    }
    Ok(())
}

impl MountCache for BrowserMountCache {
    fn restore(&self, descriptor: CacheDescriptor) -> LocalBoxFuture<'_, Option<CachedSnapshot>> {
        Box::pin(async move {
            let deadline = Deadline::new(RESTORE_TIMEOUT_MS);
            let value = match self.read(descriptor.key(), deadline).await {
                Ok(Some(value)) => value,
                _ => return None,
            };
            let record = serde_wasm_bindgen::from_value::<CacheRecord>(value).ok();
            let snapshot = record.and_then(|record| {
                record
                    .validate(&descriptor, crate::platform::time::current_timestamp())
                    .map(|scan| CachedSnapshot {
                        scan,
                        observed_at_ms: record.observed_at_ms,
                    })
            });
            if snapshot.is_none() {
                let cache = self.clone();
                spawn_local(async move {
                    let _ = cache.transact(None, false).await;
                });
            }
            if deadline.remaining() == 0 {
                None
            } else {
                snapshot
            }
        })
    }

    fn persist(&self, write: CacheWrite) -> LocalBoxFuture<'_, ()> {
        Box::pin(async move {
            if self.disabled.get() || self.writes_disabled.get() {
                return;
            }
            let key = write.record.key.clone();
            if !(write.is_current)() {
                return;
            }
            if self
                .pending
                .borrow()
                .get(&key)
                .is_some_and(|pending| !write.record.outranks(&pending.record))
            {
                return;
            }
            self.pending.borrow_mut().insert(key.clone(), write);
            if !self.running.borrow_mut().insert(key.clone()) {
                return;
            }
            loop {
                let Some(write) = self.pending.borrow_mut().remove(&key) else {
                    break;
                };
                if self.disabled.get() || self.writes_disabled.get() {
                    break;
                }
                if !(write.is_current)() {
                    continue;
                }
                if let Err(error) = self.transact(Some(write.clone()), false).await {
                    let quota = error.to_string().contains("QuotaExceeded");
                    if quota {
                        // One bounded eviction + retry, then disable writes for this session.
                        let retry = async {
                            self.transact(None, true).await?;
                            self.transact(Some(write), false).await
                        }
                        .await;
                        if retry.is_err() {
                            self.writes_disabled.set(true);
                        }
                    } else {
                        self.writes_disabled.set(true);
                    }
                }
            }
            self.pending.borrow_mut().remove(&key);
            self.running.borrow_mut().remove(&key);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    fn isolated(name: &'static str) -> BrowserMountCache {
        BrowserMountCache {
            database_name: name,
            ..Default::default()
        }
    }
    fn record(root: &str, started: u64, observed: u64) -> CacheRecord {
        let mut descriptor = super::super::tests::descriptor();
        descriptor.canonical_mount_root = root.into();
        CacheRecord::from_scan(descriptor, &ScannedSubtree::default(), started, observed).unwrap()
    }
    fn write(record: CacheRecord) -> CacheWrite {
        CacheWrite {
            record,
            is_current: Rc::new(|| true),
        }
    }

    #[wasm_bindgen_test(async)]
    async fn separate_connections_compare_and_keep_newer_request_atomically() {
        let cache = isolated("websh-cache-test-compare");
        let other = isolated("websh-cache-test-compare");
        let now = crate::platform::time::current_timestamp();
        let old = record("/db", now - 100, now - 10);
        let new = record("/db", now - 50, now);
        // The older request finishes last and even has a later observation time.
        cache.persist(write(new.clone())).await;
        let mut later_old = old;
        later_old.observed_at_ms = now + 10;
        other.persist(write(later_old)).await;
        let snapshot = cache.restore(new.descriptor).await.expect("snapshot");
        assert_eq!(snapshot.observed_at_ms, now);
        assert!(!cache.disabled.get());
        assert!(!cache.writes_disabled.get());
    }

    #[wasm_bindgen_test(async)]
    async fn record_limit_prunes_oldest_and_invalid_record_cannot_block_replacement() {
        let cache = isolated("websh-cache-test-bounds");
        let now = crate::platform::time::current_timestamp();
        let mut first = None;
        for index in 0..17 {
            let r = record(
                &format!("/mount{index}"),
                now - 100 + index,
                now - 100 + index,
            );
            if index == 0 {
                first = Some(r.descriptor.clone());
            }
            cache.persist(write(r)).await;
        }
        assert!(cache.restore(first.unwrap()).await.is_none());
        let d = record("/mount16", now, now).descriptor;
        assert!(cache.restore(d.clone()).await.is_some());
        let connection = cache.open(Deadline::new(2000)).await.unwrap();
        let tx = connection
            .0
            .transaction(&[STORE], TransactionMode::ReadWrite)
            .unwrap();
        let store = tx.object_store(STORE).unwrap();
        let done = tx.into_future();
        let mut corrupt = record("/mount16", now + 20, now + 20);
        corrupt.manifest_bytes += 1;
        store
            .put(&serde_wasm_bindgen::to_value(&corrupt).unwrap(), None)
            .unwrap()
            .await
            .unwrap();
        assert!(done.await.unwrap().is_committed());
        cache.persist(write(record("/mount16", now, now))).await;
        assert_eq!(cache.restore(d).await.unwrap().observed_at_ms, now);
    }

    #[wasm_bindgen_test(async)]
    async fn codec_preserves_bundles_metadata_extensions_and_empty_directories() {
        let cache = isolated("websh-cache-test-round-trip");
        let scan = parse_manifest_snapshot(r#"{"entries":[
            {"path":"empty","metadata":{"schema":1,"kind":"directory","authored":{},"derived":{}}},
            {"path":"article","metadata":{"schema":1,"kind":"bundle","bundle":{"default_variant":{"strategy":"static","id":"en"},"variants":[{"id":"en","path":"en.md","label":"English"},{"id":"ko","path":"ko.md","label":"Korean"}]},"authored":{"title":"Article"},"derived":{}}},
            {"path":"article/en.md","metadata":{"schema":1,"kind":"page","authored":{"title":"English","access":{"recipients":[{"address":"0xabc"}]}},"derived":{"size_bytes":123}},"mempool":{"status":"review","priority":"high","category":"writing"}},
            {"path":"article/ko.md","metadata":{"schema":1,"kind":"page","authored":{"title":"한국어"},"derived":{}}}
        ]}"#).unwrap();
        let now = crate::platform::time::current_timestamp();
        let record =
            CacheRecord::from_scan(super::super::tests::descriptor(), &scan, now, now).unwrap();
        let descriptor = record.descriptor.clone();
        cache.persist(write(record)).await;
        assert_eq!(cache.restore(descriptor).await.unwrap().scan, scan);
    }

    #[wasm_bindgen_test(async)]
    async fn aggregate_byte_budget_evicts_oldest_complete_record() {
        let cache = isolated("websh-cache-test-byte-budget");
        let mut scan = parse_manifest_snapshot(include_str!(
            "../../../../../tests/fixtures/manifest_golden.json"
        ))
        .unwrap();
        scan.files[0].meta.authored.description = Some("x".repeat(1_800_000));
        let now = crate::platform::time::current_timestamp();
        let mut descriptors = vec![];
        for index in 0..5 {
            let mut descriptor = super::super::tests::descriptor();
            descriptor.canonical_mount_root = format!("/bytes{index}");
            let record = CacheRecord::from_scan(
                descriptor.clone(),
                &scan,
                now - 10 + index,
                now - 10 + index,
            )
            .unwrap();
            descriptors.push(descriptor);
            cache.persist(write(record)).await;
        }
        assert!(cache.restore(descriptors[0].clone()).await.is_none());
        for descriptor in &descriptors[1..] {
            assert!(cache.restore(descriptor.clone()).await.is_some());
        }
        assert!(!cache.writes_disabled.get());
    }

    #[wasm_bindgen_test(async)]
    async fn unsupported_database_version_is_left_untouched_and_disables_cache() {
        let name = "websh-cache-test-newer-version";
        let db = Factory::new()
            .unwrap()
            .open(name, Some(2))
            .unwrap()
            .await
            .unwrap();
        db.close();
        let cache = isolated(name);
        assert!(
            cache
                .restore(super::super::tests::descriptor())
                .await
                .is_none()
        );
        assert!(cache.disabled.get());
        let db = Factory::new()
            .unwrap()
            .open(name, None)
            .unwrap()
            .await
            .unwrap();
        assert_eq!(db.version().unwrap(), 2);
        db.close();
    }

    #[wasm_bindgen_test(async)]
    async fn missing_store_is_not_repaired_or_deleted() {
        let name = "websh-cache-test-missing-store";
        let db = Factory::new()
            .unwrap()
            .open(name, Some(1))
            .unwrap()
            .await
            .unwrap();
        db.close();
        let cache = isolated(name);
        assert!(
            cache
                .restore(super::super::tests::descriptor())
                .await
                .is_none()
        );
        assert!(cache.disabled.get());
        let db = Factory::new()
            .unwrap()
            .open(name, None)
            .unwrap()
            .await
            .unwrap();
        assert!(db.store_names().is_empty());
        db.close();
    }

    #[wasm_bindgen_test(async)]
    async fn obsolete_write_does_not_open_database_or_block_next_write() {
        let cache = isolated("websh-cache-test-obsolete");
        let now = crate::platform::time::current_timestamp();
        let r = record("/db", now, now);
        cache
            .persist(CacheWrite {
                record: r.clone(),
                is_current: Rc::new(|| false),
            })
            .await;
        assert!(cache.pending.borrow().is_empty());
        cache.persist(write(r.clone())).await;
        assert!(cache.restore(r.descriptor).await.is_some());
    }
    #[wasm_bindgen::prelude::wasm_bindgen(
        inline_js = "export function injectPutFailure(mode) { const original = IDBObjectStore.prototype.put; let calls = 0; IDBObjectStore.prototype.put = function(...args) { calls++; if (mode === 'quota-always' || (mode === 'quota-once' && calls === 1)) throw new DOMException('test quota', 'QuotaExceededError'); const request = original.apply(this, args); if (mode === 'abort' && calls === 1) { const tx = this.transaction; request.addEventListener('success', () => tx.abort()); } if (mode === 'hang') { const store = this; request.addEventListener('success', () => { const keepAlive = () => { try { const next = store.get('__test_keepalive'); next.onsuccess = keepAlive; } catch {} }; keepAlive(); }); } return request; }; return () => { IDBObjectStore.prototype.put = original; return calls; }; } export function forceUpgradeOpen() { const original = IDBFactory.prototype.open; IDBFactory.prototype.open = function(name) { return original.call(this, name, 2); }; return () => { IDBFactory.prototype.open = original; }; }"
    )]
    extern "C" {
        fn injectPutFailure(mode: &str) -> js_sys::Function;
        fn forceUpgradeOpen() -> js_sys::Function;
    }

    #[wasm_bindgen_test(async)]
    async fn successful_put_then_abort_preserves_previous_record() {
        let cache = isolated("websh-cache-test-abort");
        let now = crate::platform::time::current_timestamp();
        let old = record("/db", now - 10, now - 10);
        cache.persist(write(old.clone())).await;
        let restore = injectPutFailure("abort");
        cache.persist(write(record("/db", now, now))).await;
        restore.call0(&JsValue::UNDEFINED).unwrap();
        assert_eq!(
            cache.restore(old.descriptor).await.unwrap().observed_at_ms,
            now - 10
        );
        assert!(cache.writes_disabled.get());
        assert!(cache.running.borrow().is_empty());
    }

    #[wasm_bindgen_test(async)]
    async fn quota_gets_one_retry_then_disables_writes_without_disabling_reads() {
        for (name, mode, succeeds) in [
            ("websh-cache-test-quota-once", "quota-once", true),
            ("websh-cache-test-quota-always", "quota-always", false),
        ] {
            let cache = isolated(name);
            let now = crate::platform::time::current_timestamp();
            let r = record("/db", now, now);
            let restore = injectPutFailure(mode);
            cache.persist(write(r.clone())).await;
            let calls = restore
                .call0(&JsValue::UNDEFINED)
                .unwrap()
                .as_f64()
                .unwrap();
            assert_eq!(calls, 2.0);
            assert_eq!(cache.writes_disabled.get(), !succeeds);
            assert_eq!(cache.restore(r.descriptor).await.is_some(), succeeds);
            assert!(!cache.disabled.get());
        }
    }

    #[wasm_bindgen_test(async)]
    async fn blocked_open_times_out_and_late_connection_does_not_block_future_upgrade() {
        let name = "websh-cache-test-blocked-late";
        let holder = Factory::new()
            .unwrap()
            .open(name, Some(1))
            .unwrap()
            .await
            .unwrap();
        let cache = isolated(name);
        let restore = forceUpgradeOpen();
        assert!(
            cache
                .restore(super::super::tests::descriptor())
                .await
                .is_none()
        );
        restore.call0(&JsValue::UNDEFINED).unwrap();
        assert!(cache.disabled.get());
        holder.close();
        TimeoutFuture::new(20).await;
        let upgrade = Factory::new()
            .unwrap()
            .open(name, Some(3))
            .unwrap()
            .into_future();
        match select(Box::pin(upgrade), Box::pin(TimeoutFuture::new(500))).await {
            Either::Left((Ok(db), _)) => db.close(),
            _ => panic!("late cache connection blocked the next upgrade"),
        }
    }

    #[wasm_bindgen_test(async)]
    async fn versionchange_releases_an_open_connection() {
        let name = "websh-cache-test-versionchange";
        let cache = isolated(name);
        let _connection = cache.open(Deadline::new(2000)).await.unwrap();
        let upgrade = Factory::new()
            .unwrap()
            .open(name, Some(2))
            .unwrap()
            .into_future();
        match select(Box::pin(upgrade), Box::pin(TimeoutFuture::new(500))).await {
            Either::Left((Ok(db), _)) => db.close(),
            _ => panic!("versionchange did not close the cache connection"),
        }
    }
    #[wasm_bindgen_test(async)]
    async fn transaction_timeout_aborts_changes_and_releases_writer_queue() {
        let cache = isolated("websh-cache-test-transaction-timeout");
        let now = crate::platform::time::current_timestamp();
        let old = record("/db", now - 10, now - 10);
        cache.persist(write(old.clone())).await;
        let restore = injectPutFailure("hang");
        cache.persist(write(record("/db", now, now))).await;
        restore.call0(&JsValue::UNDEFINED).unwrap();
        assert!(cache.disabled.get());
        assert!(cache.running.borrow().is_empty());
        assert!(cache.pending.borrow().is_empty());
        let other = isolated("websh-cache-test-transaction-timeout");
        assert_eq!(
            other.restore(old.descriptor).await.unwrap().observed_at_ms,
            now - 10
        );
    }

    #[wasm_bindgen_test(async)]
    async fn concurrent_same_tab_writes_preserve_the_newer_observation() {
        let cache = isolated("websh-cache-test-coalescing");
        let now = crate::platform::time::current_timestamp();
        let new = record("/db", now, now);
        futures_util::join!(
            cache.persist(write(record("/db", now - 20, now - 10))),
            cache.persist(write(new.clone()))
        );
        assert_eq!(
            cache.restore(new.descriptor).await.unwrap().observed_at_ms,
            now
        );
        assert!(cache.running.borrow().is_empty());
    }
}
