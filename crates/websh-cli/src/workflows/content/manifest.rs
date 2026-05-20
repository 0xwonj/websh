use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use websh_core::domain::{ContentManifestDocument, ContentManifestEntry};
use websh_core::ports::parse_manifest_snapshot;

use crate::CliResult;
use crate::infra::json::write_json;

use super::files::{
    CONTENT_MANIFEST_FILE, collect_files_recursive, relative_path_from, resolve_path,
    should_skip_content_file, should_skip_primary_content_file,
};
use super::modified_at::GitModifiedAt;
use super::sidecar::{
    default_directory_metadata, default_file_metadata, read_directory_sidecar, read_file_sidecar,
    sync_directory_sidecar, sync_file_sidecar,
};

pub(crate) const DEFAULT_CONTENT_DIR: &str = "content";

#[derive(Debug, Default)]
struct ModifiedAtIndex {
    files: BTreeMap<String, u64>,
    directories: BTreeMap<String, u64>,
}

/// Canonical entry point: walk the content tree, refresh every node's
/// sidecar (recompute `derived` fields and merge frontmatter into
/// `authored` for markdown files), then fold the sidecars into
/// `manifest.json`.
///
/// This is the only function callers should reach for when they need
/// the manifest to reflect the current on-disk content. The CLI's
/// `content manifest` subcommand and Trunk's pre-build hook both end
/// here. Internal callers that have *just* sync'd and only need to
/// re-fold the manifest after touching `.websh/ledger.json` may use
/// [`build_manifest_from_sidecars`] for the projection-only path.
pub(crate) fn sync_content(root: &Path, content_dir: &Path) -> CliResult<ContentManifestDocument> {
    let content_root = resolve_path(root, content_dir);
    fs::create_dir_all(&content_root)
        .with_context(|| format!("create directory {}", content_root.display()))?;

    let mut all_files = Vec::new();
    collect_files_recursive(&content_root, &mut all_files)?;
    let mut modified_at = GitModifiedAt::new(root)?;

    // First pass: refresh every primary file's sidecar.
    for file_path in &all_files {
        let rel_path = relative_path_from(&content_root, file_path)?;
        if should_skip_primary_content_file(&rel_path) {
            continue;
        }
        sync_file_sidecar(&content_root, file_path, &rel_path)?;
    }

    // Second pass: refresh directory sidecars.
    let directories = enumerate_directories_from_files(&content_root, &all_files)?;
    for dir_rel in &directories {
        sync_directory_sidecar(&content_root, dir_rel)?;
    }

    // Third pass: build manifest from current sidecars + the file list
    // we already have on hand.
    let modified_at = build_modified_at_index(&content_root, &all_files, &mut modified_at)?;
    bundle_manifest(&content_root, &all_files, &directories, &modified_at)
}

/// Internal-only: re-fold `manifest.json` from existing sidecars without
/// refreshing them. The caller is responsible for ensuring sidecars are
/// already current — this is intended for narrow situations like
/// "rewrote `.websh/ledger.json`, now re-bundle the manifest so the new
/// ledger hash propagates" where doing a full [`sync_content`] would be
/// wasted work.
///
/// Not exposed as a CLI subcommand: external invocations should always
/// go through `content manifest` (i.e. [`sync_content`]) so the manifest
/// is never ahead of the sidecars.
pub(crate) fn build_manifest_from_sidecars(
    root: &Path,
    content_dir: &Path,
) -> CliResult<ContentManifestDocument> {
    let content_root = resolve_path(root, content_dir);
    fs::create_dir_all(&content_root)
        .with_context(|| format!("create directory {}", content_root.display()))?;

    let mut all_files = Vec::new();
    collect_files_recursive(&content_root, &mut all_files)?;
    let directories = enumerate_directories_from_files(&content_root, &all_files)?;
    let mut modified_at = GitModifiedAt::new(root)?;
    let modified_at = build_modified_at_index(&content_root, &all_files, &mut modified_at)?;
    bundle_manifest(&content_root, &all_files, &directories, &modified_at)
}

