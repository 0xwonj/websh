//! Public crypto facade.
//!
//! Browser-safe verification primitives and acknowledgement helpers are
//! exported from this module instead of the internal engine layout.

pub use crate::engine::crypto::{ack, pgp};

#[cfg(feature = "eth-verify")]
pub use crate::engine::crypto::eth;
