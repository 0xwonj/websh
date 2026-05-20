use leptos::prelude::*;

use super::identifier_strip::IdentifierStrip;
use super::meta_table::{MetaRow, MetaTable};
use super::window_frame::WindowFrame;

stylance::import_crate_style!(css, "src/shared/components/error_page.module.css");

const ERROR_GAME_HREF: &str = "#/game";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ErrorActionCursor {
    Primary,
    Game,
}

#[derive(Clone, Copy)]
struct ErrorActionCursorState(RwSignal<ErrorActionCursor>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorPageTone {
    Pending,
    Missing,
    Failure,
}

impl ErrorPageTone {
    const fn class(self) -> &'static str {
        match self {
            Self::Pending => css::tonePending,
            Self::Missing => css::toneMissing,
            Self::Failure => css::toneFailure,
        }
    }

    const fn role(self) -> &'static str {
        match self {
            Self::Pending => "status",
            Self::Missing => "region",
            Self::Failure => "alert",
        }
    }

    const fn aria_live(self) -> &'static str {
        match self {
            Self::Pending => "polite",
            Self::Missing => "polite",
            Self::Failure => "assertive",
        }
    }

    const fn http_status(self) -> &'static str {
        match self {
            Self::Pending => "HTTP/1.1 102",
            Self::Missing => "HTTP/1.1 404",
            Self::Failure => "HTTP/1.1 500",
        }
    }

    const fn reason(self) -> &'static str {
        match self {
            Self::Pending => "route pending",
            Self::Missing => "resource not found",
            Self::Failure => "request failed",
        }
    }

    const fn disposition(self) -> &'static str {
        match self {
            Self::Pending => "resource loading",
            Self::Missing => "page does not exist",
            Self::Failure => "page unavailable",
        }
    }

    const fn window_title(self) -> &'static str {
        match self {
            Self::Pending => "route-pending.md",
            Self::Missing => "page-not-found.md",
            Self::Failure => "route-failure.md",
        }
    }

    const fn screen_title(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Missing => "404",
            Self::Failure => "error",
        }
    }
}

#[component]
pub fn ErrorPageFrame(children: Children) -> impl IntoView {
    view! {
        <div class=css::routeFrame>
            {children()}
        </div>
    }
}

#[component]
pub fn ErrorPageBody(
    tone: ErrorPageTone,
    code: &'static str,
    title: &'static str,
    message: &'static str,
    #[prop(optional, into)] meta_resolved: Option<String>,
    #[prop(optional, into)] meta_apology: Option<String>,
    children: Children,
) -> impl IntoView {
    let class = format!("{} {}", css::body, tone.class());
    let status = if tone == ErrorPageTone::Missing {
        format!("HTTP/1.1 {code}")
    } else {
        tone.http_status().to_string()
    };
    let reason = tone.reason();
    let disposition = tone.disposition();
    let window_title = tone.window_title();
    let screen_title = tone.screen_title();
    let error_window_collapsed_signal = Signal::derive(|| false);
    let left_controls = view! {
        <span class=format!("{} {}", css::trafficDot, css::trafficClose) aria-hidden="true"></span>
        <span class=format!("{} {}", css::trafficDot, css::trafficMinimize) aria-hidden="true"></span>
        <span class=format!("{} {}", css::trafficDot, css::trafficZoom) aria-hidden="true"></span>
    }
    .into_any();
    let frame_title = Signal::derive(move || window_title.to_string());

    view! {
        <section class=class role=tone.role() aria-live=tone.aria_live() aria-label=title>
            <IdentifierStrip>
                <span class=css::statusLeft>
                    <span>{status}</span>
                    <span class=css::statusDim>" · "{reason}</span>
                </span>
                <span>{disposition}</span>
            </IdentifierStrip>
            <div class=css::titleBlock>
                <div class=css::titleRow>
                    <h1 class=css::title>{title}</h1>
                </div>
                <MetaTable class=css::metaTable aria_label="error metadata">
                    <MetaRow
                        label="Type"
                        row_class=css::metaRow
                        key_class=css::metaKey
                        value_class=css::metaValue
                    >
                        <span class=css::metaTag>"error"</span>
                    </MetaRow>
                    {meta_resolved.map(|resolved| view! {
                        <MetaRow
                            label="Resolved"
                            row_class=css::metaRow
                            key_class=css::metaKey
                            value_class=css::metaValue
                        >
                            <code class=css::metaCode>{resolved}</code>
                        </MetaRow>
                    })}
                    {meta_apology.map(|apology| view! {
                        <MetaRow
                            label="Apology"
                            row_class=css::metaRow
                            key_class=css::metaKey
                            value_class=css::metaValue
                        >
                            <span class=css::metaJoke>{apology}</span>
                        </MetaRow>
                    })}
                </MetaTable>
            </div>
            <div class=css::documentBody>
                <h2 class=css::sectionTitle>"Description"</h2>
                <p class=css::sectionDescription>{message}</p>
                <div class=css::errorWindow>
                    <WindowFrame
                        title=frame_title
                        left_controls=left_controls
                        collapsed=error_window_collapsed_signal
                        body_id="error-recovery-console"
                    >
                        <div class=css::windowBody>
                            <div class=css::gameScreen aria-label="error recovery">
                                <h3 class=css::gameTitle>{screen_title}</h3>
                                {children()}
                            </div>
                        </div>
                    </WindowFrame>
                </div>
            </div>
        </section>
    }
}

