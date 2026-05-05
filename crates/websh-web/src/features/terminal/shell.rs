//! Main shell component.
//!
//! Container component for the terminal surface. Receives the current route
//! from the parent Router component and passes it to child components via
//! context.

use leptos::prelude::*;

use super::terminal::Terminal;
use crate::app::AppContext;
use crate::features::chrome::SiteChrome;
use crate::platform::dom::replace_route;
use websh_core::filesystem::{
    RouteFrame, RouteSurface, request_path_for_canonical_path, route_cwd,
};
use websh_core::shell::OutputLine;

stylance::import_crate_style!(css, "src/features/terminal/shell.module.css");

/// Context for accessing the current route from any component.
///
/// This allows child components to access the current route without prop
/// drilling.
#[derive(Clone, Copy)]
pub struct RouteContext(pub Memo<RouteFrame>);

/// Auto-scroll output to bottom when history changes.
fn setup_autoscroll_effect(
    history: RwSignal<crate::app::RingBuffer<OutputLine>>,
    output_ref: NodeRef<leptos::html::Div>,
) {
    Effect::new(move || {
        history.track();
        if let Some(el) = output_ref.get() {
            el.set_scroll_top(el.scroll_height());
        }
    });
}

/// Shell component for the terminal view.
///
/// This is a container component that:
/// - Receives the current route from the Router
/// - Provides route context to child components
/// - Handles boot sequence initialization
/// - Provides terminal surface effects
///
/// # Props
/// - `route`: The current route frame (derived from URL + engine resolution)
#[component]
pub fn Shell(route: Memo<RouteFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided at root");

    // Provide route context for child components
    provide_context(RouteContext(route));

    Effect::new(move |_| {
        let frame = route.get();
        ctx.cwd.set(route_cwd(&frame));
        match frame.surface() {
            RouteSurface::Shell => {
                let canonical =
                    request_path_for_canonical_path(&route_cwd(&frame), RouteSurface::Shell);
                if frame.request.url_path != canonical {
                    replace_route(&websh_core::filesystem::RouteRequest::new(canonical));
                }
            }
            RouteSurface::Content => {}
        }
    });

    let output_ref = NodeRef::<leptos::html::Div>::new();

    setup_autoscroll_effect(ctx.terminal.history, output_ref);

    view! {
        <div class=css::screen>
            <SiteChrome route=route />

            <div class=css::main>
                <Terminal output_ref=output_ref />
            </div>
        </div>
    }
}
