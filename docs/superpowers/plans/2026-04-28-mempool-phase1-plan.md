# Mempool Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the read-only mempool — a section above the chain on `/ledger` that renders pending entries fetched from the `0xwonj/websh-mempool` GitHub repo, with category filter integration and modal preview on click.

**Architecture:** A new `Mempool` Leptos component reads files from a runtime mount at `/mempool` (declared via `content/.websh/mounts/mempool.mount.json`, no code change to mount machinery). Frontmatter is parsed from each file; the resulting `MempoolEntry` rows render via a CSS-grid layout that mirrors `ledger.html`'s mempool aesthetic. Click opens a modal containing the existing `Reader` component.

**Tech Stack:** Rust + Leptos 0.8 (csr), wasm32 target, stylance CSS modules, GitHub raw HTTP read path, axum/Playwright already in tree for tests.

**Master plan:** [`docs/superpowers/specs/2026-04-28-mempool-master.md`](../specs/2026-04-28-mempool-master.md)
**Phase 1 design:** [`docs/superpowers/specs/2026-04-28-mempool-phase1-design.md`](../specs/2026-04-28-mempool-phase1-design.md)

---

## Prerequisites (manual, before coding)

These actions happen outside the codebase. Confirm complete before starting Task 1.

- [ ] **PR-1: Create the mempool repo**
  - On GitHub, create a new public repo `0xwonj/websh-mempool`. Branch: `main`.
  - Push at least 4 seed entries: one `writing/*.md`, one `projects/*.md`, one `papers/*.md`, one `talks/*.md`. Each file uses the frontmatter schema in §5.1 of the Phase 1 design doc — include at least one `status: review` and at least one `priority: high` so the rendering shows the differentiated states.

- [ ] **PR-2: Confirm `cargo`, `trunk`, and Playwright work**
  - `cargo test --lib` → green
  - `cargo check --target wasm32-unknown-unknown --lib` → green
  - `trunk --version` → reports a version
  - `target/qa/node_modules/.bin/playwright --version` (or run `just qa-install` first) → reports a version

---

## Task 1: Lift `iso_date_prefix` from CLI to shared utils

**Why:** Phase 1's frontmatter parser needs ISO-date validation for the `modified` field. The same helper already exists in `src/cli/ledger.rs` but it's CLI-only. Move it to `src/utils/format.rs` so wasm-targeted code can share it. (Master §3 anchor A8: keep CLI work to a minimum.)

**Files:**
- Modify: `src/utils/format.rs` (add `iso_date_prefix`)
- Modify: `src/cli/ledger.rs` (remove local copy; import from utils)

- [ ] **Step 1.1: Add the failing test in `src/utils/format.rs`**

Append to the bottom of `src/utils/format.rs` (inside or alongside any existing `#[cfg(test)] mod tests`; if no `tests` module exists, add one):

```rust
#[cfg(test)]
mod iso_date_prefix_tests {
    use super::*;

    #[test]
    fn iso_date_prefix_accepts_canonical_iso() {
        assert_eq!(iso_date_prefix("2026-04-22"), Some("2026-04-22"));
    }

    #[test]
    fn iso_date_prefix_accepts_iso_with_time_suffix() {
        assert_eq!(iso_date_prefix("2026-04-22T12:00:00Z"), Some("2026-04-22"));
    }

    #[test]
    fn iso_date_prefix_rejects_non_iso() {
        assert_eq!(iso_date_prefix(""), None);
        assert_eq!(iso_date_prefix("undated"), None);
        assert_eq!(iso_date_prefix("Apr 22, 2026"), None);
        assert_eq!(iso_date_prefix("2026/04/22"), None);
        assert_eq!(iso_date_prefix("2026-4-22"), None);
        assert_eq!(iso_date_prefix("20260422"), None);
    }
}
```

- [ ] **Step 1.2: Run the test — confirm it fails to compile**

```bash
cargo test --lib utils::format::iso_date_prefix_tests
```

Expected: `cannot find function 'iso_date_prefix' in this scope`.

- [ ] **Step 1.3: Implement `iso_date_prefix` in `src/utils/format.rs`**

Add this function at module scope (above any existing `#[cfg(test)] mod tests`):

```rust
/// If `value` begins with a 10-character `YYYY-MM-DD` prefix, return that
/// prefix as a borrowed slice. Otherwise return `None`. Used as a low-cost
/// sortable key for content dates.
pub fn iso_date_prefix(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.len() >= 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(|byte| byte.is_ascii_digit())
        && bytes[5..7].iter().all(|byte| byte.is_ascii_digit())
        && bytes[8..10].iter().all(|byte| byte.is_ascii_digit())
    {
        Some(&value[..10])
    } else {
        None
    }
}
```

- [ ] **Step 1.4: Run the test — confirm it passes**

```bash
cargo test --lib utils::format::iso_date_prefix_tests
```

Expected: 3 passed.

- [ ] **Step 1.5: Replace the duplicate in `src/cli/ledger.rs`**

In `src/cli/ledger.rs`:
1. Delete the local `fn iso_date_prefix(value: &str) -> Option<&str> { ... }` definition.
2. Delete the local `iso_date_prefix_recognizes_iso_strings` test (its content is covered by the lifted tests).
3. Add `use crate::utils::format::iso_date_prefix;` near the existing `use` statements.

- [ ] **Step 1.6: Run all CLI ledger tests — confirm green**

```bash
cargo test --lib cli::ledger::tests
```

Expected: all existing tests pass (the local `iso_date_prefix` test is removed, but the canonical-sort tests still exercise the function via the sort path).

- [ ] **Step 1.7: Run the full lib test suite as a sanity check**

```bash
cargo test --lib
```

Expected: all green.

- [ ] **Step 1.8: Commit**

```bash
git add src/utils/format.rs src/cli/ledger.rs
git commit -m "refactor(utils): lift iso_date_prefix from cli to utils::format

Phase 1 of the mempool feature shares ISO-date validation with the
existing CLI ledger sort. Moving it to utils makes it available to
wasm-targeted code in src/components/mempool."
```

---

## Task 2: Add the mempool mount declaration

**Why:** The runtime mount loader (`src/core/runtime/loader.rs::load_mount_declarations`) already discovers `*.mount.json` files at `/.websh/mounts/`. Adding one configures the GitHub backend at `/mempool` with no code change.

**Files:**
- Create: `content/.websh/mounts/mempool.mount.json`

- [ ] **Step 2.1: Create the mount declaration**

```bash
mkdir -p content/.websh/mounts
```

Then write `content/.websh/mounts/mempool.mount.json`:

```json
{
  "backend": "github",
  "mount_at": "/mempool",
  "repo": "0xwonj/websh-mempool",
  "branch": "main",
  "root": "",
  "name": "mempool",
  "writable": true
}
```

- [ ] **Step 2.2: Verify build still compiles for wasm**

```bash
cargo check --target wasm32-unknown-unknown --lib
```

Expected: clean (the mount declaration is data — no compilation impact, but this catches collateral breakage).

- [ ] **Step 2.3: Commit**

```bash
git add content/.websh/mounts/mempool.mount.json
git commit -m "feat(mounts): declare /mempool mount targeting websh-mempool repo

Mempool repo is mounted at /mempool (no /mnt prefix). Discovered at
runtime by load_mount_declarations; no code change required."
```

---

## Task 3: Add mempool data types

**Why:** The component, parsers, and tests all reference these types. Defining them up front lets later tasks compile against a stable surface.

**Files:**
- Create: `src/components/mempool/mod.rs` (skeleton with public re-exports)
- Create: `src/components/mempool/model.rs` (data types)
- Modify: `src/components/mod.rs` (declare the new module)

- [ ] **Step 3.1: Create the module directory + skeleton `mod.rs`**

```bash
mkdir -p src/components/mempool
```

Write `src/components/mempool/mod.rs`:

```rust
//! Mempool — pending content entries displayed above the chain on /ledger.

mod model;

pub use model::{MempoolEntry, MempoolModel, MempoolStatus, Priority};
```

- [ ] **Step 3.2: Create `src/components/mempool/model.rs`**

