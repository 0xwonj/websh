//! PDF view — abstract section (when description present) + iframe wrapper.

use leptos::prelude::*;

use crate::components::reader::css;

#[component]
pub fn PdfReaderView(
    title: Signal<String>,
    url: String,
    size_pretty: Option<String>,
    abstract_text: String,
) -> impl IntoView {
    let url_for_open = url.clone();
    let url_for_download = url.clone();

    view! {
        {(!abstract_text.is_empty()).then(|| view! {
            <h2 class=css::sectionTitle data-n="">"Abstract"</h2>
            <p class=css::abstractText>{abstract_text}</p>
        })}

        <h2 class=css::sectionTitle data-n="">"Document"</h2>
        <div class=css::pdfFrame>
            <div class=css::pdfChrome>
                <span class=css::pdfChromeDot></span>
                <span class=css::pdfChromeTitle>
                    {move || title.get()}
                    {size_pretty.clone().map(|s| view! {
                        " · "{s}
                    })}
                </span>
                <a class=css::pdfChromeCtrl href=url_for_download download="">"⤓ pdf"</a>
                <a class=css::pdfChromeCtrl href=url_for_open target="_blank" rel="noopener">"↗ open"</a>
            </div>
            <iframe
                src=url
                class=css::pdfViewer
                title=move || title.get()
            />
        </div>
    }
}
