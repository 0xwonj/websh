//! Bounded verified byte reuse across source commits. Request coalescing remains URL-scoped.
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};

use futures_util::FutureExt;
use websh_core::{
    ports::{LocalBoxFuture, StorageError, StorageResult},
    publication::FileIntegrity,
};

use super::mount_cache::BrowserMountCache;
use crate::platform::fetch::fetch_bytes_bounded;

const MEMORY_BYTES: usize = 16 * 1024 * 1024;
const MEMORY_ENTRIES: usize = 128;
const MAX_INFLIGHT: usize = 64;
type SharedRead = futures_util::future::Shared<LocalBoxFuture<'static, StorageResult<Vec<u8>>>>;

thread_local! {
    static MEMORY: RefCell<BodyCache> = RefCell::new(BodyCache::default());
    static INFLIGHT: RefCell<BTreeMap<String, SharedRead>> = RefCell::new(BTreeMap::new());
    static DISK: BrowserMountCache = BrowserMountCache::default();
}

#[derive(Default)]
struct BodyCache {
    entries: BTreeMap<String, Rc<Vec<u8>>>,
    order: VecDeque<String>,
    size: usize,
}

impl BodyCache {
    fn get(&mut self, expected: &FileIntegrity) -> Option<Vec<u8>> {
        let key = expected.cache_key();
        let bytes = self.entries.get(&key)?;
        // Evidence belongs to this read, not a serialized "verified" flag.
        expected.verify(bytes).ok()?;
        let bytes = bytes.as_ref().clone();
        self.order.retain(|existing| existing != &key);
        self.order.push_back(key);
        Some(bytes)
    }

    fn insert(&mut self, expected: &FileIntegrity, bytes: Vec<u8>) {
        if bytes.len() > MEMORY_BYTES || expected.verify(&bytes).is_err() {
            return;
        }
        let key = expected.cache_key();
        if let Some(old) = self.entries.remove(&key) {
            self.size -= old.len();
        }
        self.order.retain(|existing| existing != &key);
        self.size += bytes.len();
        self.entries.insert(key.clone(), Rc::new(bytes));
        self.order.push_back(key);
        while self.size > MEMORY_BYTES || self.entries.len() > MEMORY_ENTRIES {
            if let Some(old) = self
                .order
                .pop_front()
                .and_then(|key| self.entries.remove(&key))
            {
                self.size -= old.len();
            } else {
                break;
            }
        }
    }
}

fn rejected(error: impl std::fmt::Display) -> StorageError {
    StorageError::RemoteRejected {
        message: error.to_string(),
    }
}

pub async fn read_verified(url: String, expected: FileIntegrity) -> StorageResult<Vec<u8>> {
    if let Some(bytes) = MEMORY.with(|cache| cache.borrow_mut().get(&expected)) {
        return Ok(bytes);
    }
    let request_key = format!("{}:{url}", expected.cache_key());
    let read = INFLIGHT.with(|inflight| {
        let mut inflight = inflight.borrow_mut();
        if let Some(read) = inflight.get(&request_key) {
            return read.clone();
        }
        let future: LocalBoxFuture<'static, StorageResult<Vec<u8>>> = Box::pin(load(url, expected));
        let read = future.shared();
        // An abandoned caller must not leave an unbounded set of unpolled requests.
        if inflight.len() >= MAX_INFLIGHT {
            inflight.pop_first();
        }
        inflight.insert(request_key.clone(), read.clone());
        read
    });
    let result = read.clone().await;
    INFLIGHT.with(|inflight| {
        let mut inflight = inflight.borrow_mut();
        if inflight
            .get(&request_key)
            .is_some_and(|current| current.ptr_eq(&read))
        {
            inflight.remove(&request_key);
        }
    });
    result
}

