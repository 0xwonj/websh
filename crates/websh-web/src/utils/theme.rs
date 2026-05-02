//! Theme palette helpers.
//!
//! The browser stores the selected palette as local preference. CSS consumes
//! it from `html[data-theme]`; Rust only validates and applies the attribute.
//!
//! When adding a new theme, three places must stay in sync:
//!   1. `THEMES` below.
//!   2. `index.html` pre-paint script's `themes` map.
//!   3. `index.html`'s `<link data-trunk rel="css" href="assets/themes/<id>.css">`.

pub const DEFAULT_THEME: &str = "kanagawa-wave";
/// localStorage key for the active theme. The `user.` prefix exposes it as
/// `$THEME` and at `/.websh/state/env/THEME` via the runtime env machinery.
pub const STORAGE_KEY: &str = "user.THEME";

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

pub fn normalize_theme_id(raw: &str) -> Option<&'static str> {
    let normalized = raw.trim().to_ascii_lowercase();
    let normalized = normalized.replace('_', "-");

    match normalized.as_str() {
        "sepia" | "sepia-dark" | "flexoki" | "flexoki-dark" | "dark-paper" => Some("sepia-dark"),
        "black" | "black-ink" | "ink" | "paper" | "paper-light" | "flexoki-light" => {
            Some("black-ink")
        }
        "gruvbox" | "gruvbox-dark" => Some("gruvbox-dark"),
        "tokyonight" | "tokyo-night" | "tokyonight-night" | "night" => Some("tokyonight-night"),
        "solarized" | "solarized-light" | "light" => Some("solarized-light"),
        "dracula" | "vampire" => Some("dracula"),
        "catppuccin" | "catppuccin-mocha" | "mocha" => Some("catppuccin-mocha"),
        "catppuccin-latte" | "latte" => Some("catppuccin-latte"),
        "nord" | "nordic" | "arctic" => Some("nord"),
        "rose-pine" | "rosepine" | "rose" | "pine" => Some("rose-pine"),
        "kanagawa" | "kanagawa-wave" | "wave" => Some("kanagawa-wave"),
        _ => None,
    }
}

pub fn initial_theme() -> &'static str {
    let Some(saved) = stored_theme() else {
        return DEFAULT_THEME;
    };
    normalize_theme_id(&saved).unwrap_or(DEFAULT_THEME)
}

#[allow(clippy::unnecessary_wraps)]
pub fn apply_theme(id: &str) -> Result<&'static str, String> {
    let Some(theme_id) = normalize_theme_id(id) else {
        return Err(format!("unknown theme: {id}"));
    };

    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return Ok(theme_id);
        };

        if let Some(document) = window.document() {
            if let Some(root) = document.document_element() {
                let _ = root.set_attribute("data-theme", theme_id);
            }

            if let Some(meta) = document
                .query_selector(r#"meta[name="theme-color"]"#)
                .ok()
                .flatten()
            {
                let meta_color = THEMES
                    .iter()
                    .find(|theme| theme.id == theme_id)
                    .map(|theme| theme.meta_color)
                    .unwrap_or("#1f1f28");
                let _ = meta.set_attribute("content", meta_color);
            }
        }

        // Persist via the runtime env adapter so `user.THEME` mirrors here,
        // `/.websh/state/env/THEME`, and `$THEME` stay in sync from one write.
        let _ = crate::core::runtime::state::set_env_var("THEME", theme_id);
    }

    Ok(theme_id)
}

fn stored_theme() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_theme_aliases() {
        assert_eq!(normalize_theme_id("sepia"), Some("sepia-dark"));
        assert_eq!(normalize_theme_id("paper"), Some("black-ink"));
        assert_eq!(normalize_theme_id("tokyo-night"), Some("tokyonight-night"));
        assert_eq!(normalize_theme_id("light"), Some("solarized-light"));
        assert_eq!(normalize_theme_id("dracula"), Some("dracula"));
        assert_eq!(normalize_theme_id("catppuccin"), Some("catppuccin-mocha"));
        assert_eq!(normalize_theme_id("mocha"), Some("catppuccin-mocha"));
        assert_eq!(normalize_theme_id("latte"), Some("catppuccin-latte"));
        assert_eq!(normalize_theme_id("nord"), Some("nord"));
        assert_eq!(normalize_theme_id("rose-pine"), Some("rose-pine"));
        assert_eq!(normalize_theme_id("rosepine"), Some("rose-pine"));
        assert_eq!(normalize_theme_id("kanagawa"), Some("kanagawa-wave"));
        assert_eq!(normalize_theme_id("unknown"), None);
    }

    /// Guards against drift between `THEMES` and `index.html`.
    #[test]
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
