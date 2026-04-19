# Phase 2 Track D: Route/FS Resolve — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the extension-heuristic file/dir detection in `AppRoute::from_path` with filesystem-backed resolution, and make `cd ""` a POSIX-correct error.

**Architecture:** Keep `AppRoute::from_path` pure (URL parsing only, heuristic fallback). Add a new `AppRoute::resolve(self, fs) -> AppRoute` method that promotes `Browse↔Read` based on what the actual `VirtualFs` knows. Wire resolution into `AppRouter`'s route signal so `is_file()` becomes authoritative once fs is loaded. Fix `execute_cd("")` to emit a POSIX-style error.

**Tech Stack:** Rust 2024, Leptos 0.8, wasm32-unknown-unknown, Trunk. Same as Phase 1.

**Addresses issues:** H2 (file/dir heuristic is wrong for `Makefile`, `archive.2024`, etc.), M8 (`cd ""` silently stays in place instead of erroring).

---

## Scope

**In scope:**
- `AppRoute::resolve(self, &VirtualFs) -> AppRoute` (new)
- `AppRouter` in `src/components/router.rs` wires resolution into the route signal
- `execute_cd` rejects empty-string target

**Out of scope (deferred):**
- `AppRoute::join` internal heuristic (callers can `.resolve()` if they need correctness)
- `AppRoute::from_path` heuristic itself — kept as fallback when fs has no info about the path
- URL normalization (trailing slash canonicalization)
- Any change to `Mount` / `MountRegistry` API

## File Structure

| Path | Responsibility | Action |
|---|---|---|
| `src/models/route.rs` | `AppRoute::resolve` method + tests | Modify |
| `src/components/router.rs` | Thread `fs` signal into route `Memo` so resolution runs on fs load | Modify |
| `src/core/commands/execute.rs` | `execute_cd` rejects `""` | Modify |

No new files.

---

## Task D.1: Add `AppRoute::resolve(fs) -> AppRoute`

**Files:**
- Modify: `src/models/route.rs`

- [ ] **Step 1: Write failing tests**

Append to the existing `#[cfg(test)] mod tests` block in `src/models/route.rs`:

```rust
    // ------------------------------------------------------------------------
    // AppRoute::resolve tests
    // ------------------------------------------------------------------------

    use crate::core::VirtualFs;
    use crate::models::{Manifest, FileEntry, DirectoryEntry};

    fn fs_with_entries() -> VirtualFs {
        // Build a manifest with:
        //   /Makefile                 (file, no extension)
        //   /archive.2024/            (directory, dot in name)
        //   /archive.2024/index.md    (file)
        //   /blog/                    (directory)
        //   /blog/post.md             (file)
        let manifest = Manifest {
            files: vec![
                FileEntry {
                    path: "Makefile".into(),
                    title: "Makefile".into(),
                    content_path: Some("Makefile".into()),
                    ..Default::default()
                },
                FileEntry {
                    path: "archive.2024/index.md".into(),
                    title: "Archive".into(),
                    content_path: Some("archive.2024/index.md".into()),
                    ..Default::default()
                },
                FileEntry {
                    path: "blog/post.md".into(),
                    title: "Post".into(),
                    content_path: Some("blog/post.md".into()),
                    ..Default::default()
                },
            ],
            directories: vec![
                DirectoryEntry {
                    path: "archive.2024".into(),
                    title: "Archive".into(),
                    ..Default::default()
                },
                DirectoryEntry {
                    path: "blog".into(),
                    title: "Blog".into(),
                    ..Default::default()
                },
            ],
        };
        VirtualFs::from_manifest(&manifest)
    }

    #[test]
    fn test_resolve_root_stays_root() {
        let fs = fs_with_entries();
        assert_eq!(AppRoute::Root.resolve(&fs), AppRoute::Root);
    }

    #[test]
    fn test_resolve_promotes_makefile_to_read() {
        // `Makefile` has no extension → heuristic parses as Browse, but it IS a file.
        let fs = fs_with_entries();
        let parsed = AppRoute::Browse {
            mount: test_mount(),
            path: "Makefile".to_string(),
        };
        let resolved = parsed.resolve(&fs);
        assert!(matches!(resolved, AppRoute::Read { ref path, .. } if path == "Makefile"));
    }

    #[test]
    fn test_resolve_demotes_archive_dir_to_browse() {
        // `archive.2024` has a `.` → heuristic parses as Read, but it IS a directory.
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: "archive.2024".to_string(),
        };
        let resolved = parsed.resolve(&fs);
        assert!(matches!(resolved, AppRoute::Browse { ref path, .. } if path == "archive.2024"));
    }

    #[test]
    fn test_resolve_keeps_correct_browse() {
        let fs = fs_with_entries();
        let parsed = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        let resolved = parsed.clone().resolve(&fs);
        assert_eq!(resolved, parsed);
    }

    #[test]
    fn test_resolve_keeps_correct_read() {
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: "blog/post.md".to_string(),
        };
        let resolved = parsed.clone().resolve(&fs);
        assert_eq!(resolved, parsed);
    }

    #[test]
    fn test_resolve_unknown_path_keeps_heuristic() {
        // When fs has no info about the path, resolve must be a no-op —
        // falls back to whatever from_path produced.
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: "nonexistent.md".to_string(),
        };
        let resolved = parsed.clone().resolve(&fs);
        assert_eq!(resolved, parsed);
    }

    #[test]
    fn test_resolve_empty_path_is_browse_root() {
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: String::new(),
        };
        let resolved = parsed.resolve(&fs);
        assert!(matches!(
            resolved,
            AppRoute::Browse { ref path, .. } if path.is_empty()
        ));
    }
```

