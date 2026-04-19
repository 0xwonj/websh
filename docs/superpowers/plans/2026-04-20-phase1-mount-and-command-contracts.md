# Phase 1: Mount Registry & CommandResult Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish two core contracts that the rest of the Phase 2 refactors depend on: (1) a cached, non-empty `MountRegistry` global that eliminates the `configured_mounts()` hot path, and (2) a `CommandResult` with `SideEffect` enum + `exit_code` that absorbs login/logout/explorer special-casing from `terminal.rs`.

**Architecture:** Introduce `config::mounts() -> &'static MountRegistry` initialized once via `OnceLock`, exposing `home()` / `resolve()` / `all()` accessors. Reshape `CommandResult` to carry an optional `SideEffect` variant (`Navigate`, `Login`, `Logout`, `SwitchView`, `SwitchViewAndNavigate`) plus `exit_code: i32`. `execute_command` returns those side effects natively; the UI layer (`terminal.rs`) dispatches via a single match, removing string-match special cases.

**Tech Stack:** Rust 2024 edition, Leptos 0.8 (reactive UI), wasm32-unknown-unknown target, Trunk, Stylance CSS modules.

**Addresses issues:** C1 (configured_mounts hot path), H6 (mount NonEmpty invariant), H3 (CommandResult SideEffect), H7 (exit_code).

---

## Scope

This plan covers exactly four issues from the review. Other Phase 2/3/4 work (parser, filters, UI accessibility, etc.) is deliberately out of scope — they will be executed as parallel tracks on top of this baseline.

Out of scope (deferred to later phases):
- Grep regex (H5), head/tail stricter parsing (M6), pipe iterator streaming (H4)
- File/dir resolve (H2, M8)
- VirtualFs clone removal (H1)
- UI fixes (H9, H10, H11, M1, M2, M3, M9, M10, M11)
- Error type unification (M4)
- All LOW severity items
- All `⏸️ DEFERRED` (crypto, deployment, CSP)

---

## File Structure

Files to modify/create in Phase 1:

| Path | Responsibility | Action |
|---|---|---|
| `src/models/mount.rs` | `MountRegistry` methods (`home`, `resolve`) + non-empty invariant | Modify |
| `src/config.rs` | `mounts()` global accessor; remove/slim `configured_mounts()` | Modify |
| `src/models/route.rs` | Use `mounts()` instead of `configured_mounts()` | Modify |
| `src/core/commands/result.rs` | `SideEffect` enum + reshape `CommandResult` with `exit_code` | Modify |
| `src/core/commands/mod.rs` | Pipeline exit_code propagation | Modify |
| `src/core/commands/execute.rs` | Native side-effect emission for Login/Logout/Explorer; use `mounts()` | Modify |
| `src/core/commands/filters.rs` | `apply_filter` returns `CommandResult`; exit_code per filter | Modify |
| `src/components/terminal/terminal.rs` | Unified `dispatch_side_effect`; remove special-case strings | Modify |
| `src/components/explorer/file_list.rs` | Use `mounts()` | Modify |
| `src/components/explorer/preview/content.rs` | Use `mounts()` | Modify |
| `src/components/explorer/preview/hook.rs` | Use `mounts()` | Modify |
| `src/components/explorer/preview/sheet.rs` | Use `mounts()` | Modify |
| `src/app.rs` | Initialize via `mounts()` instead of `configured_mounts()` | Modify |

No new files are created; the refactor is in-place.

---

## Section 1: Mount Registry Refactor (C1 + H6)

### Task 1.1: Add non-empty invariant + accessors to `MountRegistry`

**Files:**
- Modify: `src/models/mount.rs`

- [ ] **Step 1: Write the failing tests**

Add at the end of the `tests` module in `src/models/mount.rs`:

