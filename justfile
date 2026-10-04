set shell := ["bash", "-cu"]
export PATH := justfile_directory() / "target/tools/bin" + ":" + env("PATH")

# Run dev server
serve:
    env -u NO_COLOR trunk serve --locked --dist dist-dev

# Release build
build:
    trunk build --release --locked

# Isolated unsigned release build for browser and size checks.
build-check:
    npm run build:check

# Prepare exact developer tools and locked browser QA dependencies.
setup:
    npm run setup

# Browser checks against the unsigned verification build.
[positional-arguments]
e2e *args:
    npm run e2e -- "$@"

# Browser-owned wasm-bindgen tests.
test-wasm filter="":
    node tests/wasm/run.cjs {{quote(filter)}}

# Tooling contracts and one-time migration checks.
test-tools:
    npm run test:tools

# CSS lint (token enforcement)
lint-css:
    npm run lint:css

# Workspace boundaries and architecture documentation.
docs-check:
    npm run docs:drift

# Compressed runtime asset budgets.
size dist="target/verify/dist":
    npm run size:check -- {{quote(dist)}}

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
    cargo clippy --locked --workspace --all-targets -- -D warnings
    cargo clippy --locked -p websh-web --target wasm32-unknown-unknown --all-targets --all-features -- -D warnings
    cargo test --locked --workspace
    cargo test --locked --manifest-path vendor/syn_derive/Cargo.toml --target-dir target/vendor-tests
    cargo check --locked -p websh-core --target wasm32-unknown-unknown
    just test-wasm
    just test-tools
    just lint-css
    just docs-check
    just build-check
    just size
    just e2e

# Remove generated site outputs and reports; pass --dry-run to preview.
[positional-arguments]
clean *args:
    node scripts/clean.cjs outputs "$@"

# Also remove owned Cargo build caches, preserving installed tools.
[positional-arguments]
clean-cache *args:
    node scripts/clean.cjs cache "$@"

# Build and upload to Pinata
pin:
    cargo run --locked -p websh-cli -- deploy pinata
