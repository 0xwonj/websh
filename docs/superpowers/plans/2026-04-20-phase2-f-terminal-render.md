# Phase 2 Track F: Terminal Render — Implementation Plan

**Goal:** Reduce per-render allocations in the terminal path. Three targeted fixes.

**Addresses:** H1 (VirtualFs cloned in execute path), M2 (OutputLineId newtype + derive PartialEq), M3 (RingBuffer double-clone per render).

## Scope

- **H1**: `ctx.fs.get()` in `components/terminal/terminal.rs::create_submit_callback` clones the full `VirtualFs`. Replace with `ctx.fs.with(|fs| execute_pipeline(...))`.
- **M2**: `OutputLine.id: usize` + hand-written `PartialEq` that ignores `id`. Replace with `OutputLineId(u64)` newtype, derive `PartialEq` / `Eq` / `Hash` structurally on `OutputLine` (including id). Atomic counter becomes `AtomicU64`.
- **M3**: `history_signal.get().to_vec()` in `For each=...` double-clones (get → RingBuffer clone + to_vec → Vec clone). Use `history_signal.with(|buf| buf.iter().cloned().collect::<Vec<_>>())`.

## File Structure

| Path | Action |
|---|---|
| `src/components/terminal/terminal.rs` | Modify (H1, M3) |
| `src/models/terminal.rs` | Modify (M2) |
| Any callers comparing whole `OutputLine` via `PartialEq` | May break; should compare `.data` instead |

---

## Task F.1: `OutputLineId` newtype (M2)

- [ ] **Step 1: Write failing tests**

Add to `src/models/terminal.rs::tests`:

```rust
    #[test]
    fn test_output_line_ids_are_unique_and_monotonic() {
        let a = OutputLine::text("a");
        let b = OutputLine::text("b");
        assert_ne!(a.id, b.id);
        // Newtype: compare via .0
        assert!(a.id.0 < b.id.0);
    }

    #[test]
    fn test_output_line_id_is_copy() {
        let a = OutputLine::text("a");
        let _copy = a.id; // Copy trait
        let _copy2 = a.id; // can copy twice
    }

    #[test]
    fn test_output_line_structural_eq() {
        let a = OutputLine::text("hello");
        let b = OutputLine::text("hello");
        // Different ids → structural PartialEq says not equal.
        assert_ne!(a, b);
        // But .data equality still works.
        assert_eq!(a.data, b.data);
    }
```

- [ ] **Step 2: Verify failure**

```bash
cargo test --bin websh models::terminal::tests
```

Expected: compile errors (OutputLineId not defined, `a.id.0` field access fails on usize).

- [ ] **Step 3: Introduce newtype**

In `src/models/terminal.rs`:

Replace the `static OUTPUT_LINE_COUNTER: AtomicUsize` section and the custom `PartialEq` on `OutputLine` with:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for an `OutputLine`, used as a stable key in Leptos `For` lists.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct OutputLineId(pub u64);

/// Represents a single line of output in the terminal with a unique ID
#[derive(Clone, Debug, PartialEq)]
pub struct OutputLine {
    /// Unique ID for efficient keying in For loops
    pub id: OutputLineId,
    /// The actual output data
    pub data: OutputLineData,
}

// Derive OutputLineData PartialEq should already exist; confirm it's `PartialEq`.

// Global counter
static OUTPUT_LINE_COUNTER: AtomicU64 = AtomicU64::new(0);

impl OutputLine {
    fn new(data: OutputLineData) -> Self {
        Self {
            id: OutputLineId(OUTPUT_LINE_COUNTER.fetch_add(1, Ordering::Relaxed)),
            data,
        }
    }
}
```

Delete the hand-written `impl PartialEq for OutputLine`. The derive on the struct now gives structural equality (compares both `id` and `data`).

- [ ] **Step 4: Update callers**

`src/components/terminal/terminal.rs` line ~116: `key=|line| line.id` — still works because `OutputLineId: Hash + Eq`. No change needed there.

Run `grep -rn "line.id" src/ | grep -v "^src/models/terminal.rs"` — any consumer that treats `id` as `usize` (e.g., indexes, comparisons via numeric ops) needs to access `.0`. Most uses just pass the id to Leptos `For` which doesn't care.

Update existing `test_unique_ids` in `src/models/terminal.rs::tests`:

```rust
    #[test]
    fn test_unique_ids() {
        let line1 = OutputLine::text("first");
        let line2 = OutputLine::text("second");
        let line3 = OutputLine::text("first"); // Same content as line1
        assert_ne!(line1.id, line2.id);
        assert_ne!(line1.id, line3.id);
        assert_ne!(line2.id, line3.id);
        assert_eq!(line1.data, line3.data);
    }
```

(The test uses `assert_ne!(line1.id, line2.id)` which works with the derived `PartialEq` on `OutputLineId`.)

- [ ] **Step 5: Full suite**

```bash
cargo test --bin websh
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/models/terminal.rs
git commit -m "refactor(terminal): OutputLineId newtype, derive PartialEq structurally"
```

---

## Task F.2: Avoid `VirtualFs` clone in execute path (H1)

- [ ] **Step 1: Read existing code**

In `src/components/terminal/terminal.rs`, `create_submit_callback` currently has:

```rust
let current_fs = ctx.fs.get();
let wallet_state = ctx.wallet.get();
let result = execute_pipeline(
    &pipeline,
    &ctx.terminal,
    &wallet_state,
    &current_fs,
    &current_route,
);
```

`ctx.fs.get()` clones the entire `VirtualFs` (which is `HashMap<String, FsEntry>` — potentially large).

- [ ] **Step 2: Replace with `with`**

Replace with:

```rust
let wallet_state = ctx.wallet.get();
let result = ctx.fs.with(|current_fs| {
    execute_pipeline(
        &pipeline,
        &ctx.terminal,
        &wallet_state,
        current_fs,
        &current_route,
    )
});
```

- [ ] **Step 3: Build + run tests**

```bash
cargo build && cargo test --bin websh
```

Expected: clean, tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/components/terminal/terminal.rs
git commit -m "perf(terminal): borrow VirtualFs via with() instead of cloning in execute path"
```

---

## Task F.3: Reduce render-time clone of history (M3)

- [ ] **Step 1: Replace the For each closure**

In `src/components/terminal/terminal.rs`, find:

```rust
<For
    each=move || history_signal.get().to_vec()
    key=|line| line.id
    ...
```

Replace with:

```rust
<For
    each=move || history_signal.with(|buf| buf.iter().cloned().collect::<Vec<_>>())
    key=|line| line.id
    ...
```

- [ ] **Step 2: Build + run tests**

```bash
cargo build && cargo test --bin websh
```

- [ ] **Step 3: Commit**

```bash
git add src/components/terminal/terminal.rs
git commit -m "perf(terminal): avoid RingBuffer clone in For each — iterate in with()"
```

---

## Done Criteria

- `cargo test --bin websh`: ~189 pass / 4 pre-existing fail (186 baseline + 3 new M2 tests).
- `cargo build --release --target wasm32-unknown-unknown`: clean.
- No `ctx.fs.get()` call in terminal.rs or anywhere that leads to full `VirtualFs` clone per command dispatch.
- `OutputLineId` newtype exists; `PartialEq` on `OutputLine` is derived (structural).
- `For each` in terminal.rs uses `with()` + `iter().cloned().collect()`.
