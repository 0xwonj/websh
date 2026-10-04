use std::path::Path;

use anyhow::bail;
use pgp::composed::{Deserializable, SignedPublicKey};
use pgp::types::KeyDetails;
use websh_core::crypto::pgp::normalize_fingerprint;

use crate::CliResult;

pub(crate) struct PublicKey {
    pub(crate) fingerprint: String,
    pub(crate) user_ids: Vec<String>,
}

pub(crate) fn read_key(path: &Path) -> CliResult<PublicKey> {
    let (key, _) = SignedPublicKey::from_armor_file(path)?;
    key.verify_bindings()?;
    Ok(PublicKey {
        fingerprint: normalize_fingerprint(&key.fingerprint().to_string()),
        user_ids: key
            .details
            .users
            .iter()
            .map(|user| String::from_utf8_lossy(user.id.id()).trim().to_string())
            .collect(),
    })
}

pub(crate) fn verify_signature(
    key_path: &Path,
    signature: &str,
    message: &str,
) -> CliResult<String> {
    use pgp::composed::DetachedSignature;

    let (key, _headers) = SignedPublicKey::from_armor_file(key_path)?;
    key.verify_bindings()?;
    let (signature, _headers) = DetachedSignature::from_armor_single(signature.as_bytes())?;

    if signature.verify(&key, message.as_bytes()).is_ok()
        || key
            .public_subkeys
            .iter()
            .any(|subkey| signature.verify(subkey, message.as_bytes()).is_ok())
    {
        return Ok(normalize_fingerprint(&key.fingerprint().to_string()));
    }

    bail!("PGP detached signature did not verify with the supplied key")
}
