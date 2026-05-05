# websh deployment script

set dotenv-load
set shell := ["bash", "-cu"]

# Run dev server
serve:
    env -u NO_COLOR CARGO_TARGET_DIR=target/dev trunk serve --dist dist-dev

# Release build
build:
    trunk build --release

# Install tracked browser QA dependencies
qa-install:
    npm install

# Browser end-to-end checks; Playwright starts the release Trunk server.
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
    cargo deny check --hide-inclusion-graph
    cargo machete --with-metadata --skip-target-dir

# Full local verification gate
verify: qa-install deps-check web-wasm-test
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
