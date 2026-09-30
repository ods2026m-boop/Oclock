//! Duration formatting.
//!
//! Countdowns, stopwatch readouts and "time until" labels all need the same
//! underlying arithmetic in slightly different clothes, so it lives here once.

use std::time::Duration;

use crate::core::tz::Zoned;

/// Formats a stopwatch reading with centisecond resolution.
///
/// Hours appear only once there are hours, so the readout keeps a stable width
/// for the first hour and then grows: `07.42`, `1:04.71`, `12:07:19.44`.
pub fn stopwatch(elapsed: Duration) -> String {
    let total = elapsed.as_millis() / 10;
    let centis = total % 100;
    let total_seconds = total / 100;
    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}.{centis:02}")
    } else {
        format!("{minutes:02}:{seconds:02}.{centis:02}")
    }
}

/// Formats a countdown. Negative values are treated as elapsed time, because a
/// finished timer should read `00:00`, never `-00:01`.
pub fn countdown(remaining: Duration) -> String {
    let total_seconds = remaining.as_secs();
    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Formats a countdown in the wide, fixed-field style used by timer inputs,
/// e.g. `01 : 30 : 00`.
pub fn countdown_padded(remaining: Duration) -> String {
    let total_seconds = remaining.as_secs();
    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;
    format!("{hours:02} : {minutes:02} : {seconds:02}")
}

/// Time left until the next whole minute boundary, from a wall-clock reading.
pub fn until_next_minute(now: Zoned) -> Duration {
    Duration::from_secs(60 - u64::from(now.timestamp_subsec_micros()) / 1_000_000 % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn stopwatch_omits_hours_until_needed() {
        assert_eq!(stopwatch(Duration::ZERO), "00:00.00");
        assert_eq!(stopwatch(ms(742)), "00:00.74");
        assert_eq!(stopwatch(ms(1_042)), "00:01.04");
        assert_eq!(stopwatch(ms(64_710)), "01:04.71");
        assert_eq!(stopwatch(ms(3_600_000)), "1:00:00.00");
        assert_eq!(stopwatch(ms(45_439_440)), "12:37:19.44");
    }

    #[test]
    fn stopwatch_keeps_a_stable_width_for_the_first_hour() {
        let short = stopwatch(ms(4_000));
        let long = stopwatch(ms(3_600_000 + 4_000));
        // Two fields before the hour, three after; the minute/second pair
        // stays two digits wide in both.
        assert_eq!(short.matches(':').count(), 1);
        assert_eq!(long.matches(':').count(), 2);
        assert!(long.ends_with(":04.00"));
    }

    #[test]
    fn stopwatch_truncates_rather_than_rounds_up() {
        // 9.999s must read 09.99, never 10.00: a stopwatch that jumps ahead
        // of real time is worse than one that lags by a centisecond.
        assert_eq!(stopwatch(ms(9_999)), "00:09.99");
    }

    #[test]
    fn countdown_formats_minutes_and_hours() {
        assert_eq!(countdown(Duration::ZERO), "00:00");
        assert_eq!(countdown(ms(59_400)), "00:59");
        assert_eq!(countdown(ms(60_000)), "01:00");
        assert_eq!(countdown(ms(599_400)), "09:59");
        assert_eq!(countdown(ms(3_600_000)), "1:00:00");
        assert_eq!(countdown(ms(3_723_000)), "1:02:03");
    }

    #[test]
    fn countdown_padded_is_fixed_width() {
        assert_eq!(countdown_padded(Duration::ZERO), "00 : 00 : 00");
        assert_eq!(countdown_padded(ms(90_000)), "00 : 01 : 30");
        assert_eq!(countdown_padded(ms(3_723_000)), "01 : 02 : 03");
    }

    #[test]
    fn until_next_minute_lands_on_a_boundary() {
        let tz = chrono_tz::Europe::Istanbul;
        let now = crate::core::tz::resolve_wall_time_parts(tz, 2026, 6, 15, 10, 20).unwrap();
        let remaining = until_next_minute(now);
        assert!(remaining <= Duration::from_secs(60));
        assert!(remaining > Duration::ZERO);
    }
}
