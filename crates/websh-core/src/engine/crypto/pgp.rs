//! OpenPGP trust policy, byte verification, and display metadata shared by native and WASM.

pub fn normalize_fingerprint(raw: &str) -> String {
    raw.chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .flat_map(char::to_uppercase)
        .collect()
}

pub fn pretty_fingerprint(raw: &str) -> String {
    normalize_fingerprint(raw)
        .as_bytes()
        .chunks(4)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn fingerprint_matches(raw: &str, expected: &str) -> bool {
    normalize_fingerprint(raw) == normalize_fingerprint(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED: &str = "6CA8E0E8E0F9B9EE2F92EE49BEE7501AEA7758AD";

    #[test]
    fn fingerprint_normalization_is_stable() {
        assert_eq!(
            normalize_fingerprint("6CA8 E0E8 E0F9 B9EE 2F92  EE49 BEE7 501A EA77 58AD"),
            EXPECTED
        );
    }

    #[test]
    fn fingerprint_match_uses_supplied_expected_value() {
        assert!(fingerprint_matches(
            "6CA8 E0E8 E0F9 B9EE 2F92 EE49 BEE7 501A EA77 58AD",
            EXPECTED
        ));
    }
}

/// App-owned trust inputs. Downloaded content cannot select or widen these keys.
#[derive(Clone, Copy, Debug)]
pub struct PgpPolicy<'a> {
    pub public_key: &'a str,
    pub primary_fingerprint: &'a str,
    pub signer_fingerprints: &'a [&'a str],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureEvidence {
    primary: String,
    signer: String,
    created_at: u64,
}

impl SignatureEvidence {
    pub fn primary(&self) -> &str {
        &self.primary
    }
    pub fn signer(&self) -> &str {
        &self.signer
    }
    pub fn created_at(&self) -> u64 {
        self.created_at
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("OpenPGP parse or binding error: {0}")]
    OpenPgp(String),
    #[error("OpenPGP policy rejected the signature: {0}")]
    Policy(&'static str),
    #[error("detached signature does not verify under an authorized signer")]
    BadSignature,
}

/// Verify an exact-byte detached signature using the app's pinned certificate.
///
/// Policy supports Ed25519 signing keys and SHA-2 only. All certificate bindings
/// are checked. Revoked primary/signing keys are rejected; key/binding expiry is
/// evaluated at signing time so old publications remain readable after key expiry.
/// An explicitly expiring data signature is honored at the supplied current time.
/// Signing subkeys require an authenticated binding and embedded back-signature.
pub fn verify_detached(
    message: &[u8],
    signature: &[u8],
    policy: &PgpPolicy<'_>,
    now: u64,
) -> Result<SignatureEvidence, VerificationError> {
    use pgp::{
        composed::{Deserializable, DetachedSignature, SignedPublicKey},
        packet::SignatureType,
        types::KeyDetails,
    };
    if policy.public_key.len() > 64 * 1024 || signature.len() > 16 * 1024 {
        return Err(VerificationError::Policy(
            "certificate or signature exceeds size limit",
        ));
    }
    let (key, _) =
        SignedPublicKey::from_armor_single(policy.public_key.as_bytes()).map_err(pgp_error)?;
    key.verify_bindings().map_err(pgp_error)?;
    let primary = normalize_fingerprint(&key.fingerprint().to_string());
    if primary != policy.primary_fingerprint || !allowed_key(&key) {
        return Err(VerificationError::Policy(
            "untrusted primary key or algorithm",
        ));
    }
    if !key.details.revocation_signatures.is_empty()
        || key
            .details
            .users
            .iter()
            .flat_map(|user| &user.signatures)
            .any(|binding| binding.typ() == Some(SignatureType::CertRevocation))
    {
        return Err(VerificationError::Policy(
            "primary key or certified identity is revoked",
        ));
    }
    let (signature, _) = DetachedSignature::from_armor_single(signature).map_err(pgp_error)?;
    if signature.signature.typ() != Some(SignatureType::Binary)
        || !allowed_hash(&signature.signature)
    {
        return Err(VerificationError::Policy("requires binary SHA-2 signature"));
    }
    let signed_at = signature
        .signature
        .created()
        .ok_or(VerificationError::Policy("missing signature time"))?
        .as_secs() as u64;
    if signed_at > now.saturating_add(300) || !signature_active(&signature.signature, now) {
        return Err(VerificationError::Policy(
            "signature is future-dated or expired",
        ));
    }
    let primary_binding = key
        .details
        .direct_signatures
        .iter()
        .chain(key.details.users.iter().flat_map(|user| &user.signatures))
        .filter(|binding| binding.is_certification() || binding.typ() == Some(SignatureType::Key))
        .filter(|binding| {
            binding
                .created()
                .is_some_and(|created| created.as_secs() as u64 <= signed_at)
        })
        .max_by_key(|binding| binding.created());
    let binding = primary_binding.ok_or(VerificationError::Policy(
        "missing current primary self-certification",
    ))?;
    if !binding_active(binding, key.created_at().as_secs() as u64, signed_at) {
        return Err(VerificationError::Policy(
            "primary key or binding expired at signing time",
        ));
    }
    if policy.signer_fingerprints.contains(&primary.as_str())
        && binding.key_flags().sign()
        && signature.verify(&key, message).is_ok()
    {
        return Ok(SignatureEvidence {
            primary: primary.clone(),
            signer: primary,
            created_at: signed_at,
        });
    }
    for subkey in &key.public_subkeys {
        let signer = normalize_fingerprint(&subkey.fingerprint().to_string());
        if !policy.signer_fingerprints.contains(&signer.as_str()) || !allowed_key(subkey) {
            continue;
        }
        if subkey
            .signatures
            .iter()
            .any(|binding| binding.typ() == Some(SignatureType::SubkeyRevocation))
        {
            return Err(VerificationError::Policy(
                "authorized signing subkey is revoked",
            ));
        }
        let binding = subkey
            .signatures
            .iter()
            .filter(|binding| binding.typ() == Some(SignatureType::SubkeyBinding))
            .filter(|binding| {
                binding
                    .created()
                    .is_some_and(|created| created.as_secs() as u64 <= signed_at)
            })
            .max_by_key(|binding| binding.created());
        if let Some(binding) = binding
            && binding.key_flags().sign()
            && binding.embedded_signature().is_some_and(|backsig| {
                allowed_hash(backsig)
                    && signature_active(backsig, signed_at)
                    && backsig
                        .created()
                        .is_some_and(|created| created.as_secs() as u64 <= signed_at)
            })
            && binding_active(binding, subkey.created_at().as_secs() as u64, signed_at)
            && signature.verify(subkey, message).is_ok()
        {
            return Ok(SignatureEvidence {
                primary,
                signer,
                created_at: signed_at,
            });
        }
    }
    Err(VerificationError::BadSignature)
}

fn pgp_error(error: impl std::fmt::Display) -> VerificationError {
    VerificationError::OpenPgp(error.to_string())
}

fn allowed_key(key: &impl pgp::types::KeyDetails) -> bool {
    use pgp::crypto::public_key::PublicKeyAlgorithm;
    matches!(
        key.algorithm(),
        PublicKeyAlgorithm::EdDSALegacy | PublicKeyAlgorithm::Ed25519
    )
}

fn allowed_hash(signature: &pgp::packet::Signature) -> bool {
    use pgp::crypto::hash::HashAlgorithm;
    matches!(
        signature.hash_alg(),
        Some(HashAlgorithm::Sha256 | HashAlgorithm::Sha384 | HashAlgorithm::Sha512)
    )
}

fn signature_active(signature: &pgp::packet::Signature, at: u64) -> bool {
    let Some(created) = signature.created().map(|created| created.as_secs() as u64) else {
        return false;
    };
    // The caller permits small future clock skew for data signatures only.
    signature
        .signature_expiration_time()
        .is_none_or(|duration| {
            duration.as_secs() == 0 || at < created.saturating_add(duration.as_secs() as u64)
        })
}

fn binding_active(binding: &pgp::packet::Signature, key_created: u64, signed_at: u64) -> bool {
    key_created <= signed_at
        && allowed_hash(binding)
        && signature_active(binding, signed_at)
        && binding.key_expiration_time().is_none_or(|duration| {
            duration.as_secs() == 0
                || signed_at < key_created.saturating_add(duration.as_secs() as u64)
        })
}
