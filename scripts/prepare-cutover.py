#!/usr/bin/env python3
"""One-time page-catalog cutover. Prepare local commits; never push or sign.
Remove after the matching app and content have been published.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

APP = Path(__file__).resolve().parents[1]
PREDECESSOR = "dc6703362887f405735ca51436143918334b0953"
DIGEST = "5eb4c56efe4f036411e2634b2ef07123986785ad99b3352b86b63db59ab5f322"
OWNER = "6CA8E0E8E0F9B9EE2F92EE49BEE7501AEA7758AD"
SNAPSHOT_MESSAGE = "Migrate content to native page signatures"
POINTER_MESSAGE = "Select native page-signature snapshot"


def run(args, cwd, **kwargs):
    return subprocess.check_output(args, cwd=cwd, **kwargs)


def prepare(root):
    root = root.resolve(strict=True)
    git = lambda *args: run(["git", *args], root)
    if Path(git("rev-parse", "--show-toplevel").decode().strip()).resolve() != root:
        raise ValueError("expected content repository root")
    if git("diff", "--cached", "--name-only"):
        raise ValueError("set staged changes aside first")
    if (root / "current.json").is_symlink():
        raise ValueError("current.json must not be a symlink")
    branch = git("symbolic-ref", "--short", "HEAD").decode().strip()
    # Fetch only: this operation never changes public refs.
    git("fetch", "origin", branch)
    remote = git("rev-parse", f"origin/{branch}").decode().strip()
    pointer = json.loads(git("show", f"{remote}:current.json"))
    if pointer != {"commit": PREDECESSOR}:
        raise ValueError("remote is not the reviewed predecessor; do not reuse this migration")
    old = git("show", f"{PREDECESSOR}:content/manifest.json")
    sig = git("show", f"{PREDECESSOR}:content/manifest.sig")
    if hashlib.sha256(old).hexdigest() != DIGEST:
        raise ValueError("predecessor digest differs")
    # Verify the exact original bytes before using the reviewed sequence 3 watermark.
    with tempfile.TemporaryDirectory(prefix="websh-cutover-") as tmp:
        home = Path(tmp)
        os.chmod(home, 0o700)
        (home / "manifest").write_bytes(old)
        (home / "signature").write_bytes(sig)
        base = ["gpg", "--homedir", tmp, "--batch", "--no-auto-key-retrieve"]
        run([*base, "--import", str(APP / "assets/crypto/site.asc")], root, stderr=subprocess.DEVNULL)
        status = run([*base, "--status-fd", "1", "--verify", str(home / "signature"), str(home / "manifest")], root, stderr=subprocess.DEVNULL).decode()
        valid = [line.split()[2:] for line in status.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
        if len(valid) != 1 or valid[0][-1] != OWNER:
            raise ValueError("unexpected predecessor signer")
    subprocess.run([str(APP / "target/debug/websh-cli"), "--root", str(root), "check", "--require-signatures"], check=True, cwd=APP)
    manifest = json.loads((root / "content/manifest.json").read_bytes())
    if manifest["release"]["sequence"] <= 3:
        raise ValueError("new release must advance the authenticated predecessor sequence")
    allowed = lambda p: p.startswith("content/") or p in ("current.json", "README.md", ".github/workflows/check.yml")
    changed = git("diff", "--name-only", "-z").decode().split("\0") + git("ls-files", "--others", "--exclude-standard", "-z").decode().split("\0")
    if any(p and not allowed(p) for p in changed):
        raise ValueError("unrelated working changes")
    pending = git("rev-list", "--reverse", f"{remote}..HEAD").decode().splitlines()
    subjects = [git("show", "-s", "--format=%s", c).decode().strip() for c in pending]
    if subjects not in ([], [SNAPSHOT_MESSAGE], [SNAPSHOT_MESSAGE, POINTER_MESSAGE]):
        raise ValueError("unexpected local commits")
    if len(pending) == 2:
        expected = {"commit": pending[0]}
        if json.loads((root / "current.json").read_bytes()) != expected or any(changed):
            raise ValueError("prepared cutover has newer edits")
        print(f"Already prepared: {pending[0]}")
        return
    # Whitelist every public file; never stage author-private or unrelated files.
    if not pending:
        git("add", "-A", "--", "content", "README.md", ".github/workflows/check.yml")
        git("-c", "commit.gpgsign=false", "commit", "-m", SNAPSHOT_MESSAGE)
    else:
        git("diff", "--exit-code", "--", "content", "README.md", ".github/workflows/check.yml")
    snapshot = git("rev-parse", "HEAD").decode().strip()
    pointer_path = root / "current.json"
    if pointer_path.is_symlink():
        raise ValueError("current.json must not be a symlink")
    pointer_path.write_text(json.dumps({"commit": snapshot}, indent=2) + "\n")
    git("add", "--", "current.json")
    git("-c", "commit.gpgsign=false", "commit", "-m", POINTER_MESSAGE)
    git("diff", "--exit-code", snapshot, "HEAD", "--", "content")
    print(f"Prepared snapshot: {snapshot}\nNothing pushed. Deploy the matching app before pushing this cutover.")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Usage: python3 scripts/prepare-cutover.py CONTENT_REPOSITORY")
    prepare(Path(sys.argv[1]))
