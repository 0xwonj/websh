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
    IdentifierStrip, MetaRow, MetaTable, MonoOverflow, MonoTone, MonoValue, PageSigFooter,
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
    // Default collapsed: the chain is the primary content; pending entries
    // are opt-in.
    let mempool_collapsed = RwSignal::new(true);

    let attestation_route = Signal::derive(move || route.get().request.url_path);

    view! {
        <SiteSurface class=css::surface>
            <SiteChrome route=route />
            <SiteContentFrame class=css::page>
                <crate::shared::components::MountStatusNotice path=Signal::derive(VirtualPath::root) />
                {move || {
                    let Some(release) = ctx.content.release() else {
                        return view! { <LedgerPending message="ledger pending" /> }.into_any();
                    };
                    let frame = route.get();
                    let filter = ledger_filter_for_route(&frame.request.url_path, &frame.resolution.node_path);
                    let chain = match websh_core::publication::PublicationChain::from_manifest(release.manifest()) {
                        Ok(chain) => chain,
                        Err(error) => return view! {
                            <LedgerPending message=format!("ledger pending: {error}") />
                        }.into_any(),
                    };
                    let model = ctx.content.with_root_fs(|fs| build_ledger_model(fs, &chain, &filter));
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
                        <LedgerChain model=model />
                    }.into_any()
                }}
                <PageSigFooter route=attestation_route />
            </SiteContentFrame>
        </SiteSurface>
    }
}

#[component]
fn LedgerIdentifier(model: LedgerModel) -> impl IntoView {
    view! {
        <IdentifierStrip>
            <span>"websh chain"</span>
            <span>{format!("last appended {}", model.latest_date)}</span>
        </IdentifierStrip>
    }
}

#[component]
fn LedgerHeader(model: LedgerModel) -> impl IntoView {
    let head_hash = model.head_hash.clone();
    let head_hash_label = format!("chain head {head_hash}");

    view! {
        <MetaTable class=css::ledgerHead aria_label="Ledger metadata">
            <MetaRow label="blocks" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <span class=css::num>{model.entries.len()}</span>
                <span class=css::faintSep>" · "</span>
                " restricted "
                <span class=css::num>{model.restricted_count}</span>
            </MetaRow>
            <MetaRow label="head" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <span aria-label=head_hash_label>
                    <MonoValue
                        value=head_hash.clone()
                        tone=MonoTone::Hex
                        overflow=MonoOverflow::ResponsiveMiddle {
                            narrow: Some((12, 6)),
                            medium: Some((18, 8)),
                            wide: Some((24, 12)),
                        }
                        title=head_hash
                    />
                </span>
                " "
                <span class=css::ok aria-label="hash ok" title="hash ok">"✓"</span>
            </MetaRow>
            <MetaRow label="genesis" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <code>{model.genesis_date}</code>
            </MetaRow>
            <MetaRow label="status" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <span class=css::live>"appendable"</span>
            </MetaRow>
        </MetaTable>
    }
}

#[component]
fn LedgerPending(message: impl Into<String>) -> impl IntoView {
    view! {
        <IdentifierStrip>
            <span>"~"</span>
            <span>"ledger pending"</span>
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
fn LedgerChain(model: LedgerModel) -> impl IntoView {
    if model.entries.is_empty() {
        return view! {
            <section class=css::empty>
                "no blocks match this ledger filter"
            </section>
        }
        .into_any();
    }

    let total_entries = model.entries.len();
    let visible_count = total_entries.min(LEDGER_RENDER_LIMIT);
    let last_index = total_entries - 1;
    let rows = model
        .entries
        .iter()
        .take(visible_count)
        .enumerate()
        .map(|(index, entry)| {
            let hidden = if index < last_index {
                model
                    .entries
                    .get(index + 1)
                    .map(|next| entry.block_height.saturating_sub(next.block_height))
                    .map(|delta| delta.saturating_sub(1))
                    .unwrap_or(0)
            } else {
                entry.block_height.saturating_sub(1)
            };
            let broken = hidden > 0;
            view! {
                <LedgerBlock
                    entry=entry.clone()
                    block_number=format!("{:04}", entry.block_height)
                    previous_hash=entry.previous_hash.clone()
                />
                <LedgerConnector broken=broken hidden=hidden />
            }
        })
        .collect_view();
    let limited = total_entries > visible_count;

    view! {
        <section class=css::chain aria-label="Ledger chain">
            {rows}
            {limited.then(|| view! {
                <div class=css::empty>
                    {format!("showing newest {visible_count} of {total_entries} blocks")}
                </div>
            })}
            <div class=css::genesis>
                <span class=css::genesisLabel>"genesis"</span>
                <span class=css::genesisQuote>{model.genesis_date.clone()}</span>
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn LedgerConnector(#[prop(optional)] broken: bool, #[prop(optional)] hidden: u64) -> impl IntoView {
    let class = if broken {
        format!("{} {}", css::connector, css::connectorBroken)
    } else {
        css::connector.to_string()
    };
    let label = (broken && hidden > 0).then(|| format!("{hidden} hidden"));
    view! {
        <div class=class aria-label=label.clone() title=label></div>
    }
}

#[component]
fn LedgerBlock(entry: LedgerEntry, block_number: String, previous_hash: String) -> impl IntoView {
    let block_class = if entry.restricted {
        format!("{} {}", css::block, css::locked)
    } else {
        css::block.to_string()
    };
    let previous_hash_label = format!("previous block hash {previous_hash}");
    let block_hash = entry.hash.clone();
    let block_hash_label = format!("block hash {block_hash}");

    view! {
        <article class=block_class>
            <div class=css::blockHead>
                <span class=css::blockNumber>{format!("block {block_number}")}</span>
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
                    {model::entry_meta_line(&entry).iter().map(|part| view! {
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
            <div class=css::blockFoot>
                <span class=css::prev aria-label=previous_hash_label>
                    <span class=css::footKey>"prev"</span>
                    <MonoValue
                        value=previous_hash.clone()
                        tone=MonoTone::Hex
                        overflow=MonoOverflow::ResponsiveMiddle {
                            narrow: Some((6, 4)),
                            medium: Some((12, 6)),
                            wide: Some((18, 8)),
                        }
                        title=previous_hash
                    />
                </span>
                <span class=css::hashCell aria-label=block_hash_label>
                    <span class=css::footKey>"hash"</span>
                    <MonoValue
                        value=block_hash.clone()
                        tone=MonoTone::Hex
                        overflow=MonoOverflow::ResponsiveMiddle {
                            narrow: Some((6, 4)),
                            medium: Some((12, 6)),
                            wide: Some((18, 8)),
                        }
                        title=block_hash
                    />
                </span>
                <span class=css::sig aria-label="hash ok" title="hash ok">
                    "✓"
                </span>
            </div>
        </article>
    }
}
