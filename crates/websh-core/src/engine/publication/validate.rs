use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::{UnicodeNormalization, is_nfc};

use crate::{
    domain::{ContentManifestEntry, Manifest},
    ports::{ScannedSubtree, parse_manifest_snapshot},
};

pub const CONTENT_PURPOSE: &str = "websh.content";
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;
const MAX_BODY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 1024;
const MAX_PATH_SEGMENTS: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    #[error("invalid content snapshot: {0}")]
    Invalid(String),
    #[error("content JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("content index: {0}")]
    Index(#[from] crate::ports::ManifestSnapshotError),
    #[error("content signature: {0}")]
    Signature(#[from] crate::crypto::pgp::VerificationError),
    #[error("content body integrity mismatch")]
    Integrity,
    #[error("content release is older than the accepted live sequence")]
    Rollback,
    #[error("content release reuses an accepted sequence for different bytes")]
    SequenceConflict,
}

/// SHA-256 of the exact manifest bytes, not a Git identifier or parsed JSON.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ReleaseId(String);

impl ReleaseId {
    pub fn of(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes)))
    }
    pub fn parse(value: impl Into<String>) -> Result<Self, ReleaseError> {
        let value = value.into();
        if !is_digest(&value) {
            return Err(ReleaseError::Invalid("expected lowercase SHA-256".into()));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ReleaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl TryFrom<String> for ReleaseId {
    type Error = ReleaseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}
impl From<ReleaseId> for String {
    fn from(value: ReleaseId) -> Self {
        value.0
    }
}

/// Expected bytes from a validated index. Safe to use as a content-addressed cache key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIntegrity {
    pub sha256: ReleaseId,
    pub size: u64,
}
impl FileIntegrity {
    pub fn verify(&self, bytes: &[u8]) -> Result<(), ReleaseError> {
        if bytes.len() as u64 != self.size || ReleaseId::of(bytes) != self.sha256 {
            return Err(ReleaseError::Integrity);
        }
        Ok(())
    }
    pub fn cache_key(&self) -> String {
        format!("{}:{}", self.sha256, self.size)
    }
}

impl Manifest {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ReleaseError> {
        let manifest = Self::decode(bytes)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, ReleaseError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ReleaseError::Invalid("manifest exceeds size limit".into()));
        }
        Ok(serde_json::from_slice(bytes)?)
    }

    /// Validate structure, paths, routes, all file commitments, and root projections.
    /// Sequence/time zero are permitted here for a generated but unissued candidate.
    pub fn validate(&self) -> Result<ScannedSubtree, ReleaseError> {
        if self.entries.len() > MAX_ENTRIES {
            return Err(ReleaseError::Invalid("too many index entries".into()));
        }
        let body = serde_json::to_string(self)?;
        if body.len() > MAX_MANIFEST_BYTES {
            return Err(ReleaseError::Invalid("manifest exceeds size limit".into()));
        }
        // Bound implicit directory expansion before the route parser builds its tree.
        let mut aliases = BTreeMap::new();
        for entry in &self.entries {
            validate_path_aliases(&entry.path, &mut aliases)?;
        }
        let snapshot = parse_manifest_snapshot(&body)?;
        let paths = self
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<BTreeSet<_>>();
        let files = self
            .entries
            .iter()
            .filter(|entry| !entry.metadata.kind.is_directory_like())
            .map(|entry| entry.path.as_str())
            .collect::<BTreeSet<_>>();
        for entry in &self.entries {
            if entry
                .metadata
                .links()
                .is_some_and(|links| links.iter().any(|link| !safe_link(&link.url)))
            {
                return Err(ReleaseError::Invalid(format!(
                    "unsafe metadata link: {}",
                    entry.path
                )));
            }
            let folded = fold_path(&entry.path);
            if matches!(
                folded.as_str(),
                "manifest.json" | "manifest.sig" | "current.json"
            ) || [".websh/local", ".websh/state", ".git"]
                .iter()
                .any(|root| folded == *root || folded.starts_with(&format!("{root}/")))
            {
                return Err(ReleaseError::Invalid(format!(
                    "reserved publication path: {}",
                    entry.path
                )));
            }
            if !entry.metadata.kind.is_directory_like() {
                entry_integrity(entry)?;
            }
            let mut parent = entry.path.as_str();
            while let Some((prefix, _)) = parent.rsplit_once('/') {
                if files.contains(prefix) {
                    return Err(ReleaseError::Invalid(format!(
                        "file cannot contain another entry: {prefix}"
                    )));
                }
                parent = prefix;
            }
        }
        if let Some(release) = &self.release {
            if release.purpose != CONTENT_PURPOSE
                || release.site.trim().is_empty()
                || release.site.len() > 256
            {
                return Err(ReleaseError::Invalid("invalid release purpose/site".into()));
            }
            let profile = &release.home.profile;
            for (field, value) in [
                ("title", &profile.title),
                ("name", &profile.name),
                ("abstract", &profile.abstract_text),
            ] {
                if value.trim().is_empty() || value.len() > 64 * 1024 {
                    return Err(ReleaseError::Invalid(format!(
                        "missing or oversized profile {field}"
                    )));
                }
            }
            if profile.email.is_empty()
                || !profile.email.contains('@')
                || profile
                    .email
                    .chars()
                    .any(|ch| ch.is_whitespace() || ch.is_control() || "<>\"?&".contains(ch))
                || profile
                    .links
                    .iter()
                    .any(|link| !safe_link(&link.url) || link.label.trim().is_empty())
            {
                return Err(ReleaseError::Invalid("invalid profile email/link".into()));
            }
            if release.home.now.items.len() > 100 || release.mounts.len() > 64 {
                return Err(ReleaseError::Invalid(
                    "too many homepage items or mounts".into(),
                ));
            }
            for item in &release.home.now.items {
                if !valid_date(&item.date)
                    || item.text.trim().is_empty()
                    || item.text.len() > 16 * 1024
                {
                    return Err(ReleaseError::Invalid("invalid Now date/text".into()));
                }
            }
            release
                .home
                .ack
                .validate()
                .map_err(|error| ReleaseError::Invalid(error.to_string()))?;
            let mut roots = BTreeSet::new();
            for mount in &release.mounts {
                crate::domain::validate_mount_root(mount.mount_at())
                    .map_err(|error| ReleaseError::Invalid(error.to_string()))?;
                validate_path_aliases(
                    mount.mount_at().as_str().trim_start_matches('/'),
                    &mut aliases,
                )?;
                if !roots.insert(mount.mount_at())
                    || self.entries.iter().any(|entry| {
                        let root = mount.mount_at().as_str().trim_start_matches('/');
                        entry.path == root || entry.path.starts_with(&format!("{root}/"))
                    })
                {
                    return Err(ReleaseError::Invalid(format!(
                        "mount conflicts with content: {}",
                        mount.mount_at()
                    )));
                }
            }
            let mut publications = BTreeSet::new();
            for path in &release.publications {
                if !publications.insert(path)
                    || !paths.contains(path.as_str())
                    || path.starts_with('.')
                    || path.is_empty()
                {
                    return Err(ReleaseError::Invalid(format!(
                        "invalid publication: {path}"
                    )));
                }
            }
        }
        Ok(snapshot)
    }

    pub fn integrity(&self, path: &str) -> Result<FileIntegrity, ReleaseError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.path == path && !entry.metadata.kind.is_directory_like())
            .ok_or_else(|| ReleaseError::Invalid(format!("file is not indexed: {path}")))?;
        entry_integrity(entry)
    }

    pub fn verify_file(&self, path: &str, bytes: &[u8]) -> Result<(), ReleaseError> {
        self.integrity(path)?.verify(bytes)
    }
}

