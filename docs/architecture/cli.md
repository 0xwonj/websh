# CLI Architecture

## Shape

`websh-cli` is the host adapter for workflows that cannot run in the browser:

- content manifest and sidecar generation,
- ledger and attestation generation,
- PGP/Ethereum attestation import and verification,
- Pinata deploy,
- mempool list/promote/drop,
- mount initialization and remote operations.

## Module Boundaries

```text
crates/websh-cli/src/
  cli.rs        top-level Clap dispatch
  commands/    thin command adapters and argument mapping
  workflows/   use-case logic and domain orchestration
  infra/       process, Git, GitHub, JSON, filesystem helpers
```

Command modules should not own long-running workflow logic. They parse arguments, construct option structs, call `workflows`, and format outcomes.

`infra` is the only place that should hide process details such as `git`, `gh`, `gpg`, and `trunk` execution. Workflows can depend on infra helpers but should not directly parse process stdout unless the helper returns a typed result.

## Non-Interactive Behavior

Workflows used in automation must fail fast instead of prompting unless the command has an explicit interactive mode. Examples:

- `mempool promote` detects non-interactive mode and requires `--allow-branch-mismatch` for branch mismatch overrides.
- Mutating GitHub remote calls use status helpers that suppress raw JSON unless a workflow explicitly needs it.

## Content And Attestation

`content manifest` is idempotent and safe to run from Trunk hooks. It refreshes sidecars and `content/manifest.json`.

`attest build` is the Trunk pre-build entrypoint. It skips development profiles unless forced, refreshes content/ledger/subject artifacts, and signs when signing is enabled and the expected key is available.

`WEBSH_NO_SIGN=1` keeps generated subjects pending rather than invoking GPG.
