//! Public identity and PGP metadata for the deployed site.

use websh_core::crypto::pgp::normalize_fingerprint;

pub const APP_NAME: &str = "wonjae.eth";
pub const APP_TAGLINE: &str = "Zero-Knowledge Proofs | Compiler Design | Ethereum";

pub const IDENTITY_PATH: &str = "assets/crypto/identity.json";
pub const PUBLIC_KEY_PATH: &str = "content/keys/wonjae.asc";
pub const EXPECTED_PGP_FINGERPRINT: &str = "6CA8E0E8E0F9B9EE2F92EE49BEE7501AEA7758AD";

pub const PUBLIC_KEY_BLOCK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../content/keys/wonjae.asc"
));

pub fn fingerprint_matches(raw: &str) -> bool {
    normalize_fingerprint(raw) == EXPECTED_PGP_FINGERPRINT
}
