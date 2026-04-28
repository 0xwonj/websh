# GitHub Client Refactor — Future Design Spec

**Date:** 2026-04-28
**Status:** Parked (not scheduled). Reference for the next major GitHub-related feature.
**Predecessor reading:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md), [`2026-04-28-mempool-phase4-design.md`](./2026-04-28-mempool-phase4-design.md)

V1 mempool work surfaced GitHub-specific code in three layers (`core/storage/github/`, `cli/mount.rs`, deploy adjacents) plus inline transport boilerplate (User-Agent / Authorization / JSON parsing) at each call site. The shape works but compounds: each new endpoint duplicates plumbing and each fix has to be hand-applied across multiple sites. This doc captures the target design before the next batch of GitHub work lands.

## 1. Motivation

Concrete pain observed during V1 hardening:

- **Five HTTP construction sites in `core/storage/github/client.rs`** for what are really three GitHub domain operations (REST contents read, GraphQL HEAD lookup, GraphQL createCommit). Headers, auth, error mapping copy-pasted at each.
- **Two parallel GitHub access models**: wasm uses `gloo_net` direct; `cli/mount.rs` uses `gh` CLI subprocess. Same operations (verify repo, put contents) are written twice with different shapes and error semantics.
- **Endpoint knowledge leaks into callers**: `format!("https://api.github.com/repos/{}/contents/{}?ref={}", ...)` is constructed inside the backend's scan; the `Accept` and `Authorization` headers' meaning lives in the call site, not in a typed endpoint definition.
- **Error mapping is opt-in per call site**: `map_http_status` / `map_graphql_error` are easy to forget, leading to bugs like the `Conflict { remote_head: "" }` empty-SHA we shipped in Phase 1 (root cause of the V1 "remote changed (now ). run 'sync refresh'" loop).
- **No unit-testable boundary**: every test that wants to exercise GitHub semantics either talks to live GitHub (impractical in CI) or fabricates a half-mock at the StorageBackend level. There is no `StubGitHubClient`.

V1 ships in spite of this; V2 features (daemon, multi-user mempool, gated content) would amplify it.

## 2. Goals

