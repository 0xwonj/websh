# Implementation and verification record

Status: the read-only browser implementation and its 36 behavioral acceptance rows are complete. This is not release approval: the dependency gate has two pre-existing maintenance blockers, and the changed homepage needs release signing.

Date: 2026-10-04. Working branch: `codex/read-only-browser`. Baseline: `ce1d7a16cf1653baa80af71f90fcdf98178c755d`.

The pre-existing `AGENTS.md` and `dir_reference.html` were preserved; repository instructions were updated in place. Existing commits were not replaced. The user resolved the Xcode license prerequisite. No personal browser profile was inspected, no legacy user data was deleted, and no signing with the user's keys, deployment, GitHub publication, or push was performed. CLI signing integration tests use disposable test keys.

## Delivered scope

- Removed browser editors, direct/staged saves, draft persistence, credentials, administrator/write policies, writable fields, commit protocol, GraphQL writer transport, and write-only shared helpers/dependencies.
- Preserved wallet connection/session/events, reading, locale/bundle routes, terminal exploration, advisory read markers, native authoring/publishing, and attestations. Added `refresh [path]`; `/new` is ordinary content routing.
- Added root request sequencing, mount epochs/content revisions, atomic validated publication, retained data on failure, bounded text/binary retry, stale completion guards, and strict mount boundary lookup.
- Added `websh-cache` v1 for validated external manifest metadata only: 500 ms restore, 2-second storage attempts, 10-second complete manifest fetch, 2 MiB per record, 8 MiB total, 16 records, 30-day age, and five-minute clock skew. Old `websh-state` is never opened or migrated.
- Added saved-listing/refresh-failed notices with retry. Cold root failures, missing stale routes, and unavailable document bodies remain explicit.

## Final verification

The commands equivalent to the updated verification recipe were run individually to preserve diagnostics and use the existing local release server. `just verify` is **not fully green**, because its dependency prerequisite still fails as recorded below. The recipe and `deny.toml` were not weakened.

| Check | Result |
| --- | --- |
| Baseline core library tests before removal | 519 passed; the lower final count reflects deleted writer tests. |
| `cargo fmt --check`, `git diff --check` | Passed. |
| `cargo check --workspace` | Passed. |
| Native Clippy, all targets, warnings denied | Passed. |
| WASM web Clippy, all targets/features, warnings denied | Passed. |
| `cargo test --workspace` | 479 passed: 78 CLI unit, 10 CLI integration, 378 core unit, 9 mempool integration, 4 site. |
| Explicit core and web `wasm32-unknown-unknown` checks | Passed. |
| Chromium browser WASM harness | 183 passed, including real IndexedDB transactions, aborts, quotas, blocked/late opens, timeouts, and byte pruning. |
| CSS lint and documentation drift | Passed. |
| `WEBSH_NO_SIGN=1 env -u NO_COLOR trunk build --release` | Passed; Stylance, manifest, and attestation hooks ran. |
| `npm run perf:budgets -- dist` | Passed unchanged limits: WASM Brotli 970.0 KiB; total Brotli 1.74 MiB. No measured baseline-size comparison is claimed. |
| Fixture-backed Playwright E2E | 50 passed against the final release build. Includes separate tabs sharing one IndexedDB origin. |
| `cargo machete --with-metadata --skip-target-dir` | Passed; no unused dependencies. |
| `cargo deny check --hide-inclusion-graph` | Fails only on the two existing unmaintained packages below; bans, licenses, and sources pass. |
| Native removed flag | `websh-cli mount init --writable` exits 2 with “unexpected argument”; no workflow or network mutation executes. |
| Manual browser inspection | Home, ledger, and terminal rendered without console errors. A controlled HTTP 503 on the real external listing showed the saved-listing age, failure notice, retry, and retained directory rows; retry was exercised after removing the mock; automated E2E verifies completion. The HTTP error log was expected. Wallet/provider and additional adverse-cache flows use controlled E2E fixtures; no real wallet transaction or personal-profile migration was exercised. |

The WASM harness falls back from unavailable macOS ChromeDriver binaries to the same wasm-bindgen browser test page served locally and exercised through Chromium/Playwright. This is a browser test run, not a native substitute. A Python static server caused intermittent connection resets during an earlier E2E run; final E2E used a local Node static server and passed without those errors.

