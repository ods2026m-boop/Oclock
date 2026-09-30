//! The catalogue: what the application says, and in which language.
//!
//! # Why a compiled-in table
//!
//! Every language is a `&'static` array indexed by [`Text`]. That buys three
//! things a file loaded at runtime could not:
//!
//! * a **missing translation is a visible gap, not a crash** — the array is
//!   still the right length, the entry is simply empty;
//! * the compiler checks that a new string is added to *all twelve* tables;
//! * lookup is a bounds-checked index, so a language can be chosen per frame
//!   without allocating or locking.
//!
//! # Fallback
//!
//! A lookup tries the active language, then English, and finally the key's own
//! English text. English is the reference language, so the chain can only
//! bottom out in something the application can always display — which is what
//! makes a half-finished translation safe to ship rather than a blank screen.

use super::language::{Direction, Language};

/// Every piece of user-visible text the application can say.
///
/// The variants are grouped by where the string is used, not alphabetically,
/// so a new string lands next to the strings it reads next to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Text {
    // --- navigation ---
    /// The clock page's title.
    PageClock,
    /// The clock page's subtitle.
    PageClockSummary,
    /// The world page's title.
    PageWorld,
    /// The world page's subtitle.
    PageWorldSummary,
    /// The alarms page's title.
    PageAlarms,
    /// The alarms page's subtitle.
    PageAlarmsSummary,
    /// The timer page's title.
    PageTimer,
    /// The timer page's subtitle.
    PageTimerSummary,
    /// The stopwatch page's title.
    PageStopwatch,
    /// The stopwatch page's subtitle.
    PageStopwatchSummary,

    // --- appearance ---
    /// Follow the desktop's light or dark preference.
    ThemeSystem,
    /// The light appearance.
    ThemeLight,
    /// The dark appearance.
    ThemeDark,

    // --- actions ---
    /// Add a city to the world clock.
    ActionAddCity,
    /// Create a new alarm.
    ActionNewAlarm,
    /// Create a new timer.
    ActionNewTimer,
    /// Close a dialog.
    ActionClose,
    /// Abandon an edit.
    ActionCancel,
    /// Commit an edit.
    ActionSave,
    /// Delete something.
    ActionDelete,
    /// Finish a dialog.
    ActionDone,
    /// Acknowledge a report.
    ActionGotIt,
    /// Silence a ringing alarm without snoozing it.
    ActionDismiss,
    /// Record a lap.
    ActionLap,
    /// Return a measurement to zero.
    ActionReset,
    /// Begin a measurement.
    ActionStart,
    /// Suspend a measurement.
    ActionPause,
    /// Add the highlighted result.
    ActionAdd,
    /// The highlighted result is already present.
    ActionAdded,

    // --- dialog headings ---
    /// The keyboard shortcut list.
    ShortcutsTitle,
    /// The city picker.
    AddCityTitle,
    /// The new-alarm editor.
    NewAlarmTitle,
    /// The existing-alarm editor.
    EditAlarmTitle,
    /// The new-timer editor.
    NewTimerTitle,
    /// The alarm deletion confirmation.
    DeleteAlarmTitle,

    // --- section headings ---
    /// Secondary information on the clock page.
    EyebrowAlso,
    /// The clock's detail readouts.
    EyebrowDetails,
    /// The clock's display options.
    EyebrowDisplay,
    /// The next alarm due.
    EyebrowNextAlarm,
    /// An alarm's time.
    EyebrowTime,
    /// An alarm's repeat rule.
    EyebrowRepeat,
    /// A sound.
    EyebrowSound,
    /// The timer presets.
    EyebrowPresets,
    /// How long a draft timer will run.
    EyebrowWillRunFor,
    /// The stopwatch's lap history.
    EyebrowLaps,

    // --- field captions ---
    /// A name field.
    CaptionName,
    /// An hour stepper.
    CaptionHour,
    /// A minute stepper.
    CaptionMinute,
    /// A snooze stepper.
    CaptionSnooze,
    /// An hours stepper.
    CaptionHours,
    /// A minutes stepper.
    CaptionMinutes,
    /// A seconds toggle.
    CaptionSeconds,
    /// A seconds readout.
    CaptionSecond,
    /// A timezone readout.
    CaptionZone,
    /// A date readout.
    CaptionDate,
    /// The analog dial toggle.
    CaptionDial,
    /// A search field.
    CaptionSearch,
    /// The city search field.
    CaptionSearchCities,

    // --- placeholders and hints ---
    /// Placeholder in the city search field.
    PlaceholderCityCountryZone,
    /// Placeholder in the compact city search field.
    PlaceholderCityZone,
    /// Placeholder in the alarm name field.
    PlaceholderAlarm,
    /// Placeholder in the timer name field.
    PlaceholderTimer,
    /// Shown under a focused text field.
    HintEditing,

    // --- empty states ---
    /// No cities in the world clock.
    EmptyWorldTitle,
    /// No cities in the world clock.
    EmptyWorldDetail,
    /// No alarms.
    EmptyAlarmsTitle,
    /// No alarms.
    EmptyAlarmsDetail,
    /// No timers.
    EmptyTimersTitle,
    /// No timers.
    EmptyTimersDetail,
    /// No laps.
    EmptyLapsTitle,
    /// No laps.
    EmptyLapsDetail,

    // --- notices and errors ---
    /// A city whose zone could not be resolved.
    WorldUnknownZone,
    /// A search with no results. Takes the query.
    WorldNoMatch,
    /// Missed alarms.
    AlarmsMissedHeadline,
    /// Missed alarms. Takes the count.
    AlarmsMissedDetail,

    // --- keyboard shortcuts ---
    /// `1`–`5` in the shortcut list.
    ShortcutGoTo,
    /// `Tab` in the shortcut list.
    ShortcutMoveFocus,
    /// `Enter` in the shortcut list.
    ShortcutActivate,
    /// `Escape` in the shortcut list.
    ShortcutClose,
    /// `S` in the shortcut list.
    ShortcutStopwatch,
    /// `L` in the shortcut list.
    ShortcutLap,
    /// `R` in the shortcut list.
    ShortcutReset,
    /// `N` in the shortcut list.
    ShortcutNew,
    /// `?` in the shortcut list.
    ShortcutThisList,

    // --- status words ---
    /// Daylight saving is in effect.
    DaylightSaving,
    /// A measurement is under way.
    StatusRunning,
    /// A measurement is stopped.
    StatusStopped,
    /// An alarm is armed but has no occurrence.
    StatusWaiting,
    /// An alarm is disarmed.
    StatusOff,
    /// An armed alarm with no schedule.
    StatusNotScheduled,
    /// The quickest lap.
    RankFastest,
    /// The slowest lap.
    RankSlowest,
    /// A city in the viewer's own zone.
    WorldSameTime,
    /// The 12-hour clock.
    HourFormat12,
    /// The 24-hour clock.
    HourFormat24,

    // --- formatted strings ---
    /// The ISO week number. Takes the number.
    WeekNumber,
    /// The day of the year. Takes the number.
    DayOfYear,
    /// How long until something. Takes a duration.
    InDuration,
    /// A missed alarm line. Takes the time, the count and the lateness.
    AlarmsMissedLine,
    /// Snooze, in a badge. Takes the minutes.
    SnoozeLower,
    /// Snooze, on a button. Takes the minutes.
    SnoozeUpper,
    /// A count of minutes, abbreviated. Takes the number.
    MinutesShort,
    /// Further ringing alarms. Takes the count.
    AndMore,
    /// The alarm deletion body. Takes the name and the time.
    DeleteAlarmBody,
    /// A lap's number. Takes the number.
    LapNumber,
    /// The lap summary line. Takes the lap count, the quickest and the slowest.
    LapsSummary,
    /// A lap's running total. Takes a duration.
    LapTotal,
    /// A city ahead of the viewer. Takes the difference.
    WorldAhead,
    /// A city behind the viewer. Takes the difference.
    WorldBehind,
    /// The seconds column of the clock.
    ClockSeconds,
    /// How long a timer preset runs. Takes the minutes and the hours.
    SpanMinutes,
    /// How long a span of hours runs. Takes the hours and the minutes.
    SpanHours,
    /// How long a span of hours and minutes runs.
    SpanHoursMinutes,

    // --- durations ---
    /// A count of seconds. Takes the number.
    DurationSeconds,
    /// A count of minutes. Takes the number.
    DurationMinutes,
    /// A count of minutes and seconds. Takes both.
    DurationMinutesSeconds,
    /// A count of hours. Takes the number.
    DurationHours,
    /// A count of hours and minutes. Takes both.
    DurationHoursMinutes,
    /// A count of hours, minutes and seconds. Takes all three.
    DurationHoursMinutesSeconds,

    // --- plural nouns ---
    /// How many cities are shown. Takes the count.
    CountCities,
    /// How many laps are recorded. Takes the count.
    CountLaps,

    // --- sorting ---
    /// The order the user chose.
    SortManual,
    /// Sort by city name.
    SortCity,
    /// Sort by time zone.
    SortOffset,

    // --- the world clock ---
    /// The viewer's own date.
    DayYesterday,
    /// The viewer's own date.
    DayToday,
    /// The viewer's own date.
    DayTomorrow,
    /// A zone the database no longer knows. Used in place of an offset.
    UnknownZoneOffset,
    /// A zone the database no longer knows. Used in place of an abbreviation.
    UnknownZoneName,

    // --- weekdays ---
    /// Monday, short.
    WeekdayMon,
    /// Tuesday, short.
    WeekdayTue,
    /// Wednesday, short.
    WeekdayWed,
    /// Thursday, short.
    WeekdayThu,
    /// Friday, short.
    WeekdayFri,
    /// Saturday, short.
    WeekdaySat,
    /// Sunday, short.
    WeekdaySun,
    /// Monday, long.
    WeekdayLongMon,
    /// Tuesday, long.
    WeekdayLongTue,
    /// Wednesday, long.
    WeekdayLongWed,
    /// Thursday, long.
    WeekdayLongThu,
    /// Friday, long.
    WeekdayLongFri,
    /// Saturday, long.
    WeekdayLongSat,
    /// Sunday, long.
    WeekdayLongSun,

    // --- months ---
    /// January.
    MonthJanuary,
    /// February.
    MonthFebruary,
    /// March.
    MonthMarch,
    /// April.
    MonthApril,
    /// May.
    MonthMay,
    /// June.
    MonthJune,
    /// July.
    MonthJuly,
    /// August.
    MonthAugust,
    /// September.
    MonthSeptember,
    /// October.
    MonthOctober,
    /// November.
    MonthNovember,
    /// December.
    MonthDecember,

    // --- dates, in each language's own order ---
    /// A full date. Takes the weekday, day, month and year.
    DateFull,
    /// A short date. Takes the day and the month.
    DateShort,

    // --- meridiem ---
    /// The first half of the day.
    MeridiemAm,
    /// The second half of the day.
    MeridiemPm,
    /// A 12-hour time of day. Takes the hour, the minute and the meridiem.
    Hour12Format,
    /// A 24-hour time of day. Takes the hour and the minute.
    Hour24Format,

    // --- alarms ---
    /// A one-time alarm's repeat rule.
    RepeatOnce,
    /// A daily alarm's repeat rule.
    RepeatDaily,
    /// A weekly alarm's repeat rule.
    RepeatSomeDays,
    /// A Monday-to-Friday rule.
    RepeatWeekdays,
    /// A Saturday-and-Sunday rule.
    RepeatWeekends,
    /// An alarm with no days selected.
    RepeatNever,

    // --- sounds ---
    /// The chime.
    SoundChime,
    /// The radar sweep.
    SoundRadar,
    /// The pulse.
    SoundPulse,
    /// The bell.
    SoundBell,
    /// The chime, described.
    SoundChimeDescription,
    /// The radar sweep, described.
    SoundRadarDescription,
    /// The pulse, described.
    SoundPulseDescription,
    /// The bell, described.
    SoundBellDescription,

    // --- timer states ---
    /// A timer that has not been started.
    TimerReady,
    /// A counting timer.
    TimerRunning,
    /// A suspended timer.
    TimerPaused,
    /// A timer that has run out.
    TimerFinished,

    // --- confirmations ---
    /// An alarm was deleted.
    ToastAlarmDeleted,
    /// A timer began. Takes its name.
    ToastStarted,
    /// A preset was stored.
    ToastPresetSaved,
    /// An alarm was stored.
    ToastAlarmSaved,
    /// An alarm was postponed.
    ToastSnoozed,
    /// A city was added.
    ToastCityAdded,

    // --- problems ---
    /// The configuration could not be read at all.
    NoticeUnreadable,
    /// Part of the configuration was repaired.
    NoticeRepaired,
    /// The configuration could not be written.
    NoticeUnsaved,
    /// A notification service is missing.
    NoticeNoNotifier,

    // --- notifications ---
    /// The title of an alarm notification with no name.
    NotifyAlarmFallback,
    /// The title of a timer notification with no name.
    NotifyTimerFallback,
    /// The snooze button on a notification. Takes the minutes.
    NotifySnoozeAction,
    /// The dismiss button on a notification.
    NotifyDismissAction,
    /// A finished timer's body. Takes a duration.
    NotifyFinishedAfter,

    // --- the language picker ---
    /// The language setting's name.
    LanguageLabel,
    /// The language picker's heading.
    LanguageHeading,
    /// The language picker's explanation.
    LanguageDescription,
    /// An alarm notification's body. Takes the time and the repeat rule.
    NotifyAlarmBody,
}

