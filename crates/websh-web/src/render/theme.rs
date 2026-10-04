//! Theme palette helpers.
//!
//! CSS consumes the active palette from `html[data-theme]`. This module owns
//! the catalog, normalization, and DOM/meta application only; runtime
//! persistence is handled by `RuntimeServices`.
//!
//! When adding a new theme, three places must stay in sync:
//!   1. `THEMES` below.
//!   2. `index.html` pre-paint script's `themes` map.
//!   3. `index.html`'s `<link data-trunk rel="css" href="assets/themes/<id>.css">`.

use websh_core::shell::OutputLine;

pub const DEFAULT_THEME: &str = "kanagawa-wave";
/// localStorage key for the active theme. Runtime services persist this through
/// the user environment as `$THEME` and `/.websh/state/env/THEME`.
pub use crate::runtime::state::THEME_KEY as STORAGE_KEY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    /// `<meta name="theme-color">` value — also the bg half of the palette
    /// swatch. Mirrors `--bg-primary` for the theme.
    pub meta_color: &'static str,
    /// Signature accent color — the other half of the palette swatch.
    /// Mirrors `--accent` for the theme.
    pub accent_color: &'static str,
}

pub const THEMES: &[ThemeDescriptor] = &[
    ThemeDescriptor {
        id: "catppuccin-mocha",
        label: "Catppuccin Mocha",
        meta_color: "#1e1e2e",
        accent_color: "#cba6f7",
    },
    ThemeDescriptor {
        id: "dracula",
        label: "Dracula",
        meta_color: "#282a36",
        accent_color: "#bd93f9",
    },
    ThemeDescriptor {
        id: "gruvbox-dark",
        label: "Gruvbox Dark",
        meta_color: "#282828",
        accent_color: "#cc241d",
    },
    ThemeDescriptor {
        id: "kanagawa-wave",
        label: "Kanagawa Wave",
        meta_color: "#1f1f28",
        accent_color: "#7e9cd8",
    },
    ThemeDescriptor {
        id: "nord",
        label: "Nord",
        meta_color: "#2e3440",
        accent_color: "#88c0d0",
    },
    ThemeDescriptor {
        id: "rose-pine",
        label: "Rosé Pine",
        meta_color: "#191724",
        accent_color: "#eb6f92",
    },
    ThemeDescriptor {
        id: "sepia-dark",
        label: "Sepia Dark",
        meta_color: "#100f0f",
        accent_color: "#da702c",
    },
    ThemeDescriptor {
        id: "tokyonight-night",
        label: "TokyoNight Night",
        meta_color: "#1a1b26",
        accent_color: "#7aa2f7",
    },
    ThemeDescriptor {
        id: "black-ink",
        label: "Black Ink",
        meta_color: "#fffcf0",
        accent_color: "#100f0f",
    },
    ThemeDescriptor {
        id: "catppuccin-latte",
        label: "Catppuccin Latte",
        meta_color: "#eff1f5",
        accent_color: "#8839ef",
    },
    ThemeDescriptor {
        id: "solarized-light",
        label: "Solarized Light",
        meta_color: "#fdf6e3",
        accent_color: "#268bd2",
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
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    if let Some(root) = document.document_element() {
        let _ = root.set_attribute("data-theme", theme_id);
    }
    if let Ok(Some(meta)) = document.query_selector(r#"meta[name="theme-color"]"#) {
        let color = THEMES
            .iter()
            .find(|theme| theme.id == theme_id)
            .map(|theme| theme.meta_color)
            .unwrap_or("#1f1f28");
        let _ = meta.set_attribute("content", color);
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

            let map_key = format!("\"{}\":", theme.id);
            assert!(
                index_html.contains(&map_key),
                "index.html pre-paint themes map missing key for theme {:?} (expected {map_key:?})",
                theme.id
            );
        }
    }
}
