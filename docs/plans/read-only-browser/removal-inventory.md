# Browser writing removal inventory

Status: implemented; see [verification evidence and release blockers](progress.md).

Baseline: `ce1d7a16cf1653baa80af71f90fcdf98178c755d`, reviewed 2026-10-04. Paths below are relative to the repository root. Symbols identify the relevant behavior even if line numbers move. The [design](design.md) owns target behavior and the [plan](implementation-plan.md) owns execution order.

This inventory retains the baseline paths and action wording as an audit trail; deleted paths are intentional. Final absence checks and retained native dependencies are recorded in [progress](progress.md).

## Browser entry points

| Current source | Action | Retained behavior |
| --- | --- | --- |
| `crates/websh-web/src/features/router.rs` | Remove `BuiltinRoute::NewCompose`, `new_compose_frame`, and the `/new` branch. | Normal route resolution, pending/failure/404 views, hash navigation. |
| `crates/websh-core/src/engine/filesystem/routing.rs` and facade exports | Remove `is_new_request_path` and special-route tests. | Canonical paths, content aliases, bundle and locale resolution. |
| `crates/websh-web/src/features/reader/mod.rs` | Remove `ReaderMode`, editor seed/route keys, PAT author mode, draft/dirty/saving/error state, edit/preview/cancel/save callbacks. | Read resource, document rendering, metadata, signatures, preferences. |
| `crates/websh-web/src/features/reader/keybindings.rs` | Delete. It implements edit, preview, and save interception. | Browser-native shortcuts; existing unrelated component shortcuts. |
| `crates/websh-web/src/features/reader/toolbar.rs` | Delete edit-mode toolbar. | `features/reader/actions.rs` text scaling and copy-link menu. |
| `crates/websh-web/src/features/reader/shell.rs` | Remove `ReaderEditBindings` and editor slots. | Viewer layout and window chrome. |
| `crates/websh-web/src/features/reader/views/markdown.rs` | Remove `MarkdownEditorView`. | Rendered Markdown. |
| `crates/websh-web/src/features/reader/document.rs` | Remove `raw_source` if its editing consumer is gone; add revision-aware read completion. | Source fetching necessary to render; all document formats. |
| `crates/websh-web/src/features/mempool/commit.rs` and `error.rs` | Delete `save_raw` and `MempoolSaveError`. | Mempool metadata reading and projection. |
| `crates/websh-web/src/features/mempool/component.rs` | Remove PAT form, key/submit controls, `author_mode` property, related handlers. | Rows, filters, counts, collapse, pending statuses, source links, refresh feedback. |
| `crates/websh-web/src/features/ledger/mod.rs` | Remove author-mode wiring. | Published ledger and external mempool sections. |
| `crates/websh-web/src/app/editor.rs` and `shared/components/editor/` | Delete app editor and shared modal. | Other shared components. |
| Reader, mempool, and editor CSS modules | Delete only selectors for removed controls and modes; regenerate Stylance output. | Reader actions, list styling, wallet menu, directory and media layouts. |

There must be no privileged author UI when a wallet is connected, including the formerly allowlisted address. Mempool `draft` and `review` labels stay: those describe remote publications.

## Shell and effects

| Current source | Action |
| --- | --- |
| `crates/websh-core/src/engine/shell/executor/write.rs` | Delete content mutation executors. |
| `crates/websh-core/src/engine/shell/executor/sync.rs` | Delete status, staged commit, PAT auth, and staged mount-selection logic. Reimplement refresh independently. |
| `crates/websh-core/src/engine/shell/model.rs` | Remove write command variants/parser branches, `EchoRedirect`, `SyncSubcommand`, token auth actions, editor effects, filesystem effects, commit effects, and write-only execution arguments. Add read refresh parsing/effect. |
| `crates/websh-core/src/engine/shell/executor/mod.rs` | Remove write dispatch and `require_write_access`/`can_write_path`. Retain canonical mount resolution needed by refresh. |
| `crates/websh-core/src/engine/shell/executor/read.rs` | Remove admin policy arguments and writable computation from `ls`. Retain wallet-based advisory read display. |
| `crates/websh-core/src/engine/shell/pipeline.rs` | Remove changes/commit-head arguments and obsolete write routing. Retain pipes, filters, environment expansion, read dispatch. |
| `crates/websh-core/src/engine/shell/autocomplete/mod.rs` | Remove write/sync/auth names and completions. Add `refresh` and read-path completion. |
| `crates/websh-web/src/features/terminal/actions.rs` | Remove apply/stage/discard/commit/editor/token handlers and dependencies. Keep wallet, navigation, theme, environment, history, and new refresh dispatch. |
| `crates/websh-core/src/engine/shell/access.rs` | Delete `AccessPolicy` and `AdminStatus`. |
| `crates/websh-web/src/runtime/system.rs` | Stop injecting site access policy into `ExecutionContext`. |
| `crates/websh-site/assets/text/help.txt` and site shell copy | Describe the retained command set and read-only refresh. Remove browser authoring instructions. |

