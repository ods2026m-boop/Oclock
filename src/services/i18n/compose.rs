//! Turning domain values into words.
//!
//! Everything here used to be a method on the domain type itself:
//! `Sound::name()`, `Repeat::label()`, `SortMode::label()`,
//! `Weekdays::label()`, `Alarm::display_time()`, `fmt::verbose()`. Each returned
//! English, which meant a language could only be changed by editing the domain
//! or by leaving those methods unused — and leaving them unused is exactly the
//! kind of dead English that drifts.
//!
//! Moving them onto [`Catalog`] fixes that. The domain keeps the *value* —
//! `Sound::Chime` is still `Sound::Chime` and still plays the same three
//! rising notes — and the catalogue supplies the *word*. Nothing here changes
//! behaviour: the same inputs produce the same scheduling, the same firing and
//! the same elapsed time as before. Only the language of the output changes.

use chrono::{Datelike, Timelike, Weekday};

use crate::core::tz::Zoned;
use crate::domain::alarm::{Alarm, Repeat, Weekdays};
use crate::domain::clock::HourFormat;
use crate::domain::sound::Sound;
use crate::domain::timer::TimerState;
use crate::domain::world::{DayRelation, SortMode};
use crate::services::i18n::catalog::{Arg, Catalog, Text};

impl Catalog {
    // --- durations ------------------------------------------------------------

    /// A duration in the compact form used inline: `5m 09s`, `2h 05m 09s`.
    ///
    /// The shape is the domain's — the largest unit that applies, then
    /// downwards — but the unit words are the catalogue's.
    pub fn duration(self, value: std::time::Duration) -> String {
        let total = value.as_secs();
        let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);

