use std::path::Path;

use anyhow::Context;
use regex::Regex;

use super::process::run_output;
use crate::CliResult;

pub(crate) fn upload(
    root: &Path,
    directory: &Path,
    name: &str,
    envs: &[(String, String)],
) -> CliResult<String> {
    let directory = directory.to_string_lossy();
    let output = run_output(
        root,
        "pinata",
        &["upload", &directory, "--name", name],
        envs,
    )?;
    if !output.stderr.trim().is_empty() {
        eprint!("{}", output.stderr);
    }
    if !output.stdout.trim().is_empty() {
        println!("{}", output.stdout.trim_end());
    }
    extract_cid(&format!("{}\n{}", output.stdout, output.stderr))
        .context("failed to extract CID from Pinata output")
}

fn extract_cid(output: &str) -> Option<String> {
    let pattern = Regex::new(r"bafy[a-zA-Z0-9]+|Qm[a-zA-Z0-9]+").ok()?;
    pattern
        .find(output)
        .map(|matched| matched.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::extract_cid;

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
}
