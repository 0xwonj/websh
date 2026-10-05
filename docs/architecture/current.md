# Current Architecture

This is the authoritative architecture document for the current repository.

## System Shape

Websh is a verifiable personal archive backed by a Rust/WASM runtime and a browser-native virtual filesystem. It assembles content manifests and runtime mounts into one canonical tree rooted at `/`, renders that tree through Leptos, and offers reader, ledger, and terminal views for read-only navigation. Content authoring and publishing remain native CLI/Git workflows.

The workspace has four crates:

| Crate | Target | Owns | Must not own |
| --- | --- | --- | --- |
| `websh-core` | host + wasm | Shared domain contracts, filesystem, shell, runtime coordination, crypto, storage ports | Browser APIs, host processes, Leptos state |
| `websh-site` | host + wasm | Deployed identity, public keys, acknowledgement artifacts, site policy | Generic engine behavior, command workflows |
| `websh-cli` | host | Argument adapters, native workflows, process/filesystem adapters | Browser state, generic domain rules |
| `websh-web` | wasm | Leptos app, browser runtime, wallet, storage, rendering, styles | Host processes, private core internals |

The dependency direction is:

```text
websh-cli  -> websh-core, websh-site
websh-web  -> websh-core, websh-site
websh-site -> websh-core
websh-core -> external libraries only
```

`websh-cli` and `websh-web` must not depend on each other.

## Core Public API

`websh-core::engine` is private. External crates use the public facades:

- `websh_core::domain`
- `websh_core::filesystem`
- `websh_core::runtime`
- `websh_core::shell`
- `websh_core::mempool`
- `websh_core::attestation`
- `websh_core::crypto`
- `websh_core::ports`
- `websh_core::support`

New shared behavior should enter through one of these facades. Contextual error
types are exported from the facade that owns the capability; there is no global
`websh_core::errors` facade. Do not add new cross-crate consumers of `engine`.

## Boundaries

`websh-core` owns pure contracts and cross-target behavior. It should not reach into browser APIs, process APIs, or Leptos signals.

`websh-cli` owns host processes and filesystems. Clap command modules should stay thin and delegate use-case logic into `workflows`; process adapters live in `infra`.

`websh-web` owns browser state, IndexedDB, local storage, wallet APIs, DOM APIs, object URLs, fetch cancellation, and Leptos component state. Feature modules should call `AppContext` owners and `RuntimeServices` instead of reading browser storage directly.

`websh-site` owns stable deployed identity: public key material, expected fingerprints, acknowledgement artifacts, site copy/policy, and content fixtures that are specific to this deployment.

## Path Model

All engine paths are `VirtualPath` values. They are canonical absolute paths and reject relative or non-canonical input at construction and deserialization.

Runtime overlay paths are centralized through `runtime_state_root()` and `is_runtime_overlay_path()`. Synthetic environment and wallet/session files use that namespace; refresh rejects runtime-overlay paths.

## URL Model

The deployed browser app is a static, hash-routed application. The canonical root URL is `/#/`; internal routes use the same model, for example `/#/ledger`, `/#/websh`, and `/#/writing/example`.

Generated in-app links are hash-only (`#/ledger`, `#/writing/example`) so they preserve the current document base under path-gateway deployments such as `/ipfs/<cid>/`. Direct external links may still include the leading `/` on root hosts, but clean deep paths such as `/writing/example` are best-effort only and require the host to serve `index.html` for unknown paths.

## Runtime Model

The web app loads the root manifest and current declarations, assembles a validated `filesystem::Snapshot` containing an immutable `GlobalFs` and its route catalog, then starts concurrent cache restore and public refresh for accepted external mounts. A separate system projection adds wallet, session, and environment files. There is no browser editor, write overlay, draft persistence, GitHub credential flow, or commit protocol.

`StorageBackend` exposes only `scan`, `read_text`, `read_bytes`, and `public_read_url`. It remains a local non-`Send` port shared through `Rc`.

Optional IndexedDB database `websh-cache` stores bounded external manifest listings. Listing cache does not provide offline document bodies or offline root startup. Root sequences, mount epochs, and content revisions prevent stale publication while preserving available content during failed refreshes. See [runtime architecture](runtime.md) for identities, limits, and failure semantics.

## Authoring Boundary

Use local source files and Git to edit, commit, and publish. The native CLI generates
and checks artifacts, explicitly signs, imports local drafts, and uploads a checked
release bundle. `mempool sync CHECKOUT` prepares an external checkout's manifest;
`mempool import FILE` copies a validated draft into canonical source without touching
Git or deleting the original. Mount declarations are authored local JSON.

Browser wallet connection supports identity and advisory read display. It grants no
write capability. Terminal `refresh [path]` reloads the owning mount; `>` and `>>` in
`echo` are literal text.

## Build And Attestation

Markdown frontmatter owns Markdown metadata. Binary metadata lives in
`file.ext.meta.json`; `_index.dir.json` declares authored directory/bundle properties.
These inputs contain no computed hashes, sizes, counts, or Git timestamps.

A pure `ContentSnapshot` reads and validates the authored tree once, then projects the
manifest, ledger, and publication units. `sync` computes all current content and
attestation artifacts before writing them. It preserves identical files and matching
signatures. Missing or stale signatures are visible as pending; generation never signs.
`check` recomputes the expected snapshot and verifies current artifacts without writes.

Trunk runs two independent pre-build hooks: Stylance generates the CSS bundle and
`websh-cli sync` owns content generation. Every build profile uses this same pipeline;
only the publishing recipe invokes `attest sign` explicitly.

`attest message ROUTE` exports the exact plaintext signing request. Import requires that
message and a detached signature, verifies their binding to current subject content,
and then stores the attestation. Strict release verification requires a valid signature
from the deployed site's PGP identity on every subject. Ethereum signatures can supplement
that identity, but cannot replace it.

Generated artifacts are `content/manifest.json`, `content/.websh/ledger.json`,
`assets/crypto/ack.commitment.json`, and `assets/crypto/attestations.json`.
`deploy` validates the fixed prebuilt `dist/` against current project content and
attestations before uploading. This proves artifact consistency, not the provenance
of compiled JavaScript or WASM.

## Verification

The local gate is `just verify`; its executable definition lives only in `justfile`.
See [verification](verification.md) for test ownership and focused checks, and
[tooling](tooling.md) for build, environment, and cleanup ownership.

## Current Model

Mount declarations, manifests, and metadata accept only the current shape. Metadata has
no decorative schema counter. Owned content is regenerated through the CLI; old formats
are rejected rather than interpreted through compatibility readers. Cryptographic v1
labels remain part of the signed protocol. IndexedDB retains its required structural
version 1. Dependency and tool versions are pinned for reproducible builds.

See [migration operations](../migrations/README.md) for the external mempool patch and
one-time browser preference maintenance. Completed refactor reports live in Git history.
