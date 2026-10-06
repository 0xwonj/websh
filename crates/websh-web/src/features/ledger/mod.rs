//! Ledger-style index page for content directories.

mod model;
pub mod routes;

use leptos::prelude::*;

use crate::app::AppContext;
use crate::features::chrome::SiteChrome;
use crate::features::mempool::{
    LedgerFilterShape, Mempool, build_mempool_model, load_mempool_files,
};
use crate::shared::components::{
    IdentifierStrip, MetaRow, MetaTable, MonoOverflow, MonoTone, MonoValue, ReleaseSigFooter,
    SiteContentFrame, SiteSurface,
};
use model::{LedgerEntry, LedgerFilter, LedgerModel, build_ledger_model, ledger_filter_for_route};
use websh_core::domain::VirtualPath;
use websh_core::filesystem::RouteFrame;
use websh_core::mempool::{LEDGER_CATEGORIES, mempool_root};

stylance::import_crate_style!(css, "src/features/ledger/ledger_page.module.css");

const LEDGER_RENDER_LIMIT: usize = 200;

#[component]
pub fn LedgerPage(route: Memo<RouteFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let mempool_ctx = ctx;
    let mempool_files = Memo::new(move |_| load_mempool_files(mempool_ctx));

    // Mempool collapse state lives at the LedgerPage level so it survives
    // filter-route changes (which re-render but don't re-mount this page).
    // Independent drafts are opt-in alongside the signed publication catalog.
    let mempool_collapsed = RwSignal::new(true);

    let attestation_route = Signal::derive(|| "/ledger".to_string());

    view! {
        <SiteSurface class=css::surface>
            <SiteChrome route=route />
            <SiteContentFrame class=css::page>
                <crate::shared::components::MountStatusNotice path=Signal::derive(VirtualPath::root) />
                {move || {
                    let Some(release) = ctx.content.release() else {
                        return view! { <LedgerPending message="Waiting for a verified content release" /> }.into_any();
                    };
                    let frame = route.get();
                    let filter = ledger_filter_for_route(&frame.request.url_path, &frame.resolution.node_path);
                    let model = ctx.content.with_fs(|fs| build_ledger_model(fs, &release.release().publications, release.id().as_str(), &filter));
                    let filter_shape = match &filter {
                        LedgerFilter::All => LedgerFilterShape::All,
                        LedgerFilter::Category(category) => LedgerFilterShape::Category(category.clone()),
                    };
                    let mempool_section = move || {
                        let model = build_mempool_model(mempool_root(), mempool_files.get(), &filter_shape);
                        view! { <Mempool model=model collapsed=mempool_collapsed /> }
                    };
                    view! {
                        <LedgerIdentifier model=model.clone() />
                        <LedgerHeader model=model.clone() />
                        <LedgerFilterBar model=model.clone() />
                        {mempool_section}
                        <PublicationList model=model />
                    }.into_any()
                }}
                <ReleaseSigFooter route=attestation_route />
            </SiteContentFrame>
        </SiteSurface>
    }
}

#[component]
fn LedgerIdentifier(model: LedgerModel) -> impl IntoView {
    view! {
        <IdentifierStrip>
            <span>"websh publications"</span>
            <span>{format!("latest publication {}", model.latest_date)}</span>
        </IdentifierStrip>
    }
}

#[component]
fn LedgerHeader(model: LedgerModel) -> impl IntoView {
    let release_id = model.release_id.clone();
    let release_label = format!("content release {release_id}");

    view! {
        <MetaTable class=css::ledgerHead aria_label="Ledger metadata">
            <MetaRow label="entries" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <span class=css::num>{model.entries.len()}</span>
                <span class=css::faintSep>" · "</span>
                " restricted "
                <span class=css::num>{model.restricted_count}</span>
            </MetaRow>
            <MetaRow label="release" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <span aria-label=release_label>
                    <MonoValue
                        value=release_id.clone()
                        tone=MonoTone::Hex
                        overflow=MonoOverflow::ResponsiveMiddle {
                            narrow: Some((12, 6)),
                            medium: Some((18, 8)),
                            wide: Some((24, 12)),
                        }
                        title=release_id
                    />
                </span>
                " "
                <span class=css::ok aria-label="release verified" title="release verified">"✓"</span>
            </MetaRow>
            <MetaRow label="status" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <span class=css::live>"verified snapshot"</span>
            </MetaRow>
        </MetaTable>
    }
}

