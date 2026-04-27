use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Args, Subcommand};
use regex::Regex;

use super::{CliResult, attest};

const DEFAULT_DIST_DIR: &str = "dist";
const DEFAULT_GATEWAY: &str = "https://amethyst-decisive-whitefish-145.mypinata.cloud";
const DEFAULT_ENS_RECORDS_URL: &str = "https://app.ens.domains/wonjae.eth?tab=records";

#[derive(Args)]
pub(crate) struct DeployCommand {
    #[command(subcommand)]
    command: DeploySubcommand,
}

#[derive(Subcommand)]
enum DeploySubcommand {
    /// Build the site and upload dist/ to Pinata IPFS.
    Pinata {
        /// Directory to upload after the release build.
        #[arg(long, default_value = DEFAULT_DIST_DIR)]
        dist_dir: PathBuf,

        /// Pinata upload name. Defaults to websh-<unix timestamp>.
        #[arg(long)]
        name: Option<String>,

        /// Skip the release build and upload the existing dist directory.
        #[arg(long)]
        no_build: bool,

        /// Regenerate attestations without calling local gpg.
        #[arg(long)]
        no_sign: bool,

        /// Gateway base URL printed after upload.
        #[arg(long, default_value = DEFAULT_GATEWAY)]
        gateway: String,

        /// ENS records page printed after upload.
        #[arg(long, default_value = DEFAULT_ENS_RECORDS_URL)]
        ens_url: String,
    },
}

pub(crate) fn run(root: &Path, command: DeployCommand) -> CliResult {
    match command.command {
        DeploySubcommand::Pinata {
            dist_dir,
            name,
            no_build,
            no_sign,
            gateway,
            ens_url,
        } => pinata(root, dist_dir, name, no_build, no_sign, gateway, ens_url),
    }
}

fn pinata(
    root: &Path,
    dist_dir: PathBuf,
    name: Option<String>,
    no_build: bool,
    no_sign: bool,
    gateway: String,
    ens_url: String,
) -> CliResult {
    let dotenv = load_dotenv(root)?;

    println!("Refreshing content manifest and attestations...");
    attest::run_default(root, no_sign)?;

    if !no_build {
        println!("Cleaning previous Trunk build artifacts...");
        run_trunk(root, &["clean"], &dotenv)?;
        println!("Building release bundle...");
        run_trunk(root, &["build", "--release"], &dotenv)?;
    }

    let dist_path = root.join(&dist_dir);
    if !dist_path.is_dir() {
        return Err(format!(
            "upload directory does not exist: {}. Run without --no-build or check --dist-dir.",
            dist_path.display()
        )
        .into());
    }

    let upload_name = name.unwrap_or_else(default_upload_name);
    println!(
        "Uploading {} to Pinata as {upload_name}...",
        dist_dir.display()
    );

    let dist_arg = dist_dir.to_string_lossy().into_owned();
    let output = run_output(
        root,
        "pinata",
        &["upload", &dist_arg, "--name", &upload_name],
        &dotenv,
    )?;
    if !output.stderr.trim().is_empty() {
        eprint!("{}", output.stderr);
    }
    if !output.stdout.trim().is_empty() {
        println!("{}", output.stdout.trim_end());
    }

    let cid = extract_cid(&format!("{}\n{}", output.stdout, output.stderr))
        .ok_or("failed to extract CID from Pinata output")?;
    fs::write(root.join(".last-cid"), format!("{cid}\n"))?;

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

fn run_trunk(root: &Path, args: &[&str], envs: &[(String, String)]) -> CliResult {
    let mut command = Command::new("trunk");
    command
        .args(args)
        .envs(envs.iter().map(|(key, value)| (key, value)))
        .env_remove("NO_COLOR")
        .current_dir(root);

    let status = command
        .status()
        .map_err(|error| format!("failed to run trunk: {error}"))?;

    if !status.success() {
        return Err(format!("trunk exited with status {status}").into());
    }

    Ok(())
}

struct CapturedOutput {
    stdout: String,
    stderr: String,
}

fn run_output(
    root: &Path,
    program: &str,
    args: &[&str],
    envs: &[(String, String)],
) -> CliResult<CapturedOutput> {
    let output = Command::new(program)
        .args(args)
        .envs(envs.iter().map(|(key, value)| (key, value)))
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to run {program}: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!("{program} exited with status {}\n{stderr}", output.status).into());
    }

    Ok(CapturedOutput { stdout, stderr })
}

fn load_dotenv(root: &Path) -> CliResult<Vec<(String, String)>> {
    let path = root.join(".env");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let body = fs::read_to_string(&path)?;
    Ok(parse_dotenv(&body))
}

fn parse_dotenv(body: &str) -> Vec<(String, String)> {
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }

            let line = line.strip_prefix("export ").unwrap_or(line);
            let (key, value) = line.split_once('=')?;
            let key = key.trim();
            if key.is_empty() {
                return None;
            }

            Some((key.to_string(), unquote_env_value(value.trim()).to_string()))
        })
        .collect()
}

fn unquote_env_value(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let first = bytes[0];
        let last = bytes[value.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn default_upload_name() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("websh-{seconds}")
}

fn extract_cid(output: &str) -> Option<String> {
    let pattern = Regex::new(r"bafy[a-zA-Z0-9]+|Qm[a-zA-Z0-9]+").ok()?;
    pattern
        .find(output)
        .map(|matched| matched.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::{extract_cid, parse_dotenv};

    #[test]
    fn extracts_cid_v1() {
        let output = r#"{"cid":"bafybeig7x4exampleaq7vm"}"#;
        assert_eq!(
            extract_cid(output).as_deref(),
            Some("bafybeig7x4exampleaq7vm")
        );
    }

    #[test]
    fn extracts_cid_v0() {
        let output = "IpfsHash: QmYwAPJzv5CZsnAzt8auVTLx7Uu";
        assert_eq!(
            extract_cid(output).as_deref(),
            Some("QmYwAPJzv5CZsnAzt8auVTLx7Uu")
        );
    }

    #[test]
    fn parses_dotenv_values_for_child_processes() {
        let envs = parse_dotenv(
            r#"
            # comment
            PINATA_JWT="secret"
            export PINATA_GROUP=websh
            "#,
        );
        assert_eq!(
            envs,
            vec![
                ("PINATA_JWT".to_string(), "secret".to_string()),
                ("PINATA_GROUP".to_string(), "websh".to_string())
            ]
        );
    }
}
