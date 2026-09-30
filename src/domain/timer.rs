//! Countdown timers, measured with a monotonic clock.
//!
//! # Why there is no counter
//!
//! A timer that stores "seconds remaining" and decrements it on a tick is
//! wrong in three ways that all show up in real use: it loses accuracy every
//! time a tick is late, it cannot recover from a sleep/resume cycle, and it
//! reports a *stale* value while the process is busy. So a [`Timer`] stores
//! **monotonic timestamps** and derives everything:
//!
//! ```text
//! elapsed  = banked + (now - running_since)
//! ```
//!
//! `now` comes from a caller-supplied monotonic reading, so accuracy is
//! independent of how often the UI redraws, and [`Timer::catch_up`] folds in
//! time the process slept through.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::domain::sound::Sound;

/// Serialises a [`Duration`] as a whole number of seconds.
///
/// `std`'s own representation is `{ "secs": 60, "nanos": 0 }`, which is
/// accurate but unreadable. A configuration file a person might edit should
/// say `"duration": 60`, and second resolution is all a timer preset needs.
mod seconds {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(value.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        u64::deserialize(deserializer).map(Duration::from_secs)
    }
}

/// Which phase a timer is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerState {
    /// Created, or reset: not counting.
    #[default]
    Idle,
    /// Counting down.
    Running,
    /// Stopped part-way; the elapsed time is banked.
    Paused,
    /// Reached zero. Terminal until reset.
    Done,
}

impl TimerState {
    /// Whether the timer is counting.
    pub fn is_running(self) -> bool {
        matches!(self, TimerState::Running)
    }
}

/// Something a timer did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerEvent {
    /// The countdown reached zero.
    Completed,
}

/// A countdown timer.
#[derive(Clone, Debug, PartialEq)]
pub struct Timer {
    /// Stable identity.
    pub id: u64,
    /// User-facing name.
    pub label: String,
    /// The full duration being counted down.
    pub total: Duration,
    /// Elapsed time banked from previous run segments.
    banked: Duration,
    /// Monotonic reading when the current segment started.
    running_since: Option<Duration>,
    /// Current phase.
    pub state: TimerState,
    /// Completion sound.
    pub sound: Sound,
    /// Set once the completion event has been reported, so it reports once.
    announced: bool,
}

impl Timer {
    /// A timer counting down `total` from idle.
    pub fn new(id: u64, label: impl Into<String>, total: Duration) -> Timer {
        Timer {
            id,
            label: label.into(),
            total,
            banked: Duration::ZERO,
            running_since: None,
            state: TimerState::Idle,
            sound: Sound::default(),
            announced: false,
        }
    }

    /// Elapsed time as of the monotonic reading `now`.
    pub fn elapsed(&self, now: Duration) -> Duration {
        match self.running_since {
            Some(since) => {
                let running = now.saturating_sub(since);
                (self.banked + running).min(self.total)
            }
            None => self.banked,
        }
    }

    /// Time left as of `now`. Clamped at zero.
    pub fn remaining(&self, now: Duration) -> Duration {
        self.total.saturating_sub(self.elapsed(now))
    }

    /// Completion as a fraction, `0.0..=1.0`.
    pub fn progress(&self, now: Duration) -> f32 {
        if self.total.is_zero() {
            return 1.0;
        }
        (self.elapsed(now).as_secs_f64() / self.total.as_secs_f64()) as f32
    }

    /// Starts, or restarts after a pause.
    ///
    /// From `Idle` the banked time is discarded: pressing start on a fresh
    /// timer begins the full duration. From `Paused` it resumes where it left
    /// off.
    pub fn start(&mut self, now: Duration) {
        match self.state {
            TimerState::Paused => {
                self.running_since = Some(now);
                self.state = TimerState::Running;
            }
            TimerState::Done => {
                self.reset();
                self.running_since = Some(now);
                self.state = TimerState::Running;
            }
            _ => {
                // Idle: bank nothing, run the full duration.
                self.banked = Duration::ZERO;
                self.announced = false;
                self.running_since = Some(now);
                self.state = TimerState::Running;
            }
        }
    }

    /// Stops counting, keeping the elapsed time.
    pub fn pause(&mut self, now: Duration) {
        if self.state != TimerState::Running {
            return;
        }
        self.banked = self.elapsed(now);
        self.running_since = None;
        self.state = TimerState::Paused;
    }

