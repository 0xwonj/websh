//! OpenPGP homepage key metadata.
//!
//! Runtime code intentionally keeps this light. Full OpenPGP parsing is covered
//! by a dev-dependency test so rPGP does not become part of the WASM bundle.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityArtifact {
    pub version: u32,
    pub pgp: PgpIdentity,
    pub ethereum: EthereumIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PgpIdentity {
    pub key_path: String,
    pub fingerprint: String,
    pub user_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumIdentity {
    pub ens: String,
    pub address: String,
}

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
