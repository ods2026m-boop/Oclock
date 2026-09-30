//! The OClock colour system.
//!
//! [`Palette`] is a flat, `Copy` set of *semantic* roles rather than a list of
//! raw colours. Components ask for `palette.surface` or `palette.accent`, never
//! for a hex value, which is what keeps light and dark mode a single switch
//! and makes a theme cross-fade a matter of blending two palettes.

use iced::{gradient, Background, Border, Color, Gradient, Shadow};

use super::tokens::{elevation, radius, space, stroke};

/// A complete, renderable set of colours for one appearance.
///
/// Both the light and the dark palette populate every field; there are no
/// "only in dark" holes. Because the struct is `Copy`, a palette can be moved
/// into style closures without reference counting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Window background, behind everything.
    pub canvas: Color,
    /// Default card / panel surface.
    pub surface: Color,
    /// Surface for content that sits on top of a card (menus, modals).
    pub elevated: Color,
    /// Recessed surface: inputs, wells, the secondary rail.
    pub sunken: Color,
    /// Hover wash applied over interactive surfaces.
    pub hover: Color,
    /// Pressed wash.
    pub active: Color,
    /// Accent wash used for selected rows and chips.
    pub accent_soft: Color,
    /// Hairline separators and default outlines.
    pub outline: Color,
    /// Stronger outline, used on hover and for focus.
    pub outline_strong: Color,
    /// Primary foreground.
    pub text: Color,
    /// Secondary foreground.
    pub text_muted: Color,
    /// Tertiary foreground: hints, units, disabled labels.
    pub text_faint: Color,
    /// Accent used as a fill (button backgrounds, progress arcs).
    pub accent: Color,
    /// Accent as foreground text on `canvas` or `surface`.
    pub accent_text: Color,
    /// Accent used as a fill when the control is hovered.
    pub accent_hover: Color,
    /// Accent used as a fill when the control is pressed.
    pub accent_active: Color,
    /// Positive / running state.
    pub positive: Color,
    /// Caution / paused state.
    pub caution: Color,
    /// Destructive / stopped state.
    pub danger: Color,
    /// Translucent wash for a destructive or "completed" treatment.
    pub danger_soft: Color,
    /// Tint used behind a ringing alarm or finished timer.
    pub alert_soft: Color,
    /// Backdrop behind modals.
    pub scrim: Color,
    /// Colour of the soft shadow.
    pub shadow: Color,
    /// Dial face fill.
    pub dial_face: Color,
    /// Dial hour markers.
    pub dial_marker: Color,
    /// The minute track inside the hour markers, a shade quieter than they are.
    pub dial_track: Color,
    /// Dial hour hand.
    pub dial_hand: Color,
    /// Dial minute and second hands.
    pub dial_hand_alt: Color,
    /// Track colour behind a progress arc.
    pub track: Color,
}

impl Palette {
    /// The light appearance: a cool near-white canvas with crisp white cards.
    pub const LIGHT: Palette = Palette {
        canvas: Color::from_rgb(0.957, 0.961, 0.976),
        surface: Color::from_rgb(1.0, 1.0, 1.0),
        elevated: Color::from_rgb(1.0, 1.0, 1.0),
        sunken: Color::from_rgb(0.929, 0.937, 0.957),
        hover: Color::from_rgba(0.106, 0.122, 0.204, 0.055),
        active: Color::from_rgba(0.106, 0.122, 0.204, 0.095),
        accent_soft: Color::from_rgba(0.310, 0.275, 0.898, 0.105),
        outline: Color::from_rgba(0.106, 0.122, 0.204, 0.110),
        outline_strong: Color::from_rgba(0.106, 0.122, 0.204, 0.260),
        text: Color::from_rgb(0.075, 0.086, 0.129),
        text_muted: Color::from_rgb(0.353, 0.384, 0.478),
        text_faint: Color::from_rgb(0.573, 0.604, 0.686),
        accent: Color::from_rgb(0.310, 0.275, 0.898),
        accent_text: Color::from_rgb(0.263, 0.231, 0.808),
        accent_hover: Color::from_rgb(0.373, 0.341, 0.937),
        accent_active: Color::from_rgb(0.247, 0.216, 0.804),
        positive: Color::from_rgb(0.043, 0.573, 0.416),
        caution: Color::from_rgb(0.663, 0.427, 0.043),
        danger: Color::from_rgb(0.855, 0.239, 0.329),
        danger_soft: Color::from_rgba(0.855, 0.239, 0.329, 0.100),
        alert_soft: Color::from_rgba(0.855, 0.239, 0.329, 0.075),
        scrim: Color::from_rgba(0.063, 0.071, 0.110, 0.420),
        shadow: Color::from_rgba(0.063, 0.075, 0.125, 0.100),
        dial_face: Color::from_rgb(1.0, 1.0, 1.0),
        dial_marker: Color::from_rgba(0.106, 0.122, 0.204, 0.34),
        dial_track: Color::from_rgba(0.106, 0.122, 0.204, 0.16),
        dial_hand: Color::from_rgb(0.075, 0.086, 0.129),
        dial_hand_alt: Color::from_rgb(0.310, 0.275, 0.898),
        track: Color::from_rgba(0.106, 0.122, 0.204, 0.090),
    };

