use super::{Manifest, ReleaseError};
use crate::domain::{ContentManifestEntry, NodeKind};

/// A publication owns its node, sidecar, and descendants, excluding independently
/// declared publications. Both page proofs and ledger blocks use this boundary.
pub(super) fn publication_entries<'a>(
    manifest: &'a Manifest,
    path: &str,
) -> Result<Vec<&'a ContentManifestEntry>, ReleaseError> {
    let release = manifest
        .release
        .as_ref()
        .ok_or_else(|| ReleaseError::Invalid("missing root release".into()))?;
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| ReleaseError::Invalid(format!("publication is not indexed: {path}")))?;
    let prefix = format!("{path}/");
    let sidecar = format!("{path}.meta.json");
    let mut entries: Vec<_> = manifest
        .entries
        .iter()
        .filter(|file| {
            if file.path == path || file.path == sidecar {
                return true;
            }
            if !entry.metadata.kind.is_directory_like() || !file.path.starts_with(&prefix) {
                return false;
            }
            entry.metadata.kind == NodeKind::Bundle
                || !release.publications.iter().any(|other| {
                    other != path
                        && other.starts_with(&prefix)
                        && (file.path == *other
                            || file.path == format!("{other}.meta.json")
                            || file.path.starts_with(&format!("{other}/")))
                })
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NodeMetadata, ReleaseMetadata};

    #[test]
    fn units_include_sidecars_and_exclude_independent_nested_publications() {
        let manifest = Manifest {
            entries: [
                ("writing/group", NodeKind::Directory),
                ("writing/group/note.md", NodeKind::Page),
                ("writing/group/paper.pdf", NodeKind::Document),
                ("writing/group/paper.pdf.meta.json", NodeKind::Data),
                ("writing/group/bundle", NodeKind::Bundle),
                ("writing/group/bundle/en.md", NodeKind::Page),
            ]
            .into_iter()
            .map(|(path, kind)| ContentManifestEntry {
                path: path.into(),
                metadata: NodeMetadata {
                    kind,
                    ..Default::default()
                },
                mempool: None,
            })
            .collect(),
            release: Some(ReleaseMetadata {
                purpose: super::super::CONTENT_PURPOSE.into(),
                site: "fixture".into(),
                sequence: 1,
                issued_at: 1,
                home: super::super::tests::home(),
                mounts: vec![],
                attestations: Default::default(),
                publications: [
                    "writing/group",
                    "writing/group/paper.pdf",
                    "writing/group/bundle",
                ]
                .map(str::to_owned)
                .to_vec(),
            }),
        };
        for (path, expected) in [
            (
                "writing/group",
                vec!["writing/group", "writing/group/note.md"],
            ),
            (
                "writing/group/paper.pdf",
                vec![
                    "writing/group/paper.pdf",
                    "writing/group/paper.pdf.meta.json",
                ],
            ),
            (
                "writing/group/bundle",
                vec!["writing/group/bundle", "writing/group/bundle/en.md"],
            ),
        ] {
            assert_eq!(
                publication_entries(&manifest, path)
                    .unwrap()
                    .iter()
                    .map(|e| e.path.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
