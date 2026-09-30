//! Interaction state: the per-control animation values.
//!
//! A hover or press response has to *remember* something: when the pointer
//! arrived, so the value can be eased from wherever it was. Iced's widget state
//! tree is not reachable from a style closure, so OClock keeps that memory here
//! instead — one small map of animated values, owned by the application and
//! handed down to whatever needs it.
//!
//! Controls are identified by a `&'static str` label rather than by an object,
//! so adding a button to a page costs a label and no struct field, and the map
//! stays as small as the number of controls on screen.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::design::motion::{FrameClock, TrackedHandle};

/// The animated response of one control.
///
/// Three values rather than one, because a control answers to three different
/// things and they are worth separating: the pointer arriving, the press, and
/// the control's own state changing.
#[derive(Clone, Debug)]
pub struct Interaction {
    /// 0..1, following the pointer.
    pub hover: TrackedHandle,
    /// 0..1, following the press.
    pub press: TrackedHandle,
    /// 0..1, following the control's own state — a switch flipping, a chip
    /// being selected.
    pub state: TrackedHandle,
}

impl Interaction {
    /// A fresh set of handles, with `state` starting at `state`.
    pub fn starting(state: bool) -> Interaction {
        Interaction {
            hover: TrackedHandle::new(false),
            press: TrackedHandle::new(false),
            state: TrackedHandle::new(state),
        }
    }
}

impl Default for Interaction {
    fn default() -> Self {
        Interaction::starting(false)
    }
}

/// Every control's interaction state, for one window.
#[derive(Default)]
pub struct Interactions {
    entries: RefCell<HashMap<&'static str, Interaction>>,
}

impl Interactions {
    /// An empty set, for a window with nothing interactive yet.
    pub fn new() -> Interactions {
        Interactions {
            entries: RefCell::new(HashMap::new()),
        }
    }

    /// The animated state for `key`, created on first use.
    ///
    /// Handles are cloned, not handed out, so a control can be dropped and
    /// rebuilt each frame without losing its animation.
    pub fn get(&self, key: &'static str) -> Interaction {
        let mut entries = self.entries.borrow_mut();
        entries.entry(key).or_default().clone()
    }

    /// How many controls are tracking state.
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }

    /// True when nothing is being tracked.
    pub fn is_empty(&self) -> bool {
        self.entries.borrow().is_empty()
    }

    /// Whether anything in here is still animating.
    ///
    /// Checked on the idle tick so the application can drop back to its cheap
    /// refresh rate the moment the last control settles.
    pub fn is_settled(&self, clock: &FrameClock) -> bool {
        // A conservative answer: if the frame budget is spent, nothing is
        // animating, because every animation requests frames when it starts.
        !clock.active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::motion::Spec;
    use std::time::Duration;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn a_state_handle_starts_where_the_control_does() {
        let clock = FrameClock::new();
        let on = Interaction::starting(true);
        assert!(
            (on.state
                .drive(1.0, &clock, crate::design::motion::Spec::TINT)
                - 1.0)
                .abs()
                < 1.0e-3
        );

        let off = Interaction::default();
        assert!(
            off.state
                .drive(0.0, &clock, crate::design::motion::Spec::TINT)
                .abs()
                < 1.0e-3
        );
    }

    #[test]
    fn a_new_set_is_empty() {
        let interactions = Interactions::new();
        assert!(interactions.is_empty());
        assert_eq!(interactions.len(), 0);
    }

    #[test]
    fn state_is_created_on_first_use_and_then_kept() {
        let interactions = Interactions::new();
        let first = interactions.get("clock.toggle");
        assert_eq!(interactions.len(), 1);

        // A second control is a new entry.
        let _ = interactions.get("clock.dismiss");
        assert_eq!(interactions.len(), 2);

        // Asking again returns handles onto the same state, not fresh ones.
        let again = interactions.get("clock.toggle");
        let clock = FrameClock::new();
        assert!((first.hover.track(true, &clock, Spec::HOVER) - 1.0).abs() < 1.0e-3);
        std::thread::sleep(ms(40));
        again.hover.track(true, &clock, Spec::HOVER);
        let value = first.hover.track(true, &clock, Spec::HOVER);
        assert!(
            value > 0.0,
            "the animation should have continued, not restarted"
        );
    }

    #[test]
    fn a_cloned_handle_still_shares_its_state() {
        let interactions = Interactions::new();
        let handle = interactions.get("a");
        let again = interactions.get("a");

        let clock = FrameClock::new();
        handle.hover.track(true, &clock, Spec::HOVER);
        std::thread::sleep(ms(40));
        let value = again.hover.track(true, &clock, Spec::HOVER);
        assert!(value > 0.0, "the clone must see the same animated value");
    }

    #[test]
    fn a_control_that_appears_mid_hover_snaps_rather_than_fading() {
        // This is what stops a page appearing under the pointer from visibly
        // animating every control at once.
        let clock = FrameClock::new();
        let interaction = Interaction::default();
        assert!((interaction.hover.track(true, &clock, Spec::HOVER) - 1.0).abs() < 1.0e-3);
    }

    #[test]
    fn a_control_that_never_interacted_does_not_ask_for_frames() {
        let clock = FrameClock::new();
        let interaction = Interaction::default();
        interaction.press.track(false, &clock, Spec::PRESS);
        interaction.hover.track(false, &clock, Spec::HOVER);
        assert!(
            !clock.active(),
            "nothing is animating, so nothing needs frames"
        );
    }

    #[test]
    fn settled_follows_the_frame_budget() {
        let clock = FrameClock::new();
        let interactions = Interactions::new();
        let interaction = interactions.get("a");

        // A first observation snaps, so an animation only starts once the
        // state actually changes — which is also how it behaves on screen.
        interaction.hover.track(false, &clock, Spec::HOVER);
        interaction.hover.track(true, &clock, Spec::HOVER);
        assert!(
            !interactions.is_settled(&clock),
            "an animation in flight means frames are needed"
        );

        // The budget is longer than the animation, so once it expires nothing
        // is left to drive and the window can drop to its idle refresh rate.
        std::thread::sleep(ms(900));
        assert!(interactions.is_settled(&clock));
    }
}
