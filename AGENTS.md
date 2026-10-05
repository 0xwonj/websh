# AGENTS.md

Read [architecture](docs/architecture/current.md) for the crate map and native contracts.
Detailed behavior belongs to the linked owning document, not duplicated summaries.

## Operating rules

- Keep the documented dependency boundaries; use public core facades, never `engine` internals.
- Accept current contracts only. Migrate owned data instead of retaining compatibility readers or aliases.
- Keep necessary cryptographic versions and toolchain pins; avoid decorative internal versions.
- CLI argument adapters delegate to workflows; process and filesystem effects belong in `infra`.
- Browser features use the `Content`, `Wallet`, and `Preferences` owners and `RuntimeServices`.
  Preserve refresh/read identity guards; wallet and preference updates must not copy the content tree.
- Browser content is read-only. Wallet connection grants no write privilege.
- Treat mounted content as untrusted. Sanitize Markdown/HTML; access metadata is advisory, not confidentiality.
- Validate terminal input before echo/history. Unsupported input must not be retained.
- Do not load deployment `.env` into general checks, development, generation, or signing.

## Work and verification

`justfile` owns developer commands and the executable gate. Use `just --list` and
`just --show verify`; [tooling](docs/architecture/tooling.md) documents setup and cleanup.
Use focused checks while editing, then the relevant wider gate before finishing.
Browser changes require a WASM-target check; native compilation can miss browser-only code.
See [verification](docs/architecture/verification.md) for exact commands and test ownership.

Keep tests focused on current behavior at its owning layer. Remove obsolete and duplicate
cases; use small fixtures instead of a generic testing framework.

Edit authored inputs and run their owning generator. Do not hand-edit the outputs listed
in [CLI source and generation](docs/architecture/cli.md#source-and-generation), or
`assets/bundle.css`. Generation never signs; signing is an explicit owner operation.

Keep maintained guidance under `docs/architecture/`. Retire one-time maintenance tools
and their tests only after the corresponding migration is complete. Completed proposals
and refactor reports belong in Git history.
