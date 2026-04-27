# Mempool Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the authoring path — author-mode detection, compose modal for new drafts, edit modal for existing drafts, with both flows committing to the mempool GitHub repo via the existing Phase 3a commit infrastructure.

**Architecture:** A `ComposeModal` component drives a unified frontmatter form + body textarea, dispatched in either `New` or `Edit` mode. Saves build a `ChangeSet`, resolve the mempool backend + auth token + remote head from `AppContext`, and call `commit_backend`. The mempool `LocalResource` refetches on success so the new/updated entry appears in the list without a manual reload.

**Tech Stack:** Rust + Leptos 0.8 (csr), wasm32. Reuses `ChangeSet`, `commit_backend`, `runtime::state::github_token_for_commit`, existing `RuntimeStateSnapshot.github_token_present` signal.

**Master plan:** [`docs/superpowers/specs/2026-04-28-mempool-master.md`](../specs/2026-04-28-mempool-master.md)
**Phase 2 design:** [`docs/superpowers/specs/2026-04-28-mempool-phase2-design.md`](../specs/2026-04-28-mempool-phase2-design.md)

---

## Prerequisites

- Phase 1 merged
- A GitHub personal access token with `contents:write` on `0xwonj/websh-mempool` available; can be set in browser via `set_github_token` from the existing settings flow

---

## Task 1: Add slug + frontmatter serialization helpers

**Files:**
- Create: `src/components/mempool/serialize.rs`
- Modify: `src/components/mempool/mod.rs` (declare module + re-export)

**Steps:**

- [ ] **1.1: Create `serialize.rs` with the test module first** (TDD).

```rust
//! Slug derivation and frontmatter serialization for mempool authoring.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_from_title_kebab_cases_basic() {
        assert_eq!(slug_from_title("On Writing Slow"), "on-writing-slow");
    }

    #[test]
    fn slug_from_title_strips_punctuation() {
        assert_eq!(slug_from_title("Hello, World!"), "hello-world");
    }

    #[test]
    fn slug_from_title_collapses_double_dashes() {
        assert_eq!(slug_from_title("foo  --  bar"), "foo-bar");
    }

    #[test]
    fn slug_from_title_falls_back_for_empty() {
        assert_eq!(slug_from_title(""), "untitled");
        assert_eq!(slug_from_title("!!!"), "untitled");
    }

    #[test]
    fn serialize_emits_required_fields_only() {
        let body = serialize_mempool_file(&ComposePayload {
            title: "foo".into(),
            status: "draft".into(),
            modified: "2026-04-28".into(),
            priority: None,
            tags: vec![],
            body: "Hello.".into(),
        });
        assert!(body.starts_with("---\n"));
        assert!(body.contains("title: \"foo\"\n"));
        assert!(body.contains("status: draft\n"));
        assert!(body.contains("modified: \"2026-04-28\"\n"));
        assert!(!body.contains("priority"));
        assert!(!body.contains("tags"));
        assert!(body.ends_with("Hello.\n"));
    }

    #[test]
    fn serialize_includes_optional_fields_when_set() {
        let body = serialize_mempool_file(&ComposePayload {
            title: "foo".into(),
            status: "review".into(),
            modified: "2026-04-28".into(),
            priority: Some("high".into()),
            tags: vec!["zk".into(), "essay".into()],
            body: "Body.".into(),
        });
        assert!(body.contains("priority: high\n"));
        assert!(body.contains("tags: [zk, essay]\n"));
    }
}
```

- [ ] **1.2: Run test — confirm fails to compile.**
- [ ] **1.3: Implement helpers above the test module.**

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposePayload {
    pub title: String,
    pub status: String,
    pub modified: String,
    pub priority: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
}

pub fn slug_from_title(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut prev_dash = false;
    for ch in title.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            slug.push(lower);
            prev_dash = false;
        } else if !slug.is_empty() && !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

