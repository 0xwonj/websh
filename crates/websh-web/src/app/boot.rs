//! Application boot component and root effects.

use leptos::prelude::*;

use super::{AppContext, RuntimeServices};
use crate::features::RouterView;
use crate::shared::components::{
    ErrorPageActionButton, ErrorPageActionLink, ErrorPageActions, ErrorPageBody, ErrorPageDetails,
    ErrorPageTone, SiteContentFrame, SiteSurface,
};

/// Root application component with error boundary.
#[component]
pub fn App() -> impl IntoView {
    let ctx = AppContext::new();
    provide_context(ctx);
    let services = RuntimeServices::new(ctx);
    if let Err(error) = services.set_theme(ctx.theme.get_untracked()) {
        web_sys::console::error_1(&format!("theme hydration: {error}").into());
    }
    services.install_wallet_event_listeners();

    let boot_started = StoredValue::new(false);
    Effect::new(move |_| {
        if !boot_started.get_value() {
            boot_started.set_value(true);
            crate::features::terminal::boot::run(ctx);
        }
    });

    view! {
        <ErrorBoundary
            fallback=|errors| view! {
                <SiteSurface class="">
                    <SiteContentFrame class="">
                        <ErrorPageBody
                            tone=ErrorPageTone::Failure
                            code="render"
                            title="Something went wrong"
                            message="An unexpected error occurred while rendering this page."
                        >
                            <ErrorPageDetails summary="Error details" open=false>
                                <ul>
                                    {move || errors.get()
                                        .into_iter()
                                        .map(|(_, e)| view! { <li>{e.to_string()}</li> })
                                        .collect::<Vec<_>>()
                                    }
                                </ul>
                            </ErrorPageDetails>
                            <ErrorPageActions>
                                <ErrorPageActionButton on_click=Callback::new(move |()| {
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().reload();
                                    }
                                })>
                                    "Reload page"
                                </ErrorPageActionButton>
                                <ErrorPageActionLink href="#/">"Go home"</ErrorPageActionLink>
                            </ErrorPageActions>
                        </ErrorPageBody>
                    </SiteContentFrame>
                </SiteSurface>
            }
        >
            <RouterView />
        </ErrorBoundary>
    }
}
