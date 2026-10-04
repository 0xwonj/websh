# Native model refactor

Status: completed and verified on 2026-10-04.
All work is local on `codex/read-only-browser`; no push or deployment.

## Result

- Retain four crates with enforced dependencies: shared domain/engine, site identity,
  native CLI, and browser app. Contextual errors belong to their owning facades.
- Publish immutable, validated content snapshots. `Content` owns mounts, backends,
  read identities, and publication. Routing reuses the snapshot's validated catalog;
  individual reads do not clone the filesystem or backend registry.
- Separate `Wallet` and `Preferences` ownership. Guard account, chain, ENS, and connect
  results against newer events and disconnects. Dispose provider listeners explicitly.
  Derive the visual theme from the canonical environment preference.
- Accept one strict content model. Remove browser authoring, credential flows,
  draft persistence, unused contracts, compatibility aliases, and decorative versions.
  Preserve native CLI authoring/publishing, browser reading, and wallet identity.
- Keep only bounded, disposable external listings in IndexedDB. Retain its required
  structural version 1 and cryptographic v1 domain separators.
- Validate terminal input before retaining it. History stores replayable executed
  arguments; repeated history expansion preserves literal shell characters.
- Pin tools and separate setup from verification. Build once in an isolated source copy
  for size checks and E2E. Verification is unsigned and never installs missing tools.

## Commits

| Commit | Completed work |
| --- | --- |
| `ffb66fa` | Remove browser authoring and isolate external listing cache |
| `77abca3` | Simplify native contracts, validate immutable snapshots, migrate owned metadata |
| `1b822ad` | Preserve unreadable/malformed attestation artifacts instead of overwriting them |
| `bdeae53` | Isolate content, wallet, preferences, platform requests, and terminal history |
| `4421ad4` | Reproducible verification, dependency hygiene, and tooling regression coverage |
| `8d5a919` | Test the vendored derive macro independently of browser target features |
| `88a06d6` | Guard same-turn wallet events and cover signed/unsigned attestation presentation |
| `02e433b` | Keep verification artifacts independent of existing release signatures |

The final documentation commit records the integrated evidence, migration operations,
and regenerated attestation inputs. Historical proposals were removed from active docs;
their history remains in Git.

## Verification

`just verify` passed. After the final staged-signature correction, the release build,
size budgets, and complete E2E suite passed again. The exact gate commands live in
[verification.md](../architecture/verification.md).

- Native workspace: 492 tests passed.
- Browser WASM: 194 tests passed, including stale publication/read cancellation,
  cache races and limits, wallet event races, listener disposal, and theme behavior.
- Vendored derive macro: one diagnostic test and four doctests passed, including two
  compile-fail cases. Its standalone lock uses the workspace's dependency versions.
- Tooling: five tests passed, covering browser preference migration, architecture checks,
  and rejection of forged or failing WASM test summaries.
- Native/WASM Clippy, explicit WASM checks, Rust dependency denial, unused dependency
  checks, formatting, CSS lint, and documentation drift checks passed.
- Release build and all 50 E2E scenarios passed. Brotli WASM: 967.9 KiB; total assets:
  1.74 MiB. All existing budgets passed without increases.
- All 36 recorded source content, crypto, and lockfile hashes were unchanged by
  verification. Regenerated staged manifest, ledger, attestations, and CSS matched the
  checked-in source artifacts exactly.
- A signed-fixture staging check confirmed that verification removes only copied
  subjects, preserves source bytes and protocol headers, and rejects malformed input.

The read-only baseline had 479 native, 183 WASM, and 50 E2E tests. Its release Brotli size
was 970.0 KiB WASM / 1.74 MiB total. These are artifact-size measurements, not runtime
latency benchmarks.

## Migration and release boundary

Owned metadata, manifests, and ledger were regenerated through their CLI workflows.
The external mempool patch was applied to its recorded base and checked against the
current producer. Browser preference maintenance is separate, repeatable, and tested;
the application has no startup migration or old-format reader.

See [migration operations](../migrations/README.md). A later release must publish the
external manifest update and sign the six changed attestation subjects with the owner's
key. No deployed browser state, remote repository, or site was changed by this refactor.

Rust dependency denial passes without new ignores; the existing RSA and `paste`
exceptions remain documented in `deny.toml`. One upstream npm advisory remains
in Stylelint's development dependency chain, with no published fix. It is disclosed in
the verification contract; the CSS gate uses source-text linting and fixed repository
configuration rather than accepting filename globs. The dependencies are not shipped.

## Reviews

Independent internal architecture, contract/migration, and tooling reviews were completed.
Integration review also fixed history replay, a same-turn wallet event race, unsafe
attestation read-error fallback, and WASM harness completion/deadline handling.

Claude Opus 5.5 xhigh was attempted, but the installed CLI was not authenticated.
No Claude review is claimed.
