//! Terminal view component.
//!
//! The terminal interface with output history and command input.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::features::terminal::{Input, Output, RouteContext};
use crate::platform::dom::focus_terminal_input;
use websh_core::filesystem::route_cwd;

use super::actions::{
    create_autocomplete_callback, create_hint_callback, create_history_nav_callback,
    create_submit_callback,
};

stylance::import_crate_style!(css, "src/features/terminal/terminal.module.css");

#[component]
pub fn Terminal(output_ref: NodeRef<leptos::html::Div>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    // Derived signals
    let prompt = Signal::derive(move || {
        let route = route_ctx.0.get();
        ctx.get_prompt(&route_cwd(&route))
    });

    // Callbacks need route access
    let on_submit = create_submit_callback(ctx, route_ctx);
    let on_history_nav = create_history_nav_callback(ctx);
    let on_autocomplete = create_autocomplete_callback(ctx, route_ctx);
    let on_get_hint = create_hint_callback(ctx, route_ctx);

    let handle_click = move |_| focus_terminal_input();
    let history_signal = ctx.terminal.history;

    view! {
        <div class=css::container on:click=handle_click>
            <div
                node_ref=output_ref
                class=css::output
                role="log"
                aria-live="polite"
                aria-relevant="additions text"
                aria-label="Terminal output"
            >
                <For
                    each=move || history_signal.with(|buf| buf.iter().cloned().collect::<Vec<_>>())
                    key=|line| line.id
                    children=|line| view! { <Output line=line /> }
                />
            </div>

            <div class=css::inputArea>
                <Input
                    prompt=prompt
                    on_submit=on_submit
                    on_history_nav=on_history_nav
                    on_autocomplete=on_autocomplete
                    on_get_hint=on_get_hint
                />
            </div>
        </div>
    }
}