    /// Clears all progress and returns to `Idle`.
    pub fn reset(&mut self) {
        self.banked = Duration::ZERO;
        self.running_since = None;
        self.announced = false;
        self.state = TimerState::Idle;
    }

    /// True once the countdown has reached zero.
    pub fn is_complete(&self, now: Duration) -> bool {
        matches!(self.state, TimerState::Done) || self.total <= self.elapsed(now)
    }

    /// Advances the timer to `now`, reporting completion exactly once.
    pub fn poll(&mut self, now: Duration) -> Option<TimerEvent> {
        if self.state != TimerState::Running {
            return None;
        }

        if self.elapsed(now) < self.total {
            return None;
        }

        self.banked = self.total;
        self.running_since = None;
        self.state = TimerState::Done;

        if self.announced {
            return None;
        }
        self.announced = true;
        Some(TimerEvent::Completed)
    }

    /// Folds in `elapsed` that the process did not observe.
    ///
    /// Called after a suspend/resume cycle. Only a running timer is advanced,
    /// and only up to its total, so waking from a long sleep finishes the
    /// timer rather than leaving it spinning past zero.
    pub fn catch_up(&mut self, elapsed: Duration) {
        if self.state != TimerState::Running {
            return;
        }
        self.banked = (self.banked + elapsed).min(self.total);
    }
}

/// A saved duration offered as a one-tap timer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimerPreset {
    /// Stable identity.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// How long the timer runs for.
    #[serde(with = "seconds")]
    pub duration: Duration,
    /// Whether it is offered in the preset strip.
    pub built_in: bool,
}

impl Default for TimerPreset {
    fn default() -> Self {
        TimerPreset {
            id: 0,
            name: "Timer".to_string(),
            duration: Duration::from_secs(300),
            built_in: false,
        }
    }
}

impl TimerPreset {
    /// A user preset.
    pub fn new(id: u64, name: impl Into<String>, duration: Duration) -> TimerPreset {
        TimerPreset {
            id,
            name: name.into(),
            duration,
            built_in: false,
        }
    }

    /// The presets every OClock installation starts with.
    pub fn defaults() -> Vec<TimerPreset> {
        let built_in = [
            ("1 minute", 60),
            ("5 minutes", 5 * 60),
            ("10 minutes", 10 * 60),
            ("25 minutes", 25 * 60),
            ("1 hour", 3600),
        ];

        built_in
            .into_iter()
            .enumerate()
            .map(|(index, (name, seconds))| TimerPreset {
                id: index as u64 + 1,
                name: name.to_string(),
                duration: Duration::from_secs(seconds),
                built_in: true,
            })
            .collect()
    }
}

/// A collection of running and ready timers.
#[derive(Clone, Debug, Default)]
pub struct TimerSet {
    timers: Vec<Timer>,
    presets: Vec<TimerPreset>,
    next_id: u64,
}

impl TimerSet {
    /// An empty set carrying the built-in presets.
    pub fn new(presets: Vec<TimerPreset>) -> TimerSet {
        // See `WorldClocks::new`: a stored document is untrusted input, so the
        // identifier after the largest in it is a ceiling, not a promise.
        let next_id = presets
            .iter()
            .map(|preset| preset.id)
            .max()
            .map_or(1, |highest| highest.saturating_add(1));
        TimerSet {
            timers: Vec::new(),
            presets,
            next_id,
        }
    }

    /// All timers, active ones first is *not* applied: the caller decides the
    /// presentation order, and the natural creation order reads best.
    pub fn timers(&self) -> &[Timer] {
        &self.timers
    }

    /// The saved presets.
    pub fn presets(&self) -> &[TimerPreset] {
        &self.presets
    }

    /// How many timers exist.
    pub fn len(&self) -> usize {
        self.timers.len()
    }

    /// True when no timers exist.
    pub fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// True when at least one timer is counting down.
    pub fn any_running(&self, now: Duration) -> bool {
        self.timers
            .iter()
            .any(|timer| timer.state.is_running() && !timer.is_complete(now))
    }

    /// Looks a timer up.
    pub fn get(&self, id: u64) -> Option<&Timer> {
        self.timers.iter().find(|timer| timer.id == id)
    }

