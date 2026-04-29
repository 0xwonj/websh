//! Built-in homepage.
//!
//! The root URL is an application surface, not a filesystem document reader.
//! Content routes such as `/#/index.html` remain available through the
//! filesystem router.

use gloo_timers::callback::Timeout;
use leptos::ev;
use leptos::prelude::*;
use serde::Deserialize;

use crate::app::AppContext;
use crate::components::chrome::{
    SiteChrome, SiteChromeBreadcrumb, SiteChromeBreadcrumbItem, SiteChromeChip, SiteChromeIdentity,
    SiteChromeLead, SiteChromeRoot, SiteChromeSurface, SiteChromeTextChip,
};
use crate::components::markdown::InlineMarkdownView;
use crate::components::shared::{
    AttestationSigFooter, MetaRow as SharedMetaRow, MetaTable as SharedMetaTable, SiteContentFrame,
    SiteSurface,
};
use crate::config::{APP_NAME, APP_VERSION};
use crate::core::engine::{GlobalFs, RouteFrame};
use crate::crypto::ack::{
    AckArtifact, AckMembershipProof, AckReceipt, normalize_ack_name, public_proof_for_name,
    short_hash, verify_private_receipt,
};
use crate::crypto::attestation::AttestationArtifact;
use crate::crypto::pgp::{EXPECTED_PGP_FINGERPRINT, pretty_fingerprint};
use crate::models::VirtualPath;
use crate::utils::content_routes::content_href_for_path;
use crate::utils::render_inline_markdown;

stylance::import_crate_style!(css, "src/components/home/home.module.css");

const PUBLIC_KEY_BLOCK: &str = include_str!("../../../content/keys/wonjae.asc");

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

#[derive(Clone, Debug, Default)]
struct AckResult {
    message: String,
    proof: Option<AckMembershipProof>,
    included: bool,
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
        <div class=css::identifier>
            <span class=css::id><b>{paper_id}</b></span>
            <span class=css::rev>{revised}</span>
        </div>

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
                .map(|subject| subject.issued_at.clone())
        })
}

