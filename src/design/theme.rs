//! Light and dark appearance, and the transition between them.

use iced::Color;
use serde::{Deserialize, Serialize};

use super::motion::{ease, FrameClock, Spec, Tween};
use super::palette::Palette;
use super::tokens::motion;

/// Which appearance the user has asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    /// Follow the desktop's light/dark setting.
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    /// All three choices, in menu order.
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

    /// Menu label.
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "Match system",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }

    /// Resolves this mode against the desktop's current preference.
    pub fn resolve(self, system_is_dark: bool) -> Appearance {
        match self {
            ThemeMode::System => {
                if system_is_dark {
                    Appearance::Dark
                } else {
                    Appearance::Light
                }
            }
            ThemeMode::Light => Appearance::Light,
            ThemeMode::Dark => Appearance::Dark,
        }
    }

    /// The mode reached by pressing the appearance control.
    ///
    /// The whole cycle, `System → Light → Dark → System`, because that is what
    /// the control in the shell does and names: it shows the appearance in
    /// effect and offers the next one, so the sequence it runs through has to
    /// come back to where it started. Skipping `System` left the keyboard's
    /// version of the control and the control on screen on different cycles,
    /// which is only ever visible as a shortcut that does not do what the
    /// button beside it says.
    pub fn next(self) -> ThemeMode {
        match self {
            ThemeMode::System => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
        }
    }

    /// Whether the desktop's setting decides the appearance.
    pub fn follows_system(self) -> bool {
        matches!(self, ThemeMode::System)
    }
}

/// The appearance actually in effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    /// The palette for this appearance.
    pub fn palette(self) -> Palette {
        match self {
            Appearance::Light => Palette::LIGHT,
            Appearance::Dark => Palette::DARK,
        }
    }

    /// The other appearance.
    pub fn opposite(self) -> Appearance {
        match self {
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
        }
    }
}

/// Reads the desktop's light/dark preference.
pub fn system_is_dark() -> bool {
    matches!(dark_light::detect(), dark_light::Mode::Dark)
}

/// Where the veil starts covering the content, as a fraction of progress.
const VEIL_OUT: f32 = 0.32;
/// Where the veil starts uncovering again.
const VEIL_IN: f32 = 0.64;

/// The application theme, and the transition between appearances.
///
/// # Why this is a dissolve and not a colour cross-fade
///
/// Interpolating a light palette into a dark one drags every colour towards
/// the middle: at the halfway point the canvas is a mid grey and the body text
/// is a slightly different mid grey. Measured against WCAG, that moment sits
/// at a contrast ratio of about 1.0 — completely illegible — and no choice of
/// easing or timing can avoid it, because any continuous path between a light
/// theme and a dark one must pass through the point where the foreground
/// meets its own background. Sampling a *faster* easing only makes the
/// illegible window narrower; it cannot make it absent.
///
/// So OClock changes theme the way a film changes scene: the content is
/// covered by a veil of the new theme's canvas colour, the palette is swapped
/// while the veil is fully opaque, and the content is revealed again. Every
/// frame a user can actually see is a complete, fully contrasted theme.
/// [`ThemeState::palette`] therefore never returns a blend — it returns one
/// whole palette or the other — and [`ThemeState::veil`] does the covering.
#[derive(Debug)]
pub struct ThemeState {
    mode: ThemeMode,
    /// The appearance fading away from.
    from: Appearance,
    /// The appearance fading towards.
    to: Appearance,
    /// 0.0 = fully `from`, 1.0 = fully `to`.
    progress: Tween,
}

impl ThemeState {
    /// Creates a theme state targeting `mode`, starting fully resolved.
    pub fn new(mode: ThemeMode, system_is_dark: bool) -> ThemeState {
        let appearance = mode.resolve(system_is_dark);
        ThemeState {
            mode,
            from: appearance,
            to: appearance,
            progress: Tween::at(1.0),
        }
    }

