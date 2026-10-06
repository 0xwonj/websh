//! Local app build inventory. Remote gateway checks remain separate evidence.
use crate::{CliResult, infra::process::run_output};
use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use websh_core::publication::ReleaseId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileDigest {
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) bytes: u64,
}

pub(crate) fn inventory(root: &Path) -> CliResult<Vec<FileDigest>> {
    fn walk(base: &Path, relative: &Path, files: &mut Vec<FileDigest>) -> CliResult {
        for entry in fs::read_dir(base.join(relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                bail!("app bundle contains a symlink: {}", path.display());
            }
            if kind.is_dir() {
                walk(base, &path, files)?;
            } else if kind.is_file() {
                let bytes = fs::read(entry.path())?;
                files.push(FileDigest {
                    path: path.to_str().context("app paths must be UTF-8")?.to_owned(),
                    sha256: ReleaseId::of(&bytes).as_str().to_owned(),
                    bytes: bytes.len() as u64,
                });
            } else {
                bail!("unsupported app file: {}", path.display());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(&root.join("dist"), Path::new(""), &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

pub(crate) fn source(root: &Path) -> CliResult<(String, bool)> {
    let commit = run_output(root, "git", &["rev-parse", "HEAD"], &[])?
        .stdout
        .trim()
        .to_owned();
    let dirty = !run_output(
        root,
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
        &[],
    )?
    .stdout
    .trim()
    .is_empty();
    Ok((commit, dirty))
}
