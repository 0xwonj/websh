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

use crate::app::AppContext;
use crate::core::SideEffect;
use crate::core::changes::ChangeType;

stylance::import_crate_style!(css, "src/components/editor/modal.module.css");

#[component]
pub fn EditModal() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext");
    let content = RwSignal::new(String::new());

    // When editor_open transitions to Some(path), seed the textarea with the
    // current view-fs content (staged overrides merged over base). For a new
    // file with no staged content, read_file returns None → start empty.
    Effect::new(move |_| {
        if let Some(path) = ctx.editor_open.get() {
            let initial = ctx
                .view_fs
                .with(|fs| fs.read_file(&path).unwrap_or_default());
            content.set(initial);
        }
    });

    let on_save = move |_| {
        if let Some(path) = ctx.editor_open.get_untracked() {
            let body = content.get_untracked();
            let is_existing = ctx
                .view_fs
                .with_untracked(|fs| fs.read_file(&path).is_some());
            let change = if is_existing {
                ChangeType::UpdateFile {
                    content: body,
                    description: None,
                }
            } else {
                ChangeType::CreateFile {
                    content: body,
                    meta: Default::default(),
                }
            };
            crate::components::terminal::dispatch_side_effect(
                &ctx,
                SideEffect::ApplyChange { path, change },
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
