//! Canonical generated content ledger artifact.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::crypto::attestation::{
    CONTENT_HASH, SubjectContent, compute_content_sha256, sha256_hex,
};

pub const CONTENT_LEDGER_SCHEME: &str = "websh.content-ledger.v1";
pub const CONTENT_LEDGER_PATH: &str = "content/.websh/ledger.json";
pub const CONTENT_LEDGER_CONTENT_PATH: &str = ".websh/ledger.json";
pub const CONTENT_LEDGER_ROUTE: &str = "/ledger";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentLedgerArtifact {
    pub version: u32,
    pub scheme: String,
    pub hash: String,
    pub entries: Vec<ContentLedgerEntry>,
    pub entry_count: usize,
    pub ledger_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentLedgerEntry {
    pub id: String,
    pub route: String,
    pub path: String,
    pub content: SubjectContent,
    pub content_sha256: String,
    pub entry_sha256: String,
}

#[derive(Serialize)]
struct ContentLedgerArtifactForHash<'a> {
    version: u32,
    scheme: &'a str,
    hash: &'a str,
    entries: &'a [ContentLedgerEntry],
    entry_count: usize,
}

#[derive(Serialize)]
struct ContentLedgerEntryForHash<'a> {
    id: &'a str,
    route: &'a str,
    path: &'a str,
    content: &'a SubjectContent,
    content_sha256: &'a str,
}

impl ContentLedgerArtifact {
    pub fn new(entries: Vec<ContentLedgerEntry>) -> Result<Self, serde_json::Error> {
        let entry_count = entries.len();
        let mut artifact = Self {
            version: 1,
            scheme: CONTENT_LEDGER_SCHEME.to_string(),
            hash: CONTENT_HASH.to_string(),
            entries,
            entry_count,
            ledger_sha256: String::new(),
        };
        artifact.ledger_sha256 = compute_ledger_sha256(&artifact)?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("unsupported ledger version {}", self.version));
        }
        if self.scheme != CONTENT_LEDGER_SCHEME {
            return Err(format!("unsupported ledger scheme {}", self.scheme));
        }
        if self.hash != CONTENT_HASH {
            return Err(format!("unsupported ledger hash {}", self.hash));
        }
        if self.entry_count != self.entries.len() {
            return Err("ledger entry_count does not match entries".to_string());
        }
        validate_sha256_field("ledger_sha256", &self.ledger_sha256)?;

        let mut ids = BTreeSet::new();
        let mut routes = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for entry in &self.entries {
            if !ids.insert(entry.id.as_str()) {
                return Err(format!("duplicate ledger id {}", entry.id));
            }
            if !routes.insert(entry.route.as_str()) {
                return Err(format!("duplicate ledger route {}", entry.route));
            }
            if !paths.insert(entry.path.as_str()) {
                return Err(format!("duplicate ledger path {}", entry.path));
            }

            validate_absolute_route(&entry.route)?;
            validate_content_path(&entry.path)?;
            let expected_id = format!("route:{}", entry.route);
            if entry.id != expected_id {
                return Err(format!("ledger id mismatch for {}", entry.path));
            }
            validate_entry_content(entry)?;
            validate_sha256_field("content_sha256", &entry.content_sha256)?;
            validate_sha256_field("entry_sha256", &entry.entry_sha256)?;

            let content_sha256 =
                compute_content_sha256(&entry.content).map_err(|error| error.to_string())?;
            if content_sha256 != entry.content_sha256 {
                return Err(format!("content hash mismatch for {}", entry.id));
            }
            let entry_sha256 = compute_entry_sha256(entry).map_err(|error| error.to_string())?;
            if entry_sha256 != entry.entry_sha256 {
                return Err(format!("entry hash mismatch for {}", entry.id));
            }
        }
        let ledger_sha256 = compute_ledger_sha256(self).map_err(|error| error.to_string())?;
        if ledger_sha256 != self.ledger_sha256 {
            return Err("ledger hash mismatch".to_string());
        }
        Ok(())
    }
}

fn validate_entry_content(entry: &ContentLedgerEntry) -> Result<(), String> {
    if entry.content.hash != CONTENT_HASH {
        return Err(format!("unsupported content hash for {}", entry.id));
    }
    if entry.content.files.is_empty() {
        return Err(format!("ledger content has no files for {}", entry.id));
    }

    let primary_file = format!("content/{}", entry.path);
    let mut previous_path: Option<&str> = None;
    let mut has_primary_file = false;
    for file in &entry.content.files {
        validate_artifact_file_path(&file.path)?;
        validate_sha256_field("content file sha256", &file.sha256)?;
        if file.bytes == 0 {
            return Err(format!("empty content file {}", file.path));
        }
        if file.path == primary_file {
            has_primary_file = true;
        }
        if let Some(previous_path) = previous_path
            && previous_path >= file.path.as_str()
        {
            return Err(format!(
                "content files must be strictly sorted for {}",
                entry.id
            ));
        }
        previous_path = Some(&file.path);
    }
    if !has_primary_file {
        return Err(format!("missing primary content file {}", primary_file));
    }
    Ok(())
}

