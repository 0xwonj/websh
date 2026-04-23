//! GitHub backend — GraphQL createCommitOnBranch + manifest fetch.
//! See spec §4.2 / §4.3.

use serde::{Deserialize, Serialize};

use crate::core::VirtualFs;
use crate::core::changes::ChangeSet;
use crate::core::merge::merge_view_for_root;
use crate::core::storage::{
    BoxFuture, CommitOutcome, ScannedSubtree, StorageBackend, StorageError, StorageResult,
};
use crate::models::VirtualPath;

use super::graphql::{BranchRef, CommitMessage, CreateCommitInput, build_file_changes};
use super::manifest::{parse_snapshot, serialize_snapshot};

#[allow(dead_code)]
pub struct GitHubBackend {
    pub repo_with_owner: String, // "0xwonj/db"
    pub branch: String,          // "main"
    pub mount_root: VirtualPath, // canonical filesystem root for this mounted subtree
    pub content_prefix: String,  // mount's content_prefix, e.g., "~"
    pub manifest_url: String,    // full URL to manifest.json (raw.githubusercontent.com)
}

#[allow(dead_code)]
impl GitHubBackend {
    pub fn new(
        repo_with_owner: impl Into<String>,
        branch: impl Into<String>,
        mount_root: VirtualPath,
        content_prefix: impl Into<String>,
        manifest_url: impl Into<String>,
    ) -> Self {
        Self {
            repo_with_owner: repo_with_owner.into(),
            branch: branch.into(),
            mount_root,
            content_prefix: content_prefix.into(),
            manifest_url: manifest_url.into(),
        }
    }

    fn content_url(&self, rel_path: &str) -> String {
        let manifest_base = self
            .manifest_url
            .trim_end_matches("/manifest.json")
            .trim_end_matches('/');
        let prefix = self.content_prefix.trim_matches('/');
        let rel_path = rel_path.trim_start_matches('/');
        match (prefix.is_empty(), rel_path.is_empty()) {
            (true, true) => manifest_base.to_string(),
            (true, false) => format!("{manifest_base}/{rel_path}"),
            (false, true) => format!("{manifest_base}/{prefix}"),
            (false, false) => format!("{manifest_base}/{prefix}/{rel_path}"),
        }
    }

    async fn load_manifest_snapshot(&self) -> StorageResult<ScannedSubtree> {
        let resp = gloo_net::http::Request::get(&self.manifest_url)
            .send()
            .await
            .map_err(|e| StorageError::NetworkError(e.to_string()))?;
        if !(200..300).contains(&resp.status()) {
            return Err(map_http_status(resp.status(), None));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| StorageError::ValidationFailed(e.to_string()))?;
        parse_snapshot(&body)
    }
}

#[derive(Serialize)]
struct GraphQLRequest<'a> {
    query: &'static str,
    variables: GraphQLVariables<'a>,
}

#[derive(Serialize)]
struct GraphQLVariables<'a> {
    input: &'a CreateCommitInput,
}

#[derive(Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
    #[serde(default)]
    errors: Vec<GraphQLErrorItem>,
}

#[derive(Deserialize)]
struct GraphQLData {
    #[serde(rename = "createCommitOnBranch")]
    create_commit_on_branch: Option<CreateCommitResult>,
}

#[derive(Deserialize)]
struct CreateCommitResult {
    commit: CommitOid,
}

#[derive(Deserialize)]
struct CommitOid {
    oid: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct GraphQLErrorItem {
    message: String,
    #[serde(rename = "type", default)]
    err_type: Option<String>,
}

const MUTATION: &str = "\
mutation ($input: CreateCommitOnBranchInput!) {
  createCommitOnBranch(input: $input) {
    commit { oid }
  }
}
";

const GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

#[allow(dead_code)]
fn map_graphql_error(errors: &[GraphQLErrorItem]) -> StorageError {
    for e in errors {
        let msg = e.message.to_lowercase();
        if msg.contains("expected") && msg.contains("head") {
            return StorageError::Conflict {
                remote_head: extract_sha(&e.message).unwrap_or_default(),
            };
        }
        if msg.contains("not authorized") || msg.contains("must have push access") {
            return StorageError::AuthFailed;
        }
        if msg.contains("could not resolve") || msg.contains("not found") {
            return StorageError::NotFound(e.message.clone());
        }
    }
    StorageError::ValidationFailed(
        errors
            .first()
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "unknown error".into()),
    )
}

#[allow(dead_code)]
fn map_http_status(status: u16, retry_after: Option<u64>) -> StorageError {
    match status {
        401 | 403 => StorageError::AuthFailed,
        404 => StorageError::NotFound(String::new()),
        409 => StorageError::Conflict {
            remote_head: String::new(),
        },
        422 => StorageError::ValidationFailed(String::new()),
        429 => StorageError::RateLimited { retry_after },
        500..=599 => StorageError::ServerError(status),
        _ => StorageError::ServerError(status),
    }
}

fn extract_sha(msg: &str) -> Option<String> {
    msg.split_whitespace()
        .find(|w| w.len() == 40 && w.chars().all(|c| c.is_ascii_hexdigit()))
        .map(String::from)
}

