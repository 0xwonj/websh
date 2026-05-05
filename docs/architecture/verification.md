# Verification Contract

## Default Gate

Run:

```bash
just verify
```

The `verify` recipe currently runs:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p websh-web --target wasm32-unknown-unknown --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test -p websh-core --features mock --test commit_integration
cargo check -p websh-core --target wasm32-unknown-unknown
cargo check -p websh-web --target wasm32-unknown-unknown
npm run lint:css
npm run docs:drift
env -u NO_COLOR trunk build --release
npm run perf:budgets
npm run e2e
```

The recipe also depends on `qa-install`, `deps-check`, and `web-wasm-test`.

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

`npm run e2e` starts a release Trunk server on `127.0.0.1:4173` unless `WEBSH_E2E_BASE_URL` is set. Use `WEBSH_LIVE_MEMPOOL=1` only when intentionally testing the live mempool backend; fixture mode is the default. Browser smoke tests fail on same-origin asset 404s and cover root-host plus simulated `/ipfs/<cid>/` hash navigation.

## Trunk And Attestation Gate

`trunk build --release` runs the real pre-build chain:

1. Stylance bundle generation.
2. Content manifest refresh.
3. Attestation build.

`attest build` runs fully in release profile, skips in dev profile unless `--force` is used, and honors `WEBSH_NO_SIGN=1`.
