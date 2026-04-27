//! Cryptographic helpers used by the homepage and future attestations.
//!
//! UI components should keep presentation concerns local and call into this
//! module for any claim that needs to be reproducible or verifiable.

pub mod ack;
pub mod attestation;
pub mod eth;
pub mod ledger;
pub mod pgp;
