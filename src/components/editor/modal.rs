//! Minimal edit modal: textarea + Save/Cancel.
//!
//! # Contract
//!
//! The modal is a UI surface, **not** a mutation source. It never calls
//! `ctx.changes.update(...)` directly. Save always emits
//! `SideEffect::ApplyChange` through `dispatch_side_effect`, which keeps
//! persistence, autosave debounce, and any future hooks in one place
//! (spec §9.1).
//!
//! Visibility is driven entirely by `ctx.editor_open: RwSignal<Option<VirtualPath>>`:
//! `Some(path)` = open, `None` = closed.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::models::EntryExtensions;

use crate::app::AppContext;
use crate::core::SideEffect;
use crate::core::changes::ChangeType;

stylance::import_crate_style!(css, "src/components/editor/modal.module.css");

#[component]
pub fn EditModal() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext");
    let content = RwSignal::new(String::new());

    // When editor_open transitions to Some(path), seed the textarea with the
    // current runtime content. Pending text wins; otherwise the app runtime
    // read facade resolves the responsible backend.
    Effect::new(move |_| {
        if let Some(path) = ctx.editor_open.get() {
            let fs = ctx.view_global_fs.get();
            if let Some(initial) = fs.read_pending_text(&path) {
                content.set(initial);
                return;
            }

            if !fs.exists(&path) {
                content.set(String::new());
                return;
            }

            let editor_open = ctx.editor_open;
            let content_signal = content;
            content.set(String::new());
            spawn_local(async move {
                let initial = ctx.read_text(&path).await.unwrap_or_default();
                if editor_open.get_untracked().as_ref() == Some(&path) {
                    content_signal.set(initial);
                }
            });
        }
    });

    let on_save = move |_| {
        if let Some(path) = ctx.editor_open.get_untracked() {
            let body = content.get_untracked();
            let is_existing = ctx.view_global_fs.with_untracked(|fs| fs.exists(&path));
            let change = if is_existing {
                ChangeType::UpdateFile {
                    content: body,
                    meta: None,
                    extensions: None,
                }
            } else {
                ChangeType::CreateFile {
                    content: body,
                    meta: Default::default(),
                    extensions: EntryExtensions::default(),
                }
            };
            crate::components::terminal::dispatch_side_effect(
                &ctx,
                SideEffect::ApplyChange {
                    path,
                    change: Box::new(change),
                },
            );
            ctx.editor_open.set(None);
        }
    };

    let on_cancel = move |_| {
        ctx.editor_open.set(None);
    };

    view! {
        <Show when=move || ctx.editor_open.get().is_some()>
            <div class=css::backdrop on:click=on_cancel>
                <div
                    class=css::modal
                    on:click=|ev: leptos::ev::MouseEvent| ev.stop_propagation()
                >
                    <header class=css::header>
                        <span class=css::path>
                            {move || {
                                ctx.editor_open
                                    .get()
                                    .map(|p| p.as_str().to_string())
                                    .unwrap_or_default()
                            }}
                        </span>
                    </header>
                    <textarea
                        class=css::textarea
                        prop:value=move || content.get()
                        on:input=move |ev| content.set(event_target_value(&ev))
                    />
                    <footer class=css::footer>
                        <button class=css::cancel on:click=on_cancel>"Cancel"</button>
                        <button class=css::save on:click=on_save>"Save"</button>
                    </footer>
                </div>
            </div>
        </Show>
    }
}