```rust
    #[test]
    fn test_registry_home() {
        let mounts = vec![
            Mount::github("~", "https://example.com"),
            Mount::ipfs("data", "QmXyz"),
        ];
        let registry = MountRegistry::from_mounts(mounts);
        assert_eq!(registry.home().alias(), "~");
    }

    #[test]
    fn test_registry_resolve() {
        let mounts = vec![
            Mount::github("~", "https://example.com"),
            Mount::ipfs("data", "QmXyz"),
        ];
        let registry = MountRegistry::from_mounts(mounts);
        assert_eq!(registry.resolve("~").map(|m| m.alias()), Some("~"));
        assert_eq!(registry.resolve("data").map(|m| m.alias()), Some("data"));
        assert!(registry.resolve("unknown").is_none());
    }

    #[test]
    #[should_panic(expected = "at least one mount")]
    fn test_registry_from_empty_panics() {
        let _ = MountRegistry::from_mounts(vec![]);
    }
```

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test --lib models::mount::tests
```

Expected: 3 new tests fail (compile error: `home` / `resolve` not found; `from_mounts` doesn't panic).

- [ ] **Step 3: Implement the accessors and non-empty invariant**

In `src/models/mount.rs`, change `from_mounts` and add `home` / `resolve`:

```rust
impl MountRegistry {
    pub fn new() -> Self {
        Self {
            mounts: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Create a registry from a list of mounts.
    ///
    /// # Panics
    /// Panics if the input list is empty. The registry's `home()` method
    /// relies on the non-empty invariant.
    pub fn from_mounts(mounts: Vec<Mount>) -> Self {
        assert!(
            !mounts.is_empty(),
            "MountRegistry requires at least one mount",
        );
        let mut registry = Self::new();
        for mount in mounts {
            registry.register(mount);
        }
        registry
    }

    fn register(&mut self, mount: Mount) {
        let alias = mount.alias().to_string();
        if !self.mounts.contains_key(&alias) {
            self.order.push(alias.clone());
        }
        self.mounts.insert(alias, mount);
    }

    /// Get the home mount (first registered).
    ///
    /// Infallible: `from_mounts` guarantees at least one mount.
    pub fn home(&self) -> &Mount {
        self.order
            .first()
            .and_then(|alias| self.mounts.get(alias))
            .expect("MountRegistry invariant: non-empty")
    }

    /// Resolve an alias to a mount.
    pub fn resolve(&self, alias: &str) -> Option<&Mount> {
        self.mounts.get(alias)
    }

    pub fn all(&self) -> impl Iterator<Item = &Mount> {
        self.order.iter().filter_map(|alias| self.mounts.get(alias))
    }
}
```

- [ ] **Step 4: Run tests to verify pass**

```bash
cargo test --lib models::mount::tests
```

Expected: all 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/models/mount.rs
git commit -m "feat(mount): add home/resolve accessors and non-empty invariant"
```

---

### Task 1.2: Add `config::mounts()` global accessor

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write the failing test**

Add to the bottom of `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mounts_is_singleton() {
        let a = mounts() as *const _;
        let b = mounts() as *const _;
        assert_eq!(a, b, "mounts() must return the same static reference");
    }

    #[test]
    fn test_mounts_has_home() {
        let home = mounts().home();
        assert_eq!(home.alias(), "~");
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test --lib config::tests
```

Expected: compile error — `mounts` not defined.

- [ ] **Step 3: Add `mounts()` alongside existing helpers**

Replace the `Mount Configuration` section at the bottom of `src/config.rs` with:

```rust
// =============================================================================
// Mount Configuration
// =============================================================================

use crate::models::{Mount, MountRegistry};
use std::sync::OnceLock;

/// Static mount list. Private — external callers use `mounts()`.
fn mount_list() -> Vec<Mount> {
    vec![Mount::github_with_prefix(
        "~",
        "https://raw.githubusercontent.com/0xwonj/db/main",
        "~",
    )]
}

/// Get the global mount registry.
///
/// Initialized once on first access. The registry is guaranteed to be
/// non-empty (see `MountRegistry::from_mounts`).
pub fn mounts() -> &'static MountRegistry {
    static REGISTRY: OnceLock<MountRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| MountRegistry::from_mounts(mount_list()))
}

// --- Backwards-compat shims (to be removed in Task 1.7) ---

/// DEPRECATED: use `mounts().all().cloned().collect()`.
#[doc(hidden)]
pub fn configured_mounts() -> Vec<Mount> {
    mounts().all().cloned().collect()
}

/// DEPRECATED: use `mounts().home().clone()`.
#[doc(hidden)]
pub fn default_mount() -> Mount {
    mounts().home().clone()
}

/// DEPRECATED: use `mounts().home().content_base_url()`.
#[doc(hidden)]
pub fn default_base_url() -> String {
    mounts().home().content_base_url()
}
```

The `#[doc(hidden)]` shims let callers keep compiling. They are removed in Task 1.7 after every call site has been migrated.

- [ ] **Step 4: Run tests to verify pass**

```bash
cargo build
cargo test --lib config::tests
```

Expected: clean build, both `mounts()` tests pass, existing callers still work via shims.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add mounts() global with deprecated shims"
```

---

### Task 1.3: Update `AppRoute` to use `mounts()`

**Files:**
- Modify: `src/models/route.rs`

- [ ] **Step 1: Verify existing tests still pass after refactor**

The existing tests in `src/models/route.rs::tests` already cover `from_path`, `to_path`, `parent`, etc. No new tests needed — we are preserving behavior.

- [ ] **Step 2: Replace `configured_mounts` usage**

In `src/models/route.rs`:

Change the import at the top:

```rust
use super::mount::Mount;
use crate::config::mounts;
use crate::utils::dom;
```

Replace the helper functions at the bottom (around lines 353–364):

```rust
/// Get the home mount from the global registry.
fn home_mount() -> Mount {
    mounts().home().clone()
}

/// Resolve an alias to a mount via the global registry.
fn resolve_mount(alias: &str) -> Option<Mount> {
    mounts().resolve(alias).cloned()
}
```

- [ ] **Step 3: Run route tests**

```bash
cargo test --lib models::route::tests
```

Expected: all existing route tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/models/route.rs
git commit -m "refactor(route): use mounts() instead of configured_mounts()"
```

---

### Task 1.4: Update `core/commands/execute.rs`

**Files:**
- Modify: `src/core/commands/execute.rs`

- [ ] **Step 1: Replace the import**

Change the import at the top of `src/core/commands/execute.rs`:

```rust
use crate::config::{ASCII_PROFILE, HELP_TEXT, PROFILE_FILE, mounts};
```

- [ ] **Step 2: Rewrite `list_mounts`**

Replace the body of `list_mounts` (around lines 164–194) with:

```rust
fn list_mounts(long: bool) -> CommandResult {
    let registry = mounts();

    let output: Vec<OutputLine> = if long {
        registry
            .all()
            .map(|mount| {
                let perms = crate::models::DisplayPermissions {
                    is_dir: true,
                    read: true,
                    write: false,
                    execute: true,
                };
                let entry = crate::core::DirEntry {
                    name: mount.alias().to_string(),
                    is_dir: true,
                    title: mount.description(),
                    file_meta: None,
                };
                OutputLine::long_entry(&entry, &perms)
            })
            .collect()
    } else {
        registry
            .all()
            .map(|mount| OutputLine::dir_entry(mount.alias(), mount.description()))
            .collect()
    };

    CommandResult::output(output)
}
```

- [ ] **Step 3: Rewrite `resolve_mount_alias`**

Replace (around lines 196–199):

```rust
fn resolve_mount_alias(alias: &str) -> Option<Mount> {
    mounts().resolve(alias).cloned()
}
```

- [ ] **Step 4: Remove `.expect` calls from `execute_cd` and `execute_cat`**

In `execute_cd` (around lines 241–246), replace:

```rust
    let current_mount = current_route.mount().cloned().unwrap_or_else(|| {
        configured_mounts()
            .into_iter()
            .next()
            .expect("At least one mount must be configured")
    });
```

with:

```rust
    let current_mount = current_route
        .mount()
        .cloned()
        .unwrap_or_else(|| mounts().home().clone());
```

Apply the identical replacement in `execute_cat` (around lines 279–284).

- [ ] **Step 5: Build and run tests**

```bash
cargo build
cargo test --lib core::commands
```

Expected: builds clean; command tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/core/commands/execute.rs
git commit -m "refactor(commands): use mounts() and remove expect panics"
```

---

### Task 1.5: Update component callers

**Files:**
- Modify: `src/components/terminal/terminal.rs`
- Modify: `src/components/explorer/file_list.rs`
- Modify: `src/components/explorer/preview/content.rs`
- Modify: `src/components/explorer/preview/hook.rs`
- Modify: `src/components/explorer/preview/sheet.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Fix `src/components/terminal/terminal.rs`**

Replace the `fs_path_to_browse_route` helper (around lines 251–262) with:

```rust
/// Convert a filesystem path (relative) to a Browse route.
fn fs_path_to_browse_route(fs_path: &str) -> AppRoute {
    AppRoute::Browse {
        mount: crate::config::mounts().home().clone(),
        path: fs_path.to_string(),
    }
}
```

Note: This helper is removed entirely in Task 2.5 once `execute_explorer` absorbs it. Keep it for now.

- [ ] **Step 2: Fix `src/components/explorer/file_list.rs`**

Change the import (line 15):

```rust
use crate::config::mounts;
```

Replace `mounts_to_entries` (around lines 38–48) with:

```rust
fn mounts_to_entries() -> Vec<DirEntry> {
    mounts()
        .all()
        .map(|mount| DirEntry {
            name: mount.alias().to_string(),
            is_dir: true,
            title: mount.description(),
            file_meta: None,
        })
        .collect()
}
```

Replace the Root-navigation block (around lines 165–176) with:

```rust
            if matches!(route, AppRoute::Root)
                && let Some(mount) = mounts().resolve(&entry_name_for_nav).cloned()
            {
                AppRoute::Browse {
                    mount,
                    path: String::new(),
                }
                .push();
                return;
            }
```

Replace the `default_mount` fallback (around line 186):

```rust
            let mount = route
                .mount()
                .cloned()
                .unwrap_or_else(|| mounts().home().clone());
```

- [ ] **Step 3: Fix `src/components/explorer/preview/content.rs` (line 291)**

Open the file and locate the `default_mount` call. Replace:

```rust
            .unwrap_or_else(crate::config::default_mount);
```

with:

```rust
            .unwrap_or_else(|| crate::config::mounts().home().clone());
```

- [ ] **Step 4: Fix `src/components/explorer/preview/hook.rs` (line 187)**

Replace:

```rust
            .unwrap_or_else(crate::config::default_base_url)
```

with:

```rust
            .unwrap_or_else(|| crate::config::mounts().home().content_base_url())
```

- [ ] **Step 5: Fix `src/components/explorer/preview/sheet.rs` (line 72)**

Replace:

```rust
            .unwrap_or_else(crate::config::default_mount);
```

with:

```rust
            .unwrap_or_else(|| crate::config::mounts().home().clone());
```

- [ ] **Step 6: Fix `src/app.rs`**

Replace the `AppContext::new` body (lines 257–273) at the point it uses `configured_mounts()`. Change:

```rust
    pub fn new() -> Self {
        use crate::config::configured_mounts;

        Self {
            mounts: StoredValue::new(MountRegistry::from_mounts(configured_mounts())),
```

to:

```rust
    pub fn new() -> Self {
        Self {
            mounts: StoredValue::new(crate::config::mounts().clone()),
```

Note: `MountRegistry` derives `Clone`, so this is O(N) once on startup. The `StoredValue` still wraps a single shared value reactively.

- [ ] **Step 7: Build and full test**

```bash
cargo build
cargo test --lib
```

Expected: clean build, all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/components/terminal/terminal.rs \
        src/components/explorer/file_list.rs \
        src/components/explorer/preview/content.rs \
        src/components/explorer/preview/hook.rs \
        src/components/explorer/preview/sheet.rs \
        src/app.rs
git commit -m "refactor(ui): use mounts() in components and app context"
```

---

### Task 1.6: Verify Phase 1 Section 1 in-browser

**Files:**
- (No code changes — manual verification)

- [ ] **Step 1: Start dev server**

```bash
trunk serve
```

Wait for the build to complete (~30s incremental, ~2–5min cold).

- [ ] **Step 2: Smoke-test key paths**

Open `http://127.0.0.1:8080` and verify:
- Root (`#/`) shows mount list via `ls`
- Navigating `#/~/` shows mount contents
- `cd blog/` changes path
- `cat .profile` works
- Clicking a mount in Explorer navigates correctly

Expected: All behaviors identical to pre-refactor. If any broken, investigate the relevant caller from Task 1.5.

- [ ] **Step 3: Commit the worktree tag (optional checkpoint)**

```bash
git tag phase1-section1-complete
```

---

### Task 1.7: Remove deprecated shims

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Verify no remaining callers**

```bash
grep -rn "configured_mounts\|default_mount\|default_base_url" src/
```

Expected output: only the `pub fn` definitions in `src/config.rs` remain. All call sites are gone.

- [ ] **Step 2: Delete the shims**

In `src/config.rs`, remove the three `#[doc(hidden)]` functions (`configured_mounts`, `default_mount`, `default_base_url`) added in Task 1.2.

- [ ] **Step 3: Build and test**

```bash
cargo build
cargo test --lib
```

Expected: clean build, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs
git commit -m "refactor(config): remove deprecated mount shims"
```

---

## Section 2: CommandResult + SideEffect (H3 + H7)

### Task 2.1: Introduce `SideEffect` enum and reshape `CommandResult`

**Files:**
- Modify: `src/core/commands/result.rs`

- [ ] **Step 1: Write failing tests**

Replace the content of `src/core/commands/result.rs` with an initial test scaffold at the bottom. First, add tests that will fail until constructors exist:

```rust
// Tests (to be added at the end of result.rs)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Mount, OutputLine, ViewMode};

    fn test_mount() -> Mount {
        Mount::github("~", "https://example.com")
    }

    #[test]
    fn test_output_constructor() {
        let r = CommandResult::output(vec![OutputLine::text("hi")]);
        assert_eq!(r.exit_code, 0);
        assert!(r.side_effect.is_none());
        assert_eq!(r.output.len(), 1);
    }

    #[test]
    fn test_error_line_constructor() {
        let r = CommandResult::error_line("boom");
        assert_eq!(r.exit_code, 1);
        assert!(r.side_effect.is_none());
        assert_eq!(r.output.len(), 1);
    }

    #[test]
    fn test_navigate_constructor() {
        let route = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        let r = CommandResult::navigate(route.clone());
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.side_effect, Some(SideEffect::Navigate(route)));
    }

