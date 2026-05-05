//! Local UI icons.
//!
//! These paths are vendored from Bootstrap Icons.
//! Source: https://icons.getbootstrap.com/
//! License: MIT, Copyright (c) 2019-2024 The Bootstrap Authors.

use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiIcon {
    ChevronRight,
    File,
    Folder,
    Home,
    Lock,
    Server,
}

pub const CHEVRON_RIGHT: UiIcon = UiIcon::ChevronRight;
pub const FILE: UiIcon = UiIcon::File;
pub const FOLDER: UiIcon = UiIcon::Folder;
pub const HOME: UiIcon = UiIcon::Home;
pub const LOCK: UiIcon = UiIcon::Lock;
pub const SERVER: UiIcon = UiIcon::Server;

#[component]
pub fn SvgIcon(icon: UiIcon) -> impl IntoView {
    view! {
        <svg
            aria-hidden="true"
            focusable="false"
            width="1em"
            height="1em"
            viewBox="0 0 16 16"
            fill="currentColor"
            style="display:block"
        >
            {icon_paths(icon)}
        </svg>
    }
}

fn icon_paths(icon: UiIcon) -> AnyView {
    match icon {
        UiIcon::ChevronRight => view! {
            <path fill-rule="evenodd" d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708" />
        }.into_any(),
        UiIcon::File => view! {
            <path d="M14 4.5V14a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V2a2 2 0 0 1 2-2h5.5zm-3 0A1.5 1.5 0 0 1 9.5 3V1H4a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V4.5z" />
        }.into_any(),
        UiIcon::Folder => view! {
            <path d="M9.828 3h3.982a2 2 0 0 1 1.992 2.181l-.637 7A2 2 0 0 1 13.174 14H2.825a2 2 0 0 1-1.991-1.819l-.637-7a2 2 0 0 1 .342-1.31L.5 3a2 2 0 0 1 2-2h3.672a2 2 0 0 1 1.414.586l.828.828A2 2 0 0 0 9.828 3m-8.322.12q.322-.119.684-.12h5.396l-.707-.707A1 1 0 0 0 6.172 2H2.5a1 1 0 0 0-1 .981z" />
        }.into_any(),
        UiIcon::Home => view! {
            <>
                <path d="M8.707 1.5a1 1 0 0 0-1.414 0L.646 8.146a.5.5 0 0 0 .708.708L8 2.207l6.646 6.647a.5.5 0 0 0 .708-.708L13 5.793V2.5a.5.5 0 0 0-.5-.5h-1a.5.5 0 0 0-.5.5v1.293z" />
                <path d="m8 3.293 6 6V13.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 13.5V9.293z" />
            </>
        }.into_any(),
        UiIcon::Lock => view! {
            <path fill-rule="evenodd" d="M8 0a4 4 0 0 1 4 4v2.05a2.5 2.5 0 0 1 2 2.45v5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 13.5v-5a2.5 2.5 0 0 1 2-2.45V4a4 4 0 0 1 4-4m0 1a3 3 0 0 0-3 3v2h6V4a3 3 0 0 0-3-3" />
        }.into_any(),
        UiIcon::Server => view! {
            <path d="M0 10a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v1a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2zm2.5 1a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1m2 0a.5.5 0 1 0 0-1 .5.5 0 0 0 0 1M.91 7.204A3 3 0 0 1 2 7h12c.384 0 .752.072 1.09.204l-1.867-3.422A1.5 1.5 0 0 0 11.906 3H4.094a1.5 1.5 0 0 0-1.317.782z" />
        }.into_any(),
    }
}
