//! Ledger-style index page for content directories.

use std::collections::BTreeMap;

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::components::mempool::{
    ComposeModal, ComposeMode, LedgerFilterShape, Mempool, MempoolEntry, MempoolPreviewModal,
    build_mempool_model, load_mempool_files, mempool_root,
};
use crate::components::shared::{
    AttestationSigFooter, MetaRow, MetaTable, SiteContentFrame, SiteSurface,
};
use crate::core::engine::{GlobalFs, RouteFrame};
use crate::crypto::ack::short_hash;
use crate::crypto::ledger::{
    CONTENT_LEDGER_CONTENT_PATH, CONTENT_LEDGER_ROUTE, ContentLedgerArtifact, ContentLedgerEntry,
};
use crate::models::{FsEntry, VirtualPath};
use crate::utils::content_routes::content_href_for_path;
use crate::utils::format::{format_date_iso, format_size};

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
    block_height: usize,
    path: String,
    href: String,
    title: String,
    description: String,
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
    let ledger_ctx = ctx.clone();
    let ledger = LocalResource::new(move || {
        let ctx = ledger_ctx.clone();
        async move { load_content_ledger(ctx).await }
    });

    let mempool_ctx = ctx.clone();
    let mempool_refresh = RwSignal::new(0u32);
    let mempool_files = LocalResource::new(move || {
        let ctx = mempool_ctx.clone();
        let _ = mempool_refresh.get();
        async move { load_mempool_files(ctx).await }
    });

    let (preview_open, set_preview_open) = signal(None::<VirtualPath>);
    let (compose_open, set_compose_open) = signal(None::<ComposeMode>);

    let author_mode = Memo::new({
        let ctx = ctx.clone();
        move |_| ctx.runtime_state.with(|rs| rs.github_token_present)
    });

    let select_ctx = ctx.clone();
    let on_mempool_select = Callback::new(move |entry: MempoolEntry| {
        if author_mode.get() {
            let ctx = select_ctx.clone();
            let path = entry.path.clone();
            spawn_local(async move {
                match ctx.read_text(&path).await {
                    Ok(body) => set_compose_open.set(Some(ComposeMode::Edit { path, body })),
                    Err(error) => leptos::logging::warn!(
                        "mempool: failed to read {} for edit: {error}",
                        path.as_str()
                    ),
                }
            });
        } else {
            set_preview_open.set(Some(entry.path));
        }
    });

    let on_compose_new = Callback::new(move |_| {
        let default_category = filter_category_from_route(&route.get());
        set_compose_open.set(Some(ComposeMode::New { default_category }));
    });

    let on_compose_saved = Callback::new(move |_| {
        mempool_refresh.update(|n| *n += 1);
    });

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
                                    let filter = ledger_filter_for_route(
                                        &route.get().request.url_path,
                                        &route.get().resolution.node_path,
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
                                    let on_select = on_mempool_select;
                                    let mempool_files_signal = mempool_files;
                                    let mempool_section = move || {
                                        mempool_files_signal.get().map(|files| {
                                            let mempool_model = build_mempool_model(
                                                &mempool_root_path,
                                                files.clone(),
                                                &filter_shape,
                                            );
                                            view! {
                                                <Mempool
                                                    model=mempool_model
                                                    on_select=on_select
                                                />
                                            }
                                        })
                                    };
                                    view! {
                                        <LedgerIdentifier model=model.clone() />
                                        <LedgerHeader model=model.clone() />
                                        <LedgerFilterBar
                                            model=model.clone()
                                            author_mode=author_mode
                                            on_compose=on_compose_new
                                        />
                                        <Suspense fallback=|| view! { <span></span> }>
                                            {mempool_section}
                                        </Suspense>
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
                <AttestationSigFooter route=attestation_route show_pending=true />
            </SiteContentFrame>
            <MempoolPreviewModal open_path=preview_open set_open_path=set_preview_open />
            <ComposeModal
                open=compose_open
                set_open=set_compose_open
                on_saved=on_compose_saved
            />
        </SiteSurface>
    }
}

#[component]
fn LedgerIdentifier(model: LedgerModel) -> impl IntoView {
    view! {
        <div class=css::identifier>
            <span class=css::id>
                <b>"websh chain"</b>
            </span>
            <span class=css::rev>{format!("last appended {}", model.latest_date)}</span>
        </div>
    }
}

