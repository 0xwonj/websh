//! Deployment-specific site configuration and bundled site assets.
//!
//! `websh-core` owns reusable domain and engine behavior. This crate owns the
//! concrete deployed site's identity, policy, shell copy, and public artifact
//! locations.

pub mod artifacts;
pub mod bootstrap;
pub mod identity;
pub mod policy;
pub mod profile;

pub use artifacts::{
    ACK_ARTIFACT_PATH, ACK_COMMITMENT_JSON, ATTESTATIONS_JSON, ATTESTATIONS_PATH, ack_artifact,
    attestation_artifact,
};
pub use bootstrap::BOOTSTRAP_SITE;
pub use identity::{
    APP_NAME, APP_TAGLINE, EXPECTED_PGP_FINGERPRINT, IDENTITY_PATH, PUBLIC_KEY_BLOCK,
    PUBLIC_KEY_PATH, fingerprint_matches,
};
pub use policy::{ACCESS_POLICY, ADMIN_ADDRESSES};
pub use profile::{ASCII_BANNER, ASCII_PROFILE, HELP_TEXT, SHELL_TEXT};