    /// The user's chosen mode.
    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// The appearance the transition is heading towards.
    pub fn appearance(&self) -> Appearance {
        self.to
    }

    /// True while the transition is still running.
    pub fn is_transitioning(&self, clock: &FrameClock) -> bool {
        !self.progress.settled(clock.now())
    }

    /// Changes the mode, running the standard transition.
    pub fn set_mode(&mut self, mode: ThemeMode, system_is_dark: bool, clock: &FrameClock) {
        self.set_mode_with(
            mode,
            system_is_dark,
            clock,
            Spec {
                duration: motion::DELIBERATE,
                // The veil's shape does the work; the progress itself is linear so
                // the opaque plateau in the middle is exactly where it is drawn.
                easing: super::tokens::Easing::Linear,
            },
        );
    }

    /// Applies a new mode with a caller-chosen animation.
    pub fn set_mode_with(
        &mut self,
        mode: ThemeMode,
        system_is_dark: bool,
        clock: &FrameClock,
        spec: Spec,
    ) {
        self.set_mode_inner(mode, mode.resolve(system_is_dark), clock, spec);
    }

    fn set_mode_inner(
        &mut self,
        mode: ThemeMode,
        target: Appearance,
        clock: &FrameClock,
        spec: Spec,
    ) {
        self.mode = mode;
        let now = clock.now();

        if self.is_transitioning(clock) {
            // Reversing mid-transition. Swap the endpoints and mirror the
            // progress, which leaves the palette on screen unchanged, then run
            // the tween back the other way. Without the mirror the display
            // would flip at the halfway point of the reverse.
            self.from = self.to;
            self.to = target;
            self.progress.jump(1.0 - self.progress.value(now));
            let destination = if target == self.to { 1.0 } else { 0.0 };
            self.progress.animate(destination, now, spec);
            clock.request();
            return;
        }

        if target == self.to {
            return;
        }

        self.from = self.to;
        self.to = target;
        self.progress.jump(0.0);
        self.progress.animate(1.0, now, spec);
        clock.request();
    }

    /// Adopts a change in the desktop's preference when following the system.
    ///
    /// Returns true when the appearance actually moved.
    pub fn follow_system(&mut self, system_is_dark: bool, clock: &FrameClock) -> bool {
        if !self.mode.follows_system() {
            return false;
        }
        let target = self.mode.resolve(system_is_dark);
        if target == self.to {
            return false;
        }
        self.set_mode_inner(
            self.mode,
            target,
            clock,
            Spec {
                duration: motion::SLOW,
                easing: super::tokens::Easing::Linear,
            },
        );
        true
    }

    /// The palette to draw with: always one complete theme, never a blend.
    pub fn palette(&self, clock: &FrameClock) -> Palette {
        let progress = self.progress.value(clock.now());
        if progress >= 0.5 {
            self.to.palette()
        } else {
            self.from.palette()
        }
    }

    /// The veil to draw over the whole window, or `None` when settled.
    ///
    /// Its colour cross-fades from the old canvas to the new one, which is
    /// safe precisely because it is the one thing the user is not reading.
    pub fn veil(&self, clock: &FrameClock) -> Option<Color> {
        let now = clock.now();
        if self.progress.settled(now) {
            return None;
        }

        let progress = self.progress.value(now);
        let alpha = veil_alpha(progress);
        if alpha <= 0.0 {
            return None;
        }

        let canvas = Palette::blend(self.from.palette(), self.to.palette(), progress).canvas;
        Some(ease::fade(canvas, alpha))
    }
}

