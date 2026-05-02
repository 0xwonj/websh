//! Canonical generated content ledger hash chain.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::crypto::attestation::{CONTENT_HASH, ContentFile, compute_content_sha256, sha256_hex};

pub const CONTENT_LEDGER_SCHEME: &str = "websh.content-ledger.v1";
pub const CONTENT_LEDGER_PATH: &str = "content/.websh/ledger.json";
pub const CONTENT_LEDGER_CONTENT_PATH: &str = ".websh/ledger.json";
pub const CONTENT_LEDGER_ROUTE: &str = "/ledger";
pub const CONTENT_LEDGER_GENESIS_HASH: &str =
    "0x0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentLedger {
    pub version: u32,
    pub scheme: String,
    pub hash: String,
    pub genesis_hash: String,
    pub blocks: Vec<ContentLedgerBlock>,
    pub block_count: usize,
    pub chain_head: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentLedgerBlock {
    pub height: u64,
    pub sort_key: ContentLedgerSortKey,
    pub prev_block_sha256: String,
    pub block_sha256: String,
    pub entry: ContentLedgerEntry,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentLedgerEntry {
    pub id: String,
    pub route: String,
    pub path: String,
    pub category: ContentLedgerCategory,
    pub content_files: Vec<ContentFile>,
    pub content_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContentLedgerSortKey {
    pub date: Option<String>,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentLedgerCategory {
    Writing,
    Projects,
    Papers,
    Talks,
    Misc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentLedgerInput {
    pub sort_key: ContentLedgerSortKey,
    pub entry: ContentLedgerEntry,
}

#[derive(Serialize)]
struct ContentLedgerBlockForHash<'a> {
    height: u64,
    sort_key: &'a ContentLedgerSortKey,
    prev_block_sha256: &'a str,
    entry: &'a ContentLedgerEntry,
}

impl ContentLedger {
    pub fn new(mut inputs: Vec<ContentLedgerInput>) -> Result<Self, serde_json::Error> {
        inputs.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));

        let genesis_hash = CONTENT_LEDGER_GENESIS_HASH.to_string();
        let mut prev_block_sha256 = genesis_hash.clone();
        let mut blocks = Vec::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            let mut block = ContentLedgerBlock {
                height: index as u64 + 1,
                sort_key: input.sort_key,
                prev_block_sha256,
                block_sha256: String::new(),
                entry: input.entry,
            };
            block.block_sha256 = compute_block_sha256(&block)?;
            prev_block_sha256 = block.block_sha256.clone();
            blocks.push(block);
        }

        let block_count = blocks.len();
        let chain_head = blocks
            .last()
            .map(|block| block.block_sha256.clone())
            .unwrap_or_else(|| genesis_hash.clone());

        Ok(Self {
            version: 1,
            scheme: CONTENT_LEDGER_SCHEME.to_string(),
            hash: CONTENT_HASH.to_string(),
            genesis_hash,
            blocks,
            block_count,
            chain_head,
        })
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
        if self.genesis_hash != CONTENT_LEDGER_GENESIS_HASH {
            return Err("ledger genesis_hash mismatch".to_string());
        }
        validate_sha256_field("genesis_hash", &self.genesis_hash)?;
        validate_sha256_field("chain_head", &self.chain_head)?;
        if self.block_count != self.blocks.len() {
            return Err("ledger block_count does not match blocks".to_string());
        }

        let mut ids = BTreeSet::new();
        let mut routes = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut previous_sort_key: Option<&ContentLedgerSortKey> = None;
        let mut expected_prev_block_sha256 = self.genesis_hash.clone();

        for (index, block) in self.blocks.iter().enumerate() {
            let expected_height = index as u64 + 1;
            if block.height != expected_height {
                return Err(format!(
                    "ledger block height mismatch at index {}",
                    index + 1
                ));
            }
            validate_sort_key(&block.sort_key)?;
            if let Some(previous_sort_key) = previous_sort_key
                && previous_sort_key > &block.sort_key
            {
                return Err("ledger blocks are not sorted canonically".to_string());
            }
            previous_sort_key = Some(&block.sort_key);

            validate_sha256_field("prev_block_sha256", &block.prev_block_sha256)?;
            validate_sha256_field("block_sha256", &block.block_sha256)?;
            if block.prev_block_sha256 != expected_prev_block_sha256 {
                return Err(format!(
                    "prev_block_sha256 mismatch at block {}",
                    block.height
                ));
            }

            let entry = &block.entry;
            if block.sort_key.path != entry.path {
                return Err(format!("ledger sort_key path mismatch for {}", entry.id));
            }
            if !ids.insert(entry.id.as_str()) {
                return Err(format!("duplicate ledger id {}", entry.id));
            }
            if !routes.insert(entry.route.as_str()) {
                return Err(format!("duplicate ledger route {}", entry.route));
            }
            if !paths.insert(entry.path.as_str()) {
                return Err(format!("duplicate ledger path {}", entry.path));
            }

            validate_entry(entry)?;
            let content_sha256 =
                compute_content_sha256(&entry.content_files).map_err(|error| error.to_string())?;
            if content_sha256 != entry.content_sha256 {
                return Err(format!("content hash mismatch for {}", entry.id));
            }

            let block_sha256 = compute_block_sha256(block).map_err(|error| error.to_string())?;
            if block_sha256 != block.block_sha256 {
                return Err(format!("block hash mismatch for {}", entry.id));
            }
            expected_prev_block_sha256 = block.block_sha256.clone();
        }

        let expected_chain_head = self
            .blocks
            .last()
            .map(|block| block.block_sha256.as_str())
            .unwrap_or(&self.genesis_hash);
        if self.chain_head != expected_chain_head {
            return Err("ledger chain_head mismatch".to_string());
        }

        Ok(())
    }
}