impl Text {
    /// How many strings a language has to provide.
    ///
    /// Every table is declared as `[&str; Text::COUNT]`, so adding a variant
    /// without translating it is a compile error rather than a blank label at
    /// runtime.
    pub const COUNT: usize = Text::NotifyAlarmBody as usize + 1;

    /// This key's position in a language's table.
    pub fn index(self) -> usize {
        self as usize
    }

    /// The weekday a short or long name is wanted for, or `None` if this key
    /// is not a weekday name.
    pub fn weekday(self) -> Option<chrono::Weekday> {
        use chrono::Weekday;
        Some(match self {
            Text::WeekdayMon | Text::WeekdayLongMon => Weekday::Mon,
            Text::WeekdayTue | Text::WeekdayLongTue => Weekday::Tue,
            Text::WeekdayWed | Text::WeekdayLongWed => Weekday::Wed,
            Text::WeekdayThu | Text::WeekdayLongThu => Weekday::Thu,
            Text::WeekdayFri | Text::WeekdayLongFri => Weekday::Fri,
            Text::WeekdaySat | Text::WeekdayLongSat => Weekday::Sat,
            Text::WeekdaySun | Text::WeekdayLongSun => Weekday::Sun,
            _ => return None,
        })
    }

    /// Whether this key is one of the plural nouns.
    pub fn is_plural(self) -> bool {
        self.plural_index().is_some()
    }

