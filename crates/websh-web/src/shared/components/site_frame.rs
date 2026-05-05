use leptos::prelude::*;

stylance::import_crate_style!(css, "src/shared/components/site_frame.module.css");

#[component]
pub fn SiteSurface(class: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class=format!("{} {}", css::surface, class)>
            {children()}
        </div>
    }
}

#[component]
pub fn SiteContentFrame(class: &'static str, children: Children) -> impl IntoView {
    view! {
        <main class=format!("{} {}", css::content, class)>
            {children()}
        </main>
    }
}
