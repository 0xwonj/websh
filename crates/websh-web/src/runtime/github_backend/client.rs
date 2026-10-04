//! Public GitHub manifest and content reads.

use websh_core::domain::VirtualPath;
use websh_core::ports::{
    LocalBoxFuture, ScannedSubtree, StorageBackend, StorageError, StorageResult,
    parse_manifest_snapshot,
};

use super::path::{RepoPathError, encoded_repo_relative_path, normalize_repo_prefix};

pub struct GitHubBackend {
    repo_with_owner: String,
    branch: String,
    mount_root: VirtualPath,
    content_prefix: String,
    gateway: String,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GitHubBackendConfigError {
    #[error("invalid repo_with_owner `{value}`")]
    InvalidRepo { value: String },
    #[error("invalid content prefix: {source}")]
    InvalidContentPrefix {
        #[from]
        source: RepoPathError,
    },
}

impl GitHubBackend {
    pub fn new(
        repo_with_owner: impl Into<String>,
        branch: impl Into<String>,
        mount_root: VirtualPath,
        content_prefix: impl Into<String>,
        gateway: impl Into<String>,
    ) -> Result<Self, GitHubBackendConfigError> {
        let repo_with_owner = repo_with_owner.into();
        validate_repo_with_owner(&repo_with_owner)?;
        Ok(Self {
            repo_with_owner,
            branch: branch.into(),
            mount_root,
            content_prefix: normalize_repo_prefix(&content_prefix.into())?,
            gateway: gateway.into().trim_end_matches('/').to_string(),
        })
    }

    fn base_url(&self) -> String {
        if self.gateway == "self" {
            return if self.content_prefix.is_empty() {
                ".".to_string()
            } else {
                encoded_repo_relative_path(&self.content_prefix, false)
                    .expect("normalized content prefix must be URL-encodable")
            };
        }

        if self.content_prefix.is_empty() {
            format!("{}/{}/{}", self.gateway, self.repo_with_owner, self.branch)
        } else {
            let encoded_prefix = encoded_repo_relative_path(&self.content_prefix, false)
                .expect("normalized content prefix must be URL-encodable");
            format!(
                "{}/{}/{}/{}",
                self.gateway, self.repo_with_owner, self.branch, encoded_prefix
            )
        }
    }

    fn manifest_url(&self) -> String {
        format!("{}/manifest.json", self.base_url())
    }

    fn content_url(&self, rel_path: &str) -> Result<String, RepoPathError> {
        let base_url = self.base_url();
        let rel_path = encoded_repo_relative_path(rel_path.trim_start_matches('/'), true)?;
        if rel_path.is_empty() {
            Ok(base_url)
        } else {
            Ok(format!("{base_url}/{rel_path}"))
        }
    }

    pub fn cache_descriptor(&self) -> Option<super::super::mount_cache::CacheDescriptor> {
        let document_base = web_sys::window()?.document()?.base_uri().ok()??;
        self.cache_descriptor_for_base(&document_base)
    }

    fn cache_descriptor_for_base(
        &self,
        document_base: &str,
    ) -> Option<super::super::mount_cache::CacheDescriptor> {
        if self.mount_root.is_root() {
            return None;
        }
        let base = web_sys::Url::new_with_base(&format!("{}/", self.base_url()), document_base)
            .ok()?
            .href();
        let manifest = web_sys::Url::new_with_base("manifest.json", &base)
            .ok()?
            .href();
        Some(super::super::mount_cache::CacheDescriptor {
            root: self.mount_root.to_string(),
            repo: self.repo_with_owner.clone(),
            reference: self.branch.clone(),
            prefix: self.content_prefix.clone(),
            manifest_url: manifest,
            content_url: base,
        })
    }

