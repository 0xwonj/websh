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
| `websh-cli` | Native command adapter for content generation, attestation, deployment, mempool workflows, and mount setup. |
| `websh-web` | Leptos browser adapter, `AppContext`, runtime services, IndexedDB/localStorage/sessionStorage adapters, feature views, and browser platform APIs. |

Architecture docs live in [docs/architecture/current.md](docs/architecture/current.md).

## Prerequisites

```bash
rustup target add wasm32-unknown-unknown
cargo install just
cargo install trunk
cargo install stylance-cli
cargo install --locked cargo-deny
cargo install cargo-machete
npm install
```

Playwright browsers are required for E2E tests:

```bash
npx playwright install
```

Release signing and deployment use local tools: `gpg` is optional for PGP signatures, the Pinata CLI is required for `just pin`, and `gh` is required for mount/bootstrap workflows that touch GitHub repositories.

## Development

```bash
just serve
```

The dev server listens on `http://127.0.0.1:8080` and writes dev artifacts to `dist-dev/`. The pre-build hook chain runs Stylance, `websh-cli content manifest`, and `websh-cli attest build`. Development Trunk profiles skip the full attestation build unless `--force` is passed to the CLI command directly.

The browser app is hash-routed. The canonical root URL is `/#/`; content and app routes use the same hash model, for example `/#/ledger` and `/#/writing/example`. Clean deep paths such as `/writing/example` are best-effort only and require a host-level fallback to `index.html`; IPFS/path-gateway deployments should use hash URLs.

## Build

```bash
just build
```

Release builds write `dist/`. The release Trunk profile refreshes content manifests, `content/.websh/ledger.json`, and `assets/crypto/attestations.json`. Set `WEBSH_NO_SIGN=1` to refresh pending subjects without invoking GPG signing.

## Verification

```bash
just verify
```

Focused checks:

```bash
just deps-check
just web-wasm-test
npm run lint:css
npm run docs:drift
npm run perf:budgets
npm run perf:content
npm run e2e
```

`npm run perf:budgets` expects a release `dist/` tree. `npm run perf:content` expects a running app at `http://127.0.0.1:4173` unless `WEBSH_PERF_BASE_URL` or `WEBSH_E2E_BASE_URL` is set.

See [docs/architecture/verification.md](docs/architecture/verification.md) for the maintained command list behind `just verify`.

## Content And Attestations

Content lives under `content/`. The manifest pipeline parses frontmatter, computes derived fields, keeps sidecars current, and writes `content/manifest.json`.

```bash
cargo run --bin websh-cli -- content manifest
```

The attestation pipeline refreshes sidecars, `content/.websh/ledger.json`, subjects, and `assets/crypto/attestations.json`. It signs missing PGP attestations when the expected signing key is available.

```bash
cargo run --bin websh-cli -- attest
cargo run --bin websh-cli -- attest build --force
WEBSH_NO_SIGN=1 cargo run --bin websh-cli -- attest build --force
```

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

Admin write commands stage local changes in IndexedDB:

- `touch <path>`
- `mkdir <path>`
- `rm [-r] <path>`
- `rmdir <path>`
- `edit <path>`
- `echo "body" > <path>`
- `sync status`
- `sync commit <message>`
- `sync refresh`
- `sync auth set <github_pat>` / `sync auth clear`

Commits go through the strict mount-root backend and use GitHub compare-and-swap with the expected remote head. If the remote moved, the commit fails instead of clobbering newer content.

## Deploy

```bash
just pin
```

The deploy command builds the release bundle, uploads `dist/` to Pinata, writes `.last-cid`, and prints an `ipfs://...` contenthash for ENS. It reads `.env` for child-process environment variables such as Pinata credentials.

## Styling

CSS uses Stylance modules and a token hierarchy:

- `assets/tokens/primitive.css`
- `assets/tokens/semantic.css`
- `assets/tokens/breakpoints.css`
- `assets/tokens/typography.css`
- `assets/themes/*.css`
- `assets/base.css`
- `crates/websh-web/src/**/*.module.css`

Component CSS should use semantic tokens. `npm run lint:css` enforces the current token policy.

## License

See [LICENSE](LICENSE). Source code and published content are licensed under `CC-BY-SA-4.0`.