        match (hours, minutes, seconds) {
            (0, 0, s) => self.format(Text::DurationSeconds, &[Arg::from(s)]),
            (0, m, 0) => self.format(Text::DurationMinutes, &[Arg::from(m)]),
            (0, m, s) => self.format(Text::DurationMinutesSeconds, &[Arg::from(m), Arg::from(s)]),
            (h, 0, 0) => self.format(Text::DurationHours, &[Arg::from(h)]),
            (h, m, 0) => self.format(Text::DurationHoursMinutes, &[Arg::from(h), Arg::from(m)]),
            (h, m, s) => self.format(
                Text::DurationHoursMinutesSeconds,
                &[Arg::from(h), Arg::from(m), Arg::from(s)],
            ),
        }
    }

    /// A span given in whole minutes, as the world clock reports an offset:
    /// `45 min`, `2h 30m`.
    pub fn span_minutes(self, minutes: i32) -> String {
        let magnitude = minutes.unsigned_abs();
        if magnitude < 60 {
            return self.format(Text::SpanMinutes, &[Arg::from(magnitude)]);
        }

        let hours = magnitude / 60;
        let rest = magnitude % 60;
        if rest == 0 {
            self.format(Text::SpanHours, &[Arg::from(hours)])
        } else {
            self.format(Text::SpanHoursMinutes, &[Arg::from(hours), Arg::from(rest)])
        }
    }

    // --- simple vocabulary ---------------------------------------------------

    /// The name of a clock format, as the toggle shows it.
    pub fn hour_format(self, format: HourFormat) -> &'static str {
        match format {
            HourFormat::Twelve => self.text(Text::HourFormat12),
            HourFormat::TwentyFour => self.text(Text::HourFormat24),
        }
    }

    /// `AM` or `PM`, in the language being read.
    pub fn meridiem(self, is_pm: bool) -> &'static str {
        if is_pm {
            self.text(Text::MeridiemPm)
        } else {
            self.text(Text::MeridiemAm)
        }
    }

    /// A sound's name, as the picker shows it.
    pub fn sound_name(self, sound: Sound) -> &'static str {
        self.text(match sound {
            Sound::Chime => Text::SoundChime,
            Sound::Radar => Text::SoundRadar,
            Sound::Pulse => Text::SoundPulse,
            Sound::Bell => Text::SoundBell,
        })
    }

    /// A sound's one-line description, shown under the picker.
    pub fn sound_description(self, sound: Sound) -> &'static str {
        self.text(match sound {
            Sound::Chime => Text::SoundChimeDescription,
            Sound::Radar => Text::SoundRadarDescription,
            Sound::Pulse => Text::SoundPulseDescription,
            Sound::Bell => Text::SoundBellDescription,
        })
    }

    /// A timer's state, as the card shows it.
    pub fn timer_state(self, state: TimerState) -> &'static str {
        self.text(match state {
            TimerState::Idle => Text::TimerReady,
            TimerState::Running => Text::TimerRunning,
            TimerState::Paused => Text::TimerPaused,
            TimerState::Done => Text::TimerFinished,
        })
    }

    /// The world clock's ordering, as the chip shows it.
    pub fn sort_mode(self, mode: SortMode) -> &'static str {
        self.text(match mode {
            SortMode::Manual => Text::SortManual,
            SortMode::City => Text::SortCity,
            SortMode::Offset => Text::SortOffset,
        })
    }

    /// How a city's date reads against the viewer's own.
    pub fn day_relation(self, relation: DayRelation) -> &'static str {
        self.text(match relation {
            DayRelation::Yesterday => Text::DayYesterday,
            DayRelation::Today => Text::DayToday,
            DayRelation::Tomorrow => Text::DayTomorrow,
        })
    }

    // --- calendars -----------------------------------------------------------

    /// A weekday's short name, as a chip shows it.
    pub fn weekday_short(self, day: Weekday) -> &'static str {
        self.text(match day {
            Weekday::Mon => Text::WeekdayMon,
            Weekday::Tue => Text::WeekdayTue,
            Weekday::Wed => Text::WeekdayWed,
            Weekday::Thu => Text::WeekdayThu,
            Weekday::Fri => Text::WeekdayFri,
            Weekday::Sat => Text::WeekdaySat,
            Weekday::Sun => Text::WeekdaySun,
        })
    }

    /// A weekday's full name.
    pub fn weekday_long(self, day: Weekday) -> &'static str {
        self.text(match day {
            Weekday::Mon => Text::WeekdayLongMon,
            Weekday::Tue => Text::WeekdayLongTue,
            Weekday::Wed => Text::WeekdayLongWed,
            Weekday::Thu => Text::WeekdayLongThu,
            Weekday::Fri => Text::WeekdayLongFri,
            Weekday::Sat => Text::WeekdayLongSat,
            Weekday::Sun => Text::WeekdayLongSun,
        })
    }

    /// A month name, 1-based.
    pub fn month(self, month: u32) -> &'static str {
        let key = match month {
            1 => Text::MonthJanuary,
            2 => Text::MonthFebruary,
            3 => Text::MonthMarch,
            4 => Text::MonthApril,
            5 => Text::MonthMay,
            6 => Text::MonthJune,
            7 => Text::MonthJuly,
            8 => Text::MonthAugust,
            9 => Text::MonthSeptember,
            10 => Text::MonthOctober,
            11 => Text::MonthNovember,
            12 => Text::MonthDecember,
            _ => return "",
        };
        self.text(key)
    }

    /// A full date line, in the language's own word order.
    ///
    /// The pieces are built here and the *order* comes from the translation, so
    /// a language that puts the year first, or writes the day before the
    /// month, gets that without the code branching on language.
    pub fn date_full(self, at: Zoned) -> String {
        let date = at.date_naive();
        self.format(
            Text::DateFull,
            &[
                Arg::from(self.weekday_long(date.weekday())),
                Arg::from(date.day()),
                Arg::from(self.month(date.month())),
                Arg::from(date.year()),
            ],
        )
    }

    /// A short date, as a world-clock card shows it.
    pub fn date_short(self, at: Zoned) -> String {
        let date = at.date_naive();
        self.format(
            Text::DateShort,
            &[Arg::from(date.day()), Arg::from(self.month(date.month()))],
        )
    }

    // --- alarms --------------------------------------------------------------

    /// The days a repeat rule selects, as one phrase.
    ///
    /// The shortcuts come first — "Weekdays" and "Weekends" are what a person
    /// means — and only an irregular selection is spelled out day by day.
    pub fn weekdays(self, days: Weekdays) -> String {
        match days.len() {
            0 => self.text(Text::RepeatNever).to_string(),
            7 => self.text(Text::RepeatDaily).to_string(),
            5 if days == Weekdays::WORKDAYS => self.text(Text::RepeatWeekdays).to_string(),
            2 if days
                == Weekdays::NONE
                    .with(Weekday::Sat, true)
                    .with(Weekday::Sun, true) =>
            {
                self.text(Text::RepeatWeekends).to_string()
            }
            _ => {
                let names: Vec<&'static str> = days
                    .days()
                    .iter()
                    .map(|day| self.weekday_short(*day))
                    .collect();
                names.join(self.list_separator())
            }
        }
    }

    /// A repeat rule, as an alarm row shows it.
    pub fn repeat(self, repeat: Repeat) -> String {
        match repeat {
            Repeat::Once => self.text(Text::RepeatOnce).to_string(),
            Repeat::Daily => self.text(Text::RepeatDaily).to_string(),
            Repeat::Weekly(days) => self.weekdays(days),
        }
    }

    /// An alarm's time, honouring the reader's clock preference.
    ///
    /// The 24-hour form is digits and colons and is identical in every
    /// language, so it comes straight from the domain. The 12-hour form needs a
    /// meridiem word, which is the catalogue's.
    pub fn alarm_time(self, alarm: &Alarm, format: HourFormat) -> String {
        match format {
            HourFormat::TwentyFour => alarm.canonical_time(),
            HourFormat::Twelve => {
                let hour = alarm.hour;
                let (hour12, is_pm) = match hour {
                    0 => (12, false),
                    1..=11 => (hour, false),
                    12 => (12, true),
                    h => (h - 12, true),
                };
                self.format(
                    Text::Hour12Format,
                    &[
                        Arg::from(hour12),
                        Arg::from(alarm.minute),
                        Arg::from(self.meridiem(is_pm)),
                    ],
                )
            }
        }
    }

    /// A 12-hour alarm's time as two pieces: the digits, and the word that says
    /// which half of the day they are in.
    ///
    /// The meridiem is worth separating out. In Persian, Arabic and Urdu it is
    /// spelled out and is several times wider than the English "AM", and every
    /// one of the twelve tables puts it after the digits — so set at the same
    /// size as the digits, in a form of ordinary width, it wraps onto a second
    /// line and turns a one-line reading into a two-line one. Beside the digits
    /// at a smaller size it reads as a suffix, which is what it is.
    ///
    /// The digits come from the catalogue's own 24-hour pattern applied to the
    /// 12-hour hour, rather than from a format string written here, so that a
    /// language is still described in exactly one place.
    pub fn alarm_time_parts(
        self,
        alarm: &Alarm,
        format: HourFormat,
    ) -> (String, Option<&'static str>) {
        match format {
            HourFormat::TwentyFour => (alarm.canonical_time(), None),
            HourFormat::Twelve => {
                let hour = alarm.hour;
                let (hour12, is_pm) = match hour {
                    0 => (12, false),
                    1..=11 => (hour, false),
                    12 => (12, true),
                    h => (h - 12, true),
                };
                let digits = self.format(
                    Text::Hour24Format,
                    &[Arg::from(hour12), Arg::from(alarm.minute)],
                );
                (digits, Some(self.meridiem(is_pm)))
            }
        }
    }

    /// How a list is joined: `, ` for most, `، ` where the language uses an
    /// Arabic comma.
    pub fn list_separator(self) -> &'static str {
        match self.language() {
            crate::services::i18n::Language::Persian
            | crate::services::i18n::Language::Arabic
            | crate::services::i18n::Language::Urdu => "، ",
            _ => ", ",
        }
    }
}

