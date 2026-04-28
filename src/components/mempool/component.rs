//! Leptos component rendering a MempoolModel.

use leptos::prelude::*;

use super::model::{LedgerFilterShape, MempoolEntry, MempoolModel, MempoolStatus, Priority};

stylance::import_crate_style!(css, "src/components/mempool/mempool.module.css");

#[component]
pub fn Mempool(
    model: MempoolModel,
    #[prop(into)] on_select: Callback<MempoolEntry>,
    author_mode: Memo<bool>,
    #[prop(into)] on_compose: Callback<()>,
) -> impl IntoView {
    let header = render_header(&model, author_mode, on_compose);
    let rows = render_rows(&model, on_select);

    view! {
        <section class=css::mempool aria-label="Mempool — pending entries">
            {header}
            <div class=css::mpList>
                {rows}
            </div>
        </section>
    }
    .into_any()
}

fn render_header(
    model: &MempoolModel,
    author_mode: Memo<bool>,
    on_compose: Callback<()>,
) -> AnyView {
    let count_text = match &model.filter {
        LedgerFilterShape::All => format!("· {} pending", model.total_count),
        LedgerFilterShape::Category(_) => format!(
            "· {} / {} pending",
            model.entries.len(),
            model.total_count
        ),
    };
    view! {
        <div class=css::mpHead>
            <span class=css::mpLabel>"mempool"</span>
            <span class=css::mpCount>{count_text}</span>
            <span class=css::mpHeadRight>
                <Show when=move || author_mode.get()>
                    <button
                        class=css::mpCompose
                        type="button"
                        aria-label="Compose new mempool entry"
                        on:click=move |_| on_compose.run(())
                    >
                        "+ compose"
                    </button>
                </Show>
            </span>
        </div>
    }
    .into_any()
}

fn render_rows(model: &MempoolModel, on_select: Callback<MempoolEntry>) -> AnyView {
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
            let entry_for_click = entry.clone();
            let on_select = on_select;
            view! {
                <MempoolItem
                    entry=entry
                    on_click=Callback::new(move |_| {
                        on_select.run(entry_for_click.clone());
                    })
                />
            }
        })
        .collect_view()
        .into_any()
}

#[component]
fn MempoolItem(entry: MempoolEntry, on_click: Callback<()>) -> impl IntoView {
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

    let on_click_kbd = on_click;

    view! {
        <div
            class=item_class
            tabindex="0"
            role="button"
            on:click=move |_| on_click.run(())
            on:keydown=move |event| {
                if event.key() == "Enter" || event.key() == " " {
                    event.prevent_default();
                    on_click_kbd.run(());
                }
            }
        >
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
        </div>
    }
}