/// Opacity of the transition veil across the progress range.
///
/// Rises quickly, holds fully opaque across a wide plateau, then falls. The
/// plateau is what makes the palette swap invisible, and it is also wide
/// enough that a user reversing the toggle mid-transition cannot see a seam.
fn veil_alpha(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    if progress < VEIL_OUT {
        smoothstep(0.0, VEIL_OUT, progress)
    } else if progress > VEIL_IN {
        1.0 - smoothstep(VEIL_IN, 1.0, progress)
    } else {
        1.0
    }
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::palette::contrast;
    use std::time::Duration;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn modes_resolve_against_the_system() {
        assert_eq!(ThemeMode::System.resolve(true), Appearance::Dark);
        assert_eq!(ThemeMode::System.resolve(false), Appearance::Light);
        assert_eq!(ThemeMode::Light.resolve(true), Appearance::Light);
        assert_eq!(ThemeMode::Dark.resolve(false), Appearance::Dark);
    }

    #[test]
    fn mode_cycling_visits_every_mode_and_returns() {
        // The same cycle the control in the shell runs, so the keyboard cannot
        // land somewhere the button beside it would not.
        assert_eq!(ThemeMode::System.next(), ThemeMode::Light);
        assert_eq!(ThemeMode::Light.next(), ThemeMode::Dark);
        assert_eq!(ThemeMode::Dark.next(), ThemeMode::System);

        let mut mode = ThemeMode::System;
        for _ in 0..ThemeMode::ALL.len() {
            mode = mode.next();
        }
        assert_eq!(mode, ThemeMode::System, "and it is a cycle, not a walk");
    }

    #[test]
    fn a_new_state_is_fully_resolved_and_unveiled() {
        let clock = FrameClock::new();
        let state = ThemeState::new(ThemeMode::Dark, false);
        assert_eq!(state.appearance(), Appearance::Dark);
        assert_eq!(state.palette(&clock), Palette::DARK);
        assert_eq!(state.veil(&clock), None);
        assert!(!state.is_transitioning(&clock));
    }

    #[test]
    fn the_palette_always_one_whole_theme() {
        let clock = FrameClock::new();
        let mut state = ThemeState::new(ThemeMode::Light, false);
        state.set_mode(ThemeMode::Dark, false, &clock);

        // Sample the whole transition. The palette must only ever be exactly
        // the light one or exactly the dark one.
        for step in 0..60 {
            std::thread::sleep(motion::DELIBERATE / 60);
            let palette = state.palette(&clock);
            assert!(
                palette == Palette::LIGHT || palette == Palette::DARK,
                "step {step}: palette was a blend"
            );
        }
        assert_eq!(state.palette(&clock), Palette::DARK);
    }

    #[test]
    fn whatever_is_visible_is_always_readable() {
        let clock = FrameClock::new();
        let mut state = ThemeState::new(ThemeMode::Light, false);
        state.set_mode(ThemeMode::Dark, false, &clock);

        // The core guarantee: at any moment where the veil is not essentially
        // opaque, the palette underneath is a complete, legible theme.
        for _ in 0..40 {
            let palette = state.palette(&clock);
            let veil = state.veil(&clock).map_or(0.0, |c| c.a);

            if veil < 0.9 {
                assert!(
                    contrast(palette.text, palette.canvas) >= 7.0,
                    "readable content drawn under a {veil:.2} veil failed the contrast floor"
                );
            }
            std::thread::sleep(motion::DELIBERATE / 40);
        }
    }

    #[test]
    fn the_veil_reaches_full_opacity_around_the_swap() {
        assert_eq!(veil_alpha(0.0), 0.0);
        assert_eq!(veil_alpha(1.0), 0.0);
        assert!((veil_alpha(0.5) - 1.0).abs() < 1.0e-6);
        // A wide plateau, so a reversal cannot land on a visible seam.
        for step in 0..=10 {
            let progress = VEIL_OUT + (VEIL_IN - VEIL_OUT) * step as f32 / 10.0;
            assert!((veil_alpha(progress) - 1.0).abs() < 1.0e-6);
        }
    }

    #[test]
    fn the_veil_rises_quicker_than_it_falls() {
        // Outgoing: the content should clear the screen promptly.
        assert!(veil_alpha(VEIL_OUT / 2.0) > 0.4);
        // Incoming: a slightly slower reveal feels calmer.
        assert!(veil_alpha(VEIL_IN + (1.0 - VEIL_IN) / 2.0) < 0.7);
    }

    #[test]
    fn changing_mode_actually_transitions() {
        let clock = FrameClock::new();
        let mut state = ThemeState::new(ThemeMode::Light, false);
        state.set_mode(ThemeMode::Dark, false, &clock);

        assert!(state.is_transitioning(&clock));
        assert!(state.veil(&clock).is_some());
        assert_eq!(state.palette(&clock), Palette::LIGHT, "before the swap");

        std::thread::sleep(motion::DELIBERATE + ms(60));
        assert!(!state.is_transitioning(&clock));
        assert_eq!(state.veil(&clock), None);
        assert_eq!(state.palette(&clock), Palette::DARK);
    }

    #[test]
    fn reversing_mid_transition_does_not_change_what_is_on_screen() {
        let clock = FrameClock::new();
        let mut state = ThemeState::new(ThemeMode::Light, false);
        state.set_mode(ThemeMode::Dark, false, &clock);
        std::thread::sleep(motion::DELIBERATE / 3);

        let before = state.palette(&clock);
        state.set_mode(ThemeMode::Light, false, &clock);
        assert_eq!(
            state.palette(&clock),
            before,
            "reversing swapped the palette immediately"
        );

        std::thread::sleep(motion::DELIBERATE + ms(60));
        assert_eq!(state.palette(&clock), Palette::LIGHT);
    }

    #[test]
    fn reversing_stays_readable_throughout() {
        let clock = FrameClock::new();
        let mut state = ThemeState::new(ThemeMode::Light, false);
        state.set_mode(ThemeMode::Dark, false, &clock);
        std::thread::sleep(motion::DELIBERATE / 4);
        state.set_mode(ThemeMode::Light, false, &clock);

        for _ in 0..30 {
            let palette = state.palette(&clock);
            let veil = state.veil(&clock).map_or(0.0, |c| c.a);
            if veil < 0.9 {
                assert!(contrast(palette.text, palette.canvas) >= 7.0);
            }
            std::thread::sleep(motion::DELIBERATE / 30);
        }
    }

    #[test]
    fn setting_the_same_mode_does_nothing() {
        let clock = FrameClock::new();
        let mut state = ThemeState::new(ThemeMode::Dark, false);
        state.set_mode(ThemeMode::Dark, false, &clock);
        assert!(!state.is_transitioning(&clock));
        assert_eq!(state.veil(&clock), None);
    }

    #[test]
    fn system_changes_are_followed_only_in_system_mode() {
        let clock = FrameClock::new();
        let mut state = ThemeState::new(ThemeMode::System, false);
        assert!(state.follow_system(true, &clock));
        assert_eq!(state.appearance(), Appearance::Dark);
        assert!(
            !state.follow_system(true, &clock),
            "no change, no transition"
        );

        let mut pinned = ThemeState::new(ThemeMode::Light, false);
        assert!(!pinned.follow_system(true, &clock));
        assert_eq!(pinned.appearance(), Appearance::Light);
    }

    #[test]
    fn modes_round_trip_through_json() {
        for mode in ThemeMode::ALL {
            let json = serde_json::to_string(&mode).expect("serializes");
            let restored: ThemeMode = serde_json::from_str(&json).expect("deserializes");
            assert_eq!(mode, restored);
        }
    }

    #[test]
    fn labels_and_opposites() {
        let labels: Vec<&str> = ThemeMode::ALL.iter().map(|m| m.label()).collect();
        assert_eq!(labels, vec!["Match system", "Light", "Dark"]);
        assert_eq!(Appearance::Light.opposite(), Appearance::Dark);
        assert_eq!(Appearance::Dark.opposite(), Appearance::Light);
    }
}