| # | Goal | Why |
|---|---|---|
| G1 | A single `GitHubClient` trait exposes domain operations (`get_contents_raw`, `put_contents`, `get_branch_head`, `create_commit_on_branch`, `repo_exists`). | Hides protocol (REST/GraphQL) and transport (gloo_net/gh-subprocess/reqwest). |
| G2 | Two shipped impls: `WasmGitHubClient` (gloo_net + session token) and `GhSubprocessClient` (host, leverages `gh` CLI's auth). | Same trait, same domain types, same error enum on both sides. |
| G3 | A normalized `GitHubError` enum captures every reachable failure with the data the caller needs (status, retry_after, remote_head, message). | No more `Conflict { remote_head: "" }` pretending to be informative. |
| G4 | `StubGitHubClient` for in-memory tests so the storage and CLI layers gain unit-test coverage without live network. | Phase 4's CLI mount-init has zero test coverage of the GitHub-dependent path; this fixes it. |
| G5 | All existing call sites migrate to the trait. `gloo_net::http::Request` is not directly invoked outside `core::github`. | Searchability: future GitHub work has a single place to learn from. |

## 3. Non-goals

- Replacing `gh` CLI as a tool the user runs from the shell. Only the CLI's *internal* GitHub calls move to the trait.
- A general "HTTP client" abstraction. The trait is GitHub-specific by design — a generic HTTP layer is over-abstraction.
- Multi-account / org-aware token routing. V1 is single-user.
- IPFS / Pinata refactor. Different domain, different trait.

## 4. Anchor Decisions

| # | Decision | Rationale |
|---|---|---|
| A1 | New top-level module `src/core/github/` (sibling of `core/storage`) | The trait is a peer concern, not storage-internal. Storage *uses* it. |
| A2 | Trait is `async_trait(?Send)` — wasm-friendly | Leptos / wasm runtime is single-threaded; `Send` bound only forces unnecessary `Arc` and `Mutex`. |
| A3 | `Repo` is a typed newtype `Repo { owner, name }`, not bare `String` | Prevents owner/name swap bugs at compile time. |
| A4 | Endpoints live in `endpoints/<name>.rs`, each with its own DTOs | URL + headers + DTOs colocated; one file per resource. |
| A5 | Transport is internal — no `pub use http::*` | Callers compose against the domain; if we swap gloo_net for something else, callers don't notice. |
| A6 | `WasmGitHubClient` constructor takes `Option<String>` token | Public repos work without auth (60/hr); auth raises to 5000/hr. Same as today. |
| A7 | `GhSubprocessClient` shells out to `gh api` for parity with `gh auth login` token storage | Avoid a second auth model on the host side; the user's existing `gh auth status` session is the source of truth. |
| A8 | Migration is callsite-by-callsite, with the trait shipped first as additive code | No big-bang flip; old direct-fetch code can coexist until each site moves. |

## 5. Module Layout

```
src/core/github/
├── mod.rs                    // pub use; module index
├── error.rs                  // GitHubError enum + From<StorageError> bridge
├── repo.rs                   // Repo, Branch, Oid newtypes
├── client.rs                 // GitHubClient trait + auth policy
├── http_wasm.rs              // wasm transport via gloo_net (cfg-gated)
├── http_subprocess.rs        // host transport via `gh api` (cfg-gated)
├── stub.rs                   // StubGitHubClient for tests
└── endpoints/
    ├── mod.rs                // re-exports
    ├── contents.rs           // get_contents_raw, put_contents (REST)
    ├── refs.rs               // get_branch_head (GraphQL)
    ├── commits.rs            // create_commit_on_branch (GraphQL)
    └── repos.rs              // repo_exists (REST HEAD)

src/core/storage/github/
└── client.rs                 // GitHubBackend uses GitHubClient — much thinner

src/cli/mount.rs              // uses GhSubprocessClient (or shared with WasmGitHubClient sans wasm cfg)
```

## 6. Trait Sketch

```rust
// src/core/github/client.rs
use async_trait::async_trait;

#[async_trait(?Send)]
pub trait GitHubClient {
    async fn repo_exists(&self, repo: &Repo) -> Result<bool, GitHubError>;

    async fn get_contents_raw(
        &self,
        repo: &Repo,
        path: &str,
        ref_: &str,
    ) -> Result<Vec<u8>, GitHubError>;

    async fn put_contents(
        &self,
        repo: &Repo,
        path: &str,
        body: &PutContents<'_>,
    ) -> Result<PutContentsResponse, GitHubError>;

    async fn get_branch_head(
        &self,
        repo: &Repo,
        branch: &str,
    ) -> Result<Option<Oid>, GitHubError>;

    async fn create_commit_on_branch(
        &self,
        input: &CreateCommitOnBranchInput<'_>,
    ) -> Result<Oid, GitHubError>;
}

pub struct PutContents<'a> {
    pub message: &'a str,
    pub content_base64: &'a str,
    pub branch: &'a str,
    pub sha: Option<&'a str>, // for updates
}
```

```rust
// src/core/github/error.rs
#[derive(Clone, Debug)]
pub enum GitHubError {
    AuthFailed,
    NotFound { path: String },
    Conflict { remote_head: Option<Oid> },
    RateLimited { retry_after: Option<u64> },
    NetworkError(String),
    BadRequest(String),
    ServerError(u16),
    GraphQLError { messages: Vec<String> },
}
```

`StorageError::From<GitHubError>` in `core/storage/error.rs` keeps the storage trait surface intact.

## 7. Migration Plan

**Phase A — additive scaffolding.** Land `core/github/` with trait, two impls, error enum, stub. No call sites move. Compile-time no-op.

**Phase B — storage backend migration.** `GitHubBackend` accepts a `Box<dyn GitHubClient>` (or constructed with one) and replaces direct `gloo_net` calls. `StorageBackend::scan` keeps its `auth_token: Option<&str>` parameter — the backend constructs (or reuses) a `WasmGitHubClient { token }` for each call. Reviewer-agent + integration tests run against `StubGitHubClient`.

**Phase C — CLI migration.** `cli/mount.rs` switches `gh_succeeds(...)` and `Process::new("gh").args([...])` to the same trait via `GhSubprocessClient`. Same code paths gain stub-test coverage.

**Phase D — cleanups.** Delete the now-unused REST URL helpers (`prefixed_repo_path` + raw URL formatter), inline GraphQL strings (move into `endpoints/`), and the `extract_sha` regex. CI gate: forbid `gloo_net::http::Request` outside `core/github/http_wasm.rs` via a grep test.

Each phase is its own PR with the same per-phase workflow as the V1 mempool docs (design + plan + reviewer agent).

## 8. Test Strategy

| Tier | Coverage |
|---|---|
| Unit | `StubGitHubClient` exercises every endpoint — happy path, 4xx mapping, 5xx mapping, GraphQL errors, conflict with/without SHA. |
| Integration | `tests/github_client.rs` (new) drives `WasmGitHubClient` against a mock HTTP server (or stub) for the full request shape. |
| Property | `Repo::parse("owner/name")` round-trips; URL encoding of weird paths. |
| Smoke | Optional live test gated on `GITHUB_TOKEN` env (off in CI by default). |

Storage and CLI tests no longer construct GitHub responses directly — they pass a `StubGitHubClient` configured with the canned outcomes they need.

## 9. Risks

| Risk | Mitigation |
|---|---|
| Big-bang refactor breaks unrelated tests | Phased migration; each phase passes existing tests before next starts. |
| Wasm async-trait overhead | `async_trait(?Send)` keeps it cheap; benchmark only if commit-path latency regresses. |
| `gh` subprocess spawn cost on host | Acceptable — CLI calls are infrequent and one-shot. Optional: long-lived `gh api --paginate` not required. |
| Auth token leakage via error string | Sanitize `GitHubError::BadRequest` / `NetworkError` to never echo back the request body / Authorization header. |
| Migration introduces a regression nobody catches | Phase B and C each ship with reviewer agent + a manual smoke against the deployed mempool flow. |

## 10. Acceptance

The refactor is complete when:

1. Phases A–D shipped as separate PRs.
2. `rg gloo_net::http::Request /Users/wonj/Projects/websh/src` returns matches only inside `core/github/http_wasm.rs`.
3. `rg "Process::new\(\"gh\"" /Users/wonj/Projects/websh/src` returns matches only inside `core/github/http_subprocess.rs`.
4. Storage and CLI test suites have unit coverage for the post-migration paths via `StubGitHubClient`.
5. V1 mempool flow (compose / promote / mount-init / attest-via-deploy) works end-to-end after migration.

## 11. Open Questions

- **Does `GhSubprocessClient` need to support `gh api graphql`?** GitHub's `gh api graphql` is a thing; using it parity-keeps the host with the wasm GraphQL path. Decision deferred to Phase A.
- **Should `WasmGitHubClient` cache the branch HEAD per repo for the session?** Phase 4's `apply_commit_outcome` already does this at the runtime layer; the client could be stateless. Default to stateless for now.
- **`Bytes` vs `Vec<u8>` for raw content reads?** Probably `Vec<u8>` — wasm doesn't benefit from `bytes` crate's reference counting in single-threaded contexts.
- **Should we expose a "create or update" `put_contents`?** The Contents API supports it via the `sha` field. Leaving it on the trait makes mount-init's bootstrap idempotent in one call.

## 12. Scheduling

This is **parked**, not active. Trigger conditions for promoting to active:

1. **A new GitHub-touching V2 feature is queued** (daemon, multi-account, gated reads). Refactor before adding the feature; the feature gets the clean trait.
2. **A GitHub-related bug crosses both wasm and CLI sides simultaneously.** That's the signal that the parallel models are now actively costing more than they save.
3. **Manual prompt**: someone explicitly schedules a quarter-of-a-week to do it.

Until then, this doc is the blueprint to consult before adding *any* new GitHub call.
