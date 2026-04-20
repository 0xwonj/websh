# Phase 2 Follow-ups — Implementation Plan

**Goal:** Address all issues raised in the Phase 2 multi-agent review. Bounded scope; no write-mode prep (that's Phase 3).

**Addresses:** pre-existing test failures, documentation staleness, test coverage gaps, P2 polish items from the review, and preemptive AppError unification.

## Commits (6 grouped)

1. `fix(perm): DisplayPermissions format matches Unix ls -l (no spaces)` — fixes 4 pre-existing test failures
2. `docs: update help.txt and CLAUDE.md for Phase 2 semantics`
3. `test: breadcrumb path unit tests, strengthen route fallback, drop duplicate head test`
4. `feat(error): unify WalletError/FetchError/EnvironmentError under AppError with From impls`
5. `feat(grep): -F fixed-strings flag + clearer message for extra positional arg`
6. `perf(reactive): Router Memo fast-path for Root + command_history navigate via with()`

## File Structure

| Path | Action |
|---|---|
| `src/models/filesystem.rs` | Modify (DisplayPermissions format) |
| `assets/text/help.txt` | Modify |
| `CLAUDE.md` | Modify |
| `src/components/breadcrumb.rs` | Modify (extract testable helper) |
| `src/models/route.rs` | Modify (strengthen test_resolve_unknown_path_keeps_heuristic) |
| `src/core/commands/filters.rs` | Modify (remove duplicate test, add -F, improve error msg) |
| `src/core/error.rs` | Modify (add AppError + From) |
| `src/components/router.rs` | Modify (Memo Root fast-path) |
| `src/app.rs` | Modify (navigate_history use with()) |

---

## Commit 1: DisplayPermissions format fix

**Files:** `src/models/filesystem.rs`

In `impl fmt::Display for DisplayPermissions` (around line 78-90):

Current:
```rust
write!(
    f,
    "{} {} {} {}",
    if self.is_dir { 'd' } else { '-' },
    if self.read { 'r' } else { '-' },
    if self.write { 'w' } else { '-' },
    if self.execute { 'x' } else { '-' },
)
```

Change to (remove spaces):
```rust
write!(
    f,
    "{}{}{}{}",
    if self.is_dir { 'd' } else { '-' },
    if self.read { 'r' } else { '-' },
    if self.write { 'w' } else { '-' },
    if self.execute { 'x' } else { '-' },
)
```

**Verification:**
```bash
cargo test --bin websh core::filesystem::tests::test_permissions
```
Expected: all 4 now pass.

```bash
cargo test --bin websh
```
Expected: **193 pass / 0 fail** (up from 189/4).

Commit: `fix(perm): DisplayPermissions format matches Unix ls -l (no spaces)`

---

## Commit 2: Docs update

**Files:** `assets/text/help.txt`, `CLAUDE.md`

### `assets/text/help.txt` — update these sections

Replace the `Environment` and `Pipe Filters` blocks and related tips:

```
  Environment:
    export                    Show all variables
    export KEY=value [...]    Set one or more variables (localStorage)
    unset KEY                 Remove variable
    cat .profile              View all localStorage data

  Pipe Filters:
    grep [-i] [-v] [-F] <pattern>   Filter lines (regex by default; -i case-insensitive; -v invert; -F literal)
    head -N | head -n N             First N lines (default: 10)
    tail -N | tail -n N             Last N lines (default: 10)
    wc                              Count non-empty lines

  Tips:
    - Use Tab for autocomplete
    - Up/Down arrows navigate command history
    - Chain commands with pipes: ls | grep -i md | head -5
    - Press 'q' or 'Esc' to exit reader view
    - grep is case-sensitive by default; pass -i for case-insensitive matching
```

### `CLAUDE.md` — add a new section at the end (before the Wallet Integration if it's last, or after "Component Structure"):

```markdown
### Public APIs added in Phase 2

- `config::mounts() -> &'static MountRegistry` — process-wide singleton via `OnceLock`. Non-empty invariant enforced at init.
- `MountRegistry::home() -> &Mount`, `resolve(alias) -> Option<&Mount>`, `all() -> impl Iterator<Item=&Mount>`.
- `AppRoute::resolve(&VirtualFs) -> Self` — corrects Browse↔Read classification against the actual filesystem. Called from `AppRouter`'s `Memo` on hash/fs change.
- `CommandResult { output, exit_code: i32, side_effect: Option<SideEffect> }` + `SideEffect` enum (`Navigate`, `Login`, `Logout`, `SwitchView`, `SwitchViewAndNavigate`). All UI side effects flow through `dispatch_side_effect(&AppContext, SideEffect)` in `components/terminal/terminal.rs`.
- `AppError` enum (this commit) wraps domain errors with `From` impls for ergonomic `?` across boundaries.
- Pipe filters (`grep`, `head`, `tail`, `wc`) return `CommandResult` with POSIX exit codes (0 = match / success, 1 = no match, 2 = usage / syntax error, 127 = unknown).
```

Commit: `docs: update help.txt and CLAUDE.md for Phase 2 semantics`

---

## Commit 3: Test improvements

**Files:**
- `src/components/breadcrumb.rs` — extract path builder helper, add native tests
- `src/models/route.rs` — strengthen `test_resolve_unknown_path_keeps_heuristic`
- `src/core/commands/filters.rs` — remove duplicate `test_head_default_no_args`

### 3a. Breadcrumb path builder extraction

Read `src/components/breadcrumb.rs` first. Find the absolute-path block (post-cherry-pick in Track P, around the `else if let Some(mount) = route.mount() { ... }` branch). The logic:

```rust
let start_idx = if segments.first() == Some(&"~") { 1 } else { 0 };
let path = segments[start_idx..=idx].join("/");
```

Extract into a pub(super) or inline helper:

```rust
/// Build the absolute path for a breadcrumb segment click.
///
/// `segments`: full breadcrumb segments from the current route, including
/// any leading "~" mount alias.
/// `idx`: the clicked segment's index into `segments`.
///
/// If segments starts with "~", the home mount alias is skipped when joining.
fn build_segment_path(segments: &[&str], idx: usize) -> String {
    let start_idx = if segments.first() == Some(&"~") { 1 } else { 0 };
    if idx < start_idx {
        return String::new();
    }
    segments[start_idx..=idx].join("/")
}
```

Replace the inline code with `let path = build_segment_path(&segments, idx);`.

Add tests in `#[cfg(test)] mod tests` at the bottom of `breadcrumb.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::build_segment_path;

    #[test]
    fn test_build_path_simple() {
        let segments = vec!["~", "blog", "posts"];
        assert_eq!(build_segment_path(&segments, 1), "blog");
        assert_eq!(build_segment_path(&segments, 2), "blog/posts");
    }

    #[test]
    fn test_build_path_no_home_prefix() {
        let segments = vec!["work", "notes"];
        assert_eq!(build_segment_path(&segments, 0), "work");
        assert_eq!(build_segment_path(&segments, 1), "work/notes");
    }

    #[test]
    fn test_build_path_home_at_zero_returns_empty() {
        // Clicking the "~" segment itself — the caller handles this via
        // is_home_segment branch, but the builder gracefully returns "".
        let segments = vec!["~", "blog"];
        assert_eq!(build_segment_path(&segments, 0), "");
    }

    #[test]
    fn test_build_path_deep_nesting() {
        let segments = vec!["~", "a", "b", "c", "d"];
        assert_eq!(build_segment_path(&segments, 4), "a/b/c/d");
    }
}
```

### 3b. Strengthen `test_resolve_unknown_path_keeps_heuristic` in `src/models/route.rs`

Find the existing test (around lines 685-693). Currently it uses `Read { "nonexistent.md" }` — heuristic already agrees. Strengthen by feeding a mismatched variant that the fallback must explicitly handle:

```rust
    #[test]
    fn test_resolve_unknown_path_keeps_heuristic() {
        // When fs has no info, resolve falls back to the extension heuristic.
        // Feed a Browse route for a dotted path — heuristic says "file", and
        // the fallback should promote it.
        let fs = fs_with_entries();
        let unknown_file = AppRoute::Browse {
            mount: test_mount(),
            path: "nonexistent.md".to_string(),
        };
        let resolved = unknown_file.resolve(&fs);
        assert!(
            matches!(resolved, AppRoute::Read { ref path, .. } if path == "nonexistent.md"),
            "fallback should promote dotted path to Read"
        );

        // And the inverse: a non-dotted path should fall back to Browse.
        let unknown_dir = AppRoute::Read {
            mount: test_mount(),
            path: "nonexistent_dir".to_string(),
        };
        let resolved = unknown_dir.resolve(&fs);
        assert!(
            matches!(resolved, AppRoute::Browse { ref path, .. } if path == "nonexistent_dir"),
            "fallback should demote non-dotted path to Browse"
        );
    }
