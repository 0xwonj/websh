# Read only browser and external mount cache design

Status: implemented; see [verification evidence and release blockers](progress.md).

Date: 2026-10-04. Audited baseline: `ce1d7a16cf1653baa80af71f90fcdf98178c755d`.

Websh will keep its wallet-connected reader, ledger, directory views, mempool listings, and terminal navigation. Content authoring will happen through local files, Git, and the native CLI. The browser will have no content-writing capability, author mode, staged changes, draft persistence, GitHub credentials, or commit protocol. External mount listings will optionally load from a disposable IndexedDB cache while their public manifest is refreshed.

This document specifies the target behavior. [The removal inventory](removal-inventory.md) maps that behavior to the audited implementation. [The implementation plan](implementation-plan.md) defines sequencing and acceptance checks. Until implementation lands, [current architecture](../../architecture/current.md) describes the running code.

## Scope and decisions

| Decision | Target |
| --- | --- |
| Browser authoring | Remove both the reader's direct save path and the terminal's staged editing path, including their supporting code. |
| Wallet connection | Preserve connection, logout, restoration, account and chain events, ENS lookup, identity display, and existing read-related behavior. |
| Write authority | Remove the browser administrator allowlist, author mode, and writable mount flags. A connected wallet grants no content-writing capability. |
| Native authoring | Preserve CLI content generation, mempool add/promote/drop, Git/GitHub operations, attestations, and deployment. |
| Terminal | Retain exploration, pipes, wallet commands, environment variables, and presentation controls. Introduce a read-only `refresh [path]` command. |
| Persistent cache | Cache validated external manifest snapshots only. Do not cache authored drafts, commit heads, file bodies, credentials, or wallet state in it. |
| IndexedDB | Create `websh-cache` version 1. Stop using `websh-state` version 3; leave its existing user data untouched. |
| Root content | Load the deployment's bundled root manifest normally. Do not restore the root from the external cache. |
| Failure behavior | Storage failures never prevent live reads. Refresh failure preserves an already usable listing. |
| Compatibility | Ignore legacy `writable` JSON fields; remove `/new` special routing and the `sync` command namespace. Reject obsolete credential commands before echo/history. |
| Complexity limit | No write feature flag, dormant author subsystem, outbox, generic persistence framework, service worker, or cross-tab refresh leader. |

“Read only” means the browser cannot author or publish content. Local preferences, wallet sessions, disposable cache records, and in-memory filesystem assembly can still change.

## Current behavior and why it needs more than UI removal

There are two independent browser writing systems. The reader's `/new` and mempool editing flow calls `save_raw`, builds a `ChangeSet`, and commits directly. The terminal uses an edit modal, stages filesystem changes, persists drafts, and commits through `sync`. Hiding one set of controls would leave the other system and their shared services intact.

`AppContext` currently carries `changes`, `drafts_hydrated`, `editor_open`, `remote_heads`, wallet state, and multiple filesystem projections. `App` restores and saves drafts at startup. `StorageBackend` requires both reads and commits. GitHub's browser adapter includes GraphQL HEAD queries, commit-base reads, and a commit mutation.

Current IndexedDB stores `draft_changes` and `metadata`, including commit heads, in `websh-state` v3. The old snapshot note assumes v1 to v2 and cannot be applied as written. The current snapshot DTO contains paths, metadata, and mempool extensions; it contains no file bodies.

Two existing read races also matter for the new cache: root request ordering is currently guarded too late, and text cache keys have no per-mount revision. The proposed lifecycle closes both gaps rather than adding another source of late results to them.

## User visible behavior

### Reader and mempool

Remove composition, edit/preview/save/cancel controls, the editor modal, dirty/saving states, editor shortcuts, PAT entry, and submission controls. Keep rendered documents, PDFs, images, text, redirects, language variants, text-size controls, link copying, metadata, attestations, and normal navigation.

Keep mempool listings, filters, collapse state, publication links, and externally authored draft/review status. A mempool item called a draft is a publication record; it is not the browser's deleted draft-storage feature.

