use std::collections::BTreeSet;

use crate::engine::attestation::artifact::{CONTENT_HASH, compute_content_sha256};

use super::{
    CONTENT_LEDGER_GENESIS_HASH, CONTENT_LEDGER_SCHEME, ContentLedger, ContentLedgerCategory,
    ContentLedgerEntry, ContentLedgerSortKey, compute_block_sha256,
};

impl ContentLedger {
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
