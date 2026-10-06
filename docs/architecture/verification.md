# Verification

## Local gate

Run `just verify` for changes across layers. Its executable definition lives only in
`justfile`; `just --show verify` shows the sequence. It checks prerequisites, formatting,
dependencies, native and WASM code, tests, CSS, architecture,
a release build, asset budgets, and browser flows. [Tooling](tooling.md) owns bootstrap
and build isolation; checks never install tools or load deployment credentials.

The app gate has no dependency on today's content checkout. Content CI runs native
`websh-cli --root <content-checkout> check --require-signatures`; it checks generated
artifacts and public evidence without GPG signing, deployment credentials, or an app
build. Run content `sync`/`sign` deliberately before publication. Signature and deployment
policy belongs to the [CLI guide](cli.md#signing-and-checking).

Tests use a deliberately public synthetic PGP identity under `tests/fixtures/pgp/`.
GPG signs browser fixtures in an isolated fixture keyring; the real owner key is never
used. CLI publication tests use local bare Git remotes, cover rejected pushes and retry,
and do not upload to IPFS or contact production repositories.

## Test ownership

| Layer | Location | Responsibility |
| --- | --- | --- |
| Core | Tests next to owning modules | Current contracts, algorithms, paths, routing, crypto vectors |
| CLI | `crates/websh-cli/tests/cli/` | Snapshot generation/checks, signing requests, draft import, ACK receipts, publication boundaries |
| Browser WASM | Web module tests; runner in `tests/wasm/` | Browser adapters, cache transactions, async ordering, renderer safety |
| Browser E2E | `tests/e2e/` | User flows through a built app |
| Repository tools | `tests/tools/` | Result validation, architecture inspection, maintenance operations |

Test observable behavior at its owning layer. Remove obsolete and duplicate cases;
retain representative invalid-input boundaries and distinct event-ordering races. Use
small input tables and scoped fixtures, not getter/derived-trait tests or a generic test
framework. Test count is not a coverage target.

CLI fixtures use self-cleaning temporary directories and child processes for environment
isolation. E2E fixtures own their response maps, fail on unexpected browser errors, and
retain traces/screenshots on failure. Browser WASM mode is configured at the crate root.

## Focused checks

| Change | Check |
| --- | --- |
| Core/domain/runtime | `cargo test --locked -p websh-core [filter]` |
| Native workflows | `cargo test --locked -p websh-cli --test cli [filter]` |
| Browser code | `cargo check --locked -p websh-web --target wasm32-unknown-unknown`; `just test-wasm [filter]` |
| Browser flows | `just build-check`, then `just e2e [filter]` |
| Repository scripts | `just test-tools` |
| CSS | `just lint-css` |
| Architecture docs | `just docs-check` |
| Built assets | `just size [dist]` |

Native checks can miss WASM-only imports. For Rust changes also run the applicable native
or WASM Clippy scope with `--all-targets -- -D warnings`; the full recipe defines both.

## Browser, CSS, and asset checks

`just e2e` serves `target/verify/dist` at `127.0.0.1:4173` unless `WEBSH_E2E_BASE_URL` is
set. Build it first. External content and wallet traffic use local fixtures. Smoke tests
fail on same-origin asset 404s and cover root-host and `/ipfs/<cid>/` hash navigation.
WASM tests use Playwright Chromium and the exact locked wasm-bindgen runner.

`just lint-css` checks syntax and component token policy. `.stylelintrc.json` owns the
allowed units, color restrictions, and token families for stacking, font weights, radii,
and durations. Primitive values belong in tokens/palettes, not component CSS.

`just size` defaults to the verification distribution. It measures every runtime file
and computes Brotli subtotals by asset type. A bundled `content/` directory is an
error; authored content is delivered independently. Pass another built directory explicitly when needed.
Budgets change only for an explained product or packaging change.

## Dependency maintenance

`cargo deny` and `cargo machete` enforce Rust dependency policy. The vendored `syn_derive`
patch replaces an unmaintained diagnostic dependency with `syn::Error`; its license,
delta, and diagnostic tests remain under `vendor/syn_derive`. Remove the patch when a
supported upstream dependency graph no longer needs `proc-macro-error2`. Current
Leptos macro dependencies require `rstml ^0.12`; the maintained path in `rstml 0.13.1`
does not satisfy that constraint.

[deny.toml](../../deny.toml) records two exceptions: `RUSTSEC-2023-0071` for the RSA
dependency retained by rPGP, and `RUSTSEC-2024-0436` for upstream
`paste`. The accepted native/browser signature policy is Ed25519 only; owner signing runs
through GPG, not the Rust RSA dependency. Remove the RSA
exception when PGP adopts a fixed release, and the `paste` exception when every
Leptos/alloy dependency path removes or replaces it. A passing gate does not mean
the dependency graph has no advisories.

The npm audit on 2026-10-05 reported
[braces GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm) through
seven high-severity development-package entries after updating Stylelint to 17.16.0
and its standard configuration to 40.0.0. The latest matcher chain still uses
`braces 3.0.3`, with no patched release; npm proposed an unsuitable old Stylelint downgrade.
The CSS gate supplies source text and explicit filenames, with fixed configuration
patterns. These packages are not shipped, and no advisory is suppressed. A maintained
upstream fix remains follow-up work.
