//! Inline monospace value cell.
//!
//! Renders a single-line monospace value with controlled fit behavior
//! inside whatever cell the caller places it in. Three modes:
//!
//! - [`MonoOverflow::Scroll`] (default): the full value remains in the DOM and
//!   the cell scrolls horizontally when it overflows. The reader can pan to
//!   inspect the entire value end-to-end.
//! - [`MonoOverflow::TruncateEnd`]: the value is clipped at the right edge
//!   with a CSS ellipsis. Use when visual fit matters more than full
//!   readability and the full value is exposed elsewhere (e.g. via `title`
//!   or a separate popover).
//! - [`MonoOverflow::Middle`]: the value is shortened at the data layer to
//!   `head` + `…` + `tail` characters. Use for compact identifier display
//!   where both ends are recognizable (wallet addresses, short hashes).
//!   Pure CSS cannot produce a middle ellipsis, so this mode pre-truncates
//!   the value before rendering.
//!
//! Always pass the full value — `MonoValue` itself decides whether to
//! shorten. Existing helpers like `short_hash` and `format_eth_address`
//! remain available for non-cell contexts (terminal output, status
//! messages) where `MonoValue` would be inappropriate.
//!
//! Three orthogonal styling axes:
//!
//! - [`MonoTone`]: text color (`Plain` / `Accent` / `Hex`).
//! - [`MonoOverflow`]: fit behavior (`Scroll` / `TruncateEnd` / `Middle`).
//! - [`MonoFont`]: font family (`Code` default = system mono / `Body` =
//!   Plex Mono opt-in).

use leptos::prelude::*;

use crate::utils::breakpoints::{BP_LG, BP_SM, use_min_width};

stylance::import_crate_style!(css, "src/components/shared/mono_value.module.css");

/// Color treatment applied to the value text.
///
/// Maps to the project's archive palette — `Plain` for ordinary text-primary
/// values, `Accent` for identifiers tied to a signing identity (PGP
/// fingerprints, signatures), `Hex` for canonical hex digests (addresses,
/// message hashes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MonoTone {
    #[default]
    Plain,
    Accent,
    Hex,
}

/// Font family used to render the value.
///
/// `Code` (default) uses the system code stack (`--font-code`, OS-native:
/// SF Mono / Cascadia Mono / Consolas / DejaVu) for a deliberately rigid,
/// IDE-style code feel. `Body` opts back into the project's prose
/// monospace stack (`--font-mono`, IBM Plex Mono first) for cells whose
/// values read as continuous prose rather than identifier tokens.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MonoFont {
    Body,
    #[default]
    Code,
}

/// How the value should behave when it doesn't fit its container.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MonoOverflow {
    /// Full value preserved; cell scrolls horizontally on overflow.
    #[default]
    Scroll,
    /// Cell is clipped at the right edge with a CSS ellipsis.
    TruncateEnd,
    /// Value is pre-shortened to `head` + `…` + `tail` characters at the
    /// data layer. Values that already fit within `head + tail + 1` chars
    /// are rendered untouched.
    Middle { head: usize, tail: usize },
    /// Three viewport-keyed tiers, each independently `Some((head, tail))`
    /// for middle-ellipsis truncation or `None` to render the full value
    /// untruncated. `narrow` applies below [`BP_SM`], `medium` between
    /// [`BP_SM`] and [`BP_LG`], `wide` at or above [`BP_LG`]. Re-renders
    /// on resize via `use_media_query`.
    ResponsiveMiddle {
        narrow: Option<(usize, usize)>,
        medium: Option<(usize, usize)>,
        wide: Option<(usize, usize)>,
    },
}

/// Inline monospace value cell. See module docs for usage guidance.
#[component]
pub fn MonoValue(
    /// The value to display. Static at the call site — pass the already-resolved
    /// string. Reactive callers should re-render the parent fragment.
    #[prop(into)]
    value: String,
    /// Color treatment. Defaults to [`MonoTone::Plain`].
    #[prop(optional)]
    tone: MonoTone,
    /// Fit behavior. Defaults to [`MonoOverflow::Scroll`].
    #[prop(optional)]
    overflow: MonoOverflow,
    /// Font family. Defaults to [`MonoFont::Code`] (system code mono).
    #[prop(optional)]
    font: MonoFont,
    /// Native tooltip shown on hover. Useful for `TruncateEnd` and `Middle`
    /// modes where the full value would otherwise be unreachable.
    #[prop(optional, into)]
    title: Option<String>,
) -> impl IntoView {
    let class_name = compose_class(tone, overflow, font);
    let title_str = title.unwrap_or_default();

    match overflow {
        MonoOverflow::ResponsiveMiddle {
            narrow,
            medium,
            wide,
        } => {
            let above_sm = use_min_width(BP_SM);
            let above_lg = use_min_width(BP_LG);
            let display = move || {
                let tier = if above_lg.get() {
                    wide
                } else if above_sm.get() {
                    medium
                } else {
                    narrow
                };
                match tier {
                    Some((head, tail)) => middle_ellipsis(&value, head, tail),
                    None => value.clone(),
                }
            };
            view! {
                <span class=class_name title=title_str>{display}</span>
            }
            .into_any()
        }
        MonoOverflow::Middle { head, tail } => {
            let display_value = middle_ellipsis(&value, head, tail);
            view! {
                <span class=class_name title=title_str>{display_value}</span>
            }
            .into_any()
        }
        MonoOverflow::Scroll | MonoOverflow::TruncateEnd => view! {
            <span class=class_name title=title_str>{value}</span>
        }
        .into_any(),
    }
}