```rust
//! Mempool data model: entries, statuses, priorities, and the rendered model
//! that the `Mempool` component consumes.

use std::collections::BTreeMap;

use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::models::VirtualPath;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MempoolModel {
    pub filter: LedgerFilterShape,
    pub entries: Vec<MempoolEntry>,
    pub total_count: usize,
    pub counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MempoolEntry {
    pub path: VirtualPath,
    pub title: String,
    pub desc: String,
    pub status: MempoolStatus,
    pub priority: Option<Priority>,
    pub kind: String,
    pub category: String,
    pub modified: String,
    pub sort_key: Option<String>,
    pub gas: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MempoolStatus {
    Draft,
    Review,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Med,
    High,
}

/// Mirror of `LedgerFilter` used by the chain page, scoped to mempool needs.
/// We do not import `LedgerFilter` directly because it is a private item of
/// `ledger_page.rs`; copying the shape here keeps the mempool independently
/// testable. See Task 5 for the conversion at the call site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerFilterShape {
    All,
    Category(String),
}

impl LedgerFilterShape {
    pub fn includes(&self, entry: &MempoolEntry) -> bool {
        match self {
            Self::All => true,
            Self::Category(category) if LEDGER_CATEGORIES.contains(&category.as_str()) => {
                entry.category == *category
            }
            Self::Category(category) => entry.path.as_str().contains(&format!("/{category}/")),
        }
    }
}
```

- [ ] **Step 3.3: Add the module to `src/components/mod.rs`**

Add a single line near the other component module declarations:

```rust
pub mod mempool;
```

- [ ] **Step 3.4: Compile-check**

```bash
cargo check --target wasm32-unknown-unknown --lib
```

Expected: clean (warnings about unused types are acceptable at this point — they will be used in later tasks).

- [ ] **Step 3.5: Commit**

```bash
git add src/components/mempool/ src/components/mod.rs
git commit -m "feat(mempool): add data types (MempoolEntry, MempoolModel, ...)

Defines the surface that frontmatter parsers, model builders, and the
Mempool component will operate on in subsequent tasks."
```

---

## Task 4: Frontmatter parsing

**Why:** Each mempool file has YAML-like frontmatter (the same dialect as `cli/manifest.rs::parse_frontmatter`). Phase 1 needs a parser that yields a structured intermediate representation.

**Files:**
- Create: `src/components/mempool/parse.rs`
- Modify: `src/components/mempool/mod.rs` (declare module + re-export selected helpers)

- [ ] **Step 4.1: Add the failing test in `src/components/mempool/parse.rs`**

Create `src/components/mempool/parse.rs` with the *test module first* (TDD):

```rust
//! Frontmatter parsing and auto-derivation helpers for mempool entries.

#[cfg(test)]
mod tests {
    use super::*;

    fn body(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn parses_full_frontmatter() {
        let raw = body(
            "---\n\
             title: \"On writing slow\"\n\
             status: draft\n\
             priority: med\n\
             modified: \"2026-04-25\"\n\
             tags: [essay, writing-process]\n\
             ---\n\
             # On writing slow\n\nbody...\n",
        );
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.title.as_deref(), Some("On writing slow"));
        assert_eq!(meta.status.as_deref(), Some("draft"));
        assert_eq!(meta.priority.as_deref(), Some("med"));
        assert_eq!(meta.modified.as_deref(), Some("2026-04-25"));
        assert_eq!(meta.tags, vec!["essay".to_string(), "writing-process".to_string()]);
    }

    #[test]
    fn parses_minimal_frontmatter() {
        let raw = body("---\ntitle: foo\nstatus: draft\nmodified: 2026-04-22\n---\nbody\n");
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.title.as_deref(), Some("foo"));
        assert_eq!(meta.status.as_deref(), Some("draft"));
        assert!(meta.priority.is_none());
        assert_eq!(meta.modified.as_deref(), Some("2026-04-22"));
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn returns_none_when_no_frontmatter_fence() {
        assert!(parse_mempool_frontmatter("# title\nbody\n").is_none());
    }

    #[test]
    fn returns_none_for_empty_input() {
        assert!(parse_mempool_frontmatter("").is_none());
    }

    #[test]
    fn ignores_unknown_keys() {
        let raw = body("---\ntitle: foo\nstatus: draft\nmodified: 2026-04-22\nfuture: ignore\n---\n");
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.title.as_deref(), Some("foo"));
    }
}
```

- [ ] **Step 4.2: Run the test — confirm it fails to compile**

```bash
cargo test --lib components::mempool::parse::tests
```

Expected: `cannot find function 'parse_mempool_frontmatter' in this scope`.

- [ ] **Step 4.3: Implement `parse_mempool_frontmatter`**

Add to `src/components/mempool/parse.rs` *above* the test module:

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawMempoolMeta {
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub modified: Option<String>,
    pub tags: Vec<String>,
}

/// Parse mempool-file frontmatter. Returns `None` when the input does not
/// open with a `---` fence (i.e., the file has no frontmatter and we should
/// skip it). Unknown keys are ignored; values are read as raw strings.
pub fn parse_mempool_frontmatter(body: &str) -> Option<RawMempoolMeta> {
    let mut lines = body.lines();
    if lines.next() != Some("---") {
        return None;
    }

    let mut meta = RawMempoolMeta::default();
    for line in lines {
        if line == "---" {
            return Some(meta);
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key {
            "title" => meta.title = Some(value.to_string()),
            "status" => meta.status = Some(value.to_string()),
            "priority" => meta.priority = Some(value.to_string()),
            "modified" => meta.modified = Some(value.to_string()),
            "tags" => meta.tags = parse_inline_tags(value),
            _ => {}
        }
    }
    Some(meta)
}

fn parse_inline_tags(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    {
        return inner
            .split(',')
            .map(|tag| tag.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|tag| !tag.is_empty())
            .collect();
    }
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_string()]
    }
}
```

- [ ] **Step 4.4: Wire the new module into `src/components/mempool/mod.rs`**

Replace `src/components/mempool/mod.rs` with:

```rust
//! Mempool — pending content entries displayed above the chain on /ledger.

mod model;
mod parse;

pub use model::{LedgerFilterShape, MempoolEntry, MempoolModel, MempoolStatus, Priority};
pub use parse::{RawMempoolMeta, parse_mempool_frontmatter};
```

- [ ] **Step 4.5: Run tests — confirm green**

```bash
cargo test --lib components::mempool::parse::tests
```

Expected: 5 passed.

- [ ] **Step 4.6: Commit**

```bash
git add src/components/mempool/parse.rs src/components/mempool/mod.rs
git commit -m "feat(mempool): add frontmatter parser

Mempool files declare {title, status, priority, modified, tags} via the
same YAML-flavored frontmatter as content/. Parser returns a
RawMempoolMeta intermediate representation; later tasks normalize it
into MempoolEntry."
```

---

## Task 5: Auxiliary parsers — status, priority, gas, paragraph, category

**Why:** Each `MempoolEntry` field that comes from the file's body or path needs its own helper. Implementing them as pure functions keeps Task 6 (the model builder) thin.

**Files:**
- Modify: `src/components/mempool/parse.rs` (add helpers + tests)

- [ ] **Step 5.1: Add failing tests for status, priority, paragraph, category, gas**

Append to the existing `#[cfg(test)] mod tests` block in `src/components/mempool/parse.rs`:

