# websh deployment script

set dotenv-load
set shell := ["bash", "-cu"]
export PATH := justfile_directory() / "target/tools/bin" + ":" + env("PATH")

# Run dev server
serve:
    env -u NO_COLOR CARGO_TARGET_DIR=target/dev trunk serve --dist dist-dev

# Release build
build:
    trunk build --release

# Prepare exact developer tools and locked browser QA dependencies.
setup:
    npm run setup

# Browser checks against the unsigned verification build.
e2e:
    npm run e2e

# Browser performance timing snapshot. Override target with
# WEBSH_PERF_BASE_URL=https://example.invalid.
perf-content:
    npm run perf:content

# Browser-owned wasm-bindgen tests.
web-wasm-test:
    node tests/web-wasm-test.cjs

# CSS lint (token enforcement)
lint-css:
    npm run lint:css

# Rust dependency hygiene checks
deps-check:
    cargo deny --locked check --hide-inclusion-graph
    cargo machete --with-metadata --skip-target-dir crates
    cargo machete --skip-target-dir vendor

# Full local verification gate
verify:
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

# Clean build artifacts
clean:
    trunk clean
    rm -f .last-cid

# Build and upload to Pinata
pin:
    cargo run --bin websh-cli -- deploy pinata

# Swap content/ for the local content-dev/ fixture and regenerate the ledger.
# `content-dev/` is gitignored; real content is parked at `content-real/`.
dev-content:
    @if [ -d content-real ]; then echo "already in dev mode"; exit 0; fi
    @if [ ! -d content-dev ]; then echo "missing content-dev/"; exit 1; fi
    mv content content-real
    mv content-dev content
    cargo run --bin websh-cli -- content ledger

# Restore the real content/ tree and regenerate the ledger.
real-content:
    @if [ ! -d content-real ]; then echo "already in real mode"; exit 0; fi
    mv content content-dev
    mv content-real content
    cargo run --bin websh-cli -- content ledger
