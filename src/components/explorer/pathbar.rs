//! Path bar component (macOS Finder style).
//!
//! Displays full path at the bottom of the explorer with clickable segments.

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::core::engine::{RouteRequest, request_path_for_canonical_path};
use crate::models::VirtualPath;

stylance::import_crate_style!(css, "src/components/explorer/pathbar.module.css");

/// Segment data for path bar rendering.
#[derive(Clone)]
struct PathSegment {
    /// Display label
    label: String,
    /// Icon to show
    icon: icondata::Icon,
    /// Target route for navigation (None = current/disabled)
    target: Option<RouteRequest>,
}

/// Path bar component displayed at the bottom of the explorer.
///
/// Shows the full path with clickable segments for navigation.
#[component]
pub fn PathBar() -> impl IntoView {
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    view! {
        <nav class=css::pathbar>
            {move || {
                let route = route_ctx.0.get();
                let display = route.display_path();

                // Handle Root specially
                if route.is_root() {
                    return view! {
                        <SegmentCurrent icon=ic::SERVER label="/".to_string() />
                    }.into_any();
                }

                let segments: Vec<&str> = display.split('/').filter(|s| !s.is_empty()).collect();

                // Build segment data
                let mut segment_data: Vec<PathSegment> = Vec::new();

                // Root segment (always shown)
                segment_data.push(PathSegment {
                    label: "/".to_string(),
                    icon: ic::SERVER,
                    target: Some(RouteRequest::new("/fs")),
                });

                // Path segments
                for (idx, segment) in segments.iter().enumerate() {
                    let is_last = idx == segments.len() - 1;
                    let is_home_segment = *segment == "~";

                    // Determine icon
                    let icon = if is_home_segment {
                        ic::HOME
                    } else if is_last && route.is_file() {
                        ic::FILE
                    } else {
                        ic::FOLDER
                    };

                    // Build target route for navigation
                    let target = if is_last {
                        None // Current segment is not clickable
                    } else if is_home_segment {
                        Some(RouteRequest::new("/shell"))
                    } else if idx == 0 {
                        Some(RouteRequest::new(request_path_for_canonical_path(
                            &canonical_segment_path(&segments, idx),
                        )))
                    } else {
                        let start_idx = if segments.first() == Some(&"~") { 1 } else { 0 };
                        if idx >= start_idx {
                            Some(RouteRequest::new(request_path_for_canonical_path(
                                &canonical_segment_path(&segments, idx),
                            )))
                        } else {
                            Some(RouteRequest::new("/shell"))
                        }
                    };

                    segment_data.push(PathSegment {
                        label: segment.to_string(),
                        icon,
                        target,
                    });
                }

                // Render segments
                let views: Vec<_> = segment_data
                    .into_iter()
                    .enumerate()
                    .map(|(idx, seg)| {
                        let show_separator = idx > 0;

                        view! {
                            <>
                                {show_separator.then(|| view! {
                                    <span class=css::separator>
                                        <Icon icon=ic::CHEVRON_RIGHT />
                                    </span>
                                })}
                                {if seg.target.is_some() {
                                    let target = seg.target.clone().unwrap();
                                    view! {
                                        <SegmentLink
                                            icon=seg.icon
                                            label=seg.label.clone()
                                            on_click=move || target.clone().push()
                                        />
                                    }.into_any()
                                } else {
                                    view! {
                                        <SegmentCurrent icon=seg.icon label=seg.label.clone() />
                                    }.into_any()
                                }}
                            </>
                        }
                    })
                    .collect();

                views.collect_view().into_any()
            }}
        </nav>
    }
}

fn canonical_segment_path(segments: &[&str], idx: usize) -> VirtualPath {
    if segments.first() == Some(&"~") {
        if idx == 0 {
            return VirtualPath::from_absolute("/site").expect("constant path");
        }
        let rel = segments[1..=idx].join("/");
        return VirtualPath::from_absolute(format!("/site/{rel}")).expect("constant path");
    }

    VirtualPath::from_absolute(format!("/{}", segments[..=idx].join("/"))).expect("constant path")
}

/// Clickable path segment.
#[component]
fn SegmentLink<F>(icon: icondata::Icon, label: String, on_click: F) -> impl IntoView
where
    F: Fn() + 'static,
{
    view! {
        <button
            class=css::segment
            on:click=move |_| on_click()
        >
            <span class=css::icon><Icon icon=icon /></span>
            <span class=css::label>{label}</span>
        </button>
    }
}

/// Current (disabled) path segment.
#[component]
fn SegmentCurrent(icon: icondata::Icon, label: String) -> impl IntoView {
    view! {
        <button class=format!("{} {}", css::segment, css::segmentCurrent) disabled=true>
            <span class=css::icon><Icon icon=icon /></span>
            <span class=css::label>{label}</span>
        </button>
    }
}
