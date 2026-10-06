# websh

Websh is a verifiable personal archive built with Rust, WASM, and Leptos. Its browser
filesystem combines published content and public external mounts, with reader, ledger,
and terminal views. Wallet connections provide identity; content authoring stays in
local files and Git.

## Setup

Install Rust through rustup, Node matching `.node-version`, npm matching `package.json`,
and Binaryen matching `scripts/tools.json` (`wasm-opt` on `PATH`). Then:

```bash
npm run setup
export PATH="$PWD/target/tools/bin:$PATH"
```

Setup installs the remaining pinned tools, locked QA dependencies, and Playwright
Chromium. See [tooling](docs/architecture/tooling.md) for environment and output ownership.

## Develop and verify

```bash
just serve    # http://127.0.0.1:8080, output in dist-dev/
just build    # release output in dist/
just verify   # full local gate
```

Use hash URLs such as `/#/websh`, `/#/ledger`, and `/#/writing/example`.
Builds refresh generated artifacts but never sign. `just --list` shows focused tasks.

## Publish

```bash
just publish
```

Publishing requires the site's GPG signing key and Pinata CLI. The recipe generates,
explicitly signs, builds, and uploads to public IPFS. Deployment alone reads `.env`;
ENS updates remain manual. See the [CLI guide](docs/architecture/cli.md) for local
content, draft, acknowledgement, offline-signing, and deployment workflows.

## Documentation

- [Architecture](docs/architecture/current.md): crate boundaries and native contracts.
- [Runtime](docs/architecture/runtime.md): browser state, routes, mounts, and cache limits.
- [Verification](docs/architecture/verification.md): test ownership and focused checks.
- [Publication design](docs/architecture/publication.md): planned IPFS app and GitHub
  content separation without ENS.

Source code and published content are licensed under [CC-BY-SA-4.0](LICENSE).