    async fn load_manifest_snapshot(&self) -> StorageResult<ScannedSubtree> {
        let response = crate::platform::fetch::fetch_manifest(
            &self.manifest_url(),
            super::super::mount_cache::MANIFEST_TIMEOUT_MS,
        )
        .await
        .map_err(|error| StorageError::Network {
            message: error.to_string(),
        })?;
        if response.status == 404 {
            return Err(StorageError::NotFound {
                path: self.manifest_url(),
            });
        }
        if !(200..300).contains(&response.status) {
            return Err(map_http_status(response.status, response.retry_after));
        }
        parse_manifest_snapshot(&response.body).map_err(Into::into)
    }
}

fn validate_repo_with_owner(value: &str) -> Result<(), GitHubBackendConfigError> {
    let Some((owner, name)) = value.split_once('/') else {
        return Err(GitHubBackendConfigError::InvalidRepo {
            value: value.to_string(),
        });
    };
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return Err(GitHubBackendConfigError::InvalidRepo {
            value: value.to_string(),
        });
    }
    Ok(())
}

fn map_http_status(status: u16, retry_after: Option<u64>) -> StorageError {
    match status {
        401 | 403 => StorageError::AuthFailed,
        404 => StorageError::NotFound {
            path: String::new(),
        },
        422 => StorageError::RemoteRejected {
            message: String::new(),
        },
        429 => StorageError::RateLimited { retry_after },
        500..=599 => StorageError::Server { status },
        _ => StorageError::Server { status },
    }
}

fn retry_after_header(resp: &gloo_net::http::Response) -> Option<u64> {
    resp.headers()
        .get("Retry-After")
        .and_then(|value| value.parse::<u64>().ok())
}

