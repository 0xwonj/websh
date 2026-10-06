//! The crypto acceptance gate runs the shared verifier inside a real browser.
use wasm_bindgen_test::*;
use websh_core::crypto::pgp::{PgpPolicy, verify_detached};

const PUBLIC_KEY: &str = include_str!("../../../tests/fixtures/pgp/public.asc");
const MESSAGE: &[u8] = include_bytes!("../../../tests/fixtures/pgp/message.txt");
const SIGNATURE: &[u8] = include_bytes!("../../../tests/fixtures/pgp/signature.asc");
const FINGERPRINT: &str = "2BFA539F531108FFB06B99F577BB8CAE0972250C";
const SIGNED_AT: u64 = 1_791_277_096;

#[wasm_bindgen_test]
fn browser_verifies_real_pgp_and_rejects_tampering() {
    let policy = PgpPolicy {
        public_key: PUBLIC_KEY,
        primary_fingerprint: FINGERPRINT,
        signer_fingerprints: &[FINGERPRINT],
    };
    let start = js_sys::Date::now();
    let evidence = verify_detached(MESSAGE, SIGNATURE, &policy, SIGNED_AT).unwrap();
    let elapsed = js_sys::Date::now() - start;
    console_log!(
        "Real browser PGP verification: {elapsed} ms (debug WASM, fixture certificate/signature)"
    );
    assert_eq!(evidence.primary(), FINGERPRINT);
    assert_eq!(evidence.signer(), FINGERPRINT);
    assert!(verify_detached(b"tampered", SIGNATURE, &policy, SIGNED_AT).is_err());
    assert!(
        verify_detached(
            MESSAGE,
            SIGNATURE,
            &PgpPolicy {
                primary_fingerprint: "0000000000000000000000000000000000000000",
                ..policy
            },
            SIGNED_AT
        )
        .is_err()
    );
    assert!(
        verify_detached(
            MESSAGE,
            SIGNATURE,
            &PgpPolicy {
                signer_fingerprints: &[],
                ..policy
            },
            SIGNED_AT
        )
        .is_err()
    );
}
