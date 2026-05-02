//! Single-line "identifier strip" used at the top of each surface.
//! Default tone: white left + yellow right. Pass `muted=true` for the
//! reader's flatter dim-on-dim treatment.

use leptos::prelude::*;

stylance::import_crate_style!(css, "src/components/shared/identifier_strip.module.css");

#[component]
pub fn IdentifierStrip(#[prop(optional)] muted: bool, children: Children) -> impl IntoView {
    let class = if muted {
        format!("{} {}", css::strip, css::stripMuted)
    } else {
        css::strip.to_string()
    };
    view! {
        <div class=class>
            {children()}
        </div>
    }
}
