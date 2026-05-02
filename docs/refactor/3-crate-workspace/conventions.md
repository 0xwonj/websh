# Conventions

Mechanical rules that apply to every change in this migration. These are *what to write*; [principles.md](./principles.md) is *how to think about it*.

## Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/).

```
<type>(<scope>): <imperative summary, lowercase, no trailing period>

<body — optional, focused on the *why* and any non-obvious *what*>

<footer — optional; references issues, ADRs, breaking-change notes>
```

**Types**: `feat`, `fix`, `refactor`, `chore`, `docs`, `test`, `perf`, `build`, `ci`.

**Scopes** (use the most specific that applies):
- Crate-level: `core`, `cli`, `web`, `workspace`.
- Sub-module: `core/storage`, `core/runtime`, `core/mempool`, `cli/attest`, `web/reader`, etc.
- Cross-cutting: `docs`, `ci`.

**Examples (use these as the model)**:

```
refactor(workspace): add cargo workspace skeleton with three empty member crates

build(core): wire pgp crate with wasm feature for browser-side verification

feat(cli): extract attestation build pipeline from clap shim into engine module

fix(web): adjust include_str! paths for content/keys/wonjae.asc post-move

docs(refactor): record adr for storage backend split across crates
```

**Don't**:

```
chore: phase B step 3                              ← internal jargon
chore: progress on workspace migration             ← vague
WIP                                                ← never
fix: oops                                          ← never
feat(core): add new architecture (Phase B Task 4)  ← internal jargon
```

**Body guidance**:
- Wrap at ~72 chars.
- Focus on *why* the change exists, not what each line does (the diff already shows that).
- Omit the body when the subject is self-explanatory.
- Don't write "this commit does X, Y, Z" — let the diff speak.

**Breaking changes** are marked with a `!` after the type/scope and a `BREAKING CHANGE:` footer:

```
refactor(core)!: replace single crate with cargo workspace

BREAKING CHANGE: build paths and binary location have changed.
Run `cargo build -p websh-cli` instead of `cargo build --bin websh-cli`.
```

The whole migration is a breaking change. Use the `!` marker on at least one commit per phase that introduces user-visible breakage (workspace skeleton, trunk reconfig, doc updates).

## Comments

Default: **don't write comments**. Code that needs explanation usually needs renaming or restructuring first.

**Write a comment only when**:
1. The *why* is non-obvious from the code itself.
2. There's a hidden constraint or invariant a reader could violate.
3. There's a workaround for a specific bug or library behavior, with a link or reference.
4. There's a SAFETY justification for an `unsafe` block.

**Never write comments that**:
- Restate what the code does (`// add 1 to x` above `x += 1`).
- Reference the migration ("// moved here in Phase B"), the original design ("// per architecture.md section 3.1"), or the development process ("// fixed bug from earlier review").
- Apologize, explain personal preferences, or hedge ("// not sure if this is right but…").
- Contain TODOs without an owner and a concrete trigger ("// TODO: clean up later" — never; "// TODO(remove once attestation v2 lands)" — fine).

**Format**:
- Single line for one-line comments: `// short, declarative.`
- Doc comments (`///`) on every `pub` item *only* if the why isn't self-evident. Pure data structs with named fields rarely need doc comments. Public functions usually do — focus on contract / invariants, not the implementation.
- No multi-paragraph comment blocks. If something genuinely needs a paragraph, it belongs in `principles.md` or an ADR.

**Bad examples** (avoid):

```rust
// This function handles the case where the manifest is missing.
// We needed to add this in the migration to fix a bug from earlier
// where the manifest got out of sync. See the architecture doc.
//
// TODO: maybe we should split this into two functions someday.
pub fn load_manifest(...) { ... }
```

**Good examples**:

```rust
/// Returns `None` when the manifest doesn't open with a `---` fence;
/// downstream code treats this as "no recognizable frontmatter".
pub fn parse_mempool_frontmatter(body: &str) -> Option<RawMempoolMeta> { ... }

// SAFETY: wasm32 has no threads. The cleanup runs on the same JS thread
// that installed it; the wrapper is never genuinely sent or shared.
unsafe impl<F: ?Sized> Send for WasmCleanup<F> {}
```

## Naming