#[component]
fn LedgerPending(message: impl Into<String>) -> impl IntoView {
    view! {
        <IdentifierStrip>
            <span>"~"</span>
            <span>"content pending"</span>
        </IdentifierStrip>
        <section class=css::empty>
            {message.into()}
        </section>
    }
}

#[component]
fn LedgerFilterBar(model: LedgerModel) -> impl IntoView {
    view! {
        <nav class=css::filterBar aria-label="Ledger filters">
            <span class=css::dash aria-hidden="true"></span>
            <LedgerFilterLink label="all" href="#/ledger" count=model.total_count active=model.filter.is_all() />
            {LEDGER_CATEGORIES.iter().map(|category| {
                let href = format!("#/{category}");
                let count = *model.counts.get(*category).unwrap_or(&0);
                let active = model.filter.matches(category);
                view! {
                    <LedgerFilterLink label=*category href=href count=count active=active />
                }
            }).collect_view()}
            <span class=css::dash aria-hidden="true"></span>
        </nav>
    }
}

#[component]
fn LedgerFilterLink(
    label: &'static str,
    href: impl Into<String>,
    count: usize,
    active: bool,
) -> impl IntoView {
    let class_name = if active {
        format!("{} {}", css::filterLink, css::filterLinkOn)
    } else {
        css::filterLink.to_string()
    };
    view! {
        <a class=class_name href=href.into() aria-current=if active { "page" } else { "false" }>
            {label}
            " "
            <span class=css::count>{count}</span>
        </a>
    }
}

#[component]
fn PublicationList(model: LedgerModel) -> impl IntoView {
    if model.entries.is_empty() {
        return view! { <section class=css::empty>"no publications match this filter"</section> }
            .into_any();
    }
    let total = model.entries.len();
    let visible = total.min(LEDGER_RENDER_LIMIT);
    view! {
        <section class=css::chain aria-label="Publication catalog">
            {model.entries.into_iter().take(visible).map(|entry| view! { <PublicationEntry entry=entry /> }).collect_view()}
            {(total > visible).then(|| view! {
                <div class=css::empty>{format!("showing newest {visible} of {total} publications")}</div>
            })}
        </section>
    }.into_any()
}

#[component]
fn PublicationEntry(entry: LedgerEntry) -> impl IntoView {
    let block_class = if entry.restricted {
        format!("{} {}", css::block, css::locked)
    } else {
        css::block.to_string()
    };

    view! {
        <article class=block_class>
            <div class=css::blockHead>
                {entry.kind_chips.iter().map(|kind| view! {
                    <span class=css::kind data-kind=kind.clone()>{kind.clone()}</span>
                }).collect_view()}
                {entry.restricted.then(|| view! {
                    <span class=css::lock data-state="restricted">"restricted"</span>
                })}
                <span class=css::date>{entry.date.clone()}</span>
            </div>
            <div class=css::blockBody>
                <span class=css::title>
                    <a href=entry.href.clone()>{entry.title.clone()}</a>
                </span>
                {entry.description.clone().map(|text| view! {
                    <span class=css::desc>{text}</span>
                })}
                <span class=css::metaLine>
                    {entry.meta_line.iter().map(|part| view! {
                        <span>{part.clone()}</span>
                    }).collect_view()}
                </span>
                {(!entry.variants.is_empty()).then(|| view! {
                    <span class=css::variantsLine aria-label="Bundle variants">
                        {entry.variants.iter().map(|variant| view! {
                            <span class=css::variantChip>{variant.clone()}</span>
                        }).collect_view()}
                    </span>
                })}
            </div>
        </article>
    }
}