Keep `Login`/`Logout` authentication effects. Remove only token variants from their group; avoid deleting the entire auth enum on the strength of its name. Remove unused `InvalidateRuntimeState` if the final caller audit confirms it has no retained producer.

Compatibility tests must cover literal `>`/`>>`, normal missing `/new`, a real content `/new`, and token-blind rejection of the obsolete credential command before command echo/history. The small rejection guard grants no authentication or write capability.

## Application state and persistence

| Current source | Remove or replace |
| --- | --- |
| `crates/websh-web/src/app/context.rs` | Remove `changes`, `drafts_hydrated`, `editor_open`, `remote_heads`; replace cloned content overlay with direct content state; keep synthetic system view. |
| `crates/websh-web/src/app/boot.rs` | Remove draft hydration, autosave effect, and `AppEditModal`. Preserve wallet initialization and UI startup. |
| `crates/websh-web/src/app/services.rs` | Remove draft hydration/scheduling, `commit_staged`, `commit_changes`, `record_commit_outcome`, HEAD hydration, token setters/getters. Rework loading/refresh/cache lifecycle. |
| `crates/websh-web/src/app/error.rs` | Remove commit-only errors/results. Keep read/refresh and theme errors. |
| `crates/websh-web/src/runtime/drafts.rs` | Delete debounce, serialization, and global draft persistence machinery. |
| `crates/websh-web/src/runtime/storage_state.rs` | Delete remote-head persistence and path-only storage IDs. |
| `crates/websh-web/src/runtime/idb.rs` | Remove from production. Replace with dedicated `runtime/mount_cache/idb.rs`; no old database migration code. |
| `crates/websh-web/src/runtime/state.rs` | Remove token memory, load/set/get/clear and presence projection. Retain environment and wallet session; add exact-key retired-token cleanup without value reads. |
| `crates/websh-web/src/config.rs` | Stop initializing `EDITOR=vim`. Keep wallet, theme, language, history, timeout, and UI defaults. |
| `crates/websh-core/src/engine/runtime/state.rs` | Remove `github_token_present`. Retain environment and wallet-session values. |
| `crates/websh-core/src/engine/runtime/mod.rs` | Remove ChangeSet overlay construction and draft summary/token-marker files. Retain runtime env/wallet/session projection. |

Preserve existing browser preferences: `websh.wallet_session`, `user.*` including `LANG` and `THEME`, reader settings, and dino storage. Existing values named `EDITOR` need not be destructively purged; the application simply stops supplying or interpreting an editor default.

## Shared core and storage transport

| Current source | Action and boundary |
| --- | --- |
| `crates/websh-core/src/domain/changes.rs` and `domain/mod.rs` | Delete change/staging types and exports. |
| `crates/websh-core/src/engine/filesystem/merge.rs` | Delete ChangeSet merge functions and tests. |
| `crates/websh-core/src/engine/runtime/commit/` | Delete commit preparation, delta, validation, submission, and tests. |
| `crates/websh-core/src/engine/filesystem/global_fs/export.rs` | Remove commit-only tree export after caller audit. Cache original scans, not assembled tree exports. |
| `crates/websh-core/src/engine/filesystem/global_fs/mutation.rs` | Remove editing-only update methods. Keep assembly/synthetic-file mutations with retained callers. |
| `crates/websh-core/src/engine/filesystem/global_fs/mod.rs` and `content.rs` | Rename `pending_text`/`read_pending_text` to inline-text terminology; retain synthetic file reads. |
| `crates/websh-core/src/ports/storage.rs` | Delete `CommitBase`, `CommitOutcome`, `CommitFileAddition`, `CommitDelta`, `CommitRequest`, `commit_base`, `commit`, `MissingToken`, and conflict handling. Preserve reads and local futures. |
| `crates/websh-core/src/ports/manifest.rs` | Keep parser and serializer for public manifests, CLI checks, and cached snapshot validation/round trips. |
| `crates/websh-core/src/ports/mock.rs` | Remove commit-recording implementation; use small read-only test doubles where needed. Remove `mock` feature only after checking all test consumers. |
| `crates/websh-web/src/runtime/github_backend/graphql.rs` | Delete GraphQL query/mutation DTOs and encoders. |
| `crates/websh-web/src/runtime/github_backend/client.rs` | Remove HEAD lookup, manifest-at-HEAD reads, commit mutation, GraphQL errors, SHA extraction, and `allow_missing_manifest`. Keep validated public GETs and suitable HTTP errors. |
| `crates/websh-web/src/runtime/github_backend/mod.rs` | Remove writable construction and policy plumbing; derive the cache descriptor from resolved read configuration. |
| `crates/websh-web/src/runtime/content_cache.rs` | Retain bounded memory caching; add mount content revision to keys. |

Keep sanitization, canonical path validation, backend URL validation, runtime-overlay ownership, manifest validation, tree assembly, and applicable cryptographic helpers. Removing write capability does not justify removing defenses on mounted content.

