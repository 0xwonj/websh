# Page signatures

This document owns the page subject and SIG contract. [Publication](publication.md)
owns root snapshot delivery; the [CLI guide](cli.md) owns generation and owner operations.

## Decision

The footer SIG authenticates the current page's content subject. The root manifest
signature authenticates delivery of the entire content snapshot. These are distinct
signatures and must never substitute for one another in the UI.

Keep the existing footer, popup, row order, colors, hash truncation, and signature
block. Keep the snapshot URL inside that popup as a clickable, horizontally scrolling
single line. Do not add sequence, scope, or a second root-signature block to the popup.
No changes to ledger vocabulary, layout, or the home page design accompany this work.

This authenticates content, not rendered pixels or the executable app. App assets,
CSS, fixed interface wording, wallet state, theme, browser language, today's date,
network status, and independent mempool contents are outside the page subject.
The app CID identifies the renderer. Owned body/media reads still require the root
manifest's integrity checks and existing sanitization.

## Subject boundaries

| Surface | Subject and committed data |
| --- | --- |
| `/` | `home`: typed profile, Now, public ACK, displayed table-of-contents counts, and ordered recent items |
| `/ledger` | `ledger`: full ordered publication view, block commitments, displayed counts, head, genesis, and latest publication date |
| Category listing | `ledger`: canonical category route/filter, selected ordered entries, and the shared counts/head/genesis actually displayed |
| Article, PDF, authored page | Existing `page` or `document`: exact publication file set, including its authored sidecars |
| Bundle or grouped publication | Existing `bundle` or `directory`: exact declared publication unit and its constituent files |
| Independent mount | Unsigned; never use the owner's root or page signature for it |
| Session-only surfaces, errors, unmodelled ordinary directories | No page signature; never fall back to the root signature |

The required subject catalog consists of home, all built-in ledger/category routes
(including empty categories), and all owned publication units. Other listing routes
get a subject only when the route catalog actually exposes that signed listing view;
do not create signatures for arbitrary URL strings. Reject duplicate subject routes
and ambiguous route ownership.

Home counts and recent-item selection must preserve today's behavior, including
which files count, the six-item limit, ordering, and bundle handling. Their pure
computation lives in core, shared by the CLI and browser. A canonical path tie-breaker
for otherwise equal recent items prevents traversal order from changing a commitment. Ledger projection must likewise use the same shared chain and
filter rules as rendering. Commit raw typed metrics, metadata, variant labels/targets,
and ordered entries; font-specific formatted strings stay in the web layer.

Do not include the whole manifest or whole file index in a home/ledger subject.
Include exactly the typed projection inputs/values that define that view. Home can
change when recent items or counts change, even if profile and Now did not. Category
signatures can change after a different category changes because the UI displays a
global chain head and category counts. This is intentional dependency tracking.

Bundle variants share the bundle subject and its canonical route. `/writing/example`
and its `en.md`/`ko.md` variants can legitimately have the same signature. Resolve
membership from the validated catalog; never inherit any arbitrary ancestor's proof.
The popup shows the subject route, so the shared signature is not presented as a
separate signature of each variant. Links to other publications are not transitive
commitments. Shared media outside a publication unit remains protected by root file
integrity, not implicitly added to that unit's portable signature.

## Hashes and canonical messages

Retain existing publication subjects and their canonical messages. Their `content`
digest is SHA-256 of the canonical ordered file commitments, not necessarily SHA-256
of the visible Markdown/PDF alone. A sidecar is part of the unit. Preserve unchanged
valid PGP signatures and supplemental Ethereum evidence byte for byte.

Typed `home` and `ledger` subject variants have a projection digest instead
of `content_files`; do not represent generated view data as imaginary files. The subject API shares route, issuance, and attestation access; file commitments
belong to the file-based variants. There is one native reader.

For these two new variants:

1. Construct `HomePageData` or the dedicated ledger commitment from the shared pure projection.
2. Encode a fixed-order typed structure as compact UTF-8 JSON with no trailing newline.
   Map keys are sorted, ordered lists preserve semantic order, optional fields have
   one encoding, and numeric fields are integers. Rust declaration order is canonical;
   generic JSON value/map serialization is not the contract.
3. `content_sha256 = SHA256(projection_bytes)` with the existing `0x` display convention.
4. Sign the following UTF-8 message, with LF separators and no final newline:

```text
websh.subject.v1
id=route:<canonical route>
site=<configured site>
route=<canonical route>
kind=<home or ledger>
content_sha256=<0x digest>
issued_at=<UTC YYYY-MM-DD>
```

