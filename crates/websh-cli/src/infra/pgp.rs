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

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::{ArmorOptions, DetachedSignature, KeyType, SecretKeyParamsBuilder};
    use pgp::crypto::hash::HashAlgorithm;
    use pgp::types::Password;

    #[test]
    fn verifies_real_detached_signature_and_rejects_changed_message() {
        let root = crate::test_support::temp_dir("pgp-verification");
        let mut rng = rand08::thread_rng();
        let secret = SecretKeyParamsBuilder::default()
            .key_type(KeyType::Ed25519Legacy)
            .can_certify(true)
            .can_sign(true)
            .primary_user_id("Fixture <fixture@example.test>".into())
            .passphrase(None)
            .build()
            .unwrap()
            .generate(&mut rng)
            .unwrap();
        let public = secret.to_public_key();
        let key = root.join("key.asc");
        std::fs::write(
            &key,
            public.to_armored_string(ArmorOptions::default()).unwrap(),
        )
        .unwrap();
        let message = "exact canonical message";
        let signature = DetachedSignature::sign_binary_data(
            &mut rng,
            &secret.primary_key,
            &Password::empty(),
            HashAlgorithm::Sha256,
            message.as_bytes(),
        )
        .unwrap()
        .to_armored_string(ArmorOptions::default())
        .unwrap();
        assert_eq!(
            verify_signature(&key, &signature, message).unwrap(),
            read_key(&key).unwrap().fingerprint
        );
        assert!(verify_signature(&key, &signature, "changed message").is_err());
    }
}
