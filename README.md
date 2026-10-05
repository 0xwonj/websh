# websh

```text
             _       _
 __ __ _____| |__ __| |_
 \ V  V / -_) '_ (_-< ' \
  \_/\_/\___|_.__/__/_||_|
```

Websh is a verifiable personal archive backed by a Rust/WASM runtime and a browser-native virtual filesystem. It builds a static Leptos/WASM application that loads content manifests and runtime mounts into one canonical tree rooted at `/`, then exposes that tree through reader, ledger, and terminal views.

## Workspace

The current workspace has four crates:

| Crate | Role |
|---|---|
| `websh-core` | Shared domain types, filesystem engine, shell engine, runtime coordination, mempool helpers, attestation primitives, storage ports, and public facades. |
| `websh-site` | Deployed-site identity and policy constants such as public key material, expected fingerprints, acknowledgement data, and site-specific copy. |
| `websh-cli` | Native content generation and checks, explicit signing, local draft import, acknowledgements, and publication. |
| `websh-web` | Leptos browser adapter, `AppContext`, runtime owners, IndexedDB/localStorage adapters, feature views, and browser platform APIs. |

Architecture docs live in [docs/architecture/current.md](docs/architecture/current.md).

## Prerequisites

Install Rust through rustup, Node matching `.node-version`, npm matching the
`packageManager` pin in `package.json`, and Binaryen matching `scripts/tools.json`
(`wasm-opt` on `PATH`). `rust-toolchain.toml` selects Rust and the WASM target.
Bootstrap the remaining tools with:

```bash
npm run setup
export PATH="$PWD/target/tools/bin:$PATH"
```

Setup installs locked npm dependencies, missing pinned native tools, the WASM test
runner matching `Cargo.lock`, and Playwright Chromium. It reuses matching tools
already on `PATH`; otherwise native binaries go under `target/tools/bin`.
After bootstrap, use `just --list` for developer tasks and `just setup` to refresh tools.
Verification checks prerequisites without installing anything.

Publishing uses GPG with the deployed site’s signing key and the Pinata CLI. Generation and verification require neither tool. Draft repositories use ordinary editors and Git; the app CLI does not require GitHub CLI.

## Development

```bash
just serve
```

The dev server listens on `http://127.0.0.1:8080` and writes to `dist-dev/`. Stylance generates CSS and `websh-cli sync` refreshes deterministic content artifacts. Development shares the normal Cargo cache. Builds never sign.

The browser app is hash-routed. The canonical root URL is `/#/`; content and app routes use the same hash model, for example `/#/ledger` and `/#/writing/example`. Clean deep paths such as `/writing/example` are best-effort only and require a host-level fallback to `index.html`; IPFS/path-gateway deployments should use hash URLs.

## Build

```bash
just build
```

Release builds write `dist/`. Every Trunk profile runs the same `sync` pipeline for manifest, ledger, acknowledgement, and attestation artifacts. Matching signatures are preserved; changed or new subjects remain pending until explicitly signed.

## Verification

```bash
just verify
```

Focused checks:

```bash
just deps-check
just test-wasm
just lint-css
just docs-check
just build-check
just size
just e2e
```

`just build-check` creates an unsigned release in `target/verify/dist` without
modifying source artifacts. Both `just size` and `just e2e` use that build.
`just clean` removes generated outputs; `just clean-cache` also removes compilation
caches while preserving installed tools and local author data. Both accept `--dry-run`.

See [verification](docs/architecture/verification.md) for test ownership and
[tooling](docs/architecture/tooling.md) for environment, output, and cleanup contracts.

## Content And Attestations

Content lives under `content/`. Markdown frontmatter and authored binary/directory
metadata are source. Hashes, counts, and media properties are computed into generated
artifacts; authored files are not rewritten during generation.

```bash
cargo run --locked -p websh-cli -- sync
cargo run --locked -p websh-cli -- check
cargo run --locked -p websh-cli -- attest sign
cargo run --locked -p websh-cli -- check --require-signatures
```

`sync` generates without signing. `check` verifies without writing. Strict verification
requires the site's PGP signature for every subject; Ethereum attestations are supplemental.
For offline signing, export `attest message ROUTE` to a plaintext file and import the
signature with that exact message. See the [CLI guide](docs/architecture/cli.md).

Draft workflows are local too:

```bash
cargo run --locked -p websh-cli -- mempool sync /path/to/websh-mempool
cargo run --locked -p websh-cli -- mempool import /path/to/websh-mempool/writing/example.md
```

Import creates canonical source without overwriting existing content, committing,
signing, or deleting the draft. Run sync, review the result, then use Git normally.

## Browser Shell

Common read commands:

- `ls [dir]`
- `cd <dir>`
- `pwd`
- `cat <file>`
- `help`, `whoami`, `id`, `theme`, `clear`, `echo`
- `grep`, `head`, `tail`, `wc` through pipelines
- `export` / `unset` for user environment variables
- `login` / `logout` for wallet session state

`refresh [path]` re-fetches the owning mount's listing; root refresh also reloads mount declarations. Available content remains visible if a refresh fails. The browser has no editor, save/compose controls, GitHub credential input, or content-writing commands. `echo` treats `>` and `>>` as literal text.

Author through local source files, Git, and native CLI workflows. Wallet connection, restoration, identity display, read pipelines, and preferences remain available.

External listings may start from the disposable `websh-cache` IndexedDB cache while refreshing. Only public manifest metadata is persisted; document bodies and root discovery require live reads. See [runtime architecture](docs/architecture/runtime.md) and [migration operations](docs/migrations/README.md).

## Deploy

```bash
just publish
```

The recipe runs sync, explicitly signs, builds `dist/`, and deploys. The CLI `deploy`
command only uploads an already-built bundle after checking current source/artifact
consistency and site signatures; it does not attest to compiled JavaScript/WASM provenance.
It uploads to public IPFS through Pinata, writes `.last-cid`, and prints the CID.
Deployment alone loads `.env` for its upload process. Updating ENS remains manual.

## Styling

CSS uses Stylance modules and a token hierarchy:

- `assets/tokens/primitive.css`
- `assets/tokens/semantic.css`
- `assets/tokens/breakpoints.css`
- `assets/tokens/typography.css`
- `assets/themes/*.css`
- `assets/base.css`
- `crates/websh-web/src/**/*.module.css`

Component CSS should use semantic tokens. `just lint-css` enforces the current token policy.

## License

See [LICENSE](LICENSE). Source code and published content are licensed under `CC-BY-SA-4.0`.
