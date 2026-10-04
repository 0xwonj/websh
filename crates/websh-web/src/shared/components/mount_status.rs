//! One refresh notice shared by directory, reader, and ledger surfaces.
use crate::app::{AppContext, RuntimeServices};
use crate::runtime::mounts::{MountLoadStatus, RefreshState, SnapshotOrigin};
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use websh_core::domain::VirtualPath;

stylance::import_crate_style!(css, "src/shared/components/mount_status.module.css");

#[component]
pub fn MountStatusNotice(path: Signal<VirtualPath>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    view! { {move || {
        let entry = ctx.mounts.with(|mounts| mounts.owner(&path.get()).cloned());
        let entry = entry?;
        let (message, busy) = match entry.status {
            MountLoadStatus::Loading => (format!("Loading {} listing…", entry.declared.label), true),
            MountLoadStatus::Failed { error } => (format!("{} listing unavailable: {error}", entry.declared.label), false),
            MountLoadStatus::Available { origin, observed_at_ms, refresh, .. } => {
                let cached = if origin == SnapshotOrigin::Cache {
                    format!("Saved listing from {}. ", age_label(observed_at_ms))
                } else { String::new() };
                match refresh {
                    RefreshState::Running => (format!("{cached}Checking for updates…"), true),
                    RefreshState::Failed(error) => (format!("{cached}Refresh failed; showing the available listing. {error}"), false),
                    RefreshState::Idle if origin == SnapshotOrigin::Cache => (cached, false),
                    RefreshState::Idle => return None,
                }
            }
        };
        let root = entry.declared.root;
        Some(view! {
            <aside class=css::notice role="status" aria-live="polite" data-mount-status="true">
                <span>{message}</span>
                <button type="button" class=css::retry disabled=busy on:click=move |_| {
                    let root = root.clone();
                    spawn_local(async move { let _ = RuntimeServices::new(ctx).reload_runtime_mount(root).await; });
                }>"Refresh listing"</button>
            </aside>
        })
    }} }
}

fn age_label(observed_at_ms: u64) -> String {
    let minutes =
        crate::platform::time::current_timestamp().saturating_sub(observed_at_ms) / 60_000;
    match minutes {
        0 => "just now".into(),
        1..60 => format!("{minutes} min ago"),
        60..1440 => format!("{} hr ago", minutes / 60),
        _ => format!("{} days ago", minutes / 1440),
    }
}