## Wallet and site identity

| Current source | Decision |
| --- | --- |
| `crates/websh-web/src/runtime/wallet.rs` | Preserve provider calls, ENS, and account/chain listener cleanup. |
| `crates/websh-web/src/features/chrome/mod.rs` | Preserve wallet menu, address/ENS/network display, connect/disconnect. |
| `crates/websh-web/src/features/terminal/boot.rs` | Preserve wallet session restoration; remove any writer-only boot wording. |
| `crates/websh-core/src/domain/wallet.rs` and shell `executor/info.rs` | Preserve wallet types, `id`, prompt identity, and profile display. |
| `crates/websh-site/src/policy.rs` and policy exports | Delete administrator allowlist and access policy. |
| `crates/websh-site/src/identity.rs` and `artifacts.rs` | Preserve public identity, key/fingerprint data, and attestations. |
| `crates/websh-core/src/domain/filesystem.rs` | Remove mutable `DisplayPermissions.write`; formatter emits a fixed no-write marker. |
| `crates/websh-core/src/engine/filesystem/global_fs/query.rs` | Keep `get_permissions` recipient matching with entry + wallet; remove writable argument. |
| `crates/websh-core/src/domain/metadata.rs` | Keep access metadata; correct claims implying enforced privacy. |

Permission display and restriction markers are advisory. This project cleanup does not add encryption, recipient authentication, or private content fetching. Cached records never contain wallet-derived permission results.

## Mount declarations and native publishing

Remove writable capability fields from `domain/mount.rs` (`BootstrapSiteSource`, `RuntimeMount`) and `domain/site.rs` (`MountDeclaration`). Update site bootstrap, loader recovery paths, mount status helpers, tests, and constructors. Remove `RuntimeMount::storage_id()` when its commit persistence callers disappear.

In `crates/websh-cli/src/commands/mount/init.rs` and `workflows/mount/init.rs`, remove `--writable`, the option field, and emitted JSON field. Keep repository/bootstrap setup. Existing JSON with `writable` remains accepted and ignored; a missing field becomes valid. The removed CLI flag fails as an unknown argument rather than becoming a permanent no-op option.

Edit the owned source declaration `content/.websh/mounts/mempool.mount.json` to omit the field. Regenerate sidecars and other generated artifacts with owning commands during implementation. Do not force remote repositories to change before the new reader works.

Preserve the CLI's native content, attestation, deploy, mount, and mempool workflows. They use local files and typed `git`/`gh`/`gpg` adapters independently of the browser's ChangeSet/commit/PAT path.

Shared helper exceptions requiring explicit retention:

- `crates/websh-core/src/engine/mempool/form.rs`: `ComposeForm`, validation, and payload building are used by CLI `mempool add`.
- Mempool serialization, `slug_from_title`, category parsing, `build_mempool_manifest_state`, and frontmatter transformation remain required by native add/promote/remote operations.
- In `engine/mempool/path.rs`, retain `mempool_root`; remove browser-only `derive_new_path`, `placeholder_frontmatter`, and their error/placeholder types after the caller audit.
- Update `commands/mempool/add.rs` collision advice that currently says to edit in the browser, and `features/mempool/loader.rs` advice about re-committing through compose.

## Dependencies and generated files

Remove direct `base64` from `websh-web` when its GraphQL module is gone; retain core/CLI uses. Retain `idb` and `serde-wasm-bindgen` for the new cache, plus browser networking, timers, serialization, Markdown, HTML sanitization, wallet, clipboard, Blob, and object URL support. Audit actual surviving imports and wasm features rather than deleting packages by association. Native Ethereum attestation verification is unrelated to browser writing.

Remove old `commit_integration` test target and corresponding `mock` gate if no consumer remains. Regenerate lockfile/dependency metadata only through the owning tool. Generate `assets/bundle.css` through Stylance and generated content/attestation outputs through the CLI.

## Tests and documentation

Replace authoring scenarios at the end of `tests/e2e/websh.spec.js` with read-only/wallet/cache cases. Adjust the runtime-state directory test, which currently expects `drafts`. Preserve mempool fixtures, read/navigation/locale/rendering tests, and native `mempool_compose` coverage needed by CLI.

Remove commit-specific unit/integration tests together with their implementation. Preserve their independent path/manifest/mount invariants by placing relevant coverage on retained APIs. Keep the browser wasm harness in `tests/web-wasm-test.cjs`; it is needed for the new IndexedDB adapter and wallet/runtime tests.

README, AGENTS/CLAUDE instructions, current/runtime/verification architecture, justfile, shell help, and affected comments now describe the implementation. Existing crate and CLI architecture pages were checked against retained boundaries. The obsolete `errors` facade, ledger-path, and historical-doc references were corrected in the active documents.

The [acceptance matrix](implementation-plan.md#acceptance-matrix) includes structural absence checks as well as retained read and wallet behavior. A passing keyword search alone is insufficient: native authoring, publication draft labels, and the retired credential guard are valid remaining references.
