# Phase 2 Track B: Command Filters — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade `grep` to regex + flags, tighten `head`/`tail` argument parsing. H4 (iterator streaming) is deliberately **deferred** — see decision below.

**Architecture:** Keep the existing `apply_filter(&str, &[String], Vec<OutputLine>) -> CommandResult` contract (buffered pipeline) unchanged. Replace `grep`'s substring-match body with compiled `Regex`, add per-invocation flag parsing. Replace `parse_count_arg`'s lax number parsing with a strict parser rejecting double-dashes and non-numeric.

**Tech Stack:** Rust 2024, Leptos 0.8, wasm32, `regex = "1"` (new direct dep, already present transitively).

**Addresses issues:** H5 (grep regex + `-i`/`-v` flags), M6 (head/tail strict numeric parsing).

## Scope

**In scope:**
- `grep` uses compiled `Regex`. Flags `-i` (case-insensitive), `-v` (invert), `-E` (extended, alias/no-op). Short-flag combining (`-iv`, `-vi`).
- `grep` bad regex → exit 2 with error message.
- `head`/`tail` accept `-N` and `-n N`. Reject `--N`, `---N`, non-numeric. Unknown flag → exit 2.
- `regex = "1"` added to `Cargo.toml`.

**Out of scope (deferred):**
- **H4 (iterator streaming)** — **DEFERRED**. See Decision D-B-1 in master decision log. The current `Vec<OutputLine>`-buffered pipeline suits WebSH's typical input sizes (dozens to low-hundreds of lines from `ls`, `help`, `id`, etc.). Iterator streaming would require stateful exit-code accumulation (grep's exit depends on a match count that isn't known until iteration finishes), forcing `Rc<Cell<i32>>` plumbing or re-design of `CommandResult`. Cost/benefit doesn't justify the refactor now. Revisit if a command starts producing thousands of lines.
- `grep -n` (line numbers), `-c` (count-only), `-F` (fixed-strings), `-r` (recursive): YAGNI.
- `wc -l / -w / -c` flags: existing `wc` counts non-empty lines only, adequate.

## File Structure

| Path | Responsibility | Action |
|---|---|---|
| `Cargo.toml` | Add `regex = "1"` direct dep | Modify |
| `src/core/commands/filters.rs` | Rewrite `filter_grep` + `parse_count_arg` | Modify |

No new files.

---

## Task B.1: Add `regex` crate dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add the dependency**

Open `Cargo.toml`, find the `[dependencies]` section. Add:

```toml
regex = "1"
```

(Place alphabetically or at the end — follow existing style.)

- [ ] **Step 2: Verify it resolves**

```bash
cargo build
```

Expected: builds clean. `regex` was already a transitive dep, so no new download needed.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add regex crate as direct dependency"
```

---

## Task B.2: `grep` with regex + flags

**Files:**
- Modify: `src/core/commands/filters.rs`

- [ ] **Step 1: Write failing tests**

Append to `#[cfg(test)] mod tests` in `src/core/commands/filters.rs`:

```rust
    #[test]
    fn test_grep_regex_match() {
        let lines = vec![
            OutputLine::text("apple"),
            OutputLine::text("banana"),
            OutputLine::text("cherry"),
        ];
        let result = apply_filter("grep", &args(&["^b"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "banana"));
    }

    #[test]
    fn test_grep_case_sensitive_by_default() {
        let lines = vec![
            OutputLine::text("Apple"),
            OutputLine::text("apple"),
        ];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        // default is case-sensitive now (was case-insensitive previously)
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "apple"));
    }

    #[test]
    fn test_grep_ignore_case_flag() {
        let lines = vec![
            OutputLine::text("Apple"),
            OutputLine::text("apple"),
            OutputLine::text("banana"),
        ];
        let result = apply_filter("grep", &args(&["-i", "apple"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 2);
    }

    #[test]
    fn test_grep_invert_flag() {
        let lines = vec![
            OutputLine::text("apple"),
            OutputLine::text("banana"),
            OutputLine::text("cherry"),
        ];
        let result = apply_filter("grep", &args(&["-v", "apple"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 2);
    }

    #[test]
    fn test_grep_combined_short_flags() {
        let lines = vec![
            OutputLine::text("Apple"),
            OutputLine::text("banana"),
        ];
        let result = apply_filter("grep", &args(&["-iv", "apple"]), lines);
        // -i case-insensitive AND -v invert: "Apple" matches case-insensitive so is excluded;
        // "banana" doesn't match, so is kept
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "banana"));
    }

    #[test]
    fn test_grep_extended_flag_accepted() {
        // -E is accepted as alias (regex crate always uses extended syntax)
        let lines = vec![OutputLine::text("apple")];
        let result = apply_filter("grep", &args(&["-E", "a.*e"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
    }

    #[test]
    fn test_grep_invalid_regex_exit_2() {
        let lines = vec![OutputLine::text("anything")];
        // unbalanced parens
        let result = apply_filter("grep", &args(&["("]), lines);
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_grep_unknown_flag_exit_2() {
        let lines = vec![OutputLine::text("anything")];
        let result = apply_filter("grep", &args(&["-x", "pat"]), lines);
        assert_eq!(result.exit_code, 2);
    }
```

