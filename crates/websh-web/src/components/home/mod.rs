//! Built-in homepage.
//!
//! The root URL is an application surface, not a filesystem document reader.
//! Content routes such as `/#/index.html` remain available through the
//! filesystem router.

use leptos::prelude::*;
use serde::Deserialize;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::markdown::InlineMarkdownView;
use crate::components::shared::{
    IdentifierStrip, MetaRow as SharedMetaRow, MetaTable as SharedMetaTable, SiteContentFrame,
    SiteSurface,
};
use crate::core::engine::{GlobalFs, RouteFrame};
use crate::crypto::attestation::AttestationArtifact;
use crate::models::VirtualPath;
use crate::utils::content_routes::content_href_for_path;
use crate::utils::render_inline_markdown;

stylance::import_crate_style!(
    #[allow(dead_code)]
    pub(super) css,
    "src/components/home/home.module.css"
);

mod sections;
use sections::{Acknowledgements, Appendices, PageFooter};

pub(super) const PUBLIC_KEY_BLOCK: &str = include_str!("../../../../../content/keys/wonjae.asc");

const TOC_ITEMS: &[TocItem] = &[
    TocItem {
        num: "1",
        name: "about",
        href: "/#/about",
        meta: "bio · cv",
        count_root: None,
    },
    TocItem {
        num: "2",
        name: "writing",
        href: "/#/writing",
        meta: "",
        count_root: Some("/writing"),
    },
    TocItem {
        num: "3",
        name: "projects",
        href: "/#/projects",
        meta: "",
        count_root: Some("/projects"),
    },
    TocItem {
        num: "4",
        name: "papers",
        href: "/#/papers",
        meta: "",
        count_root: Some("/papers"),
    },
    TocItem {
        num: "5",
        name: "talks",
        href: "/#/talks",
        meta: "",
        count_root: Some("/talks"),
    },
    TocItem {
        num: "6",
        name: "misc",
        href: "/#/misc",
        meta: "",
        count_root: Some("/misc"),
    },
];

#[derive(Clone, Copy)]
struct TocItem {
    num: &'static str,
    name: &'static str,
    href: &'static str,
    meta: &'static str,
    count_root: Option<&'static str>,
}

#[derive(Clone, Debug, Deserialize)]
struct NowDocument {
    items: Vec<NowItem>,
}

#[derive(Clone, Debug, Deserialize)]
struct NowItem {
    date: String,
    text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecentItem {
    kind: String,
    date: String,
    title: String,
    href: String,
    tag: String,
}

#[component]
pub fn HomePage(route: Memo<RouteFrame>) -> impl IntoView {
    view! {
        <SiteSurface class=css::home>
            <SiteChrome route=route />
            <SiteContentFrame class=css::page>
                <HeroHeader />
                <HomepageMetaTable />
                <AbstractSection />
                <TocSection />
                <IntroSection />
                <RecentFeed />
                <Appendices />
                <Acknowledgements />
                <PageFooter />
            </SiteContentFrame>
        </SiteSurface>
    }
}

#[component]
fn HeroHeader() -> impl IntoView {
    let today = current_homepage_date();
    let paper_id = format!("Paper {}", compact_homepage_date(&today));
    let revised = format!("last revised {}", homepage_issued_at().unwrap_or(today));

    view! {
        <IdentifierStrip>
            <span>{paper_id}</span>
            <span>{revised}</span>
        </IdentifierStrip>

        <h1 class=css::title>
            "wonjae.eth"
            <span class=css::tagline>"A Homepage, Formalised"</span>
        </h1>

        <div class=css::authors>
            "Wonjae Choi"<sup class=css::star>"*"</sup>
        </div>
        <div class=css::aff>
            <sup>"*"</sup>" Seoul National University "
            <span class=css::dotSep>" · "</span>
            <a href="mailto:wonjae@snu.ac.kr">"wonjae@snu.ac.kr"</a>
        </div>
    }
}

fn homepage_issued_at() -> Option<String> {
    AttestationArtifact::from_homepage_asset()
        .ok()
        .and_then(|artifact| {
            artifact
                .subject_for_route("/")
                .map(|subject| subject.issued_at().to_string())
        })
}

fn current_homepage_date() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new_0();
        format!(
            "{:04}-{:02}-{:02}",
            date.get_full_year(),
            date.get_month() + 1,
            date.get_date()
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::utils::format::format_date_iso(crate::utils::current_timestamp() / 1000)
    }
}

