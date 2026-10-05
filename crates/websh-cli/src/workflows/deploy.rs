use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use cid::Cid;

use crate::CliResult;
use crate::infra::{dotenv, json::write_bytes, pinata};
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

fn publish(root: &Path) -> CliResult<Deployment> {
    let envs = dotenv::load(root)?;
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let cid = pinata::upload(root, &format!("websh-{seconds}"), &envs)?;
    // Publication cannot be rolled back by a failed local receipt write. Return
    // its CID and a warning so callers do not repeat a successful upload.
    let receipt_warning = write_bytes(&root.join(".last-cid"), format!("{cid}\n").as_bytes())
        .err()
        .map(|error| format!("upload succeeded, but .last-cid could not be saved: {error:#}"));
    Ok(Deployment {
        cid,
        receipt_warning,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use cid::Cid;

    use super::publish;
    use crate::test_support::temp_dir;

    #[test]
    fn publication_records_only_valid_cids_and_reports_receipt_failure_after_success() {
        let root = temp_dir("publish");
        fs::create_dir(root.join("dist")).unwrap();
        fs::write(root.join("dist/index.html"), "prebuilt bundle").unwrap();
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

        let result = publish(&root).unwrap();
        assert_eq!(result.cid, cid);
        assert!(result.receipt_warning.is_none());
        assert_eq!(
            fs::read_to_string(root.join(".last-cid")).unwrap(),
            format!("{cid}\n")
        );

        fs::write(
            root.join("response.json"),
            r#"{"cid":"bafybroken","network":"public"}"#,
        )
        .unwrap();
        assert!(
            publish(&root)
                .unwrap_err()
                .to_string()
                .contains("inspect the remote upload before retrying")
        );
        assert_eq!(
            fs::read_to_string(root.join(".last-cid")).unwrap(),
            format!("{cid}\n")
        );

        fs::write(root.join("response.json"), response).unwrap();
        fs::remove_file(root.join(".last-cid")).unwrap();
        fs::create_dir(root.join(".last-cid")).unwrap();
        let result = publish(&root).unwrap();
        assert_eq!(result.cid, cid);
        assert!(result.receipt_warning.unwrap().contains("upload succeeded"));
        assert_eq!(
            fs::read_to_string(root.join("calls")).unwrap(),
            "upload\nupload\nupload\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("dist/index.html")).unwrap(),
            "prebuilt bundle"
        );
    }
}
