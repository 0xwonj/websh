use std::path::{Path, PathBuf};

use crate::CliResult;
use anyhow::{Context, bail};

pub(crate) struct Project {
    root: PathBuf,
}

impl Project {
    pub(crate) fn open(root: &Path) -> CliResult<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("open project {}", root.display()))?;
        if !root.is_dir() {
            bail!("project is not a directory: {}", root.display());
        }
        Ok(Self { root })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}
