//! Filesystem directory listing surface.

mod model;

use leptos::prelude::*;

use crate::app::AppContext;
use crate::features::chrome::SiteChrome;
use crate::shared::components::{
    AttestationSigFooter, IdentifierStrip, MetaRow, MetaTable, SiteContentFrame, SiteSurface,
    nearest_attestation_route_for_content_path,
};
use model::{
    DirectoryListingEntry, DirectoryListingGroup, DirectoryModel, build_directory_model,
    build_directory_model_with_bundle_context, kind_label, surface_kind_label,
};
use websh_core::filesystem::{RouteFrame, route_request_targets_runtime_overlay};
use websh_core::support::format::format_date_compact;

stylance::import_crate_style!(css, "src/features/directory/directory_page.module.css");

#[component]
pub fn DirectoryPage(route: Memo<RouteFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let node_path = Memo::new(move |_| route.get().resolution.node_path);
    let model = Memo::new(move |_| {
        let frame = route.get();
        if route_request_targets_runtime_overlay(&frame.request) {
            ctx.system_global_fs
                .with(|fs| build_directory_model(fs, &frame.resolution.node_path))
        } else {
            ctx.content.with_fs(|fs| {
                build_directory_model_with_bundle_context(
                    fs,
                    &frame.resolution.node_path,
                    frame.resolution.bundle_variant.as_ref(),
                )
            })
        }
    });
    let attestation_route = Signal::derive(move || {
        let frame = route.get();
        frame
            .resolution
            .bundle_variant
            .as_ref()
            .map(|context| {
                websh_core::filesystem::content_route_for_path(context.bundle_path.as_str())
            })
            .unwrap_or_else(|| nearest_attestation_route_for_content_path(&node_path.get()))
    });

    view! {
        <SiteSurface class=css::surface>
            <SiteChrome route=route />
            <SiteContentFrame class=css::page>
                <crate::shared::components::MountStatusNotice path=Signal::derive(move || node_path.get()) />
                {move || view! {
                    <div class=css::content>
                        <DirectoryIdentifier model=model.get() />
                        <DirectoryTitleBlock model=model.get() />
                        <section class=css::directoryBody>
                            <DirectoryEntries model=model.get() />
                        </section>
                    </div>
                }}
                <AttestationSigFooter route=attestation_route />
            </SiteContentFrame>
        </SiteSurface>
    }
}

#[component]
fn DirectoryIdentifier(model: DirectoryModel) -> impl IntoView {
    let count = entry_count_label(model.entry_count);
    let date = model.date.as_deref().and_then(format_date_compact);
    view! {
        <IdentifierStrip muted=true>
            <span class=css::identLeft>
                <span>{surface_kind_label(model.kind)}</span>
                {date.map(|value| view! { <span>{value}</span> })}
            </span>
            <span class=css::identMetric>
                <span>{count}</span>
            </span>
        </IdentifierStrip>
    }
}

