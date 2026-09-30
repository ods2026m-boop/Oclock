//! Clock presentation: preferences, formatting and the per-tick snapshot.
//!
//! The clock page's job is to make one number — the current time — the
//! loudest thing on screen. Everything else supports it. To keep that cheap
//! at 60 Hz, the *text* of the clock is formatted once per second by
//! [`ClockSnapshot::capture`]; the view then animates the smooth parts (the
//! second hand sweep, the minute ring) from cached fractions.

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};

use crate::core::tz::{self, Zoned};

/// Whether the clock reads 12- or 24-hour time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HourFormat {
    /// `09:41 AM`
    #[default]
    Twelve,
    /// `09:41`
    TwentyFour,
}

impl HourFormat {
    /// The format used when the other one is chosen.
    pub fn toggled(self) -> HourFormat {
        match self {
            HourFormat::Twelve => HourFormat::TwentyFour,
            HourFormat::TwentyFour => HourFormat::Twelve,
        }
    }
}

/// How the user wants the main clock drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClockDisplay {
    /// 12- or 24-hour time.
    pub hour_format: HourFormat,
    /// Whether the seconds column is visible.
    pub show_seconds: bool,
    /// Whether the analog dial is shown beneath the readout.
    pub show_analog: bool,
}

impl Default for ClockDisplay {
    fn default() -> Self {
        ClockDisplay {
            hour_format: HourFormat::default(),
            show_seconds: true,
            show_analog: true,
        }
    }
}

/// The clock, split into the parts the layout draws separately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockText {
    /// `09:41`, or `9:41` with a leading zero suppressed in 12-hour mode.
    pub digits: String,
    /// `:23` when seconds are shown, otherwise `None`.
    pub seconds: Option<String>,
    /// Whether the reading is in the afternoon or evening, in 12-hour mode.
    ///
    /// A fact, not a word: `AM` and `PM` are English, and the half of the day
    /// is the same fact in every language that writes one. The word for it
    /// comes from the catalogue when the reading is drawn.
    pub meridiem: Option<bool>,
}

impl ClockText {
    /// Just the digits, for narrow layouts that place the meridiem elsewhere.
    pub fn digits_only(&self) -> &str {
        &self.digits
    }
}

/// Formats the time of day.
///
/// 12-hour mode keeps the leading zero, matching how desktop clocks
/// generally present `09:41 AM`, while still dropping it for the single-digit
/// hours some locales prefer via [`ClockText::digits`].
pub fn format_time(at: &Zoned, display: ClockDisplay) -> ClockText {
    let (pattern_digits, pattern_seconds) = match display.hour_format {
        // `%_I`-style padding is not portable across chrono versions, so the
        // hour is padded by hand and the minutes/seconds come from chrono.
        HourFormat::Twelve => ("%I:%M", "%S"),
        HourFormat::TwentyFour => ("%H:%M", "%S"),
    };

    let digits = at.format(pattern_digits).to_string();
    let seconds = display
        .show_seconds
        .then(|| at.format(pattern_seconds).to_string());

    // Whether this is the afternoon, rather than what the afternoon is called:
    // `%p` would hand back an English "AM"/"PM" on every machine.
    let meridiem = match display.hour_format {
        HourFormat::Twelve => Some(at.hour12().0),
        HourFormat::TwentyFour => None,
    };

    ClockText {
        digits,
        seconds,
        meridiem,
    }
}

/// Everything the clock page needs to draw itself, captured once per tick.
#[derive(Clone, Debug, PartialEq)]
pub struct ClockSnapshot {
    /// The instant this snapshot describes.
    pub at: Zoned,
    /// The timezone `at` is expressed in.
    pub zone: chrono_tz::Tz,
    /// The formatted reading.
    pub text: ClockText,
    /// The number of the week, already a number because it is one in every
    /// language; the *name* of the weekday is a catalogue lookup at draw time.
    /// `CET`, or a numeric offset when the database has no name.
    pub zone_abbreviation: String,
    /// `UTC+01:00`
    pub zone_offset: String,
    /// ISO week number.
    pub week: u32,
    /// Day of the year, 1-based.
    pub day_of_year: u32,
    /// Whether daylight saving is in effect in `zone` at `at`.
    pub daylight_saving: bool,
    /// Progress through the current second, `0.0..1.0`, sub-second precise.
    ///
    /// Derived from the wall clock rather than accumulated from ticks, so the
    /// sweep is smooth even if the refresh rate is not.
    pub second_fraction: f32,
    /// Progress through the current minute, `0.0..1.0`.
    pub minute_fraction: f32,
    /// Progress through the current hour, `0.0..1.0`.
    pub hour_fraction: f32,
}

