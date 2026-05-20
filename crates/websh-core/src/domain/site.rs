//! Filesystem-first site metadata models.
//!
//! Mount declarations. The unified node-level metadata model lives in
//! [`super::metadata`].

use serde::{Deserialize, Serialize};

/// Filesystem-declared mount definition loaded after bootstrap.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountDeclaration {
    pub backend: String,
    pub mount_at: String,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub root: Option<String>,
    pub gateway: Option<String>,
    pub name: Option<String>,
    pub writable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_declaration_parses_expected_shape() {
        let decl: MountDeclaration = serde_json::from_str(
            r#"{
                "backend": "github",
                "mount_at": "/db",
                "repo": "0xwonj/db",
                "branch": "main",
                "writable": true
            }"#,
        )
        .unwrap();

        assert_eq!(decl.backend, "github");
        assert_eq!(decl.mount_at, "/db");
        assert_eq!(decl.repo.as_deref(), Some("0xwonj/db"));
        assert_eq!(decl.branch.as_deref(), Some("main"));
        assert!(decl.writable);
    }
}
