//! UTC timestamps without a date crate. Civil-from-days conversion after H. Hinnant,
//! "chrono-Compatible Low-Level Date Algorithms" (published algorithm, proleptic Gregorian).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// (year, month 1..=12, day 1..=31) of a day count since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m as u32, d as u32)
}

fn parts(t: SystemTime) -> (i64, u32, u32, u64, u64, u64, u32) {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let s = d.as_secs();
    let (y, mo, da) = civil_from_days((s / 86_400) as i64);
    let r = s % 86_400;
    (y, mo, da, r / 3600, r / 60 % 60, r % 60, d.subsec_millis())
}

/// `yyyymmddThhmmss` (UTC), the run-id stamp.
pub fn compact(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s, _) = parts(t);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}")
}

/// `yyyy-mm-ddThh:mm:ss.mmmZ` (UTC, fixed width so text order is time order).
pub fn rfc3339_ms(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s, ms) = parts(t);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ms:03}Z")
}

/// [`rfc3339_ms`] of now.
pub fn now() -> String {
    rfc3339_ms(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        let at = |s: u64| rfc3339_ms(UNIX_EPOCH + Duration::from_secs(s));
        assert_eq!(at(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00.000Z");
        assert_eq!(at(951_868_800), "2000-03-01T00:00:00.000Z");
        assert_eq!(at(1_709_164_800), "2024-02-29T00:00:00.000Z");
        assert_eq!(at(1_700_000_000), "2023-11-14T22:13:20.000Z");
        assert_eq!(
            compact(UNIX_EPOCH + Duration::from_millis(1_700_000_000_999)),
            "20231114T221320"
        );
    }

    #[test]
    fn days_step_one_calendar_day_at_a_time() {
        // Every day from 1970 to about 2243 follows its predecessor in the calendar.
        let mut prev = civil_from_days(0);
        for z in 1..100_000 {
            let cur = civil_from_days(z);
            let next_day = (prev.0, prev.1, prev.2 + 1);
            let next_month = (prev.0, prev.1 + 1, 1);
            let next_year = (prev.0 + 1, 1, 1);
            assert!(
                cur == next_day || cur == next_month || cur == next_year,
                "day {z}: {prev:?} -> {cur:?}"
            );
            assert!(cur.2 <= 31 && (cur.1 != 2 || cur.2 <= 29), "{cur:?}");
            prev = cur;
        }
    }
}
