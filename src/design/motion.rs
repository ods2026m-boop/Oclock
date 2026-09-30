//! OClock's motion system.
//!
//! # Why this shape
//!
//! Every animated value in OClock is a pure function of time: a [`Tween`]
//! stores `(from, to, started, duration, easing)` and `value(now)` evaluates
//! it. Nothing has to be *stepped* frame by frame, so an animation can be
//! read from a widget style closure at any moment and is always exactly
//! where it should be for the current time — no drift, no catch-up loop, and
//! dropping frames degrades gracefully instead of desynchronising.
//!
//! The only thing an animation needs from the outside is a repaint, and the
//! application only runs its 60 Hz stream while [`FrameClock::active`] says
//! something is in flight. Idle cost is one wakeup per refresh interval.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::tokens::{motion, Easing};

/// How long after the most recent animation request the app keeps rendering
/// at frame rate. Comfortably longer than the slowest token
/// ([`motion::DELIBERATE`]) so animations always finish smoothly before the
/// stream shuts down.
const FRAME_BUDGET: Duration = Duration::from_millis(700);

/// A timing description: how long, and along which curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub duration: Duration,
    pub easing: Easing,
}

impl Spec {
    /// Instantaneous, for state that should not animate at all.
    pub const INSTANT: Spec = Spec {
        duration: motion::INSTANT,
        easing: Easing::Standard,
    };
    /// Hover response.
    pub const HOVER: Spec = Spec {
        duration: motion::FAST,
        easing: motion::EASE_HOVER,
    };
    /// Press and release.
    pub const PRESS: Spec = Spec {
        duration: motion::INSTANT,
        easing: Easing::Standard,
    };
    /// Colour and opacity changes.
    pub const TINT: Spec = Spec {
        duration: motion::NORMAL,
        easing: motion::EASE_TINT,
    };
    /// Entrances.
    pub const ENTER: Spec = Spec {
        duration: motion::SLOW,
        easing: motion::EASE_ENTER,
    };
    /// Exits, deliberately quicker than entrances.
    pub const EXIT: Spec = Spec {
        duration: motion::FAST,
        easing: motion::EASE_EXIT,
    };
    /// Movement between two layouts.
    pub const MOVE: Spec = Spec {
        duration: motion::SLOW,
        easing: motion::EASE_MOVE,
    };
    /// A celebratory flourish.
    pub const CELEBRATE: Spec = Spec {
        duration: motion::DELIBERATE,
        easing: motion::EASE_CELEBRATE,
    };

    /// The same duration, but eased out — useful when a value is chasing a
    /// moving target such as a progress arc.
    pub const fn with_easing(self, easing: Easing) -> Spec {
        Spec {
            duration: self.duration,
            easing,
        }
    }
}

/// A single interpolation between two `f32` values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    from: f32,
    to: f32,
    started: Duration,
    spec: Spec,
}

impl Tween {
    /// A tween that is already finished, at `value`.
    pub const fn at(value: f32) -> Tween {
        Tween {
            from: value,
            to: value,
            started: Duration::ZERO,
            spec: Spec {
                duration: Duration::ZERO,
                easing: Easing::Linear,
            },
        }
    }

    /// Starts a new interpolation from wherever the tween currently is, so
    /// interrupting an animation never produces a jump.
    pub fn animate(&mut self, to: f32, now: Duration, spec: Spec) {
        let from = self.value(now);
        self.from = from;
        self.to = to;
        self.started = now;
        self.spec = spec;
    }

    /// Snaps to `to` with no interpolation.
    pub fn jump(&mut self, to: f32) {
        self.from = to;
        self.to = to;
        self.started = Duration::ZERO;
        self.spec = Spec {
            duration: Duration::ZERO,
            easing: Easing::Linear,
        };
    }

    /// Evaluates the tween at `now`.
    pub fn value(&self, now: Duration) -> f32 {
        if self.spec.duration.is_zero() || self.from == self.to {
            return self.to;
        }

        let elapsed = now.saturating_sub(self.started).as_secs_f32();
        let t = (elapsed / self.spec.duration.as_secs_f32()).clamp(0.0, 1.0);

        self.from + (self.to - self.from) * self.spec.easing.apply(t)
    }

