# websh Architecture: 3-Crate Workspace

## 1. Goals and constraints

**Goals**
- Compile-time enforced layering between domain, web app, and build-time CLI.
- Eliminate cross-cutting `cfg(target_arch=...)` gates from the dependency graph.
- CLI subcommand files become thin clap dispatchers; domain engines have first-class homes.
- Browser-side attestation verification is a real feature (not a vestigial constant).
- Workflow stays single-author friendly: simple `cargo` commands, one `Cargo.lock`, one `target/`.

**Constraints (locked in by prior conversation)**
- Three crates: `websh-core`, `websh-web`, `websh-cli`. No further subdivision.
- One-shot migration. No intermediate "module reorg first, then crates" detour.
- No legacy / fallback / migration shims.
- Single-author project. Don't introduce abstractions that only pay off at multi-team or multi-consumer scale.

**Research-resolved unknowns**
- **`pgp` crate** (rPGP) **does** support `wasm32-unknown-unknown` (CI-checked per PLATFORMS.md). Enable the `"wasm"` feature; do **not** enable `"asm"`. Browser-side signature verification is in scope. The current "trust the bundled fingerprint" pattern can become real verification in the web crate.
- **Stylance multi-crate**: `[package.metadata.stylance]` lives per-crate. `import_crate_style!` resolves CSS paths relative to the crate's `Cargo.toml` directory. Workspace-level inheritance is available via `workspace = true` + `[workspace.metadata.stylance]`, but for our shape (only `websh-web` has CSS) we keep config in the web crate only — no inheritance needed.
- **Trunk in workspace**: `Trunk.toml` lives inside `crates/websh-web/`. Trunk auto-discovers `Cargo.toml` in the parent of `index.html`. Pre-build hooks use `[[hooks]]` with `stage = "pre_build"`.
- **Leptos signal lifecycle**: arena-allocated; lifetime tied to the UI scope, not Rust lexical scope. Reactive primitives drop when their owner unmounts. This means `AppContext` (pure `Copy` signal handles) doesn't leak across crate boundaries — it lives in `websh-web` because Leptos is the lifecycle owner.

---

## 2. Top-level layout

```
Cargo.toml                       workspace manifest (members + workspace.dependencies + workspace.package)
Cargo.lock                       single lockfile
target/                          shared build cache
content/                         canonical content tree (read by web at runtime; written by cli at build time)
assets/                          shared asset tree (tokens, fonts, attestations.json)
index.html                       trunk entrypoint (top-level so trunk can locate workspace root)
Trunk.toml                       trunk config (hooks, build target -> crates/websh-web)
README.md
CLAUDE.md
docs/

crates/
├── websh-core/
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
├── websh-cli/
│   ├── Cargo.toml
│   ├── src/
│   │   └── main.rs
│   └── tests/
└── websh-web/
    ├── Cargo.toml
    ├── src/
    │   ├── lib.rs
    │   └── main.rs
    └── tests/
```

`Trunk.toml` and `index.html` stay at the workspace root (rather than inside `crates/websh-web/`) because `trunk serve` from the project root is the established workflow and the `pre_build` hook then naturally runs `cargo run -p websh-cli -- ...` from the workspace root. Path-based asset references (e.g., `data-trunk rel="rust" data-bin=...`) point into `crates/websh-web/`.

---

## 3. Crate roles

### 3.1 `websh-core` — domain + engine

**Role.** Pure-Rust shared library. Compiles for both `wasm32-unknown-unknown` and native. Hosts everything the browser and CLI both need.