fn compact_homepage_date(date: &str) -> String {
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day), None)
            if year.len() == 4
                && month.len() == 2
                && day.len() == 2
                && year.chars().all(|ch| ch.is_ascii_digit())
                && month.chars().all(|ch| ch.is_ascii_digit())
                && day.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            format!("{year}/{month}{day}")
        }
        _ => date.to_string(),
    }
}

#[component]
fn HomepageMetaTable() -> impl IntoView {
    view! {
        <SharedMetaTable class=css::meta aria_label="ePrint metadata">
            <SharedMetaRow
                label="Category"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                <span class=css::tag>"cs.CR"</span>
                <span class=css::tag>"cs.PL"</span>
                <span class=css::tag>"cs.DC"</span>
            </SharedMetaRow>
            <SharedMetaRow
                label="Keywords"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                <span class=css::kwFull>"zero-knowledge proofs"</span>
                <span class=css::kwCompact>"zkp"</span>
                ", compilers, Ethereum"
            </SharedMetaRow>
            <SharedMetaRow
                label="Availability"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                <span class=css::availFull>
                    <span class=css::dim>"ens "</span>
                    <a href="https://wonjae.eth.limo">"wonjae.eth"</a>
                </span>
                <a class=css::availCompact href="https://wonjae.eth.limo">
                    <span class=css::dim>"ens"</span>
                </a>
                <span class=css::dotSep>" · "</span>
                <span class=css::availFull>
                    <span class=css::dim>"email "</span>
                    <a href="mailto:wonjae@snu.ac.kr">"wonjae@snu.ac.kr"</a>
                </span>
                <a class=css::availCompact href="mailto:wonjae@snu.ac.kr">
                    <span class=css::dim>"email"</span>
                </a>
                <span class=css::dotSep>" · "</span>
                <span class=css::availFull>
                    <span class=css::dim>"github "</span>
                    <a href="https://github.com/0xwonj">"0xwonj"</a>
                </span>
                <a class=css::availCompact href="https://github.com/0xwonj">
                    <span class=css::dim>"github"</span>
                </a>
                <span class=css::dotSep>" · "</span>
                <span class=css::availFull>
                    <span class=css::dim>"linkedin "</span>
                    <a href="https://www.linkedin.com/in/wonj">"wonjaechoi"</a>
                </span>
                <a class=css::availCompact href="https://www.linkedin.com/in/wonj">
                    <span class=css::dim>"linkedin"</span>
                </a>
            </SharedMetaRow>
            <SharedMetaRow
                label="Status"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                <span class=css::live>"accepting revisions"</span>
            </SharedMetaRow>
        </SharedMetaTable>
    }
}

#[component]
fn AbstractSection() -> impl IntoView {
    view! {
        <h2 class=css::sectionTitle data-n="">"Abstract"</h2>
        <p>
            "We present a personal homepage, formalised. The author is a PhD student working on "
            <em>"zero-knowledge proofs"</em>", "<em>"compiler design"</em>", and "<em>"Ethereum"</em>".
            The site is a virtual filesystem; "<em>"websh"</em>" is the shell that mounts it."
        </p>

        <NowSection />
    }
}

