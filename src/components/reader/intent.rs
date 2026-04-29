//! Reader-bound types and conversions.
//!
//! `ReaderIntent` is the narrow subset of `RenderIntent` that `Reader` can
//! render — surface variants (`DirectoryListing`, `TerminalApp`) are
//! syntactically rejected. `ReaderFrame` mirrors `RouteFrame` with the
//! narrower intent.
//!
//! The `From` / `TryFrom` impls bridge the `RouteFrame` ↔ `ReaderFrame`
//! boundary at the router (router → ReaderFrame) and the SiteChrome adapter
//! (ReaderFrame → RouteFrame).

use crate::core::engine::{RenderIntent, RouteFrame, RouteRequest, RouteResolution};
use crate::models::VirtualPath;

/// Reader-bound subset of [`RenderIntent`]. Constructed by the router; carries
/// only the variants `Reader` can render.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderIntent {
    Html { node_path: VirtualPath },
    Markdown { node_path: VirtualPath },
    Plain { node_path: VirtualPath },
    Asset {
        node_path: VirtualPath,
        media_type: String,
    },
    Redirect { node_path: VirtualPath },
}

/// Reader's narrowed equivalent of [`RouteFrame`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderFrame {
    pub request: RouteRequest,
    pub resolution: RouteResolution,
    pub intent: ReaderIntent,
}

impl From<ReaderIntent> for RenderIntent {
    fn from(intent: ReaderIntent) -> Self {
        match intent {
            ReaderIntent::Html { node_path } => RenderIntent::HtmlContent { node_path },
            ReaderIntent::Markdown { node_path } => RenderIntent::MarkdownContent { node_path },
            ReaderIntent::Plain { node_path } => RenderIntent::PlainContent { node_path },
            ReaderIntent::Asset {
                node_path,
                media_type,
            } => RenderIntent::Asset {
                node_path,
                media_type,
            },
            ReaderIntent::Redirect { node_path } => RenderIntent::Redirect { node_path },
        }
    }
}

impl From<ReaderFrame> for RouteFrame {
    fn from(frame: ReaderFrame) -> Self {
        RouteFrame {
            request: frame.request,
            resolution: frame.resolution,
            intent: frame.intent.into(),
        }
    }
}

impl TryFrom<RouteFrame> for ReaderFrame {
    /// On failure the original frame is returned so the caller can reroute it.
    type Error = RouteFrame;

