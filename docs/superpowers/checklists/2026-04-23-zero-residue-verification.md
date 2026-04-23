# Zero-Residue Verification Checklist

Updated: 2026-04-23
Status: executed

## Build and grep gates

- `cargo test`
  Result: pass, `339` tests green, no warnings.
- `cargo test --features mock --test commit_integration`
  Result: pass, `1` test green.
- `env -u NO_COLOR trunk build --release`
  Result: pass, no warnings.
- `NODE_PATH=target/qa/node_modules target/qa/node_modules/.bin/playwright test tests/e2e --reporter=line --workers=1`
  Result: pass, `8` browser tests green against release Trunk server on `127.0.0.1:4173`.
- Residue grep across `src tests docs README.md CLAUDE.md`
  Result: clean for the required completion-gate residue patterns.

## Browser/runtime QA

Harness:

- `trunk serve --release --port 4173 --address 127.0.0.1`
- Playwright Chromium
- pageerror and console-error assertions enabled
- stubbed GitHub raw-content responses
- stubbed EIP-1193 wallet

Executed and observed:

- `/shell` loaded with cwd `/site`.
- `/fs` loaded with cwd `/`.
- `/fs/site` loaded with cwd `/site`.
- `/fs/mnt/db` loaded with cwd `/mnt/db`.
- `login` restored admin write eligibility through the EIP-1193 stub.
- `sync auth set qa-token` wrote session state immediately.
- `export EDITOR=nano` wrote env state immediately.
- `ls /state/session` showed `github_token_present` after auth set.
- `cat /state/session/github_token` returned path not found.
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
- commit request assembly is covered by Rust integration:
  staged paths, regenerated mount snapshot, prefixed GitHub manifest path,
  empty directories, and recursive directory deletions are asserted before
  backend dispatch.
- runtime-state cleanup worked:
  `sync auth clear`
  `unset EDITOR`
  `logout`
  browser storage keys were removed/cleared
  `ls /state/session` still showed `wallet_session`
  `ls /state/env` returned `# No user variables set`
- IDB draft persistence round-tripped through the browser:
  `echo persisted > persist.md`, debounce save, page reload, then `ls` showed `persist.md`.

## Fallback route verification

- Engine fallback remains covered by `core::engine::routing::tests::resolves_root_to_index_page_via_convention_fallback`.
- Derived-index route resolution remains covered by `core::engine::routing::tests::resolves_route_from_derived_index`.
- Playwright full-page-load verification against Trunk release output passed for:
  `/#/`, `/#/shell`, `/#/fs`, `/#/fs/site`, `/#/fs/state/session`, and `/#/fs/mnt/db`.
- The direct-load run produced no browser page errors and no console errors.

## Outcome

- Runtime authority is consolidated under `BOOTSTRAP_SITE` and `core::runtime::loader`.
- Storage scan/commit surface is backend-neutral (`ScannedSubtree`).
- Scan assembly is direct `ScannedSubtree -> GlobalFs`; the old mount-local filesystem model is removed.
- Stale docs were removed; only the current architecture spec and this checklist remain under `docs/`.