`/new` ceases to be a built-in route. It uses ordinary content resolution and returns the normal missing-route view if no content claims it. A future real content route named `/new` must work. No permanent redirect or reserved placeholder remains.

### Terminal

Retain `ls`, `cd`, `pwd`, `cat`, `grep`, `head`, `tail`, `wc`, pipes, variable/history expansion, `echo`, `export`, `unset`, `help`, `whoami`, `id`, `theme`, `clear`, `login`, and `logout` as applicable to the existing command model.

Remove `edit`, `touch`, `mkdir`, `rm`, `rmdir`, staging effects, and all `sync` subcommands from parsing, execution, help, and completion. Add `refresh [path]`: resolve the current directory or supplied path to its most specific accepted mount declaration and re-fetch that mount's manifest. A path inside a loading or failed accepted mount remains refreshable even if that path is not in the current listing. Selecting the root mount performs a full root/declaration reload. Reject paths outside a known mount and runtime-overlay paths. A rejected declaration requires a corrected declaration and root refresh; do not create a backend from invalid configuration. Root and external failure panels use the same service as this command.

The final shell has no file-output redirection grammar. Removing `EchoRedirect` makes `echo hello > note.md`, `echo hello >> note.md`, and quoted `>` ordinary text output. This is deliberate and must be documented and tested. Read pipelines remain functional.

Retain a small retired-command guard at the terminal submission boundary for `sync auth set ...`, including case/whitespace variations accepted by the old input handling. Run it before command echo, history insertion, parsing, or logging. Return fixed text such as “Browser authoring is no longer supported.” It must not read, validate, store, or send the supplied token. This compatibility guard is the only retained reference to the obsolete credential input syntax.

### Wallet and permissions

Preserve the existing wallet adapter and its EIP-1193 events. Preserve the chrome wallet menu, address/ENS/network display, terminal `login`/`logout`/`id`, wallet-based prompt, restoration, and synthetic wallet/session files. `whoami` remains the site's profile command.

Remove `AccessPolicy`, `AdminStatus`, `ADMIN_ADDRESSES`, and their write checks. The write character in `ls -l` is always `-`. Preserve recipient-based read permission display and lock markers from access metadata. Current `cat` and reader paths do not enforce that metadata as confidentiality; this work must not silently add or weaken a claimed authentication boundary. Clarify misleading comments while preserving observed behavior.

Wallet data never participates in public cache identity. Switching accounts changes identity and advisory display immediately without restoring old drafts, fetching a token, or invalidating public listing data. Signature/attestation data and native signing remain independent of wallet login.

## Target module boundaries

Keep the four-crate dependency direction. Do not introduce a new crate for this work.

- `websh-core` owns canonical paths, domain contracts, manifest parsing/serialization, filesystem assembly/query/routing, read-oriented shell behavior, wallet value types, and attestation/mempool logic still used by readers or CLI.
- `websh-web::runtime` owns mount refresh coordination, the browser cache adapter, wallet integration, and browser preference storage.
- `websh-web::platform` owns HTTP request deadlines/cancellation and browser APIs.
- `websh-site` retains deployed identity, artifacts, bootstrap read configuration, and shell copy. Its administrator-policy module is removed.
- `websh-cli` retains host authoring and publishing. Its `mount init` stops emitting or accepting a writable option.

Keep the name `StorageBackend` to avoid a rename-only migration. Reduce it to `backend_type`, `scan`, `read_text`, `read_bytes`, and `public_read_url`. Preserve `Rc<dyn StorageBackend>` and local non-`Send` futures. Remove commit methods, commit DTOs, and commit-specific errors from the port. Keep applicable HTTP, path, manifest, and read errors, with descriptions suitable for public reads.

Cache access is a small browser-runtime capability, not an addition to `StorageBackend`. Proposed layout:

```text
websh-web/src/runtime/
  loader.rs               root loading and declaration validation
  mounts.rs               mount state and pure transition decisions
  mount_cache/
    mod.rs                descriptor, record, validation, cache interface
    idb.rs                browser database lifecycle and transactions
  github_backend/         public manifest and file GETs
  content_cache.rs        bounded in-memory text cache
  state.rs                environment and wallet-session persistence
  wallet.rs               retained wallet adapter
```

