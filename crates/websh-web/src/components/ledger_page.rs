//! Ledger-style index page for content directories.

use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::mempool::{
    LedgerFilterShape, Mempool, build_mempool_model, load_mempool_files,
};
use crate::components::shared::{
    AttestationSigFooter, IdentifierStrip, MetaRow, MetaTable, MonoOverflow, MonoTone, MonoValue,
    SiteContentFrame, SiteSurface, size_summary_parts,
};
use websh_core::filesystem::{GlobalFs, RouteFrame};
use websh_core::attestation::ledger::{
    CONTENT_LEDGER_CONTENT_PATH, CONTENT_LEDGER_ROUTE, ContentLedger, ContentLedgerBlock,
};
use websh_core::mempool::{LEDGER_CATEGORIES, mempool_root};
use websh_core::domain::{NodeMetadata, VirtualPath};
use crate::utils::content_routes::content_href_for_path;
use crate::utils::format::{format_date_iso, format_size, iso_date_prefix};

stylance::import_crate_style!(css, "src/components/ledger_page.module.css");

#[derive(Clone, Debug, PartialEq, Eq)]
struct LedgerModel {
    filter: LedgerFilter,
    entries: Vec<LedgerEntry>,
    counts: BTreeMap<String, usize>,
    total_count: usize,
    encrypted_count: usize,
    head_hash: String,
    genesis_date: String,
    latest_date: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LedgerFilter {
    All,
    Category(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LedgerEntry {
    block_number: String,
    block_height: u64,
    path: String,
    href: String,
    title: String,
    description: Option<String>,
    date: String,
    category: String,
    kind: String,
    meta_line: Vec<String>,
    encrypted: bool,
    hash: String,
    previous_hash: String,
}

#[component]
pub fn LedgerPage(route: Memo<RouteFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let ledger_ctx = ctx;
    let ledger = LocalResource::new(move || {
        let ctx = ledger_ctx;
        async move { load_content_ledger(ctx).await }
    });

    let mempool_ctx = ctx;
    let mempool_files = Memo::new(move |_| load_mempool_files(mempool_ctx));

    let author_mode = Memo::new(move |_| ctx.runtime_state.with(|rs| rs.github_token_present));

    // Mempool collapse state lives at the LedgerPage level so it survives
    // filter-route changes (which re-render but don't re-mount this page).
    // Default collapsed: the chain is the primary content; pending entries
    // are opt-in.
    let mempool_collapsed = RwSignal::new(true);

    let attestation_route = Signal::derive(|| CONTENT_LEDGER_ROUTE.to_string());

    view! {
        <SiteSurface class=css::surface>
            <SiteChrome route=route />
            <SiteContentFrame class=css::page>
                <Suspense fallback=move || view! { <LedgerPending message="ledger pending".to_string() /> }>
                    {move || {
                        ledger.get().map(|result| {
                            match result {
                                Ok(artifact) => {
                                    let frame = route.get();
                                    let filter = ledger_filter_for_route(
                                        &frame.request.url_path,
                                        &frame.resolution.node_path,
                                    );
                                    let model = ctx.view_global_fs.with(|fs| {
                                        build_ledger_model(fs, &artifact, &filter)
                                    });
                                    let filter_shape = match &filter {
                                        LedgerFilter::All => LedgerFilterShape::All,
                                        LedgerFilter::Category(c) => {
                                            LedgerFilterShape::Category(c.clone())
                                        }
                                    };
                                    let mempool_root_path = mempool_root();
                                    let mempool_files_signal = mempool_files;
                                    let mempool_section = move || {
                                        let files = mempool_files_signal.get();
                                        let mempool_model = build_mempool_model(
                                            mempool_root_path,
                                            files,
                                            &filter_shape,
                                        );
                                        view! {
                                            <Mempool
                                                model=mempool_model
                                                author_mode=author_mode
                                                collapsed=mempool_collapsed
                                            />
                                        }
                                    };
                                    view! {
                                        <LedgerIdentifier model=model.clone() />
                                        <LedgerHeader model=model.clone() />
                                        <LedgerFilterBar model=model.clone() />
                                        {mempool_section}
                                        <LedgerChain model=model.clone() />
                                    }.into_any()
                                }
                                Err(error) => view! {
                                    <LedgerPending message=format!("ledger pending: {error}") />
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>
                <AttestationSigFooter route=attestation_route show_pending=Signal::derive(|| true) />
            </SiteContentFrame>
        </SiteSurface>
    }
}

// Phase 5 removed the browser-side promote flow in favor of
// `websh-cli mempool promote`. DeployHint / PartialWarning / PromoteStatusBanner
// have been deleted along with PromoteConfirmModal.

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
                " encrypted "
                <span class=css::num>{model.encrypted_count}</span>
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
            <LedgerFilterLink label="all" href="/#/ledger" count=model.total_count active=model.filter.is_all() />
            {LEDGER_CATEGORIES.iter().map(|category| {
                let href = format!("/#/{category}");
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

    let last_index = model.entries.len() - 1;
    let rows = model
        .entries
        .iter()
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
                    block_number=entry.block_number.clone()
                    previous_hash=entry.previous_hash.clone()
                />
                <LedgerConnector broken=broken hidden=hidden />
            }
        })
        .collect_view();

    view! {
        <section class=css::chain aria-label="Ledger chain">
            {rows}
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
    let block_class = if entry.encrypted {
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
                <span class=css::kind data-kind=entry.kind.clone()>{entry.kind.clone()}</span>
                {entry.encrypted.then(|| view! {
                    <span class=css::lock data-state="encrypted">"encrypted"</span>
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

fn ledger_filter_for_route(request_path: &str, node_path: &VirtualPath) -> LedgerFilter {
    if request_path.trim_matches('/') == "ledger" {
        return LedgerFilter::All;
    }
    node_path
        .segments()
        .next()
        .map(|segment| LedgerFilter::Category(segment.to_string()))
        .unwrap_or(LedgerFilter::All)
}

async fn load_content_ledger(ctx: AppContext) -> Result<ContentLedger, String> {
    let path = VirtualPath::from_absolute(format!("/{CONTENT_LEDGER_CONTENT_PATH}"))
        .expect("ledger path is absolute");
    let body = ctx
        .read_text(&path)
        .await
        .map_err(|error| error.to_string())?;
    let ledger: ContentLedger = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    ledger.validate()?;
    Ok(ledger)
}

fn build_ledger_model(fs: &GlobalFs, ledger: &ContentLedger, filter: &LedgerFilter) -> LedgerModel {
    let all_entries = ledger
        .blocks
        .iter()
        .rev()
        .filter_map(|block| ledger_entry_for_block(fs, block))
        .collect::<Vec<_>>();
    let total_count = all_entries.len();

    let mut counts = BTreeMap::new();
    for category in LEDGER_CATEGORIES {
        counts.insert((*category).to_string(), 0usize);
    }
    for entry in &all_entries {
        *counts.entry(entry.category.clone()).or_default() += 1;
    }

    let entries = all_entries
        .iter()
        .filter(|entry| filter.includes(entry))
        .cloned()
        .collect::<Vec<_>>();
    let encrypted_count = entries.iter().filter(|entry| entry.encrypted).count();
    let head_hash = ledger.chain_head.clone();
    // `latest_date` mirrors the active filter — whatever the user is
    // looking at, the "last appended" badge tracks that view.
    let latest_date = entries
        .first()
        .map(|entry| entry.date.clone())
        .unwrap_or_else(|| "—".to_string());
    // `genesis_date`, by contrast, is a property of the chain itself —
    // not of the filtered view. Compute it from every block's parsed
    // date (skipping `"undated"` and other non-ISO strings) so the
    // header always reports the actual earliest dated block, regardless
    // of which category is selected.
    let genesis_date = all_entries
        .iter()
        .filter_map(|entry| iso_date_prefix(&entry.date).map(str::to_string))
        .min()
        .unwrap_or_else(|| "—".to_string());

    LedgerModel {
        filter: filter.clone(),
        entries,
        counts,
        total_count,
        encrypted_count,
        head_hash,
        genesis_date,
        latest_date,
    }
}

fn ledger_entry_for_block(fs: &GlobalFs, block: &ContentLedgerBlock) -> Option<LedgerEntry> {
    let entry = &block.entry;
    let node_path = VirtualPath::from_absolute(format!("/{}", entry.path)).ok()?;
    let node_meta = fs.node_metadata(&node_path);
    let fallback_title = fallback_file_title(&entry.path);
    let title = node_meta
        .and_then(|meta| meta.title())
        .map(str::to_string)
        .unwrap_or(fallback_title);
    // `meta.description()` resolves authored ?? derived. With sync's
    // first-paragraph auto-extraction, derived is populated for every
    // markdown file. Returning None here lets the renderer omit the
    // description line entirely instead of substituting a low-value
    // fallback like "N files · path".
    let description = node_meta
        .and_then(|meta| meta.description())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    let date = node_meta
        .and_then(|meta| meta.date())
        .map(str::to_string)
        .or_else(|| node_meta.and_then(|meta| meta.modified_at().map(format_date_iso)))
        .unwrap_or_else(|| "undated".to_string());
    let category = entry.category.as_str().to_string();
    let kind = kind_for_entry(&category, &entry.path);
    let tags = node_meta.map(NodeMetadata::tags_owned).unwrap_or_default();
    let size = node_meta
        .and_then(|meta| meta.size_bytes())
        .or_else(|| Some(entry.content_files.iter().map(|file| file.bytes).sum()));
    // Kind-aware metric chunks (e.g. ["9 min"], ["12 pages"],
    // ["1920×1080"], or ["2,140 words", "9 min"] for prose pages) when
    // the sync-time derived fields are available. Empty Vec for kinds
    // without a natural metric or unsynced files — caller falls back to
    // byte size in that case.
    let summary_parts = node_meta
        .map(|meta| {
            size_summary_parts(
                meta.effective_kind(),
                meta.word_count(),
                meta.page_count(),
                meta.image_dimensions(),
            )
        })
        .unwrap_or_default();
    let encrypted = node_meta.and_then(|meta| meta.access()).is_some();

    Some(LedgerEntry {
        block_number: format!("{:04}", block.height),
        block_height: block.height,
        path: entry.path.clone(),
        href: content_href_for_path(&entry.path),
        title,
        description,
        date,
        category,
        kind,
        meta_line: meta_line_for_entry(summary_parts, size, &tags),
        encrypted,
        hash: block.block_sha256.clone(),
        previous_hash: block.prev_block_sha256.clone(),
    })
}

/// Build the meta line shown under each ledger entry.
///
/// Prefers the kind-aware summary chunks (["2,140 words", "9 min"],
/// ["12 pages"], ["1920×1080"]) because they tell a reader what it
/// costs to consume the entry. Falls back to byte size when those
/// chunks are absent (kinds without a natural metric, or unsynced
/// files). Tags follow up to three. The renderer wraps each element in
/// its own `<span>`; the surrounding `.metaLine` CSS draws a `·`
/// between every adjacent pair so all gaps are uniform regardless of
/// whether the boundary is summary↔summary, summary↔tag, or tag↔tag.
fn meta_line_for_entry(
    summary_parts: Vec<String>,
    size: Option<u64>,
    tags: &[String],
) -> Vec<String> {
    let mut out = summary_parts;
    if out.is_empty()
        && let Some(bytes) = size
    {
        out.push(format_size(Some(bytes), false));
    }
    out.extend(tags.iter().take(3).cloned());
    if out.is_empty() {
        out.push("content".to_string());
    }
    out
}

fn fallback_file_title(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|name| name.split('.').next())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn kind_for_entry(category: &str, path: &str) -> String {
    match category {
        "papers" => "paper",
        "projects" => "project",
        "talks" => "talk",
        "writing" => "writing",
        _ if path.ends_with(".asc") => "key",
        _ if path.ends_with(".toml") || path.ends_with(".json") => "data",
        _ => "note",
    }
    .to_string()
}

impl LedgerFilter {
    fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }

    fn matches(&self, category: &str) -> bool {
        matches!(self, Self::Category(active) if active == category)
    }

    fn includes(&self, entry: &LedgerEntry) -> bool {
        match self {
            Self::All => true,
            Self::Category(category) if LEDGER_CATEGORIES.contains(&category.as_str()) => {
                entry.category == *category
            }
            Self::Category(category) => entry.path.starts_with(&format!("{category}/")),
        }
    }
}
