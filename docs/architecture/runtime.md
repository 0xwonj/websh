# Browser runtime

## Browser boot and ownership

`AppContext` composes independent `Content`, `Wallet`, and `Preferences` owners plus terminal
and navigation state. `Content` alone publishes content snapshots, mount status, backends, and
read-cache identities. Views receive read-only signals; they do not edit installed trees or
mount entries. `RuntimeServices` coordinates boot/refresh and theme commands; it does not
duplicate wallet or persistence state.

A core `filesystem::Snapshot` validates its route catalog once at construction and exposes only
borrowed reads. The browser holds it through `Rc`, and routing reuses the stored catalog.
`Content` owns backend selection and live content reads; core exposes the shared read errors.
Inline text belongs only to the browser's runtime overlay. Content replacement may assemble a
new tree; ordinary reads borrow the installed snapshot.

Root discovery reads GitHub `current.json`, then fetches the selected commit's manifest and
signature concurrently. Shared native/WASM verification authenticates exact manifest bytes,
validates the home projection and file catalog, and produces `VerifiedRelease`. Only then does
`Content` install the candidate. Profile, Now, ACK, routes, and mount declarations share that
publication boundary. No authored content is compiled into the app.

A verified root cache restores concurrently with discovery. Home rendering waits on neither
external mounts, body downloads, wallet setup, nor terminal animation. Immutable file reads
use commit URLs and verify the manifest's SHA-256 and byte length before use. Root declaration
signatures identify external sources; independently changing unsigned mounts do not inherit
root authentication.

Wallet and preference changes rebuild only the small `/.websh/state` overlay. Shell navigation,
directory listings, and reader metadata use `FsView`; ancestor listings merge by canonical path.
Path selection keeps ordinary content queries independent of runtime state. Runtime routes and
inline reads use the overlay directly. Neither rebuilding the overlay nor querying it clones
content or changes cache identity. Published manifests and authored sources reject the reserved
runtime subtree; the view also keeps it isolated from source entries.

Wallet connection grants no write capability. Access metadata is an advisory display filter, not
confidentiality.

## Routes and shell

The app uses hash routing: `/#/`, `/#/ledger`, `/#/websh`, and content routes such as
`/#/writing/example`. Generated links are hash-only, preserving the document base under
`/ipfs/<cid>/`. Clean deep paths require a host fallback to `index.html`.

The shell provides read/navigation commands, pipelines, wallet login/logout, and local
preferences; `help` lists its grammar. `refresh [path]` reloads the owning mount, and root
refresh reloads declarations too. Runtime-state paths cannot refresh remotely. `echo` treats `>`
and `>>` as literal text. Parsing happens before echo or history; unsupported input is rejected
without being retained.

## Request and content identities

Root request sequences are allocated when a request starts. Only the latest request can install
a new runtime or report root failure. An installed generation changes on accepted root
replacement. A failed root refresh retains the existing runtime and external mounts.

Each mount has a source generation, request `epoch`, and `content_revision`. Root replacement
preserves unchanged external declarations, loaded backends, and read identities. Changed or
removed declarations invalidate their earlier work. Identical manifest bytes retain content
identity and body caches. Refresh changes the epoch while
retaining an available snapshot. A validated candidate is assembled separately, checked for
routing conflicts, then published atomically. Only publication advances the content revision.
Failed refreshes preserve listing data and loaded bodies.

A mount is `Loading`, `Available`, or `Failed`. Availability includes cache/network origin,
observation time, and an independent refresh state (`Idle`, `Running`, or `Failed`). Missing
routes stay pending while a listing is cached or refreshing; only a confirmed live listing can
establish 404. Failed refresh leaves an unconfirmed missing route with a retry action.

Text cache/inflight keys include generation, mount root, content revision, and relative path.
Text and binary reads verify their identity after awaiting, retry once if obsolete, and then
return cancellation if invalidated again. A completed old request cannot remove a newer inflight
request. The reader subscribes to content identity rather than refresh progress and drops stale
document results, including owned object URLs. A rejected mount boundary never falls through to
an ancestor backend.

## Verified caches

`runtime::mount_cache` owns optional IndexedDB access. Database `websh-cache`, structural
version 2, stores exact source evidence in `mount_snapshots` and verified file bytes in a
bounded body store. The old disposable listing schema is discarded during upgrade; no old
record reader remains. Clearing storage does not affect publication or wallet authority.

