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
use crate::config::configured_mounts;
use crate::core::DirEntry;
use crate::models::{AppRoute, DisplayPermissions, FileType};
use crate::utils::format::{format_date_iso, format_size, join_path};

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

/// Convert mounts to DirEntry list for display.
fn mounts_to_entries() -> Vec<DirEntry> {
    configured_mounts()
        .into_iter()
        .map(|mount| DirEntry {
            name: mount.alias().to_string(),
            is_dir: true,
            title: mount.description(),
            file_meta: None,
        })
        .collect()
}

#[component]
pub fn FileList() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    // Get entries for current path from route
    let entries = Signal::derive(move || {
        let route = route_ctx.0.get();

        // If at Root, show mount list
        if matches!(route, AppRoute::Root) {
            return mounts_to_entries();
        }

        let path = route.fs_path();
        ctx.fs.with(|fs| fs.list_dir(path).unwrap_or_default())
    });

    view! {
        <div class=css::list role="grid" aria-label="File list">
            // Column header (desktop only, hidden on mobile via CSS)
            <div class=css::listHeader role="row">
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

    let selection = ctx.explorer.selection;

    let entry_name = entry.name.clone();
    let is_dir = entry.is_dir;
    let is_encrypted = entry
        .file_meta
        .as_ref()
        .map(|m| m.is_encrypted())
        .unwrap_or(false);
    let is_hidden = entry.name.starts_with('.');
    let icon = get_icon(&entry);
    let size = format_size(entry.file_meta.as_ref().and_then(|m| m.size), false);
    let title = entry.title.clone();
    let modified = entry
        .file_meta
        .as_ref()
        .and_then(|m| m.modified)
        .map(format_date_iso);

    // Build item path once at creation time (route doesn't change during item lifetime)
    let route = route_ctx.0.get_untracked();
    let current_path = route.fs_path();
    let item_fs_path = join_path(current_path, &entry_name);

    // Get permissions from VirtualFs (same as ls -l)
    let perms = ctx.fs.with_untracked(|fs| {
        let wallet = ctx.wallet.get_untracked();
        fs.get_entry(&item_fs_path)
            .map(|e| fs.get_permissions(e, &wallet).to_string())
            .unwrap_or_else(|| {
                // Fallback for mounts at root
                DisplayPermissions {
                    is_dir,
                    read: true,
                    write: false,
                    execute: is_dir,
                }
                .to_string()
            })
    });
    let item_fs_path_for_click = item_fs_path.clone();
    let item_fs_path_for_select = item_fs_path.clone();

    // Check if this entry is selected
    let is_selected = Signal::derive(move || {
        selection
            .get()
            .map(|s| s.path == item_fs_path_for_select)
            .unwrap_or(false)
    });

    // Single click: select the item (this is standard Finder/Explorer behavior)
    let handle_click = move |_: leptos::ev::MouseEvent| {
        ctx.explorer.select(item_fs_path_for_click.clone(), is_dir);
    };

    // Clone entry name for use in dblclick handler
    let entry_name_for_nav = entry.name.clone();

    // Double click: navigate into directory or open file
    let handle_dblclick = move |_: leptos::ev::MouseEvent| {
        ctx.explorer.clear_selection();
        let route = route_ctx.0.get();

        if is_dir {
            // Clear forward stack only when navigating to a new directory (not opening a file)
            ctx.explorer.clear_forward();

            // If at Root, navigate to mount
            if matches!(route, AppRoute::Root)
                && let Some(mount) = configured_mounts()
                    .into_iter()
                    .find(|m| m.alias() == entry_name_for_nav)
            {
                AppRoute::Browse {
                    mount,
                    path: String::new(),
                }
                .push();
                return;
            }

            // Navigate into directory
            let new_route = route.join(&entry_name_for_nav);
            new_route.push();
        } else {
            // Open file in reader - use pre-computed item_fs_path
            let mount = route
                .mount()
                .cloned()
                .unwrap_or_else(crate::config::default_mount);
            AppRoute::Read {
                mount,
                path: item_fs_path.clone(),
            }
            .push();
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

    let aria_label = if is_dir {
        format!("Folder: {}", entry.name)
    } else {
        format!("File: {}", entry.name)
    };

    view! {
        <div
            class=item_class
            on:click=handle_click
            on:dblclick=handle_dblclick
            role="row"
            tabindex="0"
            aria-label=aria_label
            aria-selected=move || is_selected.get()
        >
            // 1. Icon
            <span class=css::icon aria-hidden="true"><Icon icon=icon /></span>

            // 2. Name (with mobile meta inside)
            <div class=css::nameWrapper>
                <span class=name_class>
                    {display_name}
                    {is_encrypted.then(|| view! { <span class=css::lockIcon><Icon icon=ic::LOCK /></span> })}
                </span>
                <div class=css::mobileMeta>
                    {mobile_date.as_ref().map(|d: &String| {
                        view! { <span>{d.clone()}</span> }
                    })}
                    <span>{mobile_size}</span>
                    <span>{mobile_perms}</span>
                </div>
            </div>

            // 3. Title/Description (always render for grid alignment)
            <span class=css::itemDesc>{title}</span>

            // 4. Date (always render for grid alignment)
            <span class=css::itemDate>{date_display}</span>

            // 5. Size
            <span class=css::size>{size}</span>

            // 6. Perms
            <span class=css::perms>{perms}</span>

            // 7. Chevron (always render for grid alignment)
            <span class=css::chevron aria-hidden="true">
                {is_dir.then(|| view! { <Icon icon=ic::CHEVRON_RIGHT /> })}
            </span>
        </div>
    }
}
