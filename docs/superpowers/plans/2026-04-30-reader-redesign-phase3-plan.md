# Phase 3 — Plan: Footnote toolbar + keybindings

**Date:** 2026-04-30
**Design:** `docs/superpowers/specs/2026-04-30-reader-redesign-phase3-design.md`

## Steps

### Step 1 — `toolbar.rs`
- Create `src/components/reader/toolbar.rs`.
- Import `ReaderMode` from `super` (currently private — promote to `pub(super) enum ReaderMode` in `mod.rs`).
- Implement pure helpers `state_label(saving, dirty) -> &'static str` and `state_class(saving, dirty)` returning the modifier class (`""` or `"modefnStateDirty"`).
- Define `#[component] pub fn ReaderToolbar(mode, is_new, can_edit, saving, dirty, on_edit, on_preview, on_save, on_cancel)` per design §6. (The `is_new` prop is kept for future label re-introduction but unused in the markup; mark it `#[allow(unused_variables)]` or accept and discard.)
- Render the footnote-mark markup per design §4 using `css::modefn*` classes. Active mode item gets `format!("{} {}", css::modefnOpt, css::modefnOptOn)`.
- `cancel` and `save` items are wrapped in `<Show when=move || mode.get() == ReaderMode::Edit>`.
- Whole toolbar is wrapped in `<Show when=move || visible.get()>` where `visible = mode == Edit || (mode == View && can_edit)`.
- Add `#[cfg(test)] mod tests` covering `state_label` / `state_class` (3 cases each).

### Step 2 — `mod.rs` integration
- Remove the inline `#[component] fn ReaderToolbar` block.
- Add `mod toolbar; use toolbar::ReaderToolbar;`.
- Pass `dirty=draft_dirty.read_only()` to the new component (new prop).
- Make `ReaderMode` `pub(super)` (downgrade from private) so toolbar.rs can name it.

### Step 3 — Keybindings
- Inside `Reader`, after the callbacks are defined, install a `keydown` listener on `web_sys::window()` using `Closure::wrap + add_event_listener_with_callback`.
- Handler logic (matches design §5):
  ```rust
  let mode_now = mode.get_untracked();
  let in_textarea = ev.target()
      .and_then(|t| t.dyn_ref::<web_sys::HtmlTextAreaElement>().cloned())
      .is_some();

  if (ev.meta_key() || ev.ctrl_key()) && ev.key() == "s" {
      ev.prevent_default();
      if mode_now == ReaderMode::Edit && !saving.get_untracked() {
          on_save_cb.run(());
      }
      return;
  }
  if in_textarea { return; }
  match ev.key().as_str() {
      "r" if mode_now == ReaderMode::Edit && !saving.get_untracked() => on_preview_cb.run(()),
      "e" if mode_now == ReaderMode::View && edit_visible.get_untracked() => on_toggle_edit_cb.run(()),
      _ => {}
  }
  ```
- Wrap registration with `on_cleanup` to remove the listener and drop the closure on unmount.
- Whole block guarded by `#[cfg(target_arch = "wasm32")]` since `web_sys::window()` is wasm-only.

### Step 4 — CSS
- Append the `.modefn*` rule set from design §7 to `reader.module.css`.
- Remove the now-unused `.toolbar`, `.toolbarLabel`, `.toolbarActions`, `.actionButton`, `.actionButtonPrimary` rules. Verify no `css::toolbar` / `css::actionButton` references remain in `mod.rs` or any other file (`grep`).

### Step 5 — Verify
- `cargo fmt`.
- `cargo test --lib`.
- `cargo check --target wasm32-unknown-unknown --lib`.
- `trunk build`.
- `grep -E "css::(toolbar|actionButton)" src/components/reader/` → zero matches (except in `reader.module.css`, which we cleaned in step 4).
- `trunk serve` walkthrough per design §8.

### Step 6 — Review + commit
- `superpowers:code-reviewer` with diff + design + master.
- Address CRITICAL/HIGH.
- Master §6 Phase 3 → Complete; §10 entry; status state → "Phase 4 — pending design doc".
- Commit:
  ```
  feat(reader): footnote-mark toolbar with r/e/⌘S keybindings (phase 3)
  ```

## Risks

- Keydown listener must clean up on unmount. `on_cleanup` is the right Leptos hook. Without cleanup, navigating away from a Reader page leaves a stale listener that may fire against disposed signals — would crash or behave erratically. Verify with a route swap during manual QA.
- `target_arch = "wasm32"` gating: `web_sys::window()` only exists on wasm. The `Closure` import is wasm-bindgen-only. Whole keybindings block stays inside the gate. Tests run on host but don't exercise this path.
- Stylance: the new classes auto-emit constants under the consolidated `pub(crate) css` module from Phase 2. No new `import_crate_style!` calls.