## Acceptance evidence

Paths are repository relative. Each row links the behavioral requirement in [the plan](implementation-plan.md#acceptance-matrix) to executed coverage or a final source audit.

| ID | Evidence |
| --- | --- |
| R1 | E2E `wallet sessions remain usable...`, `new is an ordinary content route...`, wallet account changes, and source absence audit: no editor or write privilege for any address. |
| R2 | Core routing tests and E2E cover missing `/new` and a real `new.md` route. |
| R3 | Core shell model/executor/pipeline tests cover removed commands, literal redirection arguments, and retained pipes; E2E verifies unchanged content. |
| R4 | Terminal submit-boundary tests cover case, whitespace, quoted/escaped command words, and pipeline position. E2E checks output, history, storage, console, and request secrecy using sentinel credentials. |
| R5 | Core `refresh_selects_owner_without_requiring_a_listed_path` and runtime services cover owner selection, reserved paths, failed/loading mounts, and root reload. |
| R6 | Final source search finds no production ChangeSet, commit DTO/API, writer policy, editor state, or token API. Native compose helpers remain used by CLI. |
| R7 | E2E rejects non-read HTTP methods, `api.github.com`/GraphQL calls, and Authorization headers during retained wallet/read/retired-input flows. |
| R8 | Core runtime projection and E2E runtime directory/inline `cat` tests retain env, wallet, and session with no drafts/token projection. |
| R9 | Domain legacy-writable tests cover missing/true/false equivalence; CLI workflow output omits the field; removed CLI flag exits 2. |
| R10 | Native CLI tests retain manifest/ledger generation, mempool add/promote/drop, local attestation build/verify, and deploy construction. No live publication. |
| W1 | WASM wallet tests and E2E cover missing/rejected providers, connection, logout, and tolerated ENS failure. |
| W2 | E2E verifies session restore, account/chain/disconnect events, prompt/`id`/wallet files, and one listener per event. Existing listener cleanup tests remain. |
| W3 | Core `recipient_read_markers_follow_current_wallet_without_write_capability`; E2E wallet changes preserve cache descriptor/key. |
| W4 | Retained E2E covers theme, browser/shell language, reader text scale, copy link, and history. Legacy migration test preserves unrelated storage. |
| L1 | Services `latest_root_dispatch_controls_both_success_and_failure` plus context generation tests establish latest-root ordering and retained installed runtime. |
| L2 | Mount epoch tests, obsolete-attempt coordinator test, and warm-cache E2E retain the available tree on failure and discard superseded attempts. |
| L3 | E2E `404 and malformed refresh preserve the current body; a valid empty listing confirms removal`. |
| L4 | Context tests reject bad mount assembly, preserve tree/revision on invalid replacement, and replace only on valid empty success. Loader rejects conflicting declarations; coordinator persists only accepted publication. |
| L5 | Context WASM tests cover delayed text and binary replacement, shared inflight reads, generation changes, and twice-obsoleted cancellation. Reader checks the final identity and drops obsolete document resources. |
| L6 | E2E counts body reads through failed refresh, retains content/cache after invalid data, and renders 404 after valid empty replacement. Context limits automatic retries to one. |
| I1 | Real IDB tests create v1/store; E2E spies on legacy opens/deletion and preserves both v3 `draft_changes` and older `drafts` sentinels. |
| I2 | Real IDB `codec_preserves_bundles_metadata_extensions_and_empty_directories` proves exact round trip, including locale variants, recipient metadata and mempool extensions. Empty snapshots round-trip in other adapter tests. |
| I3 | Manifest parser tests reject traversal, duplicates, malformed bundles/metadata and route collisions; cache policy tests reject malformed JSON, identity/size/version violations before publication. |
| I4 | Real IDB `successful_put_then_abort_preserves_previous_record`; adapter awaits committed transaction completion. |
| I5 | Real IDB count and aggregate byte-budget tests; policy tests cover age, future skew, ordered safe integers, oversized records, ties, and backwards time. |
| I6 | Real IDB blocked/late open, version mismatch, missing store, versionchange, quota, and transaction-timeout tests. Timeout aborts changes and releases the writer queue. E2E denied IDB still reads public content. |
| I7 | Real same-tab and separate-connection ordering tests; two-tab E2E proves older-started response cannot overwrite later-started metadata. Corrupt record replacement tested. Cleanup validates current values within the same readwrite transaction. |
| I8 | Production exact-key removal only; E2E confirms no old DB opening, migration, deletion, or draft recovery. |
| C1 | Coordinator tests cover timely cache/early failure, live-before-cache, deadline/late cache, obsolete attempts; E2E covers ordinary misses and saved listing during delayed refresh. |
| C2 | Descriptor tests vary source fields and compare equivalent defaults, normalization, and changed labels. Wallet cache identity remains unchanged in E2E. |
| C3 | URL tests resolve distinct `self` deployment prefixes; existing IPFS hash-navigation E2E verifies correct assets/content URLs. |
| C4 | Root/home remains usable during delayed external scan; E2E cached external row/body appears while the live manifest is gated. Mempool fixture tests retain counts/filtering. |
| C5 | Known cached deep link renders; absent stale route remains pending until live absence confirms 404. |
| C6 | Warm-cache E2E retains listing and visible saved/failed state with retry. Adapter write failures cannot become runtime content failures. |
| C7 | E2E inspects cache payloads; record schema contains only descriptor, manifest JSON/size, and timestamps. Bodies, wallet/runtime state, credentials and attestation verdicts are absent. |
| C8 | E2E covers denied IDB, cached metadata with missing body, and root failure despite existing external records. |

## Removal audit

Searches across `crates/websh-{core,web,site}/src` for the design's writer symbols found only intentional legacy JSON fixtures, retired-command tests, and the exact `websh.gh_token` cleanup. There is no production `websh-state` access. Browser `base64`, editor/CSS modules, draft and GraphQL modules, mock writer, and commit integration gate were removed. `ComposeForm`, remote mempool draft/review labels, filesystem assembly operations, and native Git/GitHub adapters remain required by CLI or read projections.

## Generated artifacts and dependency follow-up

The owning Trunk hooks regenerated styles and manifests. The content manifest's only logical change is the mount declaration size after deleting `writable`. The homepage's renderer source hash changed; the attestation builder correctly removed its now-invalid old signature and marked it pending. Other valid subject signatures remain. Do not restore an obsolete signature manually. Run the normal signing/release workflow with the intended key before deployment.

The dependency audit found pre-existing vulnerabilities and yanked packages. Updated using Cargo: ammonia 4.1.2 → 4.1.4, anyhow 1.0.102 → 1.0.104, crossbeam-epoch 0.9.18 → 0.9.21, lopdf 0.40.0 → 0.42.0, ruint 1.18.0 → 1.20.1, chacha20 0.10.0 → 0.10.2, and spin 0.9.8 → 0.9.9. The HTML sanitizer fix follows [RustSec RUSTSEC-2026-0213](https://rustsec.org/advisories/RUSTSEC-2026-0213.html). Native/WASM tests and the release build cover the updated dependency graph.

Two maintenance advisories still block the existing dependency gate:

1. [`proc-macro-error2` 2.0.1, RUSTSEC-2026-0173](https://rustsec.org/advisories/RUSTSEC-2026-0173.html), already present in the baseline through Leptos macros, reactive-stores macros, and rstml/syn_derive. Rust also reports future incompatibility in this upstream crate. A follow-up must choose compatible maintained upstream macro releases or review an explicit narrowly scoped policy exception.
2. [`ttf-parser` 0.25.1, RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html), already present through native lopdf. It is not optional in lopdf 0.42. Follow-up should evaluate a lopdf release using a maintained font parser (or a replacement metadata-only PDF reader), preserve existing PDF geometry behavior, and rerun CLI media tests.

Neither advisory currently offers a patched version of the affected crate. They were not suppressed, vendored, or hidden by changing the verification policy. Resolving these upstream dependency migrations and release signing are explicit release prerequisites outside the browser capability-removal implementation. No new browser writer or compatibility layer is needed for either.
