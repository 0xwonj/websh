//! Wraps a wasm-bindgen `Closure` so it can travel through Leptos's
//! `Send + Sync + 'static` cleanup bound. The bound exists because
//! reactive_graph is platform-generic; on wasm32 there are no threads,
//! so the closure never crosses threads in practice.
//!
//! The accessor method (rather than direct field access) is required so
//! closure disjoint-capture (RFC 2229) sees the whole wrapper as the
//! captured value, inheriting the unsafe `Send + Sync` impl below.

#[cfg(target_arch = "wasm32")]
pub struct WasmCleanup<F: ?Sized>(pub wasm_bindgen::closure::Closure<F>);

#[cfg(target_arch = "wasm32")]
impl<F: ?Sized> WasmCleanup<F> {
    pub fn js_function(&self) -> &js_sys::Function {
        use wasm_bindgen::JsCast;
        self.0.as_ref().unchecked_ref()
    }
}

// SAFETY: wasm32 has no threads. The cleanup runs on the same JS thread
// that installed it; the wrapper is never genuinely sent or shared.
#[cfg(target_arch = "wasm32")]
unsafe impl<F: ?Sized> Send for WasmCleanup<F> {}
#[cfg(target_arch = "wasm32")]
unsafe impl<F: ?Sized> Sync for WasmCleanup<F> {}
