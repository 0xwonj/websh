# Trunk Attestation Integration — Plan

**Date:** 2026-04-30
**Design:** `docs/superpowers/specs/2026-04-30-trunk-attest-integration-design.md`

## Steps

### Step 1 — `Build` subcommand
- Edit `src/cli/attest.rs`:
  - Add `Build { #[arg(long)] force: bool }` variant to `AttestSubcommand`.
  - Extend the dispatcher to handle it.
  - Implement `attest_build(root: &Path, force: bool) -> CliResult`:
    - If `!force && profile_is_release() == false`: log skip and return.
    - Resolve `no_sign` from `WEBSH_NO_SIGN` env var.
    - Call `run_default(root, no_sign)`.
- Add `fn profile_is_release() -> bool { std::env::var("TRUNK_PROFILE").map(|p| p == "release").unwrap_or(false) }`.
- Add `fn no_sign_from_env() -> bool { std::env::var("WEBSH_NO_SIGN").map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes")).unwrap_or(false) }`.

### Step 2 — Environment fallback in `run_default`
- In `attest::run_default`, OR `no_sign` with `no_sign_from_env()` so any caller path benefits.

### Step 3 — gpg probe + fingerprint guard
- Add `fn gpg_secret_key_fingerprint(gpg_key: Option<&str>) -> Option<String>`:
  - `Command::new("gpg").args(["--with-colons", "--list-secret-keys"]); if let Some(k) = gpg_key { command.arg(k); }`.
  - Return `None` if the binary fails to spawn or exits non-zero.
  - Parse stdout: split on `\n`, find lines starting with `fpr:`, return the field at index 9 of the first such line.
- In `sign_missing_pgp_attestations`, before the loop:
  - If there are no subjects needing signature, skip both the probe and the fingerprint check.
  - Otherwise, call `gpg_secret_key_fingerprint`. If `None`, print warning, return `Ok(0)`.
  - Compare the returned fingerprint (normalized) to `crate::crypto::pgp::EXPECTED_PGP_FINGERPRINT`. On mismatch, return error.

### Step 4 — Trunk hook
- Append the new `[[hooks]]` block to `Trunk.toml`.

### Step 5 — Deploy simplification
- Edit `src/cli/deploy.rs::pinata`:
  - Remove the `attest::run_default(root, no_sign)?` call (and any `super::attest` usage that becomes dead).
  - Build the env vector for `run_trunk`: copy `.env` values, push `("WEBSH_NO_SIGN".to_string(), "1".to_string())` when `no_sign` is true.
  - Update the doc comment on `pinata` to describe the new flow.
- Verify no other uses of `attest::run_default` exist; if there is one (e.g. a top-level `attest` with no subcommand) the existing default still works.

### Step 6 — Tests
- Add unit tests in `attest.rs` (private `mod tests`):
  - `no_sign_from_env_*` — covers `1`, `true`, `TRUE`, `yes`, `0`, empty, absent.
  - `profile_is_release_*` — covers `release`, `dev`, empty, absent.
- Run `cargo test --lib`.

### Step 7 — Local verification
- `cargo fmt --all && cargo clippy --workspace --all-targets && cargo clippy --target wasm32-unknown-unknown --lib`.
- `cargo check --lib && cargo check --target wasm32-unknown-unknown --lib`.
- `trunk build` (dev) — should print the skip message; CSS bundle and manifest still update.
- `trunk build --release` — should run the full attest flow.

### Step 8 — Code review
- Spawn `superpowers:code-reviewer` with the diff + the design doc + this plan.
- Address CRITICAL / HIGH findings inline; fold MEDIUM/LOW where reasonable.

### Step 9 — Commit
- One commit covering: `attest.rs`, `deploy.rs`, `Trunk.toml`, design doc, plan.
- Message draft:
  ```
  feat(cli,build): integrate attestation refresh into release trunk builds

  - websh-cli attest build: TRUNK_PROFILE-aware entrypoint that runs
    the manifest/ledger/attestation refresh (and PGP signing of new
    subjects) only when the build profile is release. Skips silently
    on dev builds and trunk serve.
  - WEBSH_NO_SIGN=1 env disables signing across all callers.
  - gpg detection: missing binary or absent key on a release build
    now warns and continues with subjects pending instead of erroring
    the trunk build.
  - Fingerprint guard: refuses to sign with a key whose fingerprint
    doesn't match content/keys/wonjae.asc, protecting forks/co-authors.
  - Trunk.toml grows a pre_build hook that calls `attest build`; the
    deploy command drops its explicit attest call (now done by trunk)
    and propagates --no-sign through the env.
  - Doc set in docs/superpowers/{specs,plans}/2026-04-30-trunk-attest-
    integration-{design,plan}.md.
  ```

## Risks

- `cargo run --bin websh-cli` cold-start adds latency to every build. Same risk as the existing manifest hook; mitigated by trunk's incremental builds and the early-return on dev profile.
- gpg's `--with-colons --list-secret-keys` colon-separated output format is stable but verbose. The parser only looks for `fpr:` lines and the field at index 9 — if gpg ever changes the schema, the probe returns `None` and the build degrades to "subjects pending" rather than failing.
- Removing the explicit attest call from deploy means `--no-build` no longer touches attestations. Documented; users who want a refresh without rebuild should run `attest build --force` manually.
