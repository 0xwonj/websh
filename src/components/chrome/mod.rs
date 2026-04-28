//! Shared site chrome primitives.
//!
//! These components own the common site chrome visual language used by the
//! homepage, renderer pages, ledger pages, and the live shell. Route-aware callers provide plain labels,
//! links, active state, and display values.

use leptos::ev;
use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::ledger_routes::is_ledger_filter_route_segment;
use crate::config::APP_NAME;
use crate::core::engine::{RouteFrame, RouteSurface, request_path_for_canonical_path, route_cwd};
use crate::core::wallet;
use crate::models::VirtualPath;
use crate::utils::theme::{THEMES, apply_theme};

stylance::import_crate_style!(css, "src/components/chrome/site_chrome.module.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteChromeSurface {
    Home,
    Shell,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SiteChromeBreadcrumbItem {
    pub label: String,
    pub href: Option<String>,
    pub current: bool,
}

impl SiteChromeBreadcrumbItem {
    pub fn link(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: Some(href.into()),
            current: false,
        }
    }

    pub fn current(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: None,
            current: true,
        }
    }
}

#[component]
pub fn SiteChromeRoot(surface: SiteChromeSurface, children: Children) -> impl IntoView {
    let surface_class = match surface {
        SiteChromeSurface::Home => css::surfaceHome,
        SiteChromeSurface::Shell => css::surfaceShell,
    };

    view! {
        <header class=format!("{} {}", css::archive, surface_class)>
            {children()}
        </header>
    }
}

#[component]
pub fn SiteChrome(
    route: Memo<RouteFrame>,
    /// Additional reactive children appended to the actions slot
    /// (after the nav, divider, and palette picker).
    /// Used by `RendererPage` to surface a per-page edit affordance.
    #[prop(optional, into)]
    extra_actions: Option<ChildrenFn>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let theme = ctx.theme;
    let identity_href = Signal::derive(|| "/".to_string());
    let session_name = Signal::derive(move || ctx.wallet.with(|wallet| wallet.display_name()));
    let network_name = Signal::derive(move || {
        ctx.wallet.with(|wallet| {
            wallet
                .chain_id()
                .map(|id| wallet::chain_name(id).to_ascii_lowercase())
                .unwrap_or_else(|| "offline".to_string())
        })
    });
    let breadcrumbs = Signal::derive(move || route_breadcrumb_items(&route.get()));
    let active_section = Signal::derive(move || {
        route
            .get()
            .request
            .url_path
            .trim_matches('/')
            .split('/')
            .next()
            .unwrap_or("")
            .to_string()
    });
    let websh_href = Signal::derive(move || {
        let frame = route.get();
        let cwd = match frame.surface() {
            RouteSurface::Shell | RouteSurface::Explorer => route_cwd(&frame),
            RouteSurface::Content => VirtualPath::root(),
        };
        route_href(&request_path_for_canonical_path(&cwd, RouteSurface::Shell))
    });
    let websh_active = Signal::derive(move || {
        let frame = route.get();
        matches!(
            frame.surface(),
            RouteSurface::Shell | RouteSurface::Explorer
        ) || active_section.get() == "websh"
    });

    view! {
        <SiteChromeRoot surface=SiteChromeSurface::Home>
            <SiteChromeLead>
                <SiteChromeIdentity label=APP_NAME href=identity_href />
                <SiteChromeChip label="session" value=session_name />
                <SiteChromeChip label="network" value=network_name />
            </SiteChromeLead>
            <SiteChromeBreadcrumb items=breadcrumbs />
            <SiteChromeActions>
                <SiteChromeNav>
                    <SiteChromeSiteNavItems
                        active_key=active_section
                        websh_href=websh_href
                        websh_active=websh_active
                    />
                </SiteChromeNav>
                <SiteChromeDivider />
                <SiteChromePalettePicker theme=theme />
                {extra_actions.map(|c| c())}
            </SiteChromeActions>
        </SiteChromeRoot>
    }
}

#[component]
pub fn SiteChromeIdentity(label: &'static str, href: Signal<String>) -> impl IntoView {
    view! {
        <a href=move || href.get() class=css::identity>{label}</a>
    }
}

#[component]
pub fn SiteChromeLead(children: Children) -> impl IntoView {
    view! {
        <div class=css::lead>{children()}</div>
    }
}