Update the pre-existing `test_grep_case_insensitive` test — its expectation is obsolete (grep is now case-sensitive by default). Change the test to assert the opposite: case-sensitive default produces zero matches:

Find (around line 113):
```rust
    #[test]
    fn test_grep_case_insensitive() {
        let lines = vec![OutputLine::text("APPLE"), OutputLine::text("banana")];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "APPLE"));
    }
```

Note: this test references `result.len()` on an old API; it should already be `result.output.len()` after Phase 1. Regardless, replace the whole test body with case-sensitive semantics:

```rust
    #[test]
    fn test_grep_case_sensitive_default_rejects_uppercase() {
        let lines = vec![OutputLine::text("APPLE"), OutputLine::text("banana")];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        // default is case-sensitive, so "APPLE" doesn't match
        assert_eq!(result.exit_code, 1);
        assert!(result.output.is_empty());
    }
```

Rename to clarify intent.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test --bin websh core::commands::filters::tests
```

Expected: new tests fail (Regex not used yet; flags not parsed).

- [ ] **Step 3: Rewrite `filter_grep`**

Replace the body of `filter_grep` in `src/core/commands/filters.rs`:

```rust
fn filter_grep(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    // Parse flags and pattern.
    let mut ignore_case = false;
    let mut invert = false;
    let mut pattern: Option<&str> = None;

    for arg in args {
        if arg.starts_with("--") {
            match arg.as_str() {
                "--ignore-case" => ignore_case = true,
                "--invert-match" => invert = true,
                "--extended-regexp" => {} // no-op: regex crate is always extended
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
                // A bare "-" is not a flag; treat as pattern if pattern is None.
                if pattern.is_none() {
                    pattern = Some(arg.as_str());
                } else {
                    return CommandResult::error_line(
                        "grep: unexpected extra argument".to_string(),
                    )
                    .with_exit_code(2);
                }
            } else {
                for ch in rest.chars() {
                    match ch {
                        'i' => ignore_case = true,
                        'v' => invert = true,
                        'E' => {} // no-op
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
            // extra positional arg: not supported
            return CommandResult::error_line(
                "grep: unexpected extra argument".to_string(),
            )
            .with_exit_code(2);
        }
    }

    let Some(pat) = pattern else {
        return CommandResult::error_line("grep: missing pattern").with_exit_code(2);
    };

    // Compile regex (with case-insensitive flag if requested).
    let regex = match build_grep_regex(pat, ignore_case) {
        Ok(r) => r,
        Err(e) => {
            return CommandResult::error_line(format!("grep: invalid regex: {}", e))
                .with_exit_code(2);
        }
    };

    let matched: Vec<OutputLine> = lines
        .into_iter()
        .filter(|line| {
            let is_match = regex_matches_line(&regex, &line.data);
            is_match ^ invert
        })
        .collect();

    let exit_code = if matched.is_empty() { 1 } else { 0 };
    CommandResult::output(matched).with_exit_code(exit_code)
}

fn build_grep_regex(pattern: &str, ignore_case: bool) -> Result<regex::Regex, regex::Error> {
    regex::RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
}

fn regex_matches_line(re: &regex::Regex, data: &OutputLineData) -> bool {
    match data {
        OutputLineData::Text(s)
        | OutputLineData::Error(s)
        | OutputLineData::Success(s)
        | OutputLineData::Info(s)
        | OutputLineData::Ascii(s) => re.is_match(s),
        OutputLineData::ListEntry { name, .. } => re.is_match(name),
        OutputLineData::Command { input, .. } => re.is_match(input),
        OutputLineData::Empty => false,
    }
}
```

The old `line_contains(&OutputLineData, &str)` helper can be **deleted** (no remaining caller).

- [ ] **Step 4: Run tests**

```bash
cargo test --bin websh core::commands::filters::tests
```

Expected: all filter tests pass, including the 8 new ones.

- [ ] **Step 5: Full test suite**

```bash
cargo test --bin websh
```

Expected: 163+ pass (156 baseline + 8 new), 4 pre-existing fail.

- [ ] **Step 6: Commit**

```bash
git add src/core/commands/filters.rs
git commit -m "feat(grep): regex + -i/-v/-E flags, exit 2 on bad regex/unknown flag"
```

---

## Task B.3: Strict numeric parsing for `head` / `tail`

**Files:**
- Modify: `src/core/commands/filters.rs`

- [ ] **Step 1: Write failing tests**

Append to `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn test_head_double_dash_rejected() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["--5"]), lines);
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_head_n_flag() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["-n", "3"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 3);
    }

    #[test]
    fn test_head_non_numeric_rejected() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["-abc"]), lines);
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_head_default_no_args() {
        let lines = test_lines();
        let result = apply_filter("head", &[], lines);
        // default DEFAULT_HEAD_LINES = 10, test_lines has 5
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 5);
    }

    #[test]
    fn test_tail_double_dash_rejected() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["--2"]), lines);
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_tail_n_flag() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["-n", "2"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 2);
    }
