//! Un-modal'd compose form for mempool authoring.
//!
//! `MempoolEditor` is the page-level form rendered by `MempoolEditorPage`
//! for the `/#/new` and `/#/edit/<path>` routes (Phase 6). It owns the form
//! state, validation, and the save call; the parent page owns navigation.
//!
//! Extracted from the now-deleted `ComposeModal` (Phase 2). Form layout and
//! validation rules are unchanged; only the modal frame (backdrop, panel,
//! header with close button, Esc-to-close inside the panel) is gone.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::models::VirtualPath;
use crate::utils::current_timestamp;
use crate::utils::format::format_date_iso;

use super::compose::{
    ComposeError, ComposeMode, derive_form_from_mode, save_compose, save_path_for, validate_form,
};
use super::serialize::slug_from_title;

stylance::import_crate_style!(css, "src/components/mempool/editor.module.css");

#[component]
pub fn MempoolEditor(
    mode: ComposeMode,
    #[prop(into)] on_saved: Callback<VirtualPath>,
    #[prop(into)] on_cancel: Callback<()>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let initial = derive_form_from_mode(&mode, &iso_today());
    let form = RwSignal::new(initial.clone());
    let errors = RwSignal::new(validate_form(&initial));
    let save_error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);

    let on_field_change = move || {
        errors.set(validate_form(&form.get_untracked()));
    };

    let title_input = move |ev| {
        form.update(|f| {
            f.title = event_target_value(&ev);
            if f.slug.is_empty() {
                f.slug = slug_from_title(&f.title);
            }
        });
        on_field_change();
    };

    let slug_input = move |ev| {
        form.update(|f| f.slug = event_target_value(&ev));
        on_field_change();
    };

    let category_input = move |ev| {
        form.update(|f| f.category = event_target_value(&ev));
        on_field_change();
    };

    let status_input = move |ev| {
        form.update(|f| f.status = event_target_value(&ev));
        on_field_change();
    };

    let priority_input = move |ev| {
        let value = event_target_value(&ev);
        form.update(|f| {
            f.priority = if value.is_empty() { None } else { Some(value) };
        });
    };

    let modified_input = move |ev| {
        form.update(|f| f.modified = event_target_value(&ev));
        on_field_change();
    };

    let tags_input = move |ev| {
        let value = event_target_value(&ev);
        form.update(|f| {
            f.tags = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        });
    };

    let body_input = move |ev| {
        form.update(|f| f.body = event_target_value(&ev));
    };

    let mode_for_save = mode.clone();
    let try_save: Callback<()> = Callback::new(move |_| {
        if saving.get_untracked() {
            return;
        }
        let snapshot = form.get_untracked();
        let errs = validate_form(&snapshot);
        if !errs.is_empty() {
            errors.set(errs);
            save_error.set(Some("fix the highlighted fields before saving".into()));
            return;
        }
        // Compute saved path before await so the post-save callback receives
        // a deterministic path even if the form mutates during save.
        let saved_path = save_path_for(&mode_for_save, &snapshot);
        save_error.set(None);
        saving.set(true);
        let ctx_clone = ctx.clone();
        let mode_clone = mode_for_save.clone();
        spawn_local(async move {
            let result = save_compose(ctx_clone, mode_clone, snapshot).await;
            saving.set(false);
            match result {
                Ok(()) => on_saved.run(saved_path),
                Err(message) => save_error.set(Some(message)),
            }
        });
    });

    let on_save_click = move |_| try_save.run(());

    let on_cancel_click = move |_| on_cancel.run(());

    let on_keydown = move |event: leptos::ev::KeyboardEvent| {
        let key = event.key();
        if (event.meta_key() || event.ctrl_key()) && (key == "s" || key == "S") {
            event.prevent_default();
            try_save.run(());
        }
    };

    let mode_label = match &mode {
        ComposeMode::New { .. } => "compose",
        ComposeMode::Edit { .. } => "edit",
    };

    let save_disabled = move || saving.get() || !errors.with(|e| e.is_empty());

    let priority_value = move || form.with(|f| f.priority.clone().unwrap_or_default());
    let tags_value = move || form.with(|f| f.tags.join(", "));

    let has_error = move |kind: ComposeError| errors.with(|e| e.contains(&kind));

    view! {
        <div class=css::editor on:keydown=on_keydown>
            <div class=css::modeTag>{mode_label}</div>
            <div class=css::body>
                {move || save_error.get().map(|message| view! {
                    <div class=css::errorBanner role="alert">{message}</div>
                })}
                <div class=css::row>
                    <label class=css::field>
                        <span class=css::label>"title"</span>
                        <input
                            class=css::input
                            r#type="text"
                            prop:value=move || form.with(|f| f.title.clone())
                            on:input=title_input
                        />
                        {move || has_error(ComposeError::TitleEmpty).then(|| view! {
                            <span class=css::fieldError>"title is required"</span>
                        })}
                        {move || has_error(ComposeError::TitleHasReservedChars).then(|| view! {
                            <span class=css::fieldError>{"title cannot contain \" \\ : or newlines"}</span>
                        })}
                    </label>
                </div>
                <div class=css::row>
                    <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                        <span class=css::label>"category"</span>
                        <select
                            class=css::select
                            prop:value=move || form.with(|f| f.category.clone())
                            on:change=category_input
                        >
                            {LEDGER_CATEGORIES.iter().map(|cat| view! {
                                <option value=*cat>{*cat}</option>
                            }).collect_view()}
                        </select>
                        {move || has_error(ComposeError::CategoryUnknown).then(|| view! {
                            <span class=css::fieldError>"unknown category"</span>
                        })}
                    </label>
                    <label class=css::field>
                        <span class=css::label>"slug"</span>
                        <input
                            class=css::input
                            r#type="text"
                            prop:value=move || form.with(|f| f.slug.clone())
                            on:input=slug_input
                        />
                        {move || has_error(ComposeError::SlugInvalid).then(|| view! {
                            <span class=css::fieldError>"slug must be kebab-case ASCII"</span>
                        })}
                    </label>
                </div>
                <div class=css::row>
                    <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                        <span class=css::label>"status"</span>
                        <select
                            class=css::select
                            prop:value=move || form.with(|f| f.status.clone())
                            on:change=status_input
                        >
                            <option value="draft">"draft"</option>
                            <option value="review">"review"</option>
                        </select>
                        {move || has_error(ComposeError::StatusUnknown).then(|| view! {
                            <span class=css::fieldError>"status must be draft or review"</span>
                        })}
                    </label>
                    <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                        <span class=css::label>"priority"</span>
                        <select
                            class=css::select
                            prop:value=priority_value
                            on:change=priority_input
                        >
                            <option value="">"—"</option>
                            <option value="low">"low"</option>
                            <option value="med">"med"</option>
                            <option value="high">"high"</option>
                        </select>
                        {move || has_error(ComposeError::PriorityUnknown).then(|| view! {
                            <span class=css::fieldError>"priority must be low, med, or high"</span>
                        })}
                    </label>
                    <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                        <span class=css::label>"modified"</span>
                        <input
                            class=css::input
                            r#type="text"
                            placeholder="YYYY-MM-DD"
                            prop:value=move || form.with(|f| f.modified.clone())
                            on:input=modified_input
                        />
                        {move || has_error(ComposeError::ModifiedNotIso).then(|| view! {
                            <span class=css::fieldError>"date must be YYYY-MM-DD"</span>
                        })}
                    </label>
                    <label class=css::field>
                        <span class=css::label>"tags"</span>
                        <input
                            class=css::input
                            r#type="text"
                            placeholder="comma, separated"
                            prop:value=tags_value
                            on:input=tags_input
                        />
                        {move || has_error(ComposeError::TagHasReservedChars).then(|| view! {
                            <span class=css::fieldError>{"tags cannot contain [ ] \" or newlines"}</span>
                        })}
                    </label>
                </div>
                <textarea
                    class=css::bodyArea
                    placeholder="Markdown body…"
                    prop:value=move || form.with(|f| f.body.clone())
                    on:input=body_input
                />
            </div>
            <div class=css::footer>
                <button
                    class=css::cancel
                    type="button"
                    on:click=on_cancel_click
                    prop:disabled=move || saving.get()
                >
                    "Cancel"
                </button>
                <button
                    class=css::save
                    type="button"
                    on:click=on_save_click
                    prop:disabled=save_disabled
                >
                    {move || if saving.get() { "Saving…" } else { "Save" }}
                </button>
            </div>
        </div>
    }
}

/// Today as `YYYY-MM-DD` from the wall clock. Mirrors the helper that lived
/// in `compose.rs` for the deleted `ComposeModal`; kept local so this module
/// is self-contained.
fn iso_today() -> String {
    format_date_iso(current_timestamp() / 1000)
}