impl Catalog {
    /// A whole reading on one line, as a world-clock card shows it.
    ///
    /// Lives here rather than on [`crate::domain::clock::ClockText`] because the
    /// 12-hour form ends in a word, and the word is the catalogue's.
    pub fn reading(self, text: &crate::domain::clock::ClockText) -> String {
        let meridiem = text.meridiem.map(|is_pm| self.meridiem(is_pm));
        match (&text.seconds, meridiem) {
            (Some(seconds), Some(meridiem)) => {
                format!("{}:{seconds} {meridiem}", text.digits)
            }
            (Some(seconds), None) => format!("{}:{seconds}", text.digits),
            (None, Some(meridiem)) => format!("{} {meridiem}", text.digits),
            (None, None) => text.digits.clone(),
        }
    }

    /// The 12-hour time of day, as a person reads it.
    pub fn time_12h(self, at: Zoned) -> String {
        let (is_pm, hour12) = at.hour12();
        self.format(
            Text::Hour12Format,
            &[
                Arg::from(hour12),
                Arg::from(at.minute()),
                Arg::from(self.meridiem(is_pm)),
            ],
        )
    }

    /// The 24-hour time of day.
    pub fn time_24h(self, at: Zoned) -> String {
        self.format(
            Text::Hour24Format,
            &[Arg::from(at.hour()), Arg::from(at.minute())],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::i18n::Language;
    use chrono::TimeZone;

    fn morning() -> Zoned {
        chrono_tz::UTC
            .with_ymd_and_hms(2026, 9, 28, 9, 5, 0)
            .unwrap()
    }

    fn alarm(hour: u32, minute: u32) -> Alarm {
        Alarm {
            hour,
            minute,
            ..Alarm::default()
        }
    }

    #[test]
    fn english_durations_keep_the_shape_the_domain_produced() {
        let catalog = Catalog::new(Language::English);
        assert_eq!(catalog.duration(std::time::Duration::from_secs(0)), "0s");
        assert_eq!(catalog.duration(std::time::Duration::from_secs(45)), "45s");
        assert_eq!(catalog.duration(std::time::Duration::from_secs(300)), "5m");
        assert_eq!(
            catalog.duration(std::time::Duration::from_secs(309)),
            "5m 09s"
        );
        assert_eq!(catalog.duration(std::time::Duration::from_secs(7200)), "2h");
        assert_eq!(
            catalog.duration(std::time::Duration::from_secs(7509)),
            "2h 05m 09s"
        );
    }

    #[test]
    fn every_language_formats_a_duration_without_panicking() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            for seconds in [0u64, 1, 59, 60, 61, 3599, 3600, 7509] {
                let text = catalog.duration(std::time::Duration::from_secs(seconds));
                assert!(!text.is_empty(), "{language:?} at {seconds}s");
            }
        }
    }