#[component]
fn NowSection() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let now = LocalResource::new(move || async move {
        let path = VirtualPath::from_absolute("/now.toml").expect("constant path");
        if !ctx.view_global_fs.with(|fs| fs.exists(&path)) {
            return None;
        }

        ctx.read_text(&path)
            .await
            .ok()
            .and_then(|body| parse_now_toml(&body).ok())
    });

    view! {
        {move || {
            now.get().flatten().map(|doc| {
                let timestamp = latest_now_date(&doc.items)
                    .map(|date| format!("last touched {date}"))
                    .unwrap_or_default();

                view! {
                    <div class=css::nowInline>
                        <p class=css::nowFormalLead><em>"Now"</em>":"</p>
                        <ul class=css::nowFormal>
                            {doc.items.into_iter().map(|item| {
                                let rendered = render_inline_markdown(&item.text);
                                let rendered = Signal::derive(move || rendered.clone());
                                view! {
                                    <li><InlineMarkdownView rendered=rendered /></li>
                                }
                            }).collect_view()}
                        </ul>
                        <p class=css::ts>{timestamp}</p>
                    </div>
                }
            })
        }}
    }
}

fn parse_now_toml(body: &str) -> Result<NowDocument, String> {
    let mut doc: NowDocument = toml::from_str(body).map_err(|error| error.to_string())?;

    doc.items = doc
        .items
        .into_iter()
        .map(|mut item| {
            item.date = item.date.trim().to_string();
            item.text = item.text.trim().to_string();
            item
        })
        .filter(|item| !item.date.is_empty() && !item.text.is_empty())
        .collect();

    if doc.items.is_empty() {
        return Err("now.toml must contain at least one item".to_string());
    }

    Ok(doc)
}

fn latest_now_date(items: &[NowItem]) -> Option<String> {
    items
        .iter()
        .map(|item| item.date.as_str())
        .max()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_now_toml_trims_and_filters_items() {
        let doc = parse_now_toml(
            r#"
[[items]]
date = " 2026-04-25 "
text = " content-backed now section "

[[items]]
date = "2026-04-26"
text = "newer content-backed item"

[[items]]
date = ""
text = "also ignored"
"#,
        )
        .expect("valid now.toml");

        assert_eq!(doc.items.len(), 2);
        assert_eq!(doc.items[0].date, "2026-04-25");
        assert_eq!(doc.items[0].text, "content-backed now section");
        assert_eq!(latest_now_date(&doc.items).as_deref(), Some("2026-04-26"));
    }

    #[test]
    fn parse_now_toml_rejects_empty_items() {
        assert!(parse_now_toml("[[items]]\ndate = \"\"\ntext = \"\"").is_err());
    }

    #[test]
    fn compact_homepage_date_formats_iso_date() {
        assert_eq!(compact_homepage_date("2026-04-26"), "2026/0426");
        assert_eq!(compact_homepage_date("not-a-date"), "not-a-date");
    }

    #[test]
    fn homepage_issued_at_uses_homepage_attestation_subject() {
        assert!(homepage_issued_at().is_some());
    }

    #[test]
    fn recent_items_use_folder_category_metadata_and_content_route() {
        use crate::core::storage::{ScannedFile, ScannedSubtree};
        use crate::models::{EntryExtensions, Fields, NodeKind, NodeMetadata, SCHEMA_VERSION};

        let make_meta = |date: &str, tags: &[&str]| NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            authored: Fields {
                date: Some(date.to_string()),
                tags: Some(tags.iter().map(|t| t.to_string()).collect()),
                ..Fields::default()
            },
            derived: Fields::default(),
        };

        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "projects/websh.md".to_string(),
                    meta: make_meta("2026-04-22", &["local app", "rust"]),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "papers/tabula.md".to_string(),
                    meta: make_meta("2026-04-26", &["EuroSys 2027", "systems"]),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: Vec::new(),
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .expect("mount snapshot");

        let items = recent_items_from_fs(&fs);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, "paper");
        assert_eq!(items[0].href, "/#/papers/tabula");
        assert_eq!(items[0].tag, "EuroSys 2027");
        assert_eq!(items[1].kind, "project");
    }

    #[test]
    fn toc_counts_visible_content_files_under_each_directory() {
        use crate::core::storage::{ScannedFile, ScannedSubtree};
        use crate::models::{EntryExtensions, Fields, NodeKind, NodeMetadata, SCHEMA_VERSION};

        let blank = || NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            authored: Fields::default(),
            derived: Fields::default(),
        };

        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/hello.md".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/deep/post.html".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/hello.meta.json".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/notes.toml".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "projects/websh.md".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "papers/tabula.pdf".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: Vec::new(),
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .expect("mount snapshot");

        assert_eq!(
            count_toc_entries(&fs, &VirtualPath::from_absolute("/writing").unwrap()),
            2
        );
        assert_eq!(
            toc_item_meta(
                &fs,
                TOC_ITEMS
                    .iter()
                    .find(|item| item.name == "projects")
                    .unwrap()
            ),
            "1"
        );
        assert_eq!(
            toc_item_meta(
                &fs,
                TOC_ITEMS.iter().find(|item| item.name == "about").unwrap()
            ),
            "bio · cv"
        );
    }
}

