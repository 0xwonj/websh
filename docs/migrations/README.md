# Browser migration

Run [migrate-browser-state.js](../../tools/migrate-browser-state.js) once in the browser
console on the site's origin to preserve reader scale, game score, and saved theme aliases.
The script preserves an existing new-key preference, removes an old key only after saving
its replacement, and removes the retired session token without reading it. It is repeatable.
It is never included in the application bundle.

The disposable external listing cache can refill automatically; incompatible records are
misses. No old IndexedDB draft data is opened or deleted by this migration. Inspect each
known browser/profile and origin, preserve any authored drafts, and verify the resulting
preferences before retiring this tool.
