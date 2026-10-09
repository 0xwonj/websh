//! Shared chrome, document metadata, reader actions, and attestation footer.

use leptos::prelude::*;

use crate::features::chrome::SiteChrome;
use crate::shared::components::PageSigFooter;
use websh_core::filesystem::RouteFrame;

use super::actions::ReaderActionsBindings;
use super::css;
use super::intent::ReaderIntent;
use super::meta::ReaderMeta;
use super::title_block::{Ident, TitleBlock};

/// Document-side reactive inputs to the shell — what the chrome, title
/// block, and footer need.
#[derive(Clone, Copy)]
pub struct ReaderShellState {
    pub ready: Signal<crate::runtime::content::ReadStatus>,
    pub intent: Memo<ReaderIntent>,
    pub meta: Memo<ReaderMeta>,
    pub chrome_route: Memo<RouteFrame>,
    pub attestation_route: Signal<String>,
    pub set_preferred_locale: Callback<String>,
}

#[component]
pub fn ReaderShell(
    state: ReaderShellState,
    actions: ReaderActionsBindings,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=css::surface>
            <SiteChrome route=state.chrome_route />
            <main class=css::page>
                <div class=css::content>
                    <Show when=move || !matches!(state.intent.get(), ReaderIntent::Redirect { .. })>
                        <Ident meta=state.meta />
                        <TitleBlock
                            intent=state.intent
                            meta=state.meta
                            actions=actions
                            set_preferred_locale=state.set_preferred_locale
                        />
                    </Show>
                    <div
                        class=css::readerBody
                        data-reader-body="true"
                        data-text-scale=move || actions.text_scale.get().attr()
                    >
                        {children()}
                    </div>
                </div>
                <PageSigFooter
                    route=state.attestation_route
                    ready=state.ready
                />
            </main>
        </div>
    }
}
