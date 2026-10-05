# Repository tooling

## Bootstrap and ownership

`justfile` is the developer command map: `just --list` discovers tasks and
`just --show verify` shows the full gate. Just calls Node scripts directly; npm has
only the `setup` entry needed to bootstrap Just itself.

`npm run setup` installs locked npm dependencies, Playwright Chromium, missing pinned
native tools, and the WASM test runner matching `Cargo.lock`. Matching tools on `PATH`
are reused; other native binaries go under `target/tools/bin`. Install Rust through
rustup and provide the pinned Node, npm, and Binaryen prerequisites first. Compiler
pins live in `rust-toolchain.toml`, `.node-version`, and `package.json`; native tool pins
live in `scripts/tools.json`. Setup installs; verification checks without installing.
GPG is needed only for local signing, Pinata only for deployment.

| Location | Responsibility |
| --- | --- |
| `scripts/` | Tool setup, isolated build, artifact serving, lint, architecture checks, cleanup |
| `tests/` | Browser runners, E2E fixtures, tool checks, asset budgets |
| `docs/architecture/` | Maintained contracts and operational guidance |
| `.websh/local/` | Ignored private author data, deployment receipt, local archives |

Root tool configuration files stay at their native tool entry points. `CLAUDE.md`
forwards to `AGENTS.md` so agent instructions have one source.

## Builds and outputs

Trunk runs independent Stylance and `websh-cli sync` pre-build hooks. Same-stage hooks
may run concurrently: Stylance owns `assets/bundle.css`, and sync owns the
[content artifacts](cli.md#source-and-generation). Every build profile uses these hooks;
signing belongs to the explicit [publishing workflow](cli.md#publishing).

Trunk copies `content/`, public crypto artifacts under `assets/crypto/`, and original
theme sources under `assets/themes/` unchanged. The originals intentionally ship for
public verification against the attestation's recorded asset hashes; the application
loads Trunk's separately transformed CSS. These copies do not prove compiled
JavaScript/WASM provenance; the release build remains a trusted step.

Watch inputs include crates, Cargo/toolchain configuration, Trunk configuration, HTML,
headers, assets, content, and vendored dependencies. Generated CSS, manifest, and ledger
outputs are excluded from watches to avoid rebuilding on their own writes.

`just serve` runs Stylance once before starting Trunk so the ignored CSS bundle exists
even after `just clean`. Trunk validates ignored paths before running its build hooks;
the generated manifest and ledger already exist in a checkout and cleanup preserves them.
The recipe passes `--enable-cooldown` to suppress watch events during builds and the short
cooldown afterward. Trunk 0.21.14 accepts this setting only as a CLI flag, not a TOML key.
ACK commitments and retained attestations remain watched so explicit author commands
refresh the served site. For direct Trunk use, run
`stylance --output-file assets/bundle.css crates/websh-web` first, then
`trunk serve --locked --dist dist-dev --enable-cooldown` (unset `NO_COLOR` if it is `1`).

| Path | Owner |
| --- | --- |
| `dist/` | Normal release build |
| `dist-dev/`, `dist-<name>/` | Development and reserved named Trunk outputs |
| `target/verify/source/` | Isolated verification source snapshot |
| `target/verify/dist/` | Shared artifact for size and E2E checks |
| `target/verify-cargo/` | Separate compiler cache for the staged source tree |
| `target/debug/`, `target/release/`, `target/wasm32-unknown-unknown/` | Working-checkout Cargo caches |
| `target/vendor-tests/` | Vendored patch test outputs |
| `target/tools/` | Installed developer tools |
| `test-results/`, `playwright-report/` | Browser failure diagnostics |

`just build-check` stages source and generated assets, clears subjects only in the staged
attestation artifact, and builds an unsigned release there. The original content,
signatures, and lockfiles stay untouched. Its separate compiler cache prevents a staged
checkout from overwriting the working checkout's compiled outputs. No task swaps the
real `content/` directory or depends on the original Git directory.

## Cleanup and maintenance

`just clean` removes generated distributions, the verification stage, browser reports,
and generated CSS. `just clean-cache` also removes known compilation caches, including
`target/verify-cargo/`. Both accept `--dry-run` and preserve installed tools, authored
content, `.websh/local/`, and unknown directories. Stop builds and servers first.
Unqualified `cargo clean` removes the entire target directory, including installed tools;
there is no automatic cleaner or recursive cleanup of ignored files.

Dependency patches retain their provenance, licenses, and a removal condition;
see [dependency maintenance](verification.md#dependency-maintenance).

The app ships WOFF2 fonts and retains vendored asset licenses. Runtime budgets count all
shipped file types; content size is reported separately. Asset changes should preserve
math rendering and IPFS subpath navigation. Theme values belong in CSS palettes and
semantic tokens; component modules consume those tokens.

## References

- [Cargo cleanup semantics](https://doc.rust-lang.org/cargo/commands/cargo-clean.html)
- [Just settings](https://just.systems/man/en/settings.html)
- [Trunk configuration](https://github.com/trunk-rs/trunk/blob/main/Trunk.toml)
- [Pinned Trunk watch configuration](https://github.com/trunk-rs/trunk/blob/v0.21.14/src/config/rt/watch.rs)
- [KaTeX font options](https://katex.org/docs/font)
- [Web app manifest URL rules](https://www.w3.org/TR/appmanifest/#start_url-member)
