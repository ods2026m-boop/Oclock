//! The persisted configuration, and how old versions of it are read.
//!
//! One document holds everything the user has configured. Every field carries
//! `#[serde(default)]`, so a document written by an older build loads without
//! complaint, and [`Config::migrate`] upgrades the shape in memory before it is
//! ever written back. Nothing here knows about Iced or about files; see
//! [`crate::services::storage`] for the I/O.

use serde::{Deserialize, Serialize};

use crate::design::theme::ThemeMode;
use crate::domain::alarm::Alarm;
use crate::domain::clock::ClockDisplay;
use crate::domain::sound::Sound;
use crate::domain::timer::TimerPreset;
use crate::domain::world::{Location, SortMode};
use crate::services::i18n::Language;

/// The schema version written into every document.
pub const CURRENT_VERSION: u32 = 3;

/// Everything OClock remembers between launches.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Schema version, used to drive migrations.
    pub version: u32,
    /// Appearance preferences.
    pub appearance: Appearance,
    /// Main clock preferences.
    pub clock: ClockDisplay,
    /// The user's alarms.
    pub alarms: Vec<Alarm>,
    /// The world-clock collection.
    pub world: World,
    /// Timer presets and stopwatch preferences.
    pub timers: Timers,
    /// Audio preferences.
    pub sound: SoundSettings,
    /// The language the application speaks.
    ///
    /// Read starting from [`Language::default`] — English — so that a document
    /// written before this preference existed, and a hand-edited one that simply
    /// omits it, both start in the language the catalogue is written against. A
    /// fresh installation starts in English for the same reason: a desktop's
    /// locale is an ambient guess, and starting a first run in one would greet
    /// somebody in a language they never asked for. The choice is stored like
    /// any other preference once it is made.
    pub language: Language,
}

/// Appearance preferences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Which appearance the user asked for.
    pub mode: ThemeMode,
}

impl Appearance {
    /// The chosen mode.
    pub fn mode(self) -> ThemeMode {
        self.mode
    }
}

/// The world-clock collection as stored.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct World {
    /// The locations being watched.
    pub locations: Vec<Location>,
    /// How they are ordered.
    pub sort: SortMode,
}

/// Timer and stopwatch preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Timers {
    /// Saved timer durations.
    pub presets: Vec<TimerPreset>,
    /// Whether the stopwatch keeps a lap history across resets.
    ///
    /// A reset starts the run again either way; this decides whether the laps
    /// recorded so far survive it. Somebody timing a sequence of laps wants them
    /// to, and somebody timing a single run does not — so it is a preference,
    /// and it is applied rather than written back as a constant.
    pub keep_laps: bool,
}

impl Default for Timers {
    fn default() -> Self {
        Timers {
            presets: TimerPreset::defaults(),
            keep_laps: false,
        }
    }
}

/// Audio preferences.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundSettings {
    /// Master switch for playback.
    pub enabled: bool,
    /// Whether a sound is played alongside a desktop notification.
    pub notify: bool,
    /// The sound used for a finished timer.
    pub timer_sound: Sound,
    /// Default snooze length, in minutes.
    pub snooze_minutes: u8,
}

impl Default for SoundSettings {
    fn default() -> Self {
        SoundSettings {
            enabled: true,
            notify: true,
            timer_sound: Sound::default(),
            snooze_minutes: 9,
        }
    }
}

