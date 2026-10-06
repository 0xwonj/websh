# Independent app and content publication

Status: target design, recorded 2026-10-06; implementation has not started.
The [current architecture](current.md) and its linked guides describe the running
code until the migration below is complete. Commands and layouts in this document
are implementation targets, not commands already available today.

The app stays on IPFS as part of the site's identity. Content lives in a separate
GitHub repository and is delivered over HTTPS. ENS is removed from website discovery
and release operations. Editing Now or publishing a paper requires a content Git
push, without an app build, IPFS upload, pin management, or app address update.
Wallet connection remains available and grants no write privilege.

App releases have CIDs; root content snapshots have Git commits and signed manifests.
GitHub owns content authoring history, latest-snapshot discovery, and file delivery.
Content has no IPFS publication or fallback path. The everyday operation is edit,
generate/check/sign, and commit/push through one CLI command. Generation and signing
prepare data; they do not rebuild or deploy the app.

## Publication boundaries

| Unit | Owns | Changes when |
| --- | --- | --- |
| App on IPFS | HTML, JS/WASM, rendering, routes, shell grammar/help, styles, fonts, wallet integration, trust key and source configuration | Code, presentation, trust policy, or bootstrap configuration changes |
| Owned content on GitHub | Profile, homepage prose, Now, public ACK data, publications, media, metadata, and external mount declarations | Authored information changes |
| External mounts | Independently published drafts or third-party listings, including mempool | Their source changes |
| Browser state | Wallet session, preferences, and disposable read caches | The local session changes |

Profile/Now and durable publications have different editorial roles, but share one
content repository and one atomic release. Media stays with that release at the
current size. Separate media storage is justified only by actual large-file or
delivery requirements. There is no separate service for each homepage section.

The app repository retains the existing four crates. A new content repository,
`0xwonj/websh-content`, contains public authored inputs and generated
publication artifacts. It is an ordinary checkout, not a submodule. Installed CLI
commands can work on it without a Rust build for every content edit.

```text
websh/                           app and tooling repository
  crates/
  assets/                        app assets and pinned public trust key
  docs/architecture/
  dist/                          ignored app build output

websh-content/                   content repository
  content/                       public files delivered from a fixed Git commit
    .site/
      profile.toml
      now.toml
    .websh/
      mounts/
      ack.commitment.json
      attestations.json          independent publication proofs, when present
    writing/
    papers/
    talks/
    projects/
    manifest.json                generated release metadata and file index
    manifest.sig                 detached signature over exact manifest bytes
  current.json                   current content commit; outside the signed file set
  .websh/local/                  ignored private ACK inputs
```

Nothing private belongs under `content/`. The publisher prepares a validated public
file set in an isolated Git index; it does not commit arbitrary checkout or staged
files. GitHub reads use `<commit>/content/<path>`. The content workflow does not need
Pinata, deployment credentials, or a second storage provider.

## External mounts and mempool

Keep the existing `0xwonj/websh-mempool` repository separate from `websh-content`.
The repositories define independent authoring and publication boundaries:

| Repository | Role | Site path |
| --- | --- | --- |
| `websh` | App and native tooling | Executing app, published to IPFS |
| `websh-content` | Profile, Now, formal publications, and mount declarations | Root content |
| `websh-mempool` | Public drafts and work in progress | `/mempool` |

A source is an independently published content snapshot, a backend transports its
bytes, and a mount attaches it to a site path. GitHub is the supported backend;
repository identity and mount path are configuration. Root bootstrap owns the root
source and its trust policy. Root content may declare public mounts, but cannot
replace root bootstrap or occupy the reserved runtime namespace.

After the app/content split, adding a supported GitHub source requires a valid source
snapshot and a declaration in `websh-content`, followed by content publication. It
does not require an app build. Arbitrary repositories without the native manifest are
not mountable. A new backend implementation would require an app release; no plugin
loader, speculative provider framework, or R2 adapter is part of this work.

Mempool uses the same raw-GitHub discovery and immutable-read mechanism: a repository
root `current.json` selects one complete content commit. Keep its category directories
at the existing repository root and place the generated manifest there; the frozen
commit predates the pointer update. Share the file-index contract, path validation,
transport, hashing, request guards, and bounded cache machinery. Homepage projections
are required for root content only; drafts retain their native status/category fields.
Migrate the owned mempool manifest and publisher to that contract in the cutover,
without an old-format reader or a mutable-branch body-read fallback.