```rust
    #[test]
    fn parses_status_canonical_values() {
        assert_eq!(parse_mempool_status("draft"), Some(MempoolStatus::Draft));
        assert_eq!(parse_mempool_status("review"), Some(MempoolStatus::Review));
        assert!(parse_mempool_status("published").is_none());
        assert!(parse_mempool_status("DRAFT").is_none());
        assert!(parse_mempool_status("").is_none());
    }

    #[test]
    fn parses_priority_canonical_values() {
        assert_eq!(parse_priority("low"), Some(Priority::Low));
        assert_eq!(parse_priority("med"), Some(Priority::Med));
        assert_eq!(parse_priority("high"), Some(Priority::High));
        assert!(parse_priority("medium").is_none());
        assert!(parse_priority("").is_none());
    }

    #[test]
    fn extracts_first_paragraph_skipping_heading() {
        let body = "# Title\n\nFirst paragraph here.\nStill same paragraph.\n\nSecond para.\n";
        assert_eq!(
            extract_first_paragraph(body),
            "First paragraph here. Still same paragraph."
        );
    }

    #[test]
    fn extracts_first_paragraph_with_no_heading() {
        let body = "Standalone para.\n\nAnother.\n";
        assert_eq!(extract_first_paragraph(body), "Standalone para.");
    }

    #[test]
    fn extracts_first_paragraph_truncates_long_text() {
        let long = "x".repeat(200);
        let body = format!("{long}\n");
        let out = extract_first_paragraph(&body);
        assert!(out.len() <= 143, "got len={} body={}", out.len(), out);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn category_for_path_uses_first_segment_under_mempool() {
        use crate::models::VirtualPath;
        let path = VirtualPath::from_absolute("/mempool/writing/foo.md").unwrap();
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        assert_eq!(category_for_mempool_path(&path, &mempool_root), "writing");
    }

    #[test]
    fn category_for_path_handles_root_level_files() {
        use crate::models::VirtualPath;
        let path = VirtualPath::from_absolute("/mempool/loose.md").unwrap();
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        assert_eq!(category_for_mempool_path(&path, &mempool_root), "misc");
    }

    #[test]
    fn category_for_path_handles_nested_paths() {
        use crate::models::VirtualPath;
        let path = VirtualPath::from_absolute("/mempool/papers/series/foo.md").unwrap();
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        assert_eq!(category_for_mempool_path(&path, &mempool_root), "papers");
    }

    #[test]
    fn derives_gas_for_markdown_word_count() {
        let body = "---\nfront: matter\n---\n# Heading\n\n".to_string()
            + &"word ".repeat(420);
        let gas = derive_gas(&body, body.len(), true);
        // 420 words → "~400 words" (rounded to nearest 100 below 1000)
        assert_eq!(gas, "~400 words");
    }

    #[test]
    fn derives_gas_for_binary_uses_size() {
        let gas = derive_gas("", 12_400, false);
        assert!(gas.contains("12") || gas.contains("kB") || gas.contains("KB"));
    }

    #[test]
    fn derives_gas_for_empty_markdown() {
        let gas = derive_gas("---\n---\n", 8, true);
        assert_eq!(gas, "~0 words");
    }
```

- [ ] **Step 5.2: Run tests — confirm they fail to compile**

```bash
cargo test --lib components::mempool::parse::tests
```

Expected: errors about missing `parse_mempool_status`, `parse_priority`, `extract_first_paragraph`, `category_for_mempool_path`, `derive_gas`.

- [ ] **Step 5.3: Implement the helpers**

Add to `src/components/mempool/parse.rs` *above* the existing `#[cfg(test)] mod tests`:

```rust
use crate::models::VirtualPath;
use crate::utils::format::format_size;

use super::model::{MempoolStatus, Priority};

const DESC_MAX_CHARS: usize = 140;

pub fn parse_mempool_status(value: &str) -> Option<MempoolStatus> {
    match value {
        "draft" => Some(MempoolStatus::Draft),
        "review" => Some(MempoolStatus::Review),
        _ => None,
    }
}

pub fn parse_priority(value: &str) -> Option<Priority> {
    match value {
        "low" => Some(Priority::Low),
        "med" => Some(Priority::Med),
        "high" => Some(Priority::High),
        _ => None,
    }
}

/// First non-heading paragraph from a markdown body. Skips `# ...` headings
/// at the top, joins continuation lines with a single space, truncates at
/// `DESC_MAX_CHARS` with an ellipsis.
pub fn extract_first_paragraph(body: &str) -> String {
    let mut lines = body
        .lines()
        .skip_while(|line| line.is_empty() || line.starts_with('#'));

    let mut paragraph = String::new();
    for line in lines.by_ref() {
        if line.trim().is_empty() {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line.trim());
    }

    if paragraph.chars().count() > DESC_MAX_CHARS {
        let truncated: String = paragraph.chars().take(DESC_MAX_CHARS).collect();
        format!("{}…", truncated.trim_end())
    } else {
        paragraph
    }
}

/// First path segment beneath `mempool_root`. Returns `"misc"` for files that
/// live directly under `mempool_root` (no category folder).
pub fn category_for_mempool_path(path: &VirtualPath, mempool_root: &VirtualPath) -> String {
    let path_str = path.as_str();
    let prefix = mempool_root.as_str();
    let rel = path_str
        .strip_prefix(prefix)
        .unwrap_or(path_str)
        .trim_start_matches('/');
    let mut segments = rel.split('/');
    let first = segments.next().unwrap_or("");
    if segments.next().is_none() {
        return "misc".to_string();
    }
    if first.is_empty() {
        "misc".to_string()
    } else {
        first.to_string()
    }
}

/// "Gas" — a vibe metric of entry size. Markdown gets a rounded word count;
/// binaries get `format_size`.
pub fn derive_gas(body: &str, byte_len: usize, is_markdown: bool) -> String {
    if !is_markdown {
        return format_size(Some(byte_len as u64), false);
    }
    let body_after_frontmatter = strip_frontmatter(body);
    let word_count = body_after_frontmatter.split_whitespace().count();
    let bucket = if word_count < 100 {
        word_count.next_multiple_of(10).saturating_sub(0)
    } else if word_count < 1000 {
        word_count.next_multiple_of(50).saturating_sub(0)
    } else {
        word_count.next_multiple_of(100).saturating_sub(0)
    };
    let rounded = match word_count {
        0..=99 => word_count - (word_count % 10),
        100..=999 => word_count - (word_count % 50),
        _ => word_count - (word_count % 100),
    };
    let _ = bucket; // bucket value is illustrative; rounded is the canonical output
    format_with_thousands(rounded)
}

fn strip_frontmatter(body: &str) -> &str {
    let mut iter = body.splitn(3, "---\n");
    match (iter.next(), iter.next(), iter.next()) {
        (Some(empty), Some(_meta), Some(rest)) if empty.is_empty() => rest,
        _ => body,
    }
}

fn format_with_thousands(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    for (i, byte) in bytes.iter().enumerate() {
        let from_end = bytes.len() - i;
        if i > 0 && from_end % 3 == 0 {
            out.push(',');
        }
        out.push(*byte as char);
    }
    format!("~{out} words")
}
```

> Note: the `bucket` line is an artifact of an earlier iteration; the
> canonical output uses `rounded`. The engineer is free to remove the
> `let _ = bucket;` line plus its surrounding `let bucket = ...;` block —
> all logic is in `rounded`.

- [ ] **Step 5.4: Run tests — confirm green**

```bash
cargo test --lib components::mempool::parse::tests
```

Expected: 14 passed (5 from Task 4 + 9 added here).

- [ ] **Step 5.5: Commit**

```bash
git add src/components/mempool/parse.rs
git commit -m "feat(mempool): add status/priority/paragraph/category/gas helpers

