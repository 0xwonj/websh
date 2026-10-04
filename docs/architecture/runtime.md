# Runtime Architecture

## Browser boot and ownership

`AppContext` composes independent `Content`, `Wallet`, and `Preferences` owners plus
terminal and navigation state. `Content` alone publishes content snapshots, mount status,
backends, and read-cache identities. Views receive read-only signals; they do not edit
installed trees or mount entries. `RuntimeServices` coordinates boot/refresh and theme
commands; it does not duplicate wallet or persistence state.

A core `filesystem::Snapshot` validates its route catalog once at construction and exposes
only borrowed reads. The browser holds it through `Rc`; ordinary reads resolve a backend
and relative path once, and routing reuses the stored catalog. Tree cloning occurs while
assembling a replacement, not for every document read or route lookup.

Root loading reads the live bundled manifest and its declarations, reserves accepted external mount roots, and installs the complete candidate runtime. External cache restore and public manifest requests then run concurrently. Root content and declaration discovery are never restored from IndexedDB.

Components borrow the content tree. A memoized system projection is rebuilt only when content, wallet, or preferences change:

```text
content GlobalFs + environment + wallet/session = system GlobalFs
```

`/.websh/state` exposes environment and wallet/session files. Browser content has no edit overlay, draft state, commit heads, credential state, or writable capability. Access metadata remains an advisory display filter, not confidentiality.

## Request and content identities

Root request sequences are allocated when a request starts. Only the latest request can install a new runtime or report root failure. An installed generation changes on accepted root replacement. A failed root refresh retains the existing runtime and external mounts.

Each mount has a request `epoch` and a `content_revision`. Refresh changes the epoch while retaining an available snapshot. A validated candidate is assembled separately, checked for routing conflicts, then published atomically. Only publication advances the content revision. Failed refreshes preserve listing data and loaded bodies.

A mount is `Loading`, `Available`, or `Failed`. Availability includes cache/network origin, observation time, and an independent refresh state (`Idle`, `Running`, or `Failed`). Missing routes stay pending while a listing is cached or refreshing; only a confirmed live listing can establish 404. Failed refresh leaves an unconfirmed missing route with a retry action.

Text cache/inflight keys include generation, mount root, content revision, and relative path. Text and binary reads verify their identity after awaiting, retry once if obsolete, and then return cancellation if invalidated again. A completed old request cannot remove a newer inflight request. The reader subscribes to content identity rather than refresh progress and drops stale document results, including owned object URLs. A rejected mount boundary never falls through to an ancestor backend.

## External listing cache

`runtime::mount_cache` owns a browser-local interface and an IndexedDB adapter. Database `websh-cache` v1 contains only `mount_snapshots`, keyed by `key`. Payloads use the existing manifest codec, including directory, bundle, file, and mempool metadata. Document bodies are fetched live; the cache does not provide full offline reading.

Descriptors include canonical mount root, repository, ref, normalized prefix, and resolved manifest/content base URLs. Deterministic serialization plus SHA-256 produces `mount:<digest>`. Stored descriptors must match exactly. `self` resolves against the document base, preserving deployment and IPFS prefixes. Wallet and labels do not affect cache identity.

| Limit | Value |
| --- | --- |
| Restore deadline, including open/read/validation | 500 ms |
| Other cache operation attempt | 2 seconds |
| Manifest HTTP deadline, including complete response body | 10 seconds |
| Serialized manifest per record | 2 MiB |
| Total manifest payload / record count | 8 MiB / 16 |
| Maximum usable age / future clock skew | 30 days / 5 minutes |

Validate the exact current record shape, identity, safe ordered timestamps, actual UTF-8 size, paths, metadata, bundles, and routes before publication. Oversized live results can render without persistence. Backwards wall-clock movement prevents persistence of that observation.

Live success always wins over cache. Early network failure waits for the bounded cache answer. A timely cache hit can remain available with refresh failure; a late cache result has no effect. Refreshing an available snapshot does not reopen the cache.

Writes are serialized/coalesced per key and recheck request validity before the transaction. Comparison, replacement, invalid/expired cleanup, and capacity pruning share a readwrite transaction. Newer request-start time wins, then observation time; an exact tie preserves the existing record. This orders observations, not Git revisions. Cleanup validates the current record inside its transaction, protecting replacements from other tabs.

Blocked/denied storage, version/schema errors, deadlines, and quota failures fall back to network reading. Timed-out opens close if they later succeed; versionchange closes connections. Transactions must commit before a write is successful. Quota failure gets one bounded eviction/retry; repeated failure disables writes for the session. There is no permission prompt, polling, cache management screen, or cross-tab leader.

## Wallet and preferences

`Wallet` owns the live connection, provider listeners, request identity, and chain-event
revision. Account changes and disconnects invalidate earlier asynchronous work; late
connect, chain, or ENS results cannot revive a disconnected account or overwrite a newer
one. Disconnect updates live state even if saving the session preference fails. Provider
events cannot reconnect a user who explicitly disconnected. Listener handles are removed
when their owner is disposed.

`Preferences` owns the environment/session snapshot. Its storage adapter handles the
wallet-session flag, `user.*`, `websh.reader.scale`, and `websh.dino.score`. The visual theme
is derived from the environment's canonical `THEME` value. There are no startup migrations,
old-format readers, or theme aliases. Optional one-time maintenance lives outside the app
in [migration operations](../migrations/README.md).

The terminal parses the current command grammar before echo or history insertion.
Unsupported commands and syntax produce a fixed message without retaining their input.

## Platform adapters

- `platform::fetch`: HTTP requests, response-body deadlines, and abort.
- `platform::asset::BrowserAssetUrl`: object URL ownership/revocation.
- `platform::dom`: hash routing and focus.
- `platform::wallet`: EIP-1193 requests, deadlines, listener installation, and ENS reads.
- `runtime::wallet`: guarded connection lifecycle and read-only wallet state.
- `runtime::state`: local preferences and read-only environment/session state.
- `runtime::github_backend`: public manifest and file GETs, without authoring or credentials.

Native content/mempool authoring, signing, and deployment remain CLI workflows.
