Generated public cryptographic artifacts live here.

`ack.commitment.json` is the homepage acknowledgement commitment.
`attestations.json` contains current page and document subjects and their signatures.

Run `websh-cli sync` after changing source. It computes the current manifest, ledger,
ACK commitment, and subjects before writing generated outputs. Matching signatures are
preserved; changed or new subjects remain pending. Generation never invokes a signer.

Run `websh-cli attest sign` for explicit local PGP signing. For external signing, export
`websh-cli attest message ROUTE > request.txt`, sign those exact bytes, then import with
`websh-cli attest import pgp ROUTE --message request.txt --signature signature.asc`.
Ethereum import accepts the same plaintext request plus `--address` and `--signature`.
Ethereum signatures are supplemental; strict publication requires the site's PGP identity.

`websh-cli check` validates current outputs without writes. Add `--require-signatures`
for the strict release policy. `just publish` composes generation, explicit signing,
release build, and checked deployment. See `docs/architecture/cli.md` for the full contract.
