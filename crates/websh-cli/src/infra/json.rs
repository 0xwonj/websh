use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, bail};

use crate::CliResult;

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> CliResult<T> {
    let body = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&body).with_context(|| format!("parse JSON {}", path.display()))
}

pub(crate) fn json_bytes<T: serde::Serialize>(value: &T) -> CliResult<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value).context("serialize JSON")?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(crate) fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> CliResult {
    write_bytes(path, &json_bytes(value)?)
}

/// Preserve unchanged files and replace changed files only after a complete write.
/// This is atomic for one file; a set of generated files remains reconstructible.
pub(crate) fn write_bytes(path: &Path, bytes: &[u8]) -> CliResult {
    if path.is_symlink() {
        bail!("refusing to replace symlink {}", path.display());
    }
    if path.exists() && fs::read(path).with_context(|| format!("read {}", path.display()))? == bytes
    {
        return Ok(());
    }
    let parent = path.parent().context("output has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut nonce = [0_u8; 12];
    getrandom::fill(&mut nonce).map_err(|error| anyhow::anyhow!("temporary filename: {error}"))?;
    let temporary = parent.join(format!(".websh-{}.tmp", hex::encode(nonce)));
    let result = (|| -> CliResult {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("create {}", temporary.display()))?;
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(bytes)
            .with_context(|| format!("write {}", path.display()))?;
        file.sync_all()
            .with_context(|| format!("flush {}", path.display()))?;
        fs::rename(&temporary, path).with_context(|| format!("replace {}", path.display()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
