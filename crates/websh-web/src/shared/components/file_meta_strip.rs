use leptos::prelude::*;

#[component]
pub fn FileMetaStrip(
    date: Signal<Option<String>>,
    tags: Signal<Vec<String>>,
    class: &'static str,
    date_class: &'static str,
    tags_class: &'static str,
    tag_class: &'static str,
) -> impl IntoView {
    view! {
        <div class=class>
            {move || date.get().map(|date| view! {
                <span class=date_class>{date}</span>
            })}
            <div class=tags_class>
                {move || {
                    tags.get()
                        .into_iter()
                        .map(|tag| view! { <span class=tag_class>{tag}</span> })
                        .collect_view()
                }}
            </div>
        </div>
    }
}