The projection field order is:

- Home: `home`, `counts`, `recent`. `counts` follows `LEDGER_CATEGORIES`; recent
  items use the shared six-item selection and ordering.
- Ledger: `category`, `entries`, `counts`, `total`, `restricted`, `head`,
  `genesis`, `latest`. All-category selection is JSON null; category selections are
  strings. `counts` is a sorted `BTreeMap`.
- Ledger entry: `height`, `hash`, `previous_hash`, `path`, `title`, `description`,
  `date`, `category`, `kinds`, `metric_kind`, `words`, `pages`, `dimensions`, `bytes`,
  `tags`, `variants`, `restricted`. Optional metrics are JSON null when absent.
  UI links and duplicate authored metadata are not serialized into this structure.

Nested profile, ACK, recent-item, and dimension fields follow their typed Serde
contracts. Optional projection values use that type's declared encoding; projection
structs do not serialize through an unordered map. Changing these encodings changes
the commitment and requires regeneration and signing, not a compatibility fallback.

Site and route come from validated data and must reject newline/control injection.
New kinds make this message unambiguous with the retained document message formats;
do not change existing kind encodings under their current cryptographic label.
`message_sha256` is SHA-256 of these exact message bytes. The detached OpenPGP signature
signs the message bytes, not their textual digest. No page sequence or decorative
schema version is added. Existing cryptographic scheme identifiers remain meaningful.

Issuance is assigned by explicit signing, not generation or browser rendering. Reuse
an unchanged valid subject's issuance and signature; a same-day edit still changes
its content/message hash. Home's existing `last revised` field uses the home subject's
issuance date, not the root release date. Its appearance remains unchanged. Today's
`Paper` date remains a local display value and is not signed. Issuance is in the
message, not in the projection whose hash is needed to decide whether to re-sign.

## One authoritative proof catalog

Store the complete subject catalog under `manifest.release.attestations`, using the
existing typed attestation collection extended with home/ledger subjects. Store each
subject once. The manifest's detached signature covers this catalog along with the
existing file index, home data, mount declarations, and publication paths.

There is no standalone attestation file or browser proof fetch. Existing document
messages/signatures retain their meaning inside the inline catalog; moving their
containing JSON does not re-sign them. CLI message export and signature import
operate on this catalog.

Only home/ledger digests and proofs are stored in their subject records; their complete
view data is reconstructed from the manifest's existing inputs. Do not store another
copy of the home projection or serialized ledger page. Document file commitments
remain available for independent verification/export.

The dependency direction is acyclic:

```text
authored content -> base index + home + publication chain
                -> page projections / publication file sets
                -> subject messages -> page signatures
                -> manifest containing subjects -> manifest.sig
                -> immutable Git commit -> current.json
```

No subject includes the proof catalog, root digest/signature, release sequence/time,
Git commit, app CID, or snapshot URL. Exclude operational proof records from chain
inputs. Signing a subject therefore cannot change its own content hash.

Inlining avoids a fourth cold-home metadata request and removes the lazy document
proof fetch. It increases manifest bytes; preserve the existing 4 MiB manifest bound,
validate bounded subject/attestation collections before crypto work, and report the
actual cold metadata size and verification/install timing in browser checks. Do not
claim that three requests alone guarantees unchanged latency. Verify the active page's
proof on demand rather than eagerly verifying every signature at startup.

## Generation and publication

Keep the existing command surface. `publish` remains the normal single owner command.

| Operation | Behavior |
| --- | --- |
| `sync` | Build index/projections/subjects; retain unchanged validated proofs; invalidate changed subjects; never sign |
| `sign` | Sign every missing/stale required owner subject, then sign the finalized root manifest |
| `publish` | Freeze inputs, run the same preparation/signing/checking pipeline, create snapshot/pointer commits, push once |
| `check` | Check generated consistency and every supplied proof without writes |
| `check --require-signatures` | Also require a valid owner proof for every required subject and a valid root signature |
| `attest message/import/sign` | Expert operations on canonical subject routes; imports still reject stale requests |

The normal publish workflow must not require a separate `attest sign` invocation.
Changed subject payloads discard all stale attestations, including Ethereum evidence;
unchanged supplemental evidence is retained and does not substitute for owner PGP.
Do not skip verification merely because a PGP record exists. Native and browser page
verification share the core verifier and pinned policy, as root verification does.

Prepare all output bytes against one frozen input set. Verify all new signatures and
recheck inputs before replacing generated outputs. Reuse unchanged root and page
signatures on no-op runs and failed-push retries. Preserve existing source-race,
fast-forward, private-input, and deployment-env boundaries. A partially interrupted
local write must fail `check` and be recoverable; no pointer may publish an incomplete
set. The root release sequence remains an internal anti-rollback mechanism.

