//! Shared breadcrumb navigation component.
//!
//! Used by both Explorer and Reader to display current path with clickable segments.
//! Supports mobile-responsive collapsed mode.

use leptos::prelude::*;
use leptos_icons::Icon;

use crate::components::icons as ic;
use crate::components::terminal::RouteContext;
use crate::core::engine::{RouteRequest, request_path_for_canonical_path};
use crate::models::VirtualPath;

stylance::import_crate_style!(css, "src/components/breadcrumb.module.css");

/// Segment data for breadcrumb rendering.
#[derive(Clone)]
struct BreadcrumbSegment {
    /// Display label
    label: String,
    /// Icon to show
    icon: icondata::Icon,
    /// Target route for navigation (None = current/disabled)
    target: Option<RouteRequest>,
}

/// Shared breadcrumb navigation component.
///
/// Displays the current path as clickable segments for navigation.
/// Automatically handles Root, Browse, and Read routes.
#[component]
pub fn Breadcrumb(
    /// Show root "/" segment
    #[prop(default = false)]
    show_root: bool,
) -> impl IntoView {
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    view! {
        <nav class=css::breadcrumb>
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
                let mut segment_data: Vec<BreadcrumbSegment> = Vec::new();

                // Root segment (optional)
                if show_root {
                    segment_data.push(BreadcrumbSegment {
                        label: "/".to_string(),
                        icon: ic::SERVER,
                        target: Some(RouteRequest::new("/fs")),
                    });
                }

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
                    // Use absolute path construction, not relative join
                    let target = if is_last {
                        None // Current segment is not clickable
                    } else if is_home_segment {
                        Some(RouteRequest::new("/shell"))
                    } else {
                        let path = canonical_segment_path(&segments, idx);
                        Some(RouteRequest::new(request_path_for_canonical_path(&path)))
                    };

                    segment_data.push(BreadcrumbSegment {
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
        let rel = build_segment_path(segments, idx);
        if rel.is_empty() {
            return VirtualPath::from_absolute("/site").expect("constant path");
        }
        return VirtualPath::from_absolute(format!("/site/{rel}")).expect("constant path");
    }

    let abs = format!("/{}", segments[..=idx].join("/"));
    VirtualPath::from_absolute(abs).expect("constant path")
}

/// Clickable breadcrumb segment.
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

/// Current (disabled) breadcrumb segment.
#[component]
fn SegmentCurrent(icon: icondata::Icon, label: String) -> impl IntoView {
    view! {
        <button class=format!("{} {}", css::segment, css::segmentCurrent) disabled=true>
            <span class=css::icon><Icon icon=icon /></span>
            <span class=css::label>{label}</span>
        </button>
    }
}

/// Build the absolute path for a breadcrumb segment click.
///
/// `segments`: full breadcrumb segments from the current route, including
/// any leading "~" mount alias.
/// `idx`: the clicked segment's index into `segments`.
///
/// If segments starts with "~", the home mount alias is skipped when joining.
fn build_segment_path(segments: &[&str], idx: usize) -> String {
    let start_idx = if segments.first() == Some(&"~") { 1 } else { 0 };
    if idx < start_idx {
        return String::new();
    }
    segments[start_idx..=idx].join("/")
}

#[cfg(test)]
mod tests {
    use super::build_segment_path;

    #[test]
    fn test_build_path_simple() {
        let segments = vec!["~", "blog", "posts"];
        assert_eq!(build_segment_path(&segments, 1), "blog");
        assert_eq!(build_segment_path(&segments, 2), "blog/posts");
    }

    #[test]
    fn test_build_path_no_home_prefix() {
        let segments = vec!["work", "notes"];
        assert_eq!(build_segment_path(&segments, 0), "work");
        assert_eq!(build_segment_path(&segments, 1), "work/notes");
    }

    #[test]
    fn test_build_path_home_at_zero_returns_empty() {
        // Clicking the "~" segment itself — the caller handles this via
        // is_home_segment branch, but the builder gracefully returns "".
        let segments = vec!["~", "blog"];
        assert_eq!(build_segment_path(&segments, 0), "");
    }

    #[test]
    fn test_build_path_deep_nesting() {
        let segments = vec!["~", "a", "b", "c", "d"];
        assert_eq!(build_segment_path(&segments, 4), "a/b/c/d");
    }
}
