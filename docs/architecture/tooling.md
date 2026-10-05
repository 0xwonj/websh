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
GPG is required only for local signing; Pinata is required only for deployment.
The native CLI does not execute Git or GitHub CLI. Editing and pushing authored
repositories remain ordinary Git operations.

## Build outputs

| Path | Owner |
| --- | --- |
| `dist/` | Normal release build |
| `dist-dev/` | Development server |
| `dist-<name>/` | Reserved named Trunk outputs, including development builds |
| `target/verify/source/` | Isolated verification source snapshot |
| `target/verify/dist/` | Shared artifact for size and E2E checks |
| `target/debug/`, `target/release/`, `target/wasm32-unknown-unknown/` | Cargo compilation caches |
| `target/tools/` | Installed developer tools |
| `test-results/`, `playwright-report/` | Browser failure diagnostics |

Development and checks share normal Cargo caches. Verification stages source to
protect authored files and published signatures; it reuses the repository's compiler
cache. No development task renames or swaps the real `content/` directory.

Trunk runs Stylance and one `websh-cli sync` hook. Independent hooks may execute
concurrently, so only sync writes content, ledger, acknowledgement, and subject artifacts.
All profiles generate the same deterministic data. Generation does not read Git history,
require GPG, or invoke a signer.

`just publish` explicitly runs sync, `attest sign`, a locked release build, and `deploy`.
Deployment validates the fixed existing `dist/`; it does not build or remove outputs.
For an offline signature flow, import the signatures and run `just build` followed by
`cargo run --locked -p websh-cli -- deploy`. Strict publication requires the site's PGP
identity, while supplemental Ethereum signatures do not satisfy that requirement.

Deployment alone reads the repository `.env` and passes values to its Pinata child
without changing the parent environment. General development, generation, signing,
and checks do not implicitly load deployment credentials.

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
