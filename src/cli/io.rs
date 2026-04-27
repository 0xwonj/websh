use std::fs;
use std::path::Path;

use super::CliResult;

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> CliResult<T> {
    let body = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&body)?)
}

pub(crate) fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> CliResult {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let body = format!("{}\n", serde_json::to_string_pretty(value)?);
    fs::write(path, body)?;
    Ok(())
}
