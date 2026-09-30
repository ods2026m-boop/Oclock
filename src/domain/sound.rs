//! The sounds OClock can play.
//!
//! The vocabulary lives in the domain layer so alarms and timers can name a
//! sound without depending on the audio backend; `services::audio` is what
//! actually synthesises them.

use serde::{Deserialize, Serialize};

/// An alarm or timer sound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sound {
    /// Three rising notes. The default, and the friendliest.
    #[default]
    Chime,
    /// A single smooth sweep, like a radar return.
    Radar,
    /// Three short pips. Cuts through a noisy room.
    Pulse,
    /// One struck bell with a long decay.
    Bell,
}

impl Sound {
    /// Every available sound, in menu order.
    pub const ALL: [Sound; 4] = [Sound::Chime, Sound::Radar, Sound::Pulse, Sound::Bell];

    /// How long the sound lasts, so the ringing UI can size itself.
    pub fn duration(self) -> std::time::Duration {
        match self {
            Sound::Chime => std::time::Duration::from_millis(1_450),
            Sound::Radar => std::time::Duration::from_millis(1_200),
            Sound::Pulse => std::time::Duration::from_millis(1_100),
            Sound::Bell => std::time::Duration::from_millis(1_800),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sound;

    #[test]
    fn every_sound_last_long_enough_to_hear() {
        // Naming and describing moved to the catalogue, which tests that every
        // language names every sound. What stays here is the domain's own
        // claim: a sound that rings for no time at all is not a sound.
        for sound in Sound::ALL {
            assert!(sound.duration() > std::time::Duration::ZERO);
        }
    }

    #[test]
    fn the_default_is_covered_by_the_menu_list() {
        assert!(Sound::ALL.contains(&Sound::default()));
    }

    #[test]
    fn sounds_round_trip_through_json() {
        for sound in Sound::ALL {
            let json = serde_json::to_string(&sound).expect("serializes");
            let restored: Sound = serde_json::from_str(&json).expect("deserializes");
            assert_eq!(sound, restored);
        }
    }

    #[test]
    fn an_unrecognised_serialised_sound_is_rejected_rather_than_guessed() {
        // A configuration naming a sound this build does not have must be
        // caught by the tolerant loader, which then uses the default; serde
        // itself refuses to invent one.
        assert!(serde_json::from_str::<Sound>("\"Siren\"").is_err());
    }
}
