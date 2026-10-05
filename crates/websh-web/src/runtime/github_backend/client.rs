//! Public GitHub manifest and content reads.

use websh_core::domain::{GitHubMount, VirtualPathParseError};
use websh_core::ports::{
    LocalBoxFuture, ScannedSubtree, StorageBackend, StorageError, StorageResult,
    parse_manifest_snapshot,
};

use super::path::encoded_repo_relative_path;

pub struct GitHubBackend {
    config: GitHubMount,
}

impl GitHubBackend {
    pub fn new(config: GitHubMount) -> Self {
        Self { config }
    }

    fn base_url(&self) -> String {
        if self.config.gateway() == "self" {
            return if self.config.root().is_empty() {
                ".".to_string()
            } else {
                encoded_repo_relative_path(self.config.root())
                    .expect("normalized content prefix must be URL-encodable")
            };
        }

        let branch = encoded_repo_relative_path(self.config.branch())
            .expect("validated branch must be URL-encodable");
        if self.config.root().is_empty() {
            format!(
                "{}/{}/{}",
                self.config.gateway(),
                self.config.repo(),
                branch
            )
        } else {
            let encoded_prefix = encoded_repo_relative_path(self.config.root())
                .expect("normalized content prefix must be URL-encodable");
            format!(
                "{}/{}/{}/{}",
                self.config.gateway(),
                self.config.repo(),
                branch,
                encoded_prefix
            )
        }
    }

    fn manifest_url(&self) -> String {
        format!("{}/manifest.json", self.base_url())
    }

    fn content_url(&self, rel_path: &str) -> Result<String, VirtualPathParseError> {
        let base_url = self.base_url();
        let rel_path = encoded_repo_relative_path(rel_path)?;
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
        if self.config.mount_at().is_root() {
            return None;
        }
        let base = web_sys::Url::new_with_base(&format!("{}/", self.base_url()), document_base)
            .ok()?
            .href();
        let manifest = web_sys::Url::new_with_base("manifest.json", &base)
            .ok()?
            .href();
        Some(super::super::mount_cache::CacheDescriptor {
            root: self.config.mount_at().to_string(),
            repo: self.config.repo().to_string(),
            reference: self.config.branch().to_string(),
            prefix: self.config.root().to_string(),
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
    use serde_json::json;
    use wasm_bindgen_test::*;
    use websh_core::domain::BootstrapSiteSource;

    fn backend(gateway: &str, prefix: &str) -> GitHubBackend {
        GitHubBackend::new(
            serde_json::from_value(json!({
                "backend": "github", "mount_at": "/db", "repo": "owner/repo",
                "branch": "Main", "root": prefix, "gateway": gateway
            }))
            .unwrap(),
        )
    }

    #[wasm_bindgen_test]
    fn http_failures_preserve_auth_and_retry_information() {
        assert_eq!(map_http_status(401, None), StorageError::AuthFailed);
        assert_eq!(map_http_status(403, None), StorageError::AuthFailed);
        assert_eq!(
            map_http_status(429, Some(30)),
            StorageError::RateLimited {
                retry_after: Some(30)
            }
        );
    }

    #[wasm_bindgen_test]
    fn read_urls_encode_canonical_paths_and_reject_traversal() {
        let backend = backend("https://raw.githubusercontent.com", "~");
        assert_eq!(
            backend.manifest_url(),
            "https://raw.githubusercontent.com/owner/repo/Main/~/manifest.json"
        );
        assert_eq!(
            backend.public_read_url("docs/file #1.pdf").unwrap(),
            Some("https://raw.githubusercontent.com/owner/repo/Main/~/docs/file%20%231.pdf".into())
        );
        for path in ["../secret.md", "/absolute.md", "a//b", "a/./b"] {
            assert!(backend.public_read_url(path).is_err(), "{path}");
        }
        let branch = GitHubBackend::new(
            serde_json::from_value(json!({
                "backend": "github", "mount_at": "/db", "repo": "owner/repo",
                "branch": "feature/topic#1"
            }))
            .unwrap(),
        );
        assert_eq!(
            branch.manifest_url(),
            "https://raw.githubusercontent.com/owner/repo/feature/topic%231/manifest.json"
        );
    }

    #[wasm_bindgen_test]
    fn cache_identity_resolves_local_deployment_and_excludes_bootstrap() {
        let local = backend("self", "content");
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
        let raw = backend("https://raw.githubusercontent.com", "content");
        assert_eq!(
            raw.cache_descriptor_for_base("https://site.test/ipfs/first/")
                .unwrap(),
            raw.cache_descriptor_for_base("https://site.test/ipfs/second/")
                .unwrap()
        );
        let bootstrap = GitHubBackend::new(
            GitHubMount::bootstrap(&BootstrapSiteSource {
                repo_with_owner: "owner/repo",
                branch: "main",
                content_root: "content",
                gateway: "self",
            })
            .unwrap(),
        );
        assert!(
            bootstrap
                .cache_descriptor_for_base("https://site.test/")
                .is_none()
        );
    }
}