    #[test]
    fn test_login_constructor() {
        let r = CommandResult::login();
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.side_effect, Some(SideEffect::Login));
    }

    #[test]
    fn test_logout_constructor() {
        let r = CommandResult::logout();
        assert_eq!(r.side_effect, Some(SideEffect::Logout));
    }

    #[test]
    fn test_switch_view_constructor() {
        let r = CommandResult::switch_view(ViewMode::Explorer);
        assert_eq!(r.side_effect, Some(SideEffect::SwitchView(ViewMode::Explorer)));
    }

    #[test]
    fn test_open_explorer_constructor() {
        let route = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        let r = CommandResult::open_explorer(route.clone());
        assert_eq!(
            r.side_effect,
            Some(SideEffect::SwitchViewAndNavigate(ViewMode::Explorer, route))
        );
    }

    #[test]
    fn test_with_exit_code() {
        let r = CommandResult::empty().with_exit_code(127);
        assert_eq!(r.exit_code, 127);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test --lib core::commands::result::tests
```

Expected: compile error (types and constructors missing).

- [ ] **Step 3: Rewrite `result.rs` with the new shape**

Replace the non-test portion of `src/core/commands/result.rs` with:

```rust
//! Command execution result type.

use crate::models::{AppRoute, OutputLine, ViewMode};

/// Side effect requested by a command's execution.
///
/// Commands return side effects as data; the UI layer (or executor) is
/// responsible for actually performing them. This keeps command logic
/// testable without Leptos signals or async runtimes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideEffect {
    /// Navigate to a new route.
    Navigate(AppRoute),
    /// Initiate wallet login (async).
    Login,
    /// Perform wallet logout.
    Logout,
    /// Switch view mode (e.g., Terminal ↔ Explorer).
    SwitchView(ViewMode),
    /// Switch view mode and navigate in one step.
    SwitchViewAndNavigate(ViewMode, AppRoute),
}

