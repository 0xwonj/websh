# Trunk Attestation Integration — Design

**Date:** 2026-04-30
**Status:** Approved (autonomous run)

## 1. Goal

Make `trunk build --release` produce a fully deploy-ready `dist/`: bundled CSS, refreshed manifest, refreshed content ledger, refreshed attestation artifact, and (when keys are available) freshly-signed PGP attestations for any subjects whose `content_sha256` changed. Dev builds (`trunk build`, `trunk serve`) keep their current speed — no attestation work runs there.

Currently the orchestration sits in `websh-cli deploy pinata` which runs `attest::run_default` *before* `trunk build --release`. After this change, the trunk pre-build hook does the attestation step itself when the profile is `release`, so the deploy command becomes a thin wrapper around `trunk build --release` plus the Pinata upload.

## 2. Non-goals

- Generating ack-tree, mempool drafts, or mount declarations (separate workflows).
- Reproducible byte-for-byte release builds (PGP signature timestamps already break this; CID changes anyway).
- Auto-updating the ENS contenthash record (still manual / wallet-side).
- Importing or rotating PGP keys (`crypto pgp import`, `ack add` still manual).

## 3. Constraints

1. Must not break dev builds. `trunk serve` should not invoke gpg or signing at all.
2. Missing gpg / missing secret key on a release build must **not** fail the build. Sign what we can, leave the rest pending, log a warning.
3. Must not let a fork or co-author accidentally sign with their own key. The active gpg secret key fingerprint must match the project's `EXPECTED_PGP_FINGERPRINT` constant; mismatch is a hard error.
4. CI / non-author release builds must have an explicit opt-out: `WEBSH_NO_SIGN=1` env or the existing `--no-sign` deploy flag.
5. The trunk hook must be idempotent. Repeated runs on unchanged content do zero gpg invocations (existing `sign_missing_pgp_attestations` already only signs subjects whose hash changed).

## 4. Architecture

### 4.1 New CLI subcommand: `websh-cli attest build`

```rust
// in src/cli/attest.rs, AttestSubcommand
Build {
    /// Force the attestation flow regardless of TRUNK_PROFILE.
    #[arg(long)]
    force: bool,
}
```

Dispatcher:

1. If `TRUNK_PROFILE` is **not** `release` and `force` is false: print one-line "attest: skipped (profile=<p>)" and return Ok. This is the dev-build fast path.
2. Else: read `WEBSH_NO_SIGN`; if set to `1` / `true` (case-insensitive), pass `no_sign=true` to `attest::run_default`.
3. Call `attest::run_default(root, no_sign)`.

The bare `attest` (no subcommand) keeps its current behaviour — the build flow is on a clearly-named subcommand.

### 4.2 gpg-availability probe

Add a small helper in `attest.rs`:

```rust
fn gpg_secret_key_fingerprint(gpg_key: Option<&str>) -> Option<String>
```

Invokes `gpg --with-colons --list-secret-keys <key>` (or `--list-secret-keys` with no argument) and parses the `fpr:` record. Returns `None` if:
- gpg binary is missing
- the key is not in the keyring
- output is malformed

`sign_missing_pgp_attestations` is wrapped with this probe. If `gpg_secret_key_fingerprint` returns `None` and there is at least one subject needing a signature, log a warning of the form

```
attest: gpg unavailable or signer key not in keyring; <N> subjects left pending
```

and return `Ok(0)` instead of erroring.

### 4.3 Fingerprint guard

Before any signing, compare the active secret key's fingerprint to `crate::crypto::pgp::EXPECTED_PGP_FINGERPRINT`. On mismatch, return an error:

```
attest: active gpg key fingerprint does not match project's expected fingerprint.
  active:   <ACTIVE>
  expected: <EXPECTED_PGP_FINGERPRINT>
  Refusing to sign with a non-author key. Set WEBSH_NO_SIGN=1 to build without signing.
```

The check runs once at the top of `sign_missing_pgp_attestations` before iterating subjects. Reuses `crypto::pgp::normalize_fingerprint` for comparison.

### 4.4 `WEBSH_NO_SIGN` env-var handling

Two entry points read it:

1. `attest build` subcommand — passes through to `run_default`.
2. `attest::run_default` — defensively re-checks the env var and ORs into `no_sign`. This guards against direct callers that didn't go through the subcommand (notably future tests and any caller that forgets to plumb through).

### 4.5 Trunk pre-build hook

Append to `Trunk.toml`:

```toml
# Refresh the attestation artifact (and sign newly-changed subjects when a
# matching gpg secret key is available) on release builds. Dev builds and
# `trunk serve` invocations early-return inside `websh-cli attest build`.
[[hooks]]
stage = "pre_build"
command = "cargo"
command_arguments = ["run", "--quiet", "--bin", "websh-cli", "--", "attest", "build"]
```

Same pattern as the existing `content manifest` hook — `cargo run` cold-start is amortized.

### 4.6 Deploy simplification

`src/cli/deploy.rs::pinata`:

- **Remove** the explicit `attest::run_default(...)` call. Trunk now owns it.
- **Add** propagation of `--no-sign` to the trunk subprocess via the env: insert `WEBSH_NO_SIGN=1` into the env vector when `--no-sign` is set, alongside the `.env`-loaded values.
- **Document** the new `--no-build` semantics in the doc comment: with `--no-build`, deploy uploads `dist/` as-is and does *not* refresh attestations. To regenerate without a full build, the user can run `cargo run --bin websh-cli -- attest build --force` then re-run deploy with `--no-build`.