#[component]
pub fn ErrorPageDetails(summary: &'static str, open: bool, children: Children) -> impl IntoView {
    let _ = open;

    view! {
        <div class=css::details>
            <div class=css::summary>{summary}</div>
            <div class=css::detailBody>
                {children()}
            </div>
        </div>
    }
}

#[component]
pub fn ErrorPageActions(children: Children) -> impl IntoView {
    let selected = RwSignal::new(ErrorActionCursor::Primary);
    provide_context(ErrorActionCursorState(selected));
    let actions_class = move || {
        if selected.get() == ErrorActionCursor::Game {
            format!("{} {}", css::actions, css::actionsGameSelected)
        } else {
            css::actions.to_string()
        }
    };

    view! {
        <div class=actions_class>
            <div class=css::actionPrompt>"CHOOSE NEXT STEP"</div>
            {children()}
            <a
                class=format!("{} {}", css::actionLink, css::gameAction)
                href=ERROR_GAME_HREF
                on:pointerenter=move |_| selected.set(ErrorActionCursor::Game)
                on:focus=move |_| selected.set(ErrorActionCursor::Game)
            >
                "Play game"
            </a>
        </div>
    }
}

#[component]
pub fn ErrorPageActionLink(href: &'static str, children: Children) -> impl IntoView {
    let cursor = use_context::<ErrorActionCursorState>();
    let kind = if href == ERROR_GAME_HREF {
        ErrorActionCursor::Game
    } else {
        ErrorActionCursor::Primary
    };
    let select = move || {
        if let Some(cursor) = cursor {
            cursor.0.set(kind);
        }
    };

    view! {
        <a
            class=css::actionLink
            href=href
            on:pointerenter=move |_| select()
            on:focus=move |_| select()
        >
            {children()}
        </a>
    }
}

#[component]
pub fn ErrorPageActionButton(on_click: Callback<()>, children: Children) -> impl IntoView {
    let cursor = use_context::<ErrorActionCursorState>();
    let select = move || {
        if let Some(cursor) = cursor {
            cursor.0.set(ErrorActionCursor::Primary);
        }
    };

    view! {
        <button
            class=css::actionButton
            type="button"
            on:pointerenter=move |_| select()
            on:focus=move |_| select()
            on:click=move |_| on_click.run(())
        >
            {children()}
        </button>
    }
}
