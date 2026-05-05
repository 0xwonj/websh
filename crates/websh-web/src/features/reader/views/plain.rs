//! Plain-text view — `<pre>` wrapper.

use leptos::prelude::*;

use crate::features::reader::css;

#[component]
pub fn PlainReaderView(text: String) -> impl IntoView {
    view! {
        <pre class=css::rawText>{text}</pre>
    }
}
