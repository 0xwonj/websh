# Principles, Patterns, Idioms

How to think about the code we're writing. [conventions.md](./conventions.md) is the mechanical rulebook; this is the design rulebook.

## Architectural principles

### Layering

```
domain types (pure data, no I/O)
    ↑ depended on by
ports + engines (pure-Rust I/O abstractions)
    ↑ implemented by
adapters (target-specific implementations)
```

This is the layering inside `websh-core`. Across crates, the rule is:

```
websh-cli, websh-web   → both depend on websh-core
websh-cli ↔ websh-web  no dependency in either direction
```

Cargo enforces the cross-crate rule. The domain/engine/adapter layering inside `websh-core` is enforced by module visibility (`pub(crate)`, `pub(super)`) and reviewer discipline.

### Hexagonal where it earns its keep, not everywhere

`storage::backend::StorageBackend` is the canonical port. GitHub / IDB / Mock are adapters. Don't extend this pattern to subsystems with one implementation. `mempool`, `attestation`, `content_sync` are not ports — they are concrete engines. Speculative trait-based abstraction is over-engineering.

### Engines extracted from CLI shims

CLI subcommand files are clap parsers + dispatch. The engine work is in `engine/<domain>/`. A reader trying to find "where does `mempool add` actually do its work" should land in `engine/mempool/add.rs`, not `cli/mempool.rs`.

This is the structural shape that enforces "CLI files are thin." Phase C makes it real.

### Single source of truth

Every state-producing path goes through one builder. Today, `core::mempool::build_mempool_manifest_state` is the only producer of mempool manifest state. Browser save_raw and CLI mempool add both call it. Don't introduce parallel constructors.

When a second producer is tempting, the answer is usually a parameterized version of the first.

### Two-target invariant

`websh-core` compiles for `wasm32-unknown-unknown` and the host triple. Every public API in core respects this. Wasm-only adapters (gloo-net, idb) are `cfg(target_arch = "wasm32")`-gated *within* core; the public API surface stays identical across targets.

## Leptos best practices

### `Memo` vs `Effect`

- **`Memo::new`** for derived values read in multiple places.
- **`Effect::new`** for side effects only — DOM mutations, persistence, navigation.
- **Never** put computation in an `Effect` and consume the result via a signal. That's a `Memo`.

### `AppContext` is a `Copy` struct of signal handles

Every field is a signal handle (`RwSignal<T>`, `Memo<T>`, `Signal<T>`) — handles are arena indices, not the underlying values. The struct is `Copy`. Closures capture it without `clone()` noise.

This is the existing pattern in `app.rs`. Preserve it.

### Children, slots, and render props

- **`children`** for opaque content (a single content slot).
- **`#[slot]` macro** for named children with semantic roles (`<DialogHeader>`, `<DialogBody>`, …).
- **Render props** rarely needed; prefer slots when there are multiple named hooks.

### `prop:hidden` over `<Show>` for disclosure widgets

When ARIA `aria-controls` must always resolve to an element in the DOM, use `prop:hidden=move || collapsed.get()`. `<Show>` removes the element when false, breaking ARIA.

The mempool component has the canonical example. Look there before introducing a new pattern.

### `NodeRef` lifecycle

Access via `node_ref.get_untracked()` from event handlers — the node is mounted by the time the user can fire an event. Don't read from `Effect`s or component bodies (timing not guaranteed).

### Signal scope

Signals are arena-allocated with lifetime tied to the UI scope, not Rust lexical scope. Items dropped when the component unmounts. This means no manual cleanup for ordinary signals — only DOM event listeners and external resources need explicit `on_cleanup`.

### `WasmCleanup<F>` for closures bridging into Leptos