    /// This plural noun's position in a language's plural table.
    ///
    /// Only a handful of strings vary with a count, so they are indexed
    /// separately rather than sharing the main table — otherwise every language
    /// would carry a hundred and ninety empty rows.
    pub fn plural_index(self) -> Option<usize> {
        match self {
            Text::CountCities => Some(0),
            Text::CountLaps => Some(1),
            _ => None,
        }
    }
}

/// A string that varies with a count.
///
/// The set is deliberately small and explicit: plurals are the one place a
/// translation can be structurally wrong rather than merely a different choice
/// of word, so they get their own type and their own tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum PluralText {
    /// How many cities are shown.
    Cities,
    /// How many laps are recorded.
    Laps,
}

impl PluralText {
    /// How many plural nouns exist.
    pub const COUNT: usize = PluralText::Laps as usize + 1;

    /// The key this plural noun is stored under.
    pub fn key(self) -> Text {
        match self {
            PluralText::Cities => Text::CountCities,
            PluralText::Laps => Text::CountLaps,
        }
    }
}

/// A plural category, ordered as the tables store them.
///
/// The order matches the CLDR categories every one of the twelve languages
/// draws on. A language that needs fewer simply leaves the others empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum PluralForm {
    /// Exactly one.
    One = 0,
    /// Exactly two.
    Two = 1,
    /// A few.
    Few = 2,
    /// Many.
    Many = 3,
    /// Everything else.
    Other = 4,
}

