//! Leptos component rendering a MempoolModel.

use leptos::prelude::*;

use super::model::{LedgerFilterShape, MempoolEntry, MempoolModel, MempoolStatus, Priority};
use crate::utils::content_routes::content_href_for_path;

stylance::import_crate_style!(css, "src/components/mempool/mempool.module.css");

#[component]
pub fn Mempool(
    model: MempoolModel,
    author_mode: Memo<bool>,
    collapsed: RwSignal<bool>,
) -> impl IntoView {
    let header = render_header(&model, author_mode, collapsed);
    let rows = move || render_rows(&model);

    view! {
        <section class=css::mempool aria-label="Mempool — pending entries">
            {header}
            <Show when=move || !collapsed.get()>
                <div class=css::mpList id="mempool-rows">
                    {rows()}
                </div>
            </Show>
        </section>
    }
    .into_any()
}

fn render_header(
    model: &MempoolModel,
    author_mode: Memo<bool>,
    collapsed: RwSignal<bool>,
) -> AnyView {
    let count_text = match &model.filter {
        LedgerFilterShape::All => format!("· {} pending", model.total_count),
        LedgerFilterShape::Category(_) => format!(
            "· {} / {} pending",
            model.entries.len(),
            model.total_count
        ),
    };
    let toggle = move || collapsed.update(|v| *v = !*v);
    let on_click = move |_: leptos::ev::MouseEvent| toggle();
    let on_keydown = move |event: leptos::ev::KeyboardEvent| {
        let key = event.key();
        if key == "Enter" || key == " " {
            event.prevent_default();
            toggle();
        }
    };
    view! {
        <div
            class=css::mpHead
            role="button"
            tabindex="0"
            aria-expanded=move || (!collapsed.get()).to_string()
            aria-controls="mempool-rows"
            on:click=on_click
            on:keydown=on_keydown
        >
            <span class=css::mpToggle aria-hidden="true">
                {move || if collapsed.get() { "▸" } else { "▾" }}
            </span>
            <span class=css::mpLabel>"mempool"</span>
            <span class=css::mpCount>{count_text}</span>
            <span class=css::mpHeadRight>
                <Show when=move || author_mode.get()>
                    <a
                        class=css::mpCompose
                        href="/#/new"
                        aria-label="Compose new mempool entry"
                        on:click=|ev| ev.stop_propagation()
                    >
                        "+ compose"
                    </a>
                </Show>
            </span>
        </div>
    }
    .into_any()
}

fn render_rows(model: &MempoolModel) -> AnyView {
    if model.entries.is_empty() {
        return view! {
            <div class=css::mpEmpty>
                "no pending entries match this filter"
            </div>
        }
        .into_any();
    }

    model
        .entries
        .iter()
        .cloned()
        .map(|entry| {
            view! { <MempoolItem entry=entry /> }
        })
        .collect_view()
        .into_any()
}

#[component]
fn MempoolItem(entry: MempoolEntry) -> impl IntoView {
    let item_class = match entry.status {
        MempoolStatus::Draft => format!("{} {}", css::mpItem, css::mpItemDraft),
        MempoolStatus::Review => format!("{} {}", css::mpItem, css::mpItemReview),
    };
    let status_label = match entry.status {
        MempoolStatus::Draft => "draft",
        MempoolStatus::Review => "review",
    };
    let priority_class = entry.priority.map(|p| match p {
        Priority::Low => css::mpPriLow,
        Priority::Med => css::mpPriMed,
        Priority::High => css::mpPriHigh,
    });
    let priority_text = entry.priority.map(|p| match p {
        Priority::Low => "low",
        Priority::Med => "med",
        Priority::High => "high",
    });

    let priority_view = priority_text.map(|text| {
        let value_class = format!("{} {}", css::mpMetaValue, priority_class.unwrap_or(""));
        view! {
            <span class=css::mpMetaKv>
                <span class=css::mpMetaKey>"priority"</span>
                <span class=value_class>{text}</span>
            </span>
        }
    });

    let href = content_href_for_path(entry.path.as_str());

    view! {
        <a class=item_class href=href>
            <div class=css::mpStatus>{status_label}</div>
            <div>
                <div class=css::mpTitle>
                    <span class=css::mpKindTag data-kind=entry.kind.clone()>{entry.kind.clone()}</span>
                    {entry.title.clone()}
                </div>
                <div class=css::mpDesc>{entry.desc.clone()}</div>
                <div class=css::mpMeta>
                    {priority_view}
                    <span class=css::mpMetaKv>
                        <span class=css::mpMetaKey>"gas"</span>
                        <span class=css::mpMetaValue>{entry.gas.clone()}</span>
                    </span>
                </div>
            </div>
            <div class=css::mpModified>{entry.modified.clone()}</div>
        </a>
    }
}