#[component]
fn LedgerHeader(model: LedgerModel) -> impl IntoView {
    view! {
        <MetaTable class=css::ledgerHead aria_label="Ledger metadata">
            <MetaRow label="blocks" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <span class=css::num>{model.entries.len()}</span>
                <span class=css::faintSep>" · "</span>
                " encrypted "
                <span class=css::num>{model.encrypted_count}</span>
            </MetaRow>
            <MetaRow label="head" row_class=css::headRow key_class=css::headKey value_class=css::headVal>
                <code>{model.head_hash}</code>
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
        <div class=css::identifier>
            <span class=css::id><b>"~"</b></span>
            <span class=css::rev>"ledger pending"</span>
        </div>
        <section class=css::empty>
            {message.into()}
        </section>
    }
}

#[component]
fn LedgerFilterBar(
    model: LedgerModel,
    author_mode: Memo<bool>,
    #[prop(into)] on_compose: Callback<()>,
) -> impl IntoView {
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
            <span class=css::filterBarSlot>
                <Show when=move || author_mode.get()>
                    <button
                        class=css::composeButton
                        type="button"
                        aria-label="Compose new mempool entry"
                        on:click=move |_| on_compose.run(())
                    >
                        "+ compose"
                    </button>
                </Show>
            </span>
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
        <a class=class_name href=href.into()>
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
                "no entries match this ledger filter"
            </section>
        }
        .into_any();
    }

    let rows = model
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let gap = model
                .entries
                .get(index + 1)
                .map(|next| entry.block_height.saturating_sub(next.block_height))
                .filter(|delta| *delta > 1)
                .map(|delta| delta - 1);
            view! {
                <LedgerBlock
                    entry=entry.clone()
                    block_number=entry.block_number.clone()
                    previous_hash=entry.previous_hash.clone()
                />
                {gap.map(|hidden| view! { <LedgerGap hidden=hidden /> })}
            }
        })
        .collect_view();

    let trailing_gap = model
        .entries
        .last()
        .map(|last| last.block_height.saturating_sub(1))
        .filter(|hidden| *hidden > 0);

    view! {
        <section class=css::chain aria-label="Ledger entries">
            {rows}
        </section>
        {trailing_gap.map(|hidden| view! { <LedgerGap hidden=hidden /> })}
        <div class=css::genesis>
            <span class=css::genesisLabel>"genesis"</span>
            <span class=css::hashCell>
                <span class=css::footKey>"hash"</span>
                <span class=css::hash>"0x0000…0000"</span>
            </span>
            <span class=css::sig aria-hidden="true">"✓"</span>
            <span class=css::genesisQuote>{model.genesis_date.clone()}</span>
        </div>
    }
    .into_any()
}

#[component]
fn LedgerGap(hidden: usize) -> impl IntoView {
    let label = format!("{hidden} hidden");
    view! {
        <div class=css::gap aria-label=label.clone() title=label></div>
    }
}

#[component]
fn LedgerBlock(entry: LedgerEntry, block_number: String, previous_hash: String) -> impl IntoView {
    let block_class = if entry.encrypted {
        format!("{} {}", css::block, css::locked)
    } else {
        css::block.to_string()
    };

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
                <span class=css::desc>{entry.description.clone()}</span>
                <span class=css::metaLine>
                    {entry.meta_line.iter().map(|part| view! {
                        <span>{part.clone()}</span>
                    }).collect_view()}
                </span>
            </div>
            <div class=css::blockFoot>
                <span class=css::prev>
                    <span class=css::footKey>"prev"</span>
                    <span class=css::hash>{previous_hash}</span>
                </span>
                <span class=css::hashCell>
                    <span class=css::footKey>"hash"</span>
                    <span class=css::hash>{entry.hash}</span>
                </span>
                <span class=css::sig aria-label="hash ok" title="hash ok">
                    "✓"
                </span>
            </div>
        </article>
    }
}

