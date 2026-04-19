//! Sync panel component.
//!
//! Dropdown panel showing pending/staged changes with stage/commit actions.

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::core::storage::{ChangeType, PendingChange};

stylance::import_crate_style!(css, "src/components/status/sync_panel.module.css");

/// Sync panel showing changes with actions.
#[component]
pub fn SyncPanel(
    #[prop(into)] is_open: Signal<bool>,
    on_close: Callback<()>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext");

    // Get changes with paths
    let pending_changes = Signal::derive(move || {
        ctx.fs.pending().with(|p| {
            p.iter()
                .map(|change| (change.path.clone(), change.clone()))
                .collect::<Vec<_>>()
        })
    });

    let staged_paths = Signal::derive(move || {
        ctx.fs.staged().with(|s| {
            s.paths().map(String::from).collect::<std::collections::HashSet<_>>()
        })
    });

    // Actions
    let handle_stage = move |path: String| {
        ctx.fs.staged().update(|s| s.add(path));
    };

    let handle_unstage = move |path: String| {
        ctx.fs.staged().update(|s| s.remove(&path));
    };

    let handle_stage_all = move |_| {
        let paths: Vec<String> = ctx.fs.pending().with(|p| {
            p.paths().map(String::from).collect()
        });
        ctx.fs.staged().update(|s| s.add_all(paths));
    };

    let handle_unstage_all = move |_| {
        ctx.fs.staged().update(|s| s.clear());
    };

    view! {
        <Show when=move || is_open.get()>
            <div class=css::overlay on:click=move |_| on_close.run(())></div>
            <div class=css::panel>
                <div class=css::header>
                    <div class=css::title>
                        <Icon icon=ic::SYNC />
                        <span>"Changes"</span>
                    </div>
                    <button class=css::closeButton on:click=move |_| on_close.run(()) title="Close">
                        <Icon icon=ic::CLOSE />
                    </button>
                </div>

                <div class=css::body>
                    // Staged section
                    <div class=css::section>
                        <div class=css::sectionHeader>
                            <span class=css::sectionTitle>"Staged Changes"</span>
                            <button class=css::actionButton on:click=handle_unstage_all>
                                "Unstage All"
                            </button>
                        </div>
                        <div class=css::fileList>
                            <For
                                each=move || {
                                    pending_changes.get().into_iter()
                                        .filter(move |(path, _)| staged_paths.get().contains(path))
                                        .collect::<Vec<_>>()
                                }
                                key=|(path, _): &(String, PendingChange)| path.to_string()
                                children=move |(path, change): (String, PendingChange)| {
                                    let path_clone = path.clone();
                                    view! {
                                        <ChangeItem
                                            path=path.clone()
                                            change=change
                                            is_staged=true
                                            on_toggle=Callback::new(move |_| handle_unstage(path_clone.clone()))
                                        />
                                    }
                                }
                            />
                        </div>
                    </div>

                    // Unstaged section
                    <div class=css::section>
                        <div class=css::sectionHeader>
                            <span class=css::sectionTitle>"Unstaged Changes"</span>
                            <button class=css::actionButton on:click=handle_stage_all>
                                "Stage All"
                            </button>
                        </div>
                        <div class=css::fileList>
                            <For
                                each=move || {
                                    pending_changes.get().into_iter()
                                        .filter(move |(path, _)| !staged_paths.get().contains(path))
                                        .collect::<Vec<_>>()
                                }
                                key=|(path, _): &(String, PendingChange)| path.to_string()
                                children=move |(path, change): (String, PendingChange)| {
                                    let path_clone = path.clone();
                                    view! {
                                        <ChangeItem
                                            path=path.clone()
                                            change=change
                                            is_staged=false
                                            on_toggle=Callback::new(move |_| handle_stage(path_clone.clone()))
                                        />
                                    }
                                }
                            />
                        </div>
                    </div>
                </div>

                // Footer with commit button
                <div class=css::footer>
                    <button class=css::commitButton disabled=move || staged_paths.get().is_empty()>
                        <Icon icon=ic::CLOUD />
                        <span>"Commit Changes"</span>
                    </button>
                </div>
            </div>
        </Show>
    }
}

/// Individual change item.
#[component]
fn ChangeItem(
    path: String,
    change: PendingChange,
    is_staged: bool,
    on_toggle: Callback<()>,
) -> impl IntoView {
    let change_type = &change.change_type;
    let (icon, label, color_class) = match change_type {
        ChangeType::CreateFile { .. } => (ic::PLUS, "A", css::added),
        ChangeType::UpdateFile { .. } => (ic::EDIT, "M", css::modified),
        ChangeType::DeleteFile => (ic::CLOSE, "D", css::deleted),
        ChangeType::CreateBinaryFile { .. } => (ic::FILE_IMAGE, "A", css::added),
        ChangeType::CreateDirectory { .. } => (ic::FOLDER, "A", css::added),
        ChangeType::DeleteDirectory => (ic::FOLDER, "D", css::deleted),
    };

    view! {
        <div class=css::fileItem>
            <button
                class=if is_staged { css::checkbox } else { css::checkboxUnchecked }
                on:click=move |_| on_toggle.run(())
                title=if is_staged { "Unstage" } else { "Stage" }
            >
                <Show when=move || is_staged>
                    <Icon icon=ic::STAGED />
                </Show>
            </button>
            <span class=format!("{} {}", css::changeType, color_class)>{label}</span>
            <Icon icon=icon />
            <span class=css::filePath>{path}</span>
        </div>
    }
}