    /// True once the tween has reached its target.
    pub fn settled(&self, now: Duration) -> bool {
        self.spec.duration.is_zero() || now.saturating_sub(self.started) >= self.spec.duration
    }

    /// The target the tween is heading towards.
    pub fn target(&self) -> f32 {
        self.to
    }
}

impl Default for Tween {
    fn default() -> Self {
        Tween::at(0.0)
    }
}

/// Tracks a boolean interaction flag and exposes it as an animated `0..=1`
/// value.
///
/// This is what gives every button, card and row its own hover and press
/// response without a single key or bookkeeping entry: the widget style
/// closure reports the interaction state it was handed, and the closure's own
/// [`TrackedHandle`] animates it.
#[derive(Debug)]
pub struct Tracked {
    tween: Tween,
    target: f32,
    observed: bool,
    seen: bool,
}

/// An `Rc` handle to a [`Tracked`], so it can be owned by a `Fn` style closure.
///
/// An `Rc` rather than an `Arc`: a style closure is only ever called on the
/// thread that is drawing, and the reference count of a value touched once per
/// control per frame is not worth an atomic.
#[derive(Clone, Debug)]
pub struct TrackedHandle(Rc<RefCell<Tracked>>);

impl Tracked {
    /// A tracker resting at `initial`.
    pub fn new(initial: bool) -> Self {
        let value = if initial { 1.0 } else { 0.0 };
        Tracked {
            tween: Tween::at(value),
            target: value,
            observed: initial,
            seen: false,
        }
    }

    /// Feeds the live interaction state and returns the animated value.
    ///
    /// The first observation snaps: a control that is already hovered when it
    /// first appears should not fade in from nothing.
    pub fn track(&mut self, active: bool, clock: &FrameClock, spec: Spec) -> f32 {
        if !self.seen {
            // First observation: adopt it as-is. A control that is already
            // hovered when it first appears must not fade up from nothing.
            self.seen = true;
            self.observed = active;
            self.jump(if active { 1.0 } else { 0.0 });
        } else if active != self.observed {
            self.target = if active { 1.0 } else { 0.0 };
            self.tween.animate(self.target, clock.now(), spec);
            clock.request();
        }
        self.observed = active;
        self.tween.value(clock.now())
    }

    /// Drives the value from an explicit target rather than a boolean.
    pub fn drive(&mut self, target: f32, clock: &FrameClock, spec: Spec) -> f32 {
        if (target - self.target).abs() > f32::EPSILON {
            self.target = target;
            self.tween.animate(target, clock.now(), spec);
            clock.request();
        }
        self.tween.value(clock.now())
    }

    /// Snaps to a target without animating.
    pub fn jump(&mut self, target: f32) {
        self.target = target;
        self.tween.jump(target);
    }

    /// The current animated value.
    pub fn value(&self, clock: &FrameClock) -> f32 {
        self.tween.value(clock.now())
    }
}

impl TrackedHandle {
    /// Creates a tracker resting at `initial`.
    pub fn new(initial: bool) -> Self {
        TrackedHandle(Rc::new(RefCell::new(Tracked::new(initial))))
    }

    /// Feeds the live interaction state and returns the animated value.
    pub fn track(&self, active: bool, clock: &FrameClock, spec: Spec) -> f32 {
        self.0.borrow_mut().track(active, clock, spec)
    }

    /// Drives the value from an explicit target.
    pub fn drive(&self, target: f32, clock: &FrameClock, spec: Spec) -> f32 {
        self.0.borrow_mut().drive(target, clock, spec)
    }

    /// Snaps to a target without animating.
    pub fn jump(&self, target: f32) {
        self.0.borrow_mut().jump(target);
    }
}

/// Enter/leave state for a piece of content that can come and go — a list
/// card, a dialog, a toast.
///
/// [`Presence::value`] is `1.0` while fully shown, `0.0` while fully gone,
/// and in between while entering or leaving, so a single number can drive
/// opacity, offset and height.
///
/// The exit is *not* instantaneous: the caller keeps rendering the item while
/// [`Presence::is_removable`] is false, which is what lets a deleted alarm
/// fade out of the list instead of vanishing between frames.
#[derive(Clone, Debug, PartialEq)]
pub struct Presence {
    tween: Tween,
    present: bool,
}

