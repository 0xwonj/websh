//! Canonical viewport breakpoints used across the site.
//!
//! These values are the single source of truth for responsive thresholds.
//! Mirrored in `assets/tokens/breakpoints.css` for documentation; CSS
//! `@media` rules cannot read `var()` so the literal numbers must still
//! appear in each module's `@media (min-width: ...)` query.
//!
//! Tier semantics:
//! - `BP_SM` (460): chrome chip threshold — narrow phones.
//! - `BP_MD` (640): mobile compact ↔ desktop layout split (home, ledger).
//! - `BP_LG` (760): chrome wide tier — tablet/desktop content.
//! - `BP_XL` (1080): reader TOC sidebar — wide desktop.
//!
//! Use [`use_min_width`] for content/DOM-level branching that depends on
//! viewport (e.g. picking different truncation lengths). Pure visual
//! styling should stay in CSS `@media` rules.

use leptos::prelude::*;
use leptos_use::use_media_query;

pub const BP_SM: u32 = 460;
pub const BP_MD: u32 = 640;
pub const BP_LG: u32 = 760;
pub const BP_XL: u32 = 1080;

/// Reactive signal that is `true` when the viewport is at least `px` wide.
///
/// Wraps `leptos_use::use_media_query` with the canonical `(min-width: …)`
/// query form. Pass one of [`BP_SM`] / [`BP_MD`] / [`BP_LG`] / [`BP_XL`]
/// at the call site — keeping the threshold visible there beats hiding it
/// behind a tier enum the caller has to translate.
pub fn use_min_width(px: u32) -> Signal<bool> {
    use_media_query(format!("(min-width: {px}px)"))
}