- [ ] **Step 2: Run tests to verify failure**

```bash
cargo test --bin websh models::route::tests::test_resolve
```

Expected: all 7 tests fail (compile error — method `resolve` does not exist).

- [ ] **Step 3: Implement `resolve`**

In `src/models/route.rs`, add after the existing `impl AppRoute` methods (around line 346, just before the `// ======== Helper Functions ========` divider):

```rust
    /// Refine route against the filesystem.
    ///
    /// `AppRoute::from_path` uses an extension heuristic: it cannot distinguish
    /// a file named `Makefile` (no extension) from a directory, or a directory
    /// named `archive.2024` (dot in name) from a file. `resolve` queries the
    /// actual `VirtualFs` and corrects the variant.
    ///
    /// When the path is not known to the filesystem, the route is returned
    /// unchanged (heuristic fallback).
    pub fn resolve(self, fs: &crate::core::VirtualFs) -> Self {
        match self {
            Self::Root => Self::Root,
            Self::Browse { mount, path } | Self::Read { mount, path } => {
                if path.is_empty() {
                    return Self::Browse { mount, path };
                }
                if fs.is_directory(&path) {
                    Self::Browse { mount, path }
                } else if fs.get_entry(&path).is_some() {
                    Self::Read { mount, path }
                } else {
                    // FS has no info — fall back to heuristic
                    // by reconstructing from_path's decision.
                    let last = path.rsplit('/').next().unwrap_or(&path);
                    if last.contains('.') {
                        Self::Read { mount, path }
                    } else {
                        Self::Browse { mount, path }
                    }
                }
            }
        }
    }
```

- [ ] **Step 4: Run tests to verify pass**

```bash
cargo test --bin websh models::route::tests::test_resolve
```

Expected: all 7 new tests pass.

- [ ] **Step 5: Full crate test**

```bash
cargo test --bin websh
```

Expected: no regressions. Phase 1 baseline was 148 pass / 4 pre-existing fail. After this task: 155 pass / 4 pre-existing fail.

- [ ] **Step 6: Commit**

```bash
git add src/models/route.rs
git commit -m "feat(route): add AppRoute::resolve(fs) to correct file/dir classification"
```

---

## Task D.2: Wire resolution into `AppRouter`

**Files:**
- Modify: `src/components/router.rs`

- [ ] **Step 1: Understand current flow**

Read `src/components/router.rs` lines 37–82. Current:
- `route: RwSignal<AppRoute>` initialized from `AppRoute::current()` (pure hash parse)
- `hashchange` listener calls `route.set(AppRoute::current())`
- `Memo::new(|_| route.get())` wraps for downstream

The problem: `AppRoute::current()` never sees `fs`. So `is_file()` uses the unreliable heuristic.

- [ ] **Step 2: Replace the RwSignal + Memo pair with a single resolved Memo**

The design:
- Keep a `raw_route: RwSignal<AppRoute>` for hash changes (updated by hashchange listener, same as before)
- Replace `route_memo` with a Memo that depends on both `raw_route` AND `ctx.fs`, running `raw.resolve(&fs)` each time

Open `src/components/router.rs` and replace the `AppRouter` body (lines 37–82) with:

```rust
#[component]
pub fn AppRouter() -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // Raw route from URL hash (updated on hashchange).
    let raw_route = RwSignal::new(AppRoute::current());

    // Set up hashchange event listener (runs once on mount).
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let closure = Closure::wrap(Box::new(move || {
            raw_route.set(AppRoute::current());
        }) as Box<dyn Fn()>);

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
        }

        closure.forget();
    }

    // Resolved route: re-runs whenever the hash changes OR fs loads/changes.
    // Heuristic-only on non-wasm tests (no ctx.fs).
    #[cfg(target_arch = "wasm32")]
    let route = Memo::new(move |_| ctx.fs.with(|fs| raw_route.get().resolve(fs)));
    #[cfg(not(target_arch = "wasm32"))]
    let route = Memo::new(move |_| raw_route.get());

    // Focus terminal input when returning from reader overlay.
    Effect::new(move |prev_was_file: Option<bool>| {
        let is_file = route.get().is_file();
        if prev_was_file == Some(true) && !is_file {
            focus_terminal_input();
        }
        is_file
    });

    view! {
        <Shell route=route />

        <Show when=move || route.get().is_file()>
            <ReaderOverlay route=route />
        </Show>
    }
}
```

Note: the `ReaderOverlay` component already accepts `Memo<AppRoute>`. No change there.

- [ ] **Step 3: Verify imports**

Confirm the `#[cfg(target_arch = "wasm32")] use crate::app::AppContext;` line (around line 17-18) is still present — the new code uses it.

- [ ] **Step 4: Build**

```bash
cargo build
```

Expected: clean (no new warnings).

- [ ] **Step 5: Full test suite**

```bash
cargo test --bin websh
```

Expected: no regressions. Router has no unit tests (it's a wasm-only component), but filesystem and route tests must still pass.

- [ ] **Step 6: Commit**

```bash
git add src/components/router.rs
git commit -m "feat(router): resolve route against fs so is_file() becomes authoritative"
```

---

## Task D.3: Reject `cd ""` with POSIX error

**Files:**
- Modify: `src/core/commands/execute.rs`

- [ ] **Step 1: Write failing test**

Append to the existing `#[cfg(test)] mod tests` in `src/core/commands/execute.rs`:

```rust
    #[test]
    fn test_cd_empty_string_exit_1() {
        let (ts, ws, fs) = empty_state();
        let result = execute_command(
            Command::Cd(super::PathArg::new("")),
            &ts,
            &ws,
            &fs,
            &AppRoute::Root,
        );
        assert_eq!(result.exit_code, 1);
        assert!(result.side_effect.is_none());
        assert!(
            result
                .output
                .iter()
                .any(|l| matches!(&l.data, crate::models::OutputLineData::Error(s) if s.contains("cd: ")))
        );
    }
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test --bin websh core::commands::execute::tests::test_cd_empty_string_exit_1
```

Expected: FAIL. Current behavior: `cd ""` is probably treated as `cd .` (stays in place) with no error.

- [ ] **Step 3: Implement the guard in `execute_cd`**

In `src/core/commands/execute.rs`, find `execute_cd` (around line 202). The function starts with:

```rust
fn execute_cd(path: super::PathArg, fs: &VirtualFs, current_route: &AppRoute) -> CommandResult {
    let target = path.as_str();
    let at_root = matches!(current_route, AppRoute::Root);

    // Handle special paths
    match target {
        // cd / always goes to Root
        "/" => return CommandResult::navigate(AppRoute::Root),
```

Add an empty-string arm at the **very top** of the `match target` block (before `"/"`):

```rust
    match target {
        // cd "" — POSIX: error (bash prints "cd: : No such file or directory")
        "" => {
            return CommandResult::error_line("cd: : No such file or directory");
        }

        // cd / always goes to Root
        "/" => return CommandResult::navigate(AppRoute::Root),
```

- [ ] **Step 4: Run test to verify pass**

```bash
cargo test --bin websh core::commands::execute::tests::test_cd_empty_string_exit_1
```

Expected: PASS.

- [ ] **Step 5: Full test suite**

```bash
cargo test --bin websh
```

Expected: 156 pass / 4 pre-existing fail (148 baseline + 7 D.1 tests + 1 D.3 test).

- [ ] **Step 6: Commit**

```bash
git add src/core/commands/execute.rs
git commit -m "fix(commands): reject cd with empty-string operand (POSIX)"
```

---

## Self-Review Checklist

- [x] Spec coverage: H2 → Task D.1 + D.2; M8 → Task D.3.
- [x] No placeholders: every code block is complete and runnable.
- [x] Type consistency: `resolve(self, &VirtualFs) -> Self`, `is_file()` already exists, `OutputLineData::Error` is existing.
- [x] Test-first: each task starts with a failing test.
- [x] Commits atomic: one commit per task.

## Done Criteria

- `cargo test --bin websh` passes 156 (148 + 7 + 1) / 4 pre-existing fail.
- `cargo build --release --target wasm32-unknown-unknown` clean.
- `AppRoute::current()` still pure (parse only — resolution happens in `AppRouter`).
- `execute_cd` rejects empty string with exit 1 and `"cd: : No such file or directory"`.
- No unused imports or new warnings.