impl Config {
    /// Reads a configuration out of a parsed document, one field at a time.
    ///
    /// This is what gives the requirement its teeth: a single unreadable value
    /// costs only itself. Decoding whole sections would let one bad field
    /// inside `clock` throw away the user's other three clock preferences.
    ///
    /// Each group starts from its own `Default` and is then patched field by
    /// field, so a document that predates a preference — or a hand-edited one
    /// that omits it — gets that preference's *model* default rather than the
    /// bare `false` a `bool` would default to. A missing preference is normal,
    /// not damage, and is not reported.
    pub fn read(loader: &mut crate::services::storage::Loader) -> Config {
        let mut appearance = Appearance::default();
        loader.patch(&mut appearance.mode, &["appearance", "mode"]);

        // A missing or unreadable language is not damage: it means the document
        // predates the preference, or the hand-edited value is a language OClock
        // does not speak. Either way English is the right answer — the reference
        // language, and the one every string in the catalogue is written in — and
        // `patch` only reports a repair for a value that was present and wrong.
        let mut language = Language::default();
        loader.patch(&mut language, &["language"]);

        let mut clock = ClockDisplay::default();
        loader.patch(&mut clock.hour_format, &["clock", "hour_format"]);
        loader.patch(&mut clock.show_seconds, &["clock", "show_seconds"]);
        loader.patch(&mut clock.show_analog, &["clock", "show_analog"]);

        // The two collections are read as whole values: an entry that cannot
        // be understood is dropped by `sanitised`, but a collection that cannot
        // be understood at all falls back to the built-ins rather than to
        // nothing.
        let mut world = World::default();
        loader.patch(&mut world.locations, &["world", "locations"]);
        loader.patch(&mut world.sort, &["world", "sort"]);

        let mut timers = Timers::default();
        loader.patch(&mut timers.presets, &["timers", "presets"]);
        loader.patch(&mut timers.keep_laps, &["timers", "keep_laps"]);

        let mut sound = SoundSettings::default();
        loader.patch(&mut sound.enabled, &["sound", "enabled"]);
        loader.patch(&mut sound.notify, &["sound", "notify"]);
        loader.patch(&mut sound.timer_sound, &["sound", "timer_sound"]);
        loader.patch(&mut sound.snooze_minutes, &["sound", "snooze_minutes"]);

        Config {
            version: loader.field(&["version"]),
            appearance,
            clock,
            alarms: loader.field(&["alarms"]),
            world,
            timers,
            sound,
            language,
        }
    }

    /// The configuration a fresh installation starts with.
    ///
    /// Two example alarms would be presumptuous, so a new installation gets
    /// none; the world clock does get a small, useful set of starting points.
    pub fn first_run() -> Config {
        Config {
            version: CURRENT_VERSION,
            appearance: Appearance::default(),
            clock: ClockDisplay::default(),
            alarms: Vec::new(),
            world: World {
                locations: default_locations(),
                sort: SortMode::Offset,
            },
            timers: Timers::default(),
            sound: SoundSettings::default(),
            // A fresh installation starts in English, whatever the desktop's
            // locale happens to be. The user has chosen nothing yet, and English
            // is the language the whole catalogue is written against: a locale is
            // an ambient guess, and starting a first run in one would greet
            // somebody in a language they never asked for. Anyone who wants
            // another one changes it in the window, and that choice is then
            // stored like any other preference.
            language: Language::default(),
        }
    }

    /// Brings a document read from disk up to the current schema.
    ///
    /// Runs in memory; the migrated document is written back the next time
    /// anything is saved. Each step is a no-op for documents already at or
    /// beyond that version, so applying it twice is harmless.
    pub fn migrate(mut self) -> Config {
        if self.version == 0 {
            // Version 0 is a document with no `version` field at all, written
            // before the field existed. Treat it as version 1.
            self.version = 1;
        }

        if self.version < 2 {
            // Version 2 replaced a single boolean with a three-way choice, so
            // "always dark" became an explicit mode and "match the desktop"
            // became the default.
            self.version = 2;
        }

        if self.version < 3 {
            // Version 3 introduced the shared sound settings; a document from
            // before it keeps whatever its alarms carried.
            for alarm in &mut self.alarms {
                if alarm.snooze_minutes == 0 {
                    alarm.snooze_minutes = self.sound.snooze_minutes;
                }
            }
            self.version = 3;
        }

        self
    }

    /// Repairs values that are individually invalid.
    ///
    /// Applied after deserialization so that a hand-edited or partially written
    /// document cannot put the app into a state the UI has no way to express —
    /// an alarm at 07:99, a snooze of zero, a duplicate location.
    pub fn sanitised(mut self) -> Config {
        for alarm in &mut self.alarms {
            alarm.hour = alarm.hour.min(23);
            alarm.minute = alarm.minute.min(59);
            alarm.snooze_minutes = alarm.snooze_minutes.clamp(1, 60);
        }

        let mut seen = std::collections::HashSet::new();
        self.world
            .locations
            .retain(|location| !location.city.trim().is_empty() && seen.insert(location.id));

        let mut preset_ids = std::collections::HashSet::new();
        self.timers
            .presets
            .retain(|preset| preset_ids.insert(preset.id));
        for preset in &mut self.timers.presets {
            if preset.duration.is_zero() {
                preset.duration = std::time::Duration::from_secs(60);
            }
        }
        // Keep at least something to start a timer from.
        if self.timers.presets.is_empty() {
            self.timers.presets = TimerPreset::defaults();
        }

        self.sound.snooze_minutes = self.sound.snooze_minutes.clamp(1, 60);

        self
    }
}

