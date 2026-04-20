# Phase 2 Track I: Cleanup — Implementation Plan

**Goal:** Closing Phase 2 with curated MEDIUM + LOW items that don't need their own tracks.

## Decisions (in scope)

| Issue | Action |
|---|---|
| **M9** | Extract ErrorBoundary inline styles from `src/app.rs` into a CSS module |
| **L2** | Add `wallet::disconnect(ctx)` helper to dedupe `clear_session + set Disconnected` pattern |
| **L10** | Replace hardcoded `🔒` emoji in `output.rs` with `Icon icon=ic::LOCK` |
| **L12** | Add `role`, `aria-label`, `tabindex`, and keyboard handler to BottomSheet drag handle |
| Comment | Add a convention note in `src/app.rs` about `#[derive(Clone, Copy)]` on signal-container structs (L1) |

## Decisions (deferred, recorded in master)

- **M4 (AppError + `From` chain)**: The three domain error types (`WalletError`, `EnvironmentError`, `FetchError`) are used locally within their domains and rarely crossed. Unifying into a single `AppError` with `From` impls would be a refactor with limited immediate payoff. Defer to a later phase when a concrete cross-domain error path appears.
- **L5 (`PathArg` newtype thinness)**: The newtype currently just wraps `String` with no unique methods. Either deletion or thickening would churn command match arms without user-visible benefit. Defer.

## File Structure

| Path | Action |
|---|---|
| `src/components/error_boundary.module.css` | Create |
| `src/app.rs` | Modify (swap inline style for CSS module classes; L1 doc comment) |
| `src/core/wallet.rs` | Modify (add `disconnect(ctx)`) |
| `src/components/terminal/shell.rs`, `terminal.rs`, `boot.rs` | Modify (call `disconnect`) |
| `src/components/terminal/output.rs` | Modify (L10) |
| `src/components/explorer/preview/sheet.rs` | Modify (L12) |

---

## Task I.1 — ErrorBoundary CSS extraction (M9)

- [ ] Create `src/components/error_boundary.module.css` with rules (container, title, message, details, details-list, reload-button). Use CSS variables from the existing design tokens where possible (see `assets/base.css` for tokens like `--bg`, `--error`, etc.; if those aren't defined, fall back to matching the current literal colors).
- [ ] In `src/app.rs`, replace the ~60 lines of inline `style="..."` with classes from the new module. Import via `stylance::import_crate_style!(err_css, "src/components/error_boundary.module.css")`.
- [ ] Build + check visually equivalent (skip visual test; rely on compile).
- [ ] Commit: `refactor(app): extract ErrorBoundary styles into CSS module`

## Task I.2 — Wallet disconnect helper (L2)

Current pattern duplicated in 2 places (`terminal.rs:78`, `shell.rs:66`):
```rust
wallet::clear_session();
ctx.wallet.set(WalletState::Disconnected);
// optional status message
```

- [ ] In `src/core/wallet.rs`, add:
  ```rust
  /// Disconnect the wallet: clear session storage and set state to Disconnected.
  pub fn disconnect(ctx: &crate::app::AppContext) {
      clear_session();
      ctx.wallet.set(crate::models::WalletState::Disconnected);
  }
  ```
- [ ] Replace both call sites with `wallet::disconnect(&ctx)` (or `wallet::disconnect(ctx)` depending on `AppContext: Copy`).
- [ ] `boot.rs:161` pattern — check if it also follows the same shape; if yes, use helper; if not (may just be `clear_session` without state change), leave.
- [ ] Commit: `refactor(wallet): introduce disconnect() helper, dedupe 2 call sites`

## Task I.3 — Lock icon replace (L10)

In `src/components/terminal/output.rs`, find the `lock_icon` emoji hardcoding around line 45:
```rust
let lock_icon = if encrypted { " 🔒" } else { "" };
```

Replace with a Leptos view using the `ic::LOCK` icon (already used elsewhere per `src/components/icons.rs`). Example shape:
```rust
{encrypted.then(|| view! { <span class=css::lockIcon><Icon icon=ic::LOCK /></span> })}
```

The rendering code will need to be restructured since currently `lock_icon` is a string concatenated into display output. Check how it's rendered and adapt. Add a CSS class `lockIcon` to `output.module.css` if missing (look at how `file_list.rs:245` does the same pattern).

- [ ] Build + test no regression.
- [ ] Commit: `refactor(terminal): lock emoji → ic::LOCK svg icon`

## Task I.4 — BottomSheet drag handle a11y (L12)

In `src/components/explorer/preview/sheet.rs` around line 195-205, the drag handle `<div class=css::handle>` lacks accessibility attributes.

Add:
- `role="button"`
- `aria-label="Drag to resize preview"`
- `tabindex="0"`
- `on:keydown` — `ArrowUp`/`ArrowDown` change sheet height, `Enter`/`Space` toggle expanded/collapsed (simple toggle; don't go full keyboard drag).

For keydown, a minimal implementation:
```rust
let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
    match ev.key().as_str() {
        "Enter" | " " => {
            ev.prevent_default();
            // Toggle between snap points — reuse existing "click handle" logic if any
            // Example: if current height is "expanded", go to "collapsed" snap
            // Consult sheet.rs for the existing snap-height functions.
        }
        _ => {}
    }
};
```

If the existing code has a click handler on the handle for tapping (toggle), reuse its body for Enter/Space. Otherwise keep it simple: no-op with a comment.

- [ ] Build + test.
- [ ] Commit: `feat(sheet): a11y on drag handle (role, aria-label, keyboard toggle)`

## Task I.5 — Clone+Copy convention note (L1)

Add a brief `// Convention note` comment block in `src/app.rs` above `AppContext` (around line 224 where `#[derive(Clone, Copy)]` appears):

```rust
// Convention: state container structs in this app (`AppContext`, `TerminalState`,
// `ExplorerState`) are `#[derive(Clone, Copy)]` because all their fields are
// Leptos signals (`RwSignal`, `Memo`, `StoredValue`), which are `Copy`-cheap
// pointers to reactive state in Leptos' arena. Copying a container is near-free
// and lets closures capture by move without explicit clones.
// Do NOT derive `Copy` on a struct that adds non-signal fields.
```

Same style comment (or a cross-reference) above `TerminalState` and `ExplorerState`.

- [ ] Commit: `docs: comment the Clone+Copy convention for signal containers`

---

## Verification

- `cargo test --bin websh`: 189 pass / 4 pre-existing fail (no new tests; these are UI/refactor changes).
- `cargo build --release --target wasm32-unknown-unknown`: clean.
- No new warnings.

## Expected Commits (5)

1. `refactor(app): extract ErrorBoundary styles into CSS module`
2. `refactor(wallet): introduce disconnect() helper, dedupe 2 call sites`
3. `refactor(terminal): lock emoji → ic::LOCK svg icon`
4. `feat(sheet): a11y on drag handle (role, aria-label, keyboard toggle)`
5. `docs: comment the Clone+Copy convention for signal containers`

## Done Criteria

- `cargo test --bin websh` green (aside from the 4 pre-existing failures).
- `cargo build --release --target wasm32-unknown-unknown` clean.
- All 5 commits applied.
