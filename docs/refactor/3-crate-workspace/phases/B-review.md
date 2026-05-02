# Phase B Consensus Review

Three reviewers ran in parallel on the Phase B design (`B-websh-core.md`). Findings consolidated below; resolutions are folded into the updated design doc and the canonical task list (`B-tasks.md`).

## Reviewer agreement matrix

| Finding | Architecture | Rust/Leptos | Conventions/Quality |
|---|---|---|---|
| `AppError` is forbidden global-error anti-pattern; must delete | × | × | × |
| `thiserror` conversion in scope for B (not deferred) | — | × | × |
| `state: &TerminalState` parameter cleanup must be explicit | × | × | — |
| `dom.rs` strategy: internally cfg-gated, public surface portable | × | × | — |
| Re-export shim too sketchy; enumerated table needed | × | — | — |
| Missing modules in target layout (error/admin/env/wallet/etc.) | × | — | — |
| B5/B6 sequencing impossible (filesystem deps storage::backend) | × | — | — |
| Filesystem family tests blow 800-line ceiling | — | — | × |
| Visibility audit needed before wrap-up | — | — | × |
| Comment audit at move time | — | — | × |
| Family-specific helpers misplaced in `execute/mod.rs` | — | × | — |
| `time.rs` NOT wholesale cfg-gated | — | × | — |
| `theme.rs` split: pure to core, apply/init stay in web | × | — | — |
| `tests/crypto_homepage.rs` unassigned | × | — | — |
| `#[allow(dead_code)]` audit at move time | — | × | × |
| `test_support` `#[cfg(test)]` per-crate issue | × | — | — |
| Cfg-gate at parent `mod` declaration | × | × | — |
| `changes.rs` belongs in `domain/`, not `filesystem/` | × | — | — |
| Pre-existing 800+ files (global_fs, commands/mod, autocomplete, ack, ledger) | — | — | × |
| `misc.rs` rename to `info.rs` + `listing.rs` | — | × | — |

`×` = flagged. `—` = lens not in scope or not raised.

## Consolidation decisions

### CRITICAL — folded into the design doc as resolutions

