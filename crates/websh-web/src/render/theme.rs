//! Theme palette helpers.
//!
//! CSS owns palette variables on `[data-theme]` elements. This module owns
//! the catalog, normalization, and DOM/meta application only; runtime
//! persistence is handled by `RuntimeServices`.
//!
//! The catalog supplies IDs and labels; palette colors live only in CSS.
//! Add the catalog entry and its stylesheet link in `index.html` together.

use websh_core::shell::OutputLine;

pub const DEFAULT_THEME: &str = "kanagawa-wave";
/// localStorage key for the active theme. Runtime services persist this through
/// the user environment as `$THEME` and `/.websh/state/env/THEME`.
pub use crate::runtime::state::THEME_KEY as STORAGE_KEY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeDescriptor {
    pub id: &'static str,
    pub label: &'static str,
}

pub const THEMES: &[ThemeDescriptor] = &[
    ThemeDescriptor {
        id: "catppuccin-mocha",
        label: "Catppuccin Mocha",
    },
    ThemeDescriptor {
        id: "dracula",
        label: "Dracula",
    },
    ThemeDescriptor {
        id: "gruvbox-dark",
        label: "Gruvbox Dark",
    },
    ThemeDescriptor {
        id: "kanagawa-wave",
        label: "Kanagawa Wave",
    },
    ThemeDescriptor {
        id: "nord",
        label: "Nord",
    },
    ThemeDescriptor {
        id: "rose-pine",
        label: "Rosé Pine",
    },
    ThemeDescriptor {
        id: "sepia-dark",
        label: "Sepia Dark",
    },
    ThemeDescriptor {
        id: "tokyonight-night",
        label: "TokyoNight Night",
    },
    ThemeDescriptor {
        id: "black-ink",
        label: "Black Ink",
    },
    ThemeDescriptor {
        id: "catppuccin-latte",
        label: "Catppuccin Latte",
    },
    ThemeDescriptor {
        id: "solarized-light",
        label: "Solarized Light",
    },
];

pub fn theme_ids() -> impl Iterator<Item = &'static str> {
    THEMES.iter().map(|theme| theme.id)
}

pub fn theme_label(id: &str) -> Option<&'static str> {
    THEMES
        .iter()
        .find(|theme| theme.id == id)
        .map(|theme| theme.label)
}

pub fn theme_output_lines() -> Vec<OutputLine> {
    let mut lines = vec![OutputLine::text("available themes:")];
    lines.extend(
        THEMES
            .iter()
            .map(|theme| OutputLine::text(format!("  {:<18} {}", theme.id, theme.label))),
    );
    lines
}

pub fn normalize_theme_id(raw: &str) -> Option<&'static str> {
    let id = raw.trim().to_ascii_lowercase();
    THEMES
        .iter()
        .find(|theme| theme.id == id)
        .map(|theme| theme.id)
}

pub fn apply_theme_to_document(theme_id: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(root) = document.document_element() else {
        return;
    };
    let _ = root.set_attribute("data-theme", theme_id);
    if let Ok(Some(style)) = window.get_computed_style(&root)
        && let Ok(color) = style.get_property_value("--bg-primary")
        && !color.trim().is_empty()
        && let Ok(Some(meta)) = document.query_selector(r#"meta[name="theme-color"]"#)
    {
        let _ = meta.set_attribute("content", color.trim());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn accepts_only_catalog_theme_ids() {
        for theme in THEMES {
            assert_eq!(normalize_theme_id(theme.id), Some(theme.id));
        }
        assert_eq!(normalize_theme_id(" DRACULA "), Some("dracula"));
        assert_eq!(normalize_theme_id("unknown"), None);
    }

    /// Guards against drift between `THEMES` and `index.html`.
    #[wasm_bindgen_test]
    fn index_html_lists_all_themes() {
        let index_html = include_str!("../../../../index.html");

        assert!(
            index_html.contains(STORAGE_KEY),
            "index.html missing STORAGE_KEY {STORAGE_KEY:?}"
        );
        assert!(
            index_html.contains(DEFAULT_THEME),
            "index.html missing DEFAULT_THEME {DEFAULT_THEME:?}"
        );

        for theme in THEMES {
            let link_href = format!("assets/themes/{}.css", theme.id);
            assert!(
                index_html.contains(&link_href),
                "index.html missing <link> for theme {:?} (expected href {link_href:?})",
                theme.id
            );
        }
    }
}
