use leptos::prelude::*;

#[component]
pub fn MetaTable(
    class: &'static str,
    aria_label: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <section class=class aria-label=aria_label>
            {children()}
        </section>
    }
}

#[component]
pub fn MetaRow(
    label: &'static str,
    row_class: &'static str,
    key_class: &'static str,
    value_class: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=row_class>
            <div class=key_class>{label}</div>
            <div class=value_class>{children()}</div>
        </div>
    }
}
