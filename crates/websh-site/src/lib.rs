//! Deployment-specific site configuration and bundled site assets.
//!
//! `websh-core` owns reusable domain and engine behavior. This crate owns the
//! concrete deployed site's identity, policy, shell copy, and public artifact
//! locations.

pub mod artifacts;
pub mod bootstrap;
pub mod identity;
pub mod profile;

pub use artifacts::{ACK_ARTIFACT_PATH, ATTESTATIONS_PATH};
pub use bootstrap::BOOTSTRAP_SITE;
pub use identity::{
    APP_NAME, APP_TAGLINE, EXPECTED_PGP_FINGERPRINT, PUBLIC_KEY_BLOCK, PUBLIC_KEY_PATH,
    content_trust, fingerprint_matches, pgp_policy,
};
pub use profile::{ASCII_BANNER, HELP_TEXT};
