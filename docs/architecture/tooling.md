# Repository tooling

## Ownership

`justfile` is the public developer command map. Run `just --list` to discover tasks
and `just --show verify` to inspect the full gate. Node implements repository QA;
the Rust CLI owns content authoring, preparation, signing, and deployment. Keep each
operation in its owning layer instead of adding a second task framework.

| Location | Responsibility |
| --- | --- |
| `scripts/` | Tool setup, isolated build, local artifact serving, lint, architecture checks, cleanup |
| `tests/` | Native browser runners, E2E fixtures, tool behavior checks, asset budgets |
| `tools/` | Explicit one-time maintenance, never loaded by the application |
| `docs/architecture/` | Maintained contracts and operational guidance |
| `docs/migrations/` | Release migration steps and artifacts until applied |
| `.websh/local/` | Ignored local authoring, private data, and recoverable historical archives |

`npm run setup` bootstraps the pinned `just` binary. Subsequent developer commands use
`just`. Runtime/compiler pins live in `rust-toolchain.toml`, `.node-version`, and
`package.json`; native tool pins live in `scripts/tools.json`. Matching installed tools are reused;
missing native tools are installed under `target/tools`. The WASM test runner
must match `Cargo.lock`. Setup installs; verification checks without installing.
Optional publishing tools such as GPG, GitHub CLI, and Pinata are required only by
the corresponding native workflows.

## Build outputs

| Path | Owner |
| --- | --- |
| `dist/` | Normal release build |
| `dist-dev/` | Development server |
| `dist-<name>/` | Explicit custom deployment output |
| `target/verify/source/` | Isolated verification source snapshot |
| `target/verify/dist/` | Shared artifact for size and E2E checks |
| `target/debug/`, `target/release/`, `target/wasm32-unknown-unknown/` | Cargo compilation caches |
| `target/tools/` | Installed developer tools |
| `test-results/`, `playwright-report/` | Browser failure diagnostics |

Development and checks share normal Cargo caches. Verification stages source to
protect authored files and published signatures; it reuses the repository's compiler
cache. No development task renames or swaps the real `content/` directory.

Trunk runs Stylance and one `websh-cli prepare` hook. Independent hooks may execute
concurrently, so only `prepare` writes content, ledger, and attestation artifacts.
Deployment accepts only root-level `dist` or `dist-<name>` output directories and
rejects symlinks; source paths cannot be selected for build cleanup. Deployment alone
reads the repository `.env`, and passes values to its child
processes without changing the parent environment. General development and checks
do not implicitly load deployment credentials.

## Cleanup

`just clean` removes `dist/` and reserved `dist-<name>/` distributions, the verification stage, browser reports,
and the generated CSS bundle. `just clean-cache` also removes known Cargo build caches.
Both scoped commands preserve installed tools, `.last-cid`, unknown directories, authored content,
and `.websh/local/`. Stop builds and development servers before cleaning.

Use these scoped commands instead of an unqualified `cargo clean`, which removes the
entire target directory, including repository-installed tools. There is no automatic
periodic cleaner and no recursive cleanup of ignored files.

Historical branches with unique work can be preserved as verified Git bundles in
`.websh/local/archive/` before removal. This private local archive is not an off-machine
backup. Inspect ignored worktree files before removing a checkout.

## Assets and maintenance

The app targets modern WASM-capable browsers and ships WOFF2 fonts. Vendored assets
retain their licenses and provenance. Runtime size budgets count all shipped file
types; content size is reported separately. Review math rendering and IPFS subpath
navigation after asset packaging changes.

Retire one-time migration tools and their tests after the corresponding deployed data
has been migrated. They are not application compatibility paths. Keep dependency
patches only while the dependency graph still requires them, with a removal condition
in their provenance record.

## Design references

- [Cargo cleanup semantics](https://doc.rust-lang.org/cargo/commands/cargo-clean.html)
  explain why local tool installation needs protection from blanket target cleanup.
- [Just settings](https://just.systems/man/en/settings.html) document opt-in dotenv
  loading and argument handling; only deployment loads its credentials here.
- [Trunk configuration](https://github.com/trunk-rs/trunk/blob/main/Trunk.toml)
  describes concurrent same-stage hooks, motivating a single content writer.
- [KaTeX font options](https://katex.org/docs/font) document WOFF2-only packaging.
- [Web app manifest URL rules](https://www.w3.org/TR/appmanifest/#start_url-member)
  define relative start URLs for root and path-gateway deployments.