1. **`AppError` is deleted in B0.d.** All callers re-typed to per-domain errors or `String` per `conventions.md`.
2. **`thiserror` conversions are part of each error type's move commit.** `EnvironmentError`, `FetchError`, `WalletError`, `UrlValidationError` get rewritten in B1; `StorageError` in B5a (with the port). No deferred sweep.
3. **B0.a is broadened.** Drop `state: &TerminalState` from `execute_command`, `execute_pipeline`, every test fixture; replace `Command::Clear` body with `SideEffect::ClearHistory`. Caller in `components/terminal/` applies the effect.
4. **`dom.rs` is internally cfg-gated.** The module compiles on both targets; wasm-touching function bodies guard with `#[cfg(target_arch="wasm32")]` and provide native no-ops (extends the existing `dom::window()` pattern). `routing.rs` and `runtime/state.rs` call `dom::*` unconditionally.
5. **B5/B6 split.** B5a = `storage::{backend, error}.rs` (port + records, no engine dep). B5b = `filesystem/` (engine + merge). B6 = storage adapters (`github/`, `idb.rs`, `persist.rs`, `mock.rs`) + `storage::boot.rs`.
6. **`changes.rs` lands in `domain/`** in B2, not in `filesystem/` in B5. Pure data; many storage modules consume `ChangeSet` independently of the engine.
7. **Re-export shim is rewritten as an enumerated table.** Captures every cross-module path the legacy crate walks. See updated design doc § Re-export shim.
8. **Missing modules added to target layout**: `error.rs` (core top-level), `admin.rs`, `runtime/env.rs`, `runtime/wallet.rs`, `storage/error.rs`, `storage/boot.rs`, `shell/filters.rs`. The `#[path = "../env.rs"]` and `#[path = "../wallet.rs"]` aliases are unwound during the move.
9. **`theme.rs` split (B0.f)**: pure helpers (`THEMES`, `ThemeDescriptor`, `theme_ids`, `theme_label`, `normalize_theme_id`, `DEFAULT_THEME`, `STORAGE_KEY`) move to `websh-core::theme`. `apply_theme` and `initial_theme` stay in legacy `utils/theme.rs` and migrate to `websh-web::platform` in Phase D.
10. **`node_metadata::test_support` loses its `#[cfg(test)]` gate.** Keeps `#[allow(dead_code)]`. Per-crate `cfg(test)` doesn't propagate via shim, so the gate must drop.
11. **Filesystem-family test split.** `shell/execute/filesystem.rs` would be ~1370 lines (production + filesystem-family tests) — over the 800 ceiling. Resolution: split production into `read.rs` (`ls/cd/cat`) and `write.rs` (`touch/mkdir/rm/rmdir/edit/echo_redirect`); each carries its own tests.
12. **Family-specific helpers pushed down from `execute/mod.rs`**: `change_tag` → `sync.rs`; `is_pending_create` + `blank_file_meta` + `blank_dir_meta` → `write.rs` (post-split); `can_write_path` → `listing.rs` (post-rename, see #14). Remaining shared in `mod.rs`: `resolve_path_arg`, `require_write_access`, `mount_for_path`, `is_synthetic_runtime_state_path`, `execute_command`, `execute_pipeline`.
13. **`tests/crypto_homepage.rs` moves to `crates/websh-core/tests/`** in B9.

### HIGH — folded as resolutions

14. **`misc.rs` renamed**: `info.rs` (id, whoami, theme, login, logout, clear) + `listing.rs` (ls, cd, explorer, format_ls_output). Cleaner family boundaries.
15. **B0.e — comment audit**. Scrub banner-block separators, restate-the-code, and development-history comments from in-scope files (specifically `ring_buffer.rs`, `execute.rs::execute_id`, `runtime/loader.rs:253-258`, `commands/execute.rs:432, 1322, 2126`) before the move. Move-time keeps the diff clean.
16. **B-tasks.md adds visibility audit (B9.5)**. Every `pub` in moving `mod.rs` files audited; downgrade to `pub(crate)` what isn't consumed across crates.
17. **`#[allow(dead_code)]` audit at each move commit**. Items that aren't actually dead (StorageBackend trait, CommitOutcome, GitHubBackend, etc.) lose the annotation; visibility tightens instead.
18. **`time.rs`, `asset.rs`, `sysinfo.rs` retain internal cfg-arms**. Module declarations stay non-gated. The current shape is portable; the design doc's "cfg-gated" labels are corrected to "internally cfg-gated."
19. **Cfg-gate at parent `mod` declaration** for genuinely wasm-only modules: `storage/{github,idb,persist}` gated in `storage/mod.rs`. `dom.rs` is NOT gated (per #4). `sysinfo.rs` is gated (only consumer is the cfg-gated `execute_id`).

### MEDIUM — addressed at appropriate points

20. **Web-sys feature additions** for core: `Event`, `History`, `Location` (consumed by `dom.rs`, `routing.rs`, `state.rs`). Updated in `crates/websh-core/Cargo.toml`.
21. **Pre-existing 800+ line files** (`global_fs.rs:1080`, `commands/mod.rs:962`, `autocomplete.rs:831`, `ack.rs:811`, `ledger.rs:802`): logged in `deviation-log.md` as inherited-pre-existing; not split in Phase B (out of scope; the migration's `engine/execute.rs` split is the only file-size work in B). A follow-up issue tracks the eventual split.
22. **Phase D explicitly removes the `src/lib.rs` re-export shim**. Captured in this review and in `B-websh-core.md § Out of scope`; will be reflected in `D-websh-web.md` when Phase D is designed.

### LOW — defer or skip

23. **`misc.rs` rename** — done above as part of HIGH 14.
24. **6-line `categories.rs`** — fine as-is; the `mempool/` folder shape is justified by aggregate size.
25. **`pub mod manifest` motivation** — the design doc keeps the explicit `pub mod` (preserves module shape under the shim) over a flat `pub use`.
26. **`data-bin` for cdylib** — Phase D's problem; not Phase B.
27. **Prophylactic "no B0..B9 in commits" note** — covered by `conventions.md`; no need to repeat in design doc.

## Net effect on Phase B

The design doc is rewritten with these resolutions folded in. The task plan in `B-tasks.md` adds B0.d (delete `AppError`), B0.e (comment audit), B0.f (theme split), B5a (port-only), B9.5 (visibility audit), and breaks B8's `execute/` split into `read.rs`/`write.rs`/`listing.rs`/`info.rs`/`sync.rs`/`env.rs` (was `filesystem`/`sync`/`env`/`listing`/`misc`). New target layout under `crates/websh-core/src/` adds `error.rs`, `admin.rs`, `theme.rs`, `runtime/{env,wallet}.rs`, `storage/{error,boot}.rs`, `shell/filters.rs`.

The deviation log gets an updated entry capturing decisions #4, #6, #9, #10, #18, #19 (the deviations from `architecture.md`'s prescriptions).

ADRs queued for Phase B wrap-up: ADR-0002 covering placement decisions (subject, dom, theme, error type per-domain shape), ADR-0003 covering the `execute/` family split.
