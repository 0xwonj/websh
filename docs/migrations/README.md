# Migration operations

The application accepts only the current metadata and mount models. It contains no
compatibility readers or startup migration paths.

## Repository content

Tracked content now uses authored Markdown frontmatter, binary `file.ext.meta.json`,
and directory/bundle declarations. Derived metadata is regenerated into manifest and
ledger snapshots. Run `websh-cli sync`, inspect `websh-cli check`, then explicitly sign
for release. Changed subjects remain pending; previous signatures are not transferred
to different content. No compatibility reader remains in the application.

## External mempool

The current producer is this repository's native CLI. To rebuild an external checkout:

```bash
cargo run --locked -p websh-cli -- mempool sync /path/to/websh-mempool
```

The command validates authored category Markdown and regenerates the manifest. It does
not interpret an old manifest or publish remotely.
[websh-mempool.patch](websh-mempool.patch) is the generated migration against commit
`53396fdac510b3911c6e7028ea52dfdce9434c4c` of `0xwonj/websh-mempool`. It has been
applied and verified locally. After applying it, rerun the current sync command before
committing the external checkout. Activate the new app through the owner's ENS contenthash
update, then push the external manifest and verify the live listing. The old app requires
the former metadata shape, so publishing the manifest early would break its external listing.

## Browser preferences

Run [migrate-browser-state.js](../../tools/migrate-browser-state.js) once in the browser
console on the site's origin to preserve reader scale, game score, and saved theme aliases.
The script preserves an existing new-key preference, removes an old key only after saving
its replacement, and removes the retired session token without reading it. It is repeatable.
It is never included in the application bundle.

The disposable external listing cache can refill automatically; incompatible records are
misses. No old IndexedDB draft data is opened or deleted by this migration. Inspect each
known browser/profile and origin, preserve any authored drafts, and verify the resulting
preferences before retiring this tool.