#[component]
fn DirectoryTitleBlock(model: DirectoryModel) -> impl IntoView {
    let child_count = model.child_count;
    let date = model.date.clone();
    let tags = model.tags.clone();
    let variants = model.variants.clone();
    let description = model.description.clone();
    view! {
        <div class=css::titleBlock>
            <div class=css::titleRow>
                <h1 class=css::title>{model.title}</h1>
            </div>
            <MetaTable class=css::metaTable aria_label="directory metadata">
                <MetaRow
                    label="Type"
                    row_class=css::metaRow
                    key_class=css::metaKey
                    value_class=css::metaValue
                >
                    <span class=css::metaTag>{kind_label(model.kind)}</span>
                </MetaRow>
                {child_count.map(|count| view! {
                    <MetaRow
                        label="Entries"
                        row_class=css::metaRow
                        key_class=css::metaKey
                        value_class=css::metaValue
                    >
                        {entry_count_label(count as usize)}
                    </MetaRow>
                })}
                {date.map(|date| view! {
                    <MetaRow
                        label="Date"
                        row_class=css::metaRow
                        key_class=css::metaKey
                        value_class=css::metaValue
                    >
                        {date}
                    </MetaRow>
                })}
                {(!tags.is_empty()).then(|| view! {
                    <MetaRow
                        label="Tags"
                        row_class=css::metaRow
                        key_class=css::metaKey
                        value_class=css::metaValue
                    >
                        {tags.into_iter().map(|tag| view! {
                            <span class=css::metaTag>{tag}</span>
                        }).collect_view()}
                    </MetaRow>
                })}
                {(!variants.is_empty()).then(|| view! {
                    <MetaRow
                        label="Variants"
                        row_class=css::metaRow
                        key_class=css::metaKey
                        value_class=css::metaValue
                    >
                        {variants.into_iter().map(|variant| {
                            if variant.active {
                                view! {
                                    <span class=css::metaTag aria-current="true">{variant.label}</span>
                                }.into_any()
                            } else {
                                view! {
                                    <a class=css::metaTag href=variant.href>{variant.label}</a>
                                }.into_any()
                            }
                        }).collect_view()}
                    </MetaRow>
                })}
            </MetaTable>
            {description.map(|description| view! {
                <section class=css::descriptionBlock aria-label="Directory description">
                    <h2 class=css::sectionTitle data-n="">"Description"</h2>
                    <p class=css::descriptionText>{description}</p>
                </section>
            })}
        </div>
    }
}

#[component]
fn DirectoryEntries(model: DirectoryModel) -> impl IntoView {
    if model.groups.is_empty() {
        return view! {
            <p class=css::empty>"empty directory"</p>
        }
        .into_any();
    }

    view! {
        <nav class=css::catalog aria-label="Directory entries">
            {model.groups.into_iter().map(|group| view! {
                <DirectoryGroup group=group />
            }).collect_view()}
        </nav>
    }
    .into_any()
}

#[component]
fn DirectoryGroup(group: DirectoryListingGroup) -> impl IntoView {
    let group_kind = group.kind;
    let count = entry_count_label(group.entries.len());
    view! {
        <section class=css::catalogGroup data-kind=group_kind.attr()>
            <div class=css::groupBar>
                <span class=css::groupLabel data-kind=group_kind.attr()>
                    <span class=css::groupGlyph aria-hidden="true">{group_kind.glyph()}</span>
                    {group_kind.label()}
                </span>
                <span class=css::groupCount>{count}</span>
                <span class=css::groupSpacer aria-hidden="true"></span>
                <span class=css::groupSummary>{group_kind.summary()}</span>
            </div>
            <div class=css::groupRows>
                {group.entries.into_iter().map(|entry| view! {
                    <DirectoryEntryTile entry=entry />
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
fn DirectoryEntryTile(entry: DirectoryListingEntry) -> impl IntoView {
    let DirectoryListingEntry {
        group_kind,
        name,
        href,
        title,
        description,
        extent,
        secondary_meta,
        tags,
    } = entry;
    view! {
        <a class=css::tile href=href data-kind=group_kind.attr()>
            <span class=css::tileGlyph aria-hidden="true">{group_kind.glyph()}</span>
            <span class=css::tileText>
                <span class=css::tileTopLine>
                    <span class=css::tileName>{name}</span>
                    {title.map(|title| view! {
                        <span class=css::tileTitle>{title}</span>
                    })}
                </span>
                {description.map(|description| view! {
                    <span class=css::tileDescription>{description}</span>
                })}
                {(!tags.is_empty()).then(|| view! {
                    <span class=css::tileTags aria-label="Entry tags">
                        {tags.into_iter().map(|tag| view! {
                            <span class=css::tileTag>{tag}</span>
                        }).collect_view()}
                    </span>
                })}
            </span>
            <span class=css::tileExtent>{extent}</span>
            <span class=css::tileMeta>{secondary_meta.unwrap_or_default()}</span>
        </a>
    }
}

fn entry_count_label(count: usize) -> String {
    format!("{count} {}", if count == 1 { "item" } else { "items" })
}