impl Presence {
    /// A presence that is initially shown.
    pub fn shown() -> Self {
        Presence {
            tween: Tween::at(1.0),
            present: true,
        }
    }

    /// A presence that is initially hidden.
    pub fn hidden() -> Self {
        Presence {
            tween: Tween::at(0.0),
            present: false,
        }
    }

    /// True when the content should exist at all.
    pub fn is_present(&self) -> bool {
        self.present
    }

    /// True once the entrance or exit has run to completion.
    pub fn is_settled(&self, now: Duration) -> bool {
        self.tween.settled(now)
    }

    /// True when a requested exit has finished and the caller can drop the
    /// item from its list for good.
    pub fn is_removable(&self, now: Duration) -> bool {
        !self.present && self.tween.settled(now)
    }

    /// Animates in.
    pub fn show(&mut self, now: Duration, spec: Spec) {
        self.present = true;
        if self.tween.target() >= 1.0 && self.tween.settled(now) {
            self.tween.jump(1.0);
            return;
        }
        self.tween.animate(1.0, now, spec);
    }

    /// Animates out.
    pub fn hide(&mut self, now: Duration, spec: Spec) {
        self.present = false;
        self.tween.animate(0.0, now, spec);
    }

    /// The animated presence, `0.0..=1.0`.
    pub fn value(&self, now: Duration) -> f32 {
        self.tween.value(now)
    }
}

/// Tells widgets when to keep rendering.
///
/// A [`FrameClock`] is shared between the application state and every style
/// closure. Whenever any tween starts, it stamps a budget deadline; the
/// application runs its frame-rate subscription only while that deadline is
/// in the future, so hovering a button costs ~0.4 s of 60 fps and nothing at
/// all the rest of the time.
///
/// It is shareable by the type system rather than by assertion. The two pieces
/// of state it keeps are an instant and two atomics, so there is no `Cell` for
/// two threads to race on and no `unsafe impl` whose contract could quietly stop
/// holding. Two threads do read it — the runtime's own loop, which asks whether
/// another frame is wanted, and the renderer, which draws one — and a frame that
/// read a stale budget costs one more repaint rather than anything worse, which
/// is why the atomics are relaxed and there is no lock on the renderer's hottest
/// path.
#[derive(Clone)]
pub struct FrameClock(Arc<ClockInner>);

struct ClockInner {
    origin: Instant,
    /// When a frame was last asked for, in nanoseconds from [`ClockInner::origin`].
    last_request: AtomicU64,
    /// False until something actually requests frames, so a fresh clock reads
    /// as idle rather than as busy for its first budget period.
    armed: AtomicBool,
}

impl FrameClock {
    /// Creates a clock anchored to now.
    pub fn new() -> Self {
        FrameClock(Arc::new(ClockInner {
            origin: Instant::now(),
            last_request: AtomicU64::new(0),
            armed: AtomicBool::new(false),
        }))
    }

    /// Monotonic time since the clock was created.
    ///
    /// Monotonic on purpose: animation timing must not be affected by the
    /// system clock being changed underneath the application.
    pub fn now(&self) -> Duration {
        self.0.origin.elapsed()
    }

    /// Records that something is animating, starting the frame budget.
    pub fn request(&self) {
        self.0
            .last_request
            .store(nanos(self.now()), Ordering::Relaxed);
        self.0.armed.store(true, Ordering::Relaxed);
    }

    /// True while the frame budget is unspent.
    pub fn active(&self) -> bool {
        self.0.armed.load(Ordering::Relaxed) && self.since_request() < FRAME_BUDGET
    }

    /// How long until the frame budget expires.
    pub fn remaining(&self) -> Duration {
        FRAME_BUDGET.saturating_sub(self.since_request())
    }

    /// How long ago a frame was asked for.
    ///
    /// Both halves are on the same clock: the reading stored when the frame was
    /// asked for, and the reading now. Neither is ever zero — the stored one
    /// starts at the clock's own origin, so an unasked-for clock simply has all
    /// of its budget left, which is what it says it has.
    fn since_request(&self) -> Duration {
        let requested = Duration::from_nanos(self.0.last_request.load(Ordering::Relaxed));
        self.now().saturating_sub(requested)
    }
}

