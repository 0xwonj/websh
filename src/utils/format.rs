//! Formatting utilities for file sizes, dates, and other display values.

/// Format file size for display (e.g., "1.2K", "3.4M").
///
/// Returns a right-aligned string for terminal display or compact string for UI.
pub fn format_size(size: Option<u64>, right_align: bool) -> String {
    match size {
        None => {
            if right_align {
                "    -".to_string()
            } else {
                "-".to_string()
            }
        }
        Some(bytes) => {
            if bytes >= 1_000_000 {
                if right_align {
                    format!("{:4.1}M", bytes as f64 / 1_000_000.0)
                } else {
                    format!("{:.1}M", bytes as f64 / 1_000_000.0)
                }
            } else if bytes >= 1_000 {
                if right_align {
                    format!("{:4.1}K", bytes as f64 / 1_000.0)
                } else {
                    format!("{:.1}K", bytes as f64 / 1_000.0)
                }
            } else if right_align {
                format!("{:4}B", bytes)
            } else {
                format!("{}B", bytes)
            }
        }
    }
}

/// Format Unix timestamp for terminal display (e.g., "Jan  5 12:34").
///
/// Uses approximate month/day calculation for simplicity.
pub fn format_date_short(timestamp: Option<u64>) -> String {
    match timestamp {
        None => "            ".to_string(),
        Some(ts) => {
            let months = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            // Approximate: days since epoch
            let days = ts / 86400;
            let month = ((days % 365) / 30) as usize % 12;
            let day = ((days % 365) % 30) + 1;
            let hour = (ts % 86400) / 3600;
            let min = (ts % 3600) / 60;
            format!("{} {:2} {:02}:{:02}", months[month], day, hour, min)
        }
    }
}

/// Format Unix timestamp as ISO date (YYYY-MM-DD).
///
/// Properly calculates year/month/day accounting for leap years.
pub fn format_date_iso(timestamp: u64) -> String {
    let days = timestamp / 86400;
    let mut year = 1970i64;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days_in_month in days_in_months.iter() {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }

    let day = remaining_days + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Check if a year is a leap year.
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Format elapsed time in seconds for boot messages (e.g., "[   0.123]").
pub fn format_elapsed(ms: f64) -> String {
    format!("[{:8.3}]", ms / 1000.0)
}

/// Format Ethereum address for display (0x1234...5678).
pub fn format_eth_address(address: &str) -> String {
    const PREFIX_LEN: usize = 6;
    const SUFFIX_START: usize = 38;
    const FULL_LEN: usize = 42;

    if address.len() >= FULL_LEN {
        format!("{}...{}", &address[..PREFIX_LEN], &address[SUFFIX_START..])
    } else {
        address.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(None, false), "-");
        assert_eq!(format_size(None, true), "    -");
        assert_eq!(format_size(Some(500), false), "500B");
        assert_eq!(format_size(Some(1500), false), "1.5K");
        assert_eq!(format_size(Some(1_500_000), false), "1.5M");
    }

    #[test]
    fn test_format_date_iso() {
        // Unix epoch
        assert_eq!(format_date_iso(0), "1970-01-01");
        // 2024-01-01 00:00:00 UTC = 1704067200
        assert_eq!(format_date_iso(1704067200), "2024-01-01");
    }

    #[test]
    fn test_format_eth_address() {
        let addr = "0x1234567890abcdef1234567890abcdef12345678";
        assert_eq!(format_eth_address(addr), "0x1234...5678");
        assert_eq!(format_eth_address("short"), "short");
    }

    #[test]
    fn test_format_elapsed() {
        assert_eq!(format_elapsed(123.0), "[   0.123]");
        assert_eq!(format_elapsed(1234.0), "[   1.234]");
    }
}