/// The starting world clock: a few well-separated cities, ordered by offset.
fn default_locations() -> Vec<Location> {
    [
        (1u64, "New York", "America/New_York"),
        (2, "London", "Europe/London"),
        (3, "Tokyo", "Asia/Tokyo"),
    ]
    .into_iter()
    .filter_map(|(id, city, zone)| {
        crate::core::tz::parse(zone).map(|zone| Location::new(id, city, zone))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::theme::ThemeMode;
    use crate::domain::clock::HourFormat;
    use crate::domain::world::SortMode;
    use std::time::Duration;

    #[test]
    fn a_fresh_configuration_is_current() {
        let config = Config::first_run();
        assert_eq!(config.version, CURRENT_VERSION);
        assert_eq!(config.appearance.mode, ThemeMode::System);
        assert!(config.clock.show_seconds);
        assert!(config.alarms.is_empty(), "no alarms should be invented");
        assert_eq!(config.timers.presets.len(), 5);
        assert!(config.sound.enabled);
    }

    #[test]
    fn the_default_world_clock_uses_real_zones() {
        let world = Config::first_run().world;
        assert_eq!(world.locations.len(), 3);
        for location in &world.locations {
            assert!(
                location.tz().is_some(),
                "{} could not be resolved",
                location.zone
            );
        }
    }

    #[test]
    fn a_document_with_nothing_in_it_loads_as_defaults() {
        let config = serde_json::from_str::<Config>("{}").expect("deserializes");
        assert_eq!(config.version, 0, "no version field is version zero");
        assert_eq!(config.clock, ClockDisplay::default());
        assert_eq!(config.timers.presets.len(), 5);
        assert!(config.sound.enabled);
    }

    #[test]
    fn a_missing_preference_keeps_the_models_default_not_the_types() {
        // The regression this guards is subtle: reading `show_seconds` on its
        // own would fall back to `bool::default()`, which is `false`, silently
        // turning off a preference the model says should be on.
        let document: serde_json::Value =
            serde_json::from_str(r#"{"clock": {"show_analog": false}}"#).expect("parses");
        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader);

        assert!(
            config.clock.show_seconds,
            "an absent preference must keep the model's default"
        );
        assert!(
            !config.clock.show_analog,
            "a present preference is honoured"
        );
        assert_eq!(config.clock.hour_format, HourFormat::Twelve);
        assert!(loader.into_recoveries().is_empty(), "absence is not damage");
    }

    #[test]
    fn an_unreadable_preference_keeps_the_default_and_says_so() {
        let document: serde_json::Value = serde_json::from_str(
            r#"{"clock": {"show_seconds": "perhaps"}, "sound": {"enabled": 7}}"#,
        )
        .expect("parses");
        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader);
        let recoveries = loader.into_recoveries();

        assert!(config.clock.show_seconds, "the default survived");
        assert!(config.sound.enabled, "the other default survived too");
        assert_eq!(recoveries.len(), 2, "both are reported: {recoveries:?}");
        assert!(recoveries.iter().any(|r| r.path == "clock.show_seconds"));
        assert!(recoveries.iter().any(|r| r.path == "sound.enabled"));
    }

    #[test]
    fn a_collection_that_cannot_be_read_falls_back_to_the_built_ins() {
        let document: serde_json::Value =
            serde_json::from_str(r#"{"world": "somewhere"}"#).expect("parses");
        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader);

        assert!(config.world.locations.is_empty());
        assert_eq!(
            config.timers.presets.len(),
            5,
            "the presets come from default"
        );
    }

    #[test]
    fn a_document_with_only_some_sections_keeps_them() {
        let config: Config = serde_json::from_str(
            r#"{"clock": {"show_seconds": false, "hour_format": "TwentyFour"}}"#,
        )
        .expect("deserializes");
        assert!(!config.clock.show_seconds);
        assert_eq!(
            config.clock.hour_format,
            crate::domain::clock::HourFormat::TwentyFour
        );
        // Everything else still defaults.
        assert_eq!(config.timers.presets.len(), 5);
    }

    #[test]
    fn a_document_round_trips() {
        let config = Config::first_run();
        let json = serde_json::to_string(&config).expect("serializes");
        let restored: Config = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(restored, config);
    }

    #[test]
    fn a_versionless_document_migrates_to_current() {
        let config = serde_json::from_str::<Config>("{}").expect("deserializes");
        let migrated = config.migrate();
        assert_eq!(migrated.version, CURRENT_VERSION);
    }

    #[test]
    fn migrating_twice_changes_nothing_the_second_time() {
        let once = serde_json::from_str::<Config>("{}")
            .expect("deserializes")
            .migrate();
        let twice = once.clone().migrate();
        assert_eq!(once, twice, "migration must be idempotent");
    }

    #[test]
    fn a_future_version_is_left_alone() {
        // A document written by a newer build must not be silently rewritten
        // into something older.
        let config = Config {
            version: CURRENT_VERSION + 5,
            ..Config::first_run()
        };
        assert_eq!(config.clone().migrate().version, CURRENT_VERSION + 5);
    }

    #[test]
    fn the_snooze_migration_fills_in_zeroes() {
        let config = serde_json::from_str::<Config>(
            r#"{"alarms": [{"id": 1, "hour": 7, "minute": 0, "snooze_minutes": 0}]}"#,
        )
        .expect("deserializes")
        .migrate();

        assert_eq!(config.alarms[0].snooze_minutes, 9);
    }

    #[test]
    fn the_snooze_migration_leaves_real_values_alone() {
        let config = serde_json::from_str::<Config>(
            r#"{"alarms": [{"id": 1, "hour": 7, "minute": 0, "snooze_minutes": 3}]}"#,
        )
        .expect("deserializes")
        .migrate();

        assert_eq!(config.alarms[0].snooze_minutes, 3);
    }

    #[test]
    fn sanitising_clamps_impossible_alarm_times() {
        let config = serde_json::from_str::<Config>(
            r#"{"alarms": [{"id": 1, "hour": 99, "minute": 420, "snooze_minutes": 0}]}"#,
        )
        .expect("deserializes")
        .sanitised();

        assert_eq!(config.alarms[0].hour, 23);
        assert_eq!(config.alarms[0].minute, 59);
        assert_eq!(config.alarms[0].snooze_minutes, 1);
    }

    #[test]
    fn sanitising_drops_blank_and_duplicate_locations() {
        let config = serde_json::from_str::<Config>(
            r#"{"world": {"locations": [
                {"id": 1, "city": "Tokyo", "zone": "Asia/Tokyo"},
                {"id": 1, "city": "London", "zone": "Europe/London"},
                {"id": 2, "city": "   ", "zone": "Europe/Paris"},
                {"id": 3, "city": "Oslo", "zone": "Europe/Oslo"}
            ]}}"#,
        )
        .expect("deserializes")
        .sanitised();

        let names: Vec<&str> = config
            .world
            .locations
            .iter()
            .map(|l| l.city.as_str())
            .collect();
        assert_eq!(names, vec!["Tokyo", "Oslo"]);
    }

    #[test]
    fn sanitising_guarantees_at_least_one_timer_preset() {
        let config = serde_json::from_str::<Config>(r#"{"timers": {"presets": []}}"#)
            .expect("deserializes")
            .sanitised();
        assert_eq!(config.timers.presets.len(), 5);

        // And a preset with a zero duration becomes usable.
        let config = serde_json::from_str::<Config>(
            r#"{"timers": {"presets": [{"id": 1, "name": "Nothing", "duration": 0}]}}"#,
        )
        .expect("deserializes")
        .sanitised();
        assert_eq!(config.timers.presets[0].duration, Duration::from_secs(60));
    }

    #[test]
    fn sanitising_drops_duplicate_preset_ids() {
        let config = serde_json::from_str::<Config>(
            r#"{"timers": {"presets": [
                {"id": 1, "name": "A", "duration": 60},
                {"id": 1, "name": "B", "duration": 120}
            ]}}"#,
        )
        .expect("deserializes")
        .sanitised();

        assert_eq!(config.timers.presets.len(), 1);
        assert_eq!(config.timers.presets[0].name, "A");
    }

    #[test]
    fn sanitising_clamps_the_default_snooze() {
        let config = serde_json::from_str::<Config>(r#"{"sound": {"snooze_minutes": 200}}"#)
            .expect("deserializes")
            .sanitised();
        assert_eq!(config.sound.snooze_minutes, 60);
    }

    #[test]
    fn alarms_round_trip_through_the_configuration() {
        let mut config = Config::first_run();
        config.alarms.push(Alarm {
            id: 9,
            label: "Stand up".into(),
            hour: 9,
            minute: 30,
            enabled: true,
            repeat: crate::domain::alarm::Repeat::Daily,
            sound: Sound::Pulse,
            snooze_minutes: 4,
            last_fired: None,
            spent: false,
        });

        let json = serde_json::to_string(&config).expect("serializes");
        let restored: Config = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(restored.alarms.len(), 1);
        assert_eq!(restored.alarms[0].label, "Stand up");
        assert_eq!(restored.alarms[0].sound, Sound::Pulse);
    }

    #[test]
    fn the_full_pipeline_leaves_a_clean_document_alone() {
        let config = Config::first_run();
        let through = serde_json::to_string(&config).expect("serializes");
        let restored = crate::services::storage::Store::new("/nonexistent/oclock.json");
        let _ = restored;

        let document: serde_json::Value = serde_json::from_str(&through).expect("re-parses");
        let mut loader = crate::services::storage::Loader::new(&document);
        assert_eq!(Config::read(&mut loader).migrate().sanitised(), config);
        assert!(
            loader.into_recoveries().is_empty(),
            "a clean document must not report recoveries"
        );
    }

    #[test]
    fn one_bad_preference_does_not_cost_the_others() {
        // The whole point of reading field by field: an unreadable hour format
        // must not also cost the user their seconds and dial preferences.
        let document: serde_json::Value = serde_json::from_str(
            r#"{
                "clock": {
                    "hour_format": "BaseThirteen",
                    "show_seconds": false,
                    "show_analog": true
                },
                "world": { "sort": "Alphabetic" }
            }"#,
        )
        .expect("parses");

        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader);
        let recoveries = loader.into_recoveries();

        assert_eq!(recoveries.len(), 2, "one per bad field: {recoveries:?}");
        assert!(recoveries.iter().any(|r| r.path == "clock.hour_format"));
        assert!(recoveries.iter().any(|r| r.path == "world.sort"));

        // Everything else survives.
        assert!(!config.clock.show_seconds);
        assert!(config.clock.show_analog);
        assert_eq!(config.clock.hour_format, HourFormat::Twelve);
        assert_eq!(config.world.sort, SortMode::Manual);
    }

    #[test]
    fn a_saved_document_is_readable_as_plain_json() {
        // Someone editing the file by hand should see seconds, not a nested
        // `{secs, nanos}` object.
        let config = Config::first_run();
        let json = serde_json::to_string(&config).expect("serializes");
        assert!(
            json.contains(r#""duration":300"#),
            "durations should be plain seconds: {json}"
        );
    }

    #[test]
    fn a_stored_language_is_read_back() {
        let document = serde_json::json!({"language": "de"});
        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader);
        assert_eq!(config.language, Language::German);
    }

    #[test]
    fn a_missing_language_becomes_english() {
        // A document written before this preference existed has no `language`
        // key. English is the reference language, so that is what such a
        // document reads as — the desktop's locale is an ambient guess about
        // the machine, not something this document ever chose.
        let document = serde_json::json!({});
        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader);
        assert_eq!(config.language, Language::English, "English is the default");
    }

    #[test]
    fn a_fresh_installation_starts_in_english() {
        // Independent of `LANG`: the first run must not be decided by the
        // machine's locale, so this holds whatever the environment says.
        assert_eq!(Config::first_run().language, Language::English);
        assert_eq!(Config::first_run().language, Language::default());
    }

    #[test]
    fn a_stored_language_survives_a_restart() {
        // The other half of starting in English: a preference the user did make
        // is not overwritten by the default on the way back in.
        for language in Language::ALL {
            let stored = serde_json::json!({ "language": language.code() });
            let mut loader = crate::services::storage::Loader::new(&stored);
            assert_eq!(Config::read(&mut loader).language, language);
        }
    }

    #[test]
    fn an_existing_document_is_not_rewritten_by_the_default() {
        // `first_run` is only consulted when there is no document at all, so a
        // stored Persian preference must reach the application unchanged even
        // though a fresh install is English.
        let document = serde_json::json!({ "language": "fa" });
        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader).sanitised();
        assert_eq!(config.language, Language::Persian);
        assert!(
            loader.into_recoveries().is_empty(),
            "a preference that was understood is not a repair"
        );
    }

    #[test]
    fn the_language_round_trips_through_the_document() {
        for language in Language::ALL {
            let config = Config {
                language,
                ..Config::first_run()
            };
            let json = serde_json::to_string(&config).expect("serializes");
            assert!(
                json.contains(&format!("\"{}\"", language.code())),
                "{language:?} should be stored as its code: {json}"
            );
            let document: serde_json::Value = serde_json::from_str(&json).expect("parses");
            let mut loader = crate::services::storage::Loader::new(&document);
            assert_eq!(Config::read(&mut loader).language, language);
        }
    }

    #[test]
    fn an_unreadable_language_is_repaired_rather_than_fatal() {
        let document = serde_json::json!({"language": "klingon"});
        let mut loader = crate::services::storage::Loader::new(&document);
        let config = Config::read(&mut loader);
        assert!(Language::ALL.contains(&config.language));
        assert_eq!(
            config.language,
            Language::English,
            "an unknown language falls back rather than failing the launch"
        );
    }
}