**Top-level modules (final shape).**
```
src/
├── lib.rs
├── domain/                    pure data types — no I/O, no async, no platform deps
│   ├── filesystem.rs          FsEntry, NodeMetadata, paths, virtual-path
│   ├── manifest.rs            ContentManifestEntry, ContentManifestDocument, EntryExtensions
│   ├── mempool.rs             MempoolStatus, Priority, MempoolFields
│   └── changes.rs             ChangeType, ChangeSet, Summary
├── filesystem/                merged in-memory filesystem engine
│   ├── global_fs.rs
│   ├── intent.rs              ReaderIntent, RouteRequest/Resolution/Frame
│   ├── routing.rs
│   ├── content.rs             read_text / read_bytes through backend registry
│   └── merge.rs               apply_global_change, merge_global_view
├── runtime/                   commit + load + state orchestration
│   ├── commit.rs              prepare_commit + commit_backend
│   ├── loader.rs              RuntimeLoad, scan/mount orchestration
│   ├── state.rs               RuntimeState (token presence, etc.)
│   └── mod.rs
├── storage/                   backend trait + adapters (hexagonal port + adapters)
│   ├── backend.rs             StorageBackend trait, ScannedSubtree, CommitRequest, errors
│   ├── github.rs              wasm-cfg gloo-net adapter
│   ├── idb.rs                 wasm-cfg adapter
│   └── mock.rs                feature-gated test adapter
├── mempool/                   pure mempool helpers (Phase 1's home expanded)
│   ├── categories.rs          LEDGER_CATEGORIES
│   ├── parse.rs               line-based frontmatter parser, transform_mempool_frontmatter
│   ├── serialize.rs           ComposePayload, serialize_mempool_file, slug_from_title
│   ├── form.rs                ComposeForm, ComposeError, validate_form, form_to_payload
│   ├── path.rs                mempool_root() (LazyLock), derive_new_path, placeholder_frontmatter
│   └── manifest_entry.rs      build_mempool_manifest_state — single SoT for write paths
├── attestation/               artifact loading + verification only
│   ├── artifact.rs            AttestationArtifact, subject_for_route
│   ├── verify.rs              browser-runnable signature verification (uses pgp wasm feature)
│   └── ledger.rs              ContentLedger types + verification
├── crypto/                    primitives only (no higher-level wrappers)
│   ├── eth.rs
│   ├── ack.rs
│   └── pgp.rs                 fingerprint constants + verification helpers (no signing)
├── shell/                     terminal command interpreter — runs in browser
│   ├── parse.rs               command-line parsing
│   ├── result.rs              CommandResult, SideEffect
│   ├── autocomplete.rs
│   └── execute/               (was the giant execute.rs — split per family)
│       ├── mod.rs             dispatcher + ExecuteCtx
│       ├── filesystem.rs      touch, mkdir, rm, rmdir, edit, echo_redirect
│       ├── sync.rs            sync, sync_pull, sync_commit, sync_status, sync_diff, sync_auth
│       ├── env.rs             export, unset, id
│       ├── listing.rs         ls, cd, cat, format_ls_output
│       └── misc.rs            theme, explorer
├── content_routes.rs          content_href_for_path, attestation_route_for_node_path
└── utils.rs                   small cross-platform helpers (format, time, url, ring_buffer)
```

**Dependency rules.**
- No Leptos, no clap, no `serde_yaml`, no `lopdf`, no `imagesize`.
- `pgp = { version = "0.19", default-features = false, features = ["wasm"] }`.
- `getrandom = { version = "0.2", features = ["js"] }` under `[target.'cfg(target_arch = "wasm32")'.dependencies]`.
- Wasm-only deps (`wasm-bindgen`, `wasm-bindgen-futures`, `web-sys` with narrow features, `js-sys`, `gloo-net`, `gloo-timers`, `idb`, `serde-wasm-bindgen`) gated under `[target.'cfg(target_arch = "wasm32")'.dependencies]`.
- Native test deps (`tokio` with current-thread runtime) under `[dev-dependencies]`.
- Shared deps (`serde`, `serde_json`, `sha2`, `hex`, `alloy-primitives`, `base64`, `unicode-normalization`, `regex`) declared via `workspace = true`.

**Features.**
- `mock` — gates `storage/mock.rs`. Consumed by `websh-cli`'s integration test as `dev-dependencies` opt-in.

**Two-target invariant.** Every public API compiles on both `wasm32-unknown-unknown` and the host triple. CI runs `cargo check -p websh-core` and `cargo check -p websh-core --target wasm32-unknown-unknown` on every change.

