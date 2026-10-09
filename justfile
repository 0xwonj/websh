set shell := ["bash", "-cu"]
export PATH := justfile_directory() / "target/tools/bin" + ":" + env("PATH")

# Run dev server
serve:
    stylance --output-file assets/bundle.css crates/websh-web
    env -u NO_COLOR trunk serve --locked --dist dist-dev --enable-cooldown

# Production-trust preview of a committed signed content snapshot; no .env or signing.
preview content="../websh-content" port="4186":
    cargo build --locked -p websh-cli
    env -u NO_COLOR trunk build --release --locked --dist dist-preview
    node scripts/preview.cjs {{quote(content)}} {{quote(port)}}

# Release build of the app; independent of content authoring.
build:
    env -u NO_COLOR trunk build --release --locked

# Isolated app build for browser and size checks.
build-check:
    node scripts/build-check.cjs

# Prepare exact developer tools and locked browser QA dependencies.
setup:
    npm run setup

# Browser checks against the verification build and signed content fixtures.
[positional-arguments]
e2e *args:
    cargo build --locked -p websh-core --example page-subjects
    env -u NO_COLOR node_modules/.bin/playwright test --reporter=line --workers=1 "$@"

# Browser-owned wasm-bindgen tests.
test-wasm filter="":
    node tests/wasm/run.cjs {{quote(filter)}}

# Repository tooling contracts.
test-tools:
    node --test tests/tools/*.test.cjs

# CSS lint (token enforcement)
lint-css:
    node scripts/lint-css.cjs

# Workspace boundaries and architecture documentation.
docs-check:
    node scripts/check-docs-drift.cjs

# Compressed runtime asset budgets.
size dist="target/verify/dist":
    node scripts/check-size.cjs {{quote(dist)}}

# Rust dependency hygiene checks
deps-check:
    cargo deny --locked check --hide-inclusion-graph
    cargo machete --with-metadata --skip-target-dir crates
    cargo machete --skip-target-dir vendor

# Full local verification gate
verify:
    node scripts/tools.cjs check
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

# Build and publish the app. Content publication is a separate CLI workflow.
publish:
    env -u NO_COLOR trunk build --release --locked
    cargo run --locked -p websh-cli -- deploy