```

### 3c. Remove duplicate test in `src/core/commands/filters.rs`

`test_head_default` and `test_head_default_no_args` both pass no args and assert 5 lines (default 10 truncated to `test_lines()` size 5). Keep `test_head_default`. Remove `test_head_default_no_args`.

Commit: `test: breadcrumb path unit tests, strengthen route fallback, drop duplicate head test`

---

## Commit 4: AppError unification

**Files:** `src/core/error.rs`

Append to `src/core/error.rs` (keeping existing 3 domain errors intact):

```rust
/// Unified application error wrapping the three domain errors.
///
/// Use this when code needs to propagate errors across domain boundaries
/// (e.g., a function that may hit both fetch and environment failures).
/// Each domain-specific error type remains preferred within its own module.
///
/// Implements `From` for each domain error to enable `?` across boundaries.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppError {
    Wallet(WalletError),
    Fetch(FetchError),
    Environment(EnvironmentError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wallet(e) => write!(f, "{}", e),
            Self::Fetch(e) => write!(f, "{}", e),
            Self::Environment(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wallet(e) => Some(e),
            Self::Fetch(e) => Some(e),
            Self::Environment(e) => Some(e),
        }
    }
}

impl From<WalletError> for AppError {
    fn from(e: WalletError) -> Self { Self::Wallet(e) }
}

