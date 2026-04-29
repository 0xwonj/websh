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
                    <span
                        class=move || view_class.get()
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
                    </span>
                    <span class=css::modefnSep>"·"</span>
                    <span
                        class=move || edit_class.get()
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
                    </span>
                    <Show when=move || edit.mode.get() == ReaderMode::Edit>
                        <span class=css::modefnSep>"·"</span>
                        <span
                            class=css::modefnOpt
                            on:click=move |_| {
                                if !edit.saving.get_untracked() {
                                    edit.on_cancel.run(());
                                }
                            }
                        >"cancel"</span>
                        <span class=css::modefnSep>"·"</span>
                        <span
                            class=css::modefnOpt
                            on:click=move |_| {
                                if !edit.saving.get_untracked() {
                                    edit.on_save.run(());
                                }
                            }
                        >
                            "save"
                            <span class=css::modefnKbd>"⌘S"</span>
                        </span>
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_label_synced_when_clean_idle() {
        assert_eq!(state_label(false, false), "synced");
    }

    #[test]
    fn state_label_unsaved_when_dirty_idle() {
        assert_eq!(state_label(false, true), "unsaved");
    }

    #[test]
    fn state_label_saving_overrides_dirty() {
        assert_eq!(state_label(true, true), "saving…");
        assert_eq!(state_label(true, false), "saving…");
    }

    #[test]
    fn state_class_empty_when_clean_idle() {
        assert_eq!(state_class(false, false), "");
    }

    #[test]
    fn state_class_dirty_modifier_when_dirty() {
        assert_eq!(state_class(false, true), css::modefnStateDirty);
    }

    #[test]
    fn state_class_dirty_modifier_when_saving() {
        assert_eq!(state_class(true, false), css::modefnStateDirty);
    }
}
