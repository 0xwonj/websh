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
GPG is needed for owner signing and synthetic browser-fixture signing; Pinata is needed
only for app deployment. Verification never uses an owner secret key.

| Location | Responsibility |
| --- | --- |
| `scripts/` | Tool setup, isolated build, artifact serving, lint, architecture checks, cleanup |
| `tests/` | Browser runners, E2E fixtures, tool checks, asset budgets |
| `docs/architecture/` | Maintained contracts and operational guidance |
| `.websh/local/` | Ignored private author data, deployment receipt, local archives |

Root tool configuration files stay at their native tool entry points. `CLAUDE.md`
forwards to `AGENTS.md` so agent instructions have one source.

## Builds and outputs

Trunk runs one Stylance pre-build hook, which owns `assets/bundle.css`. App builds
never generate, sign, watch, or copy authored content. Content workflows operate on
an independent checkout through the [CLI](cli.md#content-publication).

Trunk copies the pinned app trust certificate, fonts, theme assets, and vendored
renderer resources. It transforms CSS separately. These outputs are app assets, not
signed content evidence. Watch inputs include crates, Cargo/toolchain and Trunk
configuration, HTML, headers, assets, and vendored dependencies. Generated CSS is
excluded to prevent its own writes from causing rebuild loops.

`just serve` runs Stylance before Trunk so the ignored CSS bundle exists after cleanup.
It passes `--enable-cooldown`, supported as a CLI flag by pinned Trunk 0.21.14. Direct
Trunk use should first run `stylance --output-file assets/bundle.css crates/websh-web`,
then `trunk serve --locked --dist dist-dev --enable-cooldown` (unset `NO_COLOR` if `1`).

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

`just build-check` stages app source and public app assets without content or deployment
`.env`. Only the staged certificate/fingerprint is replaced with the explicit public
fixture identity from `tests/fixtures/pgp/`; production trust is untouched and no runtime
verification bypass exists. Browser fixtures sign native manifests with the deliberately
public test key in `target/verify/gnupg`. No owner secret is required. The separate compiler
cache prevents the staged checkout from overwriting working-checkout outputs.

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
shipped app file types and reject a bundled `content/` directory. Asset changes should preserve
math rendering and IPFS subpath navigation. Theme values belong in CSS palettes and
semantic tokens; component modules consume those tokens.

## References

- [Cargo cleanup semantics](https://doc.rust-lang.org/cargo/commands/cargo-clean.html)
- [Just settings](https://just.systems/man/en/settings.html)
- [Trunk configuration](https://github.com/trunk-rs/trunk/blob/main/Trunk.toml)
- [Pinned Trunk watch configuration](https://github.com/trunk-rs/trunk/blob/v0.21.14/src/config/rt/watch.rs)
- [KaTeX font options](https://katex.org/docs/font)
- [Web app manifest URL rules](https://www.w3.org/TR/appmanifest/#start_url-member)