Preserve the lightweight draft workflow: mempool publication generates/checks its
index and commits/pushes its snapshot and pointer; it does not require a root release
signature. Explicit mount policy permits this unsigned source and labels it as such.
File hashes bind bodies to that observed manifest, but cannot authenticate its author;
unsigned draft discovery relies on GitHub HTTPS/repository control. It must not create
an owner-authenticated `VerifiedRelease` or inherit root signature status. Signed-root
sequence checks are not claims of author-authenticated ordering for unsigned drafts.

Each source has its own refresh, request identity, cache namespace, and failure state.
Do not advance or invalidate an unchanged mempool source when Now changes. A mempool
update requires neither a root content push nor an app deployment. Root configuration
changes reconcile mount descriptors: preserve unchanged sources, invalidate removed
or changed sources, and reject late reads from replaced descriptors. External loading
must not block the homepage. A root historical snapshot does not freeze independently
changing mounts; a historical draft link must also identify its own source commit.

Draft promotion remains an explicit import into `websh-content`, with draft-only
metadata converted and the destination validated before publication. It does not
silently delete the draft or publish two repositories together. Keep specialized
draft validation, but share generation and Git publication mechanics with the root
workflow instead of building a second delivery system. The existing mempool command
group owns its publish adapter; unsigned operation is not an option on root publish.

## App access without ENS

The primary app artifact is an immutable public IPFS directory. Use a dedicated or
otherwise operationally controlled gateway with CDN caching for routine HTTP
delivery. Keep direct `ipfs://<app-cid>/` access and a CID-specific HTTPS address.
Do not replace this with a separately built ordinary-hosting copy.