    #[test]
    fn a_russian_duration_agrees_with_its_plural_forms() {
        let catalog = Catalog::new(Language::Russian);
        // The compact form uses the abbreviated unit, which is not plural in
        // Russian; what matters is that the numbers survived translation.
        let text = catalog.duration(std::time::Duration::from_secs(309));
        assert!(text.contains('5'), "{text}");
        assert!(text.contains('9'), "{text}");
    }

    #[test]
    fn a_span_shorter_than_an_hour_reads_in_minutes() {
        let catalog = Catalog::new(Language::English);
        assert_eq!(catalog.span_minutes(0), "0 min");
        assert_eq!(catalog.span_minutes(45), "45 min");
        assert_eq!(catalog.span_minutes(60), "1 h");
        assert_eq!(catalog.span_minutes(90), "1 h 30");
        assert_eq!(catalog.span_minutes(630), "10 h 30");
    }

    #[test]
    fn a_span_ignores_the_sign_of_its_input() {
        let catalog = Catalog::new(Language::English);
        assert_eq!(catalog.span_minutes(-45), catalog.span_minutes(45));
    }

    #[test]
    fn weekday_names_differ_between_the_two_lengths() {
        let catalog = Catalog::new(Language::English);
        let short = catalog.weekday_short(Weekday::Mon);
        let long = catalog.weekday_long(Weekday::Mon);
        assert_eq!(short, "Mon");
        assert_eq!(long, "Monday");
        assert_ne!(short, long);
    }

