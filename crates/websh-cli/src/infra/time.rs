//! UTC issuance dates are created only by explicit signing operations.
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after the Unix epoch")
        .as_secs()
        / 86_400;
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

pub(crate) fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after Unix epoch")
        .as_secs()
}
