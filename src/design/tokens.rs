//! Design tokens for OClock.
//!
//! Every visual decision that is not a specific colour lives here: the
//! spacing scale, corner radii, the type ramp, elevation and the motion
//! timings. Components read these instead of hard-coding numbers, so changing
//! a token re-skins the whole application.

use iced::{Color, Shadow, Vector};

/// Base unit of the 4pt-ish spacing grid.
pub const UNIT: f32 = 4.0;

/// Spacing scale. Use these instead of ad-hoc numbers so rhythm stays
/// consistent across every page.
pub mod space {
    /// 2pt — hairline separations.
    pub const XXS: f32 = 2.0;
    /// 4pt — inside chips and badges.
    pub const XS: f32 = 4.0;
    /// 6pt — tight label padding.
    pub const SM: f32 = 8.0;
    /// 12pt — default gap inside a control.
    pub const MD: f32 = 12.0;
    /// 16pt — gap between related controls.
    pub const LG: f32 = 16.0;
    /// 20pt — card inner padding.
    pub const XL: f32 = 20.0;
    /// 24pt — gap between cards.
    pub const XXL: f32 = 24.0;
    /// 32pt — section separation.
    pub const XXXL: f32 = 32.0;
    /// 48pt — page rhythm.
    pub const HUGE: f32 = 48.0;
}

/// The scroll bar.
///
/// Iced puts the bar for a vertical scroll on the right of its viewport, in
/// every language, and offers no way to move it. A page whose content hangs
/// from the right therefore has to make room for it, or the leading edge of
/// every line sits under the track.
pub mod scrollbar {
    /// The width of the track, as Iced draws it.
    pub const WIDTH: f32 = 10.0;
    /// The gap between the track and the content beside it.
    pub const GAP: f32 = 6.0;
}

/// Corner radii. One scale, one look.
pub mod radius {
    /// Chips, tags, small badges.
    pub const SM: f32 = 8.0;
    /// Inputs, small buttons.
    pub const MD: f32 = 12.0;
    /// Buttons, chips with content.
    pub const LG: f32 = 14.0;
    /// Cards.
    pub const XL: f32 = 18.0;
    /// Hero surfaces, modals.
    pub const XXL: f32 = 24.0;
    /// Pills and circular controls.
    pub const PILL: f32 = 999.0;
}

/// Border widths.
pub mod stroke {
    /// Hairline separators and default control outlines.
    pub const HAIRLINE: f32 = 1.0;
    /// Emphasised or focused outlines.
    pub const THICK: f32 = 2.0;
}

/// Elevation. OClock leans on layered translucent surfaces plus soft
/// shadows rather than heavy borders.
pub mod elevation {
    use super::{Color, Shadow, Vector};

    pub const FLAT: Shadow = Shadow {
        offset: Vector::new(0.0, 0.0),
        blur_radius: 0.0,
        color: Color::TRANSPARENT,
    };

    /// A whisper of lift, for controls that sit flush on a card.
    pub const LOW: Shadow = Shadow {
        offset: Vector::new(0.0, 1.0),
        blur_radius: 2.0,
        color: Color::from_rgba(0.06, 0.07, 0.12, 0.06),
    };

    /// Card lift.
    pub const MEDIUM: Shadow = Shadow {
        offset: Vector::new(0.0, 4.0),
        blur_radius: 14.0,
        color: Color::from_rgba(0.06, 0.07, 0.12, 0.10),
    };

    /// Modal and menu lift.
    pub const HIGH: Shadow = Shadow {
        offset: Vector::new(0.0, 18.0),
        blur_radius: 40.0,
        color: Color::from_rgba(0.04, 0.05, 0.10, 0.22),
    };
}

/// Motion timings and the easing curves that pair with them.
pub mod motion {
    use std::time::Duration;

    use super::Easing;

    /// Press/release feedback. Deliberately fast: it must feel like the
    /// control is part of the finger.
    pub const INSTANT: Duration = Duration::from_millis(90);
    /// Hover, toggles, small state flips.
    pub const FAST: Duration = Duration::from_millis(150);
    /// The default for most transitions.
    pub const NORMAL: Duration = Duration::from_millis(240);
    /// Page transitions and larger surfaces.
    pub const SLOW: Duration = Duration::from_millis(360);
    /// Theme cross-fade and other big, calm changes.
    pub const DELIBERATE: Duration = Duration::from_millis(520);