Prefer a CID subdomain for the running app, such as
`https://<app-cid>.ipfs.<gateway>/`. It binds relative JS/WASM/CSS requests to one
release and isolates different CIDs by browser origin, as specified by the
[IPFS subdomain gateway specification](https://specs.ipfs.tech/http-gateways/subdomain-gateway/).
Do not execute arbitrary IPFS applications on a shared path-gateway origin.

A normal domain can be a small stable entry point that temporarily redirects to the
selected CID-specific app URL. That redirect is the only mutable app routing value
required by the baseline. Preserve the app's hash route and snapshot selection
through the redirect; do not permanently cache a redirect to yesterday's release.
No custom domain is required to use the CID URL directly.

This choice avoids serving old cached HTML against a different mutable IPFS root,
where relative hashed assets may disappear during a release switch. It also means
that browser preferences, wallet permissions, and IndexedDB caches are scoped to the
CID origin: a new app CID does not inherit them automatically. Do not add a hidden
cross-origin migration service to work around this browser boundary.

DNSLink can later advertise the same app CID for domain-based IPFS discovery. It is
not needed for the default redirect flow and must not introduce a second manually
maintained release choice. If enabled, derive it from the same app deployment receipt
and report any partial update. IPFS documents both domain discovery and direct CID
access in its [custom domain guide](https://docs.ipfs.tech/how-to/websites-on-ipfs/custom-domains/).

The actual gateway and optional domain are deployment choices. Existing Pinata
integration makes its dedicated gateway worth evaluating, but custom-domain support
does not establish CID-subdomain, header, CORS, or latency suitability. Select the
provider after testing those capabilities and its current plan. Do not assume that
an unconfigured account or public gateway meets them.

App release verification must include index, JS, WASM, CSS, fonts, and a complete
browser startup through the chosen gateway. Test cold and warm requests. Eliminating
ENS does not itself guarantee low latency: IPFS retrieval, gateway caching, transfer,
WASM startup, and content verification remain separate costs.

## Content release contract

Each root content release contains an exact `manifest.json`, its detached owner signature,
and the files named by the manifest. Use one accepted native format.

The manifest binds:

- A fixed content-signature purpose and site identity.
- A monotonically increasing publication sequence and signed publication time.
- Canonical relative paths, node metadata, file byte lengths, and SHA-256 digests.
- Typed homepage data generated from profile/Now and public ACK inputs.
- Validated external mount declarations and publication grouping information.

The homepage projection is generated from the same `ContentSnapshot` as the index.
It avoids a sequence of extra metadata requests before the homepage can render.
Authored files remain readable in the virtual filesystem; generation/checking owns
agreement between those files and the projection. Views do not merge an old profile
with a new Now projection.

The manifest does not list itself or its signature as content files. It does not
contain the enclosing Git commit, which would create self-reference. The discovery
pointer and private local state are outside the content directory. Independent
document proofs may be indexed because they do not sign the enclosing manifest.

Define `ReleaseId` as SHA-256 of the exact manifest bytes. The Git commit selects a
snapshot; the signed manifest and its file hashes authenticate the accepted bytes.
The detached signature covers the manifest bytes as emitted, without JSON
parse-and-reserialize during verification. Generation is deterministic; issuance
metadata belongs to explicit signing/publication, not routine build or sync.

The publication sequence orders releases; it is not a schema version or product
version. Retain a cryptographic format discriminator where it has meaning, without
decorative `v2`/`v3` readers or compatibility aliases.

## Discovery and immutable reads

The app pins its site identity, owner public key, content repository, and GitHub raw
HTTPS endpoint. The content repository's `main/current.json` carries one locator:

```json
{
  "commit": "<full-content-commit-id>"
}
```

This locator is advisory and unsigned. It cannot change the repository, trust key,
delivery endpoint, protocol, or validation policy. Authentication happens when the
app verifies the selected manifest. A second signature on the pointer would not
prevent a server from replaying an older, valid signed release.

Read `current.json` once per boot or explicit root refresh, then freeze its commit
for that candidate. Read the manifest, signature, and owned files at that commit.
Never fetch individual documents from `main` after installing a release.

The authenticated manifest establishes the candidate's `ReleaseId`, subject to the
live publication ordering policy. Every body must match its expected length and
digest. Bound request times and retries; a failed read must not silently switch to
another snapshot. When GitHub is unavailable, use an already verified cache where
available and report unavailable reads otherwise. Do not add IPFS uploads, a content
gateway, or a provider failover framework to this path.

GitHub documents [commit-specific file links](https://docs.github.com/en/repositories/working-with-files/using-files/getting-permanent-links-to-files).
Reading a raw locator avoids a browser REST lookup of the branch head, which would
otherwise share GitHub's [60 unauthenticated requests per hour per IP](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api).
Raw delivery still has caching and availability limits; a push is not a guarantee of
instantaneous visibility at every edge.

GitHub is the network dependency for both discovery and content delivery. Existing
verified caches improve availability; they do not replace the origin. Signatures
authenticate content but do not prove first-visit latestness or guarantee service
availability. Content uses neither IPNS nor DNSLink. GitHub Pages and a separate CDN
deployment are also unnecessary for this raw-file baseline.

### GitHub read performance

Use `raw.githubusercontent.com` directly for public content. Avoid GitHub HTML/blob
pages, redirecting download URLs, and REST tree/file enumeration. Add one early
`preconnect` hint for the raw origin in app HTML, matching the credential-free CORS
fetch mode. Treat it as a browser optimization hint, not a latency guarantee; see
[preconnect behavior](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/rel/preconnect).

The initial homepage needs three logical content requests in two dependency rounds:
`current.json`, then the selected manifest and signature in parallel. The manifest
already contains the homepage projection and route catalog. Do not fetch each
profile/Now/ACK source file before rendering, or wait for wallet setup, independent
external mounts, article bodies, or PDFs. Coalesce concurrent requests for the same
immutable resource and load document/media bodies only when needed by the view.

Use browser-managed conditional revalidation (`cache: no-cache`) for the mutable
pointer on boot or explicit refresh. Use normal HTTP caching for commit-specific
manifest/signature/body URLs. Do not add timestamp query parameters, indiscriminate
`no-store`, or manual conditional headers that introduce unnecessary CORS preflights.
[Fetch cache modes](https://developer.mozilla.org/en-US/docs/Web/API/Request/cache)
control the browser cache; they do not grant control over GitHub's edge cache or
guarantee that a new push is visible immediately.

Restore and revalidate the IndexedDB snapshot concurrently with discovery. Show a
usable verified cached snapshot without waiting for the network, then install a
newer verified snapshot atomically. An unchanged commit can reuse its in-memory
release while its trust policy and verification evidence remain valid, without
another manifest download or signature check.
Digest-and-length body keys allow unchanged verified files to survive a new content
commit; do not invalidate the whole cache when only Now changes.

Keep this a single GitHub transport. Third-party Git CDNs, an R2 mirror, proxy servers,
and aggressive polling would add operations outside the chosen baseline. GitHub
controls response compression and CDN policy; app configuration cannot set their
cache lifetime. Optimize request count, payload size, connections, and local reuse.

## Trust and rendering

Reuse the existing owner PGP identity. Keep its public certificate and authorized
signer policy in app-owned assets, independent of downloaded profile data. Private
keys stay in local GPG. CI validates public artifacts; it does not receive the owner's
private key. Rotating the pinned trust identity requires an explicit app release.

Move actual signature verification into a shared, byte-oriented verifier usable by
native code and WASM. Restrict it to the accepted signing policy, including authorized
signing keys, key bindings, algorithms, and expiry/revocation semantics. rPGP has
[WASM platform support](https://github.com/rpgp/rpgp/blob/main/docs/PLATFORMS.md), but
the current app intentionally excludes it. Browser compatibility, verification cost,
and compressed bundle impact are a first implementation gate. Do not substitute a
remote verification Boolean or hand-written OpenPGP parser to avoid that gate.

Only a successful verifier constructs `VerifiedRelease`. Its status is runtime
evidence, not a serialized `verified: true` field. Validate manifest bounds, paths,
metadata, routes, and required homepage fields before building a candidate. Verify
file bytes before parsing, rendering, or placing them in an accepted body cache.

Owned PDFs and images use verified bytes and owned Blob URLs. Direct remote media
URLs must not bypass the release check. Resolve relative Markdown/HTML resources
against the selected content release rather than the app directory. Arbitrary linked
external media is not authenticated by a signature on the document containing its
URL. Sanitization still applies to signed Markdown/HTML, and a content release cannot
install executable app code or alter the app's trust policy.

An owner-signed mount declaration authenticates the chosen source configuration, not
that source's independently changing bodies. External drafts retain separate origin
and verification status. Wallet connection has no effect on publisher authority.

Ordinary HTTPS gateway delivery of app code trusts that gateway and the domain/TLS
route; application-level content checks cannot authenticate their own executing code.
Users requiring verification of the app CID can use a verifying IPFS client/gateway.
Do not claim that a requested CID or an `X-Ipfs-Path` header alone proves the returned
bytes. Likewise, a file SHA-256 is not the CID of a chunked IPFS directory.

## Runtime and cache

Keep `Content`, `Wallet`, `Preferences`, and `RuntimeServices` ownership intact.
Preserve root request sequences, mount epochs, published content revisions, stale
read rejection, and Blob URL disposal. The new loading path prepares a verified
release before it reaches the existing snapshot publication boundary.

1. Show app chrome and start bounded cache restore and live discovery concurrently.
2. Resolve the candidate commit, fetch and verify the manifest/signature, and validate
   its complete catalog and homepage projection.
3. Build the candidate filesystem, routes, proof state, and immutable backends outside
   reactive state.
4. Publish all root state together if the request is still current. Identical release
   bytes do not advance content identity or discard body caches.
5. Refresh independent external mounts and load verified owned bodies on demand.

Use the existing IndexedDB owner for a disposable root release cache as well as
external listing caches. Root records store exact manifest/signature bytes, the
validated commit, and accepted publication ordering. Body keys bind expected digest and
length. Revalidate restored evidence; persist accepted release metadata atomically.
Keep the existing bounded operations and quota failure behavior. Add only a bounded
cache of verified bodies, not full PDF prefetch or an offline app-shell service worker.

Track the highest accepted `(sequence, ReleaseId)` per site/trust identity. Reject a
lower live sequence or a conflicting digest at the same sequence. A higher sequence
reusing the same content is an explicit new publication, not an exception to the rule.
Observe the same ordering in cache transactions so an older tab cannot lower it.

| Failure | Result |
| --- | --- |
| Discovery or refresh fails with a verified snapshot available | Keep it and report that freshness could not be checked |
| Signature, schema, route, or essential data validation fails | Reject the candidate; preserve installed state |
| Body download or integrity check fails | Fail that read; do not substitute another release's file |
| First visit has no usable release | Show app chrome with content unavailable and retry |
| Storage is blocked, full, or unavailable | Continue through the network without a permission prompt |
| A stale request completes | Discard it without altering the installed snapshot or newer work |

A valid old signature is not proof of newestness. Local ordering cannot prevent replay
on a fresh browser or after storage is cleared. Show publication time separately from
last successful refresh. Do not make a quiet personal archive expire merely because
the author has not published recently; cache eviction and semantic expiry differ.
Cached content alone does not make an uncached app available offline.

## Content publishing workflow

The proposed normal entry point is:

```text
websh-cli --root <content-checkout> publish
```

`sync`, `check`, and explicit signing remain useful lower-level operations; `publish`
composes the full owner operation. CLI adapters present results, workflows coordinate
steps, and `infra` owns Git, GPG, network readback, and filesystem effects.

1. Fetch the remote publication state, validate a normal fast-forward starting point,
   and identify the exact public input set. Reject ambiguous staging, unexpected files,
   invalid paths/symlinks, or concurrent publication rather than committing them.
2. Generate the manifest and public artifacts from one input snapshot. If the content
   and relevant policy are unchanged, return the existing verified release.
3. Allocate the next publication sequence, set issuance once, and explicitly sign the
   manifest. Prepare and validate a frozen public tree in an isolated Git index.
4. Commit that tree as content commit `C`. Recheck the commit's bytes against the
   manifest so post-signing local edits cannot leak into the release.
5. Update repository-root `current.json` to `C` in a second commit. Push once with
   normal fast-forward semantics. A remote race requires reconciliation, never force.
6. Check remote Git state and public pointer/manifest reads. Distinguish a successful
   push from public visibility still pending in GitHub's delivery caches.

The CLI creates both commits and performs one push. The second commit avoids a
self-referential commit hash and keeps browser reads off the GitHub REST branch API;
it is not another deployment or a manual operation. The command publishes prepared
content through Git alone: no Trunk build, IPFS upload, pinning, app entry update,
ENS transaction, or wallet interaction occurs.

Recover interrupted work from the local commits, pointer, and fetched remote state.
Reuse a prepared valid snapshot instead of issuing another signature or sequence.
If the push succeeded but public readback failed, report that state and retry the
readback. Do not add a content upload journal or a distributed publication workflow.

Private ACK inputs remain local. Content publishing uses the owner's GPG key and
normal Git authentication. It never reads deployment `.env` or Pinata credentials.

## App publishing workflow

App publication builds and checks the app without a content checkout, uploads the
prebuilt app directory to public IPFS, verifies gateway delivery and browser startup,
then updates the optional ordinary-domain entry. Retain the app CID, source commit,
artifact digests, gateway checks, and previous entry in a deployment receipt.

Pin and warm the new immutable release before changing the entry. If entry update
fails, report the successfully uploaded CID and retain the previous entry. Keep old
app pins available for rollback. No ENS record is read or written during publication
or startup; site branding and a wallet's optional name display are separate concerns
and must not block content loading.

Keep the existing public-IPFS upload adapter scoped to the prebuilt app directory.
Only this adapter reads deployment credentials after build/checks; never load them
into content generation, tests, or signing. Do not build a general deployment provider
framework. Build tooling stays in `justfile`; receipt/error handling belongs to the
native workflow. Source attestations must not be presented as proof of the compiled
WASM; app and content evidence have separate scopes.

## History and retention

The app CID identifies app code; it no longer freezes the live content shown by that
app. An exact shareable snapshot identifies the app CID, content Git commit, manifest
digest, and route. Explicit historical viewing validates the old snapshot but neither
lowers the live sequence watermark nor replaces the cached latest head. Preserve the
content repository's published history; no force-push or history rewriting in the
publication workflow. Historical content links depend on repository availability.

Pin the active app and app rollback release, plus any app releases advertised as
historical snapshots. A CID alone does not guarantee storage, as described in
[IPFS persistence guidance](https://docs.ipfs.tech/concepts/persistence/).
Pinata remains the app upload integration; independent app retrieval is a release
validation item, not something its upload receipt proves. Content history is retained
in Git and has no pin inventory, IPFS retention policy, or gateway warming step.

The current ledger is regenerated from the current tree rather than recording an
append-only publication history. In the target model, `/ledger` remains a catalog of
durable publications derived from the signed manifest. Git and retained releases own
revision history; the snapshot signature owns current-set authentication. Remove the
redundant regenerated block chain and its separate mandatory signature. Profile, Now,
keys, and operational metadata do not become archive publications.

Retain independent article attestations where they provide portable author evidence.
Reissue current owned proofs when their signed contract changes; do not rewrite old
signatures and imply continuity. Historical proof bytes remain in historical releases
and Git, without compatibility readers in the new app.

## Crate ownership and removals

| Owner | Target responsibility |
| --- | --- |
| `websh-core` | Native release contracts, shared verification, filesystem and route validation, read identities |
| `websh-site` | Stable site identity, pinned trust certificate/policy, discovery and transport configuration |
| `websh-cli` | Content generation/check/sign/publish and app deployment workflows; external effects in `infra` |
| `websh-web` | Discovery, immutable transport, verified root installation, cache, rendering, wallet and preferences |

Use public core facades; do not add crate dependencies in the reverse direction.
Keep byte transport separate from release authentication. Views receive verified
state through `Content`, not provider URLs or private key policy.

Remove the following during migration:

- The root source's `gateway: self` dependency and compile-time mutable artifacts.
- Personal profile text hardcoded in Rust; retain app-owned labels/layout/help.
- Homepage subjects mixing Rust sources, theme files, Now, and public ACK data.
- Content sync in Trunk hooks, content watch inputs, and content/crypto directory copies.
- Deployment checks that force app output to match today's separately edited content.
- Generated `verified` Booleans used as browser authority and media verification bypasses.
- The current rebuilt ledger chain and its redundant signing requirement.
- ENS website release instructions and required ENS resolver/gateway access paths.

The new root reader accepts only the new contract. Migrate owned data and disposable
cache storage deliberately; do not retain legacy loaders or bundled-data fallbacks.
An old immutable IPFS app remains a historical release, not a supported compatibility
mode. App-specific routes and protocol/crypto versions with real meaning remain valid.

## Implementation sequence and completion criteria

| Commit phase | Deliverable | Acceptance evidence |
| --- | --- | --- |
| 1 | Shared source index, signed root contract, and real WASM verifier | Valid owner signature succeeds; altered bytes/wrong key fail; unsigned mount evidence stays distinct; compressed size and startup cost measured |
| 2 | Content workspace and pure generation | Root and draft inputs use the shared file index; home/ACK/catalog projections have no app source dependency |
| 3 | Native content publish workflows | Each repository publishes snapshot and pointer in one push; root signing stays mandatory; drafts publish independently; no app build or IPFS tools; interrupted work resumes safely |
| 4 | Remote sources and cache | Atomic per-source install, bounded requests, identity/trust isolation, signed-root replay checks, storage denial behavior |
| 5 | Data-driven homepage and authenticated readers | Profile/Now/ACK update together; owned media and relative assets cannot bypass checks; wallet remains independent |
| 6 | App build and IPFS access separation | Build without content checkout; immutable app asset requests; dedicated gateway and optional entry verified without ENS |
| 7 | Data migration, cutover and cleanup | Root and mempool migrated; independent updates and new app verified; new entry active; obsolete contracts/tools/docs removed |

Use focused owner-layer fixtures, not a second test framework. The key end-to-end
acceptance case publishes a Now-only edit: the content commit and release identity
change, while app build output, app CID, and app entry remain unchanged. Run this case
without Pinata or deployment credentials and verify that it invokes no app build or
IPFS upload. The converse case changes app styling without regenerating or signing
content.

Publish a mempool-only edit without changing the root content commit or app CID.
Verify that adding a supported mount needs only root content publication; failed
external loading leaves root content usable, root refresh preserves unchanged mounts,
and changed/removed declarations reject obsolete mount reads. An unsigned draft must
never acquire root authentication status through rendering or cache restoration.

For performance acceptance, record the three-request/two-round cold-homepage path,
repeat-visit cache restoration, an unchanged-head refresh, and a Now-only update
that reuses unchanged bodies. Measure content-ready time separately from IPFS app
download and WASM startup, including bytes transferred and signature-verification
time. Check actual raw response headers, CORS, and push-to-visible delay. Set latency
budgets from this baseline; do not claim a measured speedup from configuration alone.

Also cover a pointer changing during load; invalid signatures/manifest/body hashes;
retries returning different bytes; root refresh races; cached boot after a
network failure; denied IndexedDB; PDF/image and relative asset verification; historical
viewing without live rollback; and interrupted publication before/after remote success.
Reuse existing tests for unchanged wallet and rendering behavior.

Run the [owning focused checks](verification.md#focused-checks) while implementing,
including actual browser cryptographic verification and a WASM-target check. Run the
relevant wider gate before cutover. Content-only CI validates content generation and
proofs; it does not compile the app for every Now edit. App CI uses a small signed
fixture rather than today's personal content.

The architecture is ready for implementation. Confirm that the planned new
`0xwonj/websh-content` repository name is available before creation; retain the
existing mempool repository. A normal domain is optional, and app gateway/account
selection is a deployment task. PGP bundle cost, GitHub raw-file latency/CORS behavior,
app gateway cold/warm performance, and independent app IPFS retrieval require
measurement before release. These gates do not block starting the contract/verifier
phase and are not claims of completed verification.

On completion, reconcile the CLI, runtime, tooling, and verification guides with the
implemented contracts, then retire this migration proposal into Git history as required
by the repository's documentation policy.
