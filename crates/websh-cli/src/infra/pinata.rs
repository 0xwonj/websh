use std::path::Path;

use anyhow::{Context, bail};
use cid::Cid;
use serde::Deserialize;

use super::process::run_output;
use crate::CliResult;

pub(crate) fn upload(root: &Path, name: &str, envs: &[(String, String)]) -> CliResult<Cid> {
    // The official CLI prints one JSON object when progress output is disabled.
    // Set the network explicitly rather than inheriting the user's CLI setting.
    let output = run_output(
        root,
        "pinata",
        &["upload", "--network", "public", "--name", name, "dist"],
        envs,
    )?;
    parse_upload(&output.stdout).context("Pinata completed, but its upload receipt is invalid; inspect the remote upload before retrying")
}

#[derive(Deserialize)]
struct UploadResponse {
    cid: String,
    network: String,
}

fn parse_upload(output: &str) -> CliResult<Cid> {
    let response: UploadResponse =
        serde_json::from_str(output).context("parse Pinata upload JSON")?;
    if response.network != "public" {
        bail!("Pinata upload is not on the public network");
    }
    Cid::try_from(response.cid.as_str()).context("parse Pinata content identifier")
}

#[cfg(test)]
mod tests {
    use super::parse_upload;
    use cid::Cid;

    #[test]
    fn accepts_only_a_structured_public_upload_with_a_valid_cid() {
        let cid = Cid::new_v1(
            0x70,
            cid::multihash::Multihash::wrap(0x12, &[7; 32]).unwrap(),
        );
        assert_eq!(
            parse_upload(&format!(
                r#"{{"cid":"{cid}","network":"public","name":"site"}}"#
            ))
            .unwrap(),
            cid
        );
        for output in [
            format!(r#"{{"cid":"{cid}","network":"private"}}"#),
            format!(r#"{{"cid":"{cid}"}}"#),
            format!("uploaded {cid}"),
            r#"{"cid":"bafyfixture","network":"public"}"#.to_string(),
        ] {
            assert!(parse_upload(&output).is_err(), "accepted {output}");
        }
    }
}
