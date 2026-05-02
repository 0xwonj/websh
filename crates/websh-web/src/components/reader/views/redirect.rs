//! Redirect placeholder — short message while the browser navigates away.

use leptos::prelude::*;

use crate::components::reader::css;

#[component]
pub fn RedirectingView() -> impl IntoView {
    view! {
        <div class=css::redirecting>"Redirecting…"</div>
    }
}
