# External Mount Stale Snapshot Design

Status: design note only. This is not implemented in the current performance work.

## Problem

External mounts are currently registered as `Loading` until their backend scan completes. While a scan is blocked by network latency or a transient GitHub/IPFS failure, the mount point exists but cannot serve the last known tree. This makes cold starts depend on every external mount being reachable.

The goal for a later implementation is to cache the last successful external mount scan locally and mount it as a read-only stale snapshot while the live backend revalidates.

## Storage Model

Upgrade the browser IndexedDB schema from v1 to v2 and add a `mount_snapshots` object store.

Suggested record shape:

```json
{
  "cache_key": "github:/db:sha256-...",
  "mount_root": "/db",
  "backend": "github",
  "declaration_fingerprint": "sha256-...",
  "snapshot": { "files": [], "directories": [] },
  "remote_head": "optional backend head",
  "captured_at": 1777891200000,
  "schema_version": 1
}
```

The primary key should be `cache_key`. Build it from the canonical mount root plus a stable declaration fingerprint. The fingerprint must include the fields that affect the resolved tree: backend kind, repo, branch, root/content prefix, gateway, and writable flag. Do not use user-facing label/name as a cache identity field unless it changes backend resolution.

## Runtime Status

Extend `MountLoadStatus` with a stale state:

```rust
Stale {
    total_files: usize,
    epoch: u64,
    revalidating: bool,
    error: Option<String>,
}
```

`MountEntry::effective_mount()` must treat `Stale` as readable but not writable. This preserves the current safety rule that only a fully loaded mount can restore declared writability.

## Boot Flow

When registering an external mount:

1. Build and validate the backend from the mount declaration.
2. Compute the declaration fingerprint and cache key.
3. Look up `mount_snapshots` before scheduling the network scan.
4. If a matching snapshot exists, reserve the mount point, mount the cached `ScannedSubtree`, and mark the entry `Stale { revalidating: true }`.
5. Always schedule a backend scan for revalidation.
6. If the scan succeeds, replace the subtree with the fresh snapshot, persist it under the same cache key, and mark `Loaded`.
7. If the scan fails and stale data was mounted, keep the stale subtree and attach the error to `Stale.error`.
8. If the scan fails and no stale data exists, keep the existing `Failed` behavior.

## Read-Only Behavior

Stale mounts can satisfy reads through the canonical filesystem tree, route index, and metadata views. Writes must remain disabled:

- Do not expose stale mounts as writable through `effective_mounts()`.
- Reject commits whose target mount is `Stale`, even if the original declaration was writable.
- Do not persist drafts against a stale mount as though they were safe to commit. Existing global drafts can remain, but commit actions must require a live `Loaded` mount.

## Commit Safety

The later implementation must not commit against a stale base. Required constraints:

- `commit_changes` must require `MountLoadStatus::Loaded`.
- A stale snapshot's `remote_head` is informational only and must not seed optimistic commit preconditions.
- Successful revalidation should hydrate the latest remote head before allowing commit.
- If revalidation reports a different tree, replace the stale subtree before enabling writes.
- If revalidation fails, keep the mount read-only until a later successful reload.

## Invalidation

Cache identity is declaration-based. A mount declaration change that affects backend resolution must produce a different cache key and therefore avoid reusing a stale snapshot. Superseded records can be garbage-collected opportunistically after a successful boot by deleting `mount_snapshots` records whose `mount_root` is no longer declared or whose fingerprint no longer matches the active declaration.

## Test Matrix

Required Rust and wasm/browser coverage for the implementation pass:

- IDB v1 to v2 upgrade creates `mount_snapshots` without losing `drafts` or `metadata`.
- Snapshot save/load round-trips `ScannedSubtree` with files, directories, metadata, and mempool extensions.
- Mount declaration fingerprint changes when repo, branch, root/content prefix, gateway, backend, mount root, or writable flag changes.
- Boot with cached snapshot and slow backend mounts stale data immediately and later transitions to `Loaded`.
- Boot with cached snapshot and failing backend keeps stale data read-only and exposes the revalidation error.
- Boot without cached snapshot and failing backend remains `Failed`.
- Stale mounts are excluded from writable effective mounts and from commit targets.
- Successful revalidation persists the fresh snapshot and evicts stale error state.
- Removed or changed declarations do not reuse stale records under an old fingerprint.
- Descendant mount exclusion still works when exporting and persisting snapshots.

## Open Questions

- Whether to cap the number or total byte size of cached snapshots per origin.
- Whether a manual `sync` command should show stale age and force revalidation.
- Whether route-index derivation should run against stale external snapshots before revalidation completes or wait for the fresh scan.