    /// Mutable access to one timer.
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Timer> {
        self.timers.iter_mut().find(|timer| timer.id == id)
    }

    /// Creates a timer and returns its id.
    pub fn add(&mut self, label: impl Into<String>, total: Duration) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.timers.push(Timer::new(id, label, total));
        id
    }

    /// Replaces the preset list, keeping any id above the maximum so a new
    /// timer can never collide with a preset.
    pub fn set_presets(&mut self, presets: Vec<TimerPreset>) {
        self.next_id = self.next_id.max(
            presets
                .iter()
                .map(|preset| preset.id)
                .max()
                .map_or(1, |highest| highest.saturating_add(1)),
        );
        self.presets = presets;
    }

    /// Adds a preset, giving it the next id in this collection.
    ///
    /// The collection allocates rather than the caller: a preset id and a
    /// timer id are different namespaces, and a shared counter would let a new
    /// preset land on the id of a built-in one.
    pub fn add_preset(&mut self, name: impl Into<String>, duration: Duration) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.presets.push(TimerPreset::new(id, name, duration));
        id
    }

    /// Inserts or replaces a preset.
    pub fn upsert_preset(&mut self, preset: TimerPreset) {
        self.next_id = self.next_id.max(preset.id.saturating_add(1));
        match self
            .presets
            .iter_mut()
            .find(|existing| existing.id == preset.id)
        {
            Some(existing) => *existing = preset,
            None => self.presets.push(preset),
        }
    }

    /// Removes a user preset, leaving built-ins alone.
    pub fn remove_preset(&mut self, id: u64) -> bool {
        let before = self.presets.len();
        self.presets
            .retain(|preset| preset.built_in || preset.id != id);
        self.presets.len() != before
    }

    /// Removes a timer entirely.
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.timers.len();
        self.timers.retain(|timer| timer.id != id);
        self.timers.len() != before
    }

    /// Folds unobserved elapsed time into every running timer.
    pub fn catch_up(&mut self, elapsed: Duration) {
        for timer in &mut self.timers {
            timer.catch_up(elapsed);
        }
    }

    /// Advances every timer, returning the ones that completed.
    pub fn poll(&mut self, now: Duration) -> Vec<(u64, String, Sound)> {
        let mut finished = Vec::new();

        for timer in &mut self.timers {
            if timer.poll(now) == Some(TimerEvent::Completed) {
                finished.push((timer.id, timer.label.clone(), timer.sound));
            }
        }

        finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn timer() -> Timer {
        Timer::new(1, "Tea", Duration::from_secs(60))
    }

    #[test]
    fn a_fresh_timer_is_idle_and_has_its_full_duration_left() {
        let timer = timer();
        assert_eq!(timer.state, TimerState::Idle);
        assert_eq!(timer.remaining(ms(0)), Duration::from_secs(60));
        assert_eq!(timer.remaining(ms(59_000)), Duration::from_secs(60));
        assert_eq!(timer.elapsed(ms(5_000)), Duration::ZERO);
        assert_eq!(timer.progress(ms(5_000)), 0.0);
    }

    #[test]
    fn start_counts_down_from_the_monotonic_clock() {
        let mut timer = timer();
        timer.start(ms(1_000));

        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.elapsed(ms(1_000)), Duration::ZERO);
        assert_eq!(timer.remaining(ms(31_000)), Duration::from_secs(30));
        // A render stall changes nothing: elapsed comes from `now`, not from
        // the number of updates.
        assert_eq!(timer.remaining(ms(46_000)), Duration::from_secs(15));
    }

    #[test]
    fn pause_banks_progress_and_resume_continues_it() {
        let mut timer = timer();
        timer.start(ms(0));
        timer.pause(ms(20_000));

        assert_eq!(timer.state, TimerState::Paused);
        assert_eq!(timer.elapsed(ms(20_000)), Duration::from_secs(20));
        // Time passing while paused does not count.
        assert_eq!(timer.elapsed(ms(90_000)), Duration::from_secs(20));
        assert_eq!(timer.remaining(ms(90_000)), Duration::from_secs(40));

        timer.start(ms(90_000));
        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.elapsed(ms(100_000)), Duration::from_secs(30));
    }

    #[test]
    fn a_paused_timer_ignores_stray_pause_calls() {
        let mut timer = timer();
        timer.pause(ms(5_000));
        assert_eq!(timer.state, TimerState::Idle);
        assert_eq!(timer.elapsed(ms(5_000)), Duration::ZERO);
    }

    #[test]
    fn reset_clears_progress() {
        let mut timer = timer();
        timer.start(ms(0));
        timer.pause(ms(30_000));
        timer.reset();

        assert_eq!(timer.state, TimerState::Idle);
        assert_eq!(timer.elapsed(ms(300_000)), Duration::ZERO);
        assert_eq!(timer.remaining(ms(300_000)), Duration::from_secs(60));
    }

    #[test]
    fn completion_is_detected_exactly_once() {
        let mut timer = timer();
        timer.start(ms(0));

        assert_eq!(timer.poll(ms(59_999)), None);
        assert_eq!(timer.poll(ms(60_000)), Some(TimerEvent::Completed));
        assert_eq!(timer.state, TimerState::Done);
        // Polling again must not re-announce.
        assert_eq!(timer.poll(ms(90_000)), None);
    }

    #[test]
    fn a_finished_timer_reads_zero_remaining() {
        let mut timer = timer();
        timer.start(ms(0));
        timer.poll(ms(70_000));
        assert_eq!(timer.remaining(ms(120_000)), Duration::ZERO);
        assert_eq!(timer.elapsed(ms(120_000)), Duration::from_secs(60));
        assert_eq!(timer.progress(ms(120_000)), 1.0);
    }

    #[test]
    fn starting_a_finished_timer_runs_it_again() {
        let mut timer = timer();
        timer.start(ms(0));
        timer.poll(ms(60_000));
        assert_eq!(timer.state, TimerState::Done);

        timer.start(ms(60_000));
        assert_eq!(timer.state, TimerState::Running);
        assert_eq!(timer.elapsed(ms(60_000)), Duration::ZERO);
        assert_eq!(timer.remaining(ms(60_000)), Duration::from_secs(60));
        // And it can complete a second time.
        assert_eq!(timer.poll(ms(120_000)), Some(TimerEvent::Completed));
    }

    #[test]
    fn a_very_stalled_render_still_completes_accurately() {
        let mut timer = Timer::new(1, "Tea", ms(100));
        timer.start(ms(0));
        // The process was frozen for a full minute.
        assert_eq!(timer.poll(ms(60_000)), Some(TimerEvent::Completed));
        assert_eq!(timer.elapsed(ms(60_000)), ms(100));
    }

    #[test]
    fn a_zero_length_timer_completes_immediately() {
        let mut timer = Timer::new(1, "Now", Duration::ZERO);
        timer.start(ms(0));
        assert_eq!(timer.poll(ms(0)), Some(TimerEvent::Completed));
        assert_eq!(timer.progress(ms(0)), 1.0);
    }

    #[test]
    fn catch_up_folds_in_unobserved_time() {
        let mut timer = timer();
        timer.start(ms(0));
        // Woke from 20 minutes of sleep: the timer is long finished.
        timer.catch_up(Duration::from_secs(1_200));
        assert_eq!(timer.elapsed(ms(1_000)), Duration::from_secs(60));
        assert_eq!(timer.poll(ms(1_000)), Some(TimerEvent::Completed));
    }

    #[test]
    fn catch_up_does_nothing_to_a_paused_timer() {
        let mut timer = timer();
        timer.start(ms(0));
        timer.pause(ms(10_000));
        timer.catch_up(Duration::from_secs(600));
        assert_eq!(timer.elapsed(ms(600_000)), Duration::from_secs(10));
    }

    #[test]
    fn catch_up_preserves_a_live_measurement() {
        let mut timer = timer();
        timer.start(ms(0));
        // Ten seconds observed, then ten more while suspended.
        assert_eq!(timer.elapsed(ms(10_000)), Duration::from_secs(10));
        timer.catch_up(Duration::from_secs(10));
        assert_eq!(timer.elapsed(ms(10_000)), Duration::from_secs(20));
    }

    #[test]
    fn a_timer_is_running_only_when_counting() {
        // The state names moved to the catalogue; the question of which states
        // count as running is the domain's, and it is unchanged.
        assert!(TimerState::Running.is_running());
        assert!(!TimerState::Paused.is_running());
        assert!(!TimerState::Idle.is_running());
        assert!(!TimerState::Done.is_running());
    }

    #[test]
    fn a_set_manages_several_timers_independently() {
        let mut set = TimerSet::new(TimerPreset::defaults());
        let tea = set.add("Tea", Duration::from_secs(60));
        let coffee = set.add("Coffee", Duration::from_secs(600));

        set.get_mut(tea).unwrap().start(ms(0));
        set.get_mut(coffee).unwrap().start(ms(0));

        let finished = set.poll(ms(61_000));
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].0, tea);
        assert_eq!(finished[0].1, "Tea");

        assert_eq!(set.get(tea).unwrap().state, TimerState::Done);
        assert_eq!(set.get(coffee).unwrap().state, TimerState::Running);
        assert!(set.any_running(ms(61_000)));
    }

    #[test]
    fn a_set_catch_up_reaches_every_running_timer() {
        let mut set = TimerSet::new(Vec::new());
        let a = set.add("A", Duration::from_secs(60));
        let b = set.add("B", Duration::from_secs(60));
        set.get_mut(a).unwrap().start(ms(0));
        set.get_mut(b).unwrap().start(ms(0));

        set.catch_up(Duration::from_secs(30));
        assert_eq!(set.get(a).unwrap().elapsed(ms(0)), Duration::from_secs(30));
        assert_eq!(set.get(b).unwrap().elapsed(ms(0)), Duration::from_secs(30));
    }

    #[test]
    fn timer_ids_never_collide_with_preset_ids() {
        let mut set = TimerSet::new(TimerPreset::defaults());
        let first = set.add("Custom", Duration::from_secs(90));
        assert!(set.get(first).is_some());

        // Loading a preset list that includes higher ids keeps the allocator
        // ahead of them.
        set.set_presets(vec![TimerPreset::new(
            1_000,
            "Long",
            Duration::from_secs(3600),
        )]);
        let second = set.add("Another", Duration::from_secs(90));
        assert!(second > 1_000);
    }

    #[test]
    fn removing_timers_and_presets() {
        let mut set = TimerSet::new(TimerPreset::defaults());
        let id = set.add("Tea", Duration::from_secs(60));
        assert!(set.remove(id));
        assert!(!set.remove(id));
        assert!(set.is_empty());

        let custom = set.presets()[0].id;
        assert!(!set.remove_preset(custom), "built-ins cannot be removed");

        set.upsert_preset(TimerPreset::new(custom, "Mine", Duration::from_secs(30)));
        assert!(set.remove_preset(custom));
    }

    #[test]
    fn default_presets_are_offered_in_increasing_order() {
        let presets = TimerPreset::defaults();
        assert_eq!(presets.len(), 5);
        assert!(presets.iter().all(|preset| preset.built_in));
        let durations: Vec<u64> = presets.iter().map(|p| p.duration.as_secs()).collect();
        assert_eq!(durations, vec![60, 300, 600, 1500, 3600]);
    }

    #[test]
    fn presets_round_trip_through_json() {
        let presets = TimerPreset::defaults();
        let json = serde_json::to_string(&presets).expect("serializes");
        let restored: Vec<TimerPreset> = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(restored, presets);
    }

    #[test]
    fn a_new_preset_cannot_land_on_a_built_in_id() {
        let mut set = TimerSet::new(TimerPreset::defaults());
        let built_in: Vec<u64> = set.presets().iter().map(|preset| preset.id).collect();

        let first = set.add_preset("Tea", Duration::from_secs(60));
        let second = set.add_preset("Coffee", Duration::from_secs(60));

        assert!(
            !built_in.contains(&first),
            "the new preset replaced a built-in"
        );
        assert!(!built_in.contains(&second));
        assert_ne!(first, second);
        assert_eq!(set.presets().len(), built_in.len() + 2);
    }

    #[test]
    fn a_preset_with_missing_fields_gets_defaults() {
        let restored: TimerPreset =
            serde_json::from_str(r#"{"id":3,"name":"Egg"}"#).expect("deserializes");
        assert_eq!(restored.id, 3);
        assert_eq!(restored.name, "Egg");
        assert_eq!(restored.duration, Duration::from_secs(300));
        assert!(!restored.built_in);
    }
}
