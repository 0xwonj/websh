//! Browser-provided shell execution context.

use wasm_bindgen::JsCast;
use websh_core::runtime::RuntimeStateSnapshot;
use websh_core::shell::{ExecutionContext, SystemInfo};

use crate::config::MS_PER_SECOND;

/// Build the target context supplied to the core shell executor.
pub fn shell_execution_context(runtime_state: &RuntimeStateSnapshot) -> ExecutionContext {
    ExecutionContext {
        system_info: SystemInfo {
            uptime: get_uptime(),
            user_agent: get_user_agent(),
        },
        env: runtime_state.env.clone(),
        access_policy: websh_site::ACCESS_POLICY,
        shell_text: websh_site::SHELL_TEXT,
    }
}

fn get_uptime() -> Option<String> {
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

fn get_user_agent() -> Option<String> {
    web_sys::window()?.navigator().user_agent().ok()
}