/// Result of executing a command.
///
/// Carries output lines, a POSIX-style exit code, and an optional side
/// effect (navigation, wallet action, view switch).
#[derive(Clone, Debug)]
pub struct CommandResult {
    /// Output lines to display.
    pub output: Vec<OutputLine>,
    /// POSIX exit code. 0 = success, non-zero = error.
    pub exit_code: i32,
    /// Side effect to perform after display (if any).
    pub side_effect: Option<SideEffect>,
}

impl CommandResult {
    // --- Primary constructors ---

    /// Success with output, no side effect.
    pub fn output(lines: Vec<OutputLine>) -> Self {
        Self {
            output: lines,
            exit_code: 0,
            side_effect: None,
        }
    }

    /// Error output with exit_code=1.
    pub fn error_line(message: impl Into<String>) -> Self {
        Self {
            output: vec![OutputLine::error(message.into())],
            exit_code: 1,
            side_effect: None,
        }
    }

    /// Success, no output, no side effect.
    pub fn empty() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: None,
        }
    }

    // --- Side-effect constructors ---

    pub fn navigate(route: AppRoute) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::Navigate(route)),
        }
    }

    pub fn login() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::Login),
        }
    }

    pub fn logout() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::Logout),
        }
    }

    pub fn switch_view(mode: ViewMode) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::SwitchView(mode)),
        }
    }

    pub fn open_explorer(route: AppRoute) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::SwitchViewAndNavigate(
                ViewMode::Explorer,
                route,
            )),
        }
    }

    // --- Builder methods ---

    /// Override the exit code (chainable).
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }
}
```

- [ ] **Step 4: Update `src/core/commands/mod.rs` re-exports**

In `src/core/commands/mod.rs`, replace the re-export line (line 20):

```rust
pub use result::{CommandResult, SideEffect};
```

- [ ] **Step 5: Run tests to verify pass**

```bash
cargo test --lib core::commands::result::tests
```

Expected: all 8 constructor tests pass. The rest of the project may not build yet — that is fixed in Tasks 2.2–2.4.

- [ ] **Step 6: Commit**

```bash
git add src/core/commands/result.rs src/core/commands/mod.rs
git commit -m "feat(commands): add SideEffect enum and exit_code to CommandResult"
```

---

### Task 2.2: Update `execute_command` to emit side effects

**Files:**
- Modify: `src/core/commands/execute.rs`

- [ ] **Step 1: Write failing tests**

Add at the end of `src/core/commands/execute.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TerminalState;
    use crate::core::VirtualFs;
    use crate::models::{AppRoute, ViewMode, WalletState};

    fn empty_state() -> (TerminalState, WalletState, VirtualFs) {
        (
            TerminalState::new(),
            WalletState::Disconnected,
            VirtualFs::empty(),
        )
    }

    #[test]
    fn test_login_returns_login_side_effect() {
        let (ts, ws, fs) = empty_state();
        let result = execute_command(Command::Login, &ts, &ws, &fs, &AppRoute::Root);
        assert_eq!(result.side_effect, Some(SideEffect::Login));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_logout_returns_logout_side_effect() {
        let (ts, ws, fs) = empty_state();
        let result = execute_command(Command::Logout, &ts, &ws, &fs, &AppRoute::Root);
        assert_eq!(result.side_effect, Some(SideEffect::Logout));
    }

    #[test]
    fn test_explorer_no_arg_switches_view() {
        let (ts, ws, fs) = empty_state();
        let result = execute_command(
            Command::Explorer(None),
            &ts,
            &ws,
            &fs,
            &AppRoute::Root,
        );
        assert_eq!(
            result.side_effect,
            Some(SideEffect::SwitchView(ViewMode::Explorer))
        );
    }

    #[test]
    fn test_unknown_command_exit_127() {
        let (ts, ws, fs) = empty_state();
        let result = execute_command(
            Command::Unknown("foobar".into()),
            &ts,
            &ws,
            &fs,
            &AppRoute::Root,
        );
        assert_eq!(result.exit_code, 127);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test --lib core::commands::execute::tests
```

Expected: tests fail — `Command::Login` etc. currently return `CommandResult::empty()` (no side_effect) and Unknown has exit_code=0.

- [ ] **Step 3: Update the import and main match**

In `src/core/commands/execute.rs`, change the import line (line 12):

```rust
use super::{Command, CommandResult, SideEffect};
```

Replace the main `match cmd` block in `execute_command` (around lines 36–59):

```rust
    match cmd {
        Command::Ls { path, long } => execute_ls(path, long, wallet_state, fs, current_route),
        Command::Cd(path) => execute_cd(path, fs, current_route),
        Command::Pwd => CommandResult::output(vec![OutputLine::text(current_route.display_path())]),
        Command::Cat(file) => execute_cat(file, fs, current_path, current_route),
        Command::Whoami => {
            CommandResult::output(vec![OutputLine::ascii(ASCII_PROFILE.to_string())])
        }
        Command::Id => execute_id(wallet_state),
        Command::Help => CommandResult::output(HELP_TEXT.lines().map(OutputLine::text).collect()),
        Command::Clear => {
            state.clear_history();
            CommandResult::empty()
        }
        Command::Echo(text) => CommandResult::output(vec![OutputLine::text(text)]),
        Command::Export(arg) => execute_export(arg),
        Command::Unset(key) => execute_unset(key),
        Command::Login => CommandResult::login(),
        Command::Logout => CommandResult::logout(),
        Command::Explorer(path) => execute_explorer(path, fs, current_route),
        Command::Unknown(cmd) => CommandResult::error_line(format!(
            "Command not found: {}. Type 'help' for available commands.",
            cmd
        ))
        .with_exit_code(127),
    }
```

- [ ] **Step 4: Add `execute_explorer` helper**

Append to `src/core/commands/execute.rs` (before the `tests` module):

```rust
/// Execute `explorer` command.
fn execute_explorer(
    path: Option<super::PathArg>,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    use crate::models::ViewMode;

    let Some(path_arg) = path else {
        return CommandResult::switch_view(ViewMode::Explorer);
    };

    let current_path = current_route.fs_path();
    match fs.resolve_path(current_path, path_arg.as_str()) {
        Some(new_path) if fs.is_directory(&new_path) => {
            let mount = current_route
                .mount()
                .cloned()
                .unwrap_or_else(|| mounts().home().clone());
            CommandResult::open_explorer(AppRoute::Browse {
                mount,
                path: new_path,
            })
        }
        Some(_) => CommandResult::error_line(format!(
            "explorer: not a directory: {}",
            path_arg
        )),
        None => CommandResult::error_line(format!(
            "explorer: no such file or directory: {}",
            path_arg
        )),
    }
}
```

- [ ] **Step 5: Update `execute_cd` and `execute_cat` error branches for exit_code**

In `execute_cd` (around lines 232–236), change:

```rust
        return CommandResult::output(vec![OutputLine::error(format!(
            "cd: no such file or directory: {}",
            target
        ))]);
```

to:

```rust
        return CommandResult::error_line(format!(
            "cd: no such file or directory: {}",
            target
        ));
```

Apply the same pattern to the other two error branches in `execute_cd` (lines 253–260) and the error branches in `execute_cat` (around 291–320). Use `CommandResult::error_line(...)` uniformly so all error paths set `exit_code = 1`.

- [ ] **Step 6: Run tests**

```bash
cargo test --lib core::commands
```

Expected: all tests pass. If a test asserts on the old `navigate_to` field or missing fields, update the assertion.

- [ ] **Step 7: Commit**

```bash
git add src/core/commands/execute.rs
git commit -m "feat(commands): emit SideEffect for login/logout/explorer, exit codes for errors"
```

---

### Task 2.3: Update `apply_filter` to return `CommandResult`

**Files:**
- Modify: `src/core/commands/filters.rs`

- [ ] **Step 1: Update existing tests to the new shape**

In `src/core/commands/filters.rs::tests`, every `apply_filter(...)` call currently returns `Vec<OutputLine>`. Change the assertions to unwrap `.output` or match on `.exit_code`. Example change:

Old (line 107):
```rust
        let result = apply_filter("grep", &args(&["an"]), lines);
        assert_eq!(result.len(), 1);
```

New:
```rust
        let result = apply_filter("grep", &args(&["an"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
```

Apply this pattern to every existing test in the module. Then add two new tests at the bottom of the `tests` module:

```rust
    #[test]
    fn test_grep_no_match_exit_1() {
        let lines = test_lines();
        let result = apply_filter("grep", &args(&["xyzzy"]), lines);
        assert_eq!(result.exit_code, 1);
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_grep_missing_pattern_exit_2() {
        let lines = test_lines();
        let result = apply_filter("grep", &[], lines);
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_unknown_filter_exit_127() {
        let lines = test_lines();
        let result = apply_filter("zzz", &[], lines);
        assert_eq!(result.exit_code, 127);
    }
```

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test --lib core::commands::filters::tests
```

Expected: compile errors on existing tests (`.output`/`.exit_code` not found on `Vec`), and new tests fail.

- [ ] **Step 3: Rewrite `apply_filter` to return `CommandResult`**

Replace the top of `src/core/commands/filters.rs` (the functions, not the tests):

```rust
//! Pipe filter commands (grep, head, tail, wc).
//!
//! These filters operate on output lines from other commands,
//! enabling Unix-style piping: `ls | grep foo | head -5`

use crate::config::pipe_filters;
use crate::models::{OutputLine, OutputLineData};

use super::CommandResult;

/// Apply a filter command to output lines.
pub fn apply_filter(cmd: &str, args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    match cmd.to_lowercase().as_str() {
        "grep" => filter_grep(args, lines),
        "head" => filter_head(args, lines),
        "tail" => filter_tail(args, lines),
        "wc" => filter_wc(lines),
        _ => CommandResult::error_line(format!(
            "Pipe: unknown filter '{}'. Supported: grep, head, tail, wc",
            cmd
        ))
        .with_exit_code(127),
    }
}

fn filter_grep(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let pattern = args.first().map(|s| s.as_str()).unwrap_or("");
    if pattern.is_empty() {
        return CommandResult::error_line("grep: missing pattern").with_exit_code(2);
    }

    let pattern_lower = pattern.to_lowercase();
    let matched: Vec<OutputLine> = lines
        .into_iter()
        .filter(|line| line_contains(&line.data, &pattern_lower))
        .collect();

    let exit_code = if matched.is_empty() { 1 } else { 0 };
    CommandResult::output(matched).with_exit_code(exit_code)
}

fn line_contains(data: &OutputLineData, pattern: &str) -> bool {
    match data {
        OutputLineData::Text(s)
        | OutputLineData::Error(s)
        | OutputLineData::Success(s)
        | OutputLineData::Info(s)
        | OutputLineData::Ascii(s) => s.to_lowercase().contains(pattern),
        OutputLineData::ListEntry { name, .. } => name.to_lowercase().contains(pattern),
        OutputLineData::Command { input, .. } => input.to_lowercase().contains(pattern),
        OutputLineData::Empty => false,
    }
}

fn filter_head(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let n = parse_count_arg(args, pipe_filters::DEFAULT_HEAD_LINES);
    CommandResult::output(lines.into_iter().take(n).collect())
}

fn filter_tail(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let n = parse_count_arg(args, pipe_filters::DEFAULT_TAIL_LINES);
    let len = lines.len();
    CommandResult::output(lines.into_iter().skip(len.saturating_sub(n)).collect())
}

fn filter_wc(lines: Vec<OutputLine>) -> CommandResult {
    let count = lines
        .iter()
        .filter(|l| !matches!(l.data, OutputLineData::Empty))
        .count();
    CommandResult::output(vec![OutputLine::text(format!("{}", count))])
}

fn parse_count_arg(args: &[String], default: usize) -> usize {
    args.first()
        .and_then(|s| s.trim_start_matches('-').parse().ok())
        .unwrap_or(default)
}
```

- [ ] **Step 4: Run tests to verify pass**

```bash
cargo test --lib core::commands::filters::tests
```

Expected: all tests pass (existing + 3 new).

- [ ] **Step 5: Commit**

```bash
git add src/core/commands/filters.rs
git commit -m "feat(filters): apply_filter returns CommandResult with exit codes"
```

---

### Task 2.4: Propagate exit_code and side_effect through `execute_pipeline`

**Files:**
- Modify: `src/core/commands/mod.rs`

- [ ] **Step 1: Write failing tests**

Add at the end of the existing `tests` module in `src/core/commands/mod.rs`:

```rust
    #[test]
    fn test_pipeline_no_filters_preserves_side_effect() {
        // execute_pipeline should preserve SideEffect from first command
        // when there are no filters.
        use crate::app::TerminalState;
        use crate::core::VirtualFs;
        use crate::core::parser::parse_input;
        use crate::models::WalletState;

        let state = TerminalState::new();
        let wallet = WalletState::Disconnected;
        let fs = VirtualFs::empty();
        let route = AppRoute::Root;

        let pipeline = parse_input("login", &[]);
        let result = execute_pipeline(&pipeline, &state, &wallet, &fs, &route);
        assert_eq!(result.side_effect, Some(super::SideEffect::Login));
    }

    #[test]
    fn test_pipeline_drops_side_effect_when_piped() {
        // When a command has filters attached, side effects are discarded.
        use crate::app::TerminalState;
        use crate::core::VirtualFs;
        use crate::core::parser::parse_input;
        use crate::models::WalletState;

        let state = TerminalState::new();
        let wallet = WalletState::Disconnected;
        let fs = VirtualFs::empty();
        let route = AppRoute::Root;

        let pipeline = parse_input("help | head -1", &[]);
        let result = execute_pipeline(&pipeline, &state, &wallet, &fs, &route);
        assert!(result.side_effect.is_none());
    }

    #[test]
    fn test_pipeline_exit_code_is_last_stage() {
        use crate::app::TerminalState;
        use crate::core::VirtualFs;
        use crate::core::parser::parse_input;
        use crate::models::WalletState;

        let state = TerminalState::new();
        let wallet = WalletState::Disconnected;
        let fs = VirtualFs::empty();
        let route = AppRoute::Root;

        // `help | grep xyzzy` should exit 1 (grep no match)
        let pipeline = parse_input("help | grep xyzzy", &[]);
        let result = execute_pipeline(&pipeline, &state, &wallet, &fs, &route);
        assert_eq!(result.exit_code, 1);
    }
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test --lib core::commands::tests
```

Expected: new tests fail.

- [ ] **Step 3: Rewrite `execute_pipeline`**

Replace `execute_pipeline` (lines 188–221) in `src/core/commands/mod.rs`:

```rust
pub fn execute_pipeline(
    pipeline: &Pipeline,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    if let Some(ref err) = pipeline.error {
        return CommandResult::error_line(err.to_string());
    }

    if pipeline.is_empty() {
        return CommandResult::empty();
    }

    // Execute first command.
    let first = &pipeline.commands[0];
    let cmd = Command::parse(&first.name, &first.args);
    let mut result = execute_command(cmd, state, wallet_state, fs, current_route);

    if pipeline.commands.len() == 1 {
        return result;
    }

    // Pipeline mode: side effects are discarded (cannot navigate mid-pipe).
    result.side_effect = None;
    let mut current_lines = result.output;
    let mut current_exit = result.exit_code;

    for filter_cmd in pipeline.commands.iter().skip(1) {
        let stage = apply_filter(&filter_cmd.name, &filter_cmd.args, current_lines);
        current_lines = stage.output;
        current_exit = stage.exit_code;
    }

    CommandResult::output(current_lines).with_exit_code(current_exit)
}
```

- [ ] **Step 4: Run tests to verify pass**

```bash
cargo test --lib core::commands
```

Expected: all command tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/core/commands/mod.rs
git commit -m "feat(pipeline): propagate exit_code, drop side_effect when piped"
```

---

### Task 2.5: Update `terminal.rs` to dispatch side effects

**Files:**
- Modify: `src/components/terminal/terminal.rs`

- [ ] **Step 1: Replace the submit callback**

Open `src/components/terminal/terminal.rs` and replace the entire `create_submit_callback` function (lines 139–219) and the `fs_path_to_browse_route` helper (lines 251–262) with:

```rust
fn create_submit_callback(ctx: AppContext, route_ctx: RouteContext) -> Callback<String> {
    Callback::new(move |input: String| {
        let current_route = route_ctx.0.get();
        let prompt = ctx.get_prompt(&current_route);

        if !input.is_empty() {
            ctx.terminal
                .push_output(OutputLine::command(prompt, &input));
            ctx.terminal.add_to_command_history(&input);
        }

        let pipeline = ctx
            .terminal
            .command_history
            .with(|history| parse_input(&input, history));

        let current_fs = ctx.fs.get();
        let wallet_state = ctx.wallet.get();
        let result = execute_pipeline(
            &pipeline,
            &ctx.terminal,
            &wallet_state,
            &current_fs,
            &current_route,
        );

        ctx.terminal.push_lines(result.output);

        if let Some(effect) = result.side_effect {
            dispatch_side_effect(&ctx, effect);
        }
    })
}

/// Perform a side effect requested by a command.
fn dispatch_side_effect(ctx: &AppContext, effect: crate::core::SideEffect) {
    use crate::core::SideEffect;

    match effect {
        SideEffect::Navigate(route) => route.push(),
        SideEffect::Login => handle_login(*ctx),
        SideEffect::Logout => handle_logout(ctx),
        SideEffect::SwitchView(mode) => ctx.view_mode.set(mode),
        SideEffect::SwitchViewAndNavigate(mode, route) => {
            route.push();
            ctx.view_mode.set(mode);
        }
    }
}
```

- [ ] **Step 2: Update imports**

At the top of `src/components/terminal/terminal.rs`, ensure the `core` import exposes `SideEffect`:

```rust
use crate::core::{SideEffect, autocomplete, execute_pipeline, get_hint, parse_input, wallet};
```

(Remove unused imports the compiler flags afterward.)

- [ ] **Step 3: Update `src/core/mod.rs` re-exports**

In `src/core/mod.rs`, change line 17:

```rust
pub use commands::{Command, SideEffect, execute_pipeline};
```

- [ ] **Step 4: Build and test**

```bash
cargo build
cargo test --lib
```

Expected: clean build (may have one unused import warning to remove).

- [ ] **Step 5: Smoke-test in browser**

```bash
trunk serve
```

In the browser at `http://127.0.0.1:8080`:
- Run `login` → MetaMask prompt appears (or error if not installed)
- Run `logout` → "Disconnected from wallet." if connected; "No wallet connected." otherwise
- Run `explorer` → switches to Explorer view
- Run `explorer blog` (if `blog/` exists) → navigates and switches
- Run `explorer nonexistent` → error message stays on terminal
- Run `ls | grep xyzzy` → empty output (no crash from discarded side_effect)

Expected: all behaviors identical to pre-refactor.

- [ ] **Step 6: Commit**

```bash
git add src/components/terminal/terminal.rs src/core/mod.rs
git commit -m "refactor(terminal): dispatch SideEffect via unified handler"
```

---

### Task 2.6: Remove dead code and final verification

**Files:**
- Modify: `src/core/commands/mod.rs` (if `#[allow(dead_code)]` lingers)

- [ ] **Step 1: Remove `#[allow(dead_code)]` from `Command::Explorer`**

In `src/core/commands/mod.rs` at the `Command` enum (around line 109), remove the attribute on `Explorer`:

```rust
    /// Switch to explorer view mode with optional path
    Explorer(Option<PathArg>),
```

- [ ] **Step 2: Full test suite**

```bash
cargo test
```

Expected: all tests pass. Note any flaky or newly-failing tests.

- [ ] **Step 3: Release build to catch warnings**

```bash
cargo build --release --target wasm32-unknown-unknown
```

Expected: no warnings. Fix any that appear.

- [ ] **Step 4: Final smoke test**

```bash
trunk serve
```

Walk through the main flows once more: boot, `ls`, `cd`, `cat`, `login`, `logout`, `explorer`, pipe expressions. Navigate via Explorer UI, open files in Reader.

- [ ] **Step 5: Commit**

```bash
git add src/core/commands/mod.rs
git commit -m "chore: enable Command::Explorer lint after refactor"
```

- [ ] **Step 6: Tag the phase-1 baseline**

```bash
git tag phase1-complete
```

This tag marks the baseline that Phase 2 worktrees will branch from.

---

## Self-Review Checklist

- [x] **Spec coverage**: C1, H6, H3, H7 each have concrete tasks (1.1–1.6, 2.1–2.6).
- [x] **No placeholders**: every code block shows actual Rust code; commands are exact.
- [x] **Type consistency**: `SideEffect`, `CommandResult`, `mounts()`, `MountRegistry::home()` names used consistently across tasks.
- [x] **Test-first**: every section starts with failing tests before implementation (Tasks 1.1, 2.1, 2.2, 2.3, 2.4).
- [x] **Commits are atomic**: one commit per logical change, no squashing required.

## Done Criteria

- `cargo test` — all tests pass
- `cargo build --release --target wasm32-unknown-unknown` — no warnings
- `trunk serve` — app works identically to pre-refactor
- No remaining call sites of `configured_mounts()`, `default_mount()`, `default_base_url()`
- `terminal.rs::create_submit_callback` has no string-match special cases
- `CommandResult` has `exit_code` field; error paths set non-zero
- `phase1-complete` git tag exists