### 3.2 `websh-cli` — native build-time binary

**Role.** Build pipeline: content sync, attestation building, mempool CLI orchestration, mount declaration scanning, ledger generation. The binary that Trunk's pre-build hook calls.

**Top-level modules.**
```
src/
├── main.rs                    clap top-level dispatcher
├── cli/                       thin per-subcommand parsers + dispatch
│   ├── mempool.rs             ~50 lines: clap parser → engine::mempool::*
│   ├── attest.rs              ~50 lines: clap parser → engine::attestation::*
│   ├── content.rs             ~30 lines: clap parser → engine::content_sync::*
│   ├── mount.rs               ~50 lines
│   └── ledger.rs              ~30 lines
├── engine/                    domain engines (the actual work, extracted from clap shim)
│   ├── content_sync/
│   │   ├── mod.rs             sync_content
│   │   ├── projection.rs      bundle_manifest (from sidecars)
│   │   ├── frontmatter.rs     serde_yaml-based YAML parser + merge_authored
│   │   └── sidecar.rs         atomic .meta.json / _index.dir.json I/O
│   ├── attestation/
│   │   ├── mod.rs             attest_all top-level
│   │   ├── build.rs           per-route signing pipeline
│   │   ├── subject.rs         subject CRUD + slugify
│   │   ├── sign.rs            GPG signing helpers (uses pgp signing features)
│   │   └── verify.rs          attest verify subcommand (calls websh_core::attestation::verify)
│   ├── mempool/
│   │   ├── mod.rs             entrypoints called from cli/mempool.rs
│   │   ├── list.rs            with drift detection
│   │   ├── add.rs             builds manifest entry via core::mempool::build_mempool_manifest_state
│   │   ├── drop.rs
│   │   ├── promote.rs         orchestrator + rollback
│   │   └── gh_contents.rs     PUT/GET helpers (single dedup home)
│   ├── derived/
│   │   ├── pdf.rs             read_pdf_dimensions (lopdf)
│   │   └── image.rs           read_image_dimensions (imagesize)
│   ├── mount.rs               mount declaration writers
│   └── ledger.rs              generate_content_ledger
├── gh.rs                      gh CLI subprocess wrappers (subprocess-based, distinct from
│                              core::storage::github which is wasm/HTTP)
└── io.rs                      atomic file ops, stdin reading
```

**Dependency rules.**
- `websh-core = { path = "../websh-core" }`.
- Native-only deps in plain `[dependencies]`: `clap`, `pgp` (with signing features), `lopdf`, `imagesize`, `serde_yaml`, `rand`, `tokio` (`rt`, `macros`), `base64`, `hex`, `sha2`, `serde`, `serde_json`.
- No Leptos, no web-sys, no gloo-*.
- Dev-deps include `websh-core = { path = "../websh-core", features = ["mock"] }` for the integration test.

**Engine-extracted-from-clap pattern.** `cli/mempool.rs` is a clap parser that calls `engine::mempool::add::run(args)` etc. The work lives in `engine/`. This finally enforces what was structurally hard to do at the module level. Future contributors trying to add a CLI feature will write engine code under `engine/<domain>/` and dispatch from `cli/<domain>.rs`, not put work behind clap.

### 3.3 `websh-web` — Leptos web app (cdylib for trunk)

**Role.** The browser app. All UI. The trunk-served wasm32 binary.