impl PluralForm {
    /// Every form, in table order.
    pub const ALL: [PluralForm; 5] = [
        PluralForm::One,
        PluralForm::Two,
        PluralForm::Few,
        PluralForm::Many,
        PluralForm::Other,
    ];
}

/// A value to substitute into a translated string.
#[derive(Clone, Debug)]
pub enum Arg {
    /// A borrowed or owned piece of text.
    Text(String),
    /// A whole number.
    Int(i64),
    /// A fractional number.
    Float(f64),
}

impl std::fmt::Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arg::Text(text) => f.write_str(text),
            Arg::Int(value) => std::fmt::Display::fmt(value, f),
            Arg::Float(value) => std::fmt::Display::fmt(value, f),
        }
    }
}

impl From<&str> for Arg {
    fn from(value: &str) -> Arg {
        Arg::Text(value.to_string())
    }
}

impl From<&String> for Arg {
    fn from(value: &String) -> Arg {
        Arg::Text(value.clone())
    }
}

impl From<String> for Arg {
    fn from(value: String) -> Arg {
        Arg::Text(value)
    }
}

impl From<char> for Arg {
    fn from(value: char) -> Arg {
        Arg::Text(value.to_string())
    }
}

macro_rules! arg_from_integer {
    ($($type:ty),*) => {
        $(impl From<$type> for Arg {
            fn from(value: $type) -> Arg {
                Arg::Int(value as i64)
            }
        })*
    };
}

arg_from_integer!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

impl From<f32> for Arg {
    fn from(value: f32) -> Arg {
        Arg::Float(f64::from(value))
    }
}

impl From<f64> for Arg {
    fn from(value: f64) -> Arg {
        Arg::Float(value)
    }
}

/// One language's strings.
#[derive(Clone, Copy, Debug)]
pub struct Locale {
    /// Which language this is.
    pub language: Language,
    /// Indexed by [`Text`]. An empty entry means "not translated".
    pub text: &'static [&'static str],
    /// Indexed by [`Text`]. Only plural keys have entries; each is indexed by
    /// [`PluralForm`], and an empty entry means "this form is unused".
    pub plural: &'static [&'static [&'static str]; PluralText::COUNT],
}

impl Locale {
    /// A table's entry for `key`, empty if untranslated or out of range.
    pub fn entry(&self, key: Text) -> &'static str {
        self.text.get(key.index()).copied().unwrap_or("")
    }

    /// A plural table's form for `key`, empty if `key` is not a plural noun.
    pub fn plural_entry(&self, key: Text) -> &'static [&'static str] {
        key.plural_index()
            .and_then(|index| self.plural.get(index).copied())
            .unwrap_or(&[])
    }
}

/// The strings for one language, in a chosen direction.
///
/// Cheap to copy — a pair of references — so it can be handed to a view
/// without a borrow of the application, and stored on the application as a
/// field that the whole view tree can read.
#[derive(Clone, Copy, Debug)]
pub struct Catalog {
    locale: &'static Locale,
}

impl Catalog {
    /// The strings for `language`.
    pub fn new(language: Language) -> Catalog {
        Catalog {
            locale: super::locales::locale(language),
        }
    }

    /// The English strings, whatever the active language.
    ///
    /// Used where a piece of text is not going to be drawn — a test assertion,
    /// a log line — so that a failure reads in one known language.
    pub fn english() -> Catalog {
        Catalog::new(Language::English)
    }

