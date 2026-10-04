# CLI Architecture

## Shape

`websh-cli` is the host adapter for workflows that cannot run in the browser:

- content manifest and sidecar generation,
- ledger and attestation generation,
- PGP/Ethereum attestation import and verification,
- Pinata deploy,
- mempool list/add/promote/drop/manifest,
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

`prepare` is the single Trunk content pre-build entry point. It refreshes the manifest
in development and performs the complete content/ledger/subject workflow in release.
`attest` runs the complete workflow explicitly, independent of Trunk profiles.
Signing requires both enabled signing and the expected local secret key.

`WEBSH_NO_SIGN=1` disables new GPG signing. Unchanged subjects retain their existing
attestations; new or changed unsigned subjects remain pending.

`mempool manifest --repo-dir <checkout>` regenerates an external mempool manifest from
canonical category Markdown files, using the same current metadata builder as `mempool add`.
It does not read or translate an earlier manifest format.

## Deployment

Deployment owns `.env` loading and applies it only to child processes. Build and upload
use the same selected directory. Output paths must be root-level `dist` or
`dist-<name>` directories and cannot be symlinks. Release builds use `--locked`.
`--no-build` uploads an existing bundle without changing it; `.last-cid` records the
successful upload and is preserved by routine cleanup.