**Top-level modules.**
```
src/
├── lib.rs
├── main.rs                    leptos::mount_to_body(App)
├── app.rs                     App component, AppContext (Copy signal handles), router glue
├── components/
│   ├── chrome/                site chrome, breadcrumbs, palette, wallet menu
│   ├── pages/
│   │   ├── home/              homepage (was components/home/, post-Phase-4 split intact)
│   │   │   ├── mod.rs
│   │   │   └── sections.rs
│   │   ├── reader/            reader views + edit toolbar
│   │   ├── ledger/            (was components/ledger_page.rs)
│   │   └── explorer/          file explorer
│   ├── shared/                IdentifierStrip, MetaTable, MonoValue, SignatureFooter, SiteFrame
│   ├── editor/                EditModal
│   ├── terminal/              terminal UI surface (boots; dispatches to core::shell)
│   ├── mempool/               UI-only mempool surface
│   │   ├── component.rs       Leptos Mempool component
│   │   ├── loader.rs          load_mempool_files via view_global_fs
│   │   ├── model.rs           LoadedMempoolFile, MempoolModel, build_mempool_model
│   │   └── commit.rs          async save_raw (depends on AppContext)
│   └── markdown.rs            inline markdown renderer
├── platform/                  wasm-specific glue (was utils/dom, utils/wasm_cleanup)
│   ├── dom.rs
│   ├── wasm_cleanup.rs
│   └── breakpoints.rs         (uses leptos-use)
├── render/                    HTML rendering (browser-only; CLI doesn't render)
│   ├── markdown.rs            comrak + ammonia pipeline
│   └── theme.rs
└── style.css                  (or stylance configures bundle target)
```

**Dependency rules.**
- `websh-core = { path = "../websh-core" }`.
- Leptos and ecosystem live here only: `leptos = { version = "0.8", features = ["csr"] }`, `leptos_icons`, `icondata`, `leptos-use`.
- Wasm-only deps: `wasm-bindgen`, `wasm-bindgen-futures`, `web-sys` (broad features), `js-sys`, `gloo-net`, `gloo-timers`, `idb`, `serde-wasm-bindgen`, `comrak`, `ammonia`, `stylance`, `getrandom = { features = ["js"] }`.
- `[package.metadata.stylance]` here only: `output_file = "../../assets/bundle.css"`, `folders = ["src"]`, `extensions = [".module.css"]`, `hash_len = 7`.

**AppContext lives here** (not in core). `AppContext` is a `Copy` struct of Leptos signal handles — its lifetime is tied to the Leptos arena, which the web crate owns. Core exposes pure types that AppContext wraps in signals; AppContext itself doesn't cross the crate boundary.

**Render is web-only.** `comrak`/`ammonia` only compile in `websh-web` because only the browser HTML-renders markdown. CLI processes frontmatter via `serde_yaml`, never renders to HTML.

---

## 4. Layered design within `websh-core`

`websh-core` itself follows a 3-layer internal hierarchy, mirroring hexagonal architecture without the ceremony:

```
                        ┌────────────────────────────────────┐
   pure types           │ domain/                            │
   no I/O               │   filesystem, manifest, mempool,   │
   no async             │   changes                          │
                        └────────────────────────────────────┘
                                       ▲
                                       │ depended on by
                                       │
                        ┌────────────────────────────────────┐
   ports + engines      │ filesystem/, runtime/, mempool/,   │
   pure-Rust I/O        │   shell/, attestation/, crypto/    │
                        │ storage/backend.rs (the port)      │
                        └────────────────────────────────────┘
                                       ▲
                                       │ implemented by
                                       │
                        ┌────────────────────────────────────┐
   adapters             │ storage/{github, idb, mock}.rs     │
   target-specific      │   (cfg-gated within crate)         │
                        └────────────────────────────────────┘
```

**Why not extract `domain/` to its own crate (the 4-crate option from earlier)?** Inside one crate, `pub(crate)` boundaries plus the `domain/` subfolder convention give us the same separation without the Cargo.toml tax. The domain–engine boundary in websh isn't load-bearing enough to need compile-time enforcement — it's the wasm/native boundary that is, and that's what the 3-crate split solves.

**`storage::backend` is the port.** `StorageBackend` trait already follows hexagonal port conventions. GitHub and IDB are wasm-only adapters; `gh`-subprocess (CLI) is its own adapter outside core. Mock is the test adapter.

**`shell/execute/` runs in the browser.** This is unintuitive — terminal commands feel CLI-flavored — but the websh terminal is a *browser surface*, the "websh" panel inside the wasm app. The actual native CLI is `websh-cli`, a separate concept. Keeping `shell/` in core is the right call.

