use std::path::Path;

use websh_core::crypto::ack::{AckArtifact, short_hash};
use websh_site::{ACK_ARTIFACT_PATH, IDENTITY_PATH};

use crate::CliResult;
use crate::infra::json::read_json;

use super::pgp;

pub(super) fn verify_all(root: &Path) -> CliResult {
    let artifact = read_json::<AckArtifact>(&root.join(ACK_ARTIFACT_PATH))?;
    artifact.validate()?;
    println!("ack: ok {}", short_hash(&artifact.combined_root));

    let identity_path = root.join(IDENTITY_PATH);
    if identity_path.exists() {
        pgp::verify_identity(root)?;
        println!("pgp: ok");
    } else {
        println!("pgp: skipped ({IDENTITY_PATH} missing)");
    }
    Ok(())
}
