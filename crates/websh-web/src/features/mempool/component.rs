//! Leptos component rendering a MempoolModel.

use leptos::prelude::*;

use super::model::{LedgerFilterShape, MempoolEntry, MempoolModel};
use crate::app::AppContext;
use crate::runtime::MountLoadStatus;
use websh_core::domain::{MempoolStatus, Priority};
use websh_core::filesystem::content_href_for_path;
use websh_core::mempool::mempool_root;

stylance::import_crate_style!(css, "src/features/mempool/mempool.module.css");

const MEMPOOL_RENDER_LIMIT: usize = 100;

#[component]
pub fn Mempool(model: MempoolModel, collapsed: RwSignal<bool>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let mount_status = Signal::derive(move || ctx.content.mount_status_for(mempool_root()));

    let header = render_header(collapsed, &model);
    let rows = render_rows(&model, mount_status);

    // Toggle via `prop:hidden` (not `<Show>`) so the W3C disclosure
    // pattern's `aria-controls="mempool-rows"` always resolves.
    view! {
        <section class=css::mempool aria-label="Mempool — pending blocks">
            {header}
            <div
                class=css::mpList
                id="mempool-rows"
                prop:hidden=move || collapsed.get()
            >
                <crate::shared::components::MountStatusNotice path=Signal::derive(|| mempool_root().clone()) />
                {rows}
            </div>
        </section>
    }
    .into_any()
}

fn render_header(collapsed: RwSignal<bool>, model: &MempoolModel) -> AnyView {
    let count_text = match &model.filter {
        LedgerFilterShape::All => format!("· {} pending", model.total_count),
        LedgerFilterShape::Category(_) => {
            format!("· {} / {} pending", model.entries.len(), model.total_count)
        }
    };
    let toggle = move || collapsed.update(|v| *v = !*v);
    let on_disclosure_click = move |_: leptos::ev::MouseEvent| toggle();
    view! {
        <div class=css::mpHead>
            <button
                class=css::mpDisclosure
                type="button"
                aria-expanded=move || (!collapsed.get()).to_string()
                aria-controls="mempool-rows"
                on:click=on_disclosure_click
            >
                <span class=css::mpToggle aria-hidden="true">
                    {move || if collapsed.get() { "▸" } else { "▾" }}
                </span>
                <span class=css::mpLabel>"mempool"</span>
                <span class=css::mpCount>{count_text}</span>
            </button>
        </div>
    }
    .into_any()
}

fn render_rows(model: &MempoolModel, mount_status: Signal<Option<MountLoadStatus>>) -> AnyView {
    if model.entries.is_empty() {
        return view! {
            <div class=css::mpEmpty>
                {move || match mount_status.get() {
                    Some(MountLoadStatus::Loading) => {
                        "pending blocks are loading from the remote backend"
                    }
                    Some(MountLoadStatus::Failed { .. }) => {
                        "pending blocks are unavailable"
                    }
                    _ => "no pending blocks match this filter",
                }}
            </div>
        }
        .into_any();
    }

    let total_entries = model.entries.len();
    let visible_count = total_entries.min(MEMPOOL_RENDER_LIMIT);
    let limited = total_entries > visible_count;

    view! {
        {model
        .entries
        .iter()
        .take(visible_count)
        .cloned()
        .map(|entry| {
            view! { <MempoolItem entry=entry /> }
        })
        .collect_view()}
        {limited.then(|| view! {
            <div class=css::mpEmpty>
                {format!("showing newest {visible_count} of {total_entries} pending blocks")}
            </div>
        })}
    }
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
    let priority_view = entry.priority.map(|p| {
        let (arrows, text, tone) = match p {
            Priority::Low => ("▲", "low", css::mpPriLow),
            Priority::Med => ("▲▲", "med", css::mpPriMed),
            Priority::High => ("▲▲▲", "high", css::mpPriHigh),
        };
        let value_class = format!("{} {}", css::mpMetaValue, tone);
        view! {
            <span class=css::mpMetaKv>
                <span class=css::mpMetaKey>"priority"</span>
                <span class=value_class>
                    <span class=css::mpPriArrows>{arrows}</span>
                    <span class=css::mpPriLabel>{text}</span>
                </span>
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