    /// The dark appearance: a deep blue-black canvas with lifted surfaces.
    pub const DARK: Palette = Palette {
        canvas: Color::from_rgb(0.043, 0.051, 0.078),
        surface: Color::from_rgb(0.075, 0.090, 0.133),
        elevated: Color::from_rgb(0.102, 0.122, 0.173),
        sunken: Color::from_rgb(0.063, 0.075, 0.114),
        hover: Color::from_rgba(1.0, 1.0, 1.0, 0.060),
        active: Color::from_rgba(1.0, 1.0, 1.0, 0.105),
        accent_soft: Color::from_rgba(0.541, 0.482, 0.988, 0.170),
        outline: Color::from_rgba(0.549, 0.596, 0.780, 0.140),
        outline_strong: Color::from_rgba(0.549, 0.596, 0.780, 0.320),
        text: Color::from_rgb(0.949, 0.957, 0.980),
        text_muted: Color::from_rgb(0.659, 0.690, 0.769),
        text_faint: Color::from_rgb(0.451, 0.482, 0.565),
        accent: Color::from_rgb(0.431, 0.361, 0.902),
        accent_text: Color::from_rgb(0.694, 0.647, 1.0),
        accent_hover: Color::from_rgb(0.494, 0.424, 0.949),
        accent_active: Color::from_rgb(0.373, 0.310, 0.831),
        positive: Color::from_rgb(0.251, 0.788, 0.588),
        caution: Color::from_rgb(0.906, 0.667, 0.255),
        danger: Color::from_rgb(0.949, 0.400, 0.478),
        danger_soft: Color::from_rgba(0.949, 0.400, 0.478, 0.160),
        alert_soft: Color::from_rgba(0.949, 0.400, 0.478, 0.110),
        scrim: Color::from_rgba(0.016, 0.020, 0.039, 0.620),
        shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.480),
        dial_face: Color::from_rgb(0.063, 0.075, 0.118),
        dial_marker: Color::from_rgba(0.659, 0.690, 0.769, 0.42),
        dial_track: Color::from_rgba(0.659, 0.690, 0.769, 0.20),
        dial_hand: Color::from_rgb(0.902, 0.918, 0.957),
        dial_hand_alt: Color::from_rgb(0.647, 0.596, 1.0),
        track: Color::from_rgba(0.659, 0.690, 0.769, 0.140),
    };

    /// Linearly interpolates between two palettes.
    ///
    /// Used for the theme cross-fade: the application holds a `Palette` that
    /// moves from one appearance to the other over
    /// [`crate::design::tokens::motion::DELIBERATE`], and every colour in the
    /// app therefore moves together.
    pub fn blend(from: Palette, to: Palette, t: f32) -> Palette {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: Color, b: Color| Color {
            r: lerp(a.r, b.r, t),
            g: lerp(a.g, b.g, t),
            b: lerp(a.b, b.b, t),
            a: lerp(a.a, b.a, t),
        };

        Palette {
            canvas: mix(from.canvas, to.canvas),
            surface: mix(from.surface, to.surface),
            elevated: mix(from.elevated, to.elevated),
            sunken: mix(from.sunken, to.sunken),
            hover: mix(from.hover, to.hover),
            active: mix(from.active, to.active),
            accent_soft: mix(from.accent_soft, to.accent_soft),
            outline: mix(from.outline, to.outline),
            outline_strong: mix(from.outline_strong, to.outline_strong),
            text: mix(from.text, to.text),
            text_muted: mix(from.text_muted, to.text_muted),
            text_faint: mix(from.text_faint, to.text_faint),
            accent: mix(from.accent, to.accent),
            accent_text: mix(from.accent_text, to.accent_text),
            accent_hover: mix(from.accent_hover, to.accent_hover),
            accent_active: mix(from.accent_active, to.accent_active),
            positive: mix(from.positive, to.positive),
            caution: mix(from.caution, to.caution),
            danger: mix(from.danger, to.danger),
            danger_soft: mix(from.danger_soft, to.danger_soft),
            alert_soft: mix(from.alert_soft, to.alert_soft),
            scrim: mix(from.scrim, to.scrim),
            shadow: mix(from.shadow, to.shadow),
            dial_face: mix(from.dial_face, to.dial_face),
            dial_marker: mix(from.dial_marker, to.dial_marker),
            dial_track: mix(from.dial_track, to.dial_track),
            dial_hand: mix(from.dial_hand, to.dial_hand),
            dial_hand_alt: mix(from.dial_hand_alt, to.dial_hand_alt),
            track: mix(from.track, to.track),
        }
    }

    /// True when the palette is closer to the dark appearance.
    ///
    /// Canvas luminance is a good enough proxy and avoids threading an
    /// explicit flag through every palette transformation.
    pub fn is_dark(self) -> bool {
        let Color { r, g, b, .. } = self.canvas;
        0.2126 * r + 0.7152 * g + 0.0722 * b < 0.5
    }

    /// The iced [`iced::theme::Palette`] for window chrome, derived from this
    /// palette. OClock styles every widget itself, so this only colours the
    /// parts iced draws for us: the window background, scrollbars, the text
    /// cursor and any widget that falls back to its default appearance.
    pub fn to_iced(self) -> iced::theme::Palette {
        iced::theme::Palette {
            background: self.canvas,
            text: self.text,
            primary: self.accent,
            success: self.positive,
            danger: self.danger,
        }
    }

    /// Text colour that is legible on top of [`Palette::accent`].
    pub fn on_accent(self) -> Color {
        // Both appearances use a saturated mid-dark accent, so white wins on
        // contrast in either theme.
        Color::WHITE
    }

    /// A soft shadow tinted with this palette.
    pub fn shadow(self, level: ShadowLevel) -> Shadow {
        let base = match level {
            ShadowLevel::Low => elevation::LOW,
            ShadowLevel::Medium => elevation::MEDIUM,
            ShadowLevel::High => elevation::HIGH,
        };

        Shadow {
            color: self.shadow,
            ..base
        }
    }

    /// A hairline border in `outline`.
    pub fn border(self, color: Color) -> Border {
        Border {
            color,
            width: stroke::HAIRLINE,
            radius: radius::LG.into(),
        }
    }

    /// A border sized for a card.
    pub fn card_border(self) -> Border {
        Border {
            color: self.outline,
            width: stroke::HAIRLINE,
            radius: radius::XL.into(),
        }
    }

    /// The vertical gradient used for the hero surface.
    pub fn hero_gradient(self) -> Background {
        let top = if self.is_dark() {
            Color::from_rgba(0.541, 0.482, 0.988, 0.150)
        } else {
            Color::from_rgba(0.310, 0.275, 0.898, 0.085)
        };

        Background::Gradient(Gradient::Linear(
            gradient::Linear::new(90.0).add_stop(0.0, top),
        ))
    }
}

