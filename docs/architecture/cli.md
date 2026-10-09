# Native CLI

`websh-cli` prepares independent GitHub content snapshots and uploads prebuilt IPFS
apps. Content generation, owner signing, Git publication, and app deployment have
separate boundaries. The browser is read-only; wallet connection grants no write
privilege.

## Commands

```text
websh-cli [--root PROJECT]
  sync
  sign
  check [--require-signatures]
  publish
  attest sign [ROUTE]
  attest message ROUTE
  attest import pgp ROUTE --message FILE --signature FILE
  attest import ethereum ROUTE --message FILE --address ADDRESS --signature SIGNATURE
  mempool sync CHECKOUT
  mempool publish CHECKOUT
  mempool import FILE [--to CATEGORY/SLUG.md]
  ack add NAME --visibility public|private
  ack remove NAME
  ack list
  ack receipt NAME --out FILE
  ack verify FILE
  deploy
```

`--root` selects the content repository for root content commands and the app
repository for `deploy`. Explicit input/output paths and `CHECKOUT` resolve from the
invocation directory. Install with `cargo install --locked --path crates/websh-cli`
from the app checkout; ordinary content edits then require no Rust or app build.
Commands behave identically in a terminal and automation.

```text
cli.rs / commands/   Argument mapping and result presentation
project.rs          Explicit root resolution
workflows/          Content, signing, draft, ACK, publication, deployment use cases
infra/              Git, GPG, HTTP readback, Pinata, deployment env, atomic writes
```

## Source and generation

`websh-content/content/` is the public content tree:

- `.site/profile.toml` and `.site/now.toml` supply the typed homepage projection.
- `.site/profile.txt` supplies the terminal's ASCII `whoami` profile, read on demand.
- Markdown frontmatter supplies Markdown metadata.
- `file.ext.meta.json` supplies authored binary metadata.
- `_index.dir.json` declares a directory/bundle and optional publication grouping.
- `.websh/mounts/*.mount.json` declares independent read-only GitHub sources with
  explicit `trust: "unsigned"`.

