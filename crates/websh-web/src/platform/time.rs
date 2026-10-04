//! Browser wall clock in milliseconds since the Unix epoch.

pub fn current_timestamp() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn returns_positive_millisecond_epoch() {
        let t = current_timestamp();
        let browser_now = js_sys::Date::now() as u64;
        // 2020-01-01T00:00:00Z = 1_577_836_800_000 ms — sanity guard that we're returning ms, not s.
        assert!(
            t > 1_577_836_800_000,
            "timestamp looked like seconds: {}",
            t
        );
        assert!(
            t.abs_diff(browser_now) < 1_000,
            "timestamp {t} drifted from browser clock {browser_now}"
        );
    }

    #[wasm_bindgen_test]
    fn monotonic_across_two_calls() {
        let a = current_timestamp();
        let b = current_timestamp();
        assert!(b >= a);
    }
}