#[component]
fn TocSection() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    view! {
        <nav class=css::toc aria-label="Site index">
            <h2 class=css::tocHeading>"Index"</h2>
            <ol>
                {move || {
                    ctx.view_global_fs.with(|fs| {
                        TOC_ITEMS.iter().map(|item| {
                            let meta = toc_item_meta(fs, item);
                            view! {
                                <li>
                                    <a href=item.href>
                                        <span class=css::num>{item.num}</span>
                                        <span class=css::name>{item.name}</span>
                                        <span class=css::leader></span>
                                        <span class=css::pg>{meta}<span class=css::arrow>"→"</span></span>
                                    </a>
                                </li>
                            }
                        }).collect_view()
                    })
                }}
            </ol>
        </nav>
    }
}

fn toc_item_meta(fs: &GlobalFs, item: &TocItem) -> String {
    let Some(root) = item.count_root else {
        return item.meta.to_string();
    };
    let Ok(path) = VirtualPath::from_absolute(root) else {
        return "0".to_string();
    };
    count_toc_entries(fs, &path).to_string()
}

fn count_toc_entries(fs: &GlobalFs, root: &VirtualPath) -> usize {
    let Some(entries) = fs.list_dir(root) else {
        return 0;
    };

    entries
        .into_iter()
        .map(|entry| {
            if entry.name.starts_with('.') || entry.name.starts_with('_') {
                return 0;
            }
            if entry.is_dir {
                count_toc_entries(fs, &entry.path)
            } else if toc_countable_file(&entry.path) {
                1
            } else {
                0
            }
        })
        .sum()
}

fn toc_countable_file(path: &VirtualPath) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    if name.ends_with(".meta.json") || name == "manifest.json" || name.starts_with('_') {
        return false;
    }
    matches!(
        name.rsplit_once('.').map(|(_, ext)| ext),
        Some("md" | "html" | "pdf" | "link" | "app")
    )
}

#[component]
fn IntroSection() -> impl IntoView {
    view! {
        <h2 id="sec-intro" class=css::sectionTitle data-n="1.">
            "Introduction"<span class=css::loc>"[§1]"</span>
        </h2>
        <p class=css::introLead>
            "The author is the circuit below; this page is its proof transcript. The job is to convince you, without leaking the "
            <em>"witness"</em>", that the "<em>"constraints"</em>" are satisfiable."
        </p>

        <div class=css::protocol>
            <header>
                <b>"Circuit 1 — the author"</b>
                <span><span class=css::tag>"unaudited"</span></span>
            </header>
            <div class=css::protocolBody>
                <pre class=css::line><span class=css::kw>"public"</span>"       Wonjae Choi · PhD @ SNU · Seoul\n\n"<span class=css::kw>"private"</span>"      mood, unfinished drafts, open browser tabs (n ≫ 1)\n\n"<span class=css::kw>"constraints"</span>"  research  ∋ {zkVMs, ZK Compilers, EVM Compilers}\n             toolchain ∋ {Rust, Python, Solidity, LLVM}\n             habits    ∋ {nocturnal, infinite side projects, wasting LLM tokens}\n             output    = /papers ‖ /writing ‖ /projects ‖ /talks ‖ /misc"</pre>
            </div>
            <footer>
                <span>
                    <span class=css::protocolFootWitness>"witness: private · "</span>
                    "completeness ✓ · soundness ?"
                </span>
                <span class=css::protocolFootSetup>"no trusted setup"</span>
            </footer>
        </div>

        <p>"The rest of this site opens commitments to the above."</p>
    }
}