pub fn serialize_mempool_file(payload: &ComposePayload) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("title: \"{}\"\n", escape_yaml(&payload.title)));
    out.push_str(&format!("status: {}\n", payload.status));
    out.push_str(&format!("modified: \"{}\"\n", payload.modified));
    if let Some(p) = &payload.priority {
        out.push_str(&format!("priority: {p}\n"));
    }
    if !payload.tags.is_empty() {
        out.push_str(&format!("tags: [{}]\n", payload.tags.join(", ")));
    }
    out.push_str("---\n\n");
    out.push_str(&payload.body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn escape_yaml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
```

- [ ] **1.4: Run test — confirm green.**
- [ ] **1.5: Wire into `mod.rs`** — add `mod serialize;` and `pub use serialize::{ComposePayload, serialize_mempool_file, slug_from_title};`.
- [ ] **1.6: Commit.**

```bash
git add src/components/mempool/serialize.rs src/components/mempool/mod.rs
git commit -m "feat(mempool): add compose serializer + slug helper

Pure helpers for serializing ComposePayload to a markdown file body
with frontmatter, and deriving a kebab-case slug from a title."
```

---

## Task 2: ComposeMode enum + form validation helper

**Files:**
- Create: `src/components/mempool/compose.rs` (skeleton + `ComposeMode` + validation)

**Steps:**

- [ ] **2.1: Create `compose.rs` with test module first.**

```rust
//! Compose / Edit modal for mempool authoring.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_form_rejects_empty_title() {
        let payload = sample(|p| p.title.clear());
        let errors = validate_form(&payload);
        assert!(errors.iter().any(|e| matches!(e, ComposeError::TitleEmpty)));
    }

    #[test]
    fn validate_form_rejects_unknown_status() {
        let payload = sample(|p| p.status = "published".into());
        let errors = validate_form(&payload);
        assert!(errors.iter().any(|e| matches!(e, ComposeError::StatusUnknown)));
    }

    #[test]
    fn validate_form_rejects_invalid_modified_date() {
        let payload = sample(|p| p.modified = "April 28".into());
        let errors = validate_form(&payload);
        assert!(errors.iter().any(|e| matches!(e, ComposeError::ModifiedNotIso)));
    }

    #[test]
    fn validate_form_accepts_minimal_valid() {
        let payload = sample(|_| {});
        assert!(validate_form(&payload).is_empty());
    }

    fn sample(mutate: impl FnOnce(&mut ComposeForm)) -> ComposeForm {
        let mut form = ComposeForm {
            title: "foo".into(),
            category: "writing".into(),
            slug: "foo".into(),
            status: "draft".into(),
            modified: "2026-04-28".into(),
            priority: None,
            tags: vec![],
            body: "body".into(),
        };
        mutate(&mut form);
        form
    }
}
```

- [ ] **2.2: Run — confirm fails.**
- [ ] **2.3: Implement above test module.**

```rust
use crate::models::VirtualPath;
use crate::utils::format::iso_date_prefix;

