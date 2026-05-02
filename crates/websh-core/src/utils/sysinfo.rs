//! System information utilities.

use crate::config::MS_PER_SECOND;
use wasm_bindgen::JsCast;

/// Get session uptime in human-readable format.
pub fn get_uptime() -> Option<String> {
    let window = web_sys::window()?;
    let performance = js_sys::Reflect::get(&window, &"performance".into()).ok()?;
    let now_fn = js_sys::Reflect::get(&performance, &"now".into()).ok()?;
    let func = now_fn.dyn_ref::<js_sys::Function>()?;
    let result = func.call0(&performance).ok()?;
    let uptime_ms = result.as_f64()?;

    let uptime_secs = (uptime_ms / MS_PER_SECOND) as u64;
    let mins = uptime_secs / 60;
    let secs = uptime_secs % 60;

    if mins > 0 {
        Some(format!("{}m {}s", mins, secs))
    } else {
        Some(format!("{}s", secs))
    }
}