impl From<FetchError> for AppError {
    fn from(e: FetchError) -> Self { Self::Fetch(e) }
}

impl From<EnvironmentError> for AppError {
    fn from(e: EnvironmentError) -> Self { Self::Environment(e) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_from_wallet() {
        let wallet_err = WalletError::NoWindow;
        let app_err: AppError = wallet_err.into();
        assert!(matches!(app_err, AppError::Wallet(WalletError::NoWindow)));
    }

    #[test]
    fn test_app_error_from_fetch() {
        let fetch_err = FetchError::HttpError(404);
        let app_err: AppError = fetch_err.into();
        assert!(matches!(app_err, AppError::Fetch(FetchError::HttpError(404))));
    }

    #[test]
    fn test_app_error_from_environment() {
        let env_err = EnvironmentError::InvalidVariableName;
        let app_err: AppError = env_err.into();
        assert!(matches!(app_err, AppError::Environment(EnvironmentError::InvalidVariableName)));
    }

    #[test]
    fn test_app_error_display_delegates() {
        let app_err = AppError::Fetch(FetchError::HttpError(500));
        assert_eq!(app_err.to_string(), "HTTP error: 500");
    }

    #[test]
    fn test_app_error_source_chain() {
        let app_err = AppError::Wallet(WalletError::NoAccount);
        let source = std::error::Error::source(&app_err);
        assert!(source.is_some());
    }
}
```

**Verification:** `cargo test --bin websh core::error` passes all 5 new tests; existing domain-error callers unaffected (no callers currently use `AppError` — that's OK, `#[allow(dead_code)]` added).

Commit: `feat(error): unify domain errors under AppError with From impls`

---

## Commit 5: grep -F + clearer extra-positional error

**Files:** `src/core/commands/filters.rs`

### 5a. Add `-F` flag

In `filter_grep`, add a `fixed_strings: bool` to the flag loop and `--fixed-strings` long form:

```rust
let mut ignore_case = false;
let mut invert = false;
let mut fixed_strings = false;
let mut pattern: Option<&str> = None;

for arg in args {
    if arg.starts_with("--") {
        match arg.as_str() {
            "--ignore-case" => ignore_case = true,
            "--invert-match" => invert = true,
            "--extended-regexp" => {}
            "--fixed-strings" => fixed_strings = true,
            _ => {
                return CommandResult::error_line(format!(
                    "grep: unknown option: {}",
                    arg
                ))
                .with_exit_code(2);
            }
        }
    } else if let Some(rest) = arg.strip_prefix('-') {
        if rest.is_empty() {
            if pattern.is_none() { pattern = Some(arg.as_str()); }
            else {
                return CommandResult::error_line(
                    "grep: extra argument (multiple patterns or file args are not supported)".to_string(),
                )
                .with_exit_code(2);
            }
        } else {
            for ch in rest.chars() {
                match ch {
                    'i' => ignore_case = true,
                    'v' => invert = true,
                    'E' => {}
                    'F' => fixed_strings = true,
                    other => {
                        return CommandResult::error_line(format!(
                            "grep: unknown option: -{}",
                            other
                        ))
                        .with_exit_code(2);
                    }
                }
            }
        }
    } else if pattern.is_none() {
        pattern = Some(arg.as_str());
    } else {
        return CommandResult::error_line(
            "grep: extra argument (multiple patterns or file args are not supported)".to_string(),
        )
        .with_exit_code(2);
    }
}
```

Then escape the pattern if `fixed_strings`:

```rust
let effective_pattern = if fixed_strings {
    regex::escape(pat)
} else {
    pat.to_string()
};

let regex = match build_grep_regex(&effective_pattern, ignore_case) {
    Ok(r) => r,
    Err(e) => {
        return CommandResult::error_line(format!("grep: invalid regex: {}", e))
            .with_exit_code(2);
    }
};
```

Update `build_grep_regex` signature to accept `&str` (it already does — just pass `&effective_pattern`).

### 5b. Add tests

Append to the tests module:

```rust
    #[test]
    fn test_grep_fixed_strings_short_flag() {
        // Without -F, parens are regex metachars
        let lines = vec![OutputLine::text("hello (world)")];
        let result = apply_filter("grep", &args(&["-F", "(world)"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
    }

    #[test]
    fn test_grep_fixed_strings_long_flag() {
        let lines = vec![OutputLine::text("a.b.c")];
        let result = apply_filter("grep", &args(&["--fixed-strings", "a.b"]), lines);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_grep_fixed_strings_combined_with_i() {
        let lines = vec![
            OutputLine::text("HELLO.WORLD"),
            OutputLine::text("no match here"),
        ];
        let result = apply_filter("grep", &args(&["-iF", "hello.world"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
    }

    #[test]
    fn test_grep_extra_positional_error_message() {
        let lines = vec![OutputLine::text("x")];
        let result = apply_filter("grep", &args(&["pat1", "pat2"]), lines);
        assert_eq!(result.exit_code, 2);
        let msg = match &result.output[0].data {
            OutputLineData::Error(s) => s.clone(),
            _ => panic!("expected error"),
        };
        assert!(msg.contains("extra argument"), "msg: {}", msg);
    }
```

**Verification:** `cargo test --bin websh core::commands::filters` passes. Full: 193+4 = 197 pass / 0 fail.

Commit: `feat(grep): -F fixed-strings flag + clearer error for extra positional`

---

## Commit 6: Router Memo fast-path + navigate_history with()

**Files:** `src/components/router.rs`, `src/app.rs`

### 6a. AppRouter Memo fast-path

In `src/components/router.rs`, find:

```rust
let route = Memo::new(move |_| ctx.fs.with(|fs| raw_route.get().resolve(fs)));
```

Change to:

```rust
let route = Memo::new(move |_| {
    let raw = raw_route.get();
    // Fast path: Root doesn't depend on fs, avoid tracking ctx.fs.
    if matches!(raw, AppRoute::Root) {
        return raw;
    }
    ctx.fs.with(|fs| raw.resolve(fs))
});
```

### 6b. `navigate_history` uses `.with()`

In `src/app.rs`, find `navigate_history`:

```rust
    pub fn navigate_history(&self, direction: i32) -> Option<String> {
        let history = self.command_history.get();
        if history.is_empty() {
            return None;
        }

        let current_index = self.history_index.get();
        let new_index = match current_index {
            None if direction < 0 => Some(history.len() - 1),
            Some(i) if direction < 0 && i > 0 => Some(i - 1),
            Some(i) if direction > 0 && i < history.len() - 1 => Some(i + 1),
            Some(_) if direction > 0 => None,
            _ => current_index,
        };

        self.history_index.set(new_index);
        new_index.map(|i| history[i].clone())
    }
```

Change to:

```rust
    pub fn navigate_history(&self, direction: i32) -> Option<String> {
        let current_index = self.history_index.get();
        let (new_index, result) = self.command_history.with(|history| {
            if history.is_empty() {
                return (None, None);
            }
            let new_index = match current_index {
                None if direction < 0 => Some(history.len() - 1),
                Some(i) if direction < 0 && i > 0 => Some(i - 1),
                Some(i) if direction > 0 && i < history.len() - 1 => Some(i + 1),
                Some(_) if direction > 0 => None,
                _ => current_index,
            };
            let result = new_index.map(|i| history[i].clone());
            (new_index, result)
        });
        self.history_index.set(new_index);
        result
    }
```

No full history clone; just one indexed clone of the string result.

**Verification:** existing history navigation tests (if any) still pass. `cargo build && cargo test --bin websh`: clean, 197 pass / 0 fail.

Commit: `perf(reactive): Router Memo fast-path for Root + command_history navigate via with()`

---

## Done Criteria

- `cargo test --bin websh`: **197 pass / 0 fail** (was 189/4).
- `cargo build --release --target wasm32-unknown-unknown`: clean.
- All 6 commits applied in order.
- Docs (`help.txt`, `CLAUDE.md`) reflect Phase 2 semantics.
- AppError enum exists (unused yet; `#[allow(dead_code)]` noted).
- `grep -F` works.
- Zero new warnings.
