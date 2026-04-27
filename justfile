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

# CSS lint (token enforcement)
lint-css:
    npm run lint:css

# Full local verification gate
verify: qa-install
    cargo test
    cargo test --features mock --test commit_integration
    npm run lint:css
    env -u NO_COLOR trunk build --release
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