fn filter_category_from_route(route: &RouteFrame) -> Option<String> {
    match ledger_filter_for_route(&route.request.url_path, &route.resolution.node_path) {
        LedgerFilter::Category(c) if LEDGER_CATEGORIES.contains(&c.as_str()) => Some(c),
        _ => None,
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

async fn load_content_ledger(ctx: AppContext) -> Result<ContentLedgerArtifact, String> {
    let path = VirtualPath::from_absolute(format!("/{CONTENT_LEDGER_CONTENT_PATH}"))
        .expect("ledger path is absolute");
    let body = ctx
        .read_text(&path)
        .await
        .map_err(|error| error.to_string())?;
    let ledger: ContentLedgerArtifact =
        serde_json::from_str(&body).map_err(|error| error.to_string())?;
    ledger.validate()?;
    Ok(ledger)
}

fn build_ledger_model(
    fs: &GlobalFs,
    ledger: &ContentLedgerArtifact,
    filter: &LedgerFilter,
) -> LedgerModel {
    let mut all_entries = ledger
        .entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let previous_hash = index
                .checked_sub(1)
                .and_then(|previous| ledger.entries.get(previous))
                .map(|entry| short_hash(&entry.entry_sha256))
                .unwrap_or_else(|| "0x0000…0000".to_string());
            ledger_entry_for_artifact_entry(fs, entry, index + 1, previous_hash)
        })
        .collect::<Vec<_>>();
    // The on-disk ledger is already sorted canonically by `(date asc, path asc)`
    // by the CLI, so reversing here yields newest-first display while keeping
    // each block's `prev` aligned with the block visually beneath it.
    all_entries.reverse();
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
    let head_hash = if filter.is_all() {
        short_hash(&ledger.ledger_sha256)
    } else {
        entries
            .first()
            .map(|entry| entry.hash.clone())
            .unwrap_or_else(|| "—".to_string())
    };
    let latest_date = entries
        .first()
        .map(|entry| entry.date.clone())
        .unwrap_or_else(|| "—".to_string());
    let genesis_date = entries
        .last()
        .map(|entry| entry.date.clone())
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

fn ledger_entry_for_artifact_entry(
    fs: &GlobalFs,
    entry: &ContentLedgerEntry,
    block_height: usize,
    previous_hash: String,
) -> Option<LedgerEntry> {
    let node_path = VirtualPath::from_absolute(format!("/{}", entry.path)).ok()?;
    let node_meta = fs.node_metadata(&node_path);
    let (manifest_title, file_meta) = match fs.get_entry(&node_path) {
        Some(FsEntry::File {
            description, meta, ..
        }) => (Some(description.clone()), Some(meta.clone())),
        _ => (None, None),
    };
    let fallback_title = fallback_file_title(&entry.path);
    let title = node_meta
        .and_then(|meta| meta.title.clone())
        .or(manifest_title)
        .unwrap_or(fallback_title);
    let description = node_meta
        .and_then(|meta| meta.description.clone())
        .unwrap_or_else(|| description_for_entry(entry));
    let date = node_meta
        .and_then(|meta| meta.date.clone())
        .or_else(|| file_meta.as_ref().and_then(|meta| meta.date.clone()))
        .or_else(|| {
            file_meta
                .as_ref()
                .and_then(|meta| meta.modified.map(format_date_iso))
        })
        .unwrap_or_else(|| "undated".to_string());
    let category = category_for_path(&entry.path);
    let kind = kind_for_entry(&category, &entry.path);
    let tags = if node_meta.is_some_and(|meta| !meta.tags.is_empty()) {
        node_meta.map(|meta| meta.tags.clone()).unwrap_or_default()
    } else {
        file_meta
            .as_ref()
            .map(|meta| meta.tags.clone())
            .unwrap_or_default()
    };
    let size = file_meta
        .as_ref()
        .and_then(|meta| meta.size)
        .or_else(|| Some(entry.content.files.iter().map(|file| file.bytes).sum()));
    let encrypted = file_meta
        .as_ref()
        .and_then(|meta| meta.access.as_ref())
        .is_some();

    Some(LedgerEntry {
        block_number: format!("{block_height:04}"),
        block_height,
        path: entry.path.clone(),
        href: content_href_for_path(&entry.path),
        title,
        description,
        date,
        category,
        kind,
        meta_line: meta_line_for_entry(size, &tags),
        encrypted,
        hash: short_hash(&entry.entry_sha256),
        previous_hash,
    })
}

fn description_for_entry(entry: &ContentLedgerEntry) -> String {
    let mut parts = Vec::new();
    if entry.content.files.len() > 1 {
        parts.push(format!("{} files", entry.content.files.len()));
    }
    parts.push(entry.path.clone());
    parts.join(" · ")
}

fn meta_line_for_entry(size: Option<u64>, tags: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(size) = size {
        out.push(format_size(Some(size), false));
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

fn category_for_path(path: &str) -> String {
    match path.trim_start_matches('/').split('/').next().unwrap_or("") {
        "writing" => "writing",
        "projects" => "projects",
        "papers" => "papers",
        "talks" => "talks",
        _ => "misc",
    }
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

