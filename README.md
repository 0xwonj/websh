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
Builds need no content checkout and never generate or sign content. `just --list` shows
focused tasks.

## Publish

```bash
just publish
```

App publishing requires Pinata CLI and deployment credentials. It builds and uploads the app
to IPFS; only the deployment adapter reads `.env`. Verify the returned CID gateway URL,
then update `wonjae.eth`'s ENS content hash to that app CID in the owner's wallet.

Content lives in [websh-content](https://github.com/0xwonj/websh-content). Publish an edit with:

```bash
cargo run --locked -p websh-cli -- --root ../websh-content publish
```

That command generates, signs with the local owner GPG key, commits, and pushes the content
snapshot. It does not rebuild or upload the app, or require an ENS update. Independent unsigned drafts remain in
[websh-mempool](https://github.com/0xwonj/websh-mempool). See the
[CLI guide](docs/architecture/cli.md) for authoring, publication, and portable proofs.

## Documentation

- [Architecture](docs/architecture/current.md): crate boundaries and native contracts.
- [Runtime](docs/architecture/runtime.md): browser state, routes, mounts, and cache limits.
- [Verification](docs/architecture/verification.md): test ownership and focused checks.
- [Publication](docs/architecture/publication.md): immutable snapshots and source trust.

Source code and published content are licensed under [CC-BY-SA-4.0](LICENSE).