Use the runtime-local cache interface to inject a deterministic fake in orchestration tests. Do not move browser cache or IndexedDB abstractions into the core crate merely for test convenience.

### Application state

One content filesystem comes from root content plus accepted external scans. Components read it directly; there is no `ChangeSet` overlay or second cloned content view. A separate system projection may add `/.websh/state` from content, environment, and wallet values for terminal/runtime views.

Remove editor state, draft hydration state, commit heads, token presence, and commit services. Keep current directory, wallet, theme, terminal history, backend registry, mount states, content read caches, and request identities.

Retain assembly operations such as mounting, replacing/removing subtrees, and inserting synthetic runtime files. Rename `pending_text` to `inline_text` and related accessors: these hold synthetic file bodies as well as today's edits. Remove edit-only mutation/export helpers after checking their retained callers. Cache the original validated backend scan; never export a combined `GlobalFs` containing runtime files or other mounts into persistence.

## External mount cache contract

### Scope

The initial implementation supports the currently implemented GitHub manifest backend with allowed `self` and `https://raw.githubusercontent.com` gateways. Enum variants or the old note's mentions of IPFS/ENS do not constitute working external backends. The deployed application must continue working under an IPFS path gateway, but adding an IPFS/ENS backend is outside this plan.

Cache only external, accepted declarations obtained from the successfully loaded root. Never cache or restore root content, declaration discovery, `/.websh/state`, wallet data, or rejected duplicate/overlapping mount declarations. A root load failure cannot be repaired by an external cache entry.

The cached payload is a serialized manifest snapshot, including directories, bundle metadata, file metadata, and mempool extensions. Use `serialize_manifest_snapshot` and `parse_manifest_snapshot` to reuse the domain format and validation. `ScannedSubtree` need not become a browser-storage serialization contract.

Actual Markdown, HTML, text bodies, PDFs, and images continue to use their existing live read paths and bounded in-memory text cache. Cached metadata does not guarantee offline document reading or an atomic historical version of a file. A live file may have changed or disappeared since the listing was saved; report the normal content-read outcome without inventing a body from metadata.

### Cache identity

Build a canonical descriptor from the validated backend configuration, not from a label or `RuntimeMount::storage_id()`:

```text
descriptor_version
backend_kind
canonical_mount_root
repository_owner_and_name
branch_or_ref
normalized_content_prefix
resolved_manifest_url
resolved_content_base_url
```

Serialize these fields deterministically and hash their UTF-8 representation with SHA-256. The record key is `mount-v1:<hex digest>`. Persist the descriptor with the record and compare it exactly on read. A hash is a lookup aid, not evidence of authenticity.

Resolve `self` URLs against the actual document base used for content requests, including `/ipfs/<cid>/` or another deployment prefix. Do not fingerprint the literal string `self`. Do not lowercase case-sensitive refs or paths. Existing backend validation and URL construction define normalization. Labels, wallet address, author state, and the removed `writable` flag are excluded.

Changing repository, ref, mount root, content prefix, gateway, or effective URL produces a different key. Identical public source descriptors may be reused across deployments on the same origin. A removed declaration is never mounted from cached records alone. Do not delete all records absent from one tab's declarations; another deployment on the same origin may still use them.

### Record and limits

Create database `websh-cache`, version 1, with one object store `mount_snapshots`, key path `key`. A proposed record is:

```text
key: string
record_version: 1
descriptor: canonical descriptor
manifest_json: string
manifest_bytes: integer
request_started_at_ms: integer
observed_at_ms: integer
```

`observed_at_ms` is when a validated network response was accepted, not a Git commit time or proof of freshness. Do not store a remote HEAD. No last-access write is required when displaying a cached entry.

Initial policy constants are design defaults to validate against real manifests:

