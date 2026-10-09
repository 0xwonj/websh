//! Public identity and PGP metadata for the deployed site.

use websh_core::crypto::pgp::normalize_fingerprint;

pub const APP_NAME: &str = "wonjae.eth";
pub const APP_TAGLINE: &str = "Zero-Knowledge Proofs | Compiler Design | Ethereum";

pub const PUBLIC_KEY_PATH: &str = "assets/crypto/site.asc";
pub const EXPECTED_PGP_FINGERPRINT: &str = "6CA8E0E8E0F9B9EE2F92EE49BEE7501AEA7758AD";

pub const PUBLIC_KEY_BLOCK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/crypto/site.asc"
));

pub fn fingerprint_matches(raw: &str) -> bool {
    normalize_fingerprint(raw) == EXPECTED_PGP_FINGERPRINT
}

pub fn pgp_policy() -> websh_core::crypto::pgp::PgpPolicy<'static> {
    websh_core::crypto::pgp::PgpPolicy {
        public_key: PUBLIC_KEY_BLOCK,
        primary_fingerprint: EXPECTED_PGP_FINGERPRINT,
        signer_fingerprints: &[EXPECTED_PGP_FINGERPRINT],
    }
}

pub fn content_trust() -> websh_core::publication::TrustPolicy<'static> {
    let policy = pgp_policy();
    websh_core::publication::TrustPolicy {
        site: APP_NAME,
        public_key: policy.public_key,
        primary_fingerprint: policy.primary_fingerprint,
        signer_fingerprints: policy.signer_fingerprints,
    }
}
