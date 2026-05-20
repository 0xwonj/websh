use leptos::prelude::*;

use crate::features::dino_game::DinoGame;

use super::identifier_strip::IdentifierStrip;
use super::meta_table::{MetaRow, MetaTable};
use super::window_frame::{WindowActionButton, WindowFrame};

stylance::import_crate_style!(css, "src/shared/components/error_page.module.css");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ErrorActionCursor {
    Primary,
    Game,
}

#[derive(Clone, Copy)]
struct ErrorActionCursorState(RwSignal<ErrorActionCursor>);

#[derive(Clone)]
struct ErrorGameLauncher(Callback<()>);

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
    let game_active = RwSignal::new(false);
    let game_menu_request = RwSignal::new(0_u64);
    provide_context(ErrorGameLauncher(Callback::new(move |()| {
        game_active.set(true);
    })));

    let frame_title = Signal::derive(move || {
        if game_active.get() {
            "dino-game".to_string()
        } else {
            window_title.to_string()
        }
    });
    let screen_class = move || css::gameScreen.to_string();
    let recovery_pane_class = move || {
        if game_active.get() {
            format!("{} {}", css::screenPane, css::screenPaneInactive)
        } else {
            css::screenPane.to_string()
        }
    };
    let right_actions = view! {
        <Show when=move || game_active.get()>
            <WindowActionButton
                aria_label="Open game menu"
                on_click=Callback::new(move |()| game_menu_request.update(|request| *request += 1))
            >
                "☰"
            </WindowActionButton>
        </Show>
    }
    .into_any();

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
                        right_actions=right_actions
                        collapsed=error_window_collapsed_signal
                        body_id="error-recovery-console"
                    >
                        <div class=css::windowBody>
                            <div
                                class=screen_class
                                aria-label=move || if game_active.get() { "dino game" } else { "error recovery" }
                            >
                                <div class=recovery_pane_class aria-hidden=move || game_active.get().to_string()>
                                    <h3 class=css::gameTitle>{screen_title}</h3>
                                    {children()}
                                </div>
                                <Show when=move || game_active.get()>
                                    <div class=css::gamePane>
                                        <DinoGame
                                            autofocus=true
                                            menu_request=Signal::derive(move || game_menu_request.get())
                                            exit_label="EXIT"
                                            on_exit=Callback::new(move |()| game_active.set(false))
                                        />
                                    </div>
                                </Show>
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
    let launch_game = use_context::<ErrorGameLauncher>();
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
            <button
                class=format!("{} {}", css::actionButton, css::gameAction)
                type="button"
                on:pointerenter=move |_| selected.set(ErrorActionCursor::Game)
                on:focus=move |_| selected.set(ErrorActionCursor::Game)
                on:click=move |_| {
                    selected.set(ErrorActionCursor::Game);
                    if let Some(launcher) = launch_game.as_ref() {
                        launcher.0.run(());
                    }
                }
            >
                "Play game"
            </button>
        </div>
    }
}

#[component]
pub fn ErrorPageActionLink(href: &'static str, children: Children) -> impl IntoView {
    let cursor = use_context::<ErrorActionCursorState>();
    let select = move || {
        if let Some(cursor) = cursor {
            cursor.0.set(ErrorActionCursor::Primary);
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
