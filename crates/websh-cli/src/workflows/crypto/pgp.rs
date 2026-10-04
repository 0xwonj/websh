use std::path::{Path, PathBuf};

use anyhow::bail;

use websh_core::crypto::pgp::{
    EthereumIdentity, IdentityArtifact, PgpIdentity, normalize_fingerprint,
};
use websh_site::IDENTITY_PATH;

use crate::CliResult;
use crate::infra::json::{read_json, write_json};
use crate::infra::pgp;

pub(crate) fn verify_identity(root: &Path) -> CliResult {
    let identity = read_json::<IdentityArtifact>(&root.join(IDENTITY_PATH))?;
    let parsed = pgp::read_key(&root.join(&identity.pgp.key_path))?;
    let expected = normalize_fingerprint(&identity.pgp.fingerprint);
    if parsed.fingerprint != expected {
        bail!(
            "PGP fingerprint mismatch: expected {}, got {}",
            expected,
            parsed.fingerprint
        );
    }
    Ok(())
}

pub(crate) fn import(root: &Path, key: PathBuf, ens: String, address: String) -> CliResult {
    let key_path = root.join(&key);
    let parsed = pgp::read_key(&key_path)?;
    let identity = IdentityArtifact {
        version: 1,
        pgp: PgpIdentity {
            key_path: key.to_string_lossy().to_string(),
            fingerprint: parsed.fingerprint,
            user_ids: parsed.user_ids,
        },
        ethereum: EthereumIdentity { ens, address },
    };
    let path = root.join(IDENTITY_PATH);
    write_json(&path, &identity)?;
    println!("wrote {}", path.display());
    Ok(())
}
