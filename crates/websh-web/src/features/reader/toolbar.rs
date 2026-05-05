//! Footnote-mark toolbar for the reader.
//!
//! Renders mode toggles (rendered ↔ edit) plus cancel/save actions when
//! in Edit, with a right-side state chip showing dirty / saving / synced.
//! Visibility follows `edit_visible`; the bar is hidden entirely for
//! non-author / non-mempool routes.

use leptos::prelude::*;

use super::ReaderMode;
use super::css;
use super::shell::ReaderEditBindings;

#[component]
pub fn ReaderToolbar(edit: ReaderEditBindings) -> impl IntoView {
    let visible = Memo::new(move |_| {
        edit.mode.get() == ReaderMode::Edit
            || (edit.mode.get() == ReaderMode::View && edit.can_edit.get())
    });

    let view_class = Memo::new(move |_| {
        if edit.mode.get() == ReaderMode::View {
            format!("{} {}", css::modefnOpt, css::modefnOptOn)
        } else {
            css::modefnOpt.to_string()
        }
    });
    let edit_class = Memo::new(move |_| {
        if edit.mode.get() == ReaderMode::Edit {
            format!("{} {}", css::modefnOpt, css::modefnOptOn)
        } else {
            css::modefnOpt.to_string()
        }
    });
    let state_text = Memo::new(move |_| state_label(edit.saving.get(), edit.dirty.get()));
    let state_class_name = Memo::new(move |_| {
        let modifier = state_class(edit.saving.get(), edit.dirty.get());
        if modifier.is_empty() {
            css::modefnState.to_string()
        } else {
            format!("{} {}", css::modefnState, modifier)
        }
    });

    view! {
        <Show when=move || visible.get()>
            <div class=css::modefn>
                <div class=css::modefnRow>
                    <span class=css::modefnMark>"*"</span>
                    <span class=css::modefnLab>"mode"</span>
                    <button
                        type="button"
                        class=move || view_class.get()
                        aria-pressed=move || (edit.mode.get() == ReaderMode::View).to_string()
                        on:click=move |_| {
                            if edit.mode.get_untracked() == ReaderMode::Edit
                                && !edit.saving.get_untracked()
                            {
                                edit.on_preview.run(());
                            }
                        }
                    >
                        "rendered"
                        <span class=css::modefnKbd>"r"</span>
                    </button>
                    <span class=css::modefnSep>"·"</span>
                    <button
                        type="button"
                        class=move || edit_class.get()
                        aria-pressed=move || (edit.mode.get() == ReaderMode::Edit).to_string()
                        on:click=move |_| {
                            if edit.mode.get_untracked() == ReaderMode::View
                                && edit.can_edit.get_untracked()
                            {
                                edit.on_edit.run(());
                            }
                        }
                    >
                        "edit"
                        <span class=css::modefnKbd>"e"</span>
                    </button>
                    <Show when=move || edit.mode.get() == ReaderMode::Edit>
                        <span class=css::modefnSep>"·"</span>
                        <button
                            type="button"
                            class=css::modefnOpt
                            disabled=move || edit.saving.get()
                            on:click=move |_| {
                                if !edit.saving.get_untracked() {
                                    edit.on_cancel.run(());
                                }
                            }
                        >"cancel"</button>
                        <span class=css::modefnSep>"·"</span>
                        <button
                            type="button"
                            class=css::modefnOpt
                            disabled=move || edit.saving.get()
                            on:click=move |_| {
                                if !edit.saving.get_untracked() {
                                    edit.on_save.run(());
                                }
                            }
                        >
                            "save"
                            <span class=css::modefnKbd>"⌘S"</span>
                        </button>
                    </Show>
                    <span class=css::modefnSpacer></span>
                    <span class=move || state_class_name.get()>{move || state_text.get()}</span>
                </div>
            </div>
        </Show>
    }
}

fn state_label(saving: bool, dirty: bool) -> &'static str {
    if saving {
        "saving…"
    } else if dirty {
        "unsaved"
    } else {
        "synced"
    }
}

fn state_class(saving: bool, dirty: bool) -> &'static str {
    if saving || dirty {
        css::modefnStateDirty
    } else {
        ""
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn state_labels_and_classes() {
        let cases = [
            (false, false, "synced", ""),
            (false, true, "unsaved", css::modefnStateDirty),
            (true, true, "saving…", css::modefnStateDirty),
            (true, false, "saving…", css::modefnStateDirty),
        ];

        for (saving, dirty, label, class) in cases {
            assert_eq!(state_label(saving, dirty), label);
            assert_eq!(state_class(saving, dirty), class);
        }
    }
}