fn fold_path(path: &str) -> String {
    unicase::UniCase::new(path).to_folded_case().nfc().collect()
}

/// Include implicit directories: `Notes/a` and `notes/b` must not share a tree.
fn validate_path_aliases<'a>(
    path: &'a str,
    aliases: &mut BTreeMap<String, &'a str>,
) -> Result<(), ReleaseError> {
    if path.len() > MAX_PATH_BYTES || path.split('/').count() > MAX_PATH_SEGMENTS {
        return Err(ReleaseError::Invalid(
            "path exceeds length or depth limit".into(),
        ));
    }
    if !is_nfc(path) {
        return Err(ReleaseError::Invalid(format!("path must be NFC: {path}")));
    }
    let mut prefix = Some(path);
    while let Some(path) = prefix {
        if let Some(previous) = aliases.insert(fold_path(path), path)
            && previous != path
        {
            return Err(ReleaseError::Invalid(format!(
                "case-folding path collision: {previous} and {path}"
            )));
        }
        prefix = path.rsplit_once('/').map(|(parent, _)| parent);
    }
    Ok(())
}

fn entry_integrity(entry: &ContentManifestEntry) -> Result<FileIntegrity, ReleaseError> {
    let sha256 = ReleaseId::parse(entry.metadata.content_sha256().unwrap_or_default())?;
    let size = entry
        .metadata
        .size_bytes()
        .filter(|size| *size <= MAX_BODY_BYTES)
        .ok_or_else(|| {
            ReleaseError::Invalid(format!("missing or oversized file length: {}", entry.path))
        })?;
    Ok(FileIntegrity { sha256, size })
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, byte)| i != 4 && i != 7 && !byte.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = date[..4].parse().unwrap_or(0);
    let month: u32 = date[5..7].parse().unwrap_or(0);
    let day: u32 = date[8..].parse().unwrap_or(0);
    let max_day = match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    year > 0 && day > 0 && day <= max_day
}

fn safe_link(url: &str) -> bool {
    !url.chars().any(|ch| ch.is_whitespace() || ch.is_control())
        && ["https://", "http://", "mailto:"].iter().any(|scheme| {
            url.strip_prefix(scheme)
                .is_some_and(|rest| !rest.is_empty())
        })
}
