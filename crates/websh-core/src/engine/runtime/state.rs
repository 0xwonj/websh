use std::collections::BTreeMap;

/// Target-provided runtime state rendered into the synthetic `/.websh/state`
/// view. Core treats this as a pure snapshot; browser/native runtimes own
/// persistence and mutation.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct RuntimeStateSnapshot {
    pub env: BTreeMap<String, String>,
    pub github_token_present: bool,
    pub wallet_session: bool,
}
