# Zero-Residue Verification Checklist

Updated: 2026-04-23
Status: executed

## Build and grep gates

- `cargo test`
  Result: pass, `348` tests green, no warnings.
- `env -u NO_COLOR trunk build --release`
  Result: pass, no warnings.
- Residue grep across `src tests docs`
  Result: clean for the required completion-gate residue patterns.

## Browser/runtime QA

Harness:

- `trunk serve --release --port 4173 --address 127.0.0.1`
- Playwright Chromium
- stubbed GitHub raw-content responses
- stubbed EIP-1193 wallet
- in-page GraphQL commit stub for `createCommitOnBranch`

Executed and observed:

- `/shell` loaded with cwd `/site`.
- `/fs` loaded with cwd `/`.
- `/fs/site` loaded with cwd `/site`.
- `/fs/mnt/db` loaded with cwd `/mnt/db`.
- `login` restored admin write eligibility through the EIP-1193 stub.
- `sync auth set qa-token` wrote session state immediately.
- `export EDITOR=nano` wrote env state immediately.
- `ls /state/session` showed `github_token` after auth set.
- `ls /state/env` showed `EDITOR` after export.
- browser storage matched the runtime mutations:
  `sessionStorage["websh.gh_token"] == "qa-token"`
  `localStorage["user.EDITOR"] == "nano"`
  `localStorage["wallet_session"] == "1"`
- declared mount refresh worked:
  after mutating the mocked remote mount and running `sync refresh` from `/mnt/db`, `ls` showed `fresh.md`.
- declared mount prune/restore worked:
  removing `.websh/mounts/db.mount.json` from the mocked bootstrap site and running `sync refresh` removed `db` from `ls /mnt`;
  restoring the declaration and refreshing mounted `/mnt/db` again.
- site commit worked:
  `echo site-commit > commit.md`
  `sync commit site-check`
  `sync status` returned clean
  `ls` showed `commit.md`
- declared-mount commit worked:
  `echo db-commit > db-commit.md`
  `sync commit db-check`
  `sync status` returned clean
  `ls` showed `db-commit.md`
- runtime-state cleanup worked:
  `sync auth clear`
  `unset EDITOR`
  `logout`
  browser storage keys were removed/cleared
  `ls /state/session` still showed `wallet_session`
  `ls /state/env` returned `# No user variables set`

## Fallback route verification

- Engine fallback remains covered by `core::engine::routing::tests::resolves_root_to_index_page_via_convention_fallback`.
- Derived-index route resolution remains covered by `core::engine::routing::tests::resolves_route_from_derived_index`.
- During Playwright full-page-load runs against Trunk dev output, direct initial-load verification for `/#/` and `/#/fs/state/...` was not stable enough to treat as authoritative browser evidence, so route fallback confidence for those hashes currently relies on the engine tests above.

## Outcome

- Runtime authority is consolidated under `BOOTSTRAP_SITE` and `core::runtime::loader`.
- Storage scan/commit surface is backend-neutral (`ScannedSubtree`).
- Stale docs were removed; only the current architecture spec and this checklist remain under `docs/`.
- Remaining risk: full page-load browser verification for `/#/` and `/#/fs/state/...` should be revisited outside the Trunk dev-server harness if zero-gap route-bootstrap proof is required beyond the engine tests already passing.
