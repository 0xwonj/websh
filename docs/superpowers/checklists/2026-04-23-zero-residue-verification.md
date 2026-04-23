# Zero-Residue Verification Checklist

Updated: 2026-04-23
Status: executed

## Build and grep gates

- `cargo test`
  Result: pass, `352` tests green, no warnings.
- `cargo test --features mock --test commit_integration`
  Result: pass, `1` test green.
- `cargo clippy --all-targets --features mock -- -D warnings`
  Result: pass.
- `env -u NO_COLOR trunk build --release`
  Result: pass, no warnings.
- `npm install`
  Result: pass from tracked `package.json` / `package-lock.json`.
- `npm run e2e`
  Result: pass, `9` browser tests green; Playwright starts the release Trunk server from tracked `playwright.config.js`.
- Residue grep across `src tests docs README.md CLAUDE.md index.html assets/manifest.json Trunk.toml`
  Result: clean for the required completion-gate residue patterns.

## Browser/runtime QA

Harness:

- `npm run e2e`
- Playwright Chromium through tracked project dependencies
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
- `sync auth set qa-token` survived a page reload and was used by a later commit request.
- `sync auth clear` removed the `/state/session/github_token_present` marker after reload.
- terminal output redacted the auth command as `sync auth set <redacted>`.
- command-history recall did not expose `qa-token`.
- `ls /state/session` showed `github_token_present` after auth set.
- `cat /state/session/github_token` returned path not found.
- `sync commit qa commit` sent a browser GraphQL request to `https://api.github.com/graphql`.
- the captured GraphQL request included `Authorization: bearer qa-token`.
- the captured GraphQL input included repository `0xwonj/db`, branch `main`, and message `qa commit`.
- the captured GraphQL input included the hydrated `expectedHeadOid`.
- the captured GraphQL file changes included `~/manifest.json` and `~/commit-new.md` additions.
- the captured GraphQL file changes included recursive deletions for `~/docs/old.md` and `~/docs/deep/old.md`.
- descendant staged writes under `docs/` were suppressed from GraphQL additions when `rm -r docs` was staged.
- commit request assembly is covered by Rust integration:
  staged paths, auth handoff, regenerated mount snapshot, prefixed GitHub
  manifest path, empty directories, recursive directory deletions, path
  validation, and addition/deletion conflict normalization are asserted before
  backend dispatch.
- IDB draft persistence round-tripped through the browser:
  `echo persisted > persist.md`, global draft record polling, page reload, then `ls` showed `persist.md`.

## Fallback route verification

- Engine fallback remains covered by `core::engine::routing::tests::resolves_root_to_index_page_via_convention_fallback`.
- Derived-index route resolution remains covered by `core::engine::routing::tests::resolves_route_from_derived_index`.
- Playwright full-page-load verification against Trunk release output passed for:
  `/#/`, `/#/shell`, `/#/fs`, `/#/fs/site`, `/#/fs/state/session`, and `/#/fs/mnt/db`.
- Each direct-load case asserted the final URL hash and absence of the route-miss text.
- The direct-load run produced no browser page errors and no console errors.

## Outcome

- Runtime authority is consolidated under `BOOTSTRAP_SITE` and `core::runtime::loader`.
- Storage scan/commit surface is backend-neutral (`ScannedSubtree`).
- Commit write surface is backend-neutral (`CommitDelta` + merged `ScannedSubtree`).
- Scan assembly is direct `ScannedSubtree -> GlobalFs`; the old mount-local filesystem model is removed.
- Stale docs were removed; only the current architecture spec and this checklist remain under `docs/`.