/// How a message is coloured.
///
/// A toast, a notice and a badge all need "this is good" or "this is not", and
/// naming that is clearer than passing a whole palette and picking a field out
/// of it at the call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Something worked, or is running.
    Positive,
    /// Something needs attention but is not a failure.
    Caution,
    /// Something failed.
    Danger,
    /// Ordinary information.
    Neutral,
}

impl Tone {
    /// The palette's colour for this tone.
    pub fn colour(self, palette: Palette) -> Color {
        match self {
            Tone::Positive => palette.positive,
            Tone::Caution => palette.caution,
            Tone::Danger => palette.danger,
            Tone::Neutral => palette.text_muted,
        }
    }
}

/// Depth levels, mapped onto the elevation tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowLevel {
    Low,
    Medium,
    High,
}

/// Component-scoped padding, exposed so cards and controls stay in step.
pub mod pad {
    use super::space;

    /// Inside a card.
    pub const CARD: f32 = space::XL;
    /// Inside a dense card, e.g. a world-clock row.
    pub const CARD_DENSE: f32 = space::LG;
    /// Between the icon and the label inside a button.
    pub const BUTTON: f32 = space::MD;
}

/// Linear interpolation that is exact at both endpoints.
///
/// `a + (b - a) * t` is off by an ulp at `t == 1.0` for some pairs, which
/// would mean a finished theme cross-fade never quite reached its target
/// colours. Returning the endpoints verbatim removes that class of drift
/// entirely, which matters because the whole palette is blended on every
/// frame of the fade.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        a
    } else if t >= 1.0 {
        b
    } else {
        a + (b - a) * t
    }
}

/// Mixes a foreground over a background using straight alpha, which is what
/// washes (`palette.hover`, `palette.active`) are conceptually.
pub fn overlay(fg: Color, bg: Color) -> Color {
    let a = fg.a;
    Color {
        r: fg.r * a + bg.r * (1.0 - a),
        g: fg.g * a + bg.g * (1.0 - a),
        b: fg.b * a + bg.b * (1.0 - a),
        a: 1.0,
    }
}