#[component]
fn RecentFeed() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let recent_items = Memo::new(move |_| ctx.view_global_fs.with(|fs| recent_items_from_fs(fs)));

    view! {
        <h2 id="sec-recent" class=css::sectionTitle data-n="2.">
            "Recent"<span class=css::loc>"[§2]"</span>
        </h2>
        <div class=css::feed>
            {move || {
                recent_items
                    .get()
                    .into_iter()
                    .map(|item| {
                        let kind_class = format!("{} {}", css::kind, feed_kind_class(&item.kind));
                        view! {
                            <div class=css::feedRow>
                                <span class=kind_class>{item.kind}</span>
                                <span class=css::date>{item.date}</span>
                                <span class=css::feedTitle><a href=item.href>{item.title}</a></span>
                                <span class=css::feedTag>{item.tag}</span>
                            </div>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

fn recent_items_from_fs(fs: &GlobalFs) -> Vec<RecentItem> {
    let mut items = Vec::new();

    for root in ["papers", "projects", "writing", "talks"] {
        let path = VirtualPath::from_absolute(format!("/{root}")).expect("constant category path");
        collect_recent_items(fs, &path, &mut items);
    }

    items.sort_by(|left, right| {
        right
            .date
            .cmp(&left.date)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.title.cmp(&right.title))
    });
    items.truncate(6);
    items
}

fn collect_recent_items(fs: &GlobalFs, path: &VirtualPath, out: &mut Vec<RecentItem>) {
    let Some(entries) = fs.list_dir(path) else {
        return;
    };

    for entry in entries {
        if entry.is_dir {
            collect_recent_items(fs, &entry.path, out);
            continue;
        }

        let node_meta = fs.node_metadata(&entry.path);
        let Some(date) = non_empty_text(node_meta.and_then(|meta| meta.date()).map(str::to_string))
        else {
            continue;
        };
        let Some(kind) = category_label_for_path(entry.path.as_str()) else {
            continue;
        };
        let title = non_empty_text(node_meta.and_then(|meta| meta.title()).map(str::to_string))
            .unwrap_or(entry.title);
        let tag = node_meta
            .and_then(|meta| meta.tags())
            .and_then(first_tag)
            .unwrap_or_default();

        out.push(RecentItem {
            kind,
            date,
            title,
            href: content_href_for_path(entry.path.as_str()),
            tag,
        });
    }
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn first_tag(tags: &[String]) -> Option<String> {
    tags.iter()
        .map(|tag| tag.trim().to_string())
        .find(|tag| !tag.is_empty())
}

fn category_label_for_path(path: &str) -> Option<String> {
    let folder = path.trim_start_matches('/').split('/').next()?;
    let label = match folder {
        "papers" => "paper",
        "projects" => "project",
        "talks" => "talk",
        "writing" => "writing",
        _ => return None,
    };
    Some(label.to_string())
}

fn feed_kind_class(kind: &str) -> &'static str {
    match kind {
        "paper" => css::kindPaper,
        "project" => css::kindProject,
        "writing" => css::kindWriting,
        "talk" => css::kindTalk,
        _ => "",
    }
}
