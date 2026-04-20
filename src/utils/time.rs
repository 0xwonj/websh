//! Wall-clock current time as Unix milliseconds.
//!
//! On `wasm32`, uses `js_sys::Date::now()` (browser clock).
//! On non-wasm targets (the `cargo test` runner on the host), uses `SystemTime`.
//! Both return `u64` milliseconds since the Unix epoch. The split exists because
//! tests run on the host but production runs in the browser.

#[cfg(target_arch = "wasm32")]
pub fn current_timestamp() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn returns_positive_millisecond_epoch() {
        let t = current_timestamp();
        // 2020-01-01T00:00:00Z = 1_577_836_800_000 ms — sanity guard that we're returning ms, not s.
        assert!(t > 1_577_836_800_000, "timestamp looked like seconds: {}", t);
    }

    #[test]
    fn monotonic_across_two_calls() {
        let a = current_timestamp();
        let b = current_timestamp();
        assert!(b >= a);
    }
}
