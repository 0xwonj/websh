//! GitHub storage backend implementation.
//!
//! Uses GitHub Contents API for file CRUD operations.

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;

use super::backend::{BoxFuture, StorageBackend};
use super::error::{StorageError, StorageResult};
use super::local;
use super::pending::{ChangeType, PendingChanges};
use crate::models::{DirectoryEntry, FileEntry, Manifest, Storage};
use crate::utils::current_timestamp;

/// GitHub Contents API response.
#[derive(Debug, Deserialize)]
struct GitHubContent {
    sha: String,
    #[allow(dead_code)]
    content: Option<String>,
    #[allow(dead_code)]
    encoding: Option<String>,
}

/// GitHub file create/update request.
#[derive(Debug, Serialize)]
struct GitHubCreateUpdate {
    message: String,
    content: String, // base64 encoded
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<String>, // required for updates
    branch: String,
}

/// GitHub file delete request.
#[derive(Debug, Serialize)]
struct GitHubDelete {
    message: String,
    sha: String,
    branch: String,
}

/// GitHub storage backend.
pub struct GitHubBackend {
    /// Repository owner
    owner: String,
    /// Repository name
    repo: String,
    /// Branch name
    branch: String,
    /// Content prefix (e.g., "~" for content in ~/*)
    base_path: Option<String>,
    /// Personal Access Token (loaded from localStorage on demand).
    token: Option<String>,
}

impl GitHubBackend {
    /// Create a new GitHub backend from Storage enum.
    ///
    /// Returns None if the storage is not GitHub.
    pub fn from_storage(storage: &Storage, content_prefix: Option<String>) -> Option<Self> {
        match storage {
            Storage::GitHub {
                owner,
                repo,
                branch,
            } => {
                let token = local::get_github_token();
                Some(Self {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    branch: branch.clone(),
                    base_path: content_prefix,
                    token,
                })
            }
            _ => None,
        }
    }

    /// Set authentication token.
    pub fn set_token(&mut self, token: String) {
        // Store in localStorage
        let _ = local::store_github_token(&token);
        self.token = Some(token);
    }

    /// Clear authentication token.
    pub fn clear_token(&mut self) {
        let _ = local::clear_github_token();
        self.token = None;
    }

    /// Reload token from localStorage.
    pub fn reload_token(&mut self) {
        self.token = local::get_github_token();
    }

    /// Get the full path in repo for a content path.
    fn repo_path(&self, path: &str) -> String {
        match &self.base_path {
            Some(base) => format!("{}/{}", base, path),
            None => path.to_string(),
        }
    }