    fn try_from(frame: RouteFrame) -> Result<Self, Self::Error> {
        let intent = match frame.intent {
            RenderIntent::HtmlContent { ref node_path } => ReaderIntent::Html {
                node_path: node_path.clone(),
            },
            RenderIntent::MarkdownContent { ref node_path } => ReaderIntent::Markdown {
                node_path: node_path.clone(),
            },
            RenderIntent::PlainContent { ref node_path } => ReaderIntent::Plain {
                node_path: node_path.clone(),
            },
            RenderIntent::Asset {
                ref node_path,
                ref media_type,
            } => ReaderIntent::Asset {
                node_path: node_path.clone(),
                media_type: media_type.clone(),
            },
            RenderIntent::Redirect { ref node_path } => ReaderIntent::Redirect {
                node_path: node_path.clone(),
            },
            RenderIntent::DirectoryListing { .. } | RenderIntent::TerminalApp { .. } => {
                return Err(frame);
            }
        };
        Ok(ReaderFrame {
            request: frame.request,
            resolution: frame.resolution,
            intent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_intent_round_trip_html() {
        let intent = ReaderIntent::Html {
            node_path: VirtualPath::from_absolute("/index.html").unwrap(),
        };
        match intent {
            ReaderIntent::Html { node_path } => assert_eq!(node_path.as_str(), "/index.html"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn reader_intent_round_trip_asset() {
        let intent = ReaderIntent::Asset {
            node_path: VirtualPath::from_absolute("/cover.png").unwrap(),
            media_type: "image/png".to_string(),
        };
        if let ReaderIntent::Asset { media_type, .. } = intent {
            assert_eq!(media_type, "image/png");
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn reader_intent_round_trip_redirect() {
        let intent = ReaderIntent::Redirect {
            node_path: VirtualPath::from_absolute("/x.link").unwrap(),
        };
        if let ReaderIntent::Redirect { node_path } = intent {
            assert_eq!(node_path.as_str(), "/x.link");
        } else {
            panic!("unexpected variant");
        }
    }

    fn make_reader_frame(intent: ReaderIntent, request_path: &str) -> ReaderFrame {
        ReaderFrame {
            request: RouteRequest::new(request_path),
            resolution: RouteResolution {
                request_path: request_path.to_string(),
                surface: crate::core::engine::RouteSurface::Content,
                node_path: VirtualPath::from_absolute(request_path).unwrap(),
                kind: crate::core::engine::ResolvedKind::Document,
                params: std::collections::BTreeMap::new(),
            },
            intent,
        }
    }

    fn round_trip(intent: ReaderIntent, request_path: &str) {
        let frame = make_reader_frame(intent.clone(), request_path);
        let route_frame = RouteFrame::from(frame.clone());
        let reconverted =
            ReaderFrame::try_from(route_frame).expect("reader-bound intent round trips");
        assert_eq!(reconverted.intent, intent);
        assert_eq!(reconverted.request, frame.request);
        assert_eq!(reconverted.resolution, frame.resolution);
    }

    #[test]
    fn reader_frame_round_trips_markdown() {
        round_trip(
            ReaderIntent::Markdown {
                node_path: VirtualPath::from_absolute("/blog/hello.md").unwrap(),
            },
            "/blog/hello.md",
        );
    }

    #[test]
    fn reader_frame_round_trips_html() {
        round_trip(
            ReaderIntent::Html {
                node_path: VirtualPath::from_absolute("/index.html").unwrap(),
            },
            "/index.html",
        );
    }

    #[test]
    fn reader_frame_round_trips_plain() {
        round_trip(
            ReaderIntent::Plain {
                node_path: VirtualPath::from_absolute("/notes/x.txt").unwrap(),
            },
            "/notes/x.txt",
        );
    }

    #[test]
    fn reader_frame_round_trips_asset() {
        round_trip(
            ReaderIntent::Asset {
                node_path: VirtualPath::from_absolute("/cover.png").unwrap(),
                media_type: "image/png".to_string(),
            },
            "/cover.png",
        );
    }

    #[test]
    fn reader_frame_round_trips_redirect() {
        round_trip(
            ReaderIntent::Redirect {
                node_path: VirtualPath::from_absolute("/x.link").unwrap(),
            },
            "/x.link",
        );
    }

    #[test]
    fn reader_intent_to_render_intent_preserves_fields() {
        let asset = ReaderIntent::Asset {
            node_path: VirtualPath::from_absolute("/cover.png").unwrap(),
            media_type: "image/png".to_string(),
        };
        let render: RenderIntent = asset.into();
        match render {
            RenderIntent::Asset {
                node_path,
                media_type,
            } => {
                assert_eq!(node_path.as_str(), "/cover.png");
                assert_eq!(media_type, "image/png");
            }
            other => panic!("expected Asset, got {other:?}"),
        }

        let html = ReaderIntent::Html {
            node_path: VirtualPath::from_absolute("/index.html").unwrap(),
        };
        let render: RenderIntent = html.into();
        match render {
            RenderIntent::HtmlContent { node_path } => {
                assert_eq!(node_path.as_str(), "/index.html");
            }
            other => panic!("expected HtmlContent, got {other:?}"),
        }
    }

    #[test]
    fn try_from_route_frame_rejects_directory_listing() {
        let frame = RouteFrame {
            request: RouteRequest::new("/blog"),
            resolution: RouteResolution {
                request_path: "/blog".to_string(),
                surface: crate::core::engine::RouteSurface::Content,
                node_path: VirtualPath::from_absolute("/blog").unwrap(),
                kind: crate::core::engine::ResolvedKind::Directory,
                params: std::collections::BTreeMap::new(),
            },
            intent: RenderIntent::DirectoryListing {
                node_path: VirtualPath::from_absolute("/blog").unwrap(),
            },
        };
        assert!(ReaderFrame::try_from(frame).is_err());
    }
}
