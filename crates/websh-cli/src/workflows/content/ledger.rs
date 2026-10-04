use std::fs;
use std::path::Path;

use anyhow::Context;
use websh_core::attestation::artifact::subject_id_for_route;
use websh_core::attestation::ledger::{
    CONTENT_LEDGER_CONTENT_PATH, ContentLedger, ContentLedgerCategory, ContentLedgerEntry,
    ContentLedgerInput, ContentLedgerSortKey,
};
use websh_core::support::format::iso_date_prefix;

use super::files::{
    BundleContentUnit, DirectoryContentUnit, build_content_files, collect_files_recursive,
    discover_bundle_content_units, discover_directory_content_units, path_is_inside_bundle,
    path_is_inside_directory_unit, relative_path_from, resolve_path, route_for_content_path,
    should_skip_primary_content_file,
};
use super::frontmatter::content_entry_raw_date;
use super::sidecar::matching_file_sidecar;
use crate::CliResult;
use crate::infra::json::write_json;

pub(crate) fn generate_content_ledger(root: &Path, content_dir: &Path) -> CliResult<ContentLedger> {
    let content_root = resolve_path(root, content_dir);
    fs::create_dir_all(&content_root)
        .with_context(|| format!("create directory {}", content_root.display()))?;

    let mut files = Vec::new();
    collect_files_recursive(&content_root, &mut files)?;

    let mut staged: Vec<ContentLedgerInput> = Vec::new();
    let bundles = discover_bundle_content_units(&content_root, &files)?;
    let directories = discover_directory_content_units(&content_root, &files, &bundles)?;
    for directory in &directories {
        let route = route_for_content_path(&directory.rel_path);
        let content_files = build_content_files(root, &directory.content_paths)?;
        let sort_date = sort_date_for_directory(directory);
        staged.push(ContentLedgerInput::new(
            ContentLedgerSortKey::new(sort_date, directory.rel_path.clone()),
            ContentLedgerEntry::new(
                subject_id_for_route(&route),
                route,
                directory.rel_path.clone(),
                ContentLedgerCategory::for_path(&directory.rel_path),
                content_files,
            )?,
        ));
    }

    for bundle in &bundles {
        let route = route_for_content_path(&bundle.rel_path);
        let content_files = build_content_files(root, &bundle.content_paths)?;
        let sort_date = sort_date_for_bundle(bundle);
        let category = ContentLedgerCategory::for_path(&bundle.rel_path);
        staged.push(ContentLedgerInput::new(
            ContentLedgerSortKey::new(sort_date, bundle.rel_path.clone()),
            ContentLedgerEntry::new(
                subject_id_for_route(&route),
                route,
                bundle.rel_path.clone(),
                category,
                content_files,
            )?,
        ));
    }

    for file_path in files {
        let rel_path = relative_path_from(&content_root, &file_path)?;
        if should_skip_primary_content_file(&rel_path) {
            continue;
        }
        if path_is_inside_directory_unit(&rel_path, &directories) {
            continue;
        }
        if path_is_inside_bundle(&rel_path, &bundles) {
            continue;
        }

        let mut content_paths = vec![file_path.clone()];
        if let Some(sidecar) = matching_file_sidecar(&content_root, &rel_path) {
            content_paths.push(sidecar);
        }

        let route = route_for_content_path(&rel_path);
        let content_files = build_content_files(root, &content_paths)?;
        let sort_date = sort_date_for_entry(&content_root, &file_path, &rel_path);
        let category = ContentLedgerCategory::for_path(&rel_path);
        staged.push(ContentLedgerInput::new(
            ContentLedgerSortKey::new(sort_date, rel_path.clone()),
            ContentLedgerEntry::new(
                subject_id_for_route(&route),
                route,
                rel_path,
                category,
                content_files,
            )?,
        ));
    }
    // `ContentLedger::new` owns canonical block ordering and hash chaining:
    // `(sort_key.date asc with None first, sort_key.path asc)`.
    let ledger = ContentLedger::new(staged)?;
    ledger.validate()?;

    let ledger_path = content_root.join(CONTENT_LEDGER_CONTENT_PATH);
    if let Some(parent) = ledger_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create directory {}", parent.display()))?;
    }
    write_json(&ledger_path, &ledger)?;

    Ok(ledger)
}

fn sort_date_for_entry(content_root: &Path, file_path: &Path, rel_path: &str) -> Option<String> {
    content_entry_raw_date(content_root, file_path, rel_path)
        .as_deref()
        .and_then(iso_date_prefix)
        .map(|date| date.to_string())
}

fn sort_date_for_bundle(bundle: &BundleContentUnit) -> Option<String> {
    bundle
        .metadata
        .date()
        .and_then(iso_date_prefix)
        .map(str::to_string)
}

