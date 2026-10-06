use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use cid::Cid;
use serde::{Deserialize, Serialize};

use crate::CliResult;
use crate::infra::{
    bundle, dotenv,
    json::{read_json, write_json},
    pinata,
};
use crate::workflows::check::check_bundle;

#[derive(Debug)]
pub(crate) struct Deployment {
    pub(crate) cid: Cid,
    pub(crate) receipt_warning: Option<String>,
}

pub(crate) fn deploy(root: &Path) -> CliResult<Deployment> {
    check_bundle(root)?;
    publish(root)
}

const RECEIPT_PATH: &str = ".websh/local/deploy/release.json";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    cid: String,
    source_commit: String,
    source_dirty: bool,
    files: Vec<bundle::FileDigest>,
    previous_cid: Option<String>,
    bundle_unchanged: bool,
}

fn publish(root: &Path) -> CliResult<Deployment> {
    let files = bundle::inventory(root)?;
    let (source_commit, source_dirty) = bundle::source(root)?;
    let previous_cid = if root.join(RECEIPT_PATH).is_file() {
        let receipt: Receipt = read_json(&root.join(RECEIPT_PATH))?;
        Some(receipt.cid.parse::<Cid>()?.to_string())
    } else {
        None
    };
    let envs = dotenv::load(root)?;
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let cid = pinata::upload(root, &format!("websh-{seconds}"), &envs)?;
    let bundle_unchanged = bundle::inventory(root).is_ok_and(|current| current == files);
    let receipt = Receipt {
        cid: cid.to_string(),
        source_commit,
        source_dirty,
        files,
        previous_cid,
        bundle_unchanged,
    };
    // Publication cannot be rolled back by a failed local receipt write. Return
    // its CID and a warning so callers do not repeat a successful upload.
    let mut warnings = Vec::new();
    if !bundle_unchanged {
        warnings.push("upload succeeded, but the local bundle changed during upload; verify the remote files before using this CID".to_owned());
    }
    if let Err(error) = write_json(&root.join(RECEIPT_PATH), &receipt) {
        warnings.push(format!(
            "upload succeeded, but {RECEIPT_PATH} could not be saved: {error:#}"
        ));
    }
    Ok(Deployment {
        cid,
        receipt_warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use cid::Cid;

    use super::{RECEIPT_PATH, Receipt, deploy};
    use crate::test_support::temp_dir;

    #[test]
    fn publication_records_only_valid_cids_and_reports_receipt_failure_after_success() {
        let root = temp_dir("publish");
        for args in [
            vec!["init", "--initial-branch=main"],
            vec!["config", "user.name", "Fixture"],
            vec!["config", "user.email", "fixture@example.test"],
            vec![
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "-m",
                "Initial",
            ],
        ] {
            assert!(
                std::process::Command::new("git")
                    .current_dir(&root)
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        }
        fs::create_dir_all(root.join("dist/assets/crypto")).unwrap();
        const INDEX: &str = "<html><script src='app.js'></script></html>";
        fs::write(root.join("dist/index.html"), INDEX).unwrap();
        fs::write(root.join("dist/app.js"), "void 0;").unwrap();
        fs::write(root.join("dist/app.wasm"), b"\0asm\x01\0\0\0\0").unwrap();
        fs::write(
            root.join("dist").join(websh_site::PUBLIC_KEY_PATH),
            websh_site::PUBLIC_KEY_BLOCK,
        )
        .unwrap();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        let executable = bin.join("pinata");
        fs::write(
            &executable,
            r#"#!/bin/sh
set -eu
test "$#" = 6
test "$1" = upload
test "$2" = --network
test "$3" = public
test "$4" = --name
case "$5" in websh-*) ;; *) exit 1 ;; esac
test "$6" = dist
test -f dist/index.html
test "$PINATA_JWT" = test-only
printf 'upload\n' >> calls
/bin/cat response.json
"#,
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            root.join(".env"),
            format!("PATH={}\nPINATA_JWT=test-only\n", bin.display()),
        )
        .unwrap();
        let cid = Cid::new_v1(
            0x70,
            cid::multihash::Multihash::wrap(0x12, &[7; 32]).unwrap(),
        );
        let response = format!(r#"{{"cid":"{cid}","network":"public"}}"#);
        fs::write(root.join("response.json"), &response).unwrap();

        let result = deploy(&root).unwrap();
        assert_eq!(result.cid, cid);
        assert!(result.receipt_warning.is_none());
        let saved: Receipt = crate::infra::json::read_json(&root.join(RECEIPT_PATH)).unwrap();
        assert_eq!(saved.cid, cid.to_string());
        assert!(saved.files.iter().any(|file| file.path == "index.html"));
        assert!(saved.bundle_unchanged);
        assert!(saved.previous_cid.is_none());

        fs::write(
            root.join("response.json"),
            r#"{"cid":"bafybroken","network":"public"}"#,
        )
        .unwrap();
        assert!(
            deploy(&root)
                .unwrap_err()
                .to_string()
                .contains("inspect the remote upload before retrying")
        );
        let saved: Receipt = crate::infra::json::read_json(&root.join(RECEIPT_PATH)).unwrap();
        assert_eq!(saved.cid, cid.to_string());
        assert!(saved.files.iter().any(|file| file.path == "index.html"));
        assert!(saved.bundle_unchanged);
        assert!(saved.previous_cid.is_none());

        fs::write(root.join("response.json"), response).unwrap();
        fs::remove_file(root.join(RECEIPT_PATH)).unwrap();
        fs::create_dir(root.join(RECEIPT_PATH)).unwrap();
        let result = deploy(&root).unwrap();
        assert_eq!(result.cid, cid);
        assert!(result.receipt_warning.unwrap().contains("upload succeeded"));
        assert_eq!(
            fs::read_to_string(root.join("calls")).unwrap(),
            "upload\nupload\nupload\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("dist/index.html")).unwrap(),
            INDEX
        );
    }
}
