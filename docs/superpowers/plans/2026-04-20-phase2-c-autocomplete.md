# Phase 2 Track C: Autocomplete — Implementation Plan

**Goal:** Fix the UTF-8 boundary panic in `find_common_prefix` and remove the stale `less`/`more` entries from `FILE_COMMANDS`.

**Addresses:** H8 (UTF-8 char-vs-byte slicing bug), M12 (autocomplete lists commands that don't actually exist).

## Scope

- `find_common_prefix` slices by bytes with char-count — panics on multi-byte input. Fix to char-based slicing.
- `FILE_COMMANDS` contains `"less"`/`"more"` but `Command::parse` treats them as `Unknown(127)`. Remove them so autocomplete doesn't advertise nonfunctional commands.

## File Structure

| Path | Action |
|---|---|
| `src/core/autocomplete.rs` | Modify |

---

## Task C.1: Remove `less` / `more` from FILE_COMMANDS

- [ ] **Step 1: Test**

Add to `src/core/autocomplete.rs::tests`:

```rust
    #[test]
    fn test_completion_mode_less_no_longer_file() {
        // less is not an implemented command; it should not trigger file-path completion
        let (mode, _) = CompletionMode::from_input("less file.txt");
        assert_eq!(mode, CompletionMode::None);
    }

    #[test]
    fn test_completion_mode_more_no_longer_file() {
        let (mode, _) = CompletionMode::from_input("more file.txt");
        assert_eq!(mode, CompletionMode::None);
    }
```

- [ ] **Step 2: Verify failure**

```bash
cargo test --bin websh core::autocomplete
```

- [ ] **Step 3: Remove entries**

Change `FILE_COMMANDS` in `src/core/autocomplete.rs`:

```rust
/// Commands that accept file paths as arguments.
const FILE_COMMANDS: &[&str] = &["cat"];
```

Update the doc comment at the top of the module (`//! - File paths for \`cat\`, \`less\`, \`more\` commands`) to drop the `less`, `more` mentions:

```rust
//! - File paths for `cat` commands
```

- [ ] **Step 4: Verify pass**

```bash
cargo test --bin websh core::autocomplete
```

- [ ] **Step 5: Commit**

```bash
git add src/core/autocomplete.rs
git commit -m "fix(autocomplete): drop less/more from FILE_COMMANDS (not implemented)"
```

---

## Task C.2: UTF-8-safe common prefix

- [ ] **Step 1: Test**

Add to `src/core/autocomplete.rs::tests`:

```rust
    #[test]
    fn test_common_prefix_multibyte() {
        // Korean characters (3 bytes each in UTF-8)
        let strings = vec![
            "한국어".to_string(),
            "한국인".to_string(),
        ];
        assert_eq!(find_common_prefix(&strings), "한국");
    }

    #[test]
    fn test_common_prefix_emoji() {
        // Emoji (4-byte sequences)
        let strings = vec![
            "café_1".to_string(),
            "café_2".to_string(),
        ];
        assert_eq!(find_common_prefix(&strings), "café_");
    }

    #[test]
    fn test_common_prefix_mixed_ascii_multibyte() {
        let strings = vec![
            "abc한".to_string(),
            "abc中".to_string(),
        ];
        assert_eq!(find_common_prefix(&strings), "abc");
    }

    #[test]
    fn test_common_prefix_no_common() {
        let strings = vec!["한".to_string(), "中".to_string()];
        assert_eq!(find_common_prefix(&strings), "");
    }
```

- [ ] **Step 2: Verify failure**

Currently the function will panic (byte-boundary slicing) OR return corrupted output. Run and inspect.

```bash
cargo test --bin websh core::autocomplete::tests::test_common_prefix_multibyte
```

Expected: panic at `first[..prefix_len]` because `prefix_len` is in chars.

- [ ] **Step 3: Fix with char-based slicing**

Replace `find_common_prefix` in `src/core/autocomplete.rs`:

```rust
/// Find the common prefix of multiple strings (case-insensitive).
///
/// Operates on Unicode codepoints (chars), not bytes — safe for multi-byte UTF-8.
fn find_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    if strings.len() == 1 {
        return strings[0].clone();
    }

    let first = &strings[0];
    let mut prefix_chars = first.chars().count();

    for s in &strings[1..] {
        let matching = first
            .chars()
            .zip(s.chars())
            .take(prefix_chars)
            .take_while(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
            .count();
        prefix_chars = matching;
        if prefix_chars == 0 {
            break;
        }
    }

    first.chars().take(prefix_chars).collect()
}
```

The key change: instead of `first[..prefix_len]` (byte slice with a char count), use `first.chars().take(prefix_chars).collect()`.

- [ ] **Step 4: Verify pass**

```bash
cargo test --bin websh core::autocomplete
```

Expected: all tests pass, including multi-byte.

- [ ] **Step 5: Full suite**

```bash
cargo test --bin websh
```

Expected: 184-ish pass / 4 pre-existing fail (180 + 2 less/more + 4 multibyte = +6).

- [ ] **Step 6: Commit**

```bash
git add src/core/autocomplete.rs
git commit -m "fix(autocomplete): char-based common_prefix — no more UTF-8 panic"
```

---

## Done Criteria

- `cargo test --bin websh`: ~186 pass / 4 pre-existing fail.
- `cargo build --release --target wasm32-unknown-unknown`: clean.
- Typing `한` and tab-completing against `["한국어", "한국인"]` doesn't panic.
- Typing `less ` or `more ` doesn't trigger file-path completion.
