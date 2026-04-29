//! GitHub backend — GraphQL createCommitOnBranch + manifest fetch.
//! See spec §4.2 / §4.3.

use serde::{Deserialize, Serialize};

use crate::core::storage::{
    BoxFuture, CommitOutcome, CommitRequest, ScannedSubtree, StorageBackend, StorageError,
    StorageResult,
};
use crate::models::VirtualPath;

use super::graphql::{BranchRef, CommitMessage, CreateCommitInput, build_file_changes};
use super::manifest::{parse_snapshot, serialize_snapshot};
use super::path::{encoded_repo_relative_path, normalize_repo_prefix, prefixed_repo_path};

#[allow(dead_code)]
pub struct GitHubBackend {
    repo_with_owner: String,
    branch: String,
    mount_root: VirtualPath,
    content_prefix: String,
    gateway: String,
}

#[allow(dead_code)]
impl GitHubBackend {
    pub fn new(
        repo_with_owner: impl Into<String>,
        branch: impl Into<String>,
        mount_root: VirtualPath,
        content_prefix: impl Into<String>,
        gateway: impl Into<String>,
    ) -> Result<Self, String> {
        Ok(Self {
            repo_with_owner: repo_with_owner.into(),
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

    fn content_url(&self, rel_path: &str) -> Result<String, String> {
        let base_url = self.base_url();
        let rel_path = encoded_repo_relative_path(rel_path.trim_start_matches('/'), true)?;
        if rel_path.is_empty() {
            Ok(base_url)
        } else {
            Ok(format!("{base_url}/{rel_path}"))
        }
    }

    /// Fetch the current HEAD OID of the configured branch via the GitHub
    /// GraphQL API. Returns `None` if the branch exists but has no commits
    /// (very rare — typically only just-initialized branches), and an error
    /// for network / auth / not-found conditions.
    async fn fetch_branch_head_oid(&self, token: &str) -> StorageResult<Option<String>> {
        let (owner, name) = self
            .repo_with_owner
            .split_once('/')
            .ok_or_else(|| StorageError::BadRequest("invalid repo_with_owner".into()))?;
        let body = HeadQueryRequest {
            query: HEAD_QUERY,
            variables: HeadQueryVariables {
                owner,
                name,
                qualified_name: format!("refs/heads/{}", self.branch),
            },
        };
        let body_json =
            serde_json::to_string(&body).map_err(|e| StorageError::BadRequest(e.to_string()))?;
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
            return Err(map_http_status(status, None));
        }
        let parsed: HeadQueryResponse = resp
            .json()
            .await
            .map_err(|e| StorageError::NetworkError(e.to_string()))?;
        if !parsed.errors.is_empty() {
            return Err(map_graphql_error(&parsed.errors));
        }
        Ok(parsed
            .data
            .and_then(|d| d.repository)
            .and_then(|r| r.ref_)
            .and_then(|r| r.target)
            .map(|t| t.oid))
    }

    async fn load_manifest_snapshot(
        &self,
        auth_token: Option<&str>,
    ) -> StorageResult<ScannedSubtree> {
        // Read the manifest through the authoritative GitHub Contents API
        // instead of `raw.githubusercontent.com`. raw is fronted by a CDN
        // that lags behind git storage by minutes after a fresh commit, so
        // a post-commit reload would otherwise see a stale tree until the
        // edge expired. The Contents API hits git directly. With
        // `Accept: application/vnd.github.raw` GitHub returns the file body
        // directly (no base64 envelope).
        let manifest_repo_path = prefixed_repo_path(&self.content_prefix, "manifest.json")
            .map_err(StorageError::BadRequest)?;
        let encoded_path = encoded_repo_relative_path(&manifest_repo_path, true)
            .map_err(StorageError::BadRequest)?;
        let url = format!(
            "https://api.github.com/repos/{}/contents/{}?ref={}",
            self.repo_with_owner, encoded_path, self.branch,
        );
        // api.github.com sets `cache-control: public, max-age=60, s-maxage=60`,
        // so the browser fetch cache will serve a 60-second-stale manifest
        // after a fresh commit. NoCache forces revalidation on every scan;
        // GitHub still returns ETag-based 304s, so unchanged manifests don't
        // pay the body-transfer cost — only the round-trip to verify.
        let mut request = gloo_net::http::Request::get(&url)
            .cache(web_sys::RequestCache::NoCache)
            .header("Accept", "application/vnd.github.raw")
            .header("User-Agent", "websh/0.1");
        if let Some(token) = auth_token {
            request = request.header("Authorization", &format!("bearer {token}"));
        }
        let resp = request
            .send()
            .await
            .map_err(|e| StorageError::NetworkError(e.to_string()))?;
        // A missing manifest is the canonical signal of a fresh / empty
        // mount: the first commit atomically creates both the file and the
        // manifest. Treat 404 as "no entries yet" so newly-provisioned
        // GitHub mounts can self-bootstrap without a manual seed.
        if resp.status() == 404 {
            return Ok(ScannedSubtree::default());
        }
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

const HEAD_QUERY: &str = "\
query ($owner: String!, $name: String!, $qualifiedName: String!) {
  repository(owner: $owner, name: $name) {
    ref(qualifiedName: $qualifiedName) {
      target { oid }
    }
  }
}
";

#[derive(Serialize)]
struct HeadQueryVariables<'a> {
    owner: &'a str,
    name: &'a str,
    #[serde(rename = "qualifiedName")]
    qualified_name: String,
}

#[derive(Serialize)]
struct HeadQueryRequest<'a> {
    query: &'static str,
    variables: HeadQueryVariables<'a>,
}

#[derive(Deserialize)]
struct HeadQueryResponse {
    data: Option<HeadQueryData>,
    #[serde(default)]
    errors: Vec<GraphQLErrorItem>,
}

#[derive(Deserialize)]
struct HeadQueryData {
    repository: Option<HeadQueryRepository>,
}

#[derive(Deserialize)]
struct HeadQueryRepository {
    #[serde(rename = "ref")]
    ref_: Option<HeadQueryRef>,
}

#[derive(Deserialize)]
struct HeadQueryRef {
    target: Option<HeadQueryTarget>,
}

#[derive(Deserialize)]
struct HeadQueryTarget {
    oid: String,
}

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