fn current_homepage_date() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new_0();
        return format!(
            "{:04}-{:02}-{:02}",
            date.get_full_year(),
            date.get_month() + 1,
            date.get_date()
        );
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
                "zero-knowledge proofs, compilers, Ethereum"
            </SharedMetaRow>
            <SharedMetaRow
                label="Availability"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                <span class=css::dim>"ens "</span>
                <a href="https://wonjae.eth.limo">"wonjae.eth"</a>
                <span class=css::dotSep>" · "</span>
                <span class=css::dim>"github "</span>
                <a href="https://github.com/0xwonj">"0xwonj"</a>
                <span class=css::dotSep>" · "</span>
                <span class=css::dim>"linkedin "</span>
                <a href="https://www.linkedin.com/in/wonj">"wonjaechoi"</a>
                <span class=css::dotSep>" · "</span>
                <span class=css::dim>"email "</span>
                <a href="mailto:wonjae@snu.ac.kr">"wonjae@snu.ac.kr"</a>
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
            return fallback_now_document();
        }

        ctx.read_text(&path)
            .await
            .ok()
            .and_then(|body| parse_now_toml(&body).ok())
            .unwrap_or_else(fallback_now_document)
    });

    view! {
        {move || {
            let doc = now.get().unwrap_or_else(fallback_now_document);
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

fn fallback_now_document() -> NowDocument {
    NowDocument {
        items: vec![
            NowItem {
                date: "2026-04-22".to_string(),
                text:
                    "zkgrep - zero-knowledge regex matcher over Plonkish; writing up. Prover slow."
                        .to_string(),
            },
            NowItem {
                date: "2026-04-22".to_string(),
                text: "websh - adding | pipes so ls | grep | wc behaves.".to_string(),
            },
            NowItem {
                date: "2026-04-22".to_string(),
                text: "5 days - kissaten, secondhand bookshops, and absolutely no laptop."
                    .to_string(),
            },
        ],
    }
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
        use crate::models::FileMetadata;

        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "projects/websh.md".to_string(),
                    description: "websh".to_string(),
                    meta: FileMetadata {
                        date: Some("2026-04-22".to_string()),
                        tags: vec!["local app".to_string(), "rust".to_string()],
                        ..Default::default()
                    },
                },
                ScannedFile {
                    path: "papers/tabula.md".to_string(),
                    description: "tabula".to_string(),
                    meta: FileMetadata {
                        date: Some("2026-04-26".to_string()),
                        tags: vec!["EuroSys 2027".to_string(), "systems".to_string()],
                        ..Default::default()
                    },
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
        use crate::models::FileMetadata;

        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/hello.md".to_string(),
                    description: "hello".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "writing/deep/post.html".to_string(),
                    description: "post".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "writing/hello.meta.json".to_string(),
                    description: "sidecar".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "writing/notes.toml".to_string(),
                    description: "data".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "projects/websh.md".to_string(),
                    description: "websh".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "papers/tabula.pdf".to_string(),
                    description: "tabula".to_string(),
                    meta: FileMetadata::default(),
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
                <span><span class=css::tag>"unaudited"</span><span class=css::tag>"WIP"</span></span>
            </header>
            <div class=css::protocolBody>
                <pre class=css::line><span class=css::kw>"public"</span>"       Wonjae Choi · PhD @ SNU · Seoul\n\n"<span class=css::kw>"private"</span>"      mood, unfinished drafts, open browser tabs (n ≫ 1)\n\n"<span class=css::kw>"constraints"</span>"  research  ∋ {zkVMs, ZK Compilers, EVM Compilers}\n             toolchain ∋ {Rust, Python, Solidity, LLVM}\n             habits    ∋ {nocturnal, infinite side projects, wasting LLM tokens}\n             output    = /papers ‖ /writing ‖ /projects ‖ /talks ‖ /misc"</pre>
            </div>
            <footer>
                <span>"witness: private · completeness ✓ · soundness ?"</span>
                <span>"no trusted setup"</span>
            </footer>
        </div>

        <p>"The rest of this site opens commitments to the above."</p>
    }
}

#[component]
fn RecentFeed() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let recent_items =
        Signal::derive(move || ctx.view_global_fs.with(|fs| recent_items_from_fs(fs)));

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

        let Some(meta) = entry.file_meta else {
            continue;
        };
        let node_meta = fs.node_metadata(&entry.path);
        let Some(date) = non_empty_text(node_meta.and_then(|meta| meta.date.as_ref()))
            .or_else(|| non_empty_text(meta.date.as_ref()))
        else {
            continue;
        };
        let Some(kind) = category_label_for_path(entry.path.as_str()) else {
            continue;
        };
        let title =
            non_empty_text(node_meta.and_then(|meta| meta.title.as_ref())).unwrap_or(entry.title);
        let tag = node_meta
            .and_then(|meta| first_tag(&meta.tags))
            .or_else(|| first_tag(&meta.tags))
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

fn non_empty_text(value: Option<&String>) -> Option<String> {
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

#[component]
fn Appendices() -> impl IntoView {
    view! {
        <PublicKeyAppendix />
        <ShellAppendix />
    }
}

#[component]
fn PublicKeyAppendix() -> impl IntoView {
    let (copied, set_copied) = signal(false);
    let copy_key = move |_| {
        set_copied.set(true);
        Timeout::new(1200, move || set_copied.set(false)).forget();
    };

    view! {
        <details class=css::appendix id="appendix-a">
            <summary><h2 class=css::sectionTitle data-n="A.">"Appendix A · Public Key"<span class=css::loc>"[§A]"</span></h2></summary>
            <p>
                "OpenPGP key for "<em>"Wonjae Choi <wonjae@snu.ac.kr>"</em>". Use it to send encrypted mail or verify signatures. Rotation: when it annoys me."
            </p>
            <p class=css::footnote>
                "Fingerprint: "<span class=css::fp>{pretty_fingerprint(EXPECTED_PGP_FINGERPRINT)}</span>
            </p>
            <pre class=css::keyblock aria-label="PGP public key block">
                {PUBLIC_KEY_BLOCK.lines().map(|line| {
                    let line_class = if public_key_block_header_line(line) {
                        css::keyHeader
                    } else {
                        css::keyBody
                    };
                    let (plain, accent) = split_public_key_checksum_tail(line);
                    view! {
                        <span class=line_class>{plain}<span class=css::fp>{accent}</span></span>
                        "\n"
                    }
                }).collect_view()}<button class=css::copy type="button" on:click=copy_key>
                {move || if copied.get() { "copied" } else { "copy" }}
            </button></pre>
            <p class=css::footnote>
                "Also reachable via the virtual filesystem at "<a href="/#/keys/wonjae.asc">"/keys/wonjae.asc"</a>"."
            </p>
        </details>
    }
}

fn public_key_block_header_line(line: &str) -> bool {
    matches!(
        line,
        "-----BEGIN PGP PUBLIC KEY BLOCK-----" | "-----END PGP PUBLIC KEY BLOCK-----"
    )
}

fn split_public_key_checksum_tail(line: &str) -> (&str, &str) {
    if !line.starts_with('=') {
        return (line, "");
    }

    let Some((split_at, _)) = line.char_indices().rev().nth(3) else {
        return ("", line);
    };
    (&line[..split_at], &line[split_at..])
}

#[component]
fn ShellAppendix() -> impl IntoView {
    let preview_breadcrumbs = Signal::derive(|| {
        vec![
            SiteChromeBreadcrumbItem::link("~", "/"),
            SiteChromeBreadcrumbItem::current("websh"),
        ]
    });
    let version = Signal::derive(|| format!("websh v{APP_VERSION}"));
    let session = Signal::derive(|| "guest".to_string());
    let network = Signal::derive(|| "offline".to_string());

    view! {
        <details class=css::appendix id="appendix-b">
            <summary><h2 class=css::sectionTitle data-n="B.">"Appendix B · Reference Implementation"<span class=css::loc>"[§B]"</span></h2></summary>
            <p>
                "Below is a non-interactive transcript of "
                <span class=css::appendixShellName>"websh"</span>
                ", the browser-resident shell distributed alongside this preprint. The shell backs onto a virtual filesystem in which every section of this page is a file. "
                <a href="/#/websh" class=css::appendixShellLaunch>"Launch shell ↗"</a>
            </p>
            <div class=css::term>
                <SiteChromeRoot surface=SiteChromeSurface::Shell>
                    <SiteChromeLead>
                        <SiteChromeIdentity label=APP_NAME href=Signal::derive(|| "/".to_string()) />
                        <SiteChromeTextChip value=version />
                        <SiteChromeChip label="session" value=session />
                        <SiteChromeChip label="network" value=network />
                    </SiteChromeLead>
                    <SiteChromeBreadcrumb items=preview_breadcrumbs aria_label="preview path" />
                </SiteChromeRoot>
                <div class=css::termBody>
                    <div class=css::termLine><span class=css::out>{format!("[   0.000] Booting websh kernel v{APP_VERSION}")}</span></div>
                    <div class=css::termLine><span class=css::okOut>"[   0.030] WASM runtime initialized"</span></div>
                    <div class=css::termLine><span class=css::out>"[   0.053] Mounting filesystems..."</span></div>
                    <div class=css::termLine><span class=css::okOut>"[   0.096] Total: 7 files mounted"</span></div>
                    <div class=css::termLine><span class=css::warnOut>"[   0.139] Desktop detected, initializing Terminal mode"</span></div>
                    <div class=css::termGap></div>
                    <pre class=css::termBanner>"WONJAE.ETH"</pre>
                    <div class=css::termLine><span class=css::warnOut>"Zero-Knowledge Proofs | Compiler Design | Ethereum"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::termLine><span class=css::out>"Tips:"</span></div>
                    <div class=css::termLine><span class=css::out>"  - Type 'help' for available commands"</span></div>
                    <div class=css::termLine><span class=css::out>"  - Use the archive bar to switch between websh and explorer"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::commandLine>
                        <span class=css::prompt>{format!("guest@{APP_NAME}:~")}</span>
                        <span class=css::separator>"$ "</span>
                        <span class=css::cmd>"ls"</span>
                    </div>
                    <div class=css::listEntry><span class=css::dir>"keys/"</span><span class=css::out>"wonjae"</span></div>
                    <div class=css::listEntry><span class=css::file>"now.toml"</span><span class=css::out>"now"</span></div>
                    <div class=css::listEntry><span class=css::dir>"papers/"</span><span class=css::out>"papers"</span></div>
                    <div class=css::listEntry><span class=css::dir>"projects/"</span><span class=css::out>"projects"</span></div>
                    <div class=css::listEntry><span class=css::dir>"talks/"</span><span class=css::out>"talks"</span></div>
                    <div class=css::listEntry><span class=css::dir>"writing/"</span><span class=css::out>"writing"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::commandLine>
                        <span class=css::prompt>{format!("guest@{APP_NAME}:~")}</span>
                        <span class=css::separator>"$ "</span>
                        <span class=css::cmd>"help | grep sync"</span>
                    </div>
                    <div class=css::termLine><span class=css::out>"    sync                      Show working tree status (unstaged, staged)"</span></div>
                    <div class=css::termLine><span class=css::out>"    sync commit <message>     Commit staged changes (admin-only)"</span></div>
                    <div class=css::termLine><span class=css::out>"    sync refresh              Reload manifest from remote"</span></div>
                    <div class=css::termGap></div>
                    <div class=css::inputLine>
                        <span class=css::prompt>{format!("guest@{APP_NAME}:~")}</span>
                        <span class=css::separator>"$ "</span>
                        <span class=css::cursor></span>
                    </div>
                </div>
            </div>
        </details>
    }
}

#[component]
fn Acknowledgements() -> impl IntoView {
    let artifact = AckArtifact::from_homepage_asset().expect("homepage ACK artifact must parse");
    let root_short = short_hash(&artifact.combined_root);
    let depth = ack_public_depth(artifact.public.count);
    let ack_count = artifact.count();
    let (ack_input, set_ack_input) = signal(String::new());
    let (ack_result, set_ack_result) = signal(AckResult::default());

    let public_artifact = artifact.clone();
    let run_ack_check = Callback::new(move |_: ()| {
        let raw = ack_input.get();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            set_ack_result.set(AckResult::default());
            return;
        }

        if looks_like_ack_receipt(trimmed) {
            let receipt = match serde_json::from_str::<AckReceipt>(trimmed) {
                Ok(receipt) => receipt,
                Err(error) => {
                    set_ack_result.set(AckResult {
                        message: format!("✗ receipt JSON parse failed · {error}"),
                        proof: None,
                        included: false,
                    });
                    return;
                }
            };

            match verify_private_receipt(&public_artifact, &receipt) {
                Ok(verification) => set_ack_result.set(AckResult {
                    message: format!(
                        "✓ private acknowledgement receipt · name committed privately · root {}",
                        short_hash(&verification.combined_root)
                    ),
                    proof: None,
                    included: true,
                }),
                Err(error) => set_ack_result.set(AckResult {
                    message: format!("✗ private acknowledgement receipt invalid · {error}"),
                    proof: None,
                    included: false,
                }),
            }
            return;
        }

        if normalize_ack_name(&raw).is_empty() {
            set_ack_result.set(AckResult::default());
            return;
        }

        let proof = match public_proof_for_name(&public_artifact, &raw) {
            Ok(Some(proof)) => proof,
            Ok(None) => {
                set_ack_result.set(AckResult {
                    message: format!(
                        "✗ no public acknowledgement for \"{}\" · paste a private receipt if this entry is private.",
                        normalize_ack_name(&raw)
                    ),
                    proof: None,
                    included: false,
                });
                return;
            }
            Err(error) => {
                set_ack_result.set(AckResult {
                    message: format!("✗ commitment error · {error}"),
                    proof: None,
                    included: false,
                });
                return;
            }
        };

        let idx = proof.idx;
        let side_path = proof.side_path();
        set_ack_result.set(AckResult {
            message: format!("✓ included · leaf {idx} · path: {side_path}"),
            proof: Some(proof),
            included: true,
        });
    });
    let run_ack_check_on_key = run_ack_check;

    view! {
        <h2 class=css::sectionTitle data-n="">"Acknowledgements"</h2>
        <p>
            "The author thanks the set "
            <AckMathInline tex="S" />
            " whose membership is succinctly attested by the commitment "
            <AckMathInline tex=r"\operatorname{root}(S)" />
            " below. If your name is in "
            <AckMathInline tex="S" />
            ", you may verify it; if it is not, this is a soundness bug - please report."
        </p>
        <details class=css::ackMerkle id="ackMerkle">
            <summary>
                <span class=css::lab>"commitment"</span>
                <code class=css::root>{root_short}</code>
                <span class=css::metaBits><span class=css::dim>"n"</span>"="{ack_count}" · "<span class=css::dim>"depth"</span>"="{depth}" · "<span class=css::dim>"hash"</span>"=sha-256"</span>
            </summary>
            <div class=css::verify>
                <span class=css::verifyPrompt>"verify ▸"</span>
                <input
                    type="text"
                    placeholder="enter a public name or paste a private receipt"
                    autocomplete="off"
                    spellcheck="false"
                    prop:value=move || ack_input.get()
                    on:input=move |ev| set_ack_input.set(event_target_value(&ev))
                    on:keydown=move |ev: ev::KeyboardEvent| {
                        if ev.key() == "Enter" {
                            run_ack_check_on_key.run(());
                        }
                    }
                />
                <button type="button" on:click=move |_| run_ack_check.run(())>"verify"</button>
                <span class=move || verify_result_class(&ack_result.get())>
                    {move || ack_result.get().message}
                </span>
            </div>
            <Show when=move || ack_result.with(|result| result.proof.is_some())>
                <details class=css::proof>
                    <summary>"view inclusion proof"</summary>
                    <pre class=css::proofBody>
                        {move || ack_result.get().proof.map(|proof| view! { <AckProofView proof=proof /> })}
                    </pre>
                </details>
            </Show>
        </details>

        <p class=css::ackFootnote>
            "Reviewers: please do not reject this homepage."
        </p>
    }
}

#[component]
fn AckMathInline(tex: &'static str) -> impl IntoView {
    let rendered = render_inline_markdown(&format!("${tex}$"));
    let rendered = Signal::derive(move || rendered.clone());

    view! {
        <InlineMarkdownView rendered=rendered />
    }
}

#[component]
fn AckProofView(proof: AckMembershipProof) -> impl IntoView {
    let idx = proof.idx;
    let target = proof.target;
    let name = proof.name;
    let leaf_hex = proof.leaf_hex;
    let steps = proof.steps;
    let recomputed_hex = proof.recomputed_hex;
    let committed_hex = proof.committed_hex;
    let verified = proof.verified;

    view! {
        <span class=css::proofK>{format!("leaf[{idx}]")}</span>
        {format!("     = sha256(\"websh.ack.public.leaf.v1\" || len || \"{target}\")\n")}
        "            = "
        <span class=css::proofH>{leaf_hex}</span>
        "\n\n"
        {steps.into_iter().map(|step| view! {
            <span class=css::proofK>{format!("step {}", step.number)}</span>
            "     sibling."
            <span class=css::proofK>{step.side}</span>
            " = "
            <span class=css::proofH>{step.sibling_hex}</span>
            "\n           parent    = "
            <span class=css::proofH>{step.parent_hex}</span>
            "\n"
        }).collect_view()}
        "\nrecomputed = "
        <span class=css::proofH>{recomputed_hex}</span>
        "\ncommitted  = "
        <span class=css::proofH>{committed_hex}</span>
        "\n\n"
        <span class=css::proofH>
            {if verified {
                format!("✓ verified · \"{name}\" ∈ commitment")
            } else {
                "✗ root mismatch (this should not happen)".to_string()
            }}
        </span>
    }
}

#[component]
fn PageFooter() -> impl IntoView {
    view! {
        <AttestationSigFooter
            route=Signal::derive(|| "/".to_string())
            show_pending=Signal::derive(|| true)
            colophon=true
        />
    }
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

fn verify_result_class(result: &AckResult) -> String {
    if result.message.is_empty() {
        css::verifyResult.to_string()
    } else if result.included {
        format!("{} {}", css::verifyResult, css::verifyOk)
    } else {
        format!("{} {}", css::verifyResult, css::verifyNo)
    }
}

fn looks_like_ack_receipt(input: &str) -> bool {
    input.starts_with('{') || input.contains("\"websh.ack.private.receipt.v1\"")
}

fn ack_public_depth(count: usize) -> usize {
    if count <= 1 {
        return 0;
    }
    usize::BITS as usize - (count - 1).leading_zeros() as usize
}