fn validate_sha256_field(field: &str, value: &str) -> Result<(), String> {
    if value.len() != 66
        || !value.starts_with("0x")
        || !value[2..]
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(format!("{field} must be normalized 0x-prefixed sha256"));
    }
    Ok(())
}

fn validate_absolute_route(route: &str) -> Result<(), String> {
    if !route.starts_with('/') || route.contains('\\') || route.chars().any(char::is_control) {
        return Err(format!("ledger route must be absolute: {route}"));
    }
    if route != "/" {
        validate_path_segments(route.trim_start_matches('/'), "ledger route")?;
    }
    Ok(())
}

fn validate_content_path(path: &str) -> Result<(), String> {
    if path.starts_with('/') || path.contains('\\') || path.chars().any(char::is_control) {
        return Err(format!("ledger path must be content-root-relative: {path}"));
    }
    validate_path_segments(path, "ledger path")
}

fn validate_artifact_file_path(path: &str) -> Result<(), String> {
    if !path.starts_with("content/")
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
    {
        return Err(format!(
            "ledger content file path must be under content/: {path}"
        ));
    }
    validate_path_segments(path, "ledger content file path")
}

fn validate_path_segments(path: &str, label: &str) -> Result<(), String> {
    if path.is_empty()
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(format!("{label} contains an invalid segment: {path}"));
    }
    Ok(())
}

impl ContentLedgerEntry {
    pub fn new(
        id: String,
        route: String,
        path: String,
        content: SubjectContent,
    ) -> Result<Self, serde_json::Error> {
        let content_sha256 = compute_content_sha256(&content)?;
        let mut entry = Self {
            id,
            route,
            path,
            content,
            content_sha256,
            entry_sha256: String::new(),
        };
        entry.entry_sha256 = compute_entry_sha256(&entry)?;
        Ok(entry)
    }
}

pub fn compute_entry_sha256(entry: &ContentLedgerEntry) -> Result<String, serde_json::Error> {
    serde_json::to_vec(&ContentLedgerEntryForHash {
        id: &entry.id,
        route: &entry.route,
        path: &entry.path,
        content: &entry.content,
        content_sha256: &entry.content_sha256,
    })
    .map(|bytes| sha256_hex(&bytes))
}

