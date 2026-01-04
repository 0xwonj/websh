//! Terminal input component with autocompletion and history navigation.

use leptos::{ev, prelude::*};
use leptos::prelude::CollectView;
use wasm_bindgen::JsCast;

use super::hooks::{HintState, TabCycleState};
use crate::core::AutocompleteResult;

stylance::import_crate_style!(css, "src/components/terminal/input.module.css");

/// Terminal input field with autocomplete, history navigation, and ghost text hints.
#[component]
pub fn Input(
    #[prop(into)] prompt: Signal<String>,
    on_submit: Callback<String>,
    on_history_nav: Callback<i32, Option<String>>,
    on_autocomplete: Callback<String, AutocompleteResult>,
    on_get_hint: Callback<String, Option<String>>,
) -> impl IntoView {
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let (input_value, set_input_value) = signal(String::new());

    // State management using custom hooks
    let tab_state = TabCycleState::new();
    let hint_state = HintState::new();

    // Focus input on mount
    Effect::new(move || {
        if let Some(input) = input_ref.get() {
            let _ = input.focus();
        }
    });

    // Helper to move cursor to end of input
    let move_cursor_to_end = move || {
        if let Some(input) = input_ref.get() {
            let len = input.value().len() as u32;
            let _ = input.set_selection_range(len, len);
        }
    };

    // Reset all transient state
    let reset_state = move || {
        tab_state.clear();
        hint_state.clear();
    };

    // Handle Tab key for autocompletion
    let handle_tab = {
        move |value: String| -> Option<String> {
            if value.is_empty() {
                return None;
            }

            if tab_state.is_active() {
                // Already cycling through matches - advance to next
                tab_state.advance();
                tab_state.build_completion()
            } else {
                // First Tab press - get autocomplete result
                match on_autocomplete.run(value.clone()) {
                    AutocompleteResult::Single(completed) => {
                        hint_state.clear();
                        Some(completed)
                    }
                    AutocompleteResult::Multiple(common, matches) => {
                        hint_state.clear();
                        // Start cycling mode
                        tab_state.start(common.clone(), matches);
                        // Return common prefix if it extends, or first match if not
                        if common.len() > value.len() {
                            Some(common)
                        } else {
                            // Common prefix doesn't extend input, show first match
                            tab_state.build_completion()
                        }
                    }
                    AutocompleteResult::None => None,
                }
            }
        }
    };

    // Handle ArrowRight to accept hint
    let handle_arrow_right = move |input_value: &str| -> Option<String> {
        if let Some(input) = input_ref.get() {
            let pos = input.selection_start().ok().flatten().unwrap_or(0) as usize;
            if pos == input_value.len()
                && let Some(h) = hint_state.get()
            {
                hint_state.clear();
                return Some(format!("{}{}", input_value, h));
            }
        }
        None
    };

    let handle_keydown = move |ev: ev::KeyboardEvent| {
        match ev.key().as_str() {
            "Tab" => {
                ev.prevent_default();
                if let Some(completed) = handle_tab(input_value.get()) {
                    set_input_value.set(completed);
                    move_cursor_to_end();
                }
            }
            "Enter" => {
                reset_state();
                let value = input_value.get();
                on_submit.run(value);
                set_input_value.set(String::new());
            }
            "ArrowUp" => {
                ev.prevent_default();
                reset_state();
                if let Some(cmd) = on_history_nav.run(-1) {
                    set_input_value.set(cmd);
                    move_cursor_to_end();
                }
            }
            "ArrowDown" => {
                ev.prevent_default();
                reset_state();
                if let Some(cmd) = on_history_nav.run(1) {
                    set_input_value.set(cmd);
                } else {
                    set_input_value.set(String::new());
                }
            }
            "ArrowRight" => {
                let value = input_value.get();
                if let Some(completed) = handle_arrow_right(&value) {
                    ev.prevent_default();
                    set_input_value.set(completed);
                    move_cursor_to_end();
                }
            }
            "c" if ev.ctrl_key() => {
                reset_state();
                set_input_value.set(String::new());
            }
            "l" if ev.ctrl_key() => {
                ev.prevent_default();
                reset_state();
                on_submit.run("clear".to_string());
            }
            "Escape" => {
                reset_state();
            }
            _ => {
                // Clear Tab cycling state on other keys
                tab_state.clear();
            }
        }
    };

    let handle_input = move |ev: ev::Event| {
        let Some(target) = ev.target() else { return };
        let input = target.unchecked_into::<web_sys::HtmlInputElement>();
        let value = input.value();
        set_input_value.set(value.clone());
        tab_state.clear();

        // Update ghost text hint
        if value.is_empty() {
            hint_state.clear();
        } else {
            hint_state.set(on_get_hint.run(value));
        }
    };

    // View for suggestions list
    let suggestions_view = move || {
        let matches = tab_state.matches.get();
        let idx = tab_state.index.get();
        if matches.is_empty() {
            None
        } else {
            Some(view! {
                <div class=css::suggestions>
                    {matches.into_iter().enumerate().map(|(i, s)| {
                        let is_active = i == idx;
                        let class_name = if is_active {
                            format!("{} {}", css::suggestion, css::suggestionActive)
                        } else {
                            css::suggestion.to_string()
                        };
                        view! {
                            <span class=class_name>
                                {s}
                            </span>
                        }
                    }).collect_view()}
                </div>
            })
        }
    };

    view! {
        <div class=css::inputWrapper>
            <div class=css::line>
                <span class=css::prompt>{prompt}</span>
                <span class=css::separator>"$ "</span>
                <div class=css::field>
                    // Ghost text overlay (shows input value + hint)
                    <div class=css::ghostOverlay>
                        <span class=css::ghostText>{move || input_value.get()}</span>
                        <span class=css::ghostHint>
                            {move || hint_state.hint.get().unwrap_or_default()}
                        </span>
                    </div>
                    <input
                        node_ref=input_ref
                        type="text"
                        class=css::input
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=input_value
                        on:input=handle_input
                        on:keydown=handle_keydown
                    />
                </div>
            </div>

            // Show current Tab cycling matches
            {suggestions_view}
        </div>
    }
}
