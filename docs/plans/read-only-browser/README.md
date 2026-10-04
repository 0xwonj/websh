# Read only browser design and plan

Status: implemented; see [verification evidence and release blockers](progress.md).

The goal is to simplify Websh into a wallet-connected content reader and terminal explorer. Remove browser editing and everything required only to stage, save, or publish those edits. Preserve native authoring and publishing. Add a small, disposable cache for external mount listings.

Read these documents in order:

1. [Design](design.md): retained behavior, removed capabilities, target architecture, cache identity/schema, asynchronous lifecycle, failure behavior, and legacy-data policy.
2. [Removal inventory](removal-inventory.md): source-level changes across UI, shell, core contracts, storage, site policy, dependencies, tests, and native CLI boundaries.
3. [Implementation plan](implementation-plan.md): six ordered phases, exit criteria, behavioral acceptance matrix, legacy-data recovery, and verification commands.

## Main decisions

| Area | Decision |
| --- | --- |
| Wallet | Keep connection, restoration, account/chain events, identity display, and existing read-related behavior. Remove administrator/write privileges. |
| Browser authoring | Delete both direct reader/mempool editing and staged terminal editing, including drafts, token handling, commit APIs, and writable mount policy. |
| Navigation | Keep terminal exploration and read pipelines. Replace `sync` with `refresh [path]`; `/new` becomes ordinary content routing. |
| IndexedDB | Use a new `websh-cache` v1 database for validated external manifest listings only. File bodies, root content, credentials, and wallet state are outside this cache. |
| Existing data | Stop using `websh-state`; leave old drafts intact for explicit recovery. Remove only the exact retired token key automatically. |
| Refresh | Show saved listings while checking their source. Preserve usable listings and loaded bodies when refresh fails. Storage failure falls back to live reads. |
| Native workflow | Keep CLI content/mempool operations, Git/GitHub tooling, attestations, and deployment. |

The cache policy starts with a 500 ms restore deadline, a 10-second manifest-fetch deadline, 2-second other cache operations, 2 MiB per record, 8 MiB total, at most 16 records, and a 30-day maximum age. These enforced limits were exercised by browser storage tests. They are not browser disk-allocation guarantees.

This work does not promise complete offline startup or offline document reading. It does not introduce new external backends, a service worker, background polling, a cache-management screen, or a replacement browser authoring tool.

[Current architecture](../../architecture/current.md) describes the delivered behavior. The older [external mount snapshot note](../../performance/external-mount-stale-snapshot.md) is historical context and is superseded by this implementation.
