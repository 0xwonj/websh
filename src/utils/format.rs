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

/// Join a base path with a name, handling empty base correctly.
///
/// Examples:
/// - `join_path("", "foo")` -> `"foo"`
/// - `join_path("dir", "file")` -> `"dir/file"`
/// - `join_path("a/b", "c")` -> `"a/b/c"`
pub fn join_path(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", base, name)
    }
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

/// Words-per-minute baseline for reading-time estimates. 230 wpm sits in
/// the middle of the commonly-cited 200–250 range and matches the Medium
/// "min read" convention. Tuned to make ~2,140 words round to ~9 min,
/// which is what authors expect from prose-heavy notes.
pub const READING_WPM: u32 = 230;

/// Compact display date: `2026-03-14` → `2026/0314`. Returns `None` if the
/// input doesn't open with a valid `YYYY-MM-DD` prefix. Used in the reader
/// title strip where the full ISO form would consume too much horizontal
/// space at narrow widths.
pub fn format_date_compact(value: &str) -> Option<String> {
    let prefix = iso_date_prefix(value)?;
    Some(format!(
        "{}/{}{}",
        &prefix[..4],
        &prefix[5..7],
        &prefix[8..10]
    ))
}

/// Format an integer with thousands separators (`2140` → `"2,140"`). Used
/// for word counts in the reader strip; ASCII comma is intentional for
/// LTR/RTL agnosticism and predictable monospace alignment.
pub fn format_thousands_u32(n: u32) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}

/// Estimated reading time in whole minutes, half-up rounded with a floor
/// of 1. Uses [`READING_WPM`] as the divisor. Rounding (not ceiling) keeps
/// the figure honest at the upper end — at 230 wpm, a 2,140-word note
/// reads in 9 min, not 10.
pub fn reading_time_minutes(words: u32) -> u32 {
    if words == 0 {
        return 1;
    }
    ((words + READING_WPM / 2) / READING_WPM).max(1)
}

/// If `value` begins with a 10-character `YYYY-MM-DD` prefix, return that
/// prefix as a borrowed slice. Otherwise return `None`. Used as a low-cost
/// sortable key for content dates.
pub fn iso_date_prefix(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    if bytes.len() >= 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(|byte| byte.is_ascii_digit())
        && bytes[5..7].iter().all(|byte| byte.is_ascii_digit())
        && bytes[8..10].iter().all(|byte| byte.is_ascii_digit())
    {
        Some(&value[..10])
    } else {
        None
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

    #[test]
    fn test_join_path() {
        assert_eq!(join_path("", "foo"), "foo");
        assert_eq!(join_path("dir", "file"), "dir/file");
        assert_eq!(join_path("a/b", "c"), "a/b/c");
    }

    #[test]
    fn format_date_compact_strips_dashes() {
        assert_eq!(format_date_compact("2026-03-14"), Some("2026/0314".into()));
        assert_eq!(format_date_compact("2024-01-01"), Some("2024/0101".into()));
    }

    #[test]
    fn format_date_compact_tolerates_trailing_time() {
        assert_eq!(
            format_date_compact("2026-03-14T09:30:00Z"),
            Some("2026/0314".into()),
        );
    }

    #[test]
    fn format_date_compact_rejects_malformed() {
        assert!(format_date_compact("2026/03/14").is_none());
        assert!(format_date_compact("2026-3-14").is_none());
        assert!(format_date_compact("not a date").is_none());
        assert!(format_date_compact("").is_none());
    }

    #[test]
    fn format_thousands_u32_inserts_separators() {
        assert_eq!(format_thousands_u32(0), "0");
        assert_eq!(format_thousands_u32(42), "42");
        assert_eq!(format_thousands_u32(999), "999");
        assert_eq!(format_thousands_u32(1_000), "1,000");
        assert_eq!(format_thousands_u32(2_140), "2,140");
        assert_eq!(format_thousands_u32(1_234_567), "1,234,567");
    }

    #[test]
    fn reading_time_minutes_half_up_with_floor_of_one() {
        assert_eq!(reading_time_minutes(0), 1);
        assert_eq!(reading_time_minutes(1), 1);
        assert_eq!(reading_time_minutes(115), 1); // half-boundary rounds up to 1
        assert_eq!(reading_time_minutes(230), 1);
        assert_eq!(reading_time_minutes(345), 2); // 1.5 rounds to 2
        // Matches the example from the design discussion: 2,140 words → 9 min.
        assert_eq!(reading_time_minutes(2_140), 9);
        assert_eq!(reading_time_minutes(2_300), 10);
    }
}

#[cfg(test)]
mod iso_date_prefix_tests {
    use super::*;

    #[test]
    fn iso_date_prefix_accepts_canonical_iso() {
        assert_eq!(iso_date_prefix("2026-04-22"), Some("2026-04-22"));
    }

    #[test]
    fn iso_date_prefix_accepts_iso_with_time_suffix() {
        assert_eq!(iso_date_prefix("2026-04-22T12:00:00Z"), Some("2026-04-22"));
    }

    #[test]
    fn iso_date_prefix_rejects_non_iso() {
        assert_eq!(iso_date_prefix(""), None);
        assert_eq!(iso_date_prefix("undated"), None);
        assert_eq!(iso_date_prefix("Apr 22, 2026"), None);
        assert_eq!(iso_date_prefix("2026/04/22"), None);
        assert_eq!(iso_date_prefix("2026-4-22"), None);
        assert_eq!(iso_date_prefix("20260422"), None);
    }
}