async fn load(url: String, expected: FileIntegrity) -> StorageResult<Vec<u8>> {
    let disk = DISK.with(Clone::clone);
    // The IndexedDB owner bounds this to 500 ms, including blocked open. No permission prompt.
    if let Some(bytes) = disk.restore_body(&expected).await {
        MEMORY.with(|cache| cache.borrow_mut().insert(&expected, bytes.clone()));
        return Ok(bytes);
    }
    let max_size = usize::try_from(expected.size).map_err(rejected)?;
    let response = fetch_bytes_bounded(
        &url,
        super::mount_cache::MANIFEST_TIMEOUT_MS,
        web_sys::RequestCache::Default,
        max_size,
    )
    .await
    .map_err(rejected)?;
    if !(200..300).contains(&response.status) {
        return Err(match response.status {
            401 | 403 => StorageError::AuthFailed,
            404 => StorageError::NotFound { path: url },
            429 => StorageError::RateLimited {
                retry_after: response.retry_after,
            },
            status => StorageError::Server { status },
        });
    }
    expected.verify(&response.body).map_err(rejected)?;
    MEMORY.with(|cache| cache.borrow_mut().insert(&expected, response.body.clone()));
    let stored = response.body.clone();
    wasm_bindgen_futures::spawn_local(async move {
        disk.persist_body(expected, stored).await;
    });
    Ok(response.body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use websh_core::publication::ReleaseId;

    #[wasm_bindgen::prelude::wasm_bindgen(
        inline_js = "export function bodyFetchFixture(body) { const original = window.fetch; let count = 0; window.fetch = async input => { const url = typeof input === 'string' ? input : input.url; if (!url.startsWith('https://body-fixture.invalid/')) return original(input); count++; await new Promise(resolve => setTimeout(resolve, 20)); return new Response(url.includes('corrupt') ? body.replace(/./g, 'x') : body, {status: 200}); }; return restore => { if (restore) window.fetch = original; return count; }; }"
    )]
    extern "C" {
        fn bodyFetchFixture(body: &str) -> js_sys::Function;
    }

    #[wasm_bindgen_test(async)]
    async fn simultaneous_reads_coalesce_and_verified_body_survives_commit_change() {
        let body = format!("body fixture {}", js_sys::Date::now());
        let expected = FileIntegrity {
            sha256: ReleaseId::of(body.as_bytes()),
            size: body.len() as u64,
        };
        let calls = bodyFetchFixture(&body);
        let url = "https://body-fixture.invalid/commit-a/file".to_string();
        let (first, second) = futures_util::join!(
            read_verified(url.clone(), expected.clone()),
            read_verified(url, expected.clone())
        );
        assert_eq!(first.unwrap(), body.as_bytes());
        assert_eq!(second.unwrap(), body.as_bytes());
        assert_eq!(
            calls
                .call1(&wasm_bindgen::JsValue::UNDEFINED, &false.into())
                .unwrap()
                .as_f64(),
            Some(1.0)
        );
        assert_eq!(
            read_verified(
                "https://body-fixture.invalid/commit-b/file".into(),
                expected.clone()
            )
            .await
            .unwrap(),
            body.as_bytes()
        );
        assert_eq!(
            calls
                .call1(&wasm_bindgen::JsValue::UNDEFINED, &false.into())
                .unwrap()
                .as_f64(),
            Some(1.0)
        );
        MEMORY.with(|cache| *cache.borrow_mut() = BodyCache::default());
        // Different digest cannot reuse the accepted body even at the same URL.
        let wrong = FileIntegrity {
            sha256: ReleaseId::of(b"a different body"),
            size: body.len() as u64,
        };
        assert!(
            read_verified("https://body-fixture.invalid/corrupt".into(), wrong)
                .await
                .is_err()
        );
        calls
            .call1(&wasm_bindgen::JsValue::UNDEFINED, &true.into())
            .unwrap();
    }

    #[wasm_bindgen_test]
    fn memory_reuses_exact_bytes_and_bounds_entries_and_size() {
        let mut cache = BodyCache::default();
        let expected = FileIntegrity {
            sha256: ReleaseId::of(b"same content"),
            size: 12,
        };
        cache.insert(&expected, b"same content".to_vec());
        assert_eq!(cache.get(&expected).unwrap(), b"same content");
        cache.insert(&expected, b"wrong".to_vec());
        assert_eq!(cache.get(&expected).unwrap(), b"same content");
        for n in 0..130u16 {
            let bytes = n.to_be_bytes().to_vec();
            let integrity = FileIntegrity {
                sha256: ReleaseId::of(&bytes),
                size: bytes.len() as u64,
            };
            cache.insert(&integrity, bytes);
        }
        assert_eq!(cache.entries.len(), MEMORY_ENTRIES);
        assert!(cache.get(&expected).is_none());
        let bytes = vec![1; MEMORY_BYTES];
        let large = FileIntegrity {
            sha256: ReleaseId::of(&bytes),
            size: bytes.len() as u64,
        };
        cache.insert(&large, bytes);
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.size, MEMORY_BYTES);
    }
}
