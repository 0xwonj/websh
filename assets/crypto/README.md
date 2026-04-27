Generated homepage crypto artifacts live here.

`ack.commitment.json` is committed because the homepage uses it at compile time.
`attestations.json` is the page-level subject registry used by the homepage
footer.

Use `websh-cli attest` after changing homepage source or files under
`content/`. The command runs the same manifest builder as
`websh-cli content manifest`, rebuilds all route subjects, and writes
`assets/crypto/attestations.json`. If
`content/keys/wonjae.asc` exists, it also asks local `gpg` to create detached PGP
signatures with `Wonjae Choi <wonjae@snu.ac.kr>` and stores the verified results
in the same JSON file.

Use `websh-cli content manifest` when only `content/manifest.json` needs to be
refreshed and no attestations should be touched.

Low-level `websh-cli attest subject ...` commands remain available for manual
inspection or Ethereum signature import, but the normal publishing flow should
only need `websh-cli attest`.
