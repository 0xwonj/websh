//! Hash-based routing for IPFS-compatible navigation

/// Application routes for hash-based navigation (IPFS-compatible)
/// URL format: #/path/to/file.ext (e.g., #/blog/hello-world.md, #/papers/research.pdf)
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// Home/terminal view: #/ or empty hash
    Home,
    /// Reading content: #/path/to/file.ext (includes extension)
    Read {
        /// Full path with extension (e.g., "blog/hello-world.md" or "papers/research.pdf")
        path: String,
    },
}

impl Route {
    /// Parse URL hash into Route
    pub fn from_hash(hash: &str) -> Self {
        let path = hash.trim_start_matches('#').trim_start_matches('/');

        if path.is_empty() {
            return Self::Home;
        }

        Self::Read {
            path: path.to_string(),
        }
    }

    /// Convert Route to URL hash
    pub fn to_hash(&self) -> String {
        match self {
            Self::Home => "#/".to_string(),
            Self::Read { path } => format!("#/{}", path),
        }
    }

    /// Get current route from browser URL
    pub fn current() -> Self {
        let hash = web_sys::window()
            .and_then(|w| w.location().hash().ok())
            .unwrap_or_default();
        Self::from_hash(&hash)
    }

    /// Update browser URL to match this route (using pushState)
    pub fn push(&self) {
        if let Some(window) = web_sys::window()
            && let Ok(history) = window.history() {
                let hash = self.to_hash();
                let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&hash));
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_parsing() {
        assert_eq!(Route::from_hash(""), Route::Home);
        assert_eq!(Route::from_hash("#"), Route::Home);
        assert_eq!(Route::from_hash("#/"), Route::Home);
        // Path now includes file extension
        assert_eq!(
            Route::from_hash("#/blog/hello-world.md"),
            Route::Read {
                path: "blog/hello-world.md".to_string(),
            }
        );
        assert_eq!(
            Route::from_hash("#/projects/games/zkdungeon.md"),
            Route::Read {
                path: "projects/games/zkdungeon.md".to_string(),
            }
        );
        // PDF and other file types
        assert_eq!(
            Route::from_hash("#/papers/research.pdf"),
            Route::Read {
                path: "papers/research.pdf".to_string(),
            }
        );
    }

    #[test]
    fn test_route_to_hash() {
        assert_eq!(Route::Home.to_hash(), "#/");
        assert_eq!(
            Route::Read {
                path: "blog/hello-world.md".to_string(),
            }
            .to_hash(),
            "#/blog/hello-world.md"
        );
        assert_eq!(
            Route::Read {
                path: "projects/games/zkdungeon.md".to_string(),
            }
            .to_hash(),
            "#/projects/games/zkdungeon.md"
        );
        assert_eq!(
            Route::Read {
                path: "papers/research.pdf".to_string(),
            }
            .to_hash(),
            "#/papers/research.pdf"
        );
    }
}
