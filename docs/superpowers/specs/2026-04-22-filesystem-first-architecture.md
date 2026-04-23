# Filesystem-First Architecture

Status: current
Updated: 2026-04-23

## Runtime authority

The runtime has one authority path:

1. `config::BOOTSTRAP_SITE`
2. `core::runtime::loader`
3. `GlobalFs`
4. `RuntimeMount`
5. runtime state store (`/state`)

`core::runtime::loader` is the only place that assembles runtime data. It:

- scans the bootstrap site
- seeds bootstrap shell and filesystem apps
- validates `/site/.websh/site.json`
- loads `/site/.websh/mounts/*.mount.json`
- scans and mounts declared backends
- applies sidecar metadata
- loads the optional derived route index from `/site/.websh/index.json`
- hydrates persisted remote heads

`components/terminal/boot.rs` only runs the boot animation, applies `RuntimeLoad`, and restores wallet session state.

## Canonical model

The canonical filesystem is one global tree rooted at `/`.

Expected roots:

- `/site` for the first-party bootstrap site
- `/mnt/<name>` for declared mounts
- `/state/*` for runtime-owned state

Shell/display aliases are presentation only. `~` maps to `/site`, but engine APIs operate on canonical absolute paths.

Public engine surface:

- `GlobalFs`
- `RouteRequest`
- `RouteResolution`
- `RouteFrame`
- `RenderIntent`
- `RuntimeMount`

Backend scan rows mount directly into `GlobalFs`. There is no retained mount-local filesystem model inside core.

## Storage contract

`StorageBackend` is backend-neutral:

- `scan() -> ScannedSubtree`
- `read_text()`
- `read_bytes()`
- `commit(CommitRequest) -> CommitOutcome { new_head, committed_paths }`

GitHub manifest JSON is a private serialization detail under `core::storage::github::manifest`.

`ScannedSubtree` is the only scan result shared across runtime/storage boundaries:

- `files: Vec<ScannedFile>`
- `directories: Vec<ScannedDirectory>`

Runtime commit coordination prepares a merged mount snapshot from `GlobalFs` before calling the backend. `CommitRequest` carries the runtime-supplied auth token, so storage backends do not read browser/session state directly. GitHub then serializes `manifest.json` privately from that prepared snapshot. The app surface does not expose manifest structs.
Runtime commit preparation also emits a backend-neutral `CommitDelta` with concrete file additions/deletions. Directory deletes are expanded before backend dispatch, and descendant staged writes under a deleted directory are suppressed so a repo path cannot appear in both additions and deletions.

## State model

`/state` is the rendered runtime state surface for:

- environment variables
- GitHub auth token presence marker
- wallet session marker
- wallet connection snapshot
- draft summary

`AppContext` owns:

- `global_fs`
- `view_global_fs`
- `cwd`
- `changes`
- `backends`
- `runtime_mounts`
- `remote_heads`
- `runtime_state`
- `wallet`

`runtime_state` is a safe projection hydrated from the runtime state adapter and owned by `AppContext` for rendering. Raw GitHub tokens stay in the private runtime secret store and are exposed only through the commit-auth path. Feature code does not read browser storage directly, and `/state` does not expose the raw GitHub token.

## Command/runtime rules

- write ownership is determined from `runtime_mounts`
- `ls`, `touch`, `mkdir`, `rm`, `rmdir`, `edit`, and `sync` work against canonical roots
- `sync commit` accepts staged changes for exactly one runtime mount
- `sync refresh` and successful `sync commit` reload runtime through `core::runtime::loader`
- auth/session mutations use one side effect path: runtime-state mutation returns a fresh safe snapshot for `AppContext.runtime_state`

## Route rules

- `/shell` is the default shell entrypoint
- `/fs/*path` is the canonical browse namespace
- `/` resolves through route metadata, derived index entries, or filesystem fallback
- deleting `/site/.websh/index.json` must not break route resolution because the index is optional