```

Update the pre-existing `test_head_with_dash` and `test_tail_with_dash` — their assertion that `-5` works still holds, but note `parse_count_arg` semantics change may subtly break them. Run and see.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test --bin websh core::commands::filters::tests
```

Expected: new tests fail.

- [ ] **Step 3: Replace `parse_count_arg` and update `filter_head`/`filter_tail`**

Replace the existing `parse_count_arg`, `filter_head`, `filter_tail`:

```rust
fn filter_head(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let n = match parse_count(args, pipe_filters::DEFAULT_HEAD_LINES) {
        Ok(n) => n,
        Err(msg) => {
            return CommandResult::error_line(format!("head: {}", msg)).with_exit_code(2);
        }
    };
    CommandResult::output(lines.into_iter().take(n).collect())
}

fn filter_tail(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let n = match parse_count(args, pipe_filters::DEFAULT_TAIL_LINES) {
        Ok(n) => n,
        Err(msg) => {
            return CommandResult::error_line(format!("tail: {}", msg)).with_exit_code(2);
        }
    };
    let len = lines.len();
    CommandResult::output(lines.into_iter().skip(len.saturating_sub(n)).collect())
}

/// Parse the count argument for head/tail.
///
/// Supports:
/// - No args: returns `default`.
/// - `-N` where N is a non-negative integer (e.g., `-5`).
/// - `-n N` where N is a non-negative integer (e.g., `-n 5`).
///
/// Rejects:
/// - `--N`, `---N`, etc.
/// - Non-numeric: `-abc`, `abc`.
/// - Unknown flags.
fn parse_count(args: &[String], default: usize) -> Result<usize, String> {
    match args.len() {
        0 => Ok(default),
        1 => {
            let arg = &args[0];
            // `-n` alone is incomplete; fallthrough to error below
            if arg == "-n" {
                return Err("option requires an argument: -n".to_string());
            }
            // Bulk reject any double-dash prefix
            if arg.starts_with("--") {
                return Err(format!("unknown option: {}", arg));
            }
            if let Some(rest) = arg.strip_prefix('-') {
                // must be `-N` where N is integer
                rest.parse::<usize>()
                    .map_err(|_| format!("invalid option: -{}", rest))
            } else {
                // bare positional like "5" is not POSIX but also not accepted
                Err(format!("unexpected argument: {}", arg))
            }
        }
        2 => {
            if args[0] == "-n" {
                args[1]
                    .parse::<usize>()
                    .map_err(|_| format!("invalid number: {}", args[1]))
            } else {
                Err(format!("unknown options: {} {}", args[0], args[1]))
            }
        }
        _ => Err("too many arguments".to_string()),
    }
}
```

Delete the old `parse_count_arg` (no callers remain).

- [ ] **Step 4: Run tests**

```bash
cargo test --bin websh core::commands::filters::tests
```

Expected: all filter tests pass, including new strict-parsing tests. Existing `test_head_with_dash` and `test_tail_with_dash` should still pass (`-5`, `-2`, `-3` are valid short forms).

- [ ] **Step 5: Full test suite**

```bash
cargo test --bin websh
```

Expected: 169+ pass (163 post-B.2 + 6 new), 4 pre-existing fail.

- [ ] **Step 6: Commit**

```bash
git add src/core/commands/filters.rs
git commit -m "fix(filters): strict head/tail numeric parsing, reject --N / non-numeric"
```

---

## Self-Review Checklist

- [x] Spec coverage: H5 → Task B.2; M6 → Task B.3. H4 explicitly deferred with rationale.
- [x] No placeholders: every code block is complete.
- [x] Type consistency: `apply_filter` signature unchanged; `CommandResult` and exit codes match Phase 1.
- [x] Test-first: each task starts with failing tests.

## Done Criteria

- `cargo test --bin websh`: ~170 pass (156 baseline + 8 grep + 6 head/tail), 4 pre-existing fail.
- `cargo build --release --target wasm32-unknown-unknown`: clean.
- `Cargo.toml` has `regex = "1"` in `[dependencies]`.
- `grep` supports `-i`, `-v`, `-E`, short-flag combining.
- `head`/`tail` reject `--N`, `---N`, non-numeric, and unknown flags.
- H4 deferral noted in master decision log.