Pure helpers for converting raw frontmatter strings and path segments into
the typed fields a MempoolEntry exposes. Each helper is independently
testable; the model builder in Task 6 just composes them."
```

---

## Task 6: Build the mempool model (sync builder + integration test)

**Why:** Given a populated `GlobalFs` and a list of `(path, body, byte_len)` tuples for mempool files, produce a `MempoolModel`. This is the seam between async fetching (Task 7) and rendering.

**Files:**
- Modify: `src/components/mempool/model.rs` (add `build_mempool_model`)
- Create: `tests/mempool_model.rs` (integration test)

- [ ] **Step 6.1: Add a unit test inline in `model.rs`**

Append to `src/components/mempool/model.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::mempool::parse::*;

    fn meta(status: &str, modified: &str, priority: Option<&str>) -> RawMempoolMeta {
        RawMempoolMeta {
            title: Some("untitled".to_string()),
            status: Some(status.to_string()),
            priority: priority.map(str::to_string),
            modified: Some(modified.to_string()),
            tags: vec![],
        }
    }

    fn loaded(
        path: &str,
        meta: RawMempoolMeta,
        body: &str,
        byte_len: usize,
        is_markdown: bool,
    ) -> LoadedMempoolFile {
        LoadedMempoolFile {
            path: VirtualPath::from_absolute(path).unwrap(),
            meta,
            body: body.to_string(),
            byte_len,
            is_markdown,
        }
    }

    #[test]
    fn build_model_orders_by_modified_desc() {
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        let files = vec![
            loaded("/mempool/writing/old.md", meta("draft", "2026-03-01", None), "old", 3, true),
            loaded("/mempool/writing/new.md", meta("draft", "2026-04-01", None), "new", 3, true),
            loaded("/mempool/writing/mid.md", meta("review", "2026-03-15", Some("med")), "mid", 3, true),
        ];
        let model = build_mempool_model(&mempool_root, files, &LedgerFilterShape::All);
        assert_eq!(model.entries.len(), 3);
        assert_eq!(model.entries[0].path.as_str(), "/mempool/writing/new.md");
        assert_eq!(model.entries[1].path.as_str(), "/mempool/writing/mid.md");
        assert_eq!(model.entries[2].path.as_str(), "/mempool/writing/old.md");
        assert_eq!(model.total_count, 3);
        assert_eq!(model.counts.get("writing").copied(), Some(3));
    }

    #[test]
    fn build_model_filters_by_category() {
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        let files = vec![
            loaded("/mempool/writing/a.md", meta("draft", "2026-04-01", None), "a", 1, true),
            loaded("/mempool/papers/b.md", meta("draft", "2026-04-02", None), "b", 1, true),
        ];
        let model = build_mempool_model(
            &mempool_root,
            files,
            &LedgerFilterShape::Category("writing".to_string()),
        );
        assert_eq!(model.entries.len(), 1);
        assert_eq!(model.entries[0].category, "writing");
        // total_count and counts reflect the unfiltered set
        assert_eq!(model.total_count, 2);
        assert_eq!(model.counts.get("writing").copied(), Some(1));
        assert_eq!(model.counts.get("papers").copied(), Some(1));
    }

    #[test]
    fn build_model_treats_undated_as_lowest_priority_sort() {
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        let files = vec![
            loaded(
                "/mempool/writing/dated.md",
                meta("draft", "2026-04-01", None),
                "x",
                1,
                true,
            ),
            loaded(
                "/mempool/writing/undated.md",
                RawMempoolMeta {
                    title: Some("u".into()),
                    status: Some("draft".into()),
                    modified: None,
                    ..Default::default()
                },
                "y",
                1,
                true,
            ),
        ];
        let model = build_mempool_model(&mempool_root, files, &LedgerFilterShape::All);
        assert_eq!(model.entries.len(), 2);
        assert_eq!(model.entries[0].path.as_str(), "/mempool/writing/dated.md");
        assert_eq!(model.entries[1].path.as_str(), "/mempool/writing/undated.md");
    }
}
```

- [ ] **Step 6.2: Run the test — confirm it fails to compile**

```bash
cargo test --lib components::mempool::model::tests
```

Expected: `cannot find type 'LoadedMempoolFile'`, `cannot find function 'build_mempool_model'`.

- [ ] **Step 6.3: Implement `LoadedMempoolFile` and `build_mempool_model`**

Append to `src/components/mempool/model.rs` *above* the test module:

```rust
use crate::components::mempool::parse::{
    RawMempoolMeta, category_for_mempool_path, derive_gas, extract_first_paragraph,
    parse_mempool_status, parse_priority,
};
use crate::utils::format::iso_date_prefix;

const DEFAULT_TITLE_FALLBACK: &str = "untitled";

/// One file fetched from the mempool mount, ready to feed `build_mempool_model`.
#[derive(Clone, Debug)]
pub struct LoadedMempoolFile {
    pub path: VirtualPath,
    pub meta: RawMempoolMeta,
    pub body: String,
    pub byte_len: usize,
    pub is_markdown: bool,
}

pub fn build_mempool_model(
    mempool_root: &VirtualPath,
    files: Vec<LoadedMempoolFile>,
    filter: &LedgerFilterShape,
) -> MempoolModel {
    let mut all = files
        .into_iter()
        .filter_map(|file| build_entry(mempool_root, file))
        .collect::<Vec<_>>();

    let mut counts = BTreeMap::new();
    for entry in &all {
        *counts.entry(entry.category.clone()).or_default() += 1;
    }
    let total_count = all.len();

    sort_entries(&mut all);

    let entries = all
        .iter()
        .filter(|entry| filter.includes(entry))
        .cloned()
        .collect::<Vec<_>>();

    MempoolModel {
        filter: filter.clone(),
        entries,
        total_count,
        counts,
    }
}

fn build_entry(mempool_root: &VirtualPath, file: LoadedMempoolFile) -> Option<MempoolEntry> {
    let LoadedMempoolFile {
        path,
        meta,
        body,
        byte_len,
        is_markdown,
    } = file;

    let title = meta
        .title
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TITLE_FALLBACK.to_string());
    let status = meta
        .status
        .as_deref()
        .and_then(parse_mempool_status)
        .unwrap_or(MempoolStatus::Draft);
    let priority = meta.priority.as_deref().and_then(parse_priority);
    let modified = meta.modified.clone().unwrap_or_else(|| "undated".to_string());
    let sort_key = meta.modified.as_deref().and_then(|raw| {
        iso_date_prefix(raw).map(|prefix| prefix.to_string())
    });
    let category = category_for_mempool_path(&path, mempool_root);
    let kind = kind_for_category(&category);
    let desc = extract_first_paragraph(&body);
    let gas = derive_gas(&body, byte_len, is_markdown);

    Some(MempoolEntry {
        path,
        title,
        desc,
        status,
        priority,
        kind,
        category,
        modified,
        sort_key,
        gas,
        tags: meta.tags,
    })
}

fn kind_for_category(category: &str) -> String {
    match category {
        "writing" => "writing",
        "projects" => "project",
        "papers" => "paper",
        "talks" => "talk",
        _ => "note",
    }
    .to_string()
}

fn sort_entries(entries: &mut Vec<MempoolEntry>) {
    entries.sort_by(|left, right| match (&left.sort_key, &right.sort_key) {
        (Some(left_key), Some(right_key)) => right_key
            .cmp(left_key)
            .then_with(|| left.path.as_str().cmp(right.path.as_str())),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.path.as_str().cmp(right.path.as_str()),
    });
}
```

Update `src/components/mempool/mod.rs` to re-export `LoadedMempoolFile` and `build_mempool_model`:

```rust
//! Mempool — pending content entries displayed above the chain on /ledger.

mod model;
mod parse;

pub use model::{
    LedgerFilterShape, LoadedMempoolFile, MempoolEntry, MempoolModel, MempoolStatus, Priority,
    build_mempool_model,
};
pub use parse::{RawMempoolMeta, parse_mempool_frontmatter};
```

- [ ] **Step 6.4: Run unit tests — confirm green**

```bash
cargo test --lib components::mempool
```

Expected: all parse + model tests green.

- [ ] **Step 6.5: Add a small integration test**

Create `tests/mempool_model.rs`:

```rust
//! Integration tests for the mempool model builder. Exercises the same code
//! paths the runtime would, with synthesized inputs.

use websh::components::mempool::{
    LedgerFilterShape, LoadedMempoolFile, build_mempool_model, parse_mempool_frontmatter,
};
use websh::models::VirtualPath;

fn loaded(path: &str, body: &str) -> LoadedMempoolFile {
    let meta = parse_mempool_frontmatter(body).unwrap_or_default();
    LoadedMempoolFile {
        path: VirtualPath::from_absolute(path).unwrap(),
        meta,
        body: body.to_string(),
        byte_len: body.len(),
        is_markdown: true,
    }
}

#[test]
fn end_to_end_build_renders_mixed_categories() {
    let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
    let files = vec![
        loaded(
            "/mempool/writing/foo.md",
            "---\ntitle: foo\nstatus: draft\nmodified: 2026-04-01\n---\n# foo\n\nfoo body.\n",
        ),
        loaded(
            "/mempool/papers/bar.md",
            "---\ntitle: bar\nstatus: review\npriority: high\nmodified: 2026-04-02\n---\n# bar\n\nbar body.\n",
        ),
        loaded(
            "/mempool/talks/baz.md",
            "---\ntitle: baz\nstatus: draft\nmodified: 2026-03-10\n---\n# baz\n\nbaz body.\n",
        ),
    ];

    let model = build_mempool_model(&mempool_root, files, &LedgerFilterShape::All);
    assert_eq!(model.total_count, 3);
    assert_eq!(model.entries.len(), 3);
    assert_eq!(model.entries[0].path.as_str(), "/mempool/papers/bar.md");
    assert_eq!(model.entries[0].priority.unwrap() as u8, 2); // High
    assert_eq!(model.entries[1].path.as_str(), "/mempool/writing/foo.md");
    assert_eq!(model.entries[2].path.as_str(), "/mempool/talks/baz.md");

    let writing_only = build_mempool_model(
        &mempool_root,
        vec![
            loaded(
                "/mempool/writing/foo.md",
                "---\ntitle: foo\nstatus: draft\nmodified: 2026-04-01\n---\n# foo\n",
            ),
            loaded(
                "/mempool/papers/bar.md",
                "---\ntitle: bar\nstatus: review\nmodified: 2026-04-02\n---\n# bar\n",
            ),
        ],
        &LedgerFilterShape::Category("writing".to_string()),
    );
    assert_eq!(writing_only.entries.len(), 1);
    assert_eq!(writing_only.total_count, 2);
}
```

> Note: the `priority.unwrap() as u8` cast assumes the enum has `#[repr(u8)]`. If
> the integration test fails to cast, replace the assertion with
> `assert_eq!(format!("{:?}", model.entries[0].priority), "Some(High)")` —
> the cast was a convenience.