fn sort_date_for_directory(directory: &DirectoryContentUnit) -> Option<String> {
    directory
        .metadata
        .date()
        .and_then(iso_date_prefix)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;
    use websh_core::filesystem::RouteCatalogError;

    #[test]
    fn ledger_groups_sidecars_and_excludes_generated_files() {
        let root = temp_dir("sidecar");
        let content = root.join("content");
        fs::create_dir_all(content.join("talks")).unwrap();
        fs::create_dir_all(content.join(".websh")).unwrap();
        fs::create_dir_all(content.join(".websh/errors")).unwrap();
        fs::write(content.join("manifest.json"), "{}").unwrap();
        fs::write(content.join(".websh/old.json"), "{}").unwrap();
        fs::write(content.join(".websh/errors/404.md"), b"not found").unwrap();
        fs::write(content.join("talks/a.pdf"), b"pdf").unwrap();
        fs::write(
            content.join("talks/a.meta.json"),
            r#"{"kind":"document","authored":{"title":"Talk","tags":["zk"],"date":"2026-04-01"},"derived":{}}"#,
        )
        .unwrap();

        let ledger = generate_content_ledger(&root, Path::new("content")).unwrap();
        assert_eq!(ledger.blocks.len(), 1);
        let entry = &ledger.blocks[0].entry;
        assert_eq!(entry.path, "talks/a.pdf");
        assert_eq!(entry.route, "/talks/a.pdf");
        assert_eq!(entry.category, ContentLedgerCategory::Talks);
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
        assert!(!encoded.contains(".websh/errors/404"));
    }

    #[test]
    fn ledger_sorts_entries_by_date_with_path_tiebreaker() {
        let root = temp_dir("date-sort");
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
            r#"{"kind":"document","authored":{"date":"2026-03-10"},"derived":{}}"#,
        )
        .unwrap();
        // Undated entries have `None` sort dates, so they sort first with
        // the path as a tiebreaker.
        fs::write(content.join("misc/b.txt"), b"b").unwrap();
        fs::write(content.join("misc/a.txt"), b"a").unwrap();

        let ledger = generate_content_ledger(&root, Path::new("content")).unwrap();
        let order: Vec<&str> = ledger
            .blocks
            .iter()
            .map(|block| block.entry.path.as_str())
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
        let heights = ledger
            .blocks
            .iter()
            .map(|block| block.height)
            .collect::<Vec<_>>();
        assert_eq!(heights, vec![1, 2, 3, 4, 5]);
        assert_eq!(
            ledger.blocks[0].prev_block_sha256, ledger.genesis_hash,
            "first block points to genesis"
        );
        for pair in ledger.blocks.windows(2) {
            assert_eq!(pair[1].prev_block_sha256, pair[0].block_sha256);
        }
        assert_eq!(
            ledger.chain_head,
            ledger.blocks.last().unwrap().block_sha256
        );
        ledger.validate().unwrap();
    }

    #[test]
    fn ledger_groups_bundle_support_assets_without_standalone_blocks() {
        let root = temp_dir("bundle-assets");
        let content = root.join("content");
        fs::create_dir_all(content.join("writing/foo")).unwrap();
        fs::write(
            content.join("writing/foo/_index.dir.json"),
            r#"{
              "kind":"bundle",
              "bundle":{
                "default_variant":{"strategy":"static","id":"en"},
                "variants":[
                  {"id":"en","path":"en.md","label":"English"},
                  {"id":"ko","path":"ko.md","label":"Korean"}
                ]
              },
              "authored":{"title":"Foo","date":"2026-05-15"},
              "derived":{"kind":"bundle"}
            }"#,
        )
        .unwrap();
        fs::write(content.join("writing/foo/en.md"), b"english").unwrap();
        fs::write(content.join("writing/foo/en.meta.json"), b"{}").unwrap();
        fs::write(content.join("writing/foo/ko.md"), b"korean").unwrap();
        fs::write(content.join("writing/foo/cover.png"), b"png").unwrap();
        fs::write(
            content.join("writing/foo/cover.meta.json"),
            b"{\"authored\":{\"title\":\"Cover\"}}",
        )
        .unwrap();

        let ledger = generate_content_ledger(&root, Path::new("content")).unwrap();

        assert_eq!(ledger.blocks.len(), 1);
        let entry = &ledger.blocks[0].entry;
        assert_eq!(entry.path, "writing/foo");
        assert_eq!(entry.route, "/writing/foo");
        let paths = entry
            .content_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"content/writing/foo/_index.dir.json"));
        assert!(paths.contains(&"content/writing/foo/en.md"));
        assert!(paths.contains(&"content/writing/foo/en.meta.json"));
        assert!(paths.contains(&"content/writing/foo/ko.md"));
        assert!(paths.contains(&"content/writing/foo/cover.png"));
        assert!(paths.contains(&"content/writing/foo/cover.meta.json"));
    }

    #[test]
    fn ledger_groups_authored_directory_into_one_directory_block() {
        let root = temp_dir("authored-directory");
        let content = root.join("content");
        fs::create_dir_all(content.join(".site/errors")).unwrap();
        fs::create_dir_all(content.join(".site/keys")).unwrap();
        fs::create_dir_all(content.join(".websh/errors")).unwrap();
        fs::write(
            content.join(".site/_index.dir.json"),
            r#"{"kind":"directory","authored":{"title":"Site support","date":"2026-05-01"},"derived":{"kind":"directory"}}"#,
        )
        .unwrap();
        fs::write(content.join(".site/now.toml"), b"[[items]]\n").unwrap();
        fs::write(content.join(".site/now.meta.json"), b"{}").unwrap();
        fs::write(content.join(".site/keys/wonjae.asc"), b"key").unwrap();
        fs::write(content.join(".site/errors/404.md"), b"not found").unwrap();
        fs::write(content.join(".site/errors/empty.md"), b"").unwrap();
        fs::write(content.join(".site/.DS_Store"), b"local").unwrap();
        fs::write(content.join(".site/keys/.gitkeep"), b"").unwrap();
        fs::write(content.join(".websh/errors/404.md"), b"websh error").unwrap();

        let ledger = generate_content_ledger(&root, Path::new("content")).unwrap();

        assert_eq!(ledger.blocks.len(), 1);
        let entry = &ledger.blocks[0].entry;
        assert_eq!(entry.path, ".site");
        assert_eq!(entry.route, "/.site");
        assert_eq!(entry.category, ContentLedgerCategory::Misc);
        let paths = entry
            .content_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"content/.site/_index.dir.json"));
        assert!(paths.contains(&"content/.site/now.meta.json"));
        assert!(paths.contains(&"content/.site/now.toml"));
        assert!(paths.contains(&"content/.site/keys/wonjae.asc"));
        assert!(paths.contains(&"content/.site/errors/404.md"));
        assert!(paths.contains(&"content/.site/errors/empty.md"));
        assert!(!paths.contains(&"content/.site/.DS_Store"));
        assert!(!paths.contains(&"content/.site/keys/.gitkeep"));
        assert!(!paths.contains(&"content/.websh/errors/404.md"));
        let empty = entry
            .content_files
            .iter()
            .find(|file| file.path == "content/.site/errors/empty.md")
            .expect("zero-byte directory file is signed");
        assert_eq!(empty.bytes, 0);
        assert!(
            !ledger
                .blocks
                .iter()
                .any(|block| block.entry.path == ".site/now.toml")
        );
        ledger.validate().unwrap();
    }

    #[test]
    fn ledger_does_not_group_directory_without_authored_metadata() {
        let root = temp_dir("directory-empty-authored");
        let content = root.join("content");
        fs::create_dir_all(content.join("writing")).unwrap();
        fs::write(
            content.join("writing/_index.dir.json"),
            r#"{"kind":"directory","authored":{},"derived":{"kind":"directory"}}"#,
        )
        .unwrap();
        fs::write(content.join("writing/hello.md"), b"hello").unwrap();

        let ledger = generate_content_ledger(&root, Path::new("content")).unwrap();
        assert!(
            ledger
                .blocks
                .iter()
                .any(|block| block.entry.path == "writing/hello.md")
        );
        assert!(
            !ledger
                .blocks
                .iter()
                .any(|block| block.entry.path == "writing")
        );
    }

    #[test]
    fn ledger_rejects_bundle_root_route_collision() {
        let root = temp_dir("bundle-collision");
        let content = root.join("content");
        fs::create_dir_all(content.join("writing/foo")).unwrap();
        fs::write(
            content.join("writing/foo/_index.dir.json"),
            r#"{
              "kind":"bundle",
              "bundle":{
                "default_variant":{"strategy":"static","id":"en"},
                "variants":[{"id":"en","path":"en.md","label":"English"}]
              },
              "authored":{"title":"Foo"},
              "derived":{"kind":"bundle"}
            }"#,
        )
        .unwrap();
        fs::write(content.join("writing/foo/en.md"), b"english").unwrap();
        fs::write(content.join("writing/foo.md"), b"collision").unwrap();

        let err = generate_content_ledger(&root, Path::new("content")).unwrap_err();
        let route_error = err
            .downcast_ref::<RouteCatalogError>()
            .expect("route catalog collision error");
        assert!(matches!(
            route_error,
            RouteCatalogError::RouteCollision { route, .. } if route == "/writing/foo"
        ));
    }
}
