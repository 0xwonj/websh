//! Byte-stable round-trip test for the manifest serializer.
//!
//! The invariant under test: `manifest.json → VirtualFs → serialize_manifest →
//! pretty JSON` must produce *the same bytes* as the input (modulo trailing
//! whitespace). Any divergence — order drift, lost fields, injected synthetic
//! entries — breaks the round-trip and must fail loudly.
//!
//! See the plan at `docs/superpowers/plans/2026-04-20-phase3a-write-direct-commit.md`
//! Task 1.8 for the bootstrap procedure.

use websh::core::VirtualFs;
use websh::models::Manifest;

#[test]
fn manifest_roundtrip_is_byte_stable() {
    let golden = include_str!("fixtures/manifest_golden.json");
    let manifest: Manifest = serde_json::from_str(golden).expect("golden parses");

    let fs = VirtualFs::from_manifest(&manifest);
    let reserialized = fs.serialize_manifest();
    let out = serde_json::to_string_pretty(&reserialized).expect("serialize");

    // Trim trailing whitespace to tolerate editor newlines at EOF.
    assert_eq!(out.trim_end(), golden.trim_end());
}

#[test]
fn serialize_manifest_sorts_regardless_of_input_order() {
    // Independent of the golden fixture: construct a manifest with files
    // deliberately in reverse lexicographic order, round-trip it, and assert
    // the output is sorted. This protects the sort invariant even if someone
    // later re-sorts the golden fixture in the source tree.
    use websh::models::{DirectoryEntry, FileEntry};

    let manifest = Manifest {
        files: vec![
            FileEntry {
                path: "z.md".to_string(),
                title: "Z".to_string(),
                size: None,
                modified: None,
                tags: vec![],
                access: None,
            },
            FileEntry {
                path: "m.md".to_string(),
                title: "M".to_string(),
                size: None,
                modified: None,
                tags: vec![],
                access: None,
            },
            FileEntry {
                path: "a.md".to_string(),
                title: "A".to_string(),
                size: None,
                modified: None,
                tags: vec![],
                access: None,
            },
        ],
        directories: vec![
            DirectoryEntry {
                path: "z-dir".to_string(),
                title: "Z".to_string(),
                tags: vec!["zone".to_string()],
                description: None,
                icon: None,
                thumbnail: None,
            },
            DirectoryEntry {
                path: "a-dir".to_string(),
                title: "A".to_string(),
                tags: vec!["area".to_string()],
                description: None,
                icon: None,
                thumbnail: None,
            },
        ],
    };

    let fs = VirtualFs::from_manifest(&manifest);
    let out = fs.serialize_manifest();
    let file_paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(file_paths, vec!["a.md", "m.md", "z.md"]);
    let dir_paths: Vec<&str> = out.directories.iter().map(|d| d.path.as_str()).collect();
    assert_eq!(dir_paths, vec!["a-dir", "z-dir"]);
}

#[test]
fn serialize_manifest_omits_synthetic_dotprofile() {
    // Fresh `VirtualFs::empty()` injects a synthetic `.profile` file with no
    // manifest origin (content_path == None). It must not leak into the
    // serialized manifest on round-trip.
    let fs = VirtualFs::empty();
    let manifest = fs.serialize_manifest();
    assert!(
        manifest.files.iter().all(|f| f.path != ".profile"),
        "serialized manifest must not emit synthetic `.profile`; got: {:?}",
        manifest.files.iter().map(|f| &f.path).collect::<Vec<_>>()
    );

    // Also check from_manifest preserves that invariant for populated fses.
    let with_content = Manifest {
        files: vec![websh::models::FileEntry {
            path: "notes.md".to_string(),
            title: "Notes".to_string(),
            size: None,
            modified: None,
            tags: vec![],
            access: None,
        }],
        directories: vec![],
    };
    let fs2 = VirtualFs::from_manifest(&with_content);
    let serialized = fs2.serialize_manifest();
    assert!(
        serialized.files.iter().all(|f| f.path != ".profile"),
        "populated fs must not emit synthetic `.profile`"
    );
    assert_eq!(serialized.files.len(), 1);
    assert_eq!(serialized.files[0].path, "notes.md");
}
