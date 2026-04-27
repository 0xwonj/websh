//! Modal preview for a mempool entry. Reuses the Reader component for body
//! rendering; the modal frame itself is local to the mempool module.

use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::reader::{Reader, ReaderMode};
use crate::core::engine::{
    RenderIntent, ResolvedKind, RouteFrame, RouteRequest, RouteResolution, RouteSurface,
    build_render_intent, resolve_route,
};
use crate::models::VirtualPath;

stylance::import_crate_style!(preview_css, "src/components/mempool/preview.module.css");

#[component]
pub fn MempoolPreviewModal(
    open_path: ReadSignal<Option<VirtualPath>>,
    set_open_path: WriteSignal<Option<VirtualPath>>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let frame = Memo::new(move |_| {
        open_path
            .get()
            .map(|path| ctx.view_global_fs.with(|fs| build_frame(fs, path)))
    });

    view! {
        <Show when=move || frame.with(|f| f.is_some())>
            {move || {
                let route = Memo::new(move |_| {
                    frame.get().expect("frame is Some inside Show")
                });
                let on_close = Callback::new(move |_| set_open_path.set(None));
                view! {
                    <div
                        class=preview_css::backdrop
                        on:click=move |_| set_open_path.set(None)
                    >
                        <div
                            class=preview_css::panel
                            on:click=|event: leptos::ev::MouseEvent| event.stop_propagation()
                        >
                            <button
                                class=preview_css::close
                                type="button"
                                aria-label="Close preview"
                                on:click=move |_| set_open_path.set(None)
                            >
                                "\u{00d7}"
                            </button>
                            <Reader route=route on_close=on_close mode=ReaderMode::Preview />
                        </div>
                    </div>
                }
            }}
        </Show>
    }
}

fn build_frame(fs: &crate::core::engine::GlobalFs, path: VirtualPath) -> RouteFrame {
    let request = RouteRequest::new(path.as_str());
    let resolved = resolve_route(fs, &request).and_then(|resolution| {
        build_render_intent(fs, &resolution).map(|intent| (resolution, intent))
    });
    if let Some((resolution, intent)) = resolved {
        RouteFrame {
            request,
            resolution,
            intent,
        }
    } else {
        let resolution = RouteResolution {
            request_path: request.url_path.clone(),
            surface: RouteSurface::Content,
            node_path: path.clone(),
            kind: ResolvedKind::Document,
            params: BTreeMap::new(),
        };
        let intent = RenderIntent::DocumentReader {
            node_path: path.clone(),
        };
        RouteFrame {
            request,
            resolution,
            intent,
        }
    }
}
