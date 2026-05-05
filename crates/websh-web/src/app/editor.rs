//! App-owned editor modal wiring.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use websh_core::domain::{ChangeType, EntryExtensions};
use websh_core::shell::SideEffect;

use super::AppContext;
use crate::shared::components::EditModal;

#[component]
pub fn AppEditModal() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext");
    let content = RwSignal::new(String::new());

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
            content.set(String::new());
            spawn_local(async move {
                let initial = ctx.read_text(&path).await.unwrap_or_default();
                if editor_open.get_untracked().as_ref() == Some(&path) {
                    content.set(initial);
                }
            });
        }
    });

    let on_save = Callback::new(move |body: String| {
        if let Some(path) = ctx.editor_open.get_untracked() {
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
            crate::features::terminal::dispatch_side_effect(
                &ctx,
                SideEffect::ApplyChange {
                    path,
                    change: Box::new(change),
                },
            );
            ctx.editor_open.set(None);
        }
    });

    let on_cancel = Callback::new(move |()| {
        ctx.editor_open.set(None);
    });

    view! {
        {move || {
            ctx.editor_open
                .get()
                .map(|path| view! {
                    <EditModal
                        path=path
                        content=content
                        on_save=on_save
                        on_cancel=on_cancel
                    />
                })
        }}
    }
}