- **Crates**: `websh-core`, `websh-cli`, `websh-web`. Kebab-case in `Cargo.toml`, snake_case (`websh_core`, etc.) in `use` statements (Rust automatic).
- **Modules**: snake_case files. Folder modules use `mod.rs`. Single-file modules sit at the parent level (no `foo/mod.rs` for a 50-line `foo`).
- **Types**: `PascalCase`. Avoid suffixes like `Manager`, `Helper`, `Handler` unless they have a specific role (`StorageBackend` is fine — it's a port).
- **Functions**: `snake_case`. Verbs preferred (`build_manifest_state`, not `manifest_state_builder`).
- **Constants**: `SCREAMING_SNAKE_CASE`. Top-level only; no module-private constants for one-time values.
- **Generics**: single uppercase letters when the role is obvious (`T`, `E`); descriptive `PascalCase` when not (`type Backend: StorageBackend`).
- **Avoid version suffixes** (`*V1`, `*V2`, `*_old`, `*_new`). Replace, don't accumulate.

## File and module size

- Target: 200-400 lines per file.
- Hard ceiling: 800 lines.
- A file approaching 800 lines that isn't naturally cohesive is a refactor signal.
- Tests in `#[cfg(test)] mod tests` blocks count toward file size.

## Error handling

- **Per-domain error enums** in `websh-core` using `thiserror`:

  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum CommitError {
      #[error("staged change at {0} is outside commit root")]
      OutOfBounds(VirtualPath),
      #[error("backend rejected commit: {0}")]
      Backend(#[from] StorageError),
      // ...
  }
  ```

- **`String` errors at boundaries** where typed errors don't pay off: CLI surfaces, async UI handlers that just want a human-readable message.
- **No global `WebshError`**. Each subsystem owns its error type.
- **`Result` at every public engine entry point**. Internal helpers can panic on impossibility (`expect("invariant: …")`), but public APIs always return `Result`.
- **`?` for propagation**, no `try!`, no `.unwrap()` outside tests and proven-impossible paths.
- **Error messages are user-facing**: lowercase, no trailing period, no internal jargon. "missing GitHub token for mempool commit" — yes; "ERROR: GitHubTokenNotFound (commit_path=mempool)" — no.

## Logging

- **`leptos::logging::warn!` / `error!`** for browser. `eprintln!` for CLI.
- **No `println!` for diagnostics** — `println!` is for primary CLI output to stdout.
- **No log levels beyond warn/error** for now. Info/debug noise is more cost than value.

## Testing

- **Unit tests in `#[cfg(test)] mod tests`** at the bottom of the file.
- **Integration tests** under `crates/<crate>/tests/`.
- **Test naming**: `<thing>_<expected_behavior>_<condition>` — `parse_returns_none_when_no_fence_present`. Don't use `test_` prefix.
- **One assertion per test** when possible; name the test by what's being asserted.
- **No fixture-via-`Default::default()` magic** — explicit constructors per test or per `mod` make the input visible.

## Versioning

- `Cargo.toml`'s `version` stays at `0.1.0` for the migration. No bump.
- No `*V1`/`*V2` types. Replace.
- No "deprecated, kept for compatibility" annotations. Delete the deprecated thing.

## Imports

- **`use` ordering**: std, then external crates (alphabetical), then `crate::`/`super::`/`self::`. `cargo fmt` handles within-group ordering; the group order is convention.
- **`use crate::…` for crate-level paths**, `super::` and `self::` for sibling/ancestor module references — whichever is shorter in context.
- **No `*` glob imports** outside test modules and prelude conventions.
- **No re-exports for "in case someone needs it"** — re-export only what consumers actually use.

## Visibility

- **`pub`** only on items consumed by another crate.
- **`pub(crate)`** for items consumed across modules within a crate.
- **`pub(super)`** for items consumed by the immediate parent module only.
- **`pub(in path)`** sparingly — usually a smell.
- Default to private. Tighten visibility before each phase wrap-up.

## Documentation

- Migration-related docs live in `docs/refactor/3-crate-workspace/`. Don't pollute the project README, the project CLAUDE.md, or `docs/` root with migration internals during the migration.
- After the migration, a final commit updates `/CLAUDE.md` and the project README with the new build commands and structure.