    /// Hover-in curve: quick to respond, gentle to land.
    pub const EASE_HOVER: Easing = Easing::Emphasized;
    /// Colour/opacity cross-fades.
    pub const EASE_TINT: Easing = Easing::Standard;
    /// Movement between two layouts.
    pub const EASE_MOVE: Easing = Easing::Emphasized;
    /// Entrance curves: start fast, settle.
    pub const EASE_ENTER: Easing = Easing::Emphasized;
    /// Exit curves: leave quickly so exits never block the next action.
    pub const EASE_EXIT: Easing = Easing::Accelerate;
    /// Playful overshoot, reserved for completion celebrations.
    pub const EASE_CELEBRATE: Easing = Easing::Overshoot;

    /// Frame interval used while any animation is in flight.
    pub const FRAME: Duration = Duration::from_millis(16);
    /// Idle refresh interval. Enough to keep the sweep and rings smooth
    /// without burning CPU.
    pub const IDLE: Duration = Duration::from_millis(64);
}

/// The type ramp. Sizes only; families come from [`super::fonts`].
pub mod type_scale {
    /// The hero clock on the Clock page.
    pub const DISPLAY: f32 = 104.0;
    /// Large readouts (timer, stopwatch).
    pub const METRIC_XL: f32 = 72.0;
    /// Medium readouts (world clock cards, compact timers).
    pub const METRIC: f32 = 34.0;
    /// Page titles.
    pub const TITLE: f32 = 24.0;
    /// Section headings and card titles.
    pub const HEADING: f32 = 17.0;
    /// Default body copy.
    pub const BODY: f32 = 14.0;
    /// Secondary body copy and control labels.
    pub const LABEL: f32 = 13.0;
    /// Metadata line.
    pub const CAPTION: f32 = 12.0;
    /// Uppercase section eyebrows.
    pub const MICRO: f32 = 11.0;

    /// Line heights paired with the sizes above.
    pub const DISPLAY_LH: f32 = 1.05;
    pub const METRIC_XL_LH: f32 = 1.1;
    pub const METRIC_LH: f32 = 1.2;
    pub const TITLE_LH: f32 = 1.25;
    pub const HEADING_LH: f32 = 1.35;
    pub const BODY_LH: f32 = 1.5;
    pub const LABEL_LH: f32 = 1.4;
    pub const CAPTION_LH: f32 = 1.4;
    pub const MICRO_LH: f32 = 1.2;

    /// Extra tracking for the big tabular readouts, in pixels.
    pub const DISPLAY_TRACKING: f32 = -2.0;
    /// Tracking for uppercase eyebrows.
    pub const MICRO_TRACKING: f32 = 1.1;
}

/// Font families.
///
/// OClock deliberately uses the platform's own UI and monospaced families:
/// like a first-party system application it inherits the desktop's
/// typography instead of shipping a bundled face. The monospaced family is
/// used wherever digits must not jitter, which is what keeps the readouts
/// from twitching every second.
pub mod fonts {
    use iced::font::{Family, Font, Weight};

