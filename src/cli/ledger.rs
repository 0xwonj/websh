use std::fs;
use std::path::Path;

use crate::crypto::attestation::subject_id_for_route;
use crate::crypto::ledger::{
    CONTENT_LEDGER_CONTENT_PATH, ContentLedgerArtifact, ContentLedgerEntry,
};
use crate::utils::format::iso_date_prefix;

use super::CliResult;
use super::attest::build_content_files;
use super::io::write_json;
use super::manifest::{
    collect_files_recursive, content_entry_raw_date, matching_file_sidecar, relative_path_from,
    resolve_path, route_for_content_path, should_skip_primary_content_file,
};

pub(crate) fn generate_content_ledger(
    root: &Path,
    content_dir: &Path,
) -> CliResult<ContentLedgerArtifact> {
    let content_root = resolve_path(root, content_dir);
    fs::create_dir_all(&content_root)?;

    let mut files = Vec::new();
    collect_files_recursive(&content_root, &mut files)?;

    let mut staged: Vec<(String, ContentLedgerEntry)> = Vec::new();
    for file_path in files {
        let rel_path = relative_path_from(&content_root, &file_path)?;
        if should_skip_primary_content_file(&rel_path) {
            continue;
        }

        let mut content_paths = vec![file_path.clone()];
        if let Some(sidecar) = matching_file_sidecar(&content_root, &rel_path) {
            content_paths.push(sidecar);
        }

        let route = route_for_content_path(&rel_path);
        let content_files = build_content_files(root, &content_paths)?;
        let sort_date = sort_date_for_entry(&content_root, &file_path, &rel_path);
        staged.push((
            sort_date,
            ContentLedgerEntry::new(subject_id_for_route(&route), route, rel_path, content_files)?,
        ));
    }
    // Canonical ledger order is `(content date asc, path asc)`. Undated entries
    // collapse to the empty string so they sort first canonically (= last in
    // the newest-first display) and the chain reads coherently.
    staged.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.path.cmp(&right.1.path))
    });
    let entries = staged.into_iter().map(|(_, entry)| entry).collect();

    let ledger = ContentLedgerArtifact::new(entries)?;
    ledger.validate()?;

    let ledger_path = content_root.join(CONTENT_LEDGER_CONTENT_PATH);
    if let Some(parent) = ledger_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_json(&ledger_path, &ledger)?;

    Ok(ledger)
}

fn sort_date_for_entry(content_root: &Path, file_path: &Path, rel_path: &str) -> String {
    content_entry_raw_date(content_root, file_path, rel_path)
        .as_deref()
        .and_then(iso_date_prefix)
        .map(|date| date.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        root.push(format!("websh-ledger-test-{name}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn ledger_groups_sidecars_and_excludes_generated_files() {
        let root = temp_root("sidecar");
        let content = root.join("content");
        fs::create_dir_all(content.join("talks")).unwrap();
        fs::create_dir_all(content.join(".websh")).unwrap();
        fs::write(content.join("manifest.json"), "{}").unwrap();
        fs::write(content.join(".websh/old.json"), "{}").unwrap();
        fs::write(content.join("talks/a.pdf"), b"pdf").unwrap();
        fs::write(
            content.join("talks/a.meta.json"),
            r#"{"title":"Talk","tags":["zk"],"date":"2026-04-01"}"#,
        )
        .unwrap();

        let ledger = generate_content_ledger(&root, Path::new("content")).unwrap();
        assert_eq!(ledger.entries.len(), 1);
        let entry = &ledger.entries[0];
        assert_eq!(entry.path, "talks/a.pdf");
        assert_eq!(entry.route, "/talks/a.pdf");
        assert_eq!(
            entry
                .content_files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["content/talks/a.meta.json", "content/talks/a.pdf"]
        );

        let encoded = serde_json::to_string(&ledger).unwrap();
        assert!(!encoded.contains("\"title\""));
        assert!(!encoded.contains("\"tags\""));
        assert!(root.join("content/.websh/ledger.json").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ledger_sorts_entries_by_date_with_path_tiebreaker() {
        let root = temp_root("date-sort");
        let content = root.join("content");
        fs::create_dir_all(content.join("writing")).unwrap();
        fs::create_dir_all(content.join("papers")).unwrap();
        fs::create_dir_all(content.join("misc")).unwrap();

        // Frontmatter-dated markdown.
        fs::write(
            content.join("writing/old.md"),
            "---\ndate: \"2026-01-15\"\n---\nold writing\n",
        )
        .unwrap();
        fs::write(
            content.join("writing/new.md"),
            "---\ndate: \"2026-04-01\"\n---\nnew writing\n",
        )
        .unwrap();
        // Sidecar-dated binary.
        fs::write(content.join("papers/p.pdf"), b"pdf").unwrap();
        fs::write(
            content.join("papers/p.meta.json"),
            r#"{"date":"2026-03-10"}"#,
        )
        .unwrap();
        // Undated entries — must collapse to the bottom of canonical order.
        fs::write(content.join("misc/b.txt"), b"b").unwrap();
        fs::write(content.join("misc/a.txt"), b"a").unwrap();

        let ledger = generate_content_ledger(&root, Path::new("content")).unwrap();
        let order: Vec<&str> = ledger
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect();

        assert_eq!(
            order,
            vec![
                // Undated first (path-asc tiebreaker), then dated asc.
                "misc/a.txt",
                "misc/b.txt",
                "writing/old.md",
                "papers/p.pdf",
                "writing/new.md",
            ]
        );
        ledger.validate().unwrap();

        fs::remove_dir_all(root).unwrap();
    }
}
