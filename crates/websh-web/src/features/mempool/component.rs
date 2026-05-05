//! Leptos component rendering a MempoolModel.

use leptos::prelude::*;

use super::model::{LedgerFilterShape, MempoolEntry, MempoolModel};
use crate::app::AppContext;
use crate::app::RuntimeServices;
use crate::runtime::MountLoadStatus;
use websh_core::domain::{MempoolStatus, Priority};
use websh_core::filesystem::content_href_for_path;
use websh_core::mempool::mempool_root;

stylance::import_crate_style!(css, "src/features/mempool/mempool.module.css");

const MEMPOOL_RENDER_LIMIT: usize = 100;

#[component]
pub fn Mempool(
    model: MempoolModel,
    author_mode: Memo<bool>,
    collapsed: RwSignal<bool>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let mount_status = Signal::derive(move || ctx.mount_status_for(mempool_root()));

    let key_form_open = RwSignal::new(false);
    let key_form_error: RwSignal<Option<String>> = RwSignal::new(None);

    let header = render_header(
        author_mode,
        collapsed,
        &model,
        key_form_open,
        key_form_error,
    );
    let key_form = render_key_form(ctx, key_form_open, key_form_error);
    let rows = render_rows(&model, mount_status);
    let banner = render_mount_banner(mount_status);

    // Toggle via `prop:hidden` (not `<Show>`) so the W3C disclosure
    // pattern's `aria-controls="mempool-rows"` always resolves.
    view! {
        <section class=css::mempool aria-label="Mempool — pending blocks">
            {header}
            {key_form}
            <div
                class=css::mpList
                id="mempool-rows"
                prop:hidden=move || collapsed.get()
            >
                {banner}
                {rows}
            </div>
        </section>
    }
    .into_any()
}

fn render_mount_banner(mount_status: Signal<Option<MountLoadStatus>>) -> AnyView {
    view! {
        <Show when=move || matches!(mount_status.get(), Some(MountLoadStatus::Loading { .. }))>
            <div class=css::mpMountUnavailable>
                <span class=css::mpMountTitle>"mount loading"</span>
                <span class=css::mpMountReason>"pending blocks are loading from the remote backend"</span>
                <span class=css::mpMountHint>"root content remains available while this completes"</span>
            </div>
        </Show>
        <Show when=move || matches!(mount_status.get(), Some(MountLoadStatus::Failed { .. }))>
            {move || {
                let error = match mount_status.get() {
                    Some(MountLoadStatus::Failed { error, .. }) => error,
                    _ => String::new(),
                };
                view! {
                    <div class=css::mpMountUnavailable>
                        <span class=css::mpMountTitle>"mount unavailable"</span>
                        <span class=css::mpMountReason>{error}</span>
                        <span class=css::mpMountHint>
                            "remote backend unreachable — try refreshing later"
                        </span>
                    </div>
                }
            }}
        </Show>
    }
    .into_any()
}

fn render_header(
    author_mode: Memo<bool>,
    collapsed: RwSignal<bool>,
    model: &MempoolModel,
    key_form_open: RwSignal<bool>,
    key_form_error: RwSignal<Option<String>>,
) -> AnyView {
    let count_text = match &model.filter {
        LedgerFilterShape::All => format!("· {} pending", model.total_count),
        LedgerFilterShape::Category(_) => {
            format!("· {} / {} pending", model.entries.len(), model.total_count)
        }
    };
    let toggle = move || collapsed.update(|v| *v = !*v);
    let on_disclosure_click = move |_: leptos::ev::MouseEvent| toggle();
    // Clear the error explicitly — it lives outside `<Show>` and would
    // otherwise re-appear on the next open.
    let on_key_click = move |ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        key_form_error.set(None);
        key_form_open.update(|open| *open = !*open);
    };

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
            <span class=css::mpHeadRight>
                {move || if author_mode.get() {
                    view! {
                        <a
                            class=css::mpCompose
                            href="#/new"
                            aria-label="Submit a new mempool entry"
                        >
                            "+ submit"
                        </a>
                    }.into_any()
                } else {
                    view! {
                        <button
                            class=css::mpCompose
                            type="button"
                            aria-label="Register a signer key to enable submitting"
                            aria-expanded=move || key_form_open.get().to_string()
                            on:click=on_key_click
                        >
                            "+ key"
                        </button>
                    }.into_any()
                }}
            </span>
        </div>
    }
    .into_any()
}

/// PAT-entry form mirroring the terminal `sync auth set <token>` flow.
fn render_key_form(
    ctx: AppContext,
    key_form_open: RwSignal<bool>,
    key_form_error: RwSignal<Option<String>>,
) -> AnyView {
    let input_ref = NodeRef::<leptos::html::Input>::new();

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(input) = input_ref.get_untracked() else {
            return;
        };
        let token = input.value();
        let trimmed = token.trim();
        if trimmed.is_empty() {
            key_form_error.set(Some("token cannot be empty".to_string()));
            return;
        }
        match RuntimeServices::new(ctx).set_github_token(trimmed) {
            Ok(()) => {
                input.set_value("");
                key_form_error.set(None);
                key_form_open.set(false);
            }
            Err(error) => {
                key_form_error.set(Some(format!("save failed: {error}")));
            }
        }
    };

    let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
        if ev.key() == "Escape" {
            ev.prevent_default();
            if let Some(input) = input_ref.get_untracked() {
                input.set_value("");
            }
            key_form_error.set(None);
            key_form_open.set(false);
        }
    };

    view! {
        <Show when=move || key_form_open.get()>
            <form class=css::mpKeyForm on:submit=on_submit on:keydown=on_keydown>
                <span class=css::mpKeyTitle>"register as a proposer"</span>
                <div class=css::mpKeyRow>
                    <input
                        class=css::mpKeyInput
                        node_ref=input_ref
                        type="password"
                        name="github_pat"
                        autocomplete="off"
                        spellcheck="false"
                        placeholder="github_pat_…"
                        aria-label="GitHub Personal Access Token"
                    />
                    <button class=css::mpKeyBtn type="submit">"save"</button>
                </div>
                <span class=css::mpKeyHint>
                    "author-only · broadcasts drafts to "
                    <a
                        class=css::mpKeyHintLink
                        href="https://github.com/0xwonj/websh-mempool"
                        target="_blank"
                        rel="noopener"
                    >
                        "mempool"
                    </a>
                </span>
                {move || key_form_error.get().map(|message| view! {
                    <span class=css::mpKeyError role="alert">{message}</span>
                })}
            </form>
        </Show>
    }
    .into_any()
}

fn render_rows(model: &MempoolModel, mount_status: Signal<Option<MountLoadStatus>>) -> AnyView {
    if model.entries.is_empty() {
        return view! {
            <div class=css::mpEmpty>
                {move || match mount_status.get() {
                    Some(MountLoadStatus::Loading { .. }) => {
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
