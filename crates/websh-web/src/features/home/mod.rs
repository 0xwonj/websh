//! Built-in homepage.
//!
//! The root URL is an application surface, not a filesystem document reader.
//! Canonical content routes remain available through the filesystem router.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::features::chrome::SiteChrome;
use crate::render::render_inline_markdown;
use crate::runtime::MountLoadStatus;
use crate::shared::components::markdown::InlineMarkdownView;
use crate::shared::components::{
    IdentifierStrip, MetaRow as SharedMetaRow, MetaTable as SharedMetaTable, SiteContentFrame,
    SiteSurface,
};
use websh_core::domain::VirtualPath;
use websh_core::filesystem::{GlobalFs, RouteFrame};
use websh_core::publication::{Now, Profile};

stylance::import_crate_style!(
    pub(super) css,
    "src/features/home/home.module.css"
);

mod model;
mod sections;
use model::{
    TOC_ITEMS, compact_homepage_date, latest_now_date, publication_date, recent_items_from_fs,
    toc_item_meta,
};
use sections::{Acknowledgements, Appendices, PageFooter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootContentReadiness {
    Loading,
    Loaded,
    Failed,
}

fn root_content_readiness(ctx: AppContext) -> RootContentReadiness {
    match ctx.content.mount_status_for(&VirtualPath::root()) {
        Some(MountLoadStatus::Available { .. }) => RootContentReadiness::Loaded,
        Some(MountLoadStatus::Failed { .. }) => RootContentReadiness::Failed,
        Some(MountLoadStatus::Loading) | None => RootContentReadiness::Loading,
    }
}

#[component]
pub fn HomePage(route: Memo<RouteFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let home = Memo::new(move |_| ctx.content.home());

    view! {
        <SiteSurface class=css::home>
            <SiteChrome route=route />
            <SiteContentFrame class=css::page>
                <crate::shared::components::MountStatusNotice path=Signal::derive(VirtualPath::root) />
                {move || home.get().map(|home| {
                    let issued_at = ctx.content.issued_at().map(publication_date);
                    view! {
                        <HeroHeader profile=home.profile.clone() issued_at=issued_at />
                        <HomepageMetaTable profile=home.profile.clone() />
                        <AbstractSection text=home.profile.abstract_text.clone() now=home.now />
                        <TocSection />
                        <IntroSection profile=home.profile />
                        <RecentFeed />
                        <Appendices />
                        <Acknowledgements artifact=home.ack />
                        <PageFooter />
                    }
                })}
            </SiteContentFrame>
        </SiteSurface>
    }
}

#[component]
fn HeroHeader(profile: Profile, issued_at: Option<String>) -> impl IntoView {
    let paper_id = issued_at
        .as_deref()
        .map(compact_homepage_date)
        .map(|date| format!("Paper {date}"))
        .unwrap_or_default();
    let revised = issued_at
        .map(|date| format!("last revised {date}"))
        .unwrap_or_default();
    let email_href = format!("mailto:{}", profile.email);

    view! {
        <IdentifierStrip>
            <span>{paper_id}</span>
            <span>{revised}</span>
        </IdentifierStrip>
        <h1 class=css::title>
            {profile.title}
            <span class=css::tagline>{profile.tagline}</span>
        </h1>
        <div class=css::authors>
            {profile.name}<sup class=css::star>"*"</sup>
        </div>
        <div class=css::aff>
            <sup>"*"</sup>" "{profile.affiliation}" "
            <span class=css::dotSep>" · "</span>
            <a href=email_href>{profile.email}</a>
        </div>
    }
}

#[component]
fn HomepageMetaTable(profile: Profile) -> impl IntoView {
    let email_href = format!("mailto:{}", profile.email);
    view! {
        <SharedMetaTable class=css::meta aria_label="ePrint metadata">
            <SharedMetaRow label="Category" row_class=css::metaRow key_class=css::metaKey value_class=css::metaValue>
                {profile.categories.into_iter().map(|category| view! {
                    <span class=css::tag>{category}</span>
                }).collect_view()}
            </SharedMetaRow>
            <SharedMetaRow label="Keywords" row_class=css::metaRow key_class=css::metaKey value_class=css::metaValue>
                {profile.keywords.join(", ")}
            </SharedMetaRow>
            <SharedMetaRow label="Availability" row_class=css::metaRow key_class=css::metaKey value_class=css::metaValue>
                <span class=css::availFull>
                    <span class=css::dim>"email "</span>
                    <a href=email_href.clone()>{profile.email}</a>
                </span>
                <a class=css::availCompact href=email_href><span class=css::dim>"email"</span></a>
                {profile.links.into_iter().map(|link| {
                    let kind = link.kind.unwrap_or_else(|| link.label.clone());
                    view! {
                        <span class=css::dotSep>" · "</span>
                        <span class=css::availFull>
                            <span class=css::dim>{kind.clone()}" "</span>
                            <a href=link.url.clone()>{link.label}</a>
                        </span>
                        <a class=css::availCompact href=link.url><span class=css::dim>{kind}</span></a>
                    }
                }).collect_view()}
            </SharedMetaRow>
            <SharedMetaRow label="Status" row_class=css::metaRow key_class=css::metaKey value_class=css::metaValue>
                <span class=css::live>{profile.status}</span>
            </SharedMetaRow>
        </SharedMetaTable>
    }
}

#[component]
fn AbstractSection(text: String, now: Now) -> impl IntoView {
    let rendered = render_inline_markdown(&text);
    view! {
        <h2 class=css::sectionTitle data-n="">"Abstract"</h2>
        <p><InlineMarkdownView rendered=Signal::derive(move || rendered.clone()) /></p>
        <NowSection now=now />
    }
}

#[component]
fn NowSection(now: Now) -> impl IntoView {
    let timestamp = latest_now_date(&now.items)
        .map(|date| format!("last touched {date}"))
        .unwrap_or_default();
    view! {
        <div class=css::nowInline>
            <p class=css::nowFormalLead><em>"Now"</em>":"</p>
            <ul class=css::nowFormal>
                {now.items.into_iter().map(|item| {
                    let rendered = render_inline_markdown(&item.text);
                    view! {
                        <li><InlineMarkdownView rendered=Signal::derive(move || rendered.clone()) /></li>
                    }
                }).collect_view()}
            </ul>
            <p class=css::ts>{timestamp}</p>
        </div>
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
                    let readiness = root_content_readiness(ctx);
                    ctx.content.with_fs(|fs| {
                        TOC_ITEMS.iter().map(|item| {
                            let meta = toc_item_meta_for_readiness(fs, item, readiness);
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

#[component]
fn IntroSection(profile: Profile) -> impl IntoView {
    let introduction = render_inline_markdown(&profile.introduction);
    let constraints = format!(
        "  research  ∋ {{{}}}\n             toolchain ∋ {{{}}}\n             habits    ∋ {{{}}}\n             output    = /papers ‖ /writing ‖ /projects ‖ /talks ‖ /misc",
        profile.research.join(", "),
        profile.tools.join(", "),
        profile.habits.join(", "),
    );
    view! {
        <h2 id="sec-intro" class=css::sectionTitle data-n="1.">
            "Introduction"<span class=css::loc>"[§1]"</span>
        </h2>
        <p class=css::introLead>
            <InlineMarkdownView rendered=Signal::derive(move || introduction.clone()) />
        </p>

        <div class=css::protocol>
            <header>
                <b>"Circuit 1 — the author"</b>
                <span><span class=css::tag>"unaudited"</span></span>
            </header>
            <div class=css::protocolBody>
                <pre class=css::line><span class=css::kw>"public"</span>"       "{profile.public_identity}"\n\n"<span class=css::kw>"private"</span>"      "{profile.private_identity}"\n\n"<span class=css::kw>"constraints"</span>{constraints}</pre>
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
    let recent_items = Memo::new(move |_| {
        if root_content_readiness(ctx) != RootContentReadiness::Loaded {
            return Vec::new();
        }
        ctx.content.with_fs(recent_items_from_fs)
    });

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

fn toc_item_meta_for_readiness(
    fs: &GlobalFs,
    item: &model::TocItem,
    readiness: RootContentReadiness,
) -> String {
    if !item.is_count_backed() {
        return toc_item_meta(fs, item);
    }

    match readiness {
        RootContentReadiness::Loaded => toc_item_meta(fs, item),
        RootContentReadiness::Failed => "—".to_string(),
        RootContentReadiness::Loading => "…".to_string(),
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
