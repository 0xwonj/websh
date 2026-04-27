use serde::{Deserialize, Serialize};

use super::filesystem::AccessFilter;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifestDocument {
    pub files: Vec<ContentManifestFile>,
    pub directories: Vec<ContentManifestDirectory>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifestFile {
    pub path: String,
    pub title: String,
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub date: Option<String>,
    pub tags: Vec<String>,
    pub access: Option<AccessFilter>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifestDirectory {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub thumbnail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_manifest_document_round_trips_existing_shape() {
        let body = include_str!("../../tests/fixtures/manifest_golden.json");
        let manifest: ContentManifestDocument = serde_json::from_str(body).expect("parse");
        let encoded = serde_json::to_string_pretty(&manifest).expect("serialize");
        assert_eq!(encoded.trim_end(), body.trim_end());
    }
}