    fn scan<'a>(
        &'a self,
        auth_token: Option<&'a str>,
    ) -> BoxFuture<'a, StorageResult<ScannedSubtree>> {
        Box::pin(async move { self.load_manifest_snapshot(auth_token).await })
    }

    fn read_text<'a>(&'a self, rel_path: &'a str) -> BoxFuture<'a, StorageResult<String>> {
        Box::pin(async move {
            let url = self
                .content_url(rel_path)
                .map_err(StorageError::BadRequest)?;
            let resp = gloo_net::http::Request::get(&url)
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
            let url = self
                .content_url(rel_path)
                .map_err(StorageError::BadRequest)?;
            let resp = gloo_net::http::Request::get(&url)
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
        request: &'a CommitRequest,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>> {
        Box::pin(async move {
            let token = request.auth_token.as_deref().ok_or(StorageError::NoToken)?;
            let manifest_body = serialize_snapshot(&request.merged_snapshot)?;
            let manifest_repo_path = prefixed_repo_path(&self.content_prefix, "manifest.json")
                .map_err(StorageError::BadRequest)?;
            let file_changes = build_file_changes(
                &request.delta,
                &self.mount_root,
                &self.content_prefix,
                Some((manifest_repo_path.as_str(), &manifest_body)),
            )
            .map_err(StorageError::BadRequest)?;

            // GitHub's `createCommitOnBranch` mutation requires
            // `expectedHeadOid`. On the first UI-driven commit to a mount
            // there is nothing in IndexedDB to seed `remote_heads`, so the
            // runtime passes `None` here. Fetch the current branch HEAD
            // before committing to avoid the chicken-and-egg "remote
            // changed (now ). run 'sync refresh'" failure.
            let expected_head_oid = match request.expected_head.clone() {
                Some(head) => Some(head),
                None => self.fetch_branch_head_oid(token).await?,
            };

            let input = CreateCommitInput {
                branch: BranchRef {
                    repo_with_owner: self.repo_with_owner.clone(),
                    branch_name: self.branch.clone(),
                },
                message: CommitMessage {
                    headline: request.message.clone(),
                },
                expected_head_oid,
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

            Ok(CommitOutcome {
                new_head,
                committed_paths: request.cleanup_paths.clone(),
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

    #[test]
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
            backend.content_url(".websh/site.json").unwrap(),
            "https://raw.githubusercontent.com/owner/repo/main/~/.websh/site.json"
        );
    }

    #[test]
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

    #[test]
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

    #[test]
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
        assert!(err.contains("traversal"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn commit_requires_token_from_request() {
        let backend = GitHubBackend::new(
            "owner/repo",
            "main",
            VirtualPath::root(),
            "~",
            "https://raw.githubusercontent.com",
        )
        .unwrap();
        let request = CommitRequest {
            delta: crate::core::storage::CommitDelta::default(),
            cleanup_paths: vec![],
            merged_snapshot: ScannedSubtree::default(),
            message: "msg".to_string(),
            expected_head: None,
            auth_token: None,
        };

        let err = backend.commit(&request).await.unwrap_err();
        assert_eq!(err, StorageError::NoToken);
    }
}