/// Project current sidecars + filesystem state into a `manifest.json`
/// document. Pure projection — does not modify sidecars.
fn bundle_manifest(
    content_root: &Path,
    all_files: &[PathBuf],
    directories: &[String],
    modified_at: &ModifiedAtIndex,
) -> CliResult<ContentManifestDocument> {
    let mut entries = Vec::new();

    // Directory entries first (canonical order).
    for dir_rel in directories {
        let mut metadata = read_directory_sidecar(content_root, dir_rel)?
            .unwrap_or_else(|| default_directory_metadata(dir_rel));
        apply_manifest_modified_at(&mut metadata, modified_at.directories.get(dir_rel).copied());
        entries.push(ContentManifestEntry {
            path: dir_rel.clone(),
            metadata,
            mempool: None,
        });
    }

    // File entries. The manifest includes `.websh/*.json` artifacts (e.g.
    // ledger.json, attestations.json) so signed/derived data is reachable
    // through the same surface; only sidecars/manifest themselves are
    // skipped.
    let mut file_entries = Vec::new();
    for file_path in all_files {
        let rel_path = relative_path_from(content_root, file_path)?;
        if should_skip_content_file(&rel_path) {
            continue;
        }
        let mut metadata = read_file_sidecar(content_root, &rel_path)?
            .unwrap_or_else(|| default_file_metadata(file_path, &rel_path));
        apply_manifest_modified_at(&mut metadata, modified_at.files.get(&rel_path).copied());
        file_entries.push(ContentManifestEntry {
            path: rel_path,
            metadata,
            mempool: None,
        });
    }
    file_entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries.extend(file_entries);

    let manifest = ContentManifestDocument { entries };
    validate_manifest(&manifest)?;
    write_json(&content_root.join(CONTENT_MANIFEST_FILE), &manifest)?;
    Ok(manifest)
}

fn apply_manifest_modified_at(metadata: &mut websh_core::domain::NodeMetadata, value: Option<u64>) {
    metadata.derived.modified_at = value;
}

fn validate_manifest(manifest: &ContentManifestDocument) -> CliResult {
    let body = serde_json::to_string(manifest).context("serialize manifest for validation")?;
    parse_manifest_snapshot(&body)?;
    Ok(())
}

fn content_parent_dirs(rel_path: &str) -> Vec<String> {
    let mut parts: Vec<&str> = rel_path.split('/').collect();
    parts.pop();
    let mut out = Vec::new();
    while !parts.is_empty() {
        out.push(parts.join("/"));
        parts.pop();
    }
    out
}

fn build_modified_at_index(
    content_root: &Path,
    all_files: &[PathBuf],
    modified_at: &mut GitModifiedAt,
) -> CliResult<ModifiedAtIndex> {
    let mut index = ModifiedAtIndex::default();
    for file_path in all_files {
        let rel_path = relative_path_from(content_root, file_path)?;
        if should_skip_primary_content_file(&rel_path) {
            continue;
        }
        if let Some(timestamp) = modified_at.timestamp_for_path(file_path)? {
            index.files.insert(rel_path.clone(), timestamp);
            propagate_file_modified_at(&mut index.directories, &rel_path, timestamp);
        }
    }
    Ok(index)
}

fn propagate_file_modified_at(
    directory_modified_at: &mut BTreeMap<String, u64>,
    rel_path: &str,
    timestamp: u64,
) {
    let parent = Path::new(rel_path)
        .parent()
        .map(slash_path)
        .unwrap_or_default();
    propagate_directory_modified_at(directory_modified_at, &parent, timestamp);
}

