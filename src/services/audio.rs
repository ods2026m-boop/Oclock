//! Alarm and timer sounds.
//!
//! The sounds are **synthesised**, not sampled: each one is a small recipe of
//! tones and an envelope, rendered to PCM on demand. That keeps the
//! application free of binary assets, makes every sound reproducible on every
//! machine, and means the synthesis itself can be unit tested — a property a
//! bundled `.wav` could never have.
//!
//! Everything the application asks for goes through the [`SoundPlayer`]
//! trait, so a machine with no audio device simply falls back to
//! [`SilentPlayer`] and everything else carries on.

use crate::domain::sound::Sound;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A synthesiser configuration: 44.1 kHz, signed 16-bit, stereo.
///
/// A modest, universally supported format. The sample rate is fixed because
/// the sound recipes are written in musical terms, and letting the output
/// device resample is the audio layer's job.
const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: u16 = 2;

/// Something that can play a sound.
pub trait SoundPlayer: Send {
    /// Starts `sound`, replacing anything already playing.
    ///
    /// Re-entrant by design: ringing a second alarm restarts the sound rather
    /// than queueing it, which is what a user expects when two alarms collide.
    fn play(&self, sound: Sound);

    /// Stops whatever is playing.
    fn stop(&self);

    /// Turns all playback on or off.
    fn set_enabled(&self, enabled: bool);

    /// Whether playback is currently allowed.
    fn is_enabled(&self) -> bool;
}

/// One tone: a frequency and how long it sounds for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tone {
    /// Hz.
    frequency: f32,
    /// Seconds.
    seconds: f32,
    /// 0.0..1.0, scaled against the sound's peak level.
    level: f32,
}

impl Tone {
    const fn new(frequency: f32, seconds: f32, level: f32) -> Tone {
        Tone {
            frequency,
            seconds,
            level,
        }
    }

    /// A tone written as a note name such as `A4`, `C#5` or `Eb3`.
    fn note(name: &str, seconds: f32, level: f32) -> Tone {
        Tone {
            frequency: frequency_of(name),
            seconds,
            level,
        }
    }
}

/// A note name as a frequency in Hz.
///
/// Only sharps are handled, and only the handful of notes the sounds use, so
/// this is arithmetic rather than a table. A4 is 440 Hz and the scale is equal
/// temperament.
pub fn frequency_of(name: &str) -> f32 {
    // `A4` is a letter, an optional accidental, then a one- or two-digit
    // octave. Reading from the front avoids having to know the octave width.
    let mut characters = name.chars();
    let letter = characters.next().expect("a note name has a letter");
    let (accidental, octave) = match characters.clone().next() {
        Some(mark @ ('#' | 'b')) => {
            characters.next();
            (mark, characters.as_str())
        }
        _ => ('0', &name[letter.len_utf8()..]),
    };

    let semitone = match (letter, accidental) {
        ('C', '#') => 1,
        ('C', _) => 0,
        ('D', '#') => 3,
        ('D', _) => 2,
        ('E', '#') => 4,
        ('E', _) => 4,
        ('F', '#') => 6,
        ('F', _) => 5,
        ('G', '#') => 8,
        ('G', _) => 7,
        ('A', '#') => 10,
        ('A', _) => 9,
        ('B', '#') => 0,
        ('B', _) => 11,
        _ => 0,
    };
    let octave: i32 = octave.parse().expect("a valid octave");
    440.0 * 2f32.powf((semitone - 9 + (octave - 4) * 12) as f32 / 12.0)
}

/// A whole sound: a short phrase of tones plus how the phrase decays.
#[derive(Clone, Debug, PartialEq)]
struct Recipe {
    tones: Vec<Tone>,
    /// Gap before the first tone, in seconds.
    lead_in: f32,
    /// Decay applied across the whole phrase, in seconds. Zero means none.
    decay: f32,
    /// 0.0..1.0, to keep a loud recipe from clipping.
    peak: f32,
}