impl StorageBackend for GitHubBackend {
    fn backend_type(&self) -> &'static str {
        "github"
    }

    fn scan(&self) -> BoxFuture<'_, StorageResult<ScannedSubtree>> {
        Box::pin(async move { self.load_manifest_snapshot().await })
    }

    fn read_text<'a>(&'a self, rel_path: &'a str) -> BoxFuture<'a, StorageResult<String>> {
        Box::pin(async move {
            let resp = gloo_net::http::Request::get(&self.content_url(rel_path))
                .send()
                .await
                .map_err(|e| StorageError::NetworkError(e.to_string()))?;
            if !(200..300).contains(&resp.status()) {
                return Err(map_http_status(resp.status(), None));
            }
            resp.text()
                .await
                .map_err(|e| StorageError::ValidationFailed(e.to_string()))
        })
    }

    fn read_bytes<'a>(&'a self, rel_path: &'a str) -> BoxFuture<'a, StorageResult<Vec<u8>>> {
        Box::pin(async move {
            let resp = gloo_net::http::Request::get(&self.content_url(rel_path))
                .send()
                .await
                .map_err(|e| StorageError::NetworkError(e.to_string()))?;
            if !(200..300).contains(&resp.status()) {
                return Err(map_http_status(resp.status(), None));
            }
            resp.binary()
                .await
                .map_err(|e| StorageError::ValidationFailed(e.to_string()))
        })
    }

    fn commit<'a>(
        &'a self,
        changes: &'a ChangeSet,
        message: &'a str,
        expected_head: Option<&'a str>,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>> {
        Box::pin(async move {
            let token = crate::utils::session::get_gh_token().ok_or(StorageError::NoToken)?;
            let base_snapshot = self.load_manifest_snapshot().await?;
            let merged = merge_view_for_root(
                &VirtualFs::from_scanned_subtree(&base_snapshot),
                changes,
                &self.mount_root,
            );
            let manifest_body = serialize_snapshot(&merged.to_scanned_subtree())?;
            let file_changes = build_file_changes(
                changes,
                &self.mount_root,
                &self.content_prefix,
                Some(("manifest.json", &manifest_body)),
            );

            let input = CreateCommitInput {
                branch: BranchRef {
                    repo_with_owner: self.repo_with_owner.clone(),
                    branch_name: self.branch.clone(),
                },
                message: CommitMessage {
                    headline: message.to_string(),
                },
                expected_head_oid: expected_head.map(String::from),
                file_changes,
            };

            let body = GraphQLRequest {
                query: MUTATION,
                variables: GraphQLVariables { input: &input },
            };
            let body_json = serde_json::to_string(&body)
                .map_err(|e| StorageError::BadRequest(e.to_string()))?;

            let resp = gloo_net::http::Request::post(GRAPHQL_ENDPOINT)
                .header("Authorization", &format!("bearer {}", token))
                .header("Content-Type", "application/json")
                .header("User-Agent", "websh/0.1")
                .body(body_json)
                .map_err(|e| StorageError::BadRequest(e.to_string()))?
                .send()
                .await
                .map_err(|e| StorageError::NetworkError(e.to_string()))?;

            let status = resp.status();
            if !(200..300).contains(&status) {
                let retry_after = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.parse::<u64>().ok());
                return Err(map_http_status(status, retry_after));
            }

            let gql: GraphQLResponse = resp
                .json()
                .await
                .map_err(|e| StorageError::NetworkError(e.to_string()))?;

            if !gql.errors.is_empty() {
                return Err(map_graphql_error(&gql.errors));
            }

            let new_head = gql
                .data
                .and_then(|d| d.create_commit_on_branch)
                .map(|c| c.commit.oid)
                .ok_or_else(|| StorageError::ValidationFailed("empty data".into()))?;

            // GraphQL's createCommitOnBranch doesn't echo file contents; the
            // backend still regenerates the manifest privately before commit.
            let committed_paths: Vec<_> = changes.iter_staged().map(|(p, _)| p.clone()).collect();
            Ok(CommitOutcome {
                new_head,
                committed_paths,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_401_maps_auth_failed() {
        assert_eq!(map_http_status(401, None), StorageError::AuthFailed);
        assert_eq!(map_http_status(403, None), StorageError::AuthFailed);
    }

    #[test]
    fn http_429_preserves_retry_after() {
        assert_eq!(
            map_http_status(429, Some(30)),
            StorageError::RateLimited {
                retry_after: Some(30)
            }
        );
    }

    #[test]
    fn graphql_error_conflict_detected() {
        let e = vec![GraphQLErrorItem {
            message: "expected head oid abc123def456abc123def456abc123def4567890 was not current"
                .into(),
            err_type: None,
        }];
        let mapped = map_graphql_error(&e);
        assert!(matches!(mapped, StorageError::Conflict { .. }));
    }

    #[test]
    fn graphql_error_auth_detected() {
        let e = vec![GraphQLErrorItem {
            message: "must have push access".into(),
            err_type: None,
        }];
        assert_eq!(map_graphql_error(&e), StorageError::AuthFailed);
    }
}
