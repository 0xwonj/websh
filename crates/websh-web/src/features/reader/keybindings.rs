use leptos::prelude::*;

use super::ReaderMode;

#[derive(Clone, Copy)]
pub(super) struct KeybindingTargets {
    pub(super) mode: RwSignal<ReaderMode>,
    pub(super) edit_visible: Memo<bool>,
    pub(super) saving: ReadSignal<bool>,
    pub(super) on_save: Callback<()>,
    pub(super) on_preview: Callback<()>,
    pub(super) on_toggle_edit: Callback<()>,
}

#[cfg(target_arch = "wasm32")]
pub(super) fn install_reader_keybindings(targets: KeybindingTargets) {
    use crate::platform::wasm_cleanup::WasmCleanup;
    use leptos::prelude::on_cleanup;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(window) = web_sys::window() else {
        return;
    };

    let closure = Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
        let mode_now = targets.mode.get_untracked();
        let in_textarea = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
            .is_some();

        if (ev.meta_key() || ev.ctrl_key()) && ev.key() == "s" {
            ev.prevent_default();
            if mode_now == ReaderMode::Edit && !targets.saving.get_untracked() {
                targets.on_save.run(());
            }
            return;
        }

        if in_textarea || ev.meta_key() || ev.ctrl_key() || ev.alt_key() {
            return;
        }

        match ev.key().as_str() {
            "r" if mode_now == ReaderMode::Edit && !targets.saving.get_untracked() => {
                targets.on_preview.run(());
            }
            "e" if mode_now == ReaderMode::View && targets.edit_visible.get_untracked() => {
                targets.on_toggle_edit.run(());
            }
            _ => {}
        }
    }) as Box<dyn Fn(web_sys::KeyboardEvent)>);

    let _ = window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());

    let cleanup = WasmCleanup(closure);
    on_cleanup(move || {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback("keydown", cleanup.js_function());
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn install_reader_keybindings(_targets: KeybindingTargets) {}
