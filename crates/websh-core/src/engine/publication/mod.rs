//! Immutable content contracts, authentication, and publication ordering.
//!
//! Serialized source snapshots are untrusted bytes. Only the verifier constructs
//! `VerifiedRelease`; unsigned sources use the same validated index without that evidence.

mod chain;
mod home;
mod ledger;
mod locator;
mod membership;
mod subjects;
mod validate;
mod verify;

pub use crate::domain::{HomeProjection, Manifest, Now, NowItem, Profile, ReleaseMetadata};
pub use chain::{PublicationBlock, PublicationChain};
pub use home::{RecentItem, count_toc_entries, recent_items_from_fs};
pub use ledger::{
    LedgerEntry, LedgerFilter, LedgerModel, build_ledger_model, ledger_filter_for_route,
};
pub use locator::{GitCommit, SourcePointer, SourceSnapshot};
pub use subjects::{
    VerifiedSubject, expected_subjects, subject_for_path, verify_subject, verify_subject_signature,
};
pub use validate::{CONTENT_PURPOSE, FileIntegrity, MAX_MANIFEST_BYTES, ReleaseError, ReleaseId};
pub use verify::{AcceptedRelease, TrustPolicy, VerifiedRelease, verify_release};

#[cfg(test)]
mod tests;