impl ContentLedgerBlock {
    pub fn refresh_hash(&mut self) -> Result<(), serde_json::Error> {
        self.block_sha256 = compute_block_sha256(self)?;
        Ok(())
    }
}

impl ContentLedgerEntry {
    pub fn new(
        id: String,
        route: String,
        path: String,
        category: ContentLedgerCategory,
        content_files: Vec<ContentFile>,
    ) -> Result<Self, serde_json::Error> {
        let content_sha256 = compute_content_sha256(&content_files)?;
        Ok(Self {
            id,
            route,
            path,
            category,
            content_files,
            content_sha256,
        })
    }
}

impl ContentLedgerSortKey {
    pub fn new(date: Option<String>, path: String) -> Self {
        Self { date, path }
    }
}

impl ContentLedgerInput {
    pub fn new(sort_key: ContentLedgerSortKey, entry: ContentLedgerEntry) -> Self {
        Self { sort_key, entry }
    }
}

impl ContentLedgerCategory {
    pub fn for_path(path: &str) -> Self {
        match path.trim_start_matches('/').split('/').next().unwrap_or("") {
            "writing" => Self::Writing,
            "projects" => Self::Projects,
            "papers" => Self::Papers,
            "talks" => Self::Talks,
            _ => Self::Misc,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Writing => "writing",
            Self::Projects => "projects",
            Self::Papers => "papers",
            Self::Talks => "talks",
            Self::Misc => "misc",
        }
    }
}

pub fn compute_block_sha256(block: &ContentLedgerBlock) -> Result<String, serde_json::Error> {
    serde_json::to_vec(&ContentLedgerBlockForHash {
        height: block.height,
        sort_key: &block.sort_key,
        prev_block_sha256: &block.prev_block_sha256,
        entry: &block.entry,
    })
    .map(|bytes| sha256_hex(&bytes))
}

fn validate_entry(entry: &ContentLedgerEntry) -> Result<(), String> {
    validate_absolute_route(&entry.route)?;
    validate_content_path(&entry.path)?;
    let expected_id = format!("route:{}", entry.route);
    if entry.id != expected_id {
        return Err(format!("ledger id mismatch for {}", entry.path));
    }
    if entry.category != ContentLedgerCategory::for_path(&entry.path) {
        return Err(format!("ledger category mismatch for {}", entry.path));
    }
    validate_entry_content(entry)?;
    validate_sha256_field("content_sha256", &entry.content_sha256)
}

