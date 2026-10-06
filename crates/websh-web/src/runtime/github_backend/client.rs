//! A GitHub source resolves once, then every body read is bound to that snapshot.
use std::cell::RefCell;
use std::rc::Rc;

use crate::platform::fetch::fetch_bytes_bounded;
use websh_core::domain::GitHubMount;
use websh_core::ports::{
    LocalBoxFuture, ScannedSubtree, StorageBackend, StorageBackendRef, StorageError, StorageResult,
};
use websh_core::publication::{
    GitCommit, MAX_MANIFEST_BYTES, Manifest, ReleaseId, SourcePointer, SourceSnapshot,
    VerifiedRelease, verify_release,
};

const TIMEOUT: u32 = super::super::mount_cache::MANIFEST_TIMEOUT_MS;

pub struct LoadedSource {
    pub wire: SourceSnapshot,
    pub manifest: Manifest,
    pub verified: Option<Rc<VerifiedRelease>>,
    pub scan: ScannedSubtree,
}

pub struct GitHubBackend {
    config: GitHubMount,
    loaded: RefCell<Option<Rc<LoadedSource>>>,
    previous: Option<SourceSnapshot>,
}

pub fn rejected(error: impl std::fmt::Display) -> StorageError {
    StorageError::RemoteRejected {
        message: error.to_string(),
    }
}

impl GitHubBackend {
    pub fn new(config: GitHubMount) -> Self {
        Self {
            config,
            loaded: RefCell::new(None),
            previous: None,
        }
    }

    pub fn refreshing(config: GitHubMount, previous: Option<SourceSnapshot>) -> Self {
        Self {
            previous,
            ..Self::new(config)
        }
    }

    pub fn loaded(&self) -> Option<Rc<LoadedSource>> {
        self.loaded.borrow().clone()
    }

    pub fn from_snapshot(config: GitHubMount, wire: SourceSnapshot) -> StorageResult<Self> {
        let backend = Self::new(config);
        let (manifest, scan, verified) = if backend.config.mount_at().is_root() {
            let signature = wire
                .signature
                .as_ref()
                .ok_or_else(|| rejected("root signature missing"))?;
            let verified = Rc::new(
                verify_release(
                    wire.manifest.as_bytes(),
                    signature.as_bytes(),
                    &websh_site::content_trust(),
                    crate::platform::time::current_timestamp() / 1000,
                )
                .map_err(rejected)?,
            );
            (
                verified.manifest().clone(),
                verified.snapshot().clone(),
                Some(verified),
            )
        } else {
            if wire.manifest.len() > MAX_MANIFEST_BYTES {
                return Err(rejected("source index too large"));
            }
            let manifest: Manifest = serde_json::from_str(&wire.manifest).map_err(rejected)?;
            if manifest.release.is_some() || wire.signature.is_some() {
                return Err(rejected(
                    "unsigned mount must provide an unsigned source index",
                ));
            }
            let scan = manifest.validate().map_err(rejected)?;
            (manifest, scan, None)
        };
        *backend.loaded.borrow_mut() = Some(Rc::new(LoadedSource {
            wire,
            manifest,
            verified,
            scan,
        }));
        Ok(backend)
    }

    pub fn cache_descriptor(&self) -> Option<super::super::mount_cache::CacheDescriptor> {
        let trust = self.config.mount_at().is_root().then(|| {
            let policy = websh_site::content_trust();
            ReleaseId::of(
                format!(
                    "{}\0{}\0{}\0{:?}",
                    policy.site,
                    policy.public_key,
                    policy.primary_fingerprint,
                    policy.signer_fingerprints
                )
                .as_bytes(),
            )
            .to_string()
        });
        Some(super::super::mount_cache::CacheDescriptor {
            root: self.config.mount_at().to_string(),
            repo: self.config.repo().into(),
            reference: self.config.branch().into(),
            prefix: self.config.root().into(),
            trust,
        })
    }