/// The recipes behind [`Sound`].
///
/// Pure data, so `render` is a pure function of `(recipe, sample rate)`.
fn recipe(sound: Sound) -> Recipe {
    match sound {
        // A major triad, rolled upwards with a beat between the notes.
        Sound::Chime => Recipe {
            tones: vec![
                Tone::note("E5", 0.55, 0.9),
                Tone::note("G5", 0.55, 0.85),
                Tone::note("B5", 0.85, 1.0),
            ],
            lead_in: 0.0,
            decay: 0.0,
            peak: 0.55,
        },
        Sound::Radar => Recipe {
            tones: vec![Tone::new(0.0, 1.0, 0.8)],
            lead_in: 0.0,
            decay: 0.0,
            peak: 0.5,
        },
        Sound::Pulse => Recipe {
            tones: vec![
                Tone::new(880.0, 0.09, 0.9),
                Tone::new(880.0, 0.09, 0.9),
                Tone::new(1174.0, 0.16, 0.9),
            ],
            lead_in: 0.0,
            decay: 0.0,
            peak: 0.5,
        },
        Sound::Bell => Recipe {
            tones: vec![Tone::new(523.25, 1.4, 0.8)],
            lead_in: 0.0,
            decay: 1.1,
            peak: 0.6,
        },
    }
}

/// Renders a sound to interleaved stereo 16-bit samples.
///
/// `sample_rate` is a parameter rather than a constant so the renderer can be
/// tested at a rate that keeps the test fast, and so the same recipe works on
/// any output device.
pub fn render(sound: Sound, sample_rate: u32) -> Vec<i16> {
    let recipe = recipe(sound);
    let frames = ((sound.duration().as_secs_f32()) * sample_rate as f32) as usize;
    let mut buffer = vec![0i16; frames * CHANNELS as usize];

    for tone in &recipe.tones {
        // A sweep is expressed as a tone whose start and end frequencies are
        // both zero, which no real note uses.
        let (from, to) = if tone.frequency == 0.0 {
            (220.0f32, 1400.0f32)
        } else {
            (tone.frequency, tone.frequency)
        };
        let tone_frames = (tone.seconds * sample_rate as f32) as usize;
        let start = (recipe.lead_in * sample_rate as f32) as usize;

        for frame in 0..tone_frames {
            let index = start + frame;
            if index >= frames {
                break;
            }

            let position = frame as f32 / tone_frames.max(1) as f32;
            // Logarithmic sweep, which is what a rising sweep actually sounds
            // like; a linear one crawls at the start.
            let frequency = from * (to / from).powf(position);
            let phase =
                2.0 * std::f32::consts::PI * frequency * (frame as f32 / sample_rate as f32);
            // A pure sine is thin and easily masked by ambient noise; adding
            // the third harmonic gives it presence without a sample to play.
            let value = (phase.sin() + 0.28 * (3.0 * phase).sin()) / 1.28;

            // A short attack so the note does not start with a click, and a
            // longer release so it does not end with one.
            let attack = (position / 0.004).min(1.0);
            let release = ((1.0 - position) / 0.09).min(1.0);
            let envelope = attack.min(release).max(0.0);
            let sample = value * envelope * tone.level * recipe.peak;
            let clamped =
                (sample * i16::MAX as f32).clamp(-i16::MAX as f32, i16::MAX as f32) as i16;

            buffer[index * 2] = clamped;
            buffer[index * 2 + 1] = clamped;
        }
    }

    if recipe.decay > 0.0 {
        apply_decay(&mut buffer, recipe.decay, sample_rate);
    }

    // A short fade over the whole buffer so the sound never ends on a click.
    fade_edges(&mut buffer, (0.01 * sample_rate as f32) as usize);
    buffer
}

/// Applies a single long exponential decay across the whole buffer, the
/// characteristic of a struck bell.
fn apply_decay(buffer: &mut [i16], seconds: f32, sample_rate: u32) {
    let frames = buffer.len() / CHANNELS as usize;
    for frame in 0..frames {
        let position = frame as f32 / sample_rate as f32;
        let gain = (-position / seconds).exp();
        for channel in 0..CHANNELS as usize {
            let index = frame * CHANNELS as usize + channel;
            buffer[index] = (buffer[index] as f32 * gain) as i16;
        }
    }
}

fn fade_edges(buffer: &mut [i16], frames: usize) {
    let total = buffer.len() / CHANNELS as usize;
    let frames = frames.min(total / 2).max(1);
    for frame in 0..total {
        let gain = if frame < frames {
            frame as f32 / frames as f32
        } else if frame >= total - frames {
            (total - 1 - frame) as f32 / frames as f32
        } else {
            1.0
        };
        for channel in 0..CHANNELS as usize {
            let index = frame * CHANNELS as usize + channel;
            buffer[index] = (buffer[index] as f32 * gain) as i16;
        }
    }
}

/// The PCM specification every rendered buffer conforms to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for Format {
    fn default() -> Self {
        Format {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
        }
    }
}

/// A player that renders and plays through the system output.
pub struct SystemPlayer {
    queue: Arc<Mutex<VecDeque<Vec<i16>>>>,
    enabled: Arc<AtomicBool>,
    format: Mutex<Format>,
    /// Set once the output stream has been attempted.
    started: Arc<AtomicBool>,
}