- [ ] **Step 6.6: Run integration test — confirm green**

```bash
cargo test --test mempool_model
```

Expected: 1 passed.

- [ ] **Step 6.7: Run full lib + integration test sweep**

```bash
cargo test --lib && cargo test --test mempool_model
```

Expected: all green.

- [ ] **Step 6.8: Commit**

```bash
git add src/components/mempool/model.rs src/components/mempool/mod.rs tests/mempool_model.rs
git commit -m "feat(mempool): add build_mempool_model and integration test

Sync builder takes pre-fetched LoadedMempoolFile inputs and emits a
MempoolModel with sorted entries, filter applied to visible subset,
and unfiltered counts/total preserved. Integration test exercises
mixed categories and the writing-only filter."
```

---

## Task 7: Mempool component shell + CSS

**Why:** Build the visual surface in isolation: a Leptos component that takes a `MempoolModel` and renders it. Wiring the async data is Task 8.

**Files:**
- Create: `src/components/mempool/component.rs`
- Create: `src/components/mempool/mempool.module.css`
- Modify: `src/components/mempool/mod.rs` (declare + re-export the component)

- [ ] **Step 7.1: Create `src/components/mempool/mempool.module.css`**

Mirror the `ledger.html` mempool aesthetic, scoped via stylance:

```css
.mempool {
  border: 1px dashed var(--ledger-rule);
  margin: var(--space-3_5) 0 var(--space-7);
  font-family: var(--font-mono);
  background: rgba(214, 163, 92, 0.02);
  position: relative;
  opacity: 0.94;
}

.mpHead {
  display: flex;
  align-items: baseline;
  gap: var(--space-3_5);
  padding: var(--space-1_25) var(--space-3);
  border-bottom: 1px dashed var(--ledger-rule);
  font-size: var(--font-size-xs);
  letter-spacing: 0.04em;
  color: var(--ledger-faint);
  text-transform: uppercase;
}

.mpLabel {
  color: var(--ledger-amber);
  font-weight: 600;
  letter-spacing: 0.06em;
}

.mpLabel::before {
  content: "⏳ ";
  opacity: 0.85;
}

.mpCount {
  color: var(--ledger-dim);
}

.mpList {
  display: flex;
  flex-direction: column;
}

.mpItem {
  display: grid;
  grid-template-columns: 92px 1fr auto;
  gap: var(--space-3_5);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px dashed var(--ledger-rule);
  font-size: var(--font-size-sm);
  color: var(--ledger-dim);
  align-items: start;
  cursor: pointer;
  transition: background 0.12s ease;
}

.mpItem:last-child {
  border-bottom: 0;
}

.mpItem:hover {
  background: rgba(255, 255, 255, 0.025);
}

.mpStatus {
  font-size: 10.5px;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ledger-faint);
  display: flex;
  align-items: baseline;
  gap: 5px;
}

.mpStatus::before {
  content: "⏳";
  opacity: 0.6;
  font-size: 10px;
}

.mpItemDraft .mpStatus {
  color: var(--ledger-dim);
}

.mpItemReview .mpStatus {
  color: var(--ledger-amber);
}

.mpItemReview .mpStatus::before {
  content: "⌛";
  animation: ledgerBlink 1.6s steps(2, end) infinite;
}

.mpTitle {
  color: var(--ledger-ink);
  font-weight: 500;
  font-size: 13px;
}

.mpKindTag {
  border: 1px solid var(--ledger-rule);
  padding: 0 5px;
  font-size: 10px;
  margin-right: 6px;
  letter-spacing: 0;
  text-transform: none;
  display: inline-block;
  line-height: 1.55;
}

.mpKindTag[data-kind="paper"] {
  color: var(--ledger-accent);
  border-color: var(--ledger-accent-dim);
}

.mpKindTag[data-kind="project"] {
  color: var(--ledger-hex);
}

.mpKindTag[data-kind="writing"] {
  color: var(--ledger-amber);
}

.mpKindTag[data-kind="talk"] {
  color: var(--ledger-ink);
}

.mpKindTag[data-kind="note"] {
  color: var(--ledger-dim);
}

.mpDesc {
  color: var(--ledger-faint);
  font-size: 11.5px;
  margin-top: 2px;
}

.mpMeta {
  margin-top: 4px;
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  font-size: 10.5px;
  color: var(--ledger-faint);
}

.mpMetaKv {
  display: inline-flex;
  gap: 4px;
  align-items: baseline;
}

.mpMetaKey {
  color: var(--ledger-faint);
  opacity: 0.7;
}

.mpMetaValue {
  color: var(--ledger-dim);
}

.mpPriHigh {
  color: var(--ledger-accent);
}

.mpPriMed {
  color: var(--ledger-amber);
}

.mpPriLow {
  color: var(--ledger-dim);
}

.mpModified {
  color: var(--ledger-faint);
  font-size: 10.5px;
  text-align: right;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
  align-self: start;
  padding-top: 1px;
  letter-spacing: 0.02em;
}

.mpEmpty {
  padding: 14px 16px;
  color: var(--ledger-faint);
  font-size: 11.5px;
  text-align: center;
  font-style: italic;
  border-top: 1px dashed var(--ledger-rule);
}
```

> Note: the `ledgerBlink` keyframe is already defined in
> `src/components/ledger_page.module.css`. Stylance scopes class names but
> keyframe names are global, so the reference works as-is.

- [ ] **Step 7.2: Create `src/components/mempool/component.rs`**