impl StorageBackend for GitHubBackend {
    fn scan(&self) -> LocalBoxFuture<'_, StorageResult<ScannedSubtree>> {
        Box::pin(async move { self.load_manifest_snapshot().await })
    }

    fn read_text<'a>(&'a self, rel_path: &'a str) -> LocalBoxFuture<'a, StorageResult<String>> {
        Box::pin(async move {
            let url =
                self.content_url(rel_path)
                    .map_err(|source| StorageError::InvalidRequest {
                        message: source.to_string(),
                    })?;
            let resp = gloo_net::http::Request::get(&url)
                .send()
                .await
                .map_err(|e| StorageError::Network {
                    message: e.to_string(),
                })?;
            if !(200..300).contains(&resp.status()) {
                return Err(map_http_status(resp.status(), retry_after_header(&resp)));
            }
            resp.text().await.map_err(|e| StorageError::RemoteRejected {
                message: e.to_string(),
            })
        })
    }

    fn read_bytes<'a>(&'a self, rel_path: &'a str) -> LocalBoxFuture<'a, StorageResult<Vec<u8>>> {
        Box::pin(async move {
            let url =
                self.content_url(rel_path)
                    .map_err(|source| StorageError::InvalidRequest {
                        message: source.to_string(),
                    })?;
            let resp = gloo_net::http::Request::get(&url)
                .send()
                .await
                .map_err(|e| StorageError::Network {
                    message: e.to_string(),
                })?;
            if !(200..300).contains(&resp.status()) {
                return Err(map_http_status(resp.status(), retry_after_header(&resp)));
            }
            resp.binary()
                .await
                .map_err(|e| StorageError::RemoteRejected {
                    message: e.to_string(),
                })
        })
    }

    fn public_read_url(&self, rel_path: &str) -> StorageResult<Option<String>> {
        self.content_url(rel_path)
            .map(Some)
            .map_err(|source| StorageError::InvalidRequest {
                message: source.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn http_401_maps_auth_failed() {
        assert_eq!(map_http_status(401, None), StorageError::AuthFailed);
        assert_eq!(map_http_status(403, None), StorageError::AuthFailed);
    }

    #[wasm_bindgen_test]
    fn http_429_preserves_retry_after() {
        assert_eq!(
            map_http_status(429, Some(30)),
            StorageError::RateLimited {
                retry_after: Some(30)
            }
        );
    }

    #[wasm_bindgen_test]
    fn content_url_uses_manifest_directory_as_base() {
        let backend = GitHubBackend::new(
            "owner/repo",
            "main",
            VirtualPath::root(),
            "~",
            "https://raw.githubusercontent.com",
        )
        .unwrap();

        assert_eq!(
            backend.content_url(".site/now.toml").unwrap(),
            "https://raw.githubusercontent.com/owner/repo/main/~/.site/now.toml"
        );
    }

    #[wasm_bindgen_test]
    fn content_url_encodes_path_segments() {
        let backend = GitHubBackend::new(
            "owner/repo",
            "main",
            VirtualPath::root(),
            "~",
            "https://raw.githubusercontent.com",
        )
        .unwrap();

        assert_eq!(
            backend.content_url("docs/file #1.md").unwrap(),
            "https://raw.githubusercontent.com/owner/repo/main/~/docs/file%20%231.md"
        );
    }

    #[wasm_bindgen_test]
    fn content_url_rejects_traversal_segments() {
        let backend = GitHubBackend::new(
            "owner/repo",
            "main",
            VirtualPath::root(),
            "~",
            "https://raw.githubusercontent.com",
        )
        .unwrap();

        assert!(backend.content_url("../secret.md").is_err());
    }

    #[wasm_bindgen_test]
    fn public_read_url_reuses_encoded_content_url() {
        let backend = GitHubBackend::new(
            "owner/repo",
            "main",
            VirtualPath::root(),
            "~",
            "https://raw.githubusercontent.com",
        )
        .unwrap();

        assert_eq!(
            backend.public_read_url("docs/file #1.pdf").unwrap(),
            Some("https://raw.githubusercontent.com/owner/repo/main/~/docs/file%20%231.pdf".into())
        );
    }

    #[wasm_bindgen_test]
    fn public_read_url_rejects_traversal_segments() {
        let backend = GitHubBackend::new(
            "owner/repo",
            "main",
            VirtualPath::root(),
            "~",
            "https://raw.githubusercontent.com",
        )
        .unwrap();

        assert!(backend.public_read_url("../secret.md").is_err());
    }

    #[wasm_bindgen_test]
    fn constructor_rejects_traversal_content_prefix() {
        let err = match GitHubBackend::new(
            "owner/repo",
            "main",
            VirtualPath::root(),
            "content/../other",
            "https://raw.githubusercontent.com",
        ) {
            Ok(_) => panic!("constructor should reject traversal content prefix"),
            Err(err) => err,
        };
        assert!(matches!(
            err,
            GitHubBackendConfigError::InvalidContentPrefix {
                source: RepoPathError::Traversal { path },
            } if path == "content/../other"
        ));
    }
    #[wasm_bindgen_test]
    fn self_cache_identity_resolves_deployment_prefix_while_raw_identity_is_shared() {
        let root = VirtualPath::from_absolute("/db").unwrap();
        let local =
            GitHubBackend::new("owner/repo", "Main", root.clone(), "/content/", "self").unwrap();
        let first = local
            .cache_descriptor_for_base("https://site.test/ipfs/first/")
            .unwrap();
        let second = local
            .cache_descriptor_for_base("https://site.test/ipfs/second/")
            .unwrap();
        assert_eq!(
            first.manifest_url,
            "https://site.test/ipfs/first/content/manifest.json"
        );
        assert_ne!(first.key(), second.key());
        let raw = GitHubBackend::new(
            "owner/repo",
            "Main",
            root,
            "content",
            "https://raw.githubusercontent.com",
        )
        .unwrap();
        assert_eq!(
            raw.cache_descriptor_for_base("https://site.test/ipfs/first/")
                .unwrap(),
            raw.cache_descriptor_for_base("https://site.test/ipfs/second/")
                .unwrap()
        );
        assert!(
            GitHubBackend::new("owner/repo", "Main", VirtualPath::root(), "content", "self")
                .unwrap()
                .cache_descriptor_for_base("https://site.test/")
                .is_none()
        );
    }
}
