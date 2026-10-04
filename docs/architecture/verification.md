# Verification Contract

## Default Gate

Run:

```bash
just verify
```

Run `just setup` once to prepare the pinned tools and locked npm dependencies. Setup
and verification are separate; verification never installs missing tools automatically.
Rust is pinned in `rust-toolchain.toml`, Node in `.node-version`, native tools in
`scripts/tools.json`, and wasm-bindgen tooling follows `Cargo.lock` exactly.

The executable gate lives in `justfile`; use `just --show verify` to inspect it.
It checks tool versions, formatting, dependencies, native and WASM Clippy, native and
browser tests, repository tools, CSS, architecture, a release build, asset budgets,
and browser flows. Documentation describes these responsibilities without duplicating
the recipe.

`just build-check` stages source and generated assets under `target/verify/source`, clears
prior subjects only in the staged attestation artifact, and runs the normal generation
hooks with new signing disabled. This produces unsigned subjects regardless of existing
release signatures. It writes `target/verify/dist`; size checks and E2E use that same
artifact. The original tracked content, signatures, and lockfiles are not changed by
verification. Normal authoring/release `trunk build` still runs its hooks in the checkout
and can regenerate/sign assets.

## Focused Gates

Use these when the change is narrow:

- Core/domain/runtime: `cargo test -p websh-core` and `cargo clippy --workspace --all-targets -- -D warnings`
- CLI workflow: `cargo test -p websh-cli`
- Browser runtime or Leptos: `cargo check -p websh-web --target wasm32-unknown-unknown`, `cargo clippy -p websh-web --target wasm32-unknown-unknown --all-targets --all-features -- -D warnings`, and the relevant Playwright test
- CSS: `just lint-css`
- Docs: `just docs-check`
- Bundle budgets: `just size dist`

## Test Ownership

| Layer | Location | Responsibility |
| --- | --- | --- |
| Core unit tests | Next to the owning Rust module | Current contracts, algorithms, paths, routing, crypto vectors |
| Native workflows | `crates/websh-cli/tests/cli/` | Actual CLI processes, generated files, signing/verification |
| Browser WASM | `websh-web` module tests; runner in `tests/wasm/` | Browser adapters, cache transactions, async ordering, renderer safety |
| Browser E2E | `tests/e2e/` feature specs | User flows and integration through a built app |
| Repository tools | `tests/tools/` | Test-result validation, architecture inspection, maintenance operations |

Test current observable behavior at its owning layer. Delete tests for removed features
and historical formats; retain representative invalid-input checks for current boundaries.
Use small input tables for variants of one contract. Avoid constructor/getter/derived-trait
tests, standard-library retests, and assertions tied only to internal implementation.
Keep separate race tests when event ordering is the behavior under test.

CLI tests share self-cleaning temporary directories and run environment-sensitive checks
in child processes. E2E fixtures own their response maps per test, report unexpected
browser errors automatically, and retain traces/screenshots only on failure. Browser
WASM mode is configured once at the crate root. Maintenance-script tests remain with
their executable tools until those tools are retired.

Run the smallest relevant scope while editing:

```bash
cargo test --locked -p websh-core engine::shell
cargo test --locked -p websh-cli --test cli attest
just test-wasm runtime::wallet
just e2e cache.spec.js
just test-tools
```

Use `just verify` before completing changes across layers. Test counts are an outcome,
not a coverage target; fewer cases should mean less duplication, not weaker contracts.

## CSS Gate

`just lint-css` checks CSS syntax and the component token policy: no hexadecimal
colors, an explicit unit list, and token families for stacking, font weights, radii,
and duration declarations with the exceptions defined in `.stylelintrc.json`.
Prefer semantic tokens in components; primitive values belong in the token/theme layers.

## Performance Gate

`just size` audits every file in `target/verify/dist` by default. Runtime assets
(everything outside `content/`) are budgeted together; content and full deployment
sizes are reported separately. WASM, JavaScript, CSS, font, and vendor subtotals use
Brotli compression. Pass a different built directory explicitly with `just size dist`.
Budgets belong to the asset audit and must change only with an explained product
or packaging change, never to conceal missing file types.

## Browser Gate

`just e2e` serves the existing `target/verify/dist` on `127.0.0.1:4173` unless `WEBSH_E2E_BASE_URL` is set. Run `just build-check` first. Browser WASM tests use Playwright Chromium with the exact locked wasm-bindgen runner; there is no ChromeDriver fallback or runner cache discovery. External content and wallet requests use local fixtures; the suite never switches to live external data. Browser smoke tests fail on same-origin asset 404s and cover root-host plus simulated `/ipfs/<cid>/` hash navigation.

## Trunk And Attestation Gate

`just build` runs the normal release preparation in the checkout. Stylance generates
CSS; `websh-cli prepare` refreshes content and then builds ledger/attestation subjects
once. Development preparation refreshes only the manifest. New signing is disabled
when `WEBSH_NO_SIGN=1`; verification always uses that setting in its isolated source.

## Dependency maintenance

`cargo deny` and `cargo machete` enforce the Rust dependency gate. The small vendored
`syn_derive` patch replaces an unmaintained diagnostic dependency with `syn::Error`;
its upstream license, delta, and diagnostic tests are retained under `vendor/syn_derive`.
Remove the patch when a published upstream release supplies the maintained path.

The existing `deny.toml` exceptions remain visible: `RUSTSEC-2023-0071` for the
transitive RSA dependency used by local PGP verification/import, and
`RUSTSEC-2024-0436` for upstream `paste` dependencies. This refactor adds no ignores;
a passing gate does not mean the dependency graph has no advisories.

The npm audit on 2026-10-04 reports
[braces GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm), reported
through seven high-severity development-package entries in Stylelint's dependency chain.
The advisory lists no patched release; npm's proposed old Stylelint downgrade is not
a suitable fix. The CSS gate
passes source text and explicit code filenames to Stylelint instead of accepting glob
input; repository configuration patterns are fixed. No advisory is suppressed. These
packages are not shipped with the site. A maintained upstream fix remains follow-up work.