/// A duration as a whole number of nanoseconds.
///
/// A clock's whole life is milliseconds, so `u64` will not wrap for longer than
/// the process runs; the saturation is only here so the conversion cannot
/// overflow on the way to being compared.
fn nanos(value: Duration) -> u64 {
    u64::try_from(value.as_nanos()).unwrap_or(u64::MAX)
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FrameClock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameClock")
            .field("now", &self.now())
            .field("active", &self.active())
            .finish()
    }
}

/// Helpers for turning an animated value into visual properties.
pub mod ease {
    use std::time::Duration;

    /// Remaps `value` from `0..=1` onto `from..=to`.
    pub fn lerp(from: f32, to: f32, value: f32) -> f32 {
        from + (to - from) * value.clamp(0.0, 1.0)
    }

    /// Interpolates a colour between two points. A colour *t* of the way
    /// through an alpha cross-fade needs this, because plain `lerp` on the
    /// channels is not what the eye expects.
    pub fn color(a: iced::Color, b: iced::Color, t: f32) -> iced::Color {
        let t = t.clamp(0.0, 1.0);
        iced::Color {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
    }

    /// Alpha channel of a colour at position `t`.
    pub fn alpha(color: iced::Color, t: f32) -> f32 {
        color.a * t.clamp(0.0, 1.0)
    }

    /// A copy of `color` with its alpha scaled by `t`.
    pub fn fade(color: iced::Color, t: f32) -> iced::Color {
        iced::Color {
            a: alpha(color, t),
            ..color
        }
    }

    /// A smooth `0..1..0` pulse with the given period, for idle "breathing".
    pub fn pulse(elapsed: Duration, period: Duration) -> f32 {
        let phase = elapsed.as_secs_f64() / period.as_secs_f64().max(f64::MIN_POSITIVE);
        let phase = phase - phase.floor();
        // Raised cosine: smooth at both ends, unlike a raw sine.
        (0.5 - 0.5 * (phase * std::f64::consts::TAU).cos()) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn tween_interpolates_over_time() {
        let mut tween = Tween::at(0.0);
        tween.animate(100.0, ms(0), Spec::HOVER);

        assert!((tween.value(ms(0)) - 0.0).abs() < 1.0e-3);
        assert!(tween.value(ms(75)) > 50.0, "halfway should be past halfway");
        assert!((tween.value(ms(150)) - 100.0).abs() < 1.0e-3);
        assert!((tween.value(ms(10_000)) - 100.0).abs() < 1.0e-3);
    }

    #[test]
    fn tween_starts_from_its_current_value() {
        let mut tween = Tween::at(0.0);
        tween.animate(100.0, ms(0), Spec::HOVER);
        let interrupted_at = tween.value(ms(60));

        // Retarget mid-flight: no jump, just a new direction.
        tween.animate(0.0, ms(60), Spec::HOVER);
        assert!((tween.value(ms(60)) - interrupted_at).abs() < 1.0e-3);
        assert!(tween.value(ms(140)) < interrupted_at);
    }

    #[test]
    fn tween_settled_tracks_its_target() {
        let mut tween = Tween::at(1.0);
        assert!(tween.settled(ms(0)));

        tween.animate(0.0, ms(100), Spec::HOVER);
        assert!(!tween.settled(ms(100)));
        assert!(!tween.settled(ms(200)));
        assert!(tween.settled(ms(300)));
    }

    #[test]
    fn instant_specs_do_not_interpolate() {
        let mut tween = Tween::at(0.0);
        tween.animate(
            1.0,
            ms(0),
            Spec {
                duration: Duration::ZERO,
                easing: Easing::Linear,
            },
        );
        assert_eq!(tween.value(ms(0)), 1.0);
        assert!(tween.settled(ms(0)));
    }

    #[test]
    fn tracked_snaps_on_first_observation() {
        let clock = FrameClock::new();
        let mut tracked = Tracked::new(false);

        // Already hovered when it first appears: no fade from nothing.
        assert!((tracked.track(true, &clock, Spec::HOVER) - 1.0).abs() < 1.0e-3);
    }

    #[test]
    fn tracked_animates_on_change_and_rests_otherwise() {
        let clock = FrameClock::new();
        let mut tracked = Tracked::new(false);
        assert_eq!(tracked.track(false, &clock, Spec::HOVER), 0.0);

        let rising = tracked.track(true, &clock, Spec::HOVER);
        assert!(
            rising > 0.0 && rising < 1.0,
            "expected a transition, got {rising}"
        );

        // Repeating the same state must not restart the animation.
        let again = tracked.track(true, &clock, Spec::HOVER);
        assert!(again >= rising);
    }

    #[test]
    fn tracked_requests_frames() {
        let clock = FrameClock::new();
        // Drain the budget created at construction by an earlier request.
        std::thread::sleep(FRAME_BUDGET + ms(20));
        assert!(!clock.active());

        let mut tracked = Tracked::new(false);
        tracked.track(false, &clock, Spec::HOVER);
        tracked.track(true, &clock, Spec::HOVER);
        assert!(clock.active());
    }

    #[test]
    fn presence_runs_a_full_lifecycle() {
        let now = Duration::from_secs(1);
        let mut presence = Presence::shown();
        assert!((presence.value(now) - 1.0).abs() < 1.0e-3);
        assert!(!presence.is_removable(now));

        // Exiting: still drawn, but on its way out and not yet droppable.
        presence.hide(now, Spec::EXIT);
        assert!(!presence.is_present());
        assert!(!presence.is_removable(now));
        assert!(presence.value(now + Spec::EXIT.duration / 2) < 0.9);
        assert!(!presence.is_removable(now + Spec::EXIT.duration - ms(1)));
        assert!(presence.value(now + Spec::EXIT.duration) < 0.01);
        assert!(presence.is_removable(now + Spec::EXIT.duration + ms(1)));

        // Re-entering: present again, and never removable while it animates.
        let start = now + ms(500);
        presence.show(start, Spec::ENTER);
        assert!(presence.is_present());
        assert!(!presence.is_removable(start));
        assert!((presence.value(start + Spec::ENTER.duration) - 1.0).abs() < 1.0e-3);
        assert!(!presence.is_removable(start + Spec::ENTER.duration + ms(1)));
    }

    #[test]
    fn presence_starts_moving_the_moment_it_is_hidden() {
        let now = Duration::from_secs(1);
        let mut presence = Presence::shown();
        presence.hide(now, Spec::EXIT);

        // The first frame is still fully shown; by the next one it is already
        // on its way out, so an exit never appears to hang for a frame.
        assert!((presence.value(now) - 1.0).abs() < 1.0e-3);
        let early = presence.value(now + Spec::EXIT.duration / 4);
        assert!(
            early < 1.0 && early > 0.0,
            "expected an exit in progress, got {early}"
        );
    }

    #[test]
    fn clock_is_monotonic_and_budget_expires() {
        let clock = FrameClock::new();
        let first = clock.now();
        std::thread::sleep(ms(5));
        assert!(clock.now() > first);

        clock.request();
        assert!(clock.active());
        assert!(clock.remaining() > Duration::ZERO);

        std::thread::sleep(FRAME_BUDGET + ms(50));
        assert!(!clock.active(), "the budget must run out on its own");
    }

    #[test]
    fn a_fresh_clock_is_idle() {
        // Nothing has animated yet, so the application must not be running its
        // frame-rate stream for the first fraction of a second of its life.
        assert!(!FrameClock::new().active());
    }

    #[test]
    fn lerp_and_alpha_helpers_clamp() {
        assert_eq!(ease::lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(ease::lerp(0.0, 10.0, -1.0), 0.0);
        assert_eq!(ease::lerp(0.0, 10.0, 2.0), 10.0);

        let half = ease::color(
            iced::Color::from_rgba(0.0, 0.0, 0.0, 0.0),
            iced::Color::from_rgba(0.0, 0.0, 0.0, 1.0),
            0.5,
        );
        assert!((half.a - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn pulse_is_bounded_and_cycles() {
        let period = ms(1000);
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for step in 0..200 {
            let value = ease::pulse(ms(step * 5), period);
            min = min.min(value);
            max = max.max(value);
        }
        assert!((min - 0.0).abs() < 1.0e-3);
        assert!((max - 1.0).abs() < 1.0e-3);
    }
}
