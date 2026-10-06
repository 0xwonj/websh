//! Frozen Git publication. A private index and the real index lock keep unrelated
//! staged files and concurrent Git writers out of the two-commit transaction.
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::CliResult;
use anyhow::{Context, bail};
use websh_core::publication::SourcePointer;

pub(crate) struct Repository {
    root: PathBuf,
    branch: String,
    head: String,
    remote_head: Option<String>,
    index: PathBuf,
    index_lock: PathBuf,
    _lock: File,
    owns_index_lock: bool,
}

fn invoke(
    root: &Path,
    index: Option<&Path>,
    args: &[&str],
    input: Option<&[u8]>,
) -> CliResult<Vec<u8>> {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    let mut child = command.spawn().context("start Git")?;
    if let Some(input) = input {
        if let Err(error) = child
            .stdin
            .take()
            .context("open Git stdin")?
            .write_all(input)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error).context("write Git input");
        }
    } else {
        drop(child.stdin.take());
    }
    let output = child.wait_with_output().context("wait for Git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}
fn text(bytes: Vec<u8>) -> CliResult<String> {
    Ok(String::from_utf8(bytes)
        .context("Git output is not UTF-8")?
        .trim()
        .to_owned())
}

impl Repository {
    pub(crate) fn open(root: &Path, allowed: impl Fn(&str) -> bool) -> CliResult<Self> {
        let root = root.canonicalize()?;
        let top = text(invoke(
            &root,
            None,
            &["rev-parse", "--show-toplevel"],
            None,
        )?)?;
        if Path::new(&top).canonicalize()? != root {
            bail!("publish --root must select the repository root");
        }
        let git_dir = PathBuf::from(text(invoke(
            &root,
            None,
            &["rev-parse", "--absolute-git-dir"],
            None,
        )?)?);
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(git_dir.join("websh-publish.lock"))?;
        lock.try_lock()
            .context("another content publication is running")?;
        let head = text(invoke(&root, None, &["rev-parse", "HEAD"], None)?)
            .context("create an initial repository commit before publishing")?;
        let branch = text(invoke(
            &root,
            None,
            &["symbolic-ref", "--short", "HEAD"],
            None,
        )?)
        .context("publication requires a local branch")?;
        let staged = invoke(
            &root,
            None,
            &["diff", "--cached", "--name-only", "-z"],
            None,
        )?;
        if !staged.is_empty() {
            bail!("staged changes are ambiguous; unstage them before publishing");
        }
        let changed = invoke(
            &root,
            None,
            &[
                "ls-files",
                "--modified",
                "--deleted",
                "--others",
                "--exclude-standard",
                "-z",
            ],
            None,
        )?;
        for path in changed
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = std::str::from_utf8(path).context("Git paths must be UTF-8")?;
            if !allowed(path) {
                bail!("unexpected change outside the public input set: {path}");
            }
        }
        invoke(&root, None, &["fetch", "origin"], None)?;
        let remote_ref = format!("refs/remotes/origin/{branch}");
        let remote_head = invoke(&root, None, &["rev-parse", "--verify", &remote_ref], None)
            .ok()
            .map(text)
            .transpose()?;
        let remote = remote_head
            .as_ref()
            .context("push the initial repository setup to origin before publishing content")?;
        invoke(
            &root,
            None,
            &["merge-base", "--is-ancestor", remote, &head],
            None,
        )
        .context(
            "remote publication advanced; reconcile authored inputs with origin and republish; do not rebase prepared snapshot and pointer commits",
        )?;
        if remote != &head {
            validate_pending(&root, remote, &head, &allowed)?;
            if !changed.is_empty() {
                bail!(
                    "a prepared publication is pending; set aside newer edits and retry the unchanged prepared snapshot before publishing further changes"
                );
            }
        }
        let index_lock = git_dir.join("index.lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&index_lock)
            .context("Git index is locked by another operation")?;
        let index = git_dir.join("websh-publish.index");
        let repository = Self {
            root,
            branch,
            head,
            remote_head,
            index,
            index_lock,
            _lock: lock,
            owns_index_lock: true,
        };
        repository.run(&["read-tree", &repository.head], None)?;
        Ok(repository)
    }

    fn run(&self, args: &[&str], input: Option<&[u8]>) -> CliResult<Vec<u8>> {
        invoke(&self.root, Some(&self.index), args, input)
    }
    pub(crate) fn remote_file(&self, path: &str) -> CliResult<Option<Vec<u8>>> {
        let Some(head) = &self.remote_head else {
            return Ok(None);
        };
        let reference = format!("{head}:{path}");
        if self.run(&["cat-file", "-e", &reference], None).is_err() {
            return Ok(None);
        }
        Ok(Some(self.read_at(head, path)?))
    }
    pub(crate) fn read_at(&self, commit: &str, path: &str) -> CliResult<Vec<u8>> {
        self.run(&["show", &format!("{commit}:{path}")], None)
    }
    pub(crate) fn contains_commit(&self, commit: &str) -> bool {
        self.run(&["merge-base", "--is-ancestor", commit, &self.head], None)
            .is_ok()
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.remote_head
            .as_ref()
            .is_some_and(|remote| remote != &self.head)
    }

    /// Replace exactly one allowlisted public subtree in the isolated index.
    pub(crate) fn snapshot(
        &self,
        files: &BTreeMap<String, Vec<u8>>,
        allowed: impl Fn(&str) -> bool,
    ) -> CliResult<String> {
        let tracked = self.run(&["ls-files", "-z"], None)?;
        for path in tracked
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let path = std::str::from_utf8(path).context("Git paths must be UTF-8")?;
            if allowed(path) {
                self.run(&["update-index", "--force-remove", "--", path], None)?;
            }
        }
        for (path, bytes) in files {
            self.add(path, bytes)?;
        }
        let tree = text(self.run(&["write-tree"], None)?)?;
        text(self.run(
            &[
                "-c",
                "commit.gpgsign=false",
                "commit-tree",
                &tree,
                "-p",
                &self.head,
                "-m",
                "Publish content snapshot",
            ],
            None,
        )?)
    }
    fn add(&self, path: &str, bytes: &[u8]) -> CliResult {
        let hash = text(self.run(
            &["hash-object", "--no-filters", "-w", "--stdin"],
            Some(bytes),
        )?)?;
        self.run(
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{hash},{path}"),
            ],
            None,
        )?;
        Ok(())
    }
    pub(crate) fn pointer_commit(&self, snapshot: &str, bytes: &[u8]) -> CliResult<String> {
        self.add("current.json", bytes)?;
        let tree = text(self.run(&["write-tree"], None)?)?;
        text(self.run(
            &[
                "-c",
                "commit.gpgsign=false",
                "commit-tree",
                &tree,
                "-p",
                snapshot,
                "-m",
                "Advance current content",
            ],
            None,
        )?)
    }
    pub(crate) fn install(&mut self, commit: &str) -> CliResult {
        fs::copy(&self.index, &self.index_lock)?;
        self.run(&["update-ref", "HEAD", commit, &self.head], None)
            .context("local branch changed during publication")?;
        self.head = commit.to_owned();
        // The real index is locked throughout preparation. Publish the already
        // constructed index without checking out or overwriting authored files.
        fs::rename(&self.index_lock, self.index.with_file_name("index"))?;
        self.owns_index_lock = false;
        Ok(())
    }
    pub(crate) fn push(&self) -> CliResult {
        let target = format!("{}:refs/heads/{}", self.head, self.branch);
        let push = self.run(&["push", "origin", &target], None);
        let advertised = text(self.run(
            &[
                "ls-remote",
                "origin",
                &format!("refs/heads/{}", self.branch),
            ],
            None,
        )?)?;
        if advertised.split_whitespace().next() != Some(self.head.as_str()) {
            if let Err(error) = push {
                return Err(error).context(
                    "prepared commits remain local; rerun publish to retry without signing again",
                );
            }
            bail!("push completed but remote head differs; reconcile without force-pushing");
        }
        Ok(())
    }
    pub(crate) fn branch(&self) -> &str {
        &self.branch
    }
    pub(crate) fn raw_base(&self) -> Option<String> {
        let remote = text(self.run(&["remote", "get-url", "origin"], None).ok()?).ok()?;
        let repo = remote
            .strip_prefix("https://github.com/")
            .or_else(|| remote.strip_prefix("git@github.com:"))?
            .trim_end_matches(".git");
        if repo.split('/').count() != 2
            || !repo
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_./".contains(&byte))
        {
            return None;
        }
        Some(format!("https://raw.githubusercontent.com/{repo}"))
    }
}

