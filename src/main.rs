use leptos::prelude::*;
use wasm_bindgen::JsCast;
use websh::app::App;

fn main() {
    console_error_panic_hook::set_once();

    let root = document()
        .get_element_by_id("app")
        .expect("Failed to find #app element")
        .unchecked_into::<web_sys::HtmlElement>();

    mount_to(root, App).forget();
}