#[component]
pub fn SiteChromeBreadcrumb(
    items: Signal<Vec<SiteChromeBreadcrumbItem>>,
    #[prop(optional, default = "path")] aria_label: &'static str,
) -> impl IntoView {
    view! {
        <nav class=css::breadcrumb aria-label=aria_label>
            {move || {
                items
                    .get()
                    .into_iter()
                    .enumerate()
                    .map(|(idx, item)| {
                        let separator = (idx > 0).then(|| view! {
                            <span class=css::separator aria-hidden="true">"/"</span>
                        });
                        let class_name = if item.current {
                            css::crumbCurrent.to_string()
                        } else {
                            css::crumb.to_string()
                        };

                        view! {
                            <>
                                {separator}
                                {if let Some(href) = item.href {
                                    view! {
                                        <a href=href class=class_name>{item.label}</a>
                                    }.into_any()
                                } else {
                                    view! {
                                        <span class=class_name aria-current="location">{item.label}</span>
                                    }.into_any()
                                }}
                            </>
                        }
                    })
                    .collect_view()
            }}
        </nav>
    }
}

#[component]
pub fn SiteChromeActions(children: Children) -> impl IntoView {
    view! {
        <div class=css::actions>{children()}</div>
    }
}

#[component]
pub fn SiteChromeNav(children: Children) -> impl IntoView {
    view! {
        <nav class=css::nav aria-label="site navigation">{children()}</nav>
    }
}

#[component]
pub fn SiteChromeNavLink(
    label: &'static str,
    href: Signal<String>,
    active: Signal<bool>,
) -> impl IntoView {
    view! {
        <a
            href=move || href.get()
            class=move || {
                if active.get() {
                    format!("{} {}", css::navLink, css::navLinkActive)
                } else {
                    css::navLink.to_string()
                }
            }
            aria-current=move || if active.get() { "page" } else { "false" }
        >
            {label}
        </a>
    }
}

#[component]
pub fn SiteChromeSiteNavItems(
    active_key: Signal<String>,
    websh_href: Signal<String>,
    websh_active: Signal<bool>,
) -> impl IntoView {
    let ledger_active =
        Signal::derive(move || is_ledger_filter_route_segment(active_key.get().as_str()));

    view! {
        <SiteChromeNavLink
            label="home"
            href=Signal::derive(|| "/".to_string())
            active=Signal::derive(move || active_key.get().is_empty())
        />
        <SiteChromeNavLink
            label="ledger"
            href=Signal::derive(|| "/#/ledger".to_string())
            active=ledger_active
        />
        <SiteChromeNavLink
            label="websh"
            href=websh_href
            active=websh_active
        />
    }
}

#[component]
pub fn SiteChromeChip(label: &'static str, value: Signal<String>) -> impl IntoView {
    view! {
        <span class=css::chip>
            <span class=css::chipKey>{label}</span>
            <span class=css::chipValue>{value}</span>
        </span>
    }
}

#[component]
pub fn SiteChromeTextChip(value: Signal<String>) -> impl IntoView {
    view! {
        <span class=css::textChip>{value}</span>
    }
}

#[component]
pub fn SiteChromeDivider() -> impl IntoView {
    view! {
        <span class=css::divider aria-hidden="true"></span>
    }
}

