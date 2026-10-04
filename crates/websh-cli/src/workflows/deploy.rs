use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};

use crate::CliResult;

use crate::infra::{dotenv, pinata, trunk};

pub(crate) struct DeployOptions {
    pub(crate) dist_dir: PathBuf,
    pub(crate) name: Option<String>,
    pub(crate) no_build: bool,
    pub(crate) no_sign: bool,
    pub(crate) gateway: String,
    pub(crate) ens_url: String,
}

pub(crate) fn deploy(root: &Path, options: DeployOptions) -> CliResult {
    let DeployOptions {
        dist_dir,
        name,
        no_build,
        no_sign,
        gateway,
        ens_url,
    } = options;
    validate_dist(root, &dist_dir)?;
    let mut envs = dotenv::load(root)?;

    if no_sign {
        envs.push(("WEBSH_NO_SIGN".to_string(), "1".to_string()));
    }

    if !no_build {
        println!("Cleaning previous Trunk build artifacts...");
        trunk::clean(root, &dist_dir, &envs)?;
        println!("Building release bundle...");
        trunk::release(root, &dist_dir, &envs)?;
    } else {
        println!("Skipping build (--no-build); uploading the existing bundle as-is.");
    }

    let dist_path = root.join(&dist_dir);
    if !dist_path.is_dir() {
        bail!(
            "upload directory does not exist: {}. Run without --no-build or check --dist-dir.",
            dist_path.display()
        );
    }

    let upload_name = name.unwrap_or_else(default_upload_name);
    println!(
        "Uploading {} to Pinata as {upload_name}...",
        dist_dir.display()
    );

    let cid = pinata::upload(root, &dist_dir, &upload_name, &envs)?;
    fs::write(root.join(".last-cid"), format!("{cid}\n")).context("write .last-cid")?;

    let gateway = gateway.trim_end_matches('/');

    println!();
    println!("CID: {cid}");
    println!("Gateway: {gateway}/ipfs/{cid}");
    println!();
    println!("Update ENS contenthash:");
    println!("  ipfs://{cid}");
    println!();
    println!("{ens_url}");

    Ok(())
}

fn validate_dist(root: &Path, directory: &Path) -> CliResult {
    let mut components = directory.components();
    let name = match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) => name.to_str().unwrap_or_default(),
        _ => "",
    };
    let valid = name == "dist"
        || name.strip_prefix("dist-").is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        });
    if !valid {
        bail!(
            "--dist-dir must be dist or a root-level dist-<name> directory (letters, digits, - or _)"
        );
    }
    let path = root.join(directory);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => bail!(
            "deployment output must be a directory, not a file or symlink: {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("inspect deployment output {}", path.display()))
        }
    }
}

fn default_upload_name() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("websh-{seconds}")
}