impl SystemPlayer {
    /// Creates a player, opening the audio device on first use.
    pub fn new() -> SystemPlayer {
        SystemPlayer {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            enabled: Arc::new(AtomicBool::new(true)),
            format: Mutex::new(Format::default()),
            started: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The PCM format rendered buffers use.
    ///
    /// Read before any sound has played this is the default; afterwards it is
    /// whatever the output device settled on.
    pub fn format(&self) -> Format {
        *self.format.lock().expect("audio format")
    }

    /// Renders `sound` and hands it to the output device.
    ///
    /// Returns `false` when no device could be opened, which is a normal
    /// outcome on a machine with no sound hardware and must not be fatal.
    pub fn push(&self, sound: Sound) -> bool {
        if !self.enabled.load(Ordering::Relaxed) {
            return true;
        }

        let samples = render(sound, self.format().sample_rate);
        if samples.is_empty() {
            return false;
        }

        if !self.ensure_stream() {
            return false;
        }

        // A new sound replaces the one playing: a second alarm must be heard
        // now, not after the first one finishes.
        let mut queue = self.queue.lock().expect("audio queue");
        queue.clear();
        queue.push_back(samples);
        true
    }

    /// Opens the output stream, once.
    ///
    /// Every failure path is reported to stderr and returns `false`: a machine
    /// with no sound hardware, a busy device or a backend that refuses float
    /// samples must not stop the application from running, and the user
    /// already has the in-window presentation.
    fn ensure_stream(&self) -> bool {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        if self.started.swap(true, Ordering::SeqCst) {
            return true;
        }

        let host = cpal::default_host();
        let Some(device) = host.default_output_device() else {
            eprintln!("OClock: no audio output device; sounds are disabled");
            return false;
        };
        let Ok(default) = device.default_output_config() else {
            eprintln!("OClock: the audio output device reported no configurations");
            return false;
        };

        // Float samples are what every Linux audio stack produces natively, and
        // asking for them keeps the callback free of format conversion. When
        // the device's own default is not float, the same rate and channel
        // count are requested in float instead.
        let config = if default.sample_format() == cpal::SampleFormat::F32 {
            default
        } else {
            let wanted = default.sample_rate();
            let channels = default.channels();
            let float = device.supported_output_configs().ok().and_then(|configs| {
                configs
                    .filter(|range| {
                        range.channels() == channels
                            && range.sample_format() == cpal::SampleFormat::F32
                    })
                    .map(|range| range.with_sample_rate(wanted))
                    .next()
            });

            match float {
                Some(config) => config,
                None => {
                    eprintln!(
                        "OClock: the audio output device has no float configuration; \
                         sounds are disabled"
                    );
                    return false;
                }
            }
        };

        self.format.lock().expect("audio format").sample_rate = config.sample_rate().0;
        let queue = Arc::clone(&self.queue);
        let stream = device.build_output_stream(
            &config.config(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut queue = queue.lock().expect("audio queue");

                // Copy as much of the sound as this buffer can take, then
                // consume exactly that much from the queue.
                let filled = match queue.front_mut() {
                    Some(front) => {
                        let taken = front.len().min(data.len());
                        for (out, raw) in data.iter_mut().zip(front.iter()) {
                            *out = f32::from(*raw) / f32::from(i16::MAX);
                        }
                        taken
                    }
                    None => 0,
                };

                if filled == 0 {
                    data.fill(0.0);
                    return;
                }

                let finished = {
                    let front = queue.front().expect("just borrowed");
                    front.len() <= filled
                };
                if finished {
                    queue.pop_front();
                } else if let Some(front) = queue.front_mut() {
                    front.drain(..filled);
                }

                for out in data.iter_mut().skip(filled) {
                    *out = 0.0;
                }
            },
            |error| eprintln!("OClock: audio stream error: {error}"),
            None,
        );

        match stream {
            Ok(stream) => match stream.play() {
                Ok(_) => true,
                Err(error) => {
                    eprintln!("OClock: could not start audio playback: {error}");
                    false
                }
            },
            Err(error) => {
                eprintln!("OClock: could not open the audio output: {error}");
                false
            }
        }
    }
}

impl Default for SystemPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundPlayer for SystemPlayer {
    fn play(&self, sound: Sound) {
        self.push(sound);
    }

    fn stop(&self) {
        self.queue.lock().expect("audio queue").clear();
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            self.stop();
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

/// A player that records requests instead of making noise.
pub struct SilentPlayer {
    enabled: AtomicBool,
    last: Mutex<Option<Sound>>,
}

impl Default for SilentPlayer {
    fn default() -> Self {
        SilentPlayer {
            enabled: AtomicBool::new(true),
            last: Mutex::new(None),
        }
    }
}

impl SilentPlayer {
    /// The most recently requested sound.
    pub fn last(&self) -> Option<Sound> {
        *self.last.lock().expect("lock")
    }
}

impl SoundPlayer for SilentPlayer {
    fn play(&self, sound: Sound) {
        if self.enabled.load(Ordering::Relaxed) {
            *self.last.lock().expect("lock") = Some(sound);
        }
    }

    fn stop(&self) {}

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            *self.last.lock().expect("lock") = None;
        }
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

/// A player that remembers everything, for tests.
pub struct ScriptedPlayer {
    played: Mutex<Vec<Sound>>,
    enabled: AtomicBool,
    stops: Mutex<usize>,
}

impl Default for ScriptedPlayer {
    fn default() -> Self {
        ScriptedPlayer {
            played: Mutex::new(Vec::new()),
            // Playback starts on, matching a real player's default.
            enabled: AtomicBool::new(true),
            stops: Mutex::new(0),
        }
    }
}

impl ScriptedPlayer {
    /// Everything played, in order.
    pub fn played(&self) -> Vec<Sound> {
        self.played.lock().expect("lock").clone()
    }

    /// How many times playback was stopped.
    pub fn stops(&self) -> usize {
        *self.stops.lock().expect("lock")
    }
}

impl SoundPlayer for ScriptedPlayer {
    fn play(&self, sound: Sound) {
        if self.enabled.load(Ordering::Relaxed) {
            self.played.lock().expect("lock").push(sound);
        }
    }

    fn stop(&self) {
        *self.stops.lock().expect("lock") += 1;
    }

    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

/// The recipe's total length, used to check a render matches its sound.
#[cfg(test)]
fn expected_frames(sound: Sound, sample_rate: u32) -> usize {
    (sound.duration().as_secs_f32() * sample_rate as f32) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A low rate keeps the render tests fast while still exercising the maths.
    const TEST_RATE: u32 = 8_000;

    fn peak(buffer: &[i16]) -> i16 {
        buffer
            .iter()
            .map(|sample| sample.unsigned_abs() as i32)
            .max()
            .unwrap_or(0) as i16
    }

    fn rms(buffer: &[i16]) -> f64 {
        if buffer.is_empty() {
            return 0.0;
        }
        let sum: f64 = buffer
            .iter()
            .map(|sample| {
                let value = f64::from(*sample) / f64::from(i16::MAX);
                value * value
            })
            .sum();
        (sum / buffer.len() as f64).sqrt()
    }

    fn first_nonzero(buffer: &[i16]) -> Option<usize> {
        buffer.iter().position(|sample| *sample != 0)
    }

    #[test]
    fn note_names_become_the_right_frequencies() {
        let a440 = 440.0;
        assert!((frequency_of("A4") - a440).abs() < 0.01);
        assert!((frequency_of("A5") - a440 * 2.0).abs() < 0.01);
        assert!((frequency_of("A3") - a440 / 2.0).abs() < 0.01);
        // A semitone up from A4.
        assert!((frequency_of("A#4") - a440 * 2f32.powf(1.0 / 12.0)).abs() < 0.01);
        // Equal temperament puts these where a musician expects them.
        assert!((frequency_of("E5") - 659.255).abs() < 0.5);
        assert!((frequency_of("G5") - 783.991).abs() < 0.5);
        assert!((frequency_of("B5") - 987.767).abs() < 0.5);
        // The octave closes the loop exactly.
        assert!((frequency_of("A4") - frequency_of("A3") * 2.0).abs() < 0.01);
    }

    #[test]
    fn a_note_tone_carries_its_frequency_and_shape() {
        let tone = Tone::note("A4", 0.5, 0.8);
        assert!((tone.frequency - 440.0).abs() < 0.01);
        assert_eq!(tone.seconds, 0.5);
        assert_eq!(tone.level, 0.8);
    }

    #[test]
    fn every_sound_renders_to_the_expected_length() {
        for sound in Sound::ALL {
            let buffer = render(sound, TEST_RATE);
            assert_eq!(
                buffer.len() / CHANNELS as usize,
                expected_frames(sound, TEST_RATE),
                "{sound:?} rendered the wrong number of frames"
            );
        }
    }

    #[test]
    fn every_sound_is_stereo_interleaved() {
        for sound in Sound::ALL {
            let buffer = render(sound, TEST_RATE);
            assert_eq!(buffer.len() % CHANNELS as usize, 0);
            for frame in buffer.chunks(CHANNELS as usize) {
                assert_eq!(frame[0], frame[1], "{sound:?} is not centred");
            }
        }
    }

    #[test]
    fn every_sound_actually_makes_a_sound() {
        for sound in Sound::ALL {
            let buffer = render(sound, TEST_RATE);
            assert!(peak(&buffer) > 1_000, "{sound:?} was silent");
            assert!(rms(&buffer) > 0.01, "{sound:?} was too quiet");
        }
    }

    #[test]
    fn no_sound_clips() {
        for sound in Sound::ALL {
            let buffer = render(sound, TEST_RATE);
            assert!(peak(&buffer) < i16::MAX, "{sound:?} clipped");
        }
    }

    #[test]
    fn every_sound_starts_and_ends_in_silence() {
        for sound in Sound::ALL {
            let buffer = render(sound, TEST_RATE);
            assert_eq!(buffer[0], 0, "{sound:?} started with a click");
            assert_eq!(buffer[buffer.len() - 1], 0, "{sound:?} ended with a click");
        }
    }

    #[test]
    fn a_sound_begins_very_shortly_after_the_start() {
        for sound in Sound::ALL {
            let buffer = render(sound, TEST_RATE);
            let onset = first_nonzero(&buffer).expect("some sound");
            assert!(
                onset < TEST_RATE as usize / 20,
                "{sound:?} took {onset} samples to start"
            );
        }
    }

    #[test]
    fn sounds_are_distinct_from_one_another() {
        // Two different recipes must not accidentally render identically.
        let a = render(Sound::Chime, TEST_RATE);
        let b = render(Sound::Bell, TEST_RATE);
        assert_ne!(a, b);
    }

    #[test]
    fn the_bell_decays() {
        let buffer = render(Sound::Bell, TEST_RATE);
        let frames = buffer.len() / CHANNELS as usize;
        let first = rms(&buffer[..frames / 4 * CHANNELS as usize]);
        let last = rms(&buffer[frames * 3 / 4 * CHANNELS as usize..]);
        assert!(first > 0.0);
        assert!(last < first, "a struck bell must get quieter");
    }

    #[test]
    fn rendering_is_deterministic() {
        assert_eq!(
            render(Sound::Chime, TEST_RATE),
            render(Sound::Chime, TEST_RATE)
        );
    }

    #[test]
    fn a_different_sample_rate_changes_the_length_but_not_the_sound() {
        let low = render(Sound::Pulse, 8_000);
        let high = render(Sound::Pulse, 16_000);
        assert_eq!(high.len(), low.len() * 2);
        assert!(peak(&high) > 0 && peak(&low) > 0);
    }

    #[test]
    fn the_system_player_reports_its_format() {
        let player = SystemPlayer::new();
        let format = player.format();
        assert_eq!(format.channels, CHANNELS);
        assert!(format.sample_rate >= 8_000);
    }

    #[test]
    fn the_silent_player_records_the_last_request() {
        let player = SilentPlayer::default();
        player.play(Sound::Radar);
        assert_eq!(player.last(), Some(Sound::Radar));

        player.set_enabled(false);
        assert!(!player.is_enabled());
        assert_eq!(player.last(), None, "turning playback off clears it");
        player.play(Sound::Bell);
        assert_eq!(player.last(), None, "nothing plays when disabled");
    }

    #[test]
    fn a_scripted_player_records_everything() {
        let player = ScriptedPlayer::default();
        player.play(Sound::Chime);
        player.play(Sound::Pulse);
        player.stop();
        assert_eq!(player.played(), vec![Sound::Chime, Sound::Pulse]);
        assert_eq!(player.stops(), 1);

        player.set_enabled(false);
        player.play(Sound::Bell);
        assert_eq!(
            player.played().len(),
            2,
            "disabled playback records nothing"
        );
    }

    #[test]
    fn the_duration_a_sound_reports_matches_what_it_renders() {
        // The domain declares how long each sound lasts; the renderer has to
        // agree, or a ringing alarm would be cut off.
        for sound in Sound::ALL {
            let buffer = render(sound, SAMPLE_RATE);
            let seconds = buffer.len() as f32 / CHANNELS as usize as f32 / SAMPLE_RATE as f32;
            assert!(
                (seconds - sound.duration().as_secs_f32()).abs() < 0.05,
                "{sound:?} declared {:?} but rendered {seconds}s",
                sound.duration()
            );
        }
    }
}