    #[test]
    fn every_language_names_every_weekday_and_month() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            for day in [
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
                Weekday::Sat,
                Weekday::Sun,
            ] {
                assert!(!catalog.weekday_short(day).is_empty(), "{language:?}");
                assert!(!catalog.weekday_long(day).is_empty(), "{language:?}");
            }
            for month in 1..=12 {
                assert!(!catalog.month(month).is_empty(), "{language:?} {month}");
            }
        }
    }

    #[test]
    fn the_seventh_and_thirteenth_months_have_no_name() {
        let catalog = Catalog::new(Language::English);
        assert_eq!(catalog.month(0), "");
        assert_eq!(catalog.month(13), "");
    }

    #[test]
    fn a_full_date_keeps_every_part() {
        let catalog = Catalog::new(Language::English);
        let line = catalog.date_full(morning());
        assert!(line.contains("Monday"), "{line}");
        assert!(line.contains("28"), "{line}");
        assert!(line.contains("September"), "{line}");
        assert!(line.contains("2026"), "{line}");
    }

    #[test]
    fn a_chinese_date_puts_the_year_first() {
        let catalog = Catalog::new(Language::Chinese);
        let line = catalog.date_full(morning());
        assert!(line.starts_with("2026"), "{line}");
        // The month is a word rather than a digit, which is why the month names
        // are catalogue keys at all.
        assert!(line.contains('九'), "{line} should name the month");
        assert!(line.contains("28"), "{line}");
        assert!(line.contains("星期一"), "{line} should name the weekday");
    }

    #[test]
    fn a_russian_date_names_the_month_before_the_day() {
        let catalog = Catalog::new(Language::Russian);
        let line = catalog.date_full(morning());
        // Russian writes "понедельник, 28 сентября 2026 г." — the full form
        // keeps the day first, but the short form is "сентября 28".
        assert!(line.contains("сентября"), "{line}");
        let short = catalog.date_short(morning());
        assert_eq!(short, "сентября 28");
    }

    #[test]
    fn a_short_date_keeps_the_day_and_month() {
        let catalog = Catalog::new(Language::English);
        // The full month name rather than chrono's abbreviated one: chrono's
        // `%b` is English-only, and a translated date needs a translated month
        // in it. The world-clock card has room for it.
        assert_eq!(catalog.date_short(morning()), "28 September");
    }

    #[test]
    fn a_short_date_keeps_the_day_and_month_in_every_language() {
        for language in Language::ALL {
            let short = Catalog::new(language).date_short(morning());
            assert!(short.contains("28"), "{language:?}: {short}");
            // No language may leave a placeholder unsubstituted.
            assert!(!short.contains('{'), "{language:?}: {short}");
        }
    }

    #[test]
    fn repeat_rules_keep_their_shortcuts() {
        let catalog = Catalog::new(Language::English);
        assert_eq!(catalog.repeat(Repeat::Once), "Once");
        assert_eq!(catalog.repeat(Repeat::Daily), "Every day");
        assert_eq!(catalog.weekdays(Weekdays::WORKDAYS), "Weekdays");
        assert_eq!(
            catalog.weekdays(
                Weekdays::NONE
                    .with(Weekday::Sat, true)
                    .with(Weekday::Sun, true)
            ),
            "Weekends"
        );
        assert_eq!(catalog.weekdays(Weekdays::NONE), "Never");
    }

    #[test]
    fn an_irregular_week_is_spelled_out() {
        // These assertions used to live in `domain::alarm`, next to the code
        // that used to produce them. The wording moved; the behaviour is the
        // same, so the tests moved with it.
        let catalog = Catalog::new(Language::English);
        let days = Weekdays::from_days([Weekday::Mon, Weekday::Wed, Weekday::Fri]);
        assert_eq!(catalog.weekdays(days), "Mon, Wed, Fri");
        assert_eq!(catalog.weekdays(Weekdays::ALL), "Every day");
        assert_eq!(catalog.weekdays(Weekdays::WORKDAYS), "Weekdays");
        assert_eq!(
            catalog.weekdays(Weekdays::from_days([Weekday::Sat, Weekday::Sun])),
            "Weekends"
        );
        assert_eq!(catalog.weekdays(Weekdays::NONE), "Never");
    }

    #[test]
    fn every_language_spells_out_an_irregular_week() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            let text = catalog.weekdays(Weekdays::from_days([Weekday::Mon, Weekday::Wed]));
            assert!(!text.is_empty(), "{language:?}");
            assert!(
                !text.contains('{'),
                "{language:?} left a placeholder: {text}"
            );
        }
    }

    #[test]
    fn a_persian_week_uses_the_arabic_comma() {
        let catalog = Catalog::new(Language::Persian);
        let days = Weekdays::NONE
            .with(Weekday::Mon, true)
            .with(Weekday::Wed, true);
        let text = catalog.weekdays(days);
        assert!(text.contains('،'), "{text} should use the Arabic comma");
    }

    #[test]
    fn a_24_hour_alarm_time_is_identical_in_every_language() {
        // Digits and a colon are not language-specific, and the domain already
        // produces the canonical form; nothing should change it.
        let alarm = alarm(6, 30);
        for language in Language::ALL {
            assert_eq!(
                Catalog::new(language).alarm_time(&alarm, HourFormat::TwentyFour),
                "06:30",
                "{language:?}"
            );
        }
    }

    #[test]
    fn a_reading_names_its_meridiem_in_the_language() {
        // `ClockText::joined` moved here from the domain, because the 12-hour
        // form ends in a word and the word is the catalogue's.
        let tz = chrono_tz::UTC;
        let twelve = crate::domain::clock::ClockDisplay {
            hour_format: HourFormat::Twelve,
            show_seconds: true,
            show_analog: true,
        };
        let morning = crate::domain::clock::format_time(
            &tz.with_ymd_and_hms(2026, 6, 15, 9, 41, 7).unwrap(),
            twelve,
        );
        assert_eq!(Catalog::english().reading(&morning), "09:41:07 AM");
        assert_eq!(
            Catalog::new(Language::Persian).reading(&morning),
            format!(
                "09:41:07 {}",
                Catalog::new(Language::Persian).meridiem(false)
            )
        );

        let mut no_seconds = twelve;
        no_seconds.show_seconds = false;
        let shorter = crate::domain::clock::format_time(
            &tz.with_ymd_and_hms(2026, 6, 15, 9, 41, 7).unwrap(),
            no_seconds,
        );
        assert_eq!(Catalog::english().reading(&shorter), "09:41 AM");

        let twenty_four = crate::domain::clock::ClockDisplay {
            hour_format: HourFormat::TwentyFour,
            show_seconds: true,
            show_analog: true,
        };
        let plain = crate::domain::clock::format_time(
            &tz.with_ymd_and_hms(2026, 6, 15, 9, 41, 7).unwrap(),
            twenty_four,
        );
        assert_eq!(Catalog::english().reading(&plain), "09:41:07");
    }

    #[test]
    fn a_12_hour_alarm_time_carries_a_meridiem_word() {
        let catalog = Catalog::new(Language::English);
        let midnight = alarm(0, 0);
        let noon = alarm(12, 0);
        let evening = alarm(21, 5);

        assert!(catalog
            .alarm_time(&midnight, HourFormat::Twelve)
            .contains("AM"));
        assert!(catalog.alarm_time(&noon, HourFormat::Twelve).contains("PM"));
        assert!(catalog
            .alarm_time(&evening, HourFormat::Twelve)
            .contains("PM"));
        // And the hour is folded into 12-hour form.
        assert!(catalog
            .alarm_time(&evening, HourFormat::Twelve)
            .contains("9"));
    }

    #[test]
    fn an_alarm_time_follows_the_hour_format() {
        // This lived in `domain::alarm` while `Alarm::display_time` lived there.
        // The folding of the hour is unchanged; only the meridiem word moved.
        let catalog = Catalog::new(Language::English);
        assert_eq!(
            catalog.alarm_time(&alarm(21, 5), HourFormat::TwentyFour),
            "21:05"
        );
        assert_eq!(
            catalog.alarm_time(&alarm(21, 5), HourFormat::Twelve),
            "09:05 PM"
        );
        assert_eq!(
            catalog.alarm_time(&alarm(0, 0), HourFormat::Twelve),
            "12:00 AM"
        );
        assert_eq!(
            catalog.alarm_time(&alarm(12, 0), HourFormat::Twelve),
            "12:00 PM"
        );
    }

    #[test]
    fn the_clock_time_keeps_its_digits_in_every_language() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            let twelve = catalog.time_12h(morning());
            let twenty_four = catalog.time_24h(morning());
            assert!(twelve.contains("9"), "{language:?}: {twelve}");
            assert!(twelve.contains("05"), "{language:?}: {twelve}");
            assert_eq!(twenty_four, "09:05", "{language:?}");
        }
    }

    #[test]
    fn every_language_names_every_sound_and_timer_state() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            for sound in Sound::ALL {
                assert!(!catalog.sound_name(sound).is_empty(), "{language:?}");
                assert!(!catalog.sound_description(sound).is_empty(), "{language:?}");
            }
            for state in [
                TimerState::Idle,
                TimerState::Running,
                TimerState::Paused,
                TimerState::Done,
            ] {
                assert!(!catalog.timer_state(state).is_empty(), "{language:?}");
            }
        }
    }
}