```rust
//! Leptos component rendering a MempoolModel.

use leptos::prelude::*;

use super::model::{LedgerFilterShape, MempoolEntry, MempoolModel, MempoolStatus, Priority};

stylance::import_crate_style!(css, "src/components/mempool/mempool.module.css");

#[component]
pub fn Mempool(
    model: MempoolModel,
    #[prop(into)] on_select: Callback<MempoolEntry>,
) -> impl IntoView {
    if model.total_count == 0 {
        return view! {}.into_any();
    }

    let header = render_header(&model);
    let rows = render_rows(&model, on_select);

    view! {
        <section class=css::mempool aria-label="Mempool — pending entries">
            {header}
            <div class=css::mpList>
                {rows}
            </div>
        </section>
    }
    .into_any()
}

fn render_header(model: &MempoolModel) -> impl IntoView {
    let count_text = match &model.filter {
        LedgerFilterShape::All => format!("· {} pending", model.total_count),
        LedgerFilterShape::Category(_) => format!(
            "· {} / {} pending",
            model.entries.len(),
            model.total_count
        ),
    };
    view! {
        <div class=css::mpHead>
            <span class=css::mpLabel>"mempool"</span>
            <span class=css::mpCount>{count_text}</span>
        </div>
    }
}

fn render_rows(model: &MempoolModel, on_select: Callback<MempoolEntry>) -> AnyView {
    if model.entries.is_empty() {
        return view! {
            <div class=css::mpEmpty>
                "no pending entries match this filter"
            </div>
        }
        .into_any();
    }

    model
        .entries
        .iter()
        .cloned()
        .map(|entry| {
            let entry_for_click = entry.clone();
            let on_select = on_select.clone();
            view! {
                <MempoolItem
                    entry=entry
                    on_click=Callback::new(move |_| {
                        on_select.run(entry_for_click.clone());
                    })
                />
            }
        })
        .collect_view()
        .into_any()
}

#[component]
fn MempoolItem(entry: MempoolEntry, on_click: Callback<()>) -> impl IntoView {
    let item_class = match entry.status {
        MempoolStatus::Draft => format!("{} {}", css::mpItem, css::mpItemDraft),
        MempoolStatus::Review => format!("{} {}", css::mpItem, css::mpItemReview),
    };
    let status_label = match entry.status {
        MempoolStatus::Draft => "draft",
        MempoolStatus::Review => "review",
    };
    let priority_class = entry.priority.map(|p| match p {
        Priority::Low => css::mpPriLow,
        Priority::Med => css::mpPriMed,
        Priority::High => css::mpPriHigh,
    });
    let priority_text = entry.priority.map(|p| match p {
        Priority::Low => "low",
        Priority::Med => "med",
        Priority::High => "high",
    });

    view! {
        <div
            class=item_class
            tabindex="0"
            role="button"
            on:click=move |_| on_click.run(())
            on:keydown=move |event| {
                if event.key() == "Enter" || event.key() == " " {
                    event.prevent_default();
                    on_click.run(());
                }
            }
        >
            <div class=css::mpStatus>{status_label}</div>
            <div>
                <div class=css::mpTitle>
                    <span class=css::mpKindTag data-kind=entry.kind.clone()>{entry.kind.clone()}</span>
                    {entry.title.clone()}
                </div>
                <div class=css::mpDesc>{entry.desc.clone()}</div>
                <div class=css::mpMeta>
                    {priority_text.map(|text| view! {
                        <span class=css::mpMetaKv>
                            <span class=css::mpMetaKey>"priority"</span>
                            <span class=format!("{} {}", css::mpMetaValue, priority_class.unwrap_or(""))>
                                {text}
                            </span>
                        </span>
                    })}
                    <span class=css::mpMetaKv>
                        <span class=css::mpMetaKey>"gas"</span>
                        <span class=css::mpMetaValue>{entry.gas.clone()}</span>
                    </span>
                </div>
            </div>
            <div class=css::mpModified>{entry.modified.clone()}</div>
        </div>
    }
}
```

- [ ] **Step 7.3: Update `src/components/mempool/mod.rs`**

```rust
//! Mempool — pending content entries displayed above the chain on /ledger.

mod component;
mod model;
mod parse;

pub use component::Mempool;
pub use model::{
    LedgerFilterShape, LoadedMempoolFile, MempoolEntry, MempoolModel, MempoolStatus, Priority,
    build_mempool_model,
};
pub use parse::{RawMempoolMeta, parse_mempool_frontmatter};
```

- [ ] **Step 7.4: Compile-check (wasm)**

```bash
cargo check --target wasm32-unknown-unknown --lib
```

Expected: clean.

- [ ] **Step 7.5: Commit**

```bash
git add src/components/mempool/component.rs src/components/mempool/mempool.module.css src/components/mempool/mod.rs
git commit -m "feat(mempool): add Mempool Leptos component + CSS

Renders MempoolModel as the dashed-border section above the chain.
Three-column grid (status / body / modified) mirrors ledger.html
mempool aesthetic. Click and keyboard select dispatch via on_select
callback; modal wiring lands in Task 9."
```

---

## Task 8: Async loader + Wire Mempool into LedgerPage

**Why:** The component now needs data. Wire a `LocalResource` that fetches `/mempool` files via the GitHub backend and feeds them to `build_mempool_model`.

**Files:**
- Create: `src/components/mempool/loader.rs`
- Modify: `src/components/mempool/mod.rs` (re-export loader)
- Modify: `src/components/ledger_page.rs` (render `Mempool` between filter bar and chain)

- [ ] **Step 8.1: Create `src/components/mempool/loader.rs`**

```rust
//! Async fetcher for /mempool entries.

use crate::app::AppContext;
use crate::core::engine::GlobalFs;
use crate::models::{FsEntry, VirtualPath};

use super::model::LoadedMempoolFile;
use super::parse::parse_mempool_frontmatter;

const MEMPOOL_ROOT: &str = "/mempool";

pub fn mempool_root() -> VirtualPath {
    VirtualPath::from_absolute(MEMPOOL_ROOT).expect("mempool root is absolute")
}

/// Walk `/mempool`, fetch each file's body, and build the `LoadedMempoolFile`
/// list. Returns an empty vec if the mount is missing or the tree is empty.
/// Individual file fetch failures are logged and the file is skipped.
pub async fn load_mempool_files(ctx: AppContext) -> Vec<LoadedMempoolFile> {
    let root = mempool_root();
    let paths = ctx.view_global_fs.with(|fs| collect_mempool_files(fs, &root));

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        match ctx.read_text(&path).await {
            Ok(body) => {
                let meta = parse_mempool_frontmatter(&body).unwrap_or_default();
                let byte_len = body.as_bytes().len();
                let is_markdown = path.as_str().ends_with(".md");
                out.push(LoadedMempoolFile {
                    path,
                    meta,
                    body,
                    byte_len,
                    is_markdown,
                });
            }
            Err(error) => {
                leptos::logging::warn!("mempool: failed to read {}: {error}", path.as_str());
            }
        }
    }
    out
}

fn collect_mempool_files(fs: &GlobalFs, root: &VirtualPath) -> Vec<VirtualPath> {
    let mut out = Vec::new();
    walk(fs, root, &mut out);
    out
}

fn walk(fs: &GlobalFs, current: &VirtualPath, out: &mut Vec<VirtualPath>) {
    let Some(entry) = fs.get_entry(current) else {
        return;
    };
    match entry {
        FsEntry::Directory { children, .. } => {
            for (name, _child) in children.iter() {
                let child_path = current.join(name);
                walk(fs, &child_path, out);
            }
        }
        FsEntry::File { .. } => {
            out.push(current.clone());
        }
    }
}
```

- [ ] **Step 8.2: Re-export from `src/components/mempool/mod.rs`**

Update to:

```rust
//! Mempool — pending content entries displayed above the chain on /ledger.

mod component;
mod loader;
mod model;
mod parse;

pub use component::Mempool;
pub use loader::{load_mempool_files, mempool_root};
pub use model::{
    LedgerFilterShape, LoadedMempoolFile, MempoolEntry, MempoolModel, MempoolStatus, Priority,
    build_mempool_model,
};
pub use parse::{RawMempoolMeta, parse_mempool_frontmatter};
```

- [ ] **Step 8.3: Wire `LedgerPage` to render the mempool**

Open `src/components/ledger_page.rs`. Find the `LedgerPage` body where it renders `LedgerIdentifier`, `LedgerHeader`, `LedgerFilterBar`, `LedgerChain`. Augment the imports and render path:

Add to imports near the top:

```rust
use crate::components::mempool::{
    LedgerFilterShape, Mempool, MempoolEntry, build_mempool_model, load_mempool_files,
    mempool_root,
};
```

Add a `LocalResource` alongside the existing `ledger` resource (right after its definition):

```rust
let mempool_ctx = ctx.clone();
let mempool_files = LocalResource::new(move || {
    let ctx = mempool_ctx.clone();
    async move { load_mempool_files(ctx).await }
});
```

Add a callback that will receive the click event from a mempool item — for Task 8 we wire it as a no-op stub; Task 9 replaces it with the modal opener:

```rust
let on_mempool_select = Callback::new(move |_entry: MempoolEntry| {
    // Modal preview wiring lands in Task 9.
});
```

Inside the existing `view!` block where the children of `<SiteContentFrame>` are rendered, change:

```rust
view! {
    <LedgerIdentifier model=model.clone() />
    <LedgerHeader model=model.clone() />
    <LedgerFilterBar model=model.clone() />
    <LedgerChain model=model.clone() />
}.into_any()
```

to:

```rust
let filter_shape = match &filter {
    LedgerFilter::All => LedgerFilterShape::All,
    LedgerFilter::Category(c) => LedgerFilterShape::Category(c.clone()),
};
let mempool_root_path = mempool_root();
let on_select = on_mempool_select.clone();
let mempool_files_signal = mempool_files;
let mempool_section = move || {
    mempool_files_signal.get().map(|files| {
        let mempool_model = build_mempool_model(&mempool_root_path, files, &filter_shape);
        view! { <Mempool model=mempool_model on_select=on_select.clone() /> }
    })
};

view! {
    <LedgerIdentifier model=model.clone() />
    <LedgerHeader model=model.clone() />
    <LedgerFilterBar model=model.clone() />
    <Suspense fallback=|| view! { <span></span> }>
        {mempool_section}
    </Suspense>
    <LedgerChain model=model.clone() />
}.into_any()
```