fn compose_class(tone: MonoTone, overflow: MonoOverflow, font: MonoFont) -> String {
    let overflow_class = match overflow {
        MonoOverflow::Scroll => css::scroll,
        MonoOverflow::TruncateEnd => css::truncateEnd,
        MonoOverflow::Middle { .. } | MonoOverflow::ResponsiveMiddle { .. } => css::middle,
    };
    let tone_class = match tone {
        MonoTone::Plain => "",
        MonoTone::Accent => css::accent,
        MonoTone::Hex => css::hex,
    };
    let font_class = match font {
        MonoFont::Body => "",
        MonoFont::Code => css::code,
    };
    let mut out = String::with_capacity(64);
    out.push_str(css::mono);
    out.push(' ');
    out.push_str(overflow_class);
    if !tone_class.is_empty() {
        out.push(' ');
        out.push_str(tone_class);
    }
    if !font_class.is_empty() {
        out.push(' ');
        out.push_str(font_class);
    }
    out
}

/// Shorten `value` to `head` + `…` + `tail` characters. UTF-8 safe — counts
/// scalar values, not bytes. Returns the input untouched if it already fits
/// within `head + tail + 1` characters.
fn middle_ellipsis(value: &str, head: usize, tail: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= head + tail + 1 {
        return value.to_string();
    }
    let head_part: String = chars[..head].iter().collect();
    let tail_part: String = chars[chars.len() - tail..].iter().collect();
    format!("{head_part}…{tail_part}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_includes_base_and_overflow() {
        let cls = compose_class(MonoTone::Plain, MonoOverflow::Scroll, MonoFont::Body);
        assert!(cls.contains(css::mono));
        assert!(cls.contains(css::scroll));
    }

    #[test]
    fn class_includes_tone_when_non_plain() {
        let cls = compose_class(MonoTone::Hex, MonoOverflow::Scroll, MonoFont::Body);
        assert!(cls.contains(css::hex));
    }

    #[test]
    fn class_omits_tone_when_plain() {
        let cls = compose_class(MonoTone::Plain, MonoOverflow::TruncateEnd, MonoFont::Body);
        assert!(!cls.contains(css::accent));
        assert!(!cls.contains(css::hex));
        assert!(cls.contains(css::truncateEnd));
    }

    #[test]
    fn middle_class_replaces_overflow() {
        let cls = compose_class(
            MonoTone::Hex,
            MonoOverflow::Middle { head: 6, tail: 4 },
            MonoFont::Body,
        );
        assert!(cls.contains(css::middle));
        assert!(!cls.contains(css::scroll));
        assert!(!cls.contains(css::truncateEnd));
    }

    #[test]
    fn body_font_omits_code_class() {
        let cls = compose_class(MonoTone::Plain, MonoOverflow::Scroll, MonoFont::Body);
        assert!(!cls.contains(css::code));
    }

    #[test]
    fn code_font_includes_code_class() {
        let cls = compose_class(MonoTone::Hex, MonoOverflow::Scroll, MonoFont::Code);
        assert!(cls.contains(css::code));
        assert!(cls.contains(css::hex));
        assert!(cls.contains(css::scroll));
    }

    #[test]
    fn middle_ellipsis_long_value() {
        let addr = "0x1234567890abcdef1234567890abcdef12345678";
        let short = middle_ellipsis(addr, 6, 4);
        assert_eq!(short, "0x1234…5678");
    }

    #[test]
    fn middle_ellipsis_short_value_untouched() {
        let value = "0x1234";
        assert_eq!(middle_ellipsis(value, 6, 4), value);
    }

    #[test]
    fn middle_ellipsis_boundary_value_untouched() {
        // head + tail + 1 = 11; equal-length input remains as-is.
        let value = "abcdef12345";
        assert_eq!(middle_ellipsis(value, 6, 4), value);
    }

    #[test]
    fn middle_ellipsis_unicode_safe() {
        let value = "한글한글한글한글한글한글";
        let short = middle_ellipsis(value, 2, 2);
        assert_eq!(short, "한글…한글");
    }

    #[test]
    fn responsive_middle_uses_middle_class() {
        let cls = compose_class(
            MonoTone::Hex,
            MonoOverflow::ResponsiveMiddle {
                narrow: Some((6, 4)),
                medium: Some((10, 6)),
                wide: Some((14, 8)),
            },
            MonoFont::Code,
        );
        assert!(cls.contains(css::middle));
        assert!(!cls.contains(css::scroll));
        assert!(!cls.contains(css::truncateEnd));
    }
}