/// Only the publisher's two-commit transaction can be retried automatically.
/// Ordinary local commits must be reviewed and pushed separately.
fn validate_pending(
    root: &Path,
    remote: &str,
    head: &str,
    allowed: &impl Fn(&str) -> bool,
) -> CliResult {
    let commits = text(invoke(
        root,
        None,
        &["rev-list", "--reverse", &format!("{remote}..{head}")],
        None,
    )?)?;
    let commits: Vec<_> = commits.lines().collect();
    if commits.len() != 2 {
        bail!(
            "unpublished local commits are not a prepared content publication; review and push repository maintenance separately"
        );
    }
    let snapshot = commits[0];
    for (commit, parent) in [(snapshot, remote), (head, snapshot)] {
        if text(invoke(
            root,
            None,
            &["show", "-s", "--format=%P", commit],
            None,
        )?)? != parent
        {
            bail!("pending publication must be one linear snapshot and pointer pair");
        }
    }
    let pointer: SourcePointer = serde_json::from_slice(&invoke(
        root,
        None,
        &["show", &format!("{head}:current.json")],
        None,
    )?)
    .context("pending local commits are not a prepared content publication")?;
    if pointer.commit.as_str() != snapshot {
        bail!("pending pointer does not select its immediately preceding content snapshot");
    }
    let snapshot_paths = invoke(
        root,
        None,
        &["diff", "--name-only", "-z", remote, snapshot],
        None,
    )?;
    for path in snapshot_paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = std::str::from_utf8(path).context("Git paths must be UTF-8")?;
        if path == "current.json" || !allowed(path) {
            bail!(
                "pending snapshot changes a non-content path: {path}; review and push repository maintenance separately"
            );
        }
    }
    let pointer_paths = invoke(
        root,
        None,
        &["diff", "--name-only", "-z", snapshot, head],
        None,
    )?;
    if pointer_paths != b"current.json\0" {
        bail!("pending pointer commit must change only current.json");
    }
    Ok(())
}
impl Drop for Repository {
    fn drop(&mut self) {
        // Keep the disposable lock inode stable for other processes' file locks.
        let _ = fs::remove_file(&self.index);
        if self.owns_index_lock {
            let _ = fs::remove_file(&self.index_lock);
        }
    }
}
