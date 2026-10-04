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

The `verify` recipe currently runs:

```bash
npm run tools:check
cargo fmt --check
just deps-check
cargo check --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p websh-web --target wasm32-unknown-unknown --all-targets --all-features -- -D warnings
cargo test --locked --workspace
cargo test --locked --manifest-path vendor/syn_derive/Cargo.toml --target-dir target/vendor-tests
cargo check --locked -p websh-core --target wasm32-unknown-unknown
cargo check --locked -p websh-web --target wasm32-unknown-unknown
just web-wasm-test
npm run test:tools
npm run lint:css
npm run docs:drift
npm run build:check
npm run perf:budgets -- target/verify/dist
npm run e2e
```

`build:check` stages source and generated assets under `target/verify/source`, clears
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
- CSS: `npm run lint:css`
- Docs: `npm run docs:drift`
- Bundle budgets: `npm run perf:budgets -- dist`

## CSS Gate

`npm run lint:css` enforces the token policy for component CSS modules. Component CSS should use semantic tokens instead of raw pixel, color, or duration literals unless the lint rule intentionally allows the file.

## Performance Gate

`npm run perf:budgets` audits Brotli sizes for wasm, JavaScript, CSS, fonts, vendor assets, and total assets. The package script provides repository defaults; CI may tighten them with environment variables.

## Browser Gate

`npm run e2e` serves the existing `target/verify/dist` on `127.0.0.1:4173` unless `WEBSH_E2E_BASE_URL` is set. Run `npm run build:check` first. Browser WASM tests use Playwright Chromium with the exact locked wasm-bindgen runner; there is no ChromeDriver fallback or runner cache discovery. Use `WEBSH_LIVE_MEMPOOL=1` only when intentionally testing the live mempool backend; fixture mode is the default. Browser smoke tests fail on same-origin asset 404s and cover root-host plus simulated `/ipfs/<cid>/` hash navigation.

## Trunk And Attestation Gate

`trunk build --release` runs the real pre-build chain:

1. Stylance bundle generation.
2. Content manifest refresh.
3. Attestation build.

`attest build` runs fully in release profile, skips in dev profile unless `--force` is used, and honors `WEBSH_NO_SIGN=1`.

## Dependency maintenance

`cargo deny` and `cargo machete` enforce the Rust dependency gate. The small vendored
`syn_derive` patch replaces an unmaintained diagnostic dependency with `syn::Error`;
its upstream license, delta, and diagnostic tests are retained under `vendor/syn_derive`.
Remove the patch when a published upstream release supplies the maintained path.

The existing `deny.toml` exceptions remain visible: `RUSTSEC-2023-0071` for the
transitive RSA dependency used by local PGP verification/import, and
`RUSTSEC-2024-0436` for upstream `paste` dependencies. This refactor adds no ignores;
a passing gate does not mean the dependency graph has no advisories.

The remaining npm audit finding is the unpublished fix for
[braces GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm), reported
through seven development-package entries in Stylelint's dependency chain. The CSS gate
passes source text and explicit code filenames to Stylelint instead of accepting glob
input; repository configuration patterns are fixed. No advisory is suppressed. These
packages are not shipped with the site. A maintained upstream fix remains follow-up work.
