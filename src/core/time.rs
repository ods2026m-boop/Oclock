//! Wall-clock and monotonic time sources.
//!
//! OClock keeps two clocks apart on purpose:
//!
//! * **Wall time** ([`utc_now`]) is the civil calendar. Everything a human
//!   reads — the current time, the date, when an alarm is due — comes from
//!   it, because that is what "7:30 tomorrow" means.
//! * **Monotonic time** ([`monotonic`]) only ever moves forward and is
//!   unaffected by clock changes. Every *duration* — timer elapsed time,
//!   stopwatch laps — is derived from it, so neither can be distorted by an
//!   NTP correction or by a user changing the system clock.
//!
//! [`Watchdog`] compares the two so the application can notice when reality
//! and the clock disagree: a system clock jump, or a suspend/resume cycle.
//!
//! # What a disagreement means
//!
//! Between two samples both clocks should have advanced by the same amount, so
//! the *difference* between the two deltas is the stretch neither clock's
//! ordinary reading explains:
//!
//! * the wall clock ran **ahead** of the monotonic one — the machine slept
//!   (`CLOCK_MONOTONIC` stops while the machine sleeps, the wall clock does
//!   not), or the clock was stepped forward. Either way the application did not
//!   observe that stretch, and a measurement has to be advanced by it, or it
//!   will read short by exactly that much;
//! * the wall clock fell **behind** — the clock was set back, or an NTP
//!   correction landed. Nothing was missed and nothing elapsed as far as the
//!   measurements are concerned: a duration must neither grow nor shrink
//!   because the calendar moved underneath it.
//!
//! The amount reported for the first case is the *excess*, `wall - monotonic`,
//! and never the whole wall delta. Handing the whole delta to a measurement
//! that has already been advancing on the monotonic clock counts the observed
//! stretch a second time, which is how eleven seconds of wall time used to read
//! as twelve.

use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

/// Absolute civil time, straight from the system clock.
pub fn utc_now() -> DateTime<Utc> {
    Utc::now()
}

/// A monotonic reading. Subtract two of these to get a trustworthy duration.
pub fn monotonic() -> Instant {
    Instant::now()
}

/// A source of both clocks, injectable so discontinuity detection can be
/// tested without waiting, or without moving the real system clock.
pub trait Timeline {
    /// Civil time.
    fn wall(&self) -> DateTime<Utc>;
    /// Monotonic time since an arbitrary origin.
    fn mono(&self) -> Duration;
}

/// The real system clocks.
#[derive(Debug)]
pub struct SystemTimeline {
    origin: Instant,
}

impl SystemTimeline {
    /// Anchors a monotonic origin to now.
    pub fn new() -> Self {
        SystemTimeline {
            origin: Instant::now(),
        }
    }
}

impl Default for SystemTimeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline for SystemTimeline {
    fn wall(&self) -> DateTime<Utc> {
        utc_now()
    }

    fn mono(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// How far apart the two clocks have drifted, and in which direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Discontinuity {
    /// Nothing notable happened.
    Steady,
    /// Wall time ran ahead of monotonic time by this much more than monotonic
    /// time itself moved.
    ///
    /// This is what a suspend/resume cycle looks like on Linux, where
    /// `CLOCK_MONOTONIC` stops while the machine sleeps but the wall clock
    /// keeps running. An NTP step forward looks identical from in here, and is
    /// handled the same way: elapsed-duration measurements are brought forward
    /// by the unobserved excess, and calendar-derived state is recomputed.
    ///
    /// The amount is the *excess*, not the whole wall delta — see the module
    /// documentation — so forwarding it once brings a measurement to exactly
    /// the time the wall clock says has passed.
    Forwarded(Duration),
    /// Wall time fell behind monotonic time by this much: the clock was set
    /// back, or an NTP correction landed.
    ///
    /// Nothing was missed, so nothing is advanced: durations must neither be
    /// shortened nor stretched by a calendar that moved underneath them. The
    /// amount is kept because it is what says how large a correction landed,
    /// which is what distinguishes a correction from ordinary drift.
    Rewound(Duration),
}

impl Discontinuity {
    /// The amount of elapsed time the process did not observe, and which every
    /// measurement therefore has to be advanced by.
    ///
    /// Zero for anything but a forward jump: there is nothing to catch up after
    /// a rewind, and treating the rewind's magnitude as elapsed time is exactly
    /// the mistake of shortening or inflating a running measurement.
    pub fn missed(&self) -> Duration {
        match self {
            Discontinuity::Steady => Duration::ZERO,
            Discontinuity::Forwarded(missed) => *missed,
            Discontinuity::Rewound(_) => Duration::ZERO,
        }
    }
}

/// Deltas smaller than this are ordinary scheduler jitter, not clock changes.
const NOISE_FLOOR: Duration = Duration::from_millis(750);

/// Detects wall/monotonic divergence.
///
/// The watchdog is sampled on the ordinary refresh tick. Between two samples
/// both clocks should have advanced by the same amount; the difference is the
/// drift, and anything above [`NOISE_FLOOR`] is reported so the application
/// can reconcile its state.
///
/// There is deliberately no second threshold separating "the machine slept"
/// from "the clock was corrected". The two are indistinguishable from here —
/// both are the wall clock having moved further than the monotonic one — and,
/// more to the point, they require the same thing of the application: a
/// measurement that did not observe the stretch has to be advanced by it, and
/// the calendar has to be recomputed from the clock that did move. A threshold
/// that separated them would have to pick one of them to leave unhandled.
#[derive(Debug)]
pub struct Watchdog {
    last_wall: DateTime<Utc>,
    last_mono: Duration,
}

impl Watchdog {
    /// Starts watching, anchored to the current reading of `timeline`.
    pub fn new(timeline: &impl Timeline) -> Self {
        Watchdog {
            last_wall: timeline.wall(),
            last_mono: timeline.mono(),
        }
    }