| Policy | Initial value |
| --- | --- |
| Cache restore deadline | 500 ms from dispatch, including open/read/validation |
| Other cache operation deadline | 2 seconds per attempt, including connection open and transaction |
| Manifest network deadline | 10 seconds through response-body completion |
| Maximum cached manifest payload | 2 MiB of UTF-8 JSON |
| Total cached manifest payload budget | 8 MiB per origin |
| Maximum records | 16 per origin |
| Maximum usable cache age | 30 days |
| Allowed future timestamp skew | 5 minutes |

The byte budget measures serialized payload, not browser-internal disk overhead. An oversized live manifest may still render; it is simply not cached. Validate numeric fields and actual UTF-8 length rather than trusting stored size values. Both timestamps must be finite non-negative safe integers, with `request_started_at_ms <= observed_at_ms`, and obey the future-skew policy. Reject malformed, expired, unsupported-version, mismatched-identity, or excessive-future-time records as cache misses. Browser storage is untrusted input; reuse manifest path, metadata, bundle, and route validation before installing a snapshot.

On successful persistence, evict expired/invalid records and then the oldest `observed_at_ms` records until both count and payload budgets are met. Use deterministic key ordering for ties. Comparison, replacement, and capacity pruning belong to one readwrite transaction. If cleanup is scheduled after a corrupt read, re-read and validate the current record inside the cleanup transaction so it cannot delete a valid replacement from another tab. Do not request persistent-storage permission for this reconstructible cache. Browser eviction or cleared site data is equivalent to first visit.

## Mount state and asynchronous lifecycle

### State representation

Availability and refresh state must be separate. A mount can serve a previously accepted snapshot while another request is running or has failed. Keep the attempt epoch and content revision on `MountEntry`, so request identity is independent of availability. Proposed shape, with final Rust names left to implementation:

```rust
enum MountLoadStatus {
    Loading,
    Available {
        total_files: usize,
        observed_at_ms: u64,
        origin: SnapshotOrigin, // Cache or Network
        refresh: RefreshState, // Idle, Running, or Failed(error)
    },
    Failed { error: String },
}
```

`Available` means a validated listing can be used. `Network` means observed through the configured HTTP source, which may itself be cached by a CDN. It does not claim the latest Git HEAD or verified attestation. An empty but valid manifest is an available listing with zero files.

### Initial load

1. Allocate a root request sequence when root loading starts. Load and validate the bundled root manifest and current declarations without awaiting IndexedDB.
2. Install only the newest root request's result, advancing the installed runtime generation at that point. Validate all external declarations and reserve accepted mount points before restoring any cached subtree.
3. For each accepted external mount, allocate a mount epoch and launch cache restore and live scan concurrently. The live request never waits for cache open/read.
4. A cache hit within the restore deadline may install a listing if no live success for this generation/epoch has been accepted. Mark it `Available` from `Cache`, with refresh running or already failed as appropriate.
5. A validated, successfully assembled live result wins over cached data, updates the view, and becomes `Available` from `Network`. Persist it asynchronously after acceptance; storage completion is not on the render path.
6. Live failure retains an available listing and records a refresh error. Without a listing, keep `Loading` while the bounded cache lookup remains pending. A timely cache hit installs `Available` from `Cache` with the recorded refresh failure; a miss or deadline then produces `Failed`.
7. Ignore cache completions after the restore deadline, live success, declaration replacement, or request invalidation. A later cache result cannot downgrade a network-loaded mount.

Use a small per-attempt coordinator to remember `cache_pending` and an early network failure until the cache deadline finishes. Do not add persisted lifecycle state or a global boot barrier.

### Explicit refresh and failures

Refreshing an already available external mount keeps its current tree, sets refresh to running, and starts a new live request. Do not read IndexedDB again just to replace an in-memory tree. On success replace it; on failure keep it and show retry. A failed mount with no tree may run the initial cache-plus-live path on retry. A root refresh similarly retains the installed runtime while a replacement loads; failed refresh retains that runtime and its last accepted declarations. An initial root failure still shows the root failure state.

