use std::path::Path;

use websh_core::crypto::pgp::normalize_fingerprint;
use websh_site::{EXPECTED_PGP_FINGERPRINT, PUBLIC_KEY_PATH};

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
}

#[test]
fn pinned_pgp_key_matches_deployed_identity() {
    let path = workspace_root().join(PUBLIC_KEY_PATH);
    use pgp::composed::{Deserializable, SignedPublicKey};
    use pgp::types::KeyDetails;

    let (key, _headers) = SignedPublicKey::from_armor_file(&path).expect("parse OpenPGP key");
    assert_eq!(
        normalize_fingerprint(&key.fingerprint().to_string()),
        EXPECTED_PGP_FINGERPRINT
    );
}