### 4.7 `--no-build` semantics

Before: `--no-build` skipped trunk but `attest::run_default` still ran, which mutated on-disk JSON without re-bundling into dist. The dist could become inconsistent with the attestation files.

After: `--no-build` truly means "upload existing dist as-is". This is more honest and avoids the inconsistency.

## 5. Files touched

| File | Change |
|---|---|
| `src/cli/attest.rs` | Add `Build` subcommand variant; add `gpg_secret_key_fingerprint`, `ensure_signing_key_matches`, env-var read in `run_default`; wire the gpg probe into `sign_missing_pgp_attestations`. |
| `src/cli/deploy.rs` | Drop the explicit `attest::run_default` call; propagate `--no-sign` via env var; refresh doc comments. |
| `Trunk.toml` | Append the `attest build` pre-build hook. |
| `docs/superpowers/specs/2026-04-30-trunk-attest-integration-design.md` | This file. |
| `docs/superpowers/plans/2026-04-30-trunk-attest-integration-plan.md` | Step list (separate file). |

No CSS, no front-end changes.

## 6. Tests

### 6.1 Unit tests added in `src/cli/attest.rs`

- `parses_no_sign_env_truthy_values` — `WEBSH_NO_SIGN` accepts `1`, `true`, `TRUE`, rejects empty / `0` / `false`.
- `parses_trunk_profile_release` — small helper that returns whether the active profile means "release".

### 6.2 Existing tests stay green

`cargo test --lib` (525+ tests) and `cargo check --target wasm32-unknown-unknown --lib` should be unaffected by CLI-side changes.

### 6.3 Manual QA matrix

| Scenario | Expected |
|---|---|
| `trunk serve` with content unchanged | No gpg invocations; site loads fast. Logs include `attest: skipped (profile=)`. |
| `trunk build` (dev) | Same as above. |
| `trunk build --release` with gpg + correct key + content unchanged | `sign_missing_pgp_attestations` reports 0 new sigs. Trunk completes. |
| `trunk build --release` with gpg + correct key + new mempool entry | New subject signed; artifact JSON updated; trunk completes; dist contains fresh artifact. |
| `trunk build --release` with no gpg / no secret key | Build succeeds; warning printed; new subjects remain pending. |
| `trunk build --release` with wrong fingerprint key | Build fails with the comparison error. |
| `WEBSH_NO_SIGN=1 trunk build --release` | Build succeeds; new subjects remain pending; no gpg invocation. |
| `websh-cli deploy pinata` | Same flow but ends with Pinata upload + CID print. |
| `websh-cli deploy pinata --no-sign` | Trunk runs with `WEBSH_NO_SIGN=1`; new subjects remain pending. |
| `websh-cli deploy pinata --no-build` | No trunk invocation; no attestation refresh; uploads existing dist. |

## 7. Risks

- **Cold-start cost**: `cargo run --bin websh-cli` adds ~100–500 ms per build. Same as the existing manifest hook; users already accept it.
- **Hook recursion**: `cargo run` rebuilds the binary if needed and then runs it, but it does not invoke `trunk` again, so no recursion.
- **gpg-agent passphrase prompt**: if the key has a passphrase and the agent is not warm, gpg prompts on stdin/tty. Trunk hooks inherit the parent terminal so this should still work; if the user has `pinentry` configured it pops a GUI prompt. Documented; not solved here.
- **Forking confusion**: someone clones the repo and runs `trunk build --release` with their own gpg key in their keyring. With the fingerprint guard, this errors out with a clear message. They can use `WEBSH_NO_SIGN=1` to proceed.
- **Time-skew non-determinism**: gpg adds a timestamp to each signature, so even unchanged subjects re-signed after a content change are non-deterministic. Acceptable — CID would change anyway.

## 8. Acceptance

- All scenarios in §6.3 pass on manual QA.
- `cargo test --lib` green.
- `cargo check --target wasm32-unknown-unknown --lib` green.
- `cargo clippy --workspace --all-targets` and `cargo clippy --target wasm32-unknown-unknown --lib` green.
- `code-reviewer` clears with no CRITICAL/HIGH.
- One commit lands the entire change.

## 9. Self-review

- **Placeholders / TODOs**: none. Every API name and CLI behaviour is concrete.
- **Contradictions**: none — design and master scope of the redesign work are orthogonal (this is build-pipeline, not visual).
- **Scope creep risk**: tempted to also auto-update ENS or auto-promote mempool — both held off (they are workflow concerns with separate UX questions).
- **Cross-cutting risks**: dev-build performance is the most user-visible — protected by the `TRUNK_PROFILE` check at the very top of the subcommand. If that fails (env var not propagated by trunk), the entire dev path slows down. Mitigation: a fallback heuristic — if the binary detects neither `release` nor any known dev profile (e.g. profile is empty), default to "skip" rather than "run". Implemented in §4.1 by treating "not release" as skip.
- **Failure to detect gpg**: if a key exists but `gpg --list-secret-keys` errors out for an unrelated reason, we'd silently skip signing. Acceptable for now — the warning makes it visible. A future hardening could distinguish "gpg missing" from "gpg present, key missing" from "other errors", but the user-visible behaviour ("subjects pending") is the same.
