use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::CliResult;
use crate::workflows::content::ContentSnapshot;

use super::draft::Draft;
use super::path::EntryPath;

pub(crate) fn import(root: &Path, source: &Path, destination: Option<&str>) -> CliResult<PathBuf> {
    let inferred;
    let destination = match destination {
        Some(path) => path,
        None => {
            let category = source
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .context("source has no category directory; specify --to <category>/<slug>.md")?;
            let name = source
                .file_name()
                .and_then(|name| name.to_str())
                .context("source filename must be UTF-8")?;
            inferred = format!("{category}/{name}");
            &inferred
        }
    };
    let entry = EntryPath::parse(destination)?;
    if !fs::symlink_metadata(source)
        .with_context(|| format!("inspect {}", source.display()))?
        .is_file()
    {
        bail!("draft source must be a regular file: {}", source.display());
    }
    let body = fs::read_to_string(source).with_context(|| format!("read {}", source.display()))?;
    let canonical = Draft::parse(&body)
        .with_context(|| format!("validate {}", source.display()))?
        .canonical()?;
    ContentSnapshot::load_with_file(root, entry.as_str(), canonical.as_bytes())
        .context("validate imported content")?;
    let target = root.join("content").join(entry.as_str());
    let parent = target.parent().expect("entry has a category");

    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .with_context(|| {
            format!(
                "create {} without overwriting existing content",
                target.display()
            )
        })?;
    if let Err(error) = file.write_all(canonical.as_bytes()) {
        drop(file);
        fs::remove_file(&target).with_context(|| {
            format!(
                "remove incomplete import {} after {error}",
                target.display()
            )
        })?;
        return Err(error).with_context(|| format!("write {}", target.display()));
    }
    Ok(target)
}