use super::serialize::ComposePayload;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeMode {
    New { default_category: Option<String> },
    Edit { path: VirtualPath, body: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposeForm {
    pub title: String,
    pub category: String,
    pub slug: String,
    pub status: String,
    pub modified: String,
    pub priority: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposeError {
    TitleEmpty,
    SlugInvalid,
    StatusUnknown,
    ModifiedNotIso,
    CategoryUnknown,
}

const ALLOWED_STATUSES: &[&str] = &["draft", "review"];
const ALLOWED_CATEGORIES: &[&str] = &["writing", "projects", "papers", "talks"];

pub fn validate_form(form: &ComposeForm) -> Vec<ComposeError> {
    let mut errors = Vec::new();
    if form.title.trim().is_empty() {
        errors.push(ComposeError::TitleEmpty);
    }
    if !slug_is_valid(&form.slug) {
        errors.push(ComposeError::SlugInvalid);
    }
    if !ALLOWED_STATUSES.contains(&form.status.as_str()) {
        errors.push(ComposeError::StatusUnknown);
    }
    if iso_date_prefix(&form.modified).is_none() {
        errors.push(ComposeError::ModifiedNotIso);
    }
    if !ALLOWED_CATEGORIES.contains(&form.category.as_str()) {
        errors.push(ComposeError::CategoryUnknown);
    }
    errors
}

fn slug_is_valid(slug: &str) -> bool {
    if slug.is_empty() {
        return false;
    }
    let bytes = slug.as_bytes();
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-')
}

pub fn form_to_payload(form: &ComposeForm) -> ComposePayload {
    ComposePayload {
        title: form.title.trim().to_string(),
        status: form.status.clone(),
        modified: form.modified.clone(),
        priority: form.priority.clone(),
        tags: form.tags.clone(),
        body: form.body.clone(),
    }
}

pub fn target_path(form: &ComposeForm) -> VirtualPath {
    VirtualPath::from_absolute(format!("/mempool/{}/{}.md", form.category, form.slug))
        .expect("compose target path is absolute")
}
```

- [ ] **2.4: Run — confirm green.**
- [ ] **2.5: Wire `mod.rs`** — add `mod compose;` and `pub use compose::{ComposeError, ComposeForm, ComposeMode, form_to_payload, target_path, validate_form};`
- [ ] **2.6: Commit.**

```bash
git add src/components/mempool/compose.rs src/components/mempool/mod.rs
git commit -m "feat(mempool): add ComposeMode + form validation"
```

---

## Task 3: ComposeModal component (UI shell)

**Files:**
- Modify: `src/components/mempool/compose.rs` (add `#[component] ComposeModal`)
- Create: `src/components/mempool/compose.module.css`

The component takes:

```rust
#[component]
pub fn ComposeModal(
    open: ReadSignal<Option<ComposeMode>>,
    set_open: WriteSignal<Option<ComposeMode>>,
    on_saved: Callback<()>,
) -> impl IntoView
```

Internal state:
- `RwSignal<ComposeForm>` initialized from the `ComposeMode` (when `open` becomes `Some(...)`, derive form from mode; when `None`, render nothing)
- `RwSignal<Vec<ComposeError>>` updated on every form change
- `RwSignal<Option<String>>` for save-error banner

Layout per Phase 2 design §5.3. Reuse `editor/modal.module.css` token patterns where convenient (or define own scoped styles in `compose.module.css`).

Save click handler: dispatched via `Action`-like async fn. See Task 4 for the actual save plumbing — Task 3 only wires the `on:click` to a stub that calls a passed-in `save_callback: Callback<ComposeForm>`. Or use `LocalResource::new_with` to fire on click; either works.

Steps:
- [ ] **3.1: Implement `ComposeModal`** — full code in design §4 + serialized via Task 1+2 helpers.
- [ ] **3.2: Implement CSS** — copy editor/modal.module.css idiom; add form-row + field-error rules.
- [ ] **3.3: Wire `mod.rs`** — `pub use compose::ComposeModal;`
- [ ] **3.4: Compile-check.** `cargo check --target wasm32-unknown-unknown --lib` clean.
- [ ] **3.5: Commit.** `feat(mempool): add ComposeModal component shell`

---

## Task 4: Save flow — ChangeSet construction + commit

**Files:**
- Modify: `src/components/mempool/compose.rs` (add async save handler)
- Modify: `src/components/mempool/mod.rs` (re-export)

The async save handler:

```rust
pub async fn save_compose(
    ctx: AppContext,
    mode: ComposeMode,
    form: ComposeForm,
) -> Result<(), String>
```

Inside:
1. `validate_form(&form)` — return early if non-empty errors.
2. Build `ComposePayload` via `form_to_payload`.
3. Serialize to body bytes via `serialize_mempool_file`.
4. Determine target path:
   - New: `target_path(&form)`
   - Edit: existing `path` from `ComposeMode::Edit`
5. Build `ChangeSet`:
   - New: `ChangeSet::new()` then `.add_file(path, body_bytes)`
   - Edit: `ChangeSet::new()` then `.edit_file(path, body_bytes)` (or whatever the existing `ChangeSet` API is — look in `src/core/changes/` to confirm).
6. Resolve backend: `ctx.backend_for_path(&mempool_root()).ok_or("mempool not mounted")?`
7. Resolve token: `crate::core::runtime::state::github_token_for_commit().ok_or("missing github token")?`
8. Resolve expected_head: `ctx.remote_head_for_path(&mempool_root())`
9. Build commit message per design §4.3
10. Call `commit_backend(backend, mempool_root(), changes, message, expected_head, Some(token)).await`
11. Map `StorageError` to `String` for UI surface.

Steps:
- [ ] **4.1: Survey ChangeSet API** by grepping `src/core/changes/` for `pub fn add_file`, `pub fn edit_file`, etc. Match the call shapes.
- [ ] **4.2: Implement `save_compose` per the steps above.**
- [ ] **4.3: Add a unit test** that exercises `save_compose` against a stub backend if feasible — otherwise lean on integration test in Task 5.
- [ ] **4.4: Wire `ComposeModal`'s Save button** — on click, spawn an async task (Leptos `Action::new`), set the local error signal on Err, call `on_saved` and close on Ok.
- [ ] **4.5: Compile-check + lib tests.**
- [ ] **4.6: Commit.** `feat(mempool): implement save flow for compose modal`

---

## Task 5: Integration test for compose round-trip

**Files:**
- Create: `tests/mempool_compose.rs`

Exercises:
- `serialize_mempool_file(serialize → parse_mempool_frontmatter)` round-trip equivalence
- `target_path` for known categories
- `validate_form` with various bad inputs
- ChangeSet shape sanity for `New` and `Edit` modes (no real backend; assert structure of constructed ChangeSet)

Steps:
- [ ] **5.1: Write tests.** Reference `tests/mempool_model.rs` for shape.
- [ ] **5.2: Run.** `cargo test --test mempool_compose`
- [ ] **5.3: Commit.** `test(mempool): integration tests for compose flow`

---

## Task 6: Author-mode detection + LedgerPage wiring

**Files:**
- Modify: `src/components/ledger_page.rs`

Steps:
- [ ] **6.1: Add `author_mode` memo** reading `ctx.runtime_state.with(|rs| rs.github_token_present)`.
- [ ] **6.2: Add `compose_open` signal** of type `Option<ComposeMode>`.
- [ ] **6.3: Update mempool click handler** to branch on `author_mode.get()`:
    - `true` → fetch the file body via `ctx.read_text(&entry.path).await`, then `set_compose_open.set(Some(ComposeMode::Edit { path: entry.path, body }))`
    - `false` → existing `set_preview_open.set(Some(entry.path))` (Phase 1 behavior)
- [ ] **6.4: Add `ComposeButton`** rendered into `.filterBarSlot` when `author_mode.get()` is `true`. Click sets `compose_open` to `New { default_category: filter_category() }`.
- [ ] **6.5: Mount `<ComposeModal open=compose_open set_open=set_compose_open on_saved=... />`** at the page root, where `on_saved` triggers `mempool_files.refetch()`.
- [ ] **6.6: Add `.composeButton` CSS** in `ledger_page.module.css`.
- [ ] **6.7: Compile-check.**
- [ ] **6.8: Commit.** `feat(mempool): wire author-mode compose/edit into LedgerPage`

---

## Task 7: Final verification + reviewer agent + master plan update

Steps:
- [ ] **7.1: Run** `cargo test --lib && cargo test --test mempool_model && cargo test --test mempool_compose && cargo check --target wasm32-unknown-unknown --lib`. All green.
- [ ] **7.2: Manual visual QA per Phase 2 design §8.3.** Document results.
- [ ] **7.3: Dispatch `superpowers:code-reviewer`** with full Phase 2 diff. Address CRITICAL/HIGH findings.
- [ ] **7.4: Update master plan §4** — Phase 2 status `Complete`, Phase 3 `In Design`.
- [ ] **7.5: Update master plan §6** doc index — Phase 2 design Approved, plan Complete; Phase 3 design In progress.
- [ ] **7.6: Append to §10 Decision Log:** `Phase 2 (authoring) complete: <N> tasks shipped, reviewer findings addressed.`
- [ ] **7.7: Commit master update + Phase 2 close.** `docs(mempool): mark Phase 2 complete in master plan`

---

## Self-Review

Plan covers Phase 2 design §1 (scope), §2 (anchors), §3 (auth detection — Task 6), §4 (modal — Tasks 2/3), §5 (UX wiring — Task 6), §6 (component tree — Task 6), §7 (files — Tasks 1/2/3/4/6), §8 (test strategy — Tasks 1/2/4/5/7), §9 (risks: handled by save-error banner + validation), §10 (acceptance — Task 7).

Type signatures consistent: `ComposeMode`, `ComposeForm`, `ComposePayload`, `ComposeError`, `ComposeModal`, `save_compose` defined in Task 1 → 4 in order; consumed in Task 6.

No placeholders in code-bearing steps. Tasks 3 and 4 have prose for some logic ("see Task 4 for save plumbing") because the modal UI body and save handler are interconnected; the implementer should reference design §4–5 for layout and the existing `editor/modal.rs` for the visual idiom.
