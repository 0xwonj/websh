# Runtime Architecture

## Browser Boot

`websh-web::app::App` creates `AppContext`, installs browser persistence adapters, applies the initial theme, installs wallet listeners, hydrates drafts, and starts the terminal boot sequence.

`AppContext` owns signal handles for:

- base `GlobalFs`,
- current working directory,
- wallet state,
- theme,
- terminal history,
- staged `ChangeSet`,
- derived view filesystem,
- runtime mounts and remote heads,
- runtime state snapshot,
- editor modal state.

Feature components receive state through `AppContext` and `RuntimeServices`; they should not read browser storage directly.

## Filesystem View

The base filesystem comes from bundled content plus runtime mount scans. The displayed filesystem is derived from:

```text
base GlobalFs
+ staged ChangeSet
+ wallet state
+ runtime state overlay
= view GlobalFs
```

Runtime state is rendered under `/.websh/state` and is not writable from shell commands.

## Draft Persistence

Draft hydration must complete successfully before persistence starts. This prevents the initial empty `ChangeSet` from overwriting a stored draft when IndexedDB is unavailable or corrupt.

Draft storage uses IndexedDB:

- `draft_changes`: pathwise records keyed by `draft_id:path`
- `metadata`: path indexes keyed by `draft_paths:<draft_id>`

The persister keeps the last successfully persisted `ChangeSet` and writes only path deltas after debounce. Saves are serialized through one loop so older IndexedDB writes cannot race newer snapshots.

## Browser Platform Adapters

- `platform::fetch` owns browser fetch, timeout, and `AbortController` behavior.
- `platform::asset::BrowserAssetUrl` owns object URL revocation.
- `platform::dom` owns hash routing and focus helpers.
- `runtime::wallet` owns EIP-1193 event listeners and wallet calls.
- `runtime::state` owns local/session storage state projection.

## Commit Path

Browser commits use `RuntimeServices::commit_staged`:

1. Snapshot staged changes.
2. Resolve the strict backend for the mount root.
3. Read the expected remote head.
4. Call `websh_core::runtime::commit_backend`.
5. Persist the new remote head and evict content caches on success.

The browser never commits through longest-prefix backend fallback.