pub fn compute_ledger_sha256(
    artifact: &ContentLedgerArtifact,
) -> Result<String, serde_json::Error> {
    serde_json::to_vec(&ContentLedgerArtifactForHash {
        version: artifact.version,
        scheme: &artifact.scheme,
        hash: &artifact.hash,
        entries: &artifact.entries,
        entry_count: artifact.entry_count,
    })
    .map(|bytes| sha256_hex(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::attestation::SubjectContentFile;

    fn sha(byte: char) -> String {
        format!("0x{}", byte.to_string().repeat(64))
    }

    fn entry(path: &str) -> ContentLedgerEntry {
        ContentLedgerEntry::new(
            format!("route:/{path}"),
            format!("/{path}"),
            path.to_string(),
            SubjectContent {
                hash: CONTENT_HASH.to_string(),
                files: vec![SubjectContentFile {
                    path: format!("content/{path}"),
                    sha256: sha('a'),
                    bytes: 3,
                }],
            },
        )
        .unwrap()
    }

    #[test]
    fn ledger_hash_validates_without_metadata_fields() {
        let artifact = ContentLedgerArtifact::new(vec![entry("writing/hello.md")]).unwrap();
        artifact.validate().unwrap();
        let encoded = serde_json::to_string(&artifact).unwrap();
        assert!(!encoded.contains("title"));
        assert!(!encoded.contains("description"));
        assert!(!encoded.contains("tags"));
        assert!(!encoded.contains("date"));
    }

    #[test]
    fn ledger_validation_accepts_sidecar_in_primary_entry() {
        let entry = ContentLedgerEntry::new(
            "route:/talks/a.pdf".to_string(),
            "/talks/a.pdf".to_string(),
            "talks/a.pdf".to_string(),
            SubjectContent {
                hash: CONTENT_HASH.to_string(),
                files: vec![
                    SubjectContentFile {
                        path: "content/talks/a.meta.json".to_string(),
                        sha256: sha('b'),
                        bytes: 19,
                    },
                    SubjectContentFile {
                        path: "content/talks/a.pdf".to_string(),
                        sha256: sha('c'),
                        bytes: 3,
                    },
                ],
            },
        )
        .unwrap();
        ContentLedgerArtifact::new(vec![entry])
            .unwrap()
            .validate()
            .unwrap();
    }

    #[test]
    fn ledger_validation_accepts_arbitrary_entry_order() {
        // The CLI is responsible for canonical ordering; validation only
        // enforces no duplicates and a matching ledger hash, so an unsorted
        // (but otherwise consistent) artifact still validates.
        let artifact =
            ContentLedgerArtifact::new(vec![entry("writing/z.md"), entry("projects/a.md")])
                .unwrap();
        artifact.validate().unwrap();
    }

    #[test]
    fn ledger_validation_rejects_duplicate_routes() {
        let mut duplicate = entry("projects/b.md");
        duplicate.route = "/projects/a.md".to_string();
        duplicate.id = "route:/other".to_string();
        let artifact = ContentLedgerArtifact::new(vec![entry("projects/a.md"), duplicate]).unwrap();
        let err = artifact.validate().unwrap_err();
        assert!(err.contains("duplicate ledger route"));
    }

    #[test]
    fn ledger_validation_rejects_duplicate_ids() {
        let mut duplicate = entry("projects/b.md");
        duplicate.id = "route:/projects/a.md".to_string();
        let artifact = ContentLedgerArtifact::new(vec![entry("projects/a.md"), duplicate]).unwrap();
        let err = artifact.validate().unwrap_err();
        assert!(err.contains("duplicate ledger id"));
    }

    #[test]
    fn ledger_validation_rejects_bad_id() {
        let mut bad = entry("writing/hello.md");
        bad.id = "content:writing/hello.md".to_string();
        let artifact = ContentLedgerArtifact::new(vec![bad]).unwrap();
        let err = artifact.validate().unwrap_err();
        assert!(err.contains("ledger id mismatch"));
    }

    #[test]
    fn ledger_validation_rejects_bad_route_and_path() {
        let mut bad_route = entry("writing/hello.md");
        bad_route.route = "writing/hello.md".to_string();
        let artifact = ContentLedgerArtifact::new(vec![bad_route]).unwrap();
        assert!(artifact.validate().unwrap_err().contains("absolute"));

        let mut bad_path = entry("writing/hello.md");
        bad_path.path = "/writing/hello.md".to_string();
        let artifact = ContentLedgerArtifact::new(vec![bad_path]).unwrap();
        assert!(
            artifact
                .validate()
                .unwrap_err()
                .contains("content-root-relative")
        );
    }

    #[test]
    fn ledger_validation_rejects_bad_hash_format() {
        let mut bad = entry("writing/hello.md");
        bad.content.files[0].sha256 = "0xaaa".to_string();
        let artifact = ContentLedgerArtifact::new(vec![bad]).unwrap();
        assert!(
            artifact
                .validate()
                .unwrap_err()
                .contains("normalized 0x-prefixed sha256")
        );
    }

    #[test]
    fn ledger_validation_rejects_missing_primary_file() {
        let bad = ContentLedgerEntry::new(
            "route:/writing/hello.md".to_string(),
            "/writing/hello.md".to_string(),
            "writing/hello.md".to_string(),
            SubjectContent {
                hash: CONTENT_HASH.to_string(),
                files: vec![SubjectContentFile {
                    path: "content/writing/hello.meta.json".to_string(),
                    sha256: sha('d'),
                    bytes: 4,
                }],
            },
        )
        .unwrap();
        let artifact = ContentLedgerArtifact::new(vec![bad]).unwrap();
        assert!(artifact.validate().unwrap_err().contains("missing primary"));
    }

    #[test]
    fn ledger_validation_rejects_unsorted_content_files() {
        let bad = ContentLedgerEntry::new(
            "route:/talks/a.pdf".to_string(),
            "/talks/a.pdf".to_string(),
            "talks/a.pdf".to_string(),
            SubjectContent {
                hash: CONTENT_HASH.to_string(),
                files: vec![
                    SubjectContentFile {
                        path: "content/talks/a.pdf".to_string(),
                        sha256: sha('e'),
                        bytes: 3,
                    },
                    SubjectContentFile {
                        path: "content/talks/a.meta.json".to_string(),
                        sha256: sha('f'),
                        bytes: 4,
                    },
                ],
            },
        )
        .unwrap();
        let artifact = ContentLedgerArtifact::new(vec![bad]).unwrap();
        assert!(artifact.validate().unwrap_err().contains("content files"));
    }

    #[test]
    fn ledger_deserialize_rejects_unknown_fields() {
        let body = r#"{
            "version":1,
            "scheme":"websh.content-ledger.v1",
            "hash":"sha256",
            "entries":[],
            "entry_count":0,
            "ledger_sha256":"0x0000000000000000000000000000000000000000000000000000000000000000",
            "date":"2026-04-26"
        }"#;
        assert!(serde_json::from_str::<ContentLedgerArtifact>(body).is_err());
    }
}
