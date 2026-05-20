use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use anyhow::Context;

use crate::CliResult;
use crate::infra::git::git_output;

#[derive(Debug)]
pub(crate) struct GitModifiedAt {
    root: PathBuf,
    root_abs: PathBuf,
    cache: BTreeMap<String, Option<u64>>,
}

impl GitModifiedAt {
    pub(crate) fn new(root: &Path) -> CliResult<Self> {
        let root_abs = root
            .canonicalize()
            .with_context(|| format!("canonicalize repository root {}", root.display()))?;
        Ok(Self {
            root: root.to_path_buf(),
            root_abs,
            cache: BTreeMap::new(),
        })
    }

    pub(crate) fn timestamp_for_path(&mut self, path: &Path) -> CliResult<Option<u64>> {
        let Some(rel_path) = self.repo_relative_path(path)? else {
            return Ok(None);
        };
        if let Some(value) = self.cache.get(&rel_path).copied() {
            return Ok(value);
        }

        let value = self.query_git(&rel_path)?;
        self.cache.insert(rel_path, value);
        Ok(value)
    }

    fn repo_relative_path(&self, path: &Path) -> CliResult<Option<String>> {
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        };
        let resolved = resolved
            .canonicalize()
            .with_context(|| format!("canonicalize content path {}", path.display()))?;
        let Ok(rel) = resolved.strip_prefix(&self.root_abs) else {
            return Ok(None);
        };
        let rel_path = slash_path(rel);
        if rel_path.is_empty() {
            return Ok(None);
        }
        Ok(Some(rel_path))
    }

    fn query_git(&self, rel_path: &str) -> CliResult<Option<u64>> {
        let args = vec![
            OsString::from("log"),
            OsString::from("-1"),
            OsString::from("--format=%ct"),
            OsString::from("--"),
            OsString::from(rel_path),
        ];
        let output = match git_output(&self.root, args) {
            Ok(output) => output,
            Err(_) => return Ok(None),
        };
        if !output.success {
            return Ok(None);
        }

        let timestamp = output.stdout.lines().next().unwrap_or_default().trim();
        if timestamp.is_empty() {
            return Ok(None);
        }
        timestamp
            .parse::<u64>()
            .with_context(|| format!("parse git modified timestamp for {rel_path}"))
            .map(Some)
    }
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            Component::CurDir => None,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                Some(component.as_os_str().to_string_lossy().to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}