> Note: the `filter_shape` mapping mirrors the private `LedgerFilter` enum
> in `ledger_page.rs`. If the engineer prefers, they may make
> `LedgerFilter` public in the same PR; the duplicate is intentional in
> Phase 1 to keep the mempool independently testable (master plan §3 A1
> spirit: keep coupling minimal).

- [ ] **Step 8.4: Compile-check**

```bash
cargo check --target wasm32-unknown-unknown --lib
```

Expected: clean.

- [ ] **Step 8.5: Run lib tests**

```bash
cargo test --lib
```

Expected: all green.

- [ ] **Step 8.6: Commit**

```bash
git add src/components/mempool/loader.rs src/components/mempool/mod.rs src/components/ledger_page.rs
git commit -m "feat(mempool): async loader + render Mempool on /ledger

LedgerPage now spawns a LocalResource that fetches all files under
/mempool via AppContext.read_text, then builds a MempoolModel and
hands it to the Mempool component. The on_select callback is a stub
in this commit; modal wiring is Task 9."
```

---

## Task 9: Modal preview on click

**Why:** Phase 1 design §6.7 — clicking a mempool row opens a modal containing the existing `Reader` component, sourced from the mempool path, with no URL change.

**Files:**
- Create: `src/components/mempool/preview.rs`
- Modify: `src/components/mempool/mod.rs`
- Modify: `src/components/ledger_page.rs` (replace stub callback)

- [ ] **Step 9.1: Inspect existing modal CSS for the editor**

Read `src/components/editor/modal.module.css` to confirm the overlay pattern. If it provides a backdrop + panel, reuse the visual idiom. (Implementation notes: backdrop is `position: fixed; inset: 0; background: rgba(0,0,0,.55)`; panel is centered with max-width.)

- [ ] **Step 9.2: Create `src/components/mempool/preview.rs`**

```rust
//! Modal preview for a mempool entry. Reuses the Reader component for body
//! rendering; the modal frame itself is local to the mempool module.

use leptos::prelude::*;

use crate::components::reader::Reader;
use crate::models::VirtualPath;

stylance::import_crate_style!(preview_css, "src/components/mempool/preview.module.css");

#[component]
pub fn MempoolPreviewModal(
    open_path: ReadSignal<Option<VirtualPath>>,
    set_open_path: WriteSignal<Option<VirtualPath>>,
) -> impl IntoView {
    let close = move || set_open_path.set(None);

    let body = move || {
        open_path.get().map(|path| {
            view! {
                <div
                    class=preview_css::backdrop
                    on:click=move |_| close()
                >
                    <div
                        class=preview_css::panel
                        on:click=|event| event.stop_propagation()
                    >
                        <button
                            class=preview_css::close
                            type="button"
                            aria-label="Close preview"
                            on:click=move |_| close()
                        >
                            "×"
                        </button>
                        <Reader path=path />
                    </div>
                </div>
            }
        })
    };

    view! {
        {body}
    }
}
```

- [ ] **Step 9.3: Create `src/components/mempool/preview.module.css`**

```css
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-mono);
}

.panel {
  background: var(--bg-elevated);
  border: 1px solid var(--border-muted);
  width: min(720px, 92vw);
  max-height: 86vh;
  overflow: auto;
  padding: 28px 32px 32px;
  position: relative;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.6);
}

.close {
  position: absolute;
  top: 8px;
  right: 12px;
  background: transparent;
  border: 0;
  font: inherit;
  font-size: 22px;
  color: var(--text-dim);
  cursor: pointer;
  padding: 0 6px;
}

.close:hover {
  color: var(--text-primary);
}
```

- [ ] **Step 9.4: Confirm `Reader` accepts a `path` prop**

```bash
grep -n "fn Reader\|pub fn Reader\|#\[component\]" src/components/reader/mod.rs | head -5
```

If `Reader` accepts a different prop shape (e.g., `route` instead of `path`), adjust the call in §9.2 accordingly. The component-tree call signature is the only adapt-as-needed point.

- [ ] **Step 9.5: Re-export from `src/components/mempool/mod.rs`**

Append `MempoolPreviewModal` and the `preview` module:

```rust
mod component;
mod loader;
mod model;
mod parse;
mod preview;

pub use component::Mempool;
pub use loader::{load_mempool_files, mempool_root};
pub use model::{
    LedgerFilterShape, LoadedMempoolFile, MempoolEntry, MempoolModel, MempoolStatus, Priority,
    build_mempool_model,
};
pub use parse::{RawMempoolMeta, parse_mempool_frontmatter};
pub use preview::MempoolPreviewModal;
```

- [ ] **Step 9.6: Wire the modal in `LedgerPage`**

Open `src/components/ledger_page.rs`. Replace the stub callback from Task 8.3 and mount the modal at the top level of the page:

```rust
use crate::components::mempool::MempoolPreviewModal;
// (leave existing imports above)

// Inside `pub fn LedgerPage(...)`, replace the no-op stub:
let (preview_open, set_preview_open) = signal(None::<VirtualPath>);

let on_mempool_select = Callback::new(move |entry: MempoolEntry| {
    set_preview_open.set(Some(entry.path));
});
```

At the bottom of the `view!` block, just inside `<SiteSurface>` (after `<SiteContentFrame>` closes), add:

```rust
<MempoolPreviewModal open_path=preview_open set_open_path=set_preview_open />
```

- [ ] **Step 9.7: Compile-check**

```bash
cargo check --target wasm32-unknown-unknown --lib
```

Expected: clean. If `Reader` does not accept `path` directly, fix the call in `src/components/mempool/preview.rs` per §9.4.

- [ ] **Step 9.8: Run lib tests**

```bash
cargo test --lib
```

Expected: all green.

- [ ] **Step 9.9: Commit**

```bash
git add src/components/mempool/preview.rs src/components/mempool/preview.module.css src/components/mempool/mod.rs src/components/ledger_page.rs
git commit -m "feat(mempool): modal preview on row click

Reuses the Reader component inside a stylance-scoped overlay. The URL
bar does not change. Click outside or × button closes the modal."
```

---

## Task 10: Reserve compose-button slot in LedgerFilterBar

**Why:** Master §9 — Phase 2 will render a compose button in the filter bar's right edge. Reserving the layout slot now means Phase 2 doesn't shuffle the bar; the slot renders nothing in Phase 1.

**Files:**
- Modify: `src/components/ledger_page.rs` (add empty `<div class=css::filterBarRight />` end-cap)
- Modify: `src/components/ledger_page.module.css` (style the end-cap as a flex spacer)

- [ ] **Step 10.1: Add the right-side slot in `LedgerFilterBar` JSX**

In `src/components/ledger_page.rs`, find `fn LedgerFilterBar`. After the closing `<span class=css::dash …>` (the trailing dotted-line decorator at the right edge), wrap the slot:

```rust
view! {
    <nav class=css::filterBar aria-label="Ledger filters">
        <span class=css::dash aria-hidden="true"></span>
        <LedgerFilterLink label="all" href="/#/ledger" count=model.total_count active=model.filter.is_all() />
        {LEDGER_CATEGORIES.iter().map(|category| { /* unchanged */ }).collect_view()}
        <span class=css::dash aria-hidden="true"></span>
        <span class=css::filterBarSlot aria-hidden="true"></span>
    </nav>
}
```

- [ ] **Step 10.2: Add `.filterBarSlot` CSS**

In `src/components/ledger_page.module.css`, add (near the existing `.filterBar`/`.dash` rules):

```css
.filterBarSlot {
  flex: 0 0 auto;
  min-width: 0;
}
```

- [ ] **Step 10.3: Compile-check**

```bash
cargo check --target wasm32-unknown-unknown --lib
```

Expected: clean.

- [ ] **Step 10.4: Commit**

```bash
git add src/components/ledger_page.rs src/components/ledger_page.module.css
git commit -m "feat(mempool): reserve compose-button slot in LedgerFilterBar

Renders an empty right-side slot in Phase 1; Phase 2 author mode will
render the Compose button there without touching the bar layout."
```

---

## Task 11: Playwright E2E test