## Browser verification and presentation

`Content` owns the authenticated snapshot and selected page evidence. The footer only
renders that evidence; it must not parse artifacts or verify signatures itself. Use
core facades for projection construction and verification. Keep wallet/preferences
independent and preserve current refresh/read identity guards.

For the current canonical subject, the verifier must:

1. Require a verified root release and an unambiguous catalog subject.
2. Reconstruct the expected projection or complete publication file set from that
   release. Require exact equality, not merely that the proof's listed files are a
   subset of current files. Detect additions, deletions, sidecar changes, and moves.
3. Recompute content hash, canonical message, and message hash; reject stored mismatches.
4. Verify the detached owner signature with the pinned PGP policy.
5. Bind the result to release identity, canonical subject route, message hash, and
   loaded reader identity. Discard late results after refresh/navigation. Do not show
   success while the reader has pending or failed integrity checks for displayed bytes.

`VerifiedSubject` is constructed only by verification; serialized `verified` flags are
never accepted. Keep pending, unsigned, invalid, and verified distinct internally.
Use the existing compact chip/popup to report these states; a missing/failed page
signature never borrows the manifest's green check. A root-valid page may remain
readable when its separate proof is missing/invalid, but its SIG must not say verified.
An invalid root or failed body-integrity check retains existing rejection behavior.

For bundles, a valid proof binds all expected constituent commitments. It does not
claim every variant has already been downloaded and hashed. Actually read bytes still
pass existing integrity checks before rendering. Ethereum-only supplemental evidence
does not make the owner badge verified; retain the explicit browser-unverified state
until a supported verifier actually verifies that evidence.

| UI field | Meaning |
| --- | --- |
| `sig` chip | Shortened page message hash and actual page-verification status |
| `route` | Canonical subject route |
| `content` | Page projection or publication-unit digest |
| `signed by`, `fingerprint`, `scheme` | Selected page signature identity/evidence; fingerprint is the authority, display name is not |
| `message` | `SHA256(<kind> @ <subject route>) = <message hash>` |
| `signature` | Detached signature of that canonical message |
| `snapshot` | Full URL for the containing root snapshot and current route; single-line scrolling link |

The `content` and `message` hashes identify different objects. Verification is shown
on the SIG chip; the popup does not repeat a `verified` row. ACK root and chain head
remain committed within page data without separate popup rows. Snapshot URLs keep
their existing semantics: independent mounts remain live, and a content-only HTTP
origin is not an immutable app identity. Root evidence remains inspectable through the
manifest/signature and CLI, not a second block masquerading as the page signature.

## Change expectations

| Edit | Expected signature changes |
| --- | --- |
| Now/profile/ACK | Home and root; document and ledger subjects unchanged |
| Article body | Its publication unit, affected chain/listing subjects, and root; home only if its displayed projection changes |
| Article title/date, addition/removal | Its unit, affected ledger subjects, root, and home when recent items/counts change |
| One bundle language | Bundle signature changes for every variant; unrelated documents retain their signatures |
| Independent mempool update | No owner page or root signature changes |
| Theme/wallet/date display | No content signature changes |
| App-only styling update | New app CID; no content signing merely because CSS changed |
| No-op publish | No new subject signatures, root sequence, commits, or app build |

## Deployment and cache boundary

The catalog is a required native manifest field. A previous layout must be explicitly
migrated before using this CLI; there is no fallback reader. Cache restoration validates
the current contract and refetches incompatible roots. The existing IndexedDB object
stores are unchanged, so no structural database version bump is needed.

New app and migrated live content must be released together. Preview the matching
pair first and pin content CI to that CLI commit. ENS and a Git pointer do not switch
atomically: coordinate that interval explicitly. A rollback needs a matching old
app/content pair; old historical links may require their original app CID. Page-only
content edits after the cutover use ordinary `publish` and need no app/ENS update.

Publication membership is shared by file-set proofs and chain blocks. A file includes
its authored `.meta.json` sidecar; a group excludes independently declared nested
publications and their sidecars. Chain blocks additionally bind the selected directory
nodes and their metadata. Readers resolve proof membership with the actual file path,
including the Markdown extension; the displayed subject route remains canonical.

The Content owner exposes page evidence to the footer. Its root-only filesystem is
built once per accepted root, and reader readiness remains tied to path and ReadStamp.
Wallet, preferences, and external-mount updates do not rebuild that root projection.
