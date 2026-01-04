mod app;
mod components;
mod config;
mod core;
mod models;
mod utils;

use app::App;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

fn main() {
    console_error_panic_hook::set_once();

    let root = document()
        .get_element_by_id("app")
        .expect("Failed to find #app element")
        .unchecked_into::<web_sys::HtmlElement>();

    mount_to(root, App).forget();
}