Authored metadata and derived facts have separate fields; see the
[native contracts](current.md#native-contracts). Textual YAML fields must be strings:
quote numeric titles/tags and hexadecimal addresses. Publication dates are explicit
source values, independent of Git history. Nothing private belongs in `content/`.

| Generated path in the content repository | Owner and contents |
| --- | --- |
| `content/manifest.json` | `sync`: file index, homepage, mount declarations, publication paths, page attestations |
| `content/.websh/ack.commitment.json` | ACK generation: public acknowledgement commitment |
| `content/manifest.sig` | `sign`: owner PGP signature over exact manifest bytes |
| `current.json` | `publish`: full immutable content commit, outside the manifest |

Edit authored inputs and run `sync`; do not hand-edit generated outputs. A pure
`ContentSnapshot` interprets one source tree. Manifest file hashes include authored
metadata sidecars and generated public proofs. The manifest and its detached
signature exclude themselves. A plain directory groups into one publication only
with `"group": true`; bundles group implicitly. `.site/` and `.websh/` are operational
content, not durable publications. `/ledger` derives its block chain from the root
manifest at read time rather than storing a separately generated chain.

Generation never invokes GPG, Git, an app build, or network access, and never reads
`.env`. It preserves existing issuance metadata and matching page proofs; only
explicit signing allocates a new sequence/time. Initial generation uses zero for
unissued sequence/time. Unchanged outputs retain their bytes and modification times.
Individual replacements are atomic; rerunning sync repairs an interrupted generated
set. Damaged retained evidence requires explicit restoration from Git; the inline catalog
is part of the manifest, not a separately fetched file.

## Signing and checking

`sign` prepares current content, signs changed or missing required page subjects,
then issues and signs the finalized manifest through local GPG. It verifies every
proof with the app's pinned policy before writing the public artifacts. Unchanged
valid page and root signatures retain their exact bytes and issuance. Source changes
while GPG runs abort before generated output replacement. Private keys never enter
the content repository or CI.

`check` verifies generated projections and any existing root signature and portable
evidence without writes. `check --require-signatures` additionally requires the root
manifest signature and a valid owner signature for every required page subject. Root publication
uses the same verifier as WASM, including site identity, authorized signer, algorithms,
key binding, expiry/revocation, and manifest/body integrity rules. A stored Boolean
never grants verification status.

`attest sign [ROUTE]` signs selected subjects in the inline catalog. Ordinary `sign`
and `publish` already sign all required subjects; these expert commands support
separate export/import and supplemental evidence. Export/import preserve
exact subject bytes and reject stale requests. [Page signatures](page-signatures.md) owns
the required route set and canonical message format:


```bash
websh-cli --root ../websh-content attest message /writing/example > request.txt
websh-cli --root ../websh-content attest import pgp /writing/example \
  --message request.txt --signature signature.asc
```

Ethereum import verifies a personal-message signature; it performs no blockchain
transaction and cannot replace the owner's page or root PGP signature. Changing portable
proofs changes the containing root snapshot, which must be signed before publication.

ACK commands keep private names and nonces in `.websh/local/crypto/`. Public commitments
live under `content/.websh/`. Receipts export only to an explicit path and bind the
current public commitment; no receipt cache is maintained.

## Content publication

```bash
websh-cli --root ../websh-content publish
```

This is the normal owner operation: generate/check/sign, commit, and push. It needs
normal Git authentication and the owner's local GPG key. It does not invoke Trunk,
IPFS, Pinata, ENS, or a wallet and does not load deployment credentials.

The repository must have an initial setup commit already pushed to `origin`, and a local branch.
The publisher fetches origin, requires a fast-forward relationship, rejects staged
changes and unexpected modified/untracked paths, and locks concurrent publication.
Only an exact pending snapshot/pointer pair may be ahead of origin. Review and push
repository files such as README/CI separately with ordinary Git. Ignore
`.websh/local/` and `.env`; private files are never publisher inputs.

The publisher freezes allowlisted public bytes in a separate Git index. It creates
content commit `C`, verifies its files, then creates a second commit whose repository
root `current.json` points to `C`. Both commits reach origin through one ordinary push.
Git clean filters are disabled for these blob writes. Git LFS pointers are rejected:
restore actual bytes and remove LFS tracking rules before publishing.
The pointer commit avoids self-reference. Remote root evidence establishes the minimum
accepted publication sequence, so reverting local data cannot silently roll readers
back. A source or branch race aborts rather than force-pushing.

A failed push leaves the prepared commits available for retry. Running the same command
again reuses an unchanged signed snapshot and pending commits. Set newer edits aside
until that pending publication finishes; the publisher rejects additional edits while
the pair is unpublished. If origin advanced, reconcile authored inputs onto the fetched
branch and publish again using its accepted sequence. Do not rebase the prepared pair:
rewriting its snapshot commit would invalidate the pointer's exact commit identity.
After a successful push,
Git state and bounded public raw-file reads are checked separately. Delayed or failed
raw readback reports **push succeeded, visibility pending**; it does not issue a new
signature or claim global CDN propagation. No upload journal or second content host is
involved.

## Drafts and mounts

`websh-mempool` remains independent. Categories and Markdown files live at its repository
root. `mempool sync CHECKOUT` generates its unsigned native `manifest.json` without
network or signing. `mempool publish CHECKOUT` adds the immutable snapshot and
repository-root pointer commits and pushes once through the same Git workflow.
Publication does not change root content or the app CID. It keeps explicitly unsigned
origin status in browser rendering and caches.

Draft frontmatter accepts `title`, `category`, `status`, `priority`, `modified`, and
`tags`. Status defaults to `draft`; supplied values are validated. The category
directory determines the category; unknown fields, nested files, symlinks, and
unsupported category contents fail.

`mempool import FILE` copies one draft into `content/<category>/<slug>.md` or `--to`.
It validates a proposed snapshot before writing, preserves the body, removes draft-only
metadata, and maps `modified` to `date`. It never overwrites, deletes the draft, signs,
or contacts a remote. Run root `publish` after reviewing the imported source.

Adding a supported source means adding a validated mount declaration and publishing
root content. It does not rebuild the app or initialize the external repository.

## App publication

`just publish` builds the app and runs `deploy`. `deploy` accepts the prebuilt `dist/`,
checks its HTML/JS/WASM and symlink boundaries, rejects bundled content, and requires
the shipped owner certificate to match the CLI's pinned production identity before
uploading to public IPFS. This rejects accidentally copied verification fixtures.
It never reads a content checkout, builds, or signs. A build is a trusted step; the
certificate check does not prove the compiled WASM's identity or provenance.

Only the deployment adapter reads `.env`, applying values solely to the Pinata child
process. A successful upload records `.websh/local/deploy/release.json`: CID, source
commit/dirty state, file digests, prior receipt CID, and whether the local bundle remained
unchanged during upload. Receipt-write failure reports the successful CID separately;
it does not turn an upload into a failed publication. Local digests are not proof of
remote retrieval. Verify the gateway and browser, then have the owner update the ENS
content hash to the new app CID and confirm the wallet transaction. The CLI does not
perform this transaction. Content-only publication leaves the app CID and ENS unchanged.
