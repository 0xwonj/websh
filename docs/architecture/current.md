# Architecture

Websh is a static, read-only archive. The native CLI prepares and verifies published
content; the browser combines it with public external listings and a small runtime
projection. Editors and Git own source authoring and repository history.

## Crate boundaries

| Crate | Target | Owns | Must not own |
| --- | --- | --- | --- |
| `websh-core` | host + wasm | Domain contracts, filesystem, shell, runtime coordination, crypto, storage ports | Browser APIs, host processes, Leptos state |
| `websh-site` | host + wasm | Deployed identity, public keys, acknowledgement data, site policy | Generic engine behavior, command workflows |
| `websh-cli` | host | Argument adapters, native workflows, process/filesystem adapters | Browser state, generic domain rules |
| `websh-web` | wasm | Leptos app, browser runtime, wallet, storage, rendering, styles | Host processes, private core internals |

The CLI and web crates depend on core and site, site depends on core, and core depends
only on external libraries. CLI and web do not depend on each other.

Core implementation lives in private `engine/` modules. Its public facades are
`websh_core::domain`, `websh_core::filesystem`, `websh_core::runtime`,
`websh_core::shell`, `websh_core::mempool`, `websh_core::attestation`,
`websh_core::crypto`, `websh_core::ports`, and `websh_core::support`.
Contextual errors belong to the facade that owns the capability.

CLI `commands/` adapt Clap arguments to `workflows/`; `infra/` owns external effects.
Web `app/` composes runtime owners, `features/` implements views, `runtime/` owns
browser state, and `platform/` wraps browser APIs. Shared components and rendering
live in `shared/` and `render/`.

## Native contracts

`VirtualPath` is the canonical absolute filesystem path type. It rejects relative and
non-canonical input. `runtime_state_root()` and `is_runtime_overlay_path()` own the
reserved runtime namespace.

`NodeMetadata` separates structural `kind`, optional bundle structure, authored choices,
and computed facts:

| Section | Type | Owns |
| --- | --- | --- |
| `authored` | `AuthoredMetadata` | Title, description, date, tags, links, advisory access |
| `derived` | `DerivedMetadata` | Generated title, integrity, media properties, counts |

Only title falls back from authored to derived values. `NodeMetadata.kind` is the single
structural classification used by routes and views. Source inputs may supply a kind,
but it resolves into that field rather than a second authored or derived copy. Unknown
fields and values in the wrong section fail validation.

`GitHubMount` is the shared validated source configuration used by generation and browser
adapters. Deserialization validates the repository, ref, canonical prefix, gateway, and
public mount root before adapter construction. Public declarations occupy top-level roots
outside the reserved system namespace; only the site bootstrap owns `/`.

`StorageBackend` exposes `scan`, `read_text`, `read_bytes`, and `public_read_url` through
local, non-`Send` `Rc` handles. It grants no content-writing capability.

## Ownership and publication

The CLI's pure `ContentSnapshot` interprets and validates one authored tree, then projects
the manifest, ledger, and publication units. Generation, checking, signing, and upload
have explicit boundaries described in the [CLI guide](cli.md).

The browser's `filesystem::Snapshot` binds an immutable `GlobalFs` to its validated route
catalog. `Content` alone publishes replacement snapshots; `Wallet` and `Preferences` own
session state. Their small runtime overlay is composed with borrowed content through
`FsView` for shell navigation, listings, and runtime metadata. Ordinary content queries
do not subscribe to session state, and session changes never clone the content tree.
See [runtime](runtime.md) for request ordering, cache limits, and failure behavior.

Mounts, metadata, and manifests have one accepted representation. Owned data is migrated;
there are no compatibility readers. Cryptographic protocol labels, IndexedDB's structural
version, and dependency/tool pins retain versions where they carry meaning.

## Operations

- [Tooling](tooling.md): bootstrap, build/watch inputs, output ownership, cleanup.
- [Verification](verification.md): local gate, focused tests, dependency maintenance.
