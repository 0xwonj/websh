//! Image / generic asset view.
//!
//! When `dimensions` are present in the manifest, they are echoed to the
//! `<img>` tag's `width`/`height` attributes. Browsers use these to
//! reserve space before the image bytes finish downloading, eliminating
//! the layout shift that otherwise reflows everything below the figure.
//! `max-width: 100%` plus `height: auto` in CSS keep the actual rendered
//! size responsive — the attributes act purely as an aspect-ratio hint.

use leptos::prelude::*;

use crate::features::reader::css;
use crate::platform::BrowserAssetUrl;
use websh_core::domain::ImageDim;

#[component]
pub fn AssetReaderView(
    url: BrowserAssetUrl,
    alt: String,
    dimensions: Option<ImageDim>,
) -> impl IntoView {
    let url = StoredValue::new_local(url);
    let (width_attr, height_attr) = match dimensions {
        Some(dim) => (Some(dim.width.to_string()), Some(dim.height.to_string())),
        None => (None, None),
    };
    view! {
        <figure class=css::imageFigure>
            <img
                src=move || url.with_value(|url| url.as_str().to_string())
                alt=alt
                class=css::image
                width=width_attr
                height=height_attr
            />
        </figure>
    }
}
