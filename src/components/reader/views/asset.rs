//! Image / generic asset view.

use leptos::prelude::*;

use crate::components::reader::css;

#[component]
pub fn AssetReaderView(url: String, alt: String) -> impl IntoView {
    view! {
        <figure class=css::imageFigure>
            <img src=url alt=alt class=css::image />
        </figure>
    }
}
