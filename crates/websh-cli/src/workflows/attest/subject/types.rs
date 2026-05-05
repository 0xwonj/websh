use std::path::PathBuf;

use crate::CliResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::workflows::attest) enum SubjectKind {
    Homepage,
    Ledger,
    Document,
    Page,
}

impl SubjectKind {
    pub(super) fn parse(value: &str) -> CliResult<Self> {
        match value {
            "homepage" => Ok(Self::Homepage),
            "ledger" => Ok(Self::Ledger),
            "document" => Ok(Self::Document),
            "page" => Ok(Self::Page),
            other => Err(format!("unsupported subject kind: {other}").into()),
        }
    }
}

#[derive(Clone)]
pub(in crate::workflows::attest) struct SubjectSpec {
    pub(in crate::workflows::attest) route: String,
    pub(in crate::workflows::attest) kind: SubjectKind,
    pub(in crate::workflows::attest) content_paths: Vec<PathBuf>,
}