Remove `allow_missing_manifest`: HTTP 404 is a load failure for both root and external manifests. The previous adapter interpreted a missing external manifest as an empty mount. Retaining that behavior would erase a useful saved listing on HTTP 404. A genuine empty mount publishes a valid empty manifest. HTTP errors, invalid JSON, invalid paths, route collisions, and assembly failures must never overwrite the cache with an empty or partially applied snapshot.

Keep existing declaration validation and mount ownership rules. Assemble a candidate filesystem separately and publish filesystem plus status coherently only after all validation succeeds. Preserve reserved failed mount boundaries so reads do not fall through to a parent backend. Never install a cached snapshot for a rejected declaration.

### Request identity and body reads

Root requests carry a `root_request_sequence` allocated at dispatch; only the newest request can publish either success or failure. External results carry the installed runtime generation, mount root, mount epoch, and descriptor identity. A successful root replacement advances the installed runtime generation and invalidates the replaced runtime's external work. Starting a root refresh alone does not discard the currently installed runtime or its work. New mount refreshes invalidate older results for that mount. Both successes and failures obey the same guard.

Add a per-mount content revision to text-cache and in-flight read keys alongside runtime generation, mount root, and relative path. Advance it only when an accepted snapshot replaces the visible tree, then evict affected entries. Starting or failing a refresh preserves the installed revision, cached bodies, and rendered content; mount epochs order manifest requests independently. After publication, an old read may finish, but must not repopulate the current cache, remove a newer in-flight entry, or render into the current reader resource. Include the revision in reader resource invalidation, and handle stale read completions as cancellation/retry rather than a user-facing content failure. Runtime inline files remain synchronous and wallet-reactive.

The same rules apply to ledger prefetch, binary reader loads, and root reload. Drop stale binary results and revoke superseded object URLs. Use at most one automatic retry of an obsolete read against the current revision; continued refresh activity leaves the current reader resource responsible for its next request. Keep request cancellation best effort; identity checks provide correctness even when a fetch or IndexedDB request cannot be cancelled.

### Cache persistence ordering

Serialize and validate records before entering an IndexedDB transaction. A cache writer rechecks its generation/epoch/descriptor immediately before scheduling a transaction. Serialize writes per key within a tab and replace queued obsolete writes with the latest accepted result.

Across tabs, validate the existing record before comparing `request_started_at_ms` in the same readwrite transaction that writes the record. Treat an invalid or expired existing record as absent, so corrupt timestamps or timestamps beyond the allowed future-skew limit cannot prevent its replacement. An older-started request cannot replace a valid record from a newer-started request; use `observed_at_ms` as a tie-breaker, retaining the existing record on an exact tie. An already committed old result may remain temporarily if a newer refresh fails. Cached records are always revalidated on use; timestamps are best-effort ordering and age policy, not a server-version guarantee. If the wall clock moves backwards during a live request and creates an invalid timestamp pair, keep the live result but skip persisting that observation.

No BroadcastChannel, cross-tab locks, or replicated application state is required. Tabs refresh independently. A wallet switch cannot mutate cache identity or write authorization because the cache contains public reads only.

## IndexedDB lifecycle and legacy data

All cache operations return a bounded best-effort result: restore has a 500 ms deadline, and each other operation has a 2-second deadline including connection open and transaction. On timeout, abort any active transaction best effort, ignore late callbacks, and release the per-key writer queue so later accepted records can progress. A timeout is not persistence success and does not schedule an automatic retry. Unavailable storage, blocked opens, version errors, malformed records, failed transactions, and quota errors fall back to live reads. Do not show a site-level error, require users to clear site data, or retry indefinitely because the cache failed.

Create the store only during `onupgradeneeded`. Install a `versionchange` handler that closes the connection. If an open finishes after its deadline, close its resulting connection and ignore it. Do not delete a database automatically to recover a version or structural error. The cache is optional for the remainder of that page session after a structural/open failure.

