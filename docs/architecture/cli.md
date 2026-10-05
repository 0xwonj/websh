# Native CLI

`websh-cli` owns the archive's native content rules and publication checks. Editors and Git
handle authoring and repository history; Trunk builds the app; GPG signs explicit requests;
Pinata uploads the checked bundle.

## Commands

```text
websh-cli [--root PROJECT]
  sync
  check [--require-signatures]
  attest sign [ROUTE]
  attest message ROUTE
  attest import pgp ROUTE --message FILE --signature FILE
  attest import ethereum ROUTE --message FILE --address ADDRESS --signature SIGNATURE
  mempool sync CHECKOUT
  mempool import FILE [--to CATEGORY/SLUG.md]
  ack add NAME --visibility public|private
  ack remove NAME
  ack list
  ack receipt NAME --out FILE
  ack verify FILE
  deploy
```

`--root` selects project-owned source and output paths. Explicit input/output file arguments and
`CHECKOUT` resolve from the invocation directory. Commands do not infer interactive behavior or
change their meaning between terminals and automation. Use `cargo run --locked -p websh-cli --
<command>` from this workspace.

## Ownership

```text
cli.rs / commands/   Clap parsing, argument mapping, result presentation
project.rs          project root resolution
workflows/          content, verification, signing, draft, ACK, and deploy use cases
infra/              typed GPG/Pinata results, deployment environment, atomic file writes
```

## Source and generation

Source lives under `content/`:

- Markdown frontmatter for Markdown metadata.
- `file.ext.meta.json` containing authored fields for a binary file.
- `_index.dir.json` declaring a directory or bundle, its authored fields, and explicit grouping.
- `content/.websh/mounts/*.mount.json` declaring read-only external mounts.

Authored inputs use `AuthoredMetadata`; computed facts use `DerivedMetadata` in generated
records. See the [native contracts](current.md#native-contracts). Publication dates are explicit
source values, independent of Git history.

| Generated output | Contents |
| --- | --- |
| `content/manifest.json` | Filesystem listing and metadata |
| `content/.websh/ledger.json` | Content ledger |
| `assets/crypto/ack.commitment.json` | Public acknowledgement commitment |
| `assets/crypto/attestations.json` | Current subjects and retained/imported signatures |

Source `kind` resolves once into `NodeMetadata.kind`; generated authored metadata has no
second kind field. A plain directory becomes one ledger/attestation unit only with
`"group": true` in its declaration. Bundles group implicitly. Titles, dates, and other
cosmetic metadata do not change publication boundaries.

Edit the source inputs, then run `sync`; do not hand-edit these outputs.

`ContentSnapshot` reads and validates one source tree without writes. The resulting manifest,
ledger, and publication units share the same interpretation of content. `sync` computes the
complete artifact set before replacing changed outputs. Unchanged outputs retain their bytes and
modification times. Atomic replacement protects each individual output; an interrupted
multi-file publication is repaired by rerunning sync. `check` detects missing, stale, malformed,
or invalid artifacts without repairing them.

Generation never signs or depends on GPG, repository history, network access, or release
environment variables. [Tooling](tooling.md#builds-and-outputs) describes Trunk integration.

## Signing and checking

`sync` retains valid attestations whose subject content still matches. Changed or new subjects
remain pending. Issuance belongs to the explicit signing request, so unrelated builds and date
changes do not create new messages.

`attest sign` signs current subjects with the configured site PGP identity. A route limits the
operation to one subject. `check` verifies any existing signatures and reports pending subjects;
`check --require-signatures` requires the deployed site's PGP signature on every current
subject. An arbitrary PGP key or Ethereum signature cannot satisfy that release policy.

For an external signer, export the actual plaintext message:

```bash
cargo run --locked -p websh-cli -- attest message /ledger > request.txt
# Sign the exact bytes of request.txt with the configured site's PGP key.
cargo run --locked -p websh-cli -- attest import pgp /ledger \
  --message request.txt --signature signature.asc
```

Ethereum import uses the same message file with `--address` and `--signature`. It verifies a
personal-message signature and stores supplemental evidence; it sends no blockchain transaction.
Import checks the message's subject, issuance, and current content before accepting the
signature. A stale request fails rather than being silently reconstructed around today's
content.

ACK commands manage local source and public commitments. Private names and nonces remain under
`.websh/local/crypto/`; public commitments are generated assets. Receipts are exported only to
an explicit destination and bind one commitment. No receipt cache is maintained. Export and
verification use the current published commitment.

## Drafts and mounts

Author drafts in an ordinary external Git checkout. `mempool sync CHECKOUT` validates category
Markdown files and generates the current `manifest.json`; commit and push with Git. Draft
frontmatter accepts `title`, `category`, `status`, `priority`, `modified`, and `tags`. Status
defaults to `draft`; other supplied values are validated. The category directory determines the
published category. Unknown fields fail.

`mempool import FILE` copies a local draft into `content/<category>/<slug>.md`, or the explicit
`--to` path. It validates a projected content snapshot before writing, rejects existing
destinations, preserves the body, removes draft-only metadata, and maps `modified` to the
canonical `date`. It changes only the new source file; run sync afterward. It never signs,
commits, deletes the draft, or touches a remote.

Mount declarations deserialize into the shared validated `GitHubMount` contract. Connecting an
external repository does not initialize or modify it.

## Publishing

`just publish` is the thin ordered recipe: sync, explicitly sign, release build, deploy. For
offline signing, import signatures first, then use `just build` and the `deploy` command. The
build hook preserves matching signatures.

`deploy` accepts only the fixed prebuilt `dist/` directory. It checks current project artifacts,
site signatures, and matching bundled files before any upload. It never builds, signs, or
repairs a bundle. The consistency check does not prove compiled JavaScript/WASM provenance; the
release build remains a separate trusted step.

Deployment alone reads `.env`, applying its values only to the Pinata child process. The adapter
selects the public network, parses the structured response, and validates the returned CID.
`.websh/local/deploy/cid` records the successful upload. A receipt-write failure reports the
remote success and CID rather than suggesting that publication failed. ENS contenthash updates
remain an explicit owner operation.
