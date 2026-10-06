//! Authored home data and the signed root projection of a content snapshot.

use serde::{Deserialize, Serialize};

use crate::crypto::ack::AckArtifact;

use super::{GitHubMount, LinkRef};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseMetadata {
    pub purpose: String,
    pub site: String,
    pub sequence: u64,
    /// Seconds since the Unix epoch. Zero means an unissued generation candidate.
    pub issued_at: u64,
    pub home: HomeProjection,
    pub mounts: Vec<GitHubMount>,
    /// Canonical index paths of durable publication units, excluding profile and metadata.
    pub publications: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HomeProjection {
    pub profile: Profile,
    pub now: Now,
    pub ack: AckArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub title: String,
    pub tagline: String,
    pub name: String,
    pub affiliation: String,
    pub email: String,
    pub abstract_text: String,
    pub introduction: String,
    pub public_identity: String,
    pub private_identity: String,
    pub status: String,
    pub research: Vec<String>,
    pub tools: Vec<String>,
    pub habits: Vec<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub links: Vec<LinkRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Now {
    pub items: Vec<NowItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NowItem {
    pub date: String,
    pub text: String,
}
