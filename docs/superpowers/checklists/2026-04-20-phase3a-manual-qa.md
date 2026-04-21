# Phase 3a Manual QA Checklist

Run before tagging 3a complete.

## Dev-server smoke (no real commits)

- [ ] `trunk serve`, open in browser.
- [ ] Golden path: wallet connect → `touch` → `edit` → Save → `sync status` → reload → drafts persist.
- [ ] Edge: `rm -r` accepts dir, `rm` alone rejects dir.
- [ ] Edge: EditModal Cancel discards.
- [ ] Edge: `sync auth garbage` rejects.

## Real-commit smoke (throwaway repo + burnable PAT)

- [ ] Configure `config::mount_list()` to point at a test repo (NOT `0xwonj/db`).
- [ ] `sync auth <real-ghp-token-with-repo-scope>`
- [ ] `echo "hello" > /tmp/test.md`
- [ ] `sync commit -m "phase 3a smoke"`
- [ ] Verify on github.com: commit exists, author is the configured admin, `manifest.json` updated.
- [ ] `sync status` shows clean.
- [ ] Reload browser: ctx.fs reflects the new remote state.

## Conflict smoke

- [ ] Draft an edit (don't commit).
- [ ] From github.com UI, commit an unrelated change to the branch.
- [ ] `sync commit -m "..."` — expect `Conflict` error citing new SHA.
- [ ] `sync refresh` — base refreshes; drafts preserved.
- [ ] `sync commit -m "..."` again — succeeds.

## Rate-limit smoke (optional)

- [ ] With a legitimate PAT, make ~5 commits in quick succession.
- [ ] Verify no auto-retry loops.
- [ ] If rate-limited is hit: error surfaces with retry-after seconds.