    /// UI text.
    pub const UI: Font = Font {
        family: Family::SansSerif,
        weight: Weight::Normal,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    /// UI text, emphasised.
    pub const UI_BOLD: Font = Font {
        family: Family::SansSerif,
        weight: Weight::Bold,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    /// UI text, medium weight — the workhorse for labels and buttons.
    pub const UI_MEDIUM: Font = Font {
        family: Family::SansSerif,
        weight: Weight::Medium,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    /// UI text, semibold — page and card titles.
    pub const UI_SEMIBOLD: Font = Font {
        family: Family::SansSerif,
        weight: Weight::Semibold,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    /// Tabular readouts. Every digit shares one advance width, so a running
    /// clock never shifts sideways.
    pub const NUMERIC: Font = Font {
        family: Family::Monospace,
        weight: Weight::Normal,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    /// Tabular readouts, medium weight.
    pub const NUMERIC_MEDIUM: Font = Font {
        family: Family::Monospace,
        weight: Weight::Medium,
        stretch: iced::font::Stretch::Normal,
        style: iced::font::Style::Normal,
    };

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Every face in the ramp, with a name for the failure.
        const RAMP: [(&str, Font); 6] = [
            ("UI", UI),
            ("UI_BOLD", UI_BOLD),
            ("UI_MEDIUM", UI_MEDIUM),
            ("UI_SEMIBOLD", UI_SEMIBOLD),
            ("NUMERIC", NUMERIC),
            ("NUMERIC_MEDIUM", NUMERIC_MEDIUM),
        ];

        #[test]
        fn every_face_asks_for_a_family_rather_than_a_font() {
            // Naming a face would pin the interface to whichever machine
            // happened to ship that file. Asking for a family lets the shaper
            // pick, and — because it then falls back per character — lets it
            // pick a *second* face for a script the first one cannot draw,
            // which is the whole of Arabic, CJK, Thai and Cyrillic support.
            for (name, font) in RAMP {
                assert!(
                    matches!(font.family, Family::SansSerif | Family::Monospace),
                    "{name} names a font ({:?}) rather than a family",
                    font.family
                );
            }
        }

        #[test]
        fn the_ramp_asks_for_no_more_than_two_families() {
            let mut families: Vec<Family> = RAMP.iter().map(|(_, font)| font.family).collect();
            families.dedup();
            assert_eq!(
                families.len(),
                2,
                "the interface should be set in a proportional face and a tabular one"
            );
        }

        #[test]
        fn every_face_asks_for_an_upright_normal_width() {
            // A face that slants or stretches is a deliberate choice, and none
            // of these is one: everything here is upright, and every run of text
            // is measured against the same metrics.
            for (name, font) in RAMP {
                assert_eq!(
                    font.stretch,
                    iced::font::Stretch::Normal,
                    "{name} is not set at normal width"
                );
                assert_eq!(
                    font.style,
                    iced::font::Style::Normal,
                    "{name} is not set upright"
                );
            }
        }

        #[test]
        fn the_tabular_faces_are_the_only_monospaced_ones() {
            // Digits share one advance width so a running clock never shifts
            // sideways. A label set in the same face would look wrong, and a
            // clock set in a proportional one would drift.
            for (name, font) in RAMP {
                let tabular = matches!(font.family, Family::Monospace);
                assert_eq!(
                    tabular,
                    name.starts_with("NUMERIC"),
                    "{name} is in the wrong family for what it is for"
                );
            }
        }
    }
}

/// Layout constants for the application shell.
pub mod layout {
    /// Width of the navigation rail when there is room for it.
    pub const SIDEBAR: f32 = 236.0;
    /// Icon-only rail width used on narrow windows.
    pub const SIDEBAR_COMPACT: f32 = 68.0;
    /// Below this window width the shell switches to a top bar instead of a
    /// sidebar.
    pub const NARROW_BREAKPOINT: f32 = 760.0;
    /// Below this window width the sidebar drops its labels, leaving only the
    /// marks. Must be greater than [`NARROW_BREAKPOINT`], or the middle layout
    /// could never apply.
    pub const COMPACT_BREAKPOINT: f32 = 1040.0;
    /// Horizontal page padding.
    pub const PAGE_PADDING: f32 = 28.0;
    /// Vertical page padding.
    pub const PAGE_PADDING_Y: f32 = 24.0;
    /// Default window size.
    pub const WINDOW: (u32, u32) = (1180, 780);
    /// Smallest useful window size.
    pub const WINDOW_MIN: (u32, u32) = (680, 560);
}

/// Focus ring width, drawn for every keyboard-reachable control.
pub const FOCUS_RING: f32 = 2.0;

/// A cubic Bézier easing curve, solved the same way CSS solves
/// `cubic-bezier()`: Newton–Raphson with a bisection fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    /// Gentle in, gentle out. The workhorse.
    Standard,
    /// Fast start, slow landing. For entrances.
    Decelerate,
    /// Slow start, fast exit. For exits.
    Accelerate,
    /// A touch more kick than [`Easing::Decelerate`], for hovers.
    Emphasized,
    /// Overshoots past the target then settles. For celebrations.
    Overshoot,
}

impl Easing {
    /// The `(x1, y1, x2, y2)` control points of the curve.
    const fn points(self) -> (f32, f32, f32, f32) {
        match self {
            Easing::Linear => (0.0, 0.0, 1.0, 1.0),
            Easing::Standard => (0.2, 0.0, 0.0, 1.0),
            Easing::Decelerate => (0.0, 0.0, 0.15, 1.0),
            Easing::Accelerate => (0.3, 0.0, 0.8, 0.15),
            Easing::Emphasized => (0.05, 0.7, 0.1, 1.0),
            Easing::Overshoot => (0.34, 1.56, 0.64, 1.0),
        }
    }

    /// Evaluates the curve for `t` in `0..=1`.
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let (x1, y1, x2, y2) = self.points();

        if x1 == y1 && x2 == y2 {
            return t;
        }

        // Solve x(u) = t for u, then evaluate y(u).
        let u = solve(t, x1, x2);
        bezier(u, y1, y2)
    }
}

fn bezier(u: f32, p1: f32, p2: f32) -> f32 {
    // Polynomial Bernstein form of a 1-D cubic Bézier.
    let inv = 1.0 - u;
    3.0 * inv * inv * u * p1 + 3.0 * inv * u * u * p2 + u * u * u
}

fn bezier_derivative(u: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - u;
    3.0 * inv * inv * p1 + 6.0 * inv * u * (p2 - p1) + 3.0 * u * u * (1.0 - p2)
}

fn solve(x: f32, x1: f32, x2: f32) -> f32 {
    const EPS: f32 = 1.0e-5;

    // Newton–Raphson first; it converges in a handful of steps.
    let mut u = x;
    for _ in 0..8 {
        let err = bezier(u, x1, x2) - x;
        if err.abs() < EPS {
            return u;
        }
        let slope = bezier_derivative(u, x1, x2);
        if slope.abs() < EPS {
            break;
        }
        u -= err / slope;
    }

    // Bisection fallback keeps the curve monotonic even where the
    // derivative is flat.
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    u = x;
    for _ in 0..24 {
        if bezier(u, x1, x2) < x {
            lo = u;
        } else {
            hi = u;
        }
        u = 0.5 * (lo + hi);
    }
    u
}

#[cfg(test)]
mod tests {
    use super::Easing;

    fn assert_close(a: f32, b: f32, what: &str) {
        assert!(
            (a - b).abs() < 1.0e-3,
            "{what}: expected {b}, got {a} (delta {})",
            (a - b).abs()
        );
    }

    #[test]
    fn curves_hit_their_endpoints() {
        for easing in [
            Easing::Linear,
            Easing::Standard,
            Easing::Decelerate,
            Easing::Accelerate,
            Easing::Emphasized,
            Easing::Overshoot,
        ] {
            assert_close(easing.apply(0.0), 0.0, "t=0");
            assert_close(easing.apply(1.0), 1.0, "t=1");
        }
    }

    #[test]
    fn linear_is_the_identity() {
        for step in 0..=20 {
            let t = step as f32 / 20.0;
            assert_close(Easing::Linear.apply(t), t, "linear");
        }
    }

    #[test]
    fn curves_are_monotonic_in_the_middle() {
        for easing in [
            Easing::Standard,
            Easing::Decelerate,
            Easing::Accelerate,
            Easing::Emphasized,
        ] {
            let mut previous = 0.0;
            for step in 1..=20 {
                let t = step as f32 / 20.0;
                let value = easing.apply(t);
                assert!(
                    value >= previous,
                    "{easing:?} went backwards at t={t}: {previous} -> {value}"
                );
                previous = value;
            }
        }
    }

    #[test]
    fn accelerate_lags_and_decelerate_leads() {
        // A quarter of the way through, an exit has barely moved and an
        // entrance is nearly there. This is what lets OClock run a fast exit
        // alongside a slower, calmer entrance without special-casing either.
        let quarter = 0.25;
        assert!(Easing::Accelerate.apply(quarter) < 0.1);
        assert!(Easing::Decelerate.apply(quarter) > 0.55);
        assert!(Easing::Accelerate.apply(quarter) < Easing::Decelerate.apply(quarter));
    }

    #[test]
    fn overshoot_overshoots() {
        let mut peak: f32 = 0.0;
        for step in 0..=40 {
            peak = peak.max(Easing::Overshoot.apply(step as f32 / 40.0));
        }
        assert!(peak > 1.0, "expected an overshoot, peak was {peak}");
    }

    #[test]
    fn out_of_range_input_is_clamped() {
        assert_close(Easing::Standard.apply(-3.0), 0.0, "below");
        assert_close(Easing::Standard.apply(9.0), 1.0, "above");
    }
}