    /// Samples both clocks and reports any divergence since the last call.
    pub fn sample(&mut self, timeline: &impl Timeline) -> Discontinuity {
        let wall = timeline.wall();
        let mono = timeline.mono();

        let signed = wall.signed_duration_since(self.last_wall);
        let mono_delta = mono.saturating_sub(self.last_mono);

        self.last_wall = wall;
        self.last_mono = mono;

        // A clock set back reads as a negative difference, which is exactly the
        // case this has to understand: it used to be swallowed here as "no
        // meaningful amount to report". Microseconds are ample resolution — the
        // noise floor below is measured in hundreds of milliseconds.
        let micros = match signed.num_microseconds() {
            Some(micros) => micros,
            // The two samples are more than a few centuries apart, which no real
            // clock does. Resynchronise silently: monotonic durations are
            // unaffected either way.
            None => return Discontinuity::Steady,
        };
        let backwards = micros < 0;
        let wall_delta = Duration::from_micros(micros.unsigned_abs());

        // How far the two readings of "how long since last time" disagree. A
        // backwards jump disagrees by the whole of both, because neither of
        // them can be had from the other by adding.
        let difference = if backwards {
            wall_delta.saturating_add(mono_delta)
        } else {
            wall_delta.abs_diff(mono_delta)
        };

        if difference < NOISE_FLOOR {
            return Discontinuity::Steady;
        }

        if backwards || mono_delta > wall_delta {
            Discontinuity::Rewound(if backwards {
                wall_delta.saturating_add(mono_delta)
            } else {
                mono_delta - wall_delta
            })
        } else {
            // Only the *excess* was missed. `mono_delta` has already been
            // accounted for by whatever reads the monotonic clock directly, and
            // forwarding the wall delta as well would count it twice.
            Discontinuity::Forwarded(wall_delta - mono_delta)
        }
    }
}

/// Number of whole seconds in `duration`, saturating.
pub fn whole_seconds(duration: Duration) -> u64 {
    duration.as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use std::cell::Cell;

    /// A timeline the test drives by hand.
    struct Fake {
        wall: Cell<DateTime<Utc>>,
        mono: Cell<Duration>,
    }

    impl Fake {
        fn new() -> Self {
            Fake {
                wall: Cell::new(
                    DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp"),
                ),
                mono: Cell::new(Duration::ZERO),
            }
        }

        /// Advances both clocks by the same amount: ordinary progress.
        fn advance(&self, both: Duration) {
            self.wall
                .set(self.wall.get() + TimeDelta::from_std(both).expect("valid delta"));
            self.mono.set(self.mono.get() + both);
        }

        /// Advances the wall clock only: an NTP step forward.
        fn step_wall(&self, by: Duration) {
            self.wall
                .set(self.wall.get() + TimeDelta::from_std(by).expect("valid delta"));
        }

        /// Advances monotonic time only: the clock was set back.
        fn step_mono(&self, by: Duration) {
            self.mono.set(self.mono.get() + by);
        }
    }

    impl Timeline for Fake {
        fn wall(&self) -> DateTime<Utc> {
            self.wall.get()
        }
        fn mono(&self) -> Duration {
            self.mono.get()
        }
    }

    fn secs(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    #[test]
    fn steady_progress_is_not_reported() {
        let fake = Fake::new();
        let mut watchdog = Watchdog::new(&fake);

        for _ in 0..5 {
            fake.advance(secs(1));
            assert_eq!(watchdog.sample(&fake), Discontinuity::Steady);
        }
    }

    #[test]
    fn sub_floor_jitter_is_ignored() {
        let fake = Fake::new();
        let mut watchdog = Watchdog::new(&fake);

        // A slow render loop is not a clock problem.
        fake.step_wall(Duration::from_millis(400));
        assert_eq!(watchdog.sample(&fake), Discontinuity::Steady);
    }

    #[test]
    fn forward_step_is_reported_as_the_unobserved_excess() {
        let fake = Fake::new();
        let mut watchdog = Watchdog::new(&fake);

        // NTP jumps the clock 10s forward while only 1s of monotonic time has
        // passed. Eleven seconds have gone by, and the monotonic clock has
        // already accounted for one of them, so ten are what was missed.
        fake.advance(secs(1));
        fake.step_wall(secs(10));
        assert_eq!(watchdog.sample(&fake), Discontinuity::Forwarded(secs(10)));

        // The whole wall delta is what the *user* lost, not what a measurement
        // is short by: reporting all eleven would count the observed second a
        // second time, which is the double counting the excess exists to stop.
        assert_ne!(
            watchdog.sample(&fake),
            Discontinuity::Forwarded(secs(11)),
            "the observed second must not be reported as missed again"
        );
    }

    #[test]
    fn a_clock_step_below_the_noise_floor_is_not_reported() {
        // A slow render loop is not a clock problem, and neither is a gap right
        // under the floor.
        let below = Fake::new();
        let mut watching = Watchdog::new(&below);
        below.step_wall(NOISE_FLOOR - Duration::from_millis(1));
        assert_eq!(watching.sample(&below), Discontinuity::Steady);

        // The floor is the boundary, and at it the gap is already reported.
        let at = Fake::new();
        let mut watching = Watchdog::new(&at);
        at.step_wall(NOISE_FLOOR);
        assert_eq!(watching.sample(&at), Discontinuity::Forwarded(NOISE_FLOOR));
    }

    #[test]
    fn backward_step_is_reported_as_a_rewind_and_misses_nothing() {
        let fake = Fake::new();
        let mut watchdog = Watchdog::new(&fake);

        fake.advance(secs(2));
        fake.step_mono(secs(30));
        assert_eq!(watchdog.sample(&fake), Discontinuity::Rewound(secs(30)));
        assert_eq!(
            watchdog.sample(&fake),
            Discontinuity::Steady,
            "a discontinuity is reported once"
        );

        // And the point of the variant: nothing was missed, so there is nothing
        // for a running measurement to catch up.
        assert_eq!(
            Discontinuity::Rewound(secs(30)).missed(),
            Duration::ZERO,
            "a clock set back must not advance a duration"
        );
    }

    #[test]
    fn a_wall_clock_set_back_misses_no_time() {
        let fake = Fake::new();
        let mut watchdog = Watchdog::new(&fake);

        // The process ran on for two seconds, and the wall clock
        // ends up an hour behind where it started.
        fake.advance(secs(2));
        fake.wall
            .set(fake.wall.get() - chrono::TimeDelta::seconds(3_600));

        let reported = watchdog.sample(&fake);
        assert_eq!(
            reported,
            Discontinuity::Rewound(secs(3_600)),
            "the whole disagreement is reported, both directions of it"
        );
        assert_eq!(reported.missed(), Duration::ZERO);
    }

    #[test]
    fn repeated_reconciliation_does_not_double_count() {
        let fake = Fake::new();
        let mut watchdog = Watchdog::new(&fake);

        fake.advance(secs(1));
        fake.step_wall(secs(10));

        // The first sample reports the gap; every sample after it reports
        // nothing, however many times the application reconciles.
        let mut missed = Duration::ZERO;
        missed += watchdog.sample(&fake).missed();
        for _ in 0..5 {
            fake.advance(secs(1));
            missed += watchdog.sample(&fake).missed();
        }

        // The process observed six seconds on the monotonic clock and the wall
        // clock says sixteen have passed: the ten in between were missed, once.
        assert_eq!(missed, secs(10));
        assert_eq!(secs(6) + missed, secs(16));
        assert_ne!(
            secs(6) + missed,
            secs(17),
            "the observed seconds must not be counted a second time"
        );
    }

    #[test]
    fn missed_time_is_exposed() {
        assert_eq!(Discontinuity::Steady.missed(), Duration::ZERO);
        assert_eq!(Discontinuity::Forwarded(secs(5)).missed(), secs(5));
        assert_eq!(
            Discontinuity::Rewound(secs(3)).missed(),
            Duration::ZERO,
            "a rewind is a correction, not elapsed time"
        );
    }

    #[test]
    fn ordinary_progress_is_never_reported_as_missed() {
        // Ordinary progress, and a scheduler that fell behind, both leave the
        // monotonic clock the authority on how much time passed. Neither is a
        // discontinuity, however long the stall was.
        let fake = Fake::new();
        let mut watchdog = Watchdog::new(&fake);
        for _ in 0..5 {
            fake.advance(secs(1));
            assert_eq!(watchdog.sample(&fake), Discontinuity::Steady);
        }

        // The process was frozen for ten minutes: the wall clock kept time and
        // so did the monotonic one, so the two agree and nothing is missing.
        fake.advance(secs(600));
        assert_eq!(watchdog.sample(&fake), Discontinuity::Steady);
        assert_eq!(
            secs(605) + watchdog.sample(&fake).missed(),
            secs(605),
            "a stall on a machine that did not sleep costs nothing"
        );
    }

    #[test]
    fn system_timeline_moves_forward_on_both_clocks() {
        let timeline = SystemTimeline::new();
        let first_wall = timeline.wall();
        let first_mono = timeline.mono();
        std::thread::sleep(Duration::from_millis(12));
        assert!(timeline.mono() > first_mono);
        assert!(timeline.wall() >= first_wall);
    }
}
