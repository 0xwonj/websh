use crate::render::theme;
use crate::runtime::EnvironmentError;

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("unknown theme `{raw}`. available: {available}")]
    Unknown { raw: String, available: String },
    #[error("failed to persist {theme_id}: {source}")]
    Persist {
        theme_id: &'static str,
        #[source]
        source: EnvironmentError,
    },
}

impl ThemeError {
    pub fn unknown(raw: &str) -> Self {
        Self::Unknown {
            raw: raw.to_string(),
            available: theme::theme_ids().collect::<Vec<_>>().join(", "),
        }
    }
}
