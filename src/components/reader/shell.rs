//! `ReaderShell` — outer chrome shared by every reader view.
//!
//! All view modes (markdown / html / plain / pdf / asset / redirect, plus
//! the edit textarea) sit inside the same surface: site chrome, an
//! identifier strip, a title block + meta table, the body slot, the
//! attestation footer, and the toolbar after the footer.
//!
//! The shell takes the body as `children` and bundles every other piece
//! so `Reader` can stay focused on state and dispatch.

use leptos::prelude::*;

use crate::components::chrome::SiteChrome;
use crate::components::shared::AttestationSigFooter;
use crate::core::engine::RouteFrame;

use super::ReaderMode;
use super::css;
use super::intent::ReaderIntent;
use super::meta::ReaderMeta;
use super::title_block::{Ident, TitleBlock};
use super::toolbar::ReaderToolbar;

#[component]
#[allow(clippy::too_many_arguments)]
pub fn ReaderShell(
    intent: Memo<ReaderIntent>,
    meta: Memo<ReaderMeta>,
    chrome_route: Memo<RouteFrame>,
    #[prop(into)] attestation_route: Signal<String>,
    #[prop(into)] show_pending: Signal<bool>,
    save_error: ReadSignal<Option<String>>,
    mode: RwSignal<ReaderMode>,
    can_edit: Memo<bool>,
    saving: ReadSignal<bool>,
    dirty: ReadSignal<bool>,
    on_edit: Callback<()>,
    on_preview: Callback<()>,
    on_save: Callback<()>,
    on_cancel: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class=css::surface>
            <SiteChrome route=chrome_route />
            <main class=css::page>
                <Show when=move || !matches!(intent.get(), ReaderIntent::Redirect { .. })>
                    <Ident meta=meta />
                    <TitleBlock intent=intent meta=meta />
                </Show>
                {move || save_error.get().map(|message| view! {
                    <div class=css::errorBanner role="alert">{message}</div>
                })}
                {children()}
                <AttestationSigFooter
                    route=attestation_route
                    show_pending=show_pending
                />
                <ReaderToolbar
                    mode=mode
                    can_edit=can_edit
                    saving=saving
                    dirty=dirty
                    on_edit=on_edit
                    on_preview=on_preview
                    on_save=on_save
                    on_cancel=on_cancel
                />
            </main>
        </div>
    }
}