`Closure<F>` is `!Send + !Sync`. Leptos's `on_cleanup` requires `Send + Sync + 'static`. Use the workspace's generic `WasmCleanup<F: ?Sized>` newtype with the `unsafe impl Send + Sync` (sound on single-threaded wasm). The justification is in `websh-web/src/platform/wasm_cleanup.rs`.

## Rust idioms

### `FromStr` over `parse(&str) -> Option<Self>`

`MempoolStatus`, `Priority` use `impl FromStr`. Call sites use `.parse::<MempoolStatus>()` or `MempoolStatus::from_str(s)`. Don't reintroduce `parse(&str) -> Option<Self>` shapes.

### Iterator hygiene

- Prefer `.iter()` / `.into_iter()` / `.collect()` over manual loops when a transformation is the goal.
- Avoid intermediate `Vec` when an iterator chain works (`.filter().map().sum()` not `.collect::<Vec<_>>().sum()`).
- `for (k, _) in map.iter()` should usually be `for k in map.keys()`.

### `Option`/`Result` discipline

- `unwrap_or_default()` only when the default genuinely matches the absence semantic. For frontmatter parsing where `None` should error, use `.ok_or_else(…)` instead.
- `is_some_and` / `is_none_or` over `.map(…).unwrap_or(false)`.
- `matches!` for pattern checks: `matches!(x, ChangeType::DeleteFile | ChangeType::DeleteDirectory)`.
- `let-else` over `if let Some(x) = … else { return; }`.

### `Default` derives

Derive `Default` only when the all-fields-default value is meaningful. Manually implement when the meaningful default differs from field-wise defaults.

### Trait objects vs generics

Prefer generics (monomorphization, no vtable cost) unless heterogeneous storage demands `dyn Trait`. `Arc<dyn StorageBackend>` is necessary because the backend registry stores adapters of different concrete types; `dyn` is correct there.

### Error handling

See [conventions.md § Error handling](./conventions.md#error-handling). Per-domain `thiserror` enums; `String` at CLI boundary; no global error type.

## Anti-patterns to avoid

- **Legacy / backwards-compat shims**. The migration is breaking. Replace, don't accumulate.
- **`*_old` / `*_new` naming**. Rename the new one to the canonical name.
- **`v1` / `v2` types** in the same crate. Pick a version, delete the others.
- **Half-finished implementations**. If a subsystem isn't done, it's not landed; don't ship a stub.
- **TODOs without owners or triggers**. Either fix it now or open an issue with a concrete trigger.
- **Speculative abstractions** ("this trait will let us swap implementations later"). Make the concrete thing first; introduce the trait when the second impl actually appears.
- **Mass re-exports for ergonomics**. Re-export only what consumers use.
- **Dead-code `#[allow]`**. If something is dead, delete it.
- **"Cleaner if…" without measurable benefit**. Aesthetic preferences aren't refactor justifications.
- **Comments that restate code, reference internal process, or apologize**. See [conventions.md § Comments](./conventions.md#comments).

## Design patterns we apply

### Compound component (Leptos)

For UI components with semantic variants, prefer a compound shape:

```rust
#[component]
pub fn IdentifierStrip(
    #[prop(optional)] muted: bool,
    children: Children,
) -> impl IntoView { ... }
```

A single component with a typed prop driving a class variant. Don't ship `IdentifierStrip` and `MutedIdentifierStrip` as siblings.

### Builder for complex inputs

For values with many optional fields, builders read better than `Default::default() ..` spread:

```rust
ManifestEntry::builder()
    .path(path)
    .meta(meta)
    .build()
```

We don't have widespread use of this yet; introduce only when a struct has 5+ optional fields.

### Type-state for protocol invariants

For multi-step protocols, encode state in the type. Phase C's `PromoteCleanup` is close — consider type-state if a follow-on cleanup decision arises.

### Newtype for primitive obscurity

`VirtualPath`, `MempoolStatus`, `MempoolFields` are existing newtype/enum wrappers around what would otherwise be `String`. Continue this pattern when a primitive carries domain semantics.

## Application of principles to in-flight decisions

When a decision is non-obvious during execution:

1. Check this document first. The pattern is probably already named here.
2. If not, consult `architecture.md` for the design intent.
3. If neither answers, the decision is genuinely new — record an ADR.
4. If the decision contradicts something in this doc, surface it: either this doc updates, or the decision is wrong.

This document is the working theory. Evidence in execution updates it. Don't silently violate the doc; deviate explicitly via the ADR pathway.
