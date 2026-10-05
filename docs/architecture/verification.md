# Verification

## Local gate

Run `just verify` for changes across layers. Its executable definition lives only in
`justfile`; `just --show verify` shows the sequence. It checks prerequisites, formatting,
dependencies, native and WASM code, tests, CSS, architecture, generated artifacts,
a release build, asset budgets, and browser flows. [Tooling](tooling.md) owns bootstrap
and build isolation; checks never install tools or load deployment credentials.

The read-only CLI check precedes the isolated build, so staged regeneration cannot hide
stale committed artifacts. After changing authored input, run sync deliberately and
include generated changes in the same commit. Signature and deployment policy belongs
to the [CLI guide](cli.md#signing-and-checking). Tests use stubbed publication processes;
they never upload or sign with the owner's key.

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
One-time maintenance tests retire with their tools.

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
outside `content/`, reports content and full-deployment sizes separately, and computes
Brotli subtotals by asset type. Pass another built directory explicitly when needed.
Budgets change only for an explained product or packaging change.

## Dependency maintenance

`cargo deny` and `cargo machete` enforce Rust dependency policy. The vendored `syn_derive`
patch replaces an unmaintained diagnostic dependency with `syn::Error`; its license,
delta, and diagnostic tests remain under `vendor/syn_derive`. Remove the patch when a
published upstream release supplies the maintained path.

[deny.toml](../../deny.toml) records two exceptions: `RUSTSEC-2023-0071` for the RSA
dependency used by local PGP verification/import, and `RUSTSEC-2024-0436` for upstream
`paste`. A passing gate does not mean the dependency graph has no advisories.

The npm audit on 2026-10-05 reported
[braces GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm) through
seven high-severity development-package entries in Stylelint's chain. At that audit,
there was no patched release; npm proposed an unsuitable old Stylelint downgrade.
The CSS gate supplies source text and explicit filenames, with fixed configuration
patterns. These packages are not shipped, and no advisory is suppressed. A maintained
upstream fix remains follow-up work.