impl ClockSnapshot {
    /// Captures a snapshot of the clock in `zone`.
    pub fn capture(at: Zoned, zone: chrono_tz::Tz, display: ClockDisplay) -> ClockSnapshot {
        use chrono::Timelike;

        let naive = at.naive_local();
        let subsecond = subsecond(&at);
        let within_minute = f32::from(naive.second() as u16) + subsecond;
        let within_hour = f32::from(naive.minute() as u16) * 60.0 + within_minute;

        let date = at.date_naive();

        ClockSnapshot {
            at,
            zone,
            text: format_time(&at, display),
            zone_abbreviation: tz::abbreviation(zone, at),
            zone_offset: tz::format_offset(tz::utc_offset_at(zone, at)),
            week: date.iso_week().week(),
            // `NaiveDate` has a private inherent `ordinal`, so the trait method
            // has to be named explicitly.
            day_of_year: Datelike::ordinal(&date),
            daylight_saving: tz::is_daylight_saving(zone, at),
            second_fraction: subsecond,
            minute_fraction: within_minute / 60.0,
            hour_fraction: within_hour / 3600.0,
        }
    }
}

/// How far into the current second `at` is, `0.0..1.0`.
///
/// Read from the wall clock rather than counted forward from the last tick. A
/// snapshot rebuilt once a second can say which second it is, but only the wall
/// clock can say how far into that second the frame being drawn is — which is
/// what a sweeping hand needs in order to sweep instead of step. Everything
/// that sweeps asks for it here, so that the hero's progress bar and the second
/// hand can never disagree about what second it is.
pub fn subsecond(at: &Zoned) -> f32 {
    at.naive_local().and_utc().timestamp_subsec_nanos() as f32 / 1_000_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Tz;

    fn at(tz: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> Zoned {
        crate::core::tz::resolve_wall_time_parts(tz, y, mo, d, h, mi)
            .unwrap()
            .with_timezone(&tz)
            + chrono::TimeDelta::seconds(s as i64)
    }

    fn twelve() -> ClockDisplay {
        ClockDisplay {
            hour_format: HourFormat::Twelve,
            show_seconds: true,
            show_analog: true,
        }
    }

    fn twentyfour() -> ClockDisplay {
        ClockDisplay {
            hour_format: HourFormat::TwentyFour,
            show_seconds: true,
            show_analog: true,
        }
    }

    fn hidden() -> ClockDisplay {
        ClockDisplay {
            hour_format: HourFormat::TwentyFour,
            show_seconds: false,
            show_analog: true,
        }
    }

    #[test]
    fn twelve_hour_formatting() {
        let tz = Tz::UTC;
        let text = format_time(&at(tz, 2026, 6, 15, 9, 41, 7), twelve());
        assert_eq!(text.digits, "09:41");
        assert_eq!(text.seconds.as_deref(), Some("07"));
        assert_eq!(text.meridiem, Some(false));

        let text = format_time(&at(tz, 2026, 6, 15, 13, 5, 0), twelve());
        assert_eq!(text.digits, "01:05");
        assert_eq!(text.meridiem, Some(true));

        let text = format_time(&at(tz, 2026, 6, 15, 0, 0, 0), twelve());
        assert_eq!(text.digits, "12:00");
        assert_eq!(text.meridiem, Some(false));

        let text = format_time(&at(tz, 2026, 6, 15, 23, 59, 59), twelve());
        assert_eq!(text.digits, "11:59");
        assert_eq!(text.meridiem, Some(true));
    }

    #[test]
    fn twenty_four_hour_formatting() {
        let tz = Tz::UTC;
        let text = format_time(&at(tz, 2026, 6, 15, 9, 41, 7), twentyfour());
        assert_eq!(text.digits, "09:41");
        assert_eq!(text.seconds.as_deref(), Some("07"));
        assert_eq!(text.meridiem, None);

        let text = format_time(&at(tz, 2026, 6, 15, 13, 5, 0), twentyfour());
        assert_eq!(text.digits, "13:05");

        let text = format_time(&at(tz, 2026, 6, 15, 0, 0, 0), twentyfour());
        assert_eq!(text.digits, "00:00");
    }

    #[test]
    fn seconds_can_be_hidden() {
        let tz = Tz::UTC;
        let text = format_time(&at(tz, 2026, 6, 15, 9, 41, 7), hidden());
        assert_eq!(text.digits, "09:41");
        assert_eq!(text.seconds, None);
    }

    #[test]
    fn date_formatting() {
        let tz = Tz::UTC;
        let at = at(tz, 2026, 9, 28, 12, 0, 0);
        let snapshot = ClockSnapshot::capture(at, tz, hidden());
        assert_eq!(snapshot.zone_offset, "UTC+00:00");
        // The weekday and the date are words rather than numbers, so they are
        // no longer baked into the snapshot; `Catalog` renders them from the
        // instant the snapshot carries. What the snapshot still owes is that
        // the instant is the one that was captured.
        assert_eq!(snapshot.at.timestamp(), at.timestamp());
    }

    #[test]
    fn fractions_describe_progress_through_the_unit() {
        let tz = Tz::UTC;
        let snapshot = ClockSnapshot::capture(at(tz, 2026, 6, 15, 10, 20, 30), tz, hidden());
        assert!((snapshot.second_fraction - 0.0).abs() < 1.0e-6);
        assert!((snapshot.minute_fraction - 0.5).abs() < 1.0e-6);
        assert!(
            (snapshot.hour_fraction - (20.0 * 60.0 + 30.0) / 3600.0).abs() < 1.0e-6,
            "hour fraction was {}",
            snapshot.hour_fraction
        );
    }

    #[test]
    fn fractions_advance_within_the_minute() {
        let tz = Tz::UTC;
        let first = ClockSnapshot::capture(at(tz, 2026, 6, 15, 10, 20, 0), tz, hidden());
        let last = ClockSnapshot::capture(at(tz, 2026, 6, 15, 10, 20, 59), tz, hidden());
        assert!(first.minute_fraction < 1.0e-6);
        assert!(last.minute_fraction > first.minute_fraction);
        assert!((last.minute_fraction - 59.0 / 60.0).abs() < 1.0e-6);
    }

    #[test]
    fn snapshot_reports_calendar_metadata() {
        let tz: Tz = "Europe/Berlin".parse().unwrap();
        let january = ClockSnapshot::capture(at(tz, 2026, 1, 1, 12, 0, 0), tz, hidden());
        let july = ClockSnapshot::capture(at(tz, 2026, 7, 1, 12, 0, 0), tz, hidden());

        assert_eq!(january.zone_abbreviation, "CET");
        assert_eq!(july.zone_abbreviation, "CEST");
        assert!(!january.daylight_saving);
        assert!(july.daylight_saving);
        assert_eq!(january.day_of_year, 1);
        assert_eq!(july.zone_offset, "UTC+02:00");
        assert!(january.week >= 1 && january.week <= 53);
    }

    #[test]
    fn default_display_shows_seconds() {
        let display = ClockDisplay::default();
        assert!(display.show_seconds);
        assert_eq!(display.hour_format, HourFormat::Twelve);
    }

    #[test]
    fn display_survives_a_json_round_trip() {
        let display = ClockDisplay {
            hour_format: HourFormat::TwentyFour,
            show_seconds: false,
            show_analog: false,
        };
        let json = serde_json::to_string(&display).expect("serializes");
        let restored: ClockDisplay = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(display, restored);
    }

    #[test]
    fn a_display_missing_fields_falls_back_to_defaults() {
        let restored: ClockDisplay = serde_json::from_str("{}").expect("deserializes");
        assert_eq!(restored, ClockDisplay::default());
    }

    #[test]
    fn format_toggle() {
        assert_eq!(HourFormat::Twelve.toggled(), HourFormat::TwentyFour);
        assert_eq!(HourFormat::TwentyFour.toggled(), HourFormat::Twelve);
    }
}