Database request success alone does not mean the transaction committed. Await transaction completion before reporting persistence success; abort/error means no success. Perform network requests, hashing, serialization, and other unrelated awaits outside live transactions. These constraints follow the browser transaction lifecycle described in [MDN's IndexedDB guide](https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API/Using_IndexedDB). A blocked version upgrade is an ordinary compatibility condition; handle it without blocking the reader. [MDN blocked event](https://developer.mozilla.org/en-US/docs/Web/API/IDBOpenDBRequest/blocked_event)

On quota failure, leave the rendered listing intact, make one bounded best-effort cache eviction attempt, then retry the latest record once. Stop persistent writes for the page session if that fails. No quota assumptions are hard-coded as browser guarantees; storage is origin-scoped and subject to browser eviction. [MDN storage quotas and eviction](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria)

The old database is deliberately not migrated. Production code never opens, upgrades, hydrates, or deletes `websh-state`. Existing `draft_changes`, metadata, and any older `drafts` store remain recoverable through an explicit one-off maintenance procedure. Do not use the old `open_db()` for recovery because its upgrade code deletes the historical `drafts` store. The implementation plan records recovery/deletion as a release-preparation concern, not a new product feature.

At startup, remove the exact legacy sessionStorage key `websh.gh_token` best effort without reading its value. Preserve `websh.wallet_session`, `user.*` preferences, reader preferences, and game settings. Stop initializing the editor-only `EDITOR` default, but do not delete arbitrary user variables. Old tabs can still run old code; release guidance should recommend reloading them. No application feature can erase copies of a PAT held by an already-running old tab.

## Routing and presentation

Keep root loading/failure handling. Extend unresolved-route decisions to consider the most specific owning declared external mount, rather than checking root status alone. Resolve known cached routes immediately. Build route indexes from accepted cached snapshots using the same parser and resolver used for live data.

| Listing state | Existing route | Route absent from available snapshot |
| --- | --- | --- |
| No tree, loading | Pending mount view | Pending mount view |
| Cached or retained tree, refreshing | Render with a small saved-listing/updating indicator | Pending confirmation until refresh finishes |
| Cached or retained tree, refresh failed | Render with saved-listing age and retry | Explain that the route cannot be confirmed; offer retry and parent navigation |
| Successful live snapshot, idle | Render normally | Normal missing-route view |
| No tree, failed | Mount failure with retry | Mount failure with retry |

For paths outside a known external mount, ordinary root/content routing applies. Preserve canonical path checks, bundle aliases, locale selection, hash routing, and `/ipfs/<cid>/` bases. Do not use backend longest-prefix fallback to disguise a failed declared mount as successful parent content.

Use copy such as “Saved listing · checking for updates” and “Showing a saved listing · refresh failed.” Show age only where useful; keep database versions and storage internals out of the UI. A cache write failure after a successful live read requires no error banner. Existing attestation chips continue reporting their own evidence; cache state never creates a verified-signature claim.

## Configuration and compatibility

Remove `writable` from `RuntimeMount`, `BootstrapSiteSource`, and the owned mount declaration type, plus `mount init --writable`. Old JSON carrying the field remains accepted and ignored through serde's existing unknown-field tolerance. New emitted and hand-authored declarations omit it. Do not change the content metadata schema merely because this unrelated declaration field disappears.

Update the tracked mount declaration through normal source editing, then regenerate its sidecar, manifest, ledger, and attestations with their owning CLI workflows during implementation. Do not hand-edit generated artifacts. Native CLI credentials and GitHub write adapters remain in `websh-cli::infra`.

Mempool composition helpers used by `websh-cli` stay, even if their names include `ComposeForm` or `draft`. Browser-only placeholder/new-path helpers can be removed. Avoid moving retained native-authoring helpers solely to make a keyword search return zero.

## Completion contract

The browser has no reachable or dormant content-authoring subsystem, token getter, GitHub commit transport, `ChangeSet`, write policy, or writable mount capability. Wallet and native publishing behavior remain intact. Public reads and cached listings work without IndexedDB. Cache restore never blocks root content, overwrites a newer result, or promises offline file bodies. Existing drafts are not silently deleted or restored into the new runtime.

No implementation is claimed by this document. All remaining engineering work and verification requirements are in the [implementation plan](implementation-plan.md).