Snapshot descriptors include canonical mount root, repository, ref, prefix, and the root's
pinned site/key/signer policy digest. Deterministic serialization and SHA-256 produce the
record key. External records have no owner trust key. Labels and wallet state are not source
identity. Cache records carry exact manifest/signature bytes and a full commit; deserialization
alone does not authenticate them. Root evidence is reverified and unsigned indexes are validated
before restoration. Cached bodies are rehashed against the current expected digest and length.

| Limit | Value |
| --- | --- |
| Snapshot restore, including open/read/validation | 500 ms |
| Other cache operation attempt | 2 seconds |
| HTTP deadline, headers and complete decoded body | 10 seconds |
| Live manifest / signature / pointer | 4 MiB / 16 KiB / 1 KiB |
| Cached snapshot payload per record | 4 MiB + 16 KiB |
| Snapshot payload total / count | 8 MiB / 16 |
| External snapshot age / future clock skew | 30 days / 5 minutes |
| Body memory / entry count | 16 MiB / 128 |
| Persistent bodies / entry count | 64 MiB / 128 |
| Largest persistable body | 8 MiB |

Root publication sequence and exact manifest digest define live ordering. Lower sequences and
same-sequence conflicts are rejected against both installed evidence and cached high-water
state. A root candidate's acceptance and persistence comparison run in one readwrite
transaction before live installation when storage is available. Root records are not aged out
or capacity-evicted like external listings. An unavailable cache permits network reading;
local ordering is not a proof of first-visit latestness and cannot survive cleared storage.
Separate tabs do not synchronize their visible pages automatically.

For unsigned external snapshots, newer request-start time wins, then observation time. This
orders observations rather than Git revisions. Cache restoration can fill an empty source but
cannot replace a successful live result. A failed refresh retains installed data and marks
freshness as unavailable. An unchanged commit avoids another metadata download, while the
certificate policy is rechecked at current time.

Verified body storage uses digest-and-length keys, so unchanged articles survive a Now-only
update. Concurrent reads share a request only when both immutable URL and expected integrity
match. Documents and media are loaded on demand; there is no full-PDF prefetch or offline app
service worker. PDFs and images receive owned Blob URLs, including relative resources found
in sanitized Markdown/HTML. Replacing or disposing a view revokes its URLs.

Blocked storage, schema errors, deadlines, and quota failures fall back to network reading.
Timed-out opens close if they later succeed; version changes close connections. Writes must
commit before success. There is no permission prompt, polling, or cache-management screen.

## Historical viewing

`?content=<full Git commit>&release=<manifest SHA-256>` before the hash route selects an exact
root snapshot. The app authenticates its manifest and checks the supplied digest, labels the
view historical, and neither updates the live head cache nor lowers its sequence watermark.
The signature footer provides a snapshot link. External mounts remain independently live and
are explicitly unsigned. Removing the query returns to live discovery.

## Wallet and preferences

`Wallet` owns the live connection, provider listeners, request identity, and chain-event
revision. Account changes and disconnects invalidate earlier asynchronous work; late connect,
chain, or ENS results cannot revive a disconnected account or overwrite a newer one. Disconnect
updates live state even if saving the session preference fails. Provider events cannot reconnect
a user who explicitly disconnected. Listener handles are removed when their owner is disposed.

`Preferences` owns the environment/session snapshot. Its storage adapter handles the
wallet-session flag, `user.*`, `websh.reader.scale`, and `websh.dino.score`. The visual theme is
derived from the environment's canonical `THEME` value. Preferences accept canonical values
only.

## Platform adapters

- `platform::fetch`: HTTP requests, response-body deadlines, and abort.
- `platform::asset::BrowserAssetUrl`: object URL ownership/revocation.
- `platform::dom`: hash routing and focus.
- `platform::wallet`: EIP-1193 requests, deadlines, listener installation, and ENS reads.
- `runtime::wallet`: guarded connection lifecycle and read-only wallet state.
- `runtime::state`: local preferences and read-only environment/session state.
- `runtime::github_backend`: fixed-commit GitHub reads and exact source evidence, without credentials.

Native authoring and publication are described in the [CLI guide](cli.md).
