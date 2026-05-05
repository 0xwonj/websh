use std::time::{SystemTime, UNIX_EPOCH};

use websh_site::ACK_ARTIFACT_PATH;

mod discover;
mod gpg;
mod sign;

pub(crate) mod build;
pub(crate) mod subject;
pub(crate) mod verify;

pub(crate) use build::{AttestAllOptions, attest_all, attest_build, run_default};
pub(crate) use verify::verify;

pub(crate) const DEFAULT_HOMEPAGE_CONTENT: &[&str] = &[
    "crates/websh-web/src/features/home",
    "assets/themes",
    ACK_ARTIFACT_PATH,
];
pub(crate) const DEFAULT_SIGNATURE_DIR: &str = ".websh/local/crypto/attestations";
pub(crate) const DEFAULT_GPG_SIGNER: &str = "Wonjae Choi <wonjae@snu.ac.kr>";

pub(crate) fn today_utc() -> String {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}