fn validate_entry_content(entry: &ContentLedgerEntry) -> Result<(), String> {
    if entry.content_files.is_empty() {
        return Err(format!("ledger content has no files for {}", entry.id));
    }

    let primary_file = format!("content/{}", entry.path);
    let mut previous_path: Option<&str> = None;
    let mut has_primary_file = false;
    for file in &entry.content_files {
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

fn validate_sort_key(sort_key: &ContentLedgerSortKey) -> Result<(), String> {
    if let Some(date) = &sort_key.date {
        validate_sort_key_date(date)?;
    }
    validate_content_path(&sort_key.path)
}

fn validate_sort_key_date(date: &str) -> Result<(), String> {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(|byte| byte.is_ascii_digit())
        || !bytes[5..7].iter().all(|byte| byte.is_ascii_digit())
        || !bytes[8..10].iter().all(|byte| byte.is_ascii_digit())
        || date.chars().any(char::is_control)
    {
        return Err("ledger sort_key date is invalid".to_string());
    }

    let year = date[0..4]
        .parse::<u32>()
        .map_err(|_| "ledger sort_key date is invalid".to_string())?;
    let month = date[5..7]
        .parse::<u32>()
        .map_err(|_| "ledger sort_key date is invalid".to_string())?;
    let day = date[8..10]
        .parse::<u32>()
        .map_err(|_| "ledger sort_key date is invalid".to_string())?;

    let Some(max_day) = days_in_month(year, month) else {
        return Err("ledger sort_key date is invalid".to_string());
    };
    if day == 0 || day > max_day {
        return Err("ledger sort_key date is invalid".to_string());
    }

    Ok(())
}

fn days_in_month(year: u32, month: u32) -> Option<u32> {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    };
    Some(days)
}

fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sha(byte: char) -> String {
        format!("0x{}", byte.to_string().repeat(64))
    }

    fn entry(path: &str) -> ContentLedgerEntry {
        ContentLedgerEntry::new(
            format!("route:/{path}"),
            format!("/{path}"),
            path.to_string(),
            ContentLedgerCategory::for_path(path),
            vec![ContentFile {
                path: format!("content/{path}"),
                sha256: sha('a'),
                bytes: 3,
            }],
        )
        .unwrap()
    }

    fn input(date: Option<&str>, path: &str) -> ContentLedgerInput {
        ContentLedgerInput::new(
            ContentLedgerSortKey::new(date.map(str::to_string), path.to_string()),
            entry(path),
        )
    }

    #[test]
    fn ledger_hash_validates_without_metadata_fields() {
        let ledger =
            ContentLedger::new(vec![input(Some("2026-04-01"), "writing/hello.md")]).unwrap();
        ledger.validate().unwrap();
        let encoded = serde_json::to_string(&ledger).unwrap();
        assert!(!encoded.contains("title"));
        assert!(!encoded.contains("description"));
        assert!(!encoded.contains("tags"));
        assert!(!encoded.contains("access"));
    }

    #[test]
    fn ledger_validation_accepts_sidecar_in_primary_entry() {
        let entry = ContentLedgerEntry::new(
            "route:/talks/a.pdf".to_string(),
            "/talks/a.pdf".to_string(),
            "talks/a.pdf".to_string(),
            ContentLedgerCategory::Talks,
            vec![
                ContentFile {
                    path: "content/talks/a.meta.json".to_string(),
                    sha256: sha('b'),
                    bytes: 19,
                },
                ContentFile {
                    path: "content/talks/a.pdf".to_string(),
                    sha256: sha('c'),
                    bytes: 3,
                },
            ],
        )
        .unwrap();
        ContentLedger::new(vec![ContentLedgerInput::new(
            ContentLedgerSortKey::new(Some("2026-04-01".to_string()), "talks/a.pdf".to_string()),
            entry,
        )])
        .unwrap()
        .validate()
        .unwrap();
    }

    #[test]
    fn ledger_validation_accepts_canonical_sort_key_date_and_none() {
        let ledger = ContentLedger::new(vec![
            input(Some("2024-02-29"), "writing/leap.md"),
            input(None, "misc/undated.txt"),
        ])
        .unwrap();
        ledger.validate().unwrap();
    }

    #[test]
    fn ledger_validation_rejects_malformed_sort_key_dates() {
        for date in [
            "",
            "2026-4-01",
            "2026-04-1",
            "20260401",
            "2026/04/01",
            "2026-04-01T12:00:00Z",
            "2026-04-01\n",
            "2026-00-01",
            "2026-13-01",
            "2026-04-00",
            "2026-04-31",
            "2026-02-29",
        ] {
            let ledger =
                ContentLedger::new(vec![input(Some(date), "writing/bad-date.md")]).unwrap();
            assert!(
                ledger
                    .validate()
                    .unwrap_err()
                    .contains("ledger sort_key date is invalid"),
                "date {date:?} should fail validation"
            );
        }
    }

    #[test]
    fn ledger_assigns_canonical_order_heights_and_chain_links() {
        let ledger = ContentLedger::new(vec![
            input(Some("2026-04-01"), "writing/z.md"),
            input(None, "misc/b.txt"),
            input(Some("2026-01-15"), "projects/a.md"),
            input(None, "misc/a.txt"),
        ])
        .unwrap();
        ledger.validate().unwrap();

        let paths = ledger
            .blocks
            .iter()
            .map(|block| block.entry.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec!["misc/a.txt", "misc/b.txt", "projects/a.md", "writing/z.md"]
        );
        assert_eq!(ledger.blocks[0].height, 1);
        assert_eq!(ledger.blocks[1].height, 2);
        assert_eq!(ledger.blocks[0].prev_block_sha256, ledger.genesis_hash);
        assert_eq!(
            ledger.blocks[1].prev_block_sha256,
            ledger.blocks[0].block_sha256
        );
        assert_eq!(
            ledger.chain_head,
            ledger.blocks.last().unwrap().block_sha256
        );
    }

    #[test]
    fn empty_ledger_head_points_to_genesis() {
        let ledger = ContentLedger::new(Vec::new()).unwrap();
        ledger.validate().unwrap();
        assert_eq!(ledger.block_count, 0);
        assert_eq!(ledger.chain_head, ledger.genesis_hash);
    }

    #[test]
    fn ledger_validation_rejects_duplicate_routes_ids_and_paths() {
        let mut duplicate_route = entry("projects/b.md");
        duplicate_route.route = "/projects/a.md".to_string();
        duplicate_route.id = "route:/other".to_string();
        let ledger = ContentLedger::new(vec![
            ContentLedgerInput::new(
                ContentLedgerSortKey::new(None, "projects/a.md".to_string()),
                entry("projects/a.md"),
            ),
            ContentLedgerInput::new(
                ContentLedgerSortKey::new(
                    Some("2026-01-01".to_string()),
                    "projects/b.md".to_string(),
                ),
                duplicate_route,
            ),
        ])
        .unwrap();
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("duplicate ledger route")
        );

        let mut duplicate_id = entry("projects/b.md");
        duplicate_id.id = "route:/projects/a.md".to_string();
        let ledger = ContentLedger::new(vec![
            input(None, "projects/a.md"),
            ContentLedgerInput::new(
                ContentLedgerSortKey::new(
                    Some("2026-01-01".to_string()),
                    "projects/b.md".to_string(),
                ),
                duplicate_id,
            ),
        ])
        .unwrap();
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("duplicate ledger id")
        );

        let mut duplicate_path = entry("projects/b.md");
        duplicate_path.path = "projects/a.md".to_string();
        let ledger = ContentLedger::new(vec![
            input(None, "projects/a.md"),
            ContentLedgerInput::new(
                ContentLedgerSortKey::new(
                    Some("2026-01-01".to_string()),
                    "projects/a.md".to_string(),
                ),
                duplicate_path,
            ),
        ])
        .unwrap();
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("duplicate ledger path")
        );
    }

    #[test]
    fn ledger_validation_rejects_bad_id_route_and_path() {
        let mut bad_id = entry("writing/hello.md");
        bad_id.id = "content:writing/hello.md".to_string();
        let ledger = ContentLedger::new(vec![ContentLedgerInput::new(
            ContentLedgerSortKey::new(None, "writing/hello.md".to_string()),
            bad_id,
        )])
        .unwrap();
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("ledger id mismatch")
        );

        let mut bad_route = entry("writing/hello.md");
        bad_route.route = "writing/hello.md".to_string();
        let ledger = ContentLedger::new(vec![ContentLedgerInput::new(
            ContentLedgerSortKey::new(None, "writing/hello.md".to_string()),
            bad_route,
        )])
        .unwrap();
        assert!(ledger.validate().unwrap_err().contains("absolute"));

        let mut bad_path = entry("writing/hello.md");
        bad_path.path = "/writing/hello.md".to_string();
        let ledger = ContentLedger::new(vec![ContentLedgerInput::new(
            ContentLedgerSortKey::new(None, "/writing/hello.md".to_string()),
            bad_path,
        )])
        .unwrap();
        assert!(
            ledger
                .validate()
                .unwrap_err()
                .contains("content-root-relative")
        );
    }

    #[test]
    fn ledger_validation_rejects_missing_primary_and_unsorted_content_files() {
        let missing_primary = ContentLedgerEntry::new(
            "route:/writing/hello.md".to_string(),
            "/writing/hello.md".to_string(),
            "writing/hello.md".to_string(),
            ContentLedgerCategory::Writing,
            vec![ContentFile {
                path: "content/writing/hello.meta.json".to_string(),
                sha256: sha('d'),
                bytes: 4,
            }],
        )
        .unwrap();
        let ledger = ContentLedger::new(vec![ContentLedgerInput::new(
            ContentLedgerSortKey::new(None, "writing/hello.md".to_string()),
            missing_primary,
        )])
        .unwrap();
        assert!(ledger.validate().unwrap_err().contains("missing primary"));

        let unsorted = ContentLedgerEntry::new(
            "route:/talks/a.pdf".to_string(),
            "/talks/a.pdf".to_string(),
            "talks/a.pdf".to_string(),
            ContentLedgerCategory::Talks,
            vec![
                ContentFile {
                    path: "content/talks/a.pdf".to_string(),
                    sha256: sha('e'),
                    bytes: 3,
                },
                ContentFile {
                    path: "content/talks/a.meta.json".to_string(),
                    sha256: sha('f'),
                    bytes: 4,
                },
            ],
        )
        .unwrap();
        let ledger = ContentLedger::new(vec![ContentLedgerInput::new(
            ContentLedgerSortKey::new(None, "talks/a.pdf".to_string()),
            unsorted,
        )])
        .unwrap();
        assert!(ledger.validate().unwrap_err().contains("content files"));
    }

    #[test]
    fn ledger_validation_rejects_tampering() {
        let mut ledger = ContentLedger::new(vec![
            input(Some("2026-01-01"), "writing/a.md"),
            input(Some("2026-02-01"), "projects/b.md"),
        ])
        .unwrap();
        ledger.blocks[0].entry.content_files[0].sha256 = sha('b');
        assert!(ledger.validate().unwrap_err().contains("content hash"));

        let mut ledger = ContentLedger::new(vec![
            input(Some("2026-01-01"), "writing/a.md"),
            input(Some("2026-02-01"), "projects/b.md"),
        ])
        .unwrap();
        ledger.blocks[1].prev_block_sha256 = sha('c');
        assert!(ledger.validate().unwrap_err().contains("prev_block_sha256"));

        let mut ledger = ContentLedger::new(vec![input(None, "writing/a.md")]).unwrap();
        ledger.blocks[0].block_sha256 = sha('d');
        assert!(ledger.validate().unwrap_err().contains("block hash"));

        let mut ledger = ContentLedger::new(vec![
            input(Some("2026-01-01"), "writing/a.md"),
            input(Some("2026-02-01"), "projects/b.md"),
        ])
        .unwrap();
        ledger.blocks.swap(0, 1);
        assert!(ledger.validate().is_err());

        let mut ledger = ContentLedger::new(vec![input(None, "writing/a.md")]).unwrap();
        ledger.blocks[0].height = 2;
        assert!(ledger.validate().unwrap_err().contains("height"));

        let mut ledger = ContentLedger::new(vec![input(None, "writing/a.md")]).unwrap();
        ledger.blocks[0].entry.category = ContentLedgerCategory::Projects;
        assert!(ledger.validate().unwrap_err().contains("category"));

        let mut ledger = ContentLedger::new(vec![input(None, "writing/a.md")]).unwrap();
        ledger.block_count = 2;
        assert!(ledger.validate().unwrap_err().contains("block_count"));

        let mut ledger = ContentLedger::new(vec![input(None, "writing/a.md")]).unwrap();
        ledger.chain_head = sha('e');
        assert!(ledger.validate().unwrap_err().contains("chain_head"));
    }

    #[test]
    fn legacy_ledger_shape_is_rejected() {
        let legacy = json!({
            "version": 1,
            "scheme": "websh.content-ledger.v1",
            "hash": "sha256",
            "entries": [],
            "entry_count": 0,
            "ledger_sha256": CONTENT_LEDGER_GENESIS_HASH,
        });
        assert!(serde_json::from_value::<ContentLedger>(legacy).is_err());
    }
}