fn propagate_directory_modified_at(
    directory_modified_at: &mut BTreeMap<String, u64>,
    dir_rel: &str,
    timestamp: u64,
) {
    let mut current = dir_rel.to_string();
    loop {
        directory_modified_at
            .entry(current.clone())
            .and_modify(|existing| *existing = (*existing).max(timestamp))
            .or_insert(timestamp);
        if current.is_empty() {
            break;
        }
        current = Path::new(&current)
            .parent()
            .map(slash_path)
            .unwrap_or_default();
    }
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Build the sorted directory list from a pre-walked file list. Caller
/// passes `all_files` so the tree isn't walked twice during sync.
fn enumerate_directories_from_files(
    content_root: &Path,
    all_files: &[PathBuf],
) -> CliResult<Vec<String>> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    seen.insert(String::new());
    for file in all_files {
        let rel = relative_path_from(content_root, file)?;
        for parent in content_parent_dirs(&rel) {
            seen.insert(parent);
        }
    }
    Ok(seen.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use websh_core::domain::{
        AccessFilter, Fields, NodeKind, NodeMetadata, Recipient, SCHEMA_VERSION,
    };
    use websh_core::filesystem::RouteCatalogError;

    fn tempdir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut d = std::env::temp_dir();
        d.push(format!("websh-manifest-test-{}-{}", std::process::id(), id));
        if d.exists() {
            fs::remove_dir_all(&d).unwrap();
        }
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn read_sidecar(path: &Path) -> NodeMetadata {
        let body = fs::read_to_string(path).expect("sidecar exists");
        serde_json::from_str(&body).expect("sidecar parses")
    }

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_git(dir: &Path) {
        git(dir, &["-c", "init.defaultBranch=main", "init"]);
    }

    fn git_add_commit_at(dir: &Path, timestamp: u64, message: &str) {
        git(dir, &["add", "."]);
        let date = format!("@{timestamp} +0000");
        let output = Command::new("git")
            .current_dir(dir)
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .args([
                "-c",
                "user.name=websh test",
                "-c",
                "user.email=websh-test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                message,
            ])
            .output()
            .expect("run git commit");
        assert!(
            output.status.success(),
            "git commit failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn populates_authored_from_frontmatter() {
        let dir = tempdir();
        fs::write(
            dir.join("hello.md"),
            "---\ntitle: Greeting\ntags:\n  - intro\n  - sample\ndate: 2026-04-22\n---\n\nbody\n",
        )
        .unwrap();

        let manifest = sync_content(&dir, Path::new(".")).expect("sync ok");

        let sidecar = read_sidecar(&dir.join("hello.meta.json"));
        assert_eq!(sidecar.authored.title.as_deref(), Some("Greeting"));
        assert_eq!(sidecar.authored.date.as_deref(), Some("2026-04-22"));
        assert_eq!(
            sidecar.authored.tags.as_deref(),
            Some(&["intro".to_string(), "sample".to_string()][..]),
        );

        let entry = manifest
            .entries
            .iter()
            .find(|e| e.path == "hello.md")
            .expect("hello.md in manifest");
        assert_eq!(entry.metadata.authored.title.as_deref(), Some("Greeting"));
        assert_eq!(entry.metadata.kind, NodeKind::Page);
    }

    #[test]
    fn populates_git_modified_at_for_files_and_parent_directories() {
        let dir = tempdir();
        init_git(&dir);
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::write(dir.join("docs/readme.md"), "hello").unwrap();
        git_add_commit_at(&dir, 1_700_000_000, "add readme");

        let manifest = sync_content(&dir, Path::new(".")).expect("sync ok");

        let file_sidecar = read_sidecar(&dir.join("docs/readme.meta.json"));
        assert_eq!(file_sidecar.derived.modified_at, None);

        let directory_sidecar = read_sidecar(&dir.join("docs/_index.dir.json"));
        assert_eq!(directory_sidecar.derived.modified_at, None);

        let root_sidecar = read_sidecar(&dir.join("_index.dir.json"));
        assert_eq!(root_sidecar.derived.modified_at, None);

        let entry = manifest
            .entries
            .iter()
            .find(|entry| entry.path == "docs/readme.md")
            .expect("manifest entry");
        assert_eq!(entry.metadata.modified_at(), Some(1_700_000_000));
        let directory = manifest
            .entries
            .iter()
            .find(|entry| entry.path == "docs")
            .expect("directory manifest entry");
        assert_eq!(directory.metadata.modified_at(), Some(1_700_000_000));
        let root = manifest
            .entries
            .iter()
            .find(|entry| entry.path.is_empty())
            .expect("root manifest entry");
        assert_eq!(root.metadata.modified_at(), Some(1_700_000_000));
    }

    #[test]
    fn generated_sidecar_commit_does_not_drive_directory_modified_at() {
        let dir = tempdir();
        init_git(&dir);
        fs::create_dir_all(dir.join("docs")).unwrap();
        fs::write(dir.join("docs/readme.md"), "hello").unwrap();
        git_add_commit_at(&dir, 1_700_000_000, "add readme");

        fs::write(
            dir.join("docs/_index.dir.json"),
            r#"{
              "schema":1,
              "kind":"directory",
              "authored":{"title":"Docs"},
              "derived":{"kind":"directory"}
            }"#,
        )
        .unwrap();
        git_add_commit_at(&dir, 1_700_000_100, "describe docs");

        let manifest = sync_content(&dir, Path::new(".")).expect("sync ok");

        let directory_sidecar = read_sidecar(&dir.join("docs/_index.dir.json"));
        assert_eq!(directory_sidecar.derived.modified_at, None);

        let root_sidecar = read_sidecar(&dir.join("_index.dir.json"));
        assert_eq!(root_sidecar.derived.modified_at, None);

        let directory = manifest
            .entries
            .iter()
            .find(|entry| entry.path == "docs")
            .expect("directory manifest entry");
        assert_eq!(directory.metadata.modified_at(), Some(1_700_000_000));
        let root = manifest
            .entries
            .iter()
            .find(|entry| entry.path.is_empty())
            .expect("root manifest entry");
        assert_eq!(root.metadata.modified_at(), Some(1_700_000_000));
    }

    #[test]
    fn untracked_content_omits_modified_at() {
        let dir = tempdir();
        init_git(&dir);
        fs::write(dir.join("draft.md"), "untracked").unwrap();

        let manifest = sync_content(&dir, Path::new(".")).expect("sync ok");

        let file_sidecar = read_sidecar(&dir.join("draft.meta.json"));
        assert_eq!(file_sidecar.derived.modified_at, None);

        let root_sidecar = read_sidecar(&dir.join("_index.dir.json"));
        assert_eq!(root_sidecar.derived.modified_at, None);
        let entry = manifest
            .entries
            .iter()
            .find(|entry| entry.path == "draft.md")
            .expect("manifest entry");
        assert_eq!(entry.metadata.modified_at(), None);
    }

    #[test]
    fn idempotent_across_repeated_runs() {
        let dir = tempdir();
        fs::write(dir.join("note.md"), "---\ntitle: Note\n---\n\ncontent\n").unwrap();

        sync_content(&dir, Path::new(".")).expect("first sync");
        let bytes_a = fs::read(dir.join("manifest.json")).unwrap();
        let sidecar_a = fs::read(dir.join("note.meta.json")).unwrap();

        sync_content(&dir, Path::new(".")).expect("second sync");
        let bytes_b = fs::read(dir.join("manifest.json")).unwrap();
        let sidecar_b = fs::read(dir.join("note.meta.json")).unwrap();

        assert_eq!(bytes_a, bytes_b, "manifest must be byte-equal across syncs");
        assert_eq!(
            sidecar_a, sidecar_b,
            "sidecar must be byte-equal across syncs"
        );
    }

    #[test]
    fn preserves_sidecar_only_authored_fields() {
        let dir = tempdir();

        // Pre-existing sidecar carries an `access` recipient list — the
        // sort of field a user authors directly in the JSON, not via
        // markdown frontmatter. Sync must not clobber it.
        let prior = NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            bundle: None,
            authored: Fields {
                access: Some(AccessFilter {
                    recipients: vec![Recipient {
                        address: "0xabc".to_string(),
                    }],
                }),
                ..Fields::default()
            },
            derived: Fields::default(),
        };
        fs::write(
            dir.join("scoped.meta.json"),
            format!("{}\n", serde_json::to_string_pretty(&prior).unwrap()),
        )
        .unwrap();

        // Frontmatter sets `title` only — no `access` key.
        fs::write(dir.join("scoped.md"), "---\ntitle: Scoped\n---\n\nbody\n").unwrap();

        sync_content(&dir, Path::new(".")).expect("sync ok");

        let after = read_sidecar(&dir.join("scoped.meta.json"));
        assert_eq!(after.authored.title.as_deref(), Some("Scoped"));
        let access = after.authored.access.expect("access preserved");
        assert_eq!(access.recipients.len(), 1);
        assert_eq!(access.recipients[0].address, "0xabc");
    }

    #[test]
    fn rejects_bundle_route_collisions_during_manifest_sync() {
        let dir = tempdir();
        fs::create_dir_all(dir.join("writing/foo")).unwrap();
        fs::write(
            dir.join("writing/foo/_index.dir.json"),
            r#"{
              "schema":1,
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
        fs::write(dir.join("writing/foo/en.md"), b"english").unwrap();
        fs::write(dir.join("writing/foo.md"), b"collision").unwrap();

        let err = sync_content(&dir, Path::new(".")).unwrap_err();
        let manifest_error = err
            .downcast_ref::<websh_core::ports::ManifestSnapshotError>()
            .expect("bundle route collision error");
        assert!(matches!(
            manifest_error,
            websh_core::ports::ManifestSnapshotError::RouteCatalog(
                RouteCatalogError::RouteCollision { route, .. }
            )
                if route == "/writing/foo"
        ));
    }

    #[test]
    fn sync_creates_directory_sidecar_for_dot_site_root() {
        let dir = tempdir();
        fs::create_dir_all(dir.join(".site")).unwrap();
        fs::write(dir.join(".site/now.toml"), b"[[items]]\n").unwrap();

        sync_content(&dir, Path::new(".")).expect("sync ok");

        let sidecar = read_sidecar(&dir.join(".site/_index.dir.json"));
        assert_eq!(sidecar.kind, NodeKind::Directory);
        assert_eq!(sidecar.derived.kind, Some(NodeKind::Directory));
    }

    #[test]
    fn sync_rejects_legacy_site_sidecar_for_dot_site_root() {
        let dir = tempdir();
        fs::create_dir_all(dir.join(".site")).unwrap();
        fs::write(
            dir.join(".site/_index.dir.json"),
            r#"{"schema":1,"kind":"site","authored":{"title":"Site"},"derived":{"kind":"site"}}"#,
        )
        .unwrap();
        fs::write(dir.join(".site/now.toml"), b"[[items]]\n").unwrap();

        let err = sync_content(&dir, Path::new(".")).unwrap_err();
        assert!(err.to_string().contains("parse"));
    }
}