#[cfg(test)]
mod parts_tests {
    use super::*;
    use crate::domain::alarm::Alarm;
    use crate::services::i18n::Language;

    fn alarm(hour: u32, minute: u32) -> Alarm {
        Alarm {
            hour,
            minute,
            ..Alarm::default()
        }
    }

    #[test]
    fn the_two_parts_read_the_same_as_the_whole() {
        // The point of splitting the meridiem off is that it can be set at a
        // different size. The reading has to come out the same as it did before
        // the split, or the split has changed the application's answer and not
        // only its typography — so this is a test of both halves against the
        // whole string they replace.
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            for hour in [0, 1, 9, 11, 12, 13, 23] {
                let alarm = alarm(hour, 7);
                let (digits, meridiem) = catalog.alarm_time_parts(&alarm, HourFormat::Twelve);
                let whole = catalog.alarm_time(&alarm, HourFormat::Twelve);
                assert_eq!(
                    format!("{digits} {}", meridiem.unwrap_or_default()),
                    whole,
                    "{language:?} at {hour}:07"
                );
            }
        }
    }

    #[test]
    fn a_24_hour_reading_has_no_meridiem() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            let (digits, meridiem) =
                catalog.alarm_time_parts(&alarm(13, 5), HourFormat::TwentyFour);
            assert_eq!(digits, "13:05", "{language:?}");
            assert!(meridiem.is_none(), "{language:?} invented a meridiem");
        }
    }

    #[test]
    fn a_twelve_hour_reading_keeps_its_meridiem() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            // Midnight and noon are the two that a naive conversion gets wrong.
            assert_eq!(
                catalog.alarm_time_parts(&alarm(0, 0), HourFormat::Twelve).0,
                "12:00",
                "{language:?}"
            );
            assert_eq!(
                catalog
                    .alarm_time_parts(&alarm(12, 0), HourFormat::Twelve)
                    .0,
                "12:00",
                "{language:?}"
            );
            assert_ne!(
                catalog.alarm_time_parts(&alarm(0, 0), HourFormat::Twelve).1,
                catalog
                    .alarm_time_parts(&alarm(12, 0), HourFormat::Twelve)
                    .1,
                "{language:?} called midnight the same as noon"
            );
        }
    }
}