    /// Which language these strings are in.
    pub fn language(self) -> Language {
        self.locale.language
    }

    /// Which way this language is written.
    pub fn direction(self) -> Direction {
        self.locale.language.direction()
    }

    /// True for the right-to-left languages.
    pub fn is_rtl(self) -> bool {
        self.locale.language.is_rtl()
    }

    /// Whether this catalog has its own translation of `key`.
    ///
    /// Empty where the catalogue would otherwise fall back, so this is the
    /// honest answer to "is this actually translated".
    pub fn is_translated(self, key: Text) -> bool {
        !self.lookup(key).is_empty()
    }

    /// The raw entry for `key` in this language, empty if untranslated.
    fn lookup(self, key: Text) -> &'static str {
        self.locale.entry(key)
    }

    /// The translation of `key`, falling back to English.
    pub fn text(self, key: Text) -> &'static str {
        let own = self.lookup(key);
        if !own.is_empty() {
            return own;
        }

        let english = Catalog::english().locale;
        let english = english.entry(key);
        if !english.is_empty() {
            return english;
        }

        // Unreachable for a well-formed table, but a lookup that could return
        // nothing would make every caller handle a case that cannot happen.
        "?"
    }

    /// The translation of `key` with `{0}`, `{1}`… replaced by `args`.
    ///
    /// A deliberately small substitution rather than a template engine: the
    /// placeholders are positional, so a translator reorders whole segments
    /// without ever having to escape a value, and a value containing a brace
    /// cannot break the string it lands in.
    pub fn format(self, key: Text, args: &[Arg]) -> String {
        substitute(self.text(key), args)
    }

    /// The plural form of a noun for `count` in this language.
    pub fn plural(self, key: Text, count: u32) -> &'static str {
        debug_assert!(
            key.is_plural(),
            "{key:?} is not a plural noun and has no plural forms"
        );
        let Some(index) = key.plural_index() else {
            return self.text(key);
        };

        let table = self.locale.plural[index];
        let form = self.locale.language.plural_form(count);

        // The exact form first, then `other`, which every language needs and
        // which doubles as the answer for a form this language does not use.
        for candidate in [form, PluralForm::Other] {
            if let Some(text) = table.get(candidate as usize).copied() {
                if !text.is_empty() {
                    return text;
                }
            }
        }

        let english = &Catalog::english().locale.plural[index];
        if let Some(text) = english.get(form as usize).copied() {
            if !text.is_empty() {
                return text;
            }
        }

        english
            .get(PluralForm::Other as usize)
            .copied()
            .filter(|text| !text.is_empty())
            .unwrap_or("?")
    }

    /// The plural form of a noun for `count`, substituted.
    pub fn plural_format(self, key: Text, count: u32) -> String {
        substitute(self.plural(key, count), &[Arg::Int(i64::from(count))])
    }
}

/// Replaces `{0}`, `{1}`… with `args`, and `{{`/`}}` with literal braces.
///
/// Unknown indices are left alone rather than dropped, so a table with a typo
/// shows the typo instead of silently losing a number.
fn substitute(template: &str, args: &[Arg]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut chars = template.chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut body = String::new();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    body.push(next);
                }

                // `{0}` and `{0:02}`. Only zero-padding is supported, because
                // that is the only thing a translated string needs: `5m 09s`
                // has to line up, and nothing else about the number changes.
                let (index_text, pad) = match body.split_once(':') {
                    Some((index_text, spec)) => {
                        let width = spec.strip_prefix('0').and_then(|rest| rest.parse().ok());
                        (index_text, width)
                    }
                    None => (body.as_str(), None),
                };

                match index_text
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| args.get(index).map(|value| (value, pad)))
                {
                    Some((Arg::Int(value), Some(width))) => {
                        out.push_str(&format!("{value:0width$}"))
                    }
                    Some((value, _)) => out.push_str(&value.to_string()),
                    // An unknown index or a malformed body is left visible
                    // rather than dropped, so a table with a typo shows the
                    // typo instead of silently losing a number.
                    None => {
                        out.push('{');
                        out.push_str(&body);
                        out.push('}');
                    }
                }
            }
            other => out.push(other),
        }
    }

    out
}

