# Independent publication

Websh has three publication boundaries. Content updates do not rebuild the app.

| Repository | Role | Delivery | Authority |
| --- | --- | --- | --- |
| `0xwonj/websh` | Rust/WASM app, styles, pinned owner certificate, native CLI | Immutable IPFS app directory | App CID and chosen gateway/client |
| `0xwonj/websh-content` | Profile, Now, ACK, archive, source declarations | GitHub raw HTTPS at a fixed commit | Owner-signed root manifest |
| `0xwonj/websh-mempool` | Independent public drafts mounted at `/mempool` | GitHub raw HTTPS at a fixed commit | Explicitly unsigned source index |

The `wonjae.eth` ENS content hash points to the IPFS app directory. Direct CID gateway
URLs also open the app. Only a new app release needs an ENS update; content updates
use the GitHub pointers independently of ENS resolution.

## Source snapshots

Each repository has a small repository-root `current.json` containing one full Git
commit. This pointer is discovery information, not signed authority. Resolve it once
per boot or explicit refresh. Read `content/manifest.json` and `content/manifest.sig`
from that commit for the root, or `manifest.json` for an unsigned mount. All subsequent
body URLs use the same commit and declared prefix.

`Manifest` holds one validated file/directory index. Every file commits its exact
SHA-256 and byte length, with a 64 MiB per-file bound below GitHub's file limit. The root additionally has `ReleaseMetadata`: purpose
`websh.content`, site identity, sequence, issuance time, typed home projection, external
mount declarations, canonical publication paths, and the page attestation catalog. The detached PGP signature covers
the exact UTF-8 manifest bytes. `ReleaseId` is their SHA-256, independent of Git's commit
identifier. No serialized field claims that a signature has already been verified.

Only shared `verify_release` constructs `VerifiedRelease`. Both native publication and
WASM reading apply the pinned public certificate, primary fingerprint, authorized signer
policy, supported signature algorithms, signature/key timing, bindings, and revocation
checks. A trust-key rotation requires a new app release. Untrusted manifest validation
also checks canonical NFC paths (at most 1,024 bytes and 32 segments), full Unicode case-fold collisions across file and
directory prefixes, metadata, mount reservations, home fields, and routes. Reserved paths
cannot be reintroduced through a case alias. Native publication rejects LFS pointer files
and writes the exact hashed bytes through Git without filters.

The homepage projection is inline so the cold homepage needs three metadata requests
in two dependency rounds: pointer, then manifest and signature together. The app
preconnects to `raw.githubusercontent.com` and sends credential-free CORS requests.
Mutable pointers use HTTP revalidation; immutable resources use normal HTTP caching.
No REST directory enumeration, third-party Git CDN, or polling is involved. GitHub
controls its edge cache: successful Git push and public visibility are distinct states.

## Independent external mounts

A signed root declaration selects a repository, branch, content prefix, and top-level
mount root. It must explicitly use `trust: "unsigned"`. A root signature authenticates
that selection, not future independent source contents. The browser resolves each
source's pointer and validates its own immutable file index. Failed external loading
leaves root content usable; unchanged sources retain their backend and read identity
when the root changes. Supported source additions require only a root content release.

GitHub is the implemented transport. A future storage provider should implement the same
immutable index/read contract only when there is a real operational need; there is no
speculative provider registry, database service, or mirror deployment.

## Publishing and history

The [CLI workflow](cli.md#content-publication) prepares and verifies a frozen public
input set, explicitly signs required page subjects and then root content, creates content commit `C`, creates a second
commit pointing `current.json` to `C`, and pushes once with fast-forward semantics.
The extra commit avoids self-reference; it is not a second upload or manual operation.
Unchanged publication and failed-push retries reuse the prepared snapshot. Private ACK
inputs and GPG secret keys remain local. CI validates public evidence using a pinned CLI
commit and has no signing key or IPFS credential.

App publication builds independently, uploads the prebuilt directory to public IPFS,
and saves a local receipt containing CID, source commit, file digests, previous CID, and
whether the bundle remained unchanged during upload. Gateway/browser checks establish
retrievability; upload success alone does not. Keep the active and rollback app pins. When a future native content contract changes,
deploy a matching app before publishing migrated live content; an older app can only be
used with a compatible historical content snapshot. There is no compatibility reader.
Only deployment reads `.env`. Content publication never reads it.

An app CID freezes executable assets; it does not freeze the live content displayed by
that app. A snapshot link adds `?content=<commit>&release=<manifest digest>` before the
hash route. Historical root views authenticate the selected bytes and do not replace
the live cache or lower its sequence watermark. Independent external sources remain
live. Preserve published Git history and repository availability for historical links.

`/ledger` presents a deterministic hash chain derived from the signed root manifest.
Publications are ordered by authored date and path. Each block binds the site, its
height, the previous hash, and its publication's indexed metadata and file digests.
Filters preserve block numbers and links. Changes outside publications, such as Now,
leave the chain unchanged. This is a projection of one authenticated snapshot, not an
append-only history across releases: editing an article changes its block and later
hashes. Git owns revision history; no separate chain file is stored. Home and ledger subjects
bind their shared typed projections. Article subjects retain their canonical file-set
messages and optional supplemental Ethereum evidence. All proofs live in
`manifest.release.attestations`, covered by the root signature.

The footer's `sig` button opens the current page's signature details and full snapshot
URL. The [page-signature contract](page-signatures.md) defines subjects, hashes,
verification, and presentation. Root authentication does not grant a page badge:
missing/invalid page evidence, unread bytes, unsigned external mounts, and local
session state never inherit its green check.

## Trust and availability

Mounted bytes remain untrusted input even when owner-signed. Markdown/HTML sanitization
is mandatory. Owned media and relative embedded resources are verified before rendering
through managed Blob URLs. A resource cannot silently cross into another mounted source.
Wallet connection grants no publishing authority; access metadata is advisory.

The [runtime guide](runtime.md) owns cache limits, root sequence ordering, stale-read
rejection, and failure behavior. A valid old signature alone cannot prove newestness,
especially on a first visit or after storage is cleared. Publication time and refresh
status have different meanings. Network failure retains usable verified content;
missing uncached bytes still require the origin.

Ordinary HTTPS gateway delivery trusts the gateway and TLS route for executing app
code. Its embedded verifier cannot authenticate itself. Users requiring app-CID
verification need a verifying IPFS client/gateway; neither the URL nor an `X-Ipfs-Path`
header proves returned bytes. CID retention also requires storage/pinning, not merely
recording the identifier.
