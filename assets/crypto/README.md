Generated homepage crypto artifacts live here.

`ack.commitment.json` is committed because the homepage uses it at compile time.
`attestations.json` is the page-level subject registry used by the homepage
footer.

Use `websh-cli attest` after changing homepage source or files under
`content/`. The command runs the same manifest builder as
`websh-cli content manifest`, rebuilds all route subjects, and writes
`assets/crypto/attestations.json`. When the expected local GPG secret key is
available and signing is enabled, it creates detached PGP signatures and stores
verified results in that file. `--no-sign` or `WEBSH_NO_SIGN=1` disables new signing;
unchanged subjects keep existing attestations, while changed unsigned subjects stay pending.

Use `websh-cli content manifest` when only `content/manifest.json` needs to be
refreshed and no attestations should be touched.

Low-level `websh-cli attest subject ...` commands remain available for manual
inspection or Ethereum signature import, but the normal publishing flow should
only need `websh-cli attest`.