    pub async fn at_commit(config: GitHubMount, commit: GitCommit) -> StorageResult<Self> {
        let manifest_url = config
            .snapshot_url(&commit, "manifest.json")
            .map_err(rejected)?;
        let manifest = read_text(
            &manifest_url,
            web_sys::RequestCache::Default,
            MAX_MANIFEST_BYTES,
        );
        let (manifest, signature) = if config.mount_at().is_root() {
            let signature_url = config
                .snapshot_url(&commit, "manifest.sig")
                .map_err(rejected)?;
            let (manifest, signature) = futures_util::join!(
                manifest,
                read_text(&signature_url, web_sys::RequestCache::Default, 16 * 1024)
            );
            (manifest?, Some(signature?))
        } else {
            (manifest.await?, None)
        };
        Self::from_snapshot(
            config,
            SourceSnapshot {
                commit,
                manifest,
                signature,
            },
        )
    }

    async fn resolve(&self) -> StorageResult<Rc<LoadedSource>> {
        let pointer = read_text(
            &self.config.pointer_url(),
            web_sys::RequestCache::NoCache,
            1024,
        )
        .await?;
        let pointer: SourcePointer = serde_json::from_str(&pointer).map_err(rejected)?;
        let backend = if let Some(previous) = &self.previous
            && previous.commit == pointer.commit
        {
            // Recheck time-sensitive certificate policy while reusing exact metadata bytes.
            Self::from_snapshot(self.config.clone(), previous.clone())?
        } else {
            Self::at_commit(self.config.clone(), pointer.commit).await?
        };
        backend
            .loaded()
            .ok_or_else(|| rejected("source not loaded"))
    }

    async fn body(&self, path: &str) -> StorageResult<Vec<u8>> {
        let loaded = self
            .loaded()
            .ok_or_else(|| rejected("source has not resolved"))?;
        let expected = loaded.manifest.integrity(path).map_err(rejected)?;
        let url = self
            .config
            .snapshot_url(&loaded.wire.commit, path)
            .map_err(rejected)?;
        super::super::body_cache::read_verified(url, expected).await
    }
}

async fn read_text(
    url: &str,
    cache: web_sys::RequestCache,
    max_bytes: usize,
) -> StorageResult<String> {
    let response = fetch_bytes_bounded(url, TIMEOUT, cache, max_bytes)
        .await
        .map_err(rejected)?;
    if !(200..300).contains(&response.status) {
        return Err(map_http_status(response.status, response.retry_after));
    }
    String::from_utf8(response.body).map_err(rejected)
}

pub(super) fn map_http_status(status: u16, retry_after: Option<u64>) -> StorageError {
    match status {
        401 | 403 => StorageError::AuthFailed,
        404 => StorageError::NotFound {
            path: String::new(),
        },
        429 => StorageError::RateLimited { retry_after },
        _ => StorageError::Server { status },
    }
}

impl StorageBackend for GitHubBackend {
    fn fork_for_refresh(&self) -> Option<StorageBackendRef> {
        Some(Rc::new(Self::refreshing(
            self.config.clone(),
            self.cache_snapshot(),
        )))
    }
    fn cache_snapshot(&self) -> Option<SourceSnapshot> {
        Some(self.loaded()?.wire.clone())
    }
    fn scan(&self) -> LocalBoxFuture<'_, StorageResult<ScannedSubtree>> {
        Box::pin(async move {
            if let Some(loaded) = self.loaded() {
                return Ok(loaded.scan.clone());
            }
            let loaded = self.resolve().await?;
            let scan = loaded.scan.clone();
            *self.loaded.borrow_mut() = Some(loaded);
            Ok(scan)
        })
    }
    fn read_text<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, StorageResult<String>> {
        Box::pin(async move { String::from_utf8(self.body(path).await?).map_err(rejected) })
    }
    fn read_bytes<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, StorageResult<Vec<u8>>> {
        Box::pin(self.body(path))
    }
}
