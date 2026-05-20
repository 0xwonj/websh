use leptos::prelude::*;

stylance::import_crate_style!(css, "src/shared/components/window_frame.module.css");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowTrafficTone {
    Close,
    Minimize,
    Zoom,
}

impl WindowTrafficTone {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::Minimize => "minimize",
            Self::Zoom => "zoom",
        }
    }
}

#[component]
pub fn WindowFrame(
    #[prop(into)] title: Signal<String>,
    #[prop(optional)] left_controls: Option<AnyView>,
    #[prop(optional)] right_actions: Option<AnyView>,
    #[prop(into)] collapsed: Signal<bool>,
    #[prop(optional)] body_id: Option<&'static str>,
    #[prop(default = NodeRef::<leptos::html::Div>::new())] node_ref: NodeRef<leptos::html::Div>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=css::frame node_ref=node_ref>
            <header class=css::topBar>
                <span class=css::traffic>{left_controls}</span>
                <span class=css::title>{move || title.get()}</span>
                <span class=css::actions>{right_actions}</span>
            </header>
            <div
                class=css::body
                id=body_id
                prop:hidden=move || collapsed.get()
            >
                {children()}
            </div>
        </div>
    }
}

#[component]
pub fn WindowTrafficButton(
    tone: WindowTrafficTone,
    aria_label: &'static str,
    #[prop(optional)] aria_controls: Option<&'static str>,
    #[prop(optional)] aria_expanded: Option<Signal<bool>>,
    on_click: Callback<()>,
) -> impl IntoView {
    let tone = tone.as_str();

    view! {
        <button
            class=css::trafficButton
            data-tone=tone
            type="button"
            aria-label=aria_label
            aria-controls=aria_controls
            aria-expanded=move || aria_expanded.map(|expanded| expanded.get().to_string())
            on:click=move |_| on_click.run(())
        ></button>
    }
}

#[component]
pub fn WindowTrafficLink(href: &'static str, aria_label: &'static str) -> impl IntoView {
    view! {
        <a
            class=css::trafficButton
            data-tone=WindowTrafficTone::Zoom.as_str()
            href=href
            aria-label=aria_label
        ></a>
    }
}

#[component]
pub fn WindowActionLink(
    #[prop(into)] href: Signal<String>,
    #[prop(optional)] aria_label: Option<&'static str>,
    #[prop(optional)] download: bool,
    #[prop(optional)] external: bool,
    children: Children,
) -> AnyView {
    if download {
        view! {
            <a class=css::actionLink href=move || href.get() aria-label=aria_label download="">
                {children()}
            </a>
        }
        .into_any()
    } else if external {
        view! {
            <a class=css::actionLink href=move || href.get() aria-label=aria_label target="_blank" rel="noopener">
                {children()}
            </a>
        }
        .into_any()
    } else {
        view! {
            <a class=css::actionLink href=move || href.get() aria-label=aria_label>
                {children()}
            </a>
        }
        .into_any()
    }
}

#[component]
pub fn WindowActionButton(
    #[prop(optional)] aria_label: Option<&'static str>,
    on_click: Callback<()>,
    children: Children,
) -> AnyView {
    view! {
        <button
            class=css::actionLink
            type="button"
            aria-label=aria_label
            on:click=move |_| on_click.run(())
        >
            {children()}
        </button>
    }
    .into_any()
}
