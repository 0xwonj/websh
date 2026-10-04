//! Application boot component and root effects.

use leptos::prelude::*;

use super::AppContext;
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
    install_theme_effect(ctx);
    ctx.wallet.install_listeners();

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

/// The preference snapshot is the single source of truth for the visual palette.
pub(super) fn install_theme_effect(ctx: AppContext) {
    Effect::new(move |_| crate::render::theme::apply_theme_to_document(ctx.theme.get()));
}

#[cfg(test)]
pub(super) fn init_test_renderer() {
    use wasm_bindgen::JsCast;
    let element = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("div")
        .unwrap()
        .unchecked_into();
    // Mounting initializes Leptos's browser executor, as the real entrypoint does.
    let _ = leptos::mount::mount_to(element, || ());
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen(inline_js = r#"
    export function denyPreferenceStorage() {
        const descriptor = Object.getOwnPropertyDescriptor(window, 'localStorage');
        Object.defineProperty(window, 'localStorage', {
            configurable: true,
            get() { throw new DOMException('unavailable', 'SecurityError'); }
        });
        return () => Object.defineProperty(window, 'localStorage', descriptor);
    }
    "#)]
    extern "C" {
        #[wasm_bindgen(js_name = denyPreferenceStorage)]
        fn deny_storage() -> js_sys::Function;
    }

    #[wasm_bindgen_test]
    async fn default_theme_is_applied_without_writing_preferences() {
        init_test_renderer();
        let restore = deny_storage();
        let owner = Owner::new();
        let ctx = owner.with(|| {
            let ctx = AppContext::new();
            install_theme_effect(ctx);
            ctx
        });
        gloo_timers::future::TimeoutFuture::new(0).await;
        let document = web_sys::window().unwrap().document().unwrap();
        assert_eq!(
            ctx.theme.get_untracked(),
            crate::render::theme::DEFAULT_THEME
        );
        assert_eq!(
            document
                .document_element()
                .unwrap()
                .get_attribute("data-theme")
                .as_deref(),
            Some(crate::render::theme::DEFAULT_THEME)
        );
        assert!(
            !ctx.preferences
                .snapshot
                .with_untracked(|state| state.env.contains_key("THEME"))
        );
        owner.cleanup();
        restore.call0(&JsValue::NULL).unwrap();
    }
}