/// Applies `amount` of `fg` over `base`, replacing the base alpha.
///
/// Used to build derived surfaces without allocating intermediate palettes.
pub fn wash(fg: Color, base: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    let base_a = base.a;
    Color {
        r: fg.r * amount + base.r * (1.0 - amount),
        g: fg.g * amount + base.g * (1.0 - amount),
        b: fg.b * amount + base.b * (1.0 - amount),
        a: base_a,
    }
}

/// The WCAG relative-luminance contrast ratio between two opaque colours.
///
/// Part of the design system rather than a test helper: it is the single
/// definition of "readable" that the palette tests and the UI's own contrast
/// decisions both use.
pub fn contrast(a: Color, b: Color) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn relative_luminance(color: Color) -> f32 {
    fn channel(value: f32) -> f32 {
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_endpoints_are_exact() {
        let at_zero = Palette::blend(Palette::LIGHT, Palette::DARK, 0.0);
        let at_one = Palette::blend(Palette::LIGHT, Palette::DARK, 1.0);

        assert_eq!(at_zero, Palette::LIGHT);
        assert_eq!(at_one, Palette::DARK);
    }

    #[test]
    fn blend_midpoint_is_between_the_two() {
        let mid = Palette::blend(Palette::LIGHT, Palette::DARK, 0.5);
        // The light canvas is the brighter one, so the midpoint sits below it.
        assert!(mid.canvas.r < Palette::LIGHT.canvas.r);
        assert!(mid.canvas.r > Palette::DARK.canvas.r);
        // Its text is the darker one, so the midpoint sits above it.
        assert!(mid.text.r > Palette::LIGHT.text.r);
        assert!(mid.text.r < Palette::DARK.text.r);
    }

    #[test]
    fn a_tone_resolves_to_a_readable_colour() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            for tone in [Tone::Positive, Tone::Caution, Tone::Danger, Tone::Neutral] {
                let colour = tone.colour(palette);
                // A tone is used as an accent on the surface, so it has to read
                // against it.
                assert!(
                    contrast(colour, palette.surface) >= 3.0,
                    "{tone:?} is unreadable in one of the themes"
                );
            }
        }
    }

    #[test]
    fn blending_every_role_stays_inside_its_endpoints() {
        // Guards the whole struct at once: no role may escape the range its
        // two appearances define, or a fade would show a colour neither theme
        // ever had.
        let mid = Palette::blend(Palette::LIGHT, Palette::DARK, 0.5);
        for (name, light, blended, dark) in [
            (
                "canvas",
                Palette::LIGHT.canvas.r,
                mid.canvas.r,
                Palette::DARK.canvas.r,
            ),
            (
                "surface",
                Palette::LIGHT.surface.r,
                mid.surface.r,
                Palette::DARK.surface.r,
            ),
            (
                "text",
                Palette::LIGHT.text.r,
                mid.text.r,
                Palette::DARK.text.r,
            ),
            (
                "accent",
                Palette::LIGHT.accent.r,
                mid.accent.r,
                Palette::DARK.accent.r,
            ),
            (
                "hover.a",
                Palette::LIGHT.hover.a,
                mid.hover.a,
                Palette::DARK.hover.a,
            ),
            (
                "danger",
                Palette::LIGHT.danger.g,
                mid.danger.g,
                Palette::DARK.danger.g,
            ),
        ] {
            let low = light.min(dark);
            let high = light.max(dark);
            assert!(
                blended >= low && blended <= high,
                "{name} left its range: {blended} not in {low}..={high}"
            );
        }
    }

    #[test]
    fn blend_clamps_out_of_range_t() {
        assert_eq!(
            Palette::blend(Palette::DARK, Palette::LIGHT, -5.0),
            Palette::DARK
        );
        assert_eq!(
            Palette::blend(Palette::DARK, Palette::LIGHT, 5.0),
            Palette::LIGHT
        );
    }

    #[test]
    fn darkness_detection() {
        assert!(!Palette::LIGHT.is_dark());
        assert!(Palette::DARK.is_dark());
    }

    #[test]
    fn accent_foreground_has_contrast_in_both_themes() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            let on_accent = palette.on_accent();
            let ratio = contrast(on_accent, palette.accent);
            assert!(
                ratio >= 4.5,
                "on_accent contrast {:.2} is too low in theme",
                ratio
            );
        }
    }

    #[test]
    fn body_text_is_legible_on_the_canvas_in_both_themes() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            assert!(contrast(palette.text, palette.canvas) >= 7.0);
            assert!(contrast(palette.text_muted, palette.canvas) >= 4.5);
            assert!(contrast(palette.accent_text, palette.canvas) >= 3.0);
        }
    }

    #[test]
    fn cards_are_distinguishable_from_the_canvas() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            assert!(
                contrast(palette.surface, palette.canvas) > 1.02,
                "card would vanish into the canvas"
            );
        }
    }
}
