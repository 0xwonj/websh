//! IndexedDB persistence for drafts and metadata.
//!
//! Public API: `open_db`, `save_draft`, `load_draft`, `save_metadata`, `load_metadata`.
#![allow(dead_code)]

use idb::event::DatabaseEvent;
use idb::{Database, Factory, ObjectStoreParams, TransactionMode};
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::JsValue;

use crate::domain::changes::ChangeSet;
use crate::storage::{StorageError, StorageResult};

const DB_NAME: &str = "websh-state";
const DB_VERSION: u32 = 1;
pub const STORE_DRAFTS: &str = "drafts";
pub const STORE_METADATA: &str = "metadata";

#[derive(Serialize, Deserialize)]
struct DraftRecord {
    #[serde(rename = "mount_id")]
    draft_id: String,
    #[serde(flatten)]
    changes: ChangeSet,
}

#[derive(Serialize, Deserialize)]
struct MetadataRecord {
    key: String,
    value: String,
}

pub async fn open_db() -> StorageResult<Database> {
    let factory = Factory::new().map_err(idb_err)?;
    let mut req = factory.open(DB_NAME, Some(DB_VERSION)).map_err(idb_err)?;

    req.on_upgrade_needed(|event| {
        let db = event.database().expect("upgrade db");
        if !db.store_names().iter().any(|n| n == STORE_DRAFTS) {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("mount_id")));
            db.create_object_store(STORE_DRAFTS, params)
                .expect("create drafts store");
        }
        if !db.store_names().iter().any(|n| n == STORE_METADATA) {
            let mut params = ObjectStoreParams::new();
            params.key_path(Some(idb::KeyPath::new_single("key")));
            db.create_object_store(STORE_METADATA, params)
                .expect("create metadata store");
        }
    });

    req.await.map_err(idb_err)
}

pub async fn save_draft(db: &Database, draft_id: &str, changes: &ChangeSet) -> StorageResult<()> {
    let tx = db
        .transaction(&[STORE_DRAFTS], TransactionMode::ReadWrite)
        .map_err(idb_err)?;
    let store = tx.object_store(STORE_DRAFTS).map_err(idb_err)?;
    let record = DraftRecord {
        draft_id: draft_id.to_string(),
        changes: changes.clone(),
    };
    let value = record
        .serialize(&Serializer::json_compatible())
        .map_err(|e| StorageError::BadRequest(format!("serialize: {e}")))?;
    store
        .put(&value, None)
        .map_err(idb_err)?
        .await
        .map_err(idb_err)?;
    tx.commit().map_err(idb_err)?.await.map_err(idb_err)?;
    Ok(())
}

pub async fn load_draft(db: &Database, draft_id: &str) -> StorageResult<Option<ChangeSet>> {
    let tx = db
        .transaction(&[STORE_DRAFTS], TransactionMode::ReadOnly)
        .map_err(idb_err)?;
    let store = tx.object_store(STORE_DRAFTS).map_err(idb_err)?;
    let value: Option<JsValue> = store
        .get(JsValue::from_str(draft_id))
        .map_err(idb_err)?
        .await
        .map_err(idb_err)?;
    match value {
        None => Ok(None),
        Some(v) => {
            let record: DraftRecord = serde_wasm_bindgen::from_value(v)
                .map_err(|e| StorageError::BadRequest(format!("deserialize: {e}")))?;
            Ok(Some(record.changes))
        }
    }
}

pub async fn save_metadata(db: &Database, key: &str, value: &str) -> StorageResult<()> {
    let tx = db
        .transaction(&[STORE_METADATA], TransactionMode::ReadWrite)
        .map_err(idb_err)?;
    let store = tx.object_store(STORE_METADATA).map_err(idb_err)?;
    let record = MetadataRecord {
        key: key.to_string(),
        value: value.to_string(),
    };
    let js = record
        .serialize(&Serializer::json_compatible())
        .map_err(|e| StorageError::BadRequest(format!("serialize: {e}")))?;
    store
        .put(&js, None)
        .map_err(idb_err)?
        .await
        .map_err(idb_err)?;
    tx.commit().map_err(idb_err)?.await.map_err(idb_err)?;
    Ok(())
}

pub async fn load_metadata(db: &Database, key: &str) -> StorageResult<Option<String>> {
    let tx = db
        .transaction(&[STORE_METADATA], TransactionMode::ReadOnly)
        .map_err(idb_err)?;
    let store = tx.object_store(STORE_METADATA).map_err(idb_err)?;
    let value: Option<JsValue> = store
        .get(JsValue::from_str(key))
        .map_err(idb_err)?
        .await
        .map_err(idb_err)?;
    match value {
        None => Ok(None),
        Some(v) => {
            let record: MetadataRecord = serde_wasm_bindgen::from_value(v)
                .map_err(|e| StorageError::BadRequest(format!("deserialize: {e}")))?;
            Ok(Some(record.value))
        }
    }
}

fn idb_err<E: std::fmt::Display>(e: E) -> StorageError {
    let s = e.to_string().to_lowercase();
    if s.contains("quotaexceeded") {
        StorageError::BadRequest("local draft storage full. discard or commit to free space".into())
    } else {
        StorageError::NetworkError(format!("idb: {e}"))
    }
}
