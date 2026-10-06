//! Immutable content contracts, authentication, and publication ordering.
//!
//! Serialized source snapshots are untrusted bytes. Only the verifier constructs
//! `VerifiedRelease`; unsigned sources use the same validated index without that evidence.

mod locator;
mod validate;
mod verify;

pub use crate::domain::{HomeProjection, Manifest, Now, NowItem, Profile, ReleaseMetadata};
pub use locator::{GitCommit, SourcePointer, SourceSnapshot};
pub use validate::{CONTENT_PURPOSE, FileIntegrity, MAX_MANIFEST_BYTES, ReleaseError, ReleaseId};
pub use verify::{AcceptedRelease, TrustPolicy, VerifiedRelease, verify_release};

#[cfg(test)]
mod tests;