---

## 5. Cross-cutting patterns

### 5.1 Hexagonal storage (already applied, formalized)
- `storage/backend.rs` defines the port (`StorageBackend` trait).
- Adapters live as siblings: `github.rs` (wasm HTTP), `idb.rs` (wasm browser DB), `mock.rs` (tests). The CLI's subprocess-based GitHub access is an adapter that lives in `websh-cli` (it's not used by the browser).
- Don't extend hexagonal further. Other subsystems (mempool, attestation, content_sync) have one implementation each; trait-based abstraction is over-engineering until a second backend appears.

### 5.2 Engine extracted from clap shim
- `cli/<subcommand>.rs` files in `websh-cli` are clap parsers + dispatch. ~50 lines each.
- Real work lives in `engine/<domain>/`. Domain modules can be tested without invoking the binary.
- Browser code calls into core's engines (e.g., `core::mempool::build_mempool_manifest_state`); CLI code calls into its own engines plus core's. The shapes match.

### 5.3 UI patterns (Leptos best practices)
- **`AppContext` is a `Copy` struct of signal handles** — the existing pattern. Already exemplary; preserve in the migration.
- **Memo over derived state, Effect for side effects only.** No `create_effect` that just computes a value. Phase 1-3, 5 already removed the violations.
- **Slot pattern for compound components** (`#[slot]` macro) when a component has named children with semantic roles (e.g., `<DialogHeader>`, `<DialogBody>`, `<DialogActions>`). Children prop for opaque content. Render props rarely needed.
- **`prop:hidden` over `<Show>`** for disclosure widgets where ARIA `aria-controls` must always resolve. The mempool component already does this — propagate where appropriate.
- **`Memo<RouteFrame>` as single navigation truth** — narrow reactive inputs, broad derived outputs. Already in place; preserve.
- **NodeRef + `get_untracked()` in event handlers.** Mounted-by-the-time-event-fires invariant; avoids spurious tracking.
- **`WasmCleanup<F: ?Sized>` (already extracted)** for `Closure` values that need to satisfy Leptos's `Send + Sync + 'static` cleanup bound on wasm. Now lives in `websh-web::platform::wasm_cleanup`.

### 5.4 Error handling
- **Per-domain error types in core**: `runtime::CommitError`, `mempool::ParseError`, `attestation::VerifyError`. Not one big `WebshError`.
- **`thiserror` for domain errors**, plain `String` for CLI error messages where a typed error doesn't pay off (existing pattern; preserve).
- **`Result` at every public engine entry point.** Panics only for invariants that should be impossible at the call site (`expect("Show guards Some")` etc.).

### 5.5 Two-source-of-truth invariants (carry over from current code)
- **Mempool manifest state**: `build_mempool_manifest_state` is the single producer. Browser save_raw and CLI add both call it. Don't reintroduce parallel constructors.
- **`derived.modified_at` deliberately omitted** for byte-stability under signed attestations. Document this in `core::domain::manifest`.
- **Category field**: typed in the manifest's mempool block; falls back to path-derived. Don't add a third source.

---

## 6. Workspace configuration (concrete sketches, not final files)

### Root `Cargo.toml`
```toml
[workspace]
members = ["crates/websh-core", "crates/websh-web", "crates/websh-cli"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
hex = "0.4"
base64 = "0.22"
alloy-primitives = { version = "1", features = ["k256", "serde"] }
unicode-normalization = "0.1"

[profile.release]
lto = true
opt-level = "z"
codegen-units = 1
panic = "abort"
strip = true
```

### `crates/websh-core/Cargo.toml` (sketch)
```toml
[package]
name = "websh-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
base64 = { workspace = true }
alloy-primitives = { workspace = true }
unicode-normalization = { workspace = true }
regex = { version = "1", default-features = false, features = ["std", "perf"] }
pgp = { version = "0.19", default-features = false, features = ["wasm"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = [...] }    # narrow set: Storage, Headers, Request, Response, Url
js-sys = "0.3"
gloo-net = "0.6"
idb = "0.6"
serde-wasm-bindgen = "0.6"
getrandom = { version = "0.2", features = ["js"] }

[features]
mock = []

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
```

### `crates/websh-cli/Cargo.toml` (sketch)
```toml
[package]
name = "websh-cli"
version.workspace = true
edition.workspace = true

[[bin]]
name = "websh-cli"
path = "src/main.rs"

[dependencies]
websh-core = { path = "../websh-core" }
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
hex = { workspace = true }
base64 = { workspace = true }
clap = { version = "4", features = ["derive"] }
pgp = "0.19"     # signing features enabled (no wasm restriction)
serde_yaml = "0.9"
lopdf = "0.34"
imagesize = "0.13"
rand = "0.8"
tokio = { version = "1", features = ["rt", "macros"] }

[dev-dependencies]
websh-core = { path = "../websh-core", features = ["mock"] }
```

### `crates/websh-web/Cargo.toml` (sketch)
```toml
[package]
name = "websh-web"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
websh-core = { path = "../websh-core" }
serde = { workspace = true }
serde_json = { workspace = true }
leptos = { version = "0.8", features = ["csr"] }
leptos_icons = "0.7"
icondata = { version = "0.7", default-features = false, features = ["bootstrap-icons", "lucide"] }
leptos-use = { version = "0.18", default-features = false, features = ["use_media_query"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = { version = "0.3", features = [...] }    # broad set
js-sys = "0.3"
gloo-net = "0.6"
gloo-timers = { version = "0.3", features = ["futures"] }
idb = "0.6"
serde-wasm-bindgen = "0.6"
getrandom = { version = "0.2", features = ["js"] }
comrak = { version = "0.52", default-features = false }
ammonia = "4"
stylance = "0.7"
console_error_panic_hook = "0.1"

[package.metadata.stylance]
output_file = "../../assets/bundle.css"
folders = ["src"]
extensions = [".module.css"]
hash_len = 7
class_name_pattern = "[name]-[hash]"
```

### `Trunk.toml` (root)
```toml
[build]
target = "index.html"

[[hooks]]
stage = "pre_build"
command = "cargo"
command_arguments = ["run", "-p", "websh-cli", "--", "content", "manifest"]
```

### `index.html` (root, points into web crate)
```html
<link data-trunk rel="rust" data-bin="websh-web" data-cargo-features="csr" />
<link data-trunk rel="copy-dir" href="content" />
<link data-trunk rel="copy-dir" href="assets" />
```

(Whether `data-bin` works for a `cdylib`-only crate or the web crate needs a tiny `[[bin]]` is a detail to confirm during migration.)

---

## 7. Migration sequencing

The migration is one PR / one sustained session, but committed as a sequence so `git bisect` works.

### Commit 1 — workspace skeleton
- Create `crates/{websh-core,websh-web,websh-cli}/` with empty `lib.rs`/`main.rs`.
- Root `Cargo.toml` workspace declaration with `[workspace.dependencies]`.
- Workspace builds (empty crates compile).

### Commit 2 — `websh-core` populated
- Move `src/{models,core,crypto,mempool,utils}` into `crates/websh-core/src/` under the new `domain/`, `filesystem/`, `runtime/`, `storage/`, `mempool/`, `attestation/`, `crypto/`, `shell/`, `utils/` layout.
- Adjust internal imports.
- Verify: `cargo check -p websh-core` and `cargo check -p websh-core --target wasm32-unknown-unknown` both succeed.
- Move tests from `tests/mempool_*.rs`, `tests/commit_integration.rs` into `crates/websh-core/tests/`.

### Commit 3 — `websh-cli` populated
- Move `src/cli/` into `crates/websh-cli/src/cli/`.
- Extract engines from `cli/{manifest,attest,mempool,mount,ledger}.rs` into `crates/websh-cli/src/engine/{content_sync,attestation,mempool,mount,ledger}/`.
- Each `cli/<subcommand>.rs` becomes a thin clap parser + dispatch.
- Move `tests/cli_crypto.rs` into `crates/websh-cli/tests/`.
- Verify: `cargo build -p websh-cli`, `cargo test -p websh-cli`.

### Commit 4 — `websh-web` populated
- Move `src/{app,components}.rs/rs` and supporting files into `crates/websh-web/src/`.
- Group `components/` into `chrome/`, `pages/{home,reader,ledger,explorer}/`, `shared/`, `editor/`, `terminal/`, `mempool/`, `markdown.rs`.
- Move `utils/{dom,wasm_cleanup,breakpoints}.rs` into `crates/websh-web/src/platform/`.
- Move `utils/markdown.rs` into `crates/websh-web/src/render/`.
- `[package.metadata.stylance]` config in `crates/websh-web/Cargo.toml`.
- Verify: `cargo check -p websh-web --target wasm32-unknown-unknown`.

### Commit 5 — Trunk + asset paths
- Move/rewrite `Trunk.toml` at root with the new pre-build hook.
- Adjust `include_str!` paths in `crates/websh-web/src/components/.../sections.rs` (count levels from new file location).
- Adjust `index.html` `data-bin` attribute and `data-trunk rel="copy-dir"` paths to point into the workspace layout.
- Verify: `trunk build` succeeds; `trunk serve` opens the app.

### Commit 6 — CLAUDE.md / README updates
- Update build commands: `cargo test --workspace`, `cargo run -p websh-cli`, `cargo clippy --workspace --all-targets`.
- Add architecture section pointing readers to this document.

### Commit 7 — Browser-side PGP verification (new feature enabled by migration)
- Wire `websh-core::attestation::verify` to actually run PGP verification in the browser using the `pgp` crate's `wasm` feature.
- Replace the homepage's "trust the bundled fingerprint" path with real verification.
- This commit is optional but the migration unlocks it.

---

## 8. Verification plan

After each commit and at end of migration:

| Command | Expectation |
|---|---|
| `cargo fmt --check` | clean |
| `cargo clippy --workspace --all-targets` | clean |
| `cargo check -p websh-core` | clean |
| `cargo check -p websh-core --target wasm32-unknown-unknown` | clean |
| `cargo check -p websh-web --target wasm32-unknown-unknown` | clean |
| `cargo check -p websh-cli` | clean |
| `cargo test --workspace` | all pass (598 lib + integration tests) |
| `cargo test -p websh-core --features mock` | mock-feature integration test passes |
| `trunk build` | succeeds with stylance bundle and pre-build hook |
| `grep -rn 'use crate::components' crates/websh-cli/` | zero hits |
| `grep -rn 'use websh_web' crates/websh-cli/` | zero hits |
| `grep -rn 'cfg(target_arch' crates/websh-cli/src/` | zero hits (no more cfg gates in CLI) |
| `wc -l` on every file in `crates/` | all under 800 lines |

End-to-end manual smoke (after Commit 5):
- `trunk serve`; navigate `/`, `/ledger`, a content reader page; verify rendering.
- Author flow: open `/new`, paste a draft, save; refresh, verify it appears in mempool list with populated gas/word-count.
- Edit flow: change a frontmatter status from `draft` to `review`, save; refresh, verify the change appears in mempool list (locks in the Phase 2 CRITICAL fix).
- CLI: `cargo run -p websh-cli -- content manifest` produces no diff on a clean tree.
- CLI: `cargo run -p websh-cli -- mempool list` shows entries; if a deliberate orphan blob is seeded, drift warning appears.

---

## 9. What we're NOT doing (out of scope)

Locked-in non-goals to prevent scope creep:

- **No 4th crate** (no `websh-domain` extraction). The `domain/` subfolder convention inside `websh-core` is sufficient.
- **No per-feature crates** (`websh-mempool`, `websh-attestation`, etc.). Three crates is the commitment.
- **No SSR**. The web crate is `csr`-only. Future SSR migration is out of scope and may not happen.
- **No Cargo features for "trim leptos for CLI" hacks**. Compile-time crate boundaries are the answer; we don't need feature flags to slim builds.
- **No global error type**. Per-domain `thiserror` enums where a typed error pays off; `String` errors at CLI surface where it doesn't.
- **No further hexagonal extension beyond storage**. `mempool`, `attestation`, `content_sync` each have one implementation; trait-based ports are over-engineering until a second adapter shows up.
- **No splitting `websh-cli`'s `engine/` into sub-crates** ever. If `engine/content_sync` ever needs to be reused elsewhere, that's the moment to extract; not before.
- **No `cargo-leptos`**. We're CSR-only; trunk is enough. cargo-leptos is for SSR/hydrate/islands setups.
- **No replacing the line-based mempool frontmatter parser with `serde_yaml`** in core. The parser's writer-controls-format invariant is sufficient; CLI uses `serde_yaml` for canonical content frontmatter, which is a different format.
- **No moving `WasmCleanup` to core**. It's UI-lifecycle bound; lives in `websh-web::platform`.

---

## 10. Decisions deferred to implementation

These are deliberately under-specified in this document — better answered while moving code:

- **Exact stylance class hashing**: `class_name_pattern = "[name]-[hash]"` keeps current behavior; tweak if collisions appear.
- **Whether browser PGP verification needs key-pinning beyond the hardcoded fingerprint**: depends on UX requirements; out of scope for the migration itself, in scope for the follow-up commit.
- **Whether `index.html` and `Trunk.toml` move inside `crates/websh-web/`**: keep at root for now; revisit if the workspace ever grows a second wasm crate.
- **`web-sys` features per crate**: derive both lists at migration time from `cargo check` errors. Core gets a narrow set; web gets the broad current set.
- **`leptos-use` features**: currently `["use_media_query"]`; whatever else the migration surfaces.
- **GitHub backend in core vs CLI**: today both use GitHub differently (HTTP via gloo-net for browser, `gh` subprocess for CLI). They stay separate. If they ever need to converge, that's a separate refactor.
- **Test crate organization**: integration tests under `crates/<crate>/tests/`; some currently-flat test files may be more naturally `#[cfg(test)] mod tests` blocks inside their owning module after the move.
- **Naming nits**: `engine/` vs `engines/`, `pages/` vs `views/`, `platform/` vs `wasm/` — pick during migration based on what reads better in surrounding context.
- **Browser-side `gh` token entry UX changes** that fall out of `websh-core::attestation::verify` going live: out of scope for migration.

---

## Sources

- [Leptos book: Components and Props](https://book.leptos.dev/view/03_components.html)
- [Leptos book: Passing Children to Components (slots, render props)](https://book.leptos.dev/view/09_component_children.html)
- [Leptos book: Life Cycle of a Signal](https://book.leptos.dev/appendix_life_cycle.html)
- [Leptos book: Interlude — Styling](https://book.leptos.dev/interlude_styling.html)
- [leptos-rs/start-axum-workspace (workspace template)](https://github.com/leptos-rs/start-axum-workspace)
- [cargo-leptos workspace docs](https://github.com/leptos-rs/cargo-leptos)
- [stylance-rs README (workspace config)](https://github.com/basro/stylance-rs)
- [trunk-rs: Trunk.toml hooks](https://trunkrs.dev/configuration/)
- [trunk-rs Cargo.toml location in workspaces](https://github.com/trunk-rs/trunk/blob/main/Trunk.toml)
- [rPGP repo (wasm support, PLATFORMS.md)](https://github.com/rpgp/rpgp)
- [Master Hexagonal Architecture in Rust (howtocodeit)](https://www.howtocodeit.com/guides/master-hexagonal-architecture-in-rust)
- [Rust Cargo workspaces (the Rust Book ch.14)](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)

---

## 11. Deviation log

When execution diverges from sections 1-10, the change is recorded both here (cross-reference) and in [deviation-log.md](./deviation-log.md) (append-only timeline). Material deviations get an ADR in [adrs/](./adrs/).

(no deviations yet)
