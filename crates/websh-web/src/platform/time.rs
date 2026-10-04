//! Browser wall clock in milliseconds since the Unix epoch.

pub fn current_timestamp() -> u64 {
    js_sys::Date::now() as u64
}