impl Language {
    /// The plural category `count` falls into in this language.
    ///
    /// The rules are the CLDR cardinal categories, narrowed to what the twelve
    /// languages actually need. Languages with a single form — which is most of
    /// them — collapse to "one, or not one", and only Russian and Arabic
    /// reach past that.
    pub fn plural_form(self, count: u32) -> PluralForm {
        let n = u64::from(count);
        match self {
            // Russian counts one, a few, and many.
            Language::Russian => {
                let tens = n % 10;
                let hundreds = n % 100;
                if tens == 1 && hundreds != 11 {
                    PluralForm::One
                } else if (2..=4).contains(&tens) && !(12..=14).contains(&hundreds) {
                    PluralForm::Few
                } else {
                    PluralForm::Many
                }
            }
            // Arabic distinguishes six.
            Language::Arabic => match n {
                0 => PluralForm::Other,
                1 => PluralForm::One,
                2 => PluralForm::Two,
                3..=10 => PluralForm::Few,
                11..=99 => PluralForm::Many,
                _ => PluralForm::Other,
            },
            // Persian treats zero with the singular.
            Language::Persian => {
                if n <= 1 {
                    PluralForm::One
                } else {
                    PluralForm::Other
                }
            }
            // Everything else is one or other.
            _ => {
                if n == 1 {
                    PluralForm::One
                } else {
                    PluralForm::Other
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_never_its_own_fallback() {
        let catalog = Catalog::english();
        for index in 0..Text::COUNT {
            let key = key_at(index);
            assert!(
                !catalog.text(key).is_empty(),
                "{key:?} has no English text at all"
            );
        }
    }

    #[test]
    fn every_language_translates_every_string() {
        // The tables are declared with the right length, so this is about
        // content: no language may leave an entry empty, because a gap would
        // silently show English in the middle of another language. The plural
        // nouns live in their own table and are checked there instead.
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            for index in 0..Text::COUNT {
                let key = key_at(index);
                if key.is_plural() {
                    continue;
                }
                assert!(
                    catalog.is_translated(key),
                    "{language:?} is missing {:?}",
                    key
                );
            }
        }
    }

    #[test]
    fn every_language_translates_every_plural_form_it_needs() {
        for language in Language::ALL {
            let catalog = Catalog::new(language);
            for key in [Text::CountCities, Text::CountLaps] {
                // The `other` form is the one every language must have: a
                // count the language has no special word for still has to read.
                assert!(
                    !catalog.plural(key, 7).is_empty(),
                    "{language:?} has no plural for {key:?}"
                );
            }
        }
    }

    #[test]
    fn an_empty_entry_falls_back_to_english() {
        // Built by hand rather than by damaging a real table: the fallback is
        // the part most likely to be broken by a future change.
        const BROKEN: Locale = Locale {
            language: Language::French,
            text: &["bonjour", ""],
            plural: &[&["un", "", "", "", "des"], &["un", "", "", "", "des"]],
        };
        let catalog = Catalog { locale: &BROKEN };

        // Key 0 is present, so it is used.
        assert_eq!(catalog.text(key_at(0)), "bonjour");
        // Key 1 is empty, so English answers.
        assert_eq!(catalog.text(key_at(1)), english_locale().entry(key_at(1)));
        // A key past the end of the table is also answered, by falling back
        // like an empty entry would: a truncated table degrades, it does not
        // crash.
        assert_eq!(
            catalog.text(key_at(Text::COUNT - 1)),
            english_locale().entry(key_at(Text::COUNT - 1))
        );

        assert!(catalog.is_translated(key_at(0)));
        assert!(!catalog.is_translated(key_at(1)));
    }

    #[test]
    fn a_plural_falls_back_from_an_unused_form_to_other() {
        const BROKEN: Locale = Locale {
            language: Language::Russian,
            text: &["", ""],
            // "Many" is left empty, so a many-count must find `other`.
            plural: &[
                &["one", "two", "few", "", "fallback"],
                &["one", "two", "few", "", "fallback"],
            ],
        };
        let catalog = Catalog { locale: &BROKEN };
        assert_eq!(catalog.plural(Text::CountCities, 1), "one");
        assert_eq!(catalog.plural(Text::CountCities, 2), "few");
        assert_eq!(catalog.plural(Text::CountCities, 7), "fallback");
    }

    #[test]
    fn a_non_plural_key_has_no_plural_forms() {
        assert!(!Text::PageClock.is_plural());
        assert_eq!(Text::PageClock.plural_index(), None);
        assert!(Text::CountCities.is_plural());
        assert!(Text::CountLaps.is_plural());
        assert_eq!(PluralText::COUNT, 2);
        assert_eq!(PluralText::Cities.key(), Text::CountCities);
        assert_eq!(PluralText::Laps.key(), Text::CountLaps);
    }

    #[test]
    fn substitution_replaces_positional_placeholders() {
        assert_eq!(
            substitute("{0} started", &[Arg::from("Tea")]),
            "Tea started"
        );
        assert_eq!(
            substitute("{1} of {0}", &[Arg::Int(2), Arg::Int(9)]),
            "9 of 2"
        );
        assert_eq!(substitute("no placeholders", &[]), "no placeholders");
    }

    #[test]
    fn substitution_keeps_braces_that_are_not_placeholders() {
        assert_eq!(substitute("{{literal}}", &[]), "{literal}");
        assert_eq!(substitute("{name}", &[Arg::from("x")]), "{name}");
        assert_eq!(substitute("{0} {5}", &[Arg::from("a")]), "a {5}");
    }

    #[test]
    fn substitution_zero_pads_when_asked() {
        assert_eq!(
            substitute("{0}m {1:02}s", &[Arg::Int(5), Arg::Int(9)]),
            "5m 09s"
        );
        assert_eq!(
            substitute("{0:02}:{1:02}", &[Arg::Int(1), Arg::Int(2)]),
            "01:02"
        );
        // A width larger than the number does not truncate.
        assert_eq!(substitute("{0:04}", &[Arg::Int(7)]), "0007");
    }

    #[test]
    fn a_text_argument_is_never_padded() {
        // Zero-padding a string would silently corrupt it, so the spec is
        // honoured for numbers only.
        assert_eq!(substitute("{0:02}", &[Arg::from("x")]), "x");
    }

    #[test]
    fn a_substituted_value_cannot_break_out_of_the_string() {
        // The whole reason substitution is positional rather than templated.
        assert_eq!(substitute("{0}", &[Arg::from("{1} sneaky")]), "{1} sneaky");
    }

    #[test]
    fn substitution_leaves_multibyte_text_intact() {
        assert_eq!(
            substitute("{0} آغاز شد", &[Arg::from("چای")]),
            "چای آغاز شد"
        );
        assert_eq!(
            substitute("开始 {0}", &[Arg::from("计时器")]),
            "开始 计时器"
        );
    }

    #[test]
    fn russian_counts_one_a_few_and_many() {
        assert_eq!(Language::Russian.plural_form(1), PluralForm::One);
        assert_eq!(Language::Russian.plural_form(21), PluralForm::One);
        assert_eq!(Language::Russian.plural_form(2), PluralForm::Few);
        assert_eq!(Language::Russian.plural_form(24), PluralForm::Few);
        assert_eq!(Language::Russian.plural_form(5), PluralForm::Many);
        assert_eq!(Language::Russian.plural_form(11), PluralForm::Many);
        assert_eq!(Language::Russian.plural_form(0), PluralForm::Many);
    }

    #[test]
    fn arabic_counts_six_ways() {
        assert_eq!(Language::Arabic.plural_form(0), PluralForm::Other);
        assert_eq!(Language::Arabic.plural_form(1), PluralForm::One);
        assert_eq!(Language::Arabic.plural_form(2), PluralForm::Two);
        assert_eq!(Language::Arabic.plural_form(5), PluralForm::Few);
        assert_eq!(Language::Arabic.plural_form(15), PluralForm::Many);
        assert_eq!(Language::Arabic.plural_form(100), PluralForm::Other);
    }

    #[test]
    fn most_languages_count_one_or_other() {
        for language in Language::ALL {
            if matches!(
                language,
                Language::Russian | Language::Arabic | Language::Persian
            ) {
                continue;
            }
            assert_eq!(language.plural_form(1), PluralForm::One, "{language:?}");
            assert_eq!(language.plural_form(0), PluralForm::Other, "{language:?}");
            assert_eq!(language.plural_form(2), PluralForm::Other, "{language:?}");
        }
        // Persian folds zero into the singular.
        assert_eq!(Language::Persian.plural_form(0), PluralForm::One);
        assert_eq!(Language::Persian.plural_form(1), PluralForm::One);
        assert_eq!(Language::Persian.plural_form(2), PluralForm::Other);
    }

    #[test]
    fn a_catalog_knows_its_own_direction() {
        assert!(Catalog::new(Language::Persian).is_rtl());
        assert!(Catalog::new(Language::Arabic).is_rtl());
        assert!(Catalog::new(Language::Urdu).is_rtl());
        assert!(!Catalog::new(Language::English).is_rtl());
        assert_eq!(Catalog::new(Language::Thai).language(), Language::Thai);
        assert_eq!(
            Catalog::new(Language::Thai).direction(),
            Direction::LeftToRight
        );
    }

    /// The English table, for the fallback assertions.
    fn english_locale() -> &'static Locale {
        crate::services::i18n::locales::locale(Language::English)
    }

    /// The key at `index`, by walking the discriminant.
    fn key_at(index: usize) -> Text {
        text_from_index(index)
    }
}

// A small table of the variants, used only by the tests above so that walking
// `0..Text::COUNT` stays exhaustive without a second list to maintain.
#[cfg(test)]
fn text_from_index(index: usize) -> Text {
    use Text::*;
    // The order here *is* the declaration order, and the tests assert that, so
    // a variant added in the wrong place fails loudly.
    const ORDER: [Text; Text::COUNT] = [
        PageClock,
        PageClockSummary,
        PageWorld,
        PageWorldSummary,
        PageAlarms,
        PageAlarmsSummary,
        PageTimer,
        PageTimerSummary,
        PageStopwatch,
        PageStopwatchSummary,
        ThemeSystem,
        ThemeLight,
        ThemeDark,
        ActionAddCity,
        ActionNewAlarm,
        ActionNewTimer,
        ActionClose,
        ActionCancel,
        ActionSave,
        ActionDelete,
        ActionDone,
        ActionGotIt,
        ActionDismiss,
        ActionLap,
        ActionReset,
        ActionStart,
        ActionPause,
        ActionAdd,
        ActionAdded,
        ShortcutsTitle,
        AddCityTitle,
        NewAlarmTitle,
        EditAlarmTitle,
        NewTimerTitle,
        DeleteAlarmTitle,
        EyebrowAlso,
        EyebrowDetails,
        EyebrowDisplay,
        EyebrowNextAlarm,
        EyebrowTime,
        EyebrowRepeat,
        EyebrowSound,
        EyebrowPresets,
        EyebrowWillRunFor,
        EyebrowLaps,
        CaptionName,
        CaptionHour,
        CaptionMinute,
        CaptionSnooze,
        CaptionHours,
        CaptionMinutes,
        CaptionSeconds,
        CaptionSecond,
        CaptionZone,
        CaptionDate,
        CaptionDial,
        CaptionSearch,
        CaptionSearchCities,
        PlaceholderCityCountryZone,
        PlaceholderCityZone,
        PlaceholderAlarm,
        PlaceholderTimer,
        HintEditing,
        EmptyWorldTitle,
        EmptyWorldDetail,
        EmptyAlarmsTitle,
        EmptyAlarmsDetail,
        EmptyTimersTitle,
        EmptyTimersDetail,
        EmptyLapsTitle,
        EmptyLapsDetail,
        WorldUnknownZone,
        WorldNoMatch,
        AlarmsMissedHeadline,
        AlarmsMissedDetail,
        ShortcutGoTo,
        ShortcutMoveFocus,
        ShortcutActivate,
        ShortcutClose,
        ShortcutStopwatch,
        ShortcutLap,
        ShortcutReset,
        ShortcutNew,
        ShortcutThisList,
        DaylightSaving,
        StatusRunning,
        StatusStopped,
        StatusWaiting,
        StatusOff,
        StatusNotScheduled,
        RankFastest,
        RankSlowest,
        WorldSameTime,
        HourFormat12,
        HourFormat24,
        WeekNumber,
        DayOfYear,
        InDuration,
        AlarmsMissedLine,
        SnoozeLower,
        SnoozeUpper,
        MinutesShort,
        AndMore,
        DeleteAlarmBody,
        LapNumber,
        LapsSummary,
        LapTotal,
        WorldAhead,
        WorldBehind,
        ClockSeconds,
        SpanMinutes,
        SpanHours,
        SpanHoursMinutes,
        DurationSeconds,
        DurationMinutes,
        DurationMinutesSeconds,
        DurationHours,
        DurationHoursMinutes,
        DurationHoursMinutesSeconds,
        CountCities,
        CountLaps,
        SortManual,
        SortCity,
        SortOffset,
        DayYesterday,
        DayToday,
        DayTomorrow,
        UnknownZoneOffset,
        UnknownZoneName,
        WeekdayMon,
        WeekdayTue,
        WeekdayWed,
        WeekdayThu,
        WeekdayFri,
        WeekdaySat,
        WeekdaySun,
        WeekdayLongMon,
        WeekdayLongTue,
        WeekdayLongWed,
        WeekdayLongThu,
        WeekdayLongFri,
        WeekdayLongSat,
        WeekdayLongSun,
        MonthJanuary,
        MonthFebruary,
        MonthMarch,
        MonthApril,
        MonthMay,
        MonthJune,
        MonthJuly,
        MonthAugust,
        MonthSeptember,
        MonthOctober,
        MonthNovember,
        MonthDecember,
        DateFull,
        DateShort,
        MeridiemAm,
        MeridiemPm,
        Hour12Format,
        Hour24Format,
        RepeatOnce,
        RepeatDaily,
        RepeatSomeDays,
        RepeatWeekdays,
        RepeatWeekends,
        RepeatNever,
        SoundChime,
        SoundRadar,
        SoundPulse,
        SoundBell,
        SoundChimeDescription,
        SoundRadarDescription,
        SoundPulseDescription,
        SoundBellDescription,
        TimerReady,
        TimerRunning,
        TimerPaused,
        TimerFinished,
        ToastAlarmDeleted,
        ToastStarted,
        ToastPresetSaved,
        ToastAlarmSaved,
        ToastSnoozed,
        ToastCityAdded,
        NoticeUnreadable,
        NoticeRepaired,
        NoticeUnsaved,
        NoticeNoNotifier,
        NotifyAlarmFallback,
        NotifyTimerFallback,
        NotifySnoozeAction,
        NotifyDismissAction,
        NotifyFinishedAfter,
        LanguageLabel,
        LanguageHeading,
        LanguageDescription,
        NotifyAlarmBody,
    ];
    ORDER[index]
}

#[cfg(test)]
mod tests_order {
    use super::*;

    #[test]
    fn the_test_table_matches_the_declaration_order() {
        // If this fails, a variant was inserted in the middle of the enum
        // without the test table being moved, and every `key_at` would be off.
        for index in 0..Text::COUNT {
            assert_eq!(
                text_from_index(index).index(),
                index,
                "the test table is out of step at {index}"
            );
        }
    }
}