**Why:** Phase 1 design §7.3 lists visual-QA scenarios. A small Playwright spec exercises them in a real browser against the release Trunk server.

**Files:**
- Create: `tests/e2e/mempool.spec.js`

- [ ] **Step 11.1: Inspect existing e2e structure**

```bash
ls tests/e2e/ && head -30 tests/e2e/$(ls tests/e2e/ | head -1)
```

Confirm the Playwright setup (page object, base URL config, etc.). Match its style.

- [ ] **Step 11.2: Create `tests/e2e/mempool.spec.js`**

```javascript
// Mempool — Phase 1 visual QA
//
// Requires: trunk release build at WEBSH_E2E_BASE_URL, and at least 4 entries
// in 0xwonj/websh-mempool covering writing, projects, papers, talks
// categories. CI/local should not pin this test for green if mempool is empty
// — the assertion `expect(items).toBeGreaterThan(0)` will catch that.

import { test, expect } from '@playwright/test';

const BASE_URL = process.env.WEBSH_E2E_BASE_URL || 'http://127.0.0.1:4173';

test.describe('mempool', () => {
  test('renders above chain on /ledger with at least one entry', async ({ page }) => {
    await page.goto(`${BASE_URL}/#/ledger`);
    const mempool = page.locator('section[aria-label="Mempool — pending entries"]');
    await expect(mempool).toBeVisible();
    const items = await mempool.locator('[role="button"]').count();
    expect(items).toBeGreaterThan(0);
  });

  test('filter narrows mempool to category', async ({ page }) => {
    await page.goto(`${BASE_URL}/#/writing`);
    const mempool = page.locator('section[aria-label="Mempool — pending entries"]');
    await expect(mempool).toBeVisible();
    const count = await mempool.locator('[role="button"]').count();
    const itemKinds = await mempool.locator('[role="button"] [data-kind]').allTextContents();
    for (const kind of itemKinds) {
      expect(['writing']).toContain(kind);
    }
    // Header shows "X / Y pending"
    const header = await mempool.locator('span').filter({ hasText: /pending/ }).innerText();
    expect(header).toMatch(/\d+ \/ \d+ pending/);
    expect(count).toBeGreaterThanOrEqual(0);
  });

  test('clicking a row opens the modal preview without URL change', async ({ page }) => {
    await page.goto(`${BASE_URL}/#/ledger`);
    const initialHash = await page.evaluate(() => window.location.hash);
    const firstRow = page.locator('section[aria-label="Mempool — pending entries"] [role="button"]').first();
    await firstRow.click();
    await expect(page.locator('[aria-label="Close preview"]')).toBeVisible();
    const afterClickHash = await page.evaluate(() => window.location.hash);
    expect(afterClickHash).toBe(initialHash);
    await page.locator('[aria-label="Close preview"]').click();
    await expect(page.locator('[aria-label="Close preview"]')).toHaveCount(0);
  });
});
```

- [ ] **Step 11.3: Build the release artifacts and run the spec**

```bash
trunk build --release
WEBSH_E2E_BASE_URL=http://127.0.0.1:4173 \
  NODE_PATH=target/qa/node_modules \
  target/qa/node_modules/.bin/playwright test tests/e2e/mempool.spec.js \
  --reporter=line --workers=1
```

Expected: all 3 tests pass. If the mempool repo has no entries, the first test fails as designed — push at least 4 seed entries (Prerequisite PR-1) and rerun.

- [ ] **Step 11.4: Commit**

```bash
git add tests/e2e/mempool.spec.js
git commit -m "test(e2e): add Playwright spec for Phase 1 mempool

Covers: section visibility on /ledger, category filter narrowing on
/writing, modal preview opens on row click without changing URL hash."
```

---

## Task 12: Final verification + reviewer agent + master plan update

**Why:** Per master §5 step 6, the phase ends with a reviewer agent pass. Step 7 marks the phase complete in the master document.

**Files:**
- Modify: `docs/superpowers/specs/2026-04-28-mempool-master.md` (status table)

- [ ] **Step 12.1: Run the full local verification gate**

```bash
just verify
```

If `just verify` fails on something unrelated to mempool, the engineer documents the failure and decides whether to address before the phase closes. For mempool-related failures, fix and re-run.

- [ ] **Step 12.2: Visual smoke test via `trunk serve`**

```bash
trunk serve --dist dist-dev
```

Open `http://127.0.0.1:8080/#/ledger`, then:
- Confirm mempool section visible above chain.
- Filter to `/writing`, `/projects`, `/papers`, `/talks` — each shows a category-narrowed mempool with `N / M pending` count.
- Filter to a category that has zero mempool entries — empty placeholder visible inside the section, header still shown.
- Click a mempool row — modal preview opens; URL bar unchanged. Close button works.

- [ ] **Step 12.3: Invoke the reviewer agent**

Use the `superpowers:code-reviewer` agent. Pass it:
- Phase 1 design doc (`docs/superpowers/specs/2026-04-28-mempool-phase1-design.md`)
- This plan (`docs/superpowers/plans/2026-04-28-mempool-phase1-plan.md`)
- The diff: `git diff main..HEAD -- src/ tests/ content/`

Capture the reviewer's findings. Address all CRITICAL and HIGH items. MEDIUM items are addressed unless explicitly deferred with a follow-up task.

- [ ] **Step 12.4: Update master plan §6 document index**

In `docs/superpowers/specs/2026-04-28-mempool-master.md`, change Phase 1 row in §6 to:

```markdown
| 1 | Design | `docs/superpowers/specs/2026-04-28-mempool-phase1-design.md` | Approved |
| 1 | Plan | `docs/superpowers/plans/2026-04-28-mempool-phase1-plan.md` | Approved |
```

And §4 status table:

```markdown
| 1 | Read-only Mempool | Mempool section renders pending entries from `/mempool` mount; click opens modal preview; filter integration | **Complete** |
```

- [ ] **Step 12.5: Commit the master update**

```bash
git add docs/superpowers/specs/2026-04-28-mempool-master.md
git commit -m "docs(mempool): mark Phase 1 complete in master plan"
```

- [ ] **Step 12.6: Push the branch and open a PR**

```bash
git push -u origin HEAD
gh pr create --title "Mempool Phase 1: read-only mempool section" --body "$(cat <<'EOF'
## Summary
- Mounts `0xwonj/websh-mempool` at `/mempool`
- Renders pending entries above the chain on `/ledger` with category filter integration
- Click → modal preview using existing Reader component

## Test plan
- [x] `cargo test --lib`
- [x] `cargo test --test mempool_model`
- [x] `cargo check --target wasm32-unknown-unknown --lib`
- [x] Playwright e2e on release build
- [x] Visual QA: filter scenarios + modal interaction
- [x] Reviewer agent cleared CRITICAL/HIGH items

Master plan: docs/superpowers/specs/2026-04-28-mempool-master.md
Phase 1 design: docs/superpowers/specs/2026-04-28-mempool-phase1-design.md
EOF
)"
```

- [ ] **Step 12.7: Phase 1 complete — Phase 2 begins**

Per master §5 workflow, Phase 2 starts by re-reading the master plan and writing `docs/superpowers/specs/<date>-mempool-phase2-design.md`. Do not begin Phase 2 implementation before that design is written and approved.

---

## Self-Review

This plan covers Phase 1 design §1 (scope), §3 (mount), §5 (schema), §6 (component tree, model, build path, parsing, item rendering, filter, click, placement), §7 (test strategy: unit, integration, Playwright), §8 (file changes), §10 (acceptance criteria via Task 12).

Type signatures used in later tasks match earlier definitions: `LoadedMempoolFile` (Task 6), `MempoolEntry` / `MempoolModel` / `LedgerFilterShape` (Task 3), `Mempool` / `MempoolPreviewModal` (Tasks 7 & 9), `load_mempool_files` / `mempool_root` (Task 8).

No `TODO` / `TBD` / `fill in details` placeholders remain in code-bearing steps. Two annotated **Note** lines flag known iteration risks (the `let _ = bucket;` artifact in Task 5.3 and the `Reader` prop name in Task 9.4) — both include explicit fix instructions if the engineer hits them.

Risk: the Phase 1 design doc and the existing reader component coupling are validated only by Step 9.4's grep. If that fails, the engineer adapts the modal to the actual `Reader` API; this is a one-line change and is called out explicitly.
