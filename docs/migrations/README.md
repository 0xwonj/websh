# Migration operations

The application accepts only the current metadata and mount models. It contains no
compatibility readers or startup migration paths.

## Repository content

Tracked metadata, manifest, ledger, and attestation subjects were regenerated with the
owning CLI workflows. Removing the metadata schema field changes metadata hashes. Six
subjects are now pending; existing signatures were not carried onto changed subjects.
Signing remains a separate release action with the owner's key.

## External mempool

The current producer is this repository's native CLI. To rebuild an external checkout:

```bash
cargo run -p websh-cli -- mempool manifest --repo-dir /path/to/websh-mempool
```

The command rebuilds from canonical Markdown content and does not interpret the old manifest.
[websh-mempool.patch](websh-mempool.patch) is the generated migration against commit
`53396fdac510b3911c6e7028ea52dfdce9434c4c` of `0xwonj/websh-mempool`. It has been
applied and verified locally. Publish that repository's updated manifest together with
the new app when performing the release. This refactor does not push or deploy either repo.

## Browser preferences

Run [migrate-browser-state.js](../../tools/migrate-browser-state.js) once in the browser
console on the site's origin to preserve reader scale, game score, and saved theme aliases.
The script preserves an existing new-key preference, removes an old key only after saving
its replacement, and removes the retired session token without reading it. It is repeatable.
It is never included in the application bundle.

The disposable external listing cache can refill automatically; incompatible records are
misses. No old IndexedDB draft data is opened or deleted by this migration. Browser state
on a deployed origin has not been changed by the local refactor.