#[component]
pub fn SiteChromePalettePicker(theme: RwSignal<&'static str>) -> impl IntoView {
    let (palette_open, set_palette_open) = signal(false);
    let toggle_palette = move |_| {
        set_palette_open.update(|open| *open = !*open);
    };
    let palette_keydown = move |ev: ev::KeyboardEvent| match ev.key().as_str() {
        "Escape" => set_palette_open.set(false),
        "ArrowDown" | "Enter" | " " => {
            ev.prevent_default();
            set_palette_open.set(true);
        }
        _ => {}
    };

    view! {
        <div class=css::themePicker>
            <button
                class=css::paletteTrigger
                type="button"
                title="Palette"
                aria-haspopup="listbox"
                aria-expanded=move || palette_open.get().to_string()
                on:click=toggle_palette
                on:keydown=palette_keydown
            >
                <span class=css::themeSwatch aria-hidden="true"></span>
                <span class=css::themeLabel>"palette"</span>
                <span class=css::paletteChevron aria-hidden="true">"v"</span>
            </button>
            <Show when=move || palette_open.get()>
                <button
                    class=css::paletteDismiss
                    type="button"
                    aria-label="Close palette menu"
                    on:click=move |_| set_palette_open.set(false)
                ></button>
                <div class=css::paletteMenu role="listbox" aria-label="Palette">
                    {THEMES.iter().map(|item| {
                        let id = item.id;
                        let label = item.label;
                        let bg = item.meta_color;
                        let accent = item.accent_color;
                        let option_class = move || {
                            if theme.get() == id {
                                format!("{} {}", css::paletteOption, css::paletteOptionActive)
                            } else {
                                css::paletteOption.to_string()
                            }
                        };
                        let select_theme = move |_| {
                            if let Ok(theme_id) = apply_theme(id) {
                                theme.set(theme_id);
                            }
                            set_palette_open.set(false);
                        };
                        view! {
                            <button
                                class=option_class
                                type="button"
                                role="option"
                                aria-selected=move || (theme.get() == id).to_string()
                                style=format!("--palette-bg: {bg}; --palette-accent: {accent}")
                                on:click=select_theme
                            >
                                <span class=css::paletteOptionSwatch aria-hidden="true"></span>
                                <span class=css::paletteOptionLabel>{label}</span>
                                <span class=css::paletteOptionStatus>
                                    {move || if theme.get() == id { "on" } else { "" }}
                                </span>
                            </button>
                        }
                    }).collect_view()}
                </div>
            </Show>
        </div>
    }
}

fn route_breadcrumb_items(frame: &RouteFrame) -> Vec<SiteChromeBreadcrumbItem> {
    match frame.surface() {
        RouteSurface::Content => content_breadcrumb_items(frame),
        RouteSurface::Shell => surface_breadcrumb_items("websh", RouteSurface::Shell, frame),
        RouteSurface::Explorer => {
            surface_breadcrumb_items("explorer", RouteSurface::Explorer, frame)
        }
    }
}

fn content_breadcrumb_items(frame: &RouteFrame) -> Vec<SiteChromeBreadcrumbItem> {
    if frame.request.url_path == "/ledger" {
        return canonical_breadcrumb_items(&VirtualPath::root(), RouteSurface::Content, None);
    }

    let path = VirtualPath::from_absolute(frame.request.url_path.clone()).unwrap_or_else(|_| {
        if frame.is_file() {
            frame.resolution.node_path.clone()
        } else {
            route_cwd(frame)
        }
    });
    canonical_breadcrumb_items(&path, RouteSurface::Content, None)
}

fn surface_breadcrumb_items(
    label: &'static str,
    surface: RouteSurface,
    frame: &RouteFrame,
) -> Vec<SiteChromeBreadcrumbItem> {
    canonical_breadcrumb_items(&route_cwd(frame), surface, Some(label))
}

fn canonical_breadcrumb_items(
    path: &VirtualPath,
    surface: RouteSurface,
    surface_label: Option<&'static str>,
) -> Vec<SiteChromeBreadcrumbItem> {
    let mut items = Vec::new();

    if path.is_root() && surface_label.is_none() {
        items.push(SiteChromeBreadcrumbItem::current("~"));
        return items;
    }

    items.push(SiteChromeBreadcrumbItem::link("~", "/"));

    if let Some(label) = surface_label {
        if path.is_root() {
            items.push(SiteChromeBreadcrumbItem::current(label));
            return items;
        }
        items.push(SiteChromeBreadcrumbItem::link(
            label,
            route_href(&request_path_for_canonical_path(
                &VirtualPath::root(),
                surface,
            )),
        ));
    }

    let segments = path.segments().collect::<Vec<_>>();
    for idx in 0..segments.len() {
        let label = segments[idx];
        if idx + 1 == segments.len() {
            items.push(SiteChromeBreadcrumbItem::current(label));
        } else {
            let path = VirtualPath::from_absolute(format!("/{}", segments[..=idx].join("/")))
                .expect("route path");
            items.push(SiteChromeBreadcrumbItem::link(
                label,
                route_href(&request_path_for_canonical_path(&path, surface)),
            ));
        }
    }

    items
}

fn route_href(path: &str) -> String {
    if path == "/" {
        "/".to_string()
    } else {
        format!("/#{path}")
    }
}