    /// Get API URL for a file.
    fn contents_url(&self, path: &str) -> String {
        let repo_path = self.repo_path(path);
        format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            self.owner, self.repo, repo_path
        )
    }

    /// Make authenticated API request.
    async fn api_request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
    ) -> StorageResult<T> {
        let token = self.token.as_ref().ok_or(StorageError::NotAuthenticated)?;

        let window =
            web_sys::window().ok_or(StorageError::NetworkError("No window".to_string()))?;

        let opts = web_sys::RequestInit::new();
        opts.set_method(method);
        opts.set_mode(web_sys::RequestMode::Cors);

        if let Some(body_str) = body {
            opts.set_body(&wasm_bindgen::JsValue::from_str(body_str));
        }

        let request = web_sys::Request::new_with_str_and_init(url, &opts)
            .map_err(|_| StorageError::NetworkError("Failed to create request".to_string()))?;

        request
            .headers()
            .set("Authorization", &format!("Bearer {}", token))
            .map_err(|_| StorageError::NetworkError("Failed to set header".to_string()))?;
        request
            .headers()
            .set("Accept", "application/vnd.github.v3+json")
            .map_err(|_| StorageError::NetworkError("Failed to set header".to_string()))?;
        request
            .headers()
            .set("Content-Type", "application/json")
            .map_err(|_| StorageError::NetworkError("Failed to set header".to_string()))?;
        request
            .headers()
            .set("X-GitHub-Api-Version", "2022-11-28")
            .map_err(|_| StorageError::NetworkError("Failed to set header".to_string()))?;

        let resp = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| StorageError::NetworkError(format!("{:?}", e)))?;

        let resp: web_sys::Response = resp
            .dyn_into()
            .map_err(|_| StorageError::NetworkError("Invalid response".to_string()))?;

        let status = resp.status();
        if status == 401 {
            return Err(StorageError::NotAuthenticated);
        }
        if status == 403 {
            return Err(StorageError::PermissionDenied);
        }
        if status == 404 {
            return Err(StorageError::NotFound(url.to_string()));
        }
        if status == 409 {
            return Err(StorageError::Conflict(url.to_string()));
        }
        if status == 422 {
            return Err(StorageError::AlreadyExists(url.to_string()));
        }
        if status == 429 {
            return Err(StorageError::RateLimited);
        }
        if !resp.ok() {
            return Err(StorageError::NetworkError(format!("HTTP {}", status)));
        }

        let text = wasm_bindgen_futures::JsFuture::from(
            resp.text()
                .map_err(|_| StorageError::NetworkError("Failed to read response".to_string()))?,
        )
        .await
        .map_err(|_| StorageError::NetworkError("Failed to read response".to_string()))?
        .as_string()
        .ok_or(StorageError::NetworkError(
            "Invalid response text".to_string(),
        ))?;

        serde_json::from_str(&text)
            .map_err(|e| StorageError::BackendError(format!("JSON parse error: {}", e)))
    }

    /// Get file SHA for updates.
    async fn get_sha(&self, path: &str) -> StorageResult<Option<String>> {
        let url = self.contents_url(path);
        match self.api_request::<GitHubContent>("GET", &url, None).await {
            Ok(content) => Ok(Some(content.sha)),
            Err(StorageError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Update manifest.json with changes.
    async fn update_manifest(&self, changes: &PendingChanges) -> StorageResult<Manifest> {
        // Fetch current manifest
        let manifest_url = self.contents_url("manifest.json");
        let current: GitHubContent = self
            .api_request("GET", &manifest_url, None)
            .await
            .map_err(|_| StorageError::NotFound("manifest.json".to_string()))?;

        let content = current
            .content
            .ok_or_else(|| StorageError::BackendError("No content in manifest".to_string()))?;

        // Decode base64 content
        let decoded = base64_decode(&content)?;
        let mut manifest: Manifest = serde_json::from_str(&decoded)
            .map_err(|e| StorageError::BackendError(format!("JSON error: {}", e)))?;

        // Apply changes to manifest
        for change in changes.iter() {
            match &change.change_type {
                ChangeType::CreateFile {
                    description, meta, ..
                } => {
                    // Add new file entry
                    manifest.files.push(FileEntry {
                        path: change.path.clone(),
                        title: description.clone(),
                        size: meta.size,
                        modified: meta.modified,
                        tags: vec![],
                        encryption: meta.encryption.clone(),
                    });
                }
                ChangeType::UpdateFile { description, .. } => {
                    // Update existing file entry
                    if let Some(entry) = manifest.files.iter_mut().find(|f| f.path == change.path) {
                        if let Some(desc) = description {
                            entry.title.clone_from(desc);
                        }
                        entry.modified = Some(current_timestamp());
                    }
                }
                ChangeType::DeleteFile => {
                    manifest.files.retain(|f| f.path != change.path);
                }
                ChangeType::CreateDirectory { meta } => {
                    manifest.directories.push(DirectoryEntry {
                        path: change.path.clone(),
                        title: meta.title.clone(),
                        tags: meta.tags.clone(),
                        description: meta.description.clone(),
                        icon: meta.icon.clone(),
                        thumbnail: meta.thumbnail.clone(),
                    });
                }
                ChangeType::DeleteDirectory => {
                    manifest.directories.retain(|d| d.path != change.path);
                }
                ChangeType::CreateBinaryFile {
                    description, meta, ..
                } => {
                    // Add new binary file entry (image, etc.)
                    manifest.files.push(FileEntry {
                        path: change.path.clone(),
                        title: description.clone(),
                        size: meta.size,
                        modified: meta.modified,
                        tags: vec![],
                        encryption: meta.encryption.clone(),
                    });
                }
            }
        }

        // Upload updated manifest
        let manifest_json = serde_json::to_string_pretty(&manifest)
            .map_err(|e| StorageError::BackendError(e.to_string()))?;

        let body = GitHubCreateUpdate {
            message: "Update manifest.json".to_string(),
            content: base64_encode(&manifest_json),
            sha: Some(current.sha),
            branch: self.branch.clone(),
        };

        let body_json =
            serde_json::to_string(&body).map_err(|e| StorageError::BackendError(e.to_string()))?;

        let _: serde_json::Value = self
            .api_request("PUT", &manifest_url, Some(&body_json))
            .await?;

        Ok(manifest)
    }

    /// Create a binary file (image, etc.) with already base64-encoded content.
    pub async fn create_binary_file(
        &self,
        path: &str,
        content_base64: &str,
        message: &str,
    ) -> StorageResult<()> {
        let url = self.contents_url(path);

        let body = GitHubCreateUpdate {
            message: message.to_string(),
            content: content_base64.to_string(), // Already base64 encoded
            sha: None,
            branch: self.branch.clone(),
        };

        let body_json =
            serde_json::to_string(&body).map_err(|e| StorageError::BackendError(e.to_string()))?;

        let _: serde_json::Value = self.api_request("PUT", &url, Some(&body_json)).await?;

        Ok(())
    }
}

impl StorageBackend for GitHubBackend {
    fn backend_type(&self) -> &'static str {
        "github"
    }

    fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }

    fn get_file_sha(&self, path: &str) -> BoxFuture<'_, StorageResult<Option<String>>> {
        let path = path.to_string();
        Box::pin(async move { self.get_sha(&path).await })
    }

    fn create_file(
        &self,
        path: &str,
        content: &str,
        message: &str,
    ) -> BoxFuture<'_, StorageResult<()>> {
        let path = path.to_string();
        let content = content.to_string();
        let message = message.to_string();

        Box::pin(async move {
            let url = self.contents_url(&path);
            let encoded = base64_encode(&content);

            let body = GitHubCreateUpdate {
                message,
                content: encoded,
                sha: None,
                branch: self.branch.clone(),
            };

            let body_json = serde_json::to_string(&body)
                .map_err(|e| StorageError::BackendError(e.to_string()))?;

            let _: serde_json::Value = self.api_request("PUT", &url, Some(&body_json)).await?;

            Ok(())
        })
    }

    fn update_file(
        &self,
        path: &str,
        content: &str,
        message: &str,
    ) -> BoxFuture<'_, StorageResult<()>> {
        let path = path.to_string();
        let content = content.to_string();
        let message = message.to_string();

        Box::pin(async move {
            // Get current SHA first
            let sha = self
                .get_sha(&path)
                .await?
                .ok_or_else(|| StorageError::NotFound(path.clone()))?;

            let url = self.contents_url(&path);
            let encoded = base64_encode(&content);

            let body = GitHubCreateUpdate {
                message,
                content: encoded,
                sha: Some(sha),
                branch: self.branch.clone(),
            };

            let body_json = serde_json::to_string(&body)
                .map_err(|e| StorageError::BackendError(e.to_string()))?;

            let _: serde_json::Value = self.api_request("PUT", &url, Some(&body_json)).await?;

            Ok(())
        })
    }

    fn delete_file(&self, path: &str, message: &str) -> BoxFuture<'_, StorageResult<()>> {
        let path = path.to_string();
        let message = message.to_string();

        Box::pin(async move {
            // Get current SHA first
            let sha = self
                .get_sha(&path)
                .await?
                .ok_or_else(|| StorageError::NotFound(path.clone()))?;

            let url = self.contents_url(&path);

            let body = GitHubDelete {
                message,
                sha,
                branch: self.branch.clone(),
            };

            let body_json = serde_json::to_string(&body)
                .map_err(|e| StorageError::BackendError(e.to_string()))?;

            let _: serde_json::Value = self.api_request("DELETE", &url, Some(&body_json)).await?;

            Ok(())
        })
    }

    fn create_directory(&self, _path: &str) -> BoxFuture<'_, StorageResult<()>> {
        // GitHub doesn't have explicit directories - they're created implicitly
        Box::pin(async move { Ok(()) })
    }

    fn delete_directory(&self, _path: &str, _message: &str) -> BoxFuture<'_, StorageResult<()>> {
        // GitHub directories are deleted when all files are removed
        Box::pin(async move { Ok(()) })
    }

    fn commit(
        &self,
        changes: &PendingChanges,
        message: &str,
    ) -> BoxFuture<'_, StorageResult<Manifest>> {
        let changes = changes.clone();
        let message = message.to_string();

        Box::pin(async move {
            // Process each change in order
            for change in changes.iter() {
                match &change.change_type {
                    ChangeType::CreateFile { content, .. } => {
                        self.create_file(&change.path, content, &message).await?;
                    }
                    ChangeType::UpdateFile { content, .. } => {
                        self.update_file(&change.path, content, &message).await?;
                    }
                    ChangeType::DeleteFile => {
                        self.delete_file(&change.path, &message).await?;
                    }
                    ChangeType::CreateDirectory { .. } => {
                        self.create_directory(&change.path).await?;
                    }
                    ChangeType::DeleteDirectory => {
                        self.delete_directory(&change.path, &message).await?;
                    }
                    ChangeType::CreateBinaryFile {
                        content_base64, ..
                    } => {
                        // Binary files are uploaded directly as base64
                        self.create_binary_file(&change.path, content_base64, &message)
                            .await?;
                    }
                }
            }

            // Update manifest.json
            let manifest = self.update_manifest(&changes).await?;

            Ok(manifest)
        })
    }
}

/// Base64 encode a string.
fn base64_encode(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
}

/// Base64 decode a string (handles newlines in GitHub response).
fn base64_decode(input: &str) -> StorageResult<String> {
    use base64::Engine;
    let cleaned = input.replace(['\n', '\r'], "");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&cleaned)
        .map_err(|e| StorageError::BackendError(format!("Base64 decode error: {}", e)))?;
    String::from_utf8(decoded)
        .map_err(|e| StorageError::BackendError(format!("UTF-8 error: {}", e)))
}
