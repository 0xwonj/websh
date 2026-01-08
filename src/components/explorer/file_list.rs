//! File list component for explorer view.
//!
//! Displays files and directories in list format.
//! Grid view is intentionally deferred to avoid complexity.

#![allow(dead_code)]

use icondata::Icon as IconData;
use leptos::prelude::*;
use leptos_icons::Icon;

use crate::app::AppContext;
use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::core::DirEntry;
use crate::models::{AppRoute, FileType};
use crate::utils::format::{format_date_iso, format_size};

stylance::import_crate_style!(css, "src/components/explorer/file_list.module.css");

/// Get icon for file/directory based on type
fn get_icon(entry: &DirEntry) -> IconData {
    if entry.is_dir {
        ic::FOLDER
    } else {
        match FileType::from_path(&entry.name) {
            FileType::Markdown => ic::FILE_TEXT,
            FileType::Pdf => ic::FILE_PDF,
            FileType::Image => ic::FILE_IMAGE,
            FileType::Link => ic::FILE_LINK,
            FileType::Unknown => ic::FILE,
        }
    }
}

/// Get display permissions string
fn get_permissions(entry: &DirEntry) -> String {
    let prefix = if entry.is_dir { "d" } else { "-" };
    let read = "r";
    let write = "-";
    let exec = if entry.is_dir { "x" } else { "-" };
    format!("{}{}{}{}", prefix, read, write, exec)
}

#[component]
pub fn FileList() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    // Get entries for current path from route
    let entries = Signal::derive(move || {
        let route = route_ctx.0.get();
        let path = route.fs_path();
        ctx.fs.with(|fs| fs.list_dir(path).unwrap_or_default())
    });

    view! {
        <div class=css::list>
            // Column header (desktop only, hidden on mobile via CSS)
            <div class=css::listHeader>
                <span class=css::headerIcon></span>
                <span class=css::headerName>"Name"</span>
                <span class=css::headerDesc>"Description"</span>
                <span class=css::headerDate>"Modified"</span>
                <span class=css::headerSize>"Size"</span>
                <span class=css::headerPerms>"Permissions"</span>
                <span class=css::headerChevron></span>
            </div>
            <For
                each=move || entries.get()
                key=|entry| entry.name.clone()
                children=move |entry| {
                    view! { <FileListItem entry=entry /> }
                }
            />
        </div>
    }
}

#[component]
fn FileListItem(entry: DirEntry) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    let selected_file = ctx.explorer.selected_file;

    let entry_name = entry.name.clone();
    let entry_name_for_click = entry.name.clone();
    let is_dir = entry.is_dir;
    let is_encrypted = entry.meta.is_encrypted();
    let is_hidden = entry.name.starts_with('.');
    let icon = get_icon(&entry);
    let perms = get_permissions(&entry);
    let size = format_size(entry.meta.size, false);
    let description = entry.description.clone();
    let modified = entry.meta.modified.map(format_date_iso);

    // Build the file path for selection tracking (relative fs path)
    let file_fs_path = Signal::derive(move || {
        let route = route_ctx.0.get();
        let current_path = route.fs_path();
        if current_path.is_empty() {
            entry_name.clone()
        } else {
            format!("{}/{}", current_path, &entry_name)
        }
    });

    // Check if this entry is selected
    let is_selected =
        Signal::derive(move || selected_file.get().as_ref() == Some(&file_fs_path.get()));

    let handle_click = move |_: leptos::ev::MouseEvent| {
        let route = route_ctx.0.get();

        if is_dir {
            // Navigate into directory
            let new_route = route.join(&entry_name_for_click);
            new_route.push();
        } else {
            // Build file path (relative)
            let current_path = route.fs_path();
            let file_path = if current_path.is_empty() {
                entry_name_for_click.clone()
            } else {
                format!("{}/{}", current_path, &entry_name_for_click)
            };

            // If already selected, open in reader; otherwise select for preview
            if ctx.explorer.selected_file.get().as_ref() == Some(&file_path) {
                // Already selected - open in reader (navigate to read route)
                let mount = route.mount().cloned().unwrap_or_else(|| {
                    crate::config::configured_mounts()
                        .into_iter()
                        .next()
                        .unwrap()
                });
                let read_route = AppRoute::Read {
                    mount,
                    path: file_path,
                };
                read_route.push();
            } else {
                // Not selected - select for preview
                ctx.explorer.select_file(file_path);
            }
        }
    };

    let name_class = if is_dir {
        format!("{} {} {}", css::name, css::nameDir, css::nameBold)
    } else if is_hidden {
        format!("{} {}", css::name, css::nameHidden)
    } else {
        format!("{} {}", css::name, css::nameFile)
    };

    let suffix = if is_dir { "/" } else { "" };
    let display_name = format!("{}{}", entry.name, suffix);

    // Clone values for mobile meta section
    let mobile_date = modified.clone();
    let mobile_size = size.clone();
    let mobile_perms = perms.clone();

    let item_class = move || {
        if is_selected.get() {
            format!("{} {}", css::listItem, css::selected)
        } else {
            css::listItem.to_string()
        }
    };

    // For grid alignment, we need exactly 7 cells always
    let date_display = modified.clone().unwrap_or_default();

    view! {
        <div class=item_class on:click=handle_click>
            // 1. Icon
            <span class=css::icon><Icon icon=icon /></span>

            // 2. Name (with mobile meta inside)
            <div class=css::nameWrapper>
                <span class=name_class>
                    {display_name}
                    {is_encrypted.then(|| view! { <span class=css::lockIcon><Icon icon=ic::LOCK /></span> })}
                </span>
                <div class=css::mobileMeta>
                    {mobile_date.as_ref().map(|d| {
                        view! { <span>{d.clone()}</span> }
                    })}
                    <span>{mobile_size}</span>
                    <span>{mobile_perms}</span>
                </div>
            </div>

            // 3. Description (always render for grid alignment)
            <span class=css::itemDesc>{description}</span>

            // 4. Date (always render for grid alignment)
            <span class=css::itemDate>{date_display}</span>

            // 5. Size
            <span class=css::size>{size}</span>

            // 6. Perms
            <span class=css::perms>{perms}</span>

            // 7. Chevron (always render for grid alignment)
            <span class=css::chevron>
                {is_dir.then(|| view! { <Icon icon=ic::CHEVRON_RIGHT /> })}
            </span>
        </div>
    }
}
