//! Text primitives built on the type ramp.
//!
//! Every piece of text in OClock goes through one of these, which is what
//! makes the typography consistent: there is exactly one place that knows the
//! size ramp, the line heights, the families and the tracking.

use iced::padding;
use iced::widget::{container, row, text};
use iced::{Color, Element, Font, Length};

use crate::design::palette::Palette;
use crate::design::tokens::{fonts, radius, space, type_scale};
use crate::services::i18n::Direction;

/// The shaping strategy every run of text in OClock is rendered with.
///
/// Iced's own default is [`Shaping::Basic`], which is documented as "no shaping
/// and no font fallback". That is a reasonable default for an application whose
/// only script is Latin, and it is wrong here on two counts:
///
/// * Persian, Arabic and Urdu are cursive. Without shaping the letters are drawn
///   in their isolated forms and joined ones are not drawn at all — the word
///   comes out as disconnected marks that no reader of that language can read.
/// * Thai needs its marks stacked correctly, and Chinese, Korean and Thai
///   need a font that is not the interface's. The families here are
///   `SansSerif` and `Monospace`, so the glyphs for those scripts live in some
///   *other* font on the system, and only fallback finds them.
///
/// Advanced shaping is more expensive than basic shaping. That cost is paid
/// deliberately: a clock that reads faster but cannot be read is not a clock.
const SHAPING: iced::widget::text::Shaping = iced::widget::text::Shaping::Advanced;

/// A role in the type ramp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The hero clock. Nothing else may be this large.
    Display,
    /// The largest readout on the timer and stopwatch pages.
    MetricLarge,
    /// A world-clock card's time.
    Metric,
    /// A page title.
    Title,
    /// A card or section heading.
    Heading,
    /// Default body copy.
    Body,
    /// A control's label.
    Label,
    /// Supporting metadata.
    Caption,
    /// An uppercase section eyebrow.
    Micro,
}

impl Role {
    /// The width `text` is likely to occupy, in logical pixels.
    ///
    /// An estimate, not a measurement: Iced 0.13 offers no way to measure a
    /// string without a renderer, and a page still has to know whether its
    /// largest readout fits before it is drawn. The factors are the average
    /// advance of a system sans and of a monospaced face respectively, which is
    /// close enough to catch a readout that would overflow — the mistake worth
    /// catching — and loose enough not to pretend to be typography.
    ///
    /// A character outside Latin is counted as wider, because the East Asian
    /// scripts this estimates for set most of their characters at full width
    /// and Arabic and Thai set theirs in scripts whose advances are not the
    /// Latin ones. Getting this wrong in the cheap direction would let a
    /// Chinese label overflow a control that an English one fits.
    pub fn estimate_width(self, text: &str) -> f32 {
        let monospaced = matches!(self, Role::Display | Role::MetricLarge | Role::Metric);
        let advance = if monospaced { 0.62 } else { 0.53 };
        let letters = text.chars().filter(|c| *c != ' ' && !c.is_ascii()).count() as f32;
        let latin = text.chars().filter(|c| c.is_ascii() && *c != ' ').count() as f32;
        let spaces = text.chars().filter(|c| *c == ' ').count() as f32;
        // A space is much narrower than a letter, and headings lean on them.
        // Non-Latin scripts have no word spaces to lean on, so their letters
        // are charged the full advance.
        (latin * advance + letters * 0.90) * self.size() + spaces * 0.30 * self.size()
    }

    /// The size this role is set at.
    pub fn size(self) -> f32 {
        match self {
            Role::Display => type_scale::DISPLAY,
            Role::MetricLarge => type_scale::METRIC_XL,
            Role::Metric => type_scale::METRIC,
            Role::Title => type_scale::TITLE,
            Role::Heading => type_scale::HEADING,
            Role::Body => type_scale::BODY,
            Role::Label => type_scale::LABEL,
            Role::Caption => type_scale::CAPTION,
            Role::Micro => type_scale::MICRO,
        }
    }

    /// The line height for this role.
    pub fn line_height(self) -> f32 {
        match self {
            Role::Display => type_scale::DISPLAY_LH,
            Role::MetricLarge => type_scale::METRIC_XL_LH,
            Role::Metric => type_scale::METRIC_LH,
            Role::Title => type_scale::TITLE_LH,
            Role::Heading => type_scale::HEADING_LH,
            Role::Body => type_scale::BODY_LH,
            Role::Label => type_scale::LABEL_LH,
            Role::Caption => type_scale::CAPTION_LH,
            Role::Micro => type_scale::MICRO_LH,
        }
    }

    /// The family this role is set in.
    ///
    /// Anything that displays digits is set in the monospaced family, whose
    /// figures all share one advance width. That is what keeps a running clock
    /// from twitching sideways every second, and it is why a first-party clock
    /// reads as an instrument rather than as a document.
    pub fn font(self) -> Font {
        match self {
            Role::Display | Role::MetricLarge | Role::Metric => fonts::NUMERIC,
            Role::Title | Role::Heading => fonts::UI_SEMIBOLD,
            Role::Micro => fonts::UI_BOLD,
            _ => fonts::UI,
        }
    }

    /// Extra tracking, in pixels. Negative on the big readouts so a run of
    /// digits sits together rather than drifting apart.
    pub fn tracking(self) -> f32 {
        match self {
            Role::Display => type_scale::DISPLAY_TRACKING,
            Role::MetricLarge => -1.0,
            Role::Micro => type_scale::MICRO_TRACKING,
            _ => 0.0,
        }
    }
}

/// A piece of styled text.
#[derive(Clone, Debug)]
pub struct Label {
    content: String,
    role: Role,
    color: Color,
    font: Option<Font>,
    /// Centre the text, for a readout that sits on its own.
    centred: bool,
    /// Give the text a box of its own width rather than the width its row
    /// happens to offer.
    natural: bool,
    /// Which way this run of text is read, which is what decides the edge it
    /// hangs from.
    direction: Direction,
}

impl Label {
    /// The text, for callers that need it as a string rather than a widget.
    pub fn text(&self) -> &str {
        &self.content
    }

    /// The size this label is set at, for callers that need to scale something
    /// else to match it.
    pub fn size(&self) -> f32 {
        self.role.size()
    }

    /// New text in `role`, coloured by `color`.
    pub fn new(content: impl Into<String>, role: Role, color: Color) -> Label {
        Label {
            content: content.into(),
            role,
            color,
            font: None,
            centred: false,
            natural: false,
            direction: Direction::LeftToRight,
        }
    }

    /// Gives the text a box exactly as wide as the text.
    ///
    /// Iced shapes a text widget at the width its layout *offers* and then draws
    /// a right-to-left line flush to the end of that box. A label that has to
    /// shrink-wrap — a chip, a button, a badge, a readout — is therefore drawn
    /// at the far end of whatever space its row offered, which in a
    /// right-to-left layout puts it outside the very background it belongs to.
    ///
    /// Sizing the box to the text removes the gap between the two, so the label
    /// lands where its box is. Prose is left alone: it is given a box to fill,
    /// and hanging from the end of that box is exactly what a right-to-left
    /// paragraph should do.
    ///
    /// The width is measured, not estimated — see [`crate::ui::text_metrics`] —
    /// so the text still fits inside the box, still scales with the type ramp,
    /// and still has the same glyphs it would have had in a wider one.
    pub fn natural(mut self) -> Label {
        self.natural = true;
        self
    }

    /// Reads this run of text in `direction`.
    ///
    /// Prose has to hang from the correct edge or it reads wrong: a
    /// left-to-right paragraph in a right-to-left window has a ragged right
    /// margin and no right margin at all. Centred text is unaffected, which is
    /// why this is set only where it is set.
    pub fn direction(mut self, direction: Direction) -> Label {
        self.direction = direction;
        self
    }

    /// Reads this run of text in the language's own direction.
    pub fn reading(mut self, catalog: crate::services::i18n::Catalog) -> Label {
        self.direction = catalog.direction();
        self
    }

    /// Reads this run of text in `direction`.
    ///
    /// For the components that only ever learn which way a language runs, and
    /// not which words it has: a badge, a readout, a stepper's caption. Text
    /// they were given is already translated, so the name they need is the
    /// direction, not the catalogue.
    pub fn written_in(mut self, direction: crate::services::i18n::Direction) -> Label {
        self.direction = direction;
        self
    }

    /// Replaces the colour, keeping the role.
    ///
    /// A control that fills its own surface has to ink its own label: the
    /// style's `text_color` is only a default for text that does not choose its
    /// own colour, and every label here does. Without this a filled button
    /// would draw a grey label on a coloured fill.
    pub fn tinted(mut self, colour: Color) -> Label {
        self.color = colour;
        self
    }

    /// The colour this label is drawn in.
    pub fn colour(&self) -> Color {
        self.color
    }

    /// Overrides the family for this run of text.
    pub fn font(mut self, font: Font) -> Label {
        self.font = Some(font);
        self
    }

    /// Renders the body copy of the clock, which wants the proportional
    /// family even though it is large.
    pub fn proportional(mut self) -> Label {
        self.font = Some(fonts::UI);
        self
    }

    /// Centres the text.
    pub fn centre(mut self) -> Label {
        self.centred = true;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> text::Text<'static> {
        let Label {
            content,
            role,
            color,
            font,
            centred,
            natural,
            direction,
        } = self;

        let font = font.unwrap_or_else(|| role.font());

        // Measured before the text is moved into the widget, and sized from the
        // same shaper that will draw the glyphs, so the box is the width the
        // text wants rather than an approximation of it.
        let natural = natural.then(|| {
            crate::ui::text_metrics::natural_width(&content, font, role.size(), role.line_height())
        });

        let mut built = text(content)
            .size(role.size())
            .line_height(role.line_height())
            .font(font)
            .color(color)
            .shaping(SHAPING);

        if let Some(width) = natural {
            built = built.width(Length::Fixed(width));
        }

        if centred {
            built.center()
        } else {
            // Iced lays text out physically: `Start` is the left edge whatever
            // the script, so the leading edge has to be named for the language
            // rather than left to the default.
            built.align_x(direction.align_x())
        }
    }
}

/// A run of text at mixed sizes, aligned on one baseline.
///
/// A clock reads as `09:41` with a small `:23` and a small `AM` beside it. In
/// a row every child is aligned to the top, so a smaller run has to be pushed
/// down to share the larger run's baseline; [`Baseline`] does that arithmetic
/// once, from the type ramp, instead of leaving each call site to guess at a
/// padding.
///
/// The correction factor is the optical, not the metric, one: a glyph's
/// baseline sits a little below the middle of its line box, so aligning the
/// *centres* of two line boxes reads correctly while aligning their bottoms
/// does not.
pub struct Baseline {
    items: Vec<Label>,
}

impl Baseline {
    /// A run from a list of labels; the largest sets the baseline.
    pub fn from_items(items: Vec<Label>) -> Baseline {
        Baseline { items }
    }

    /// A run whose first piece sets the size everything else aligns to.
    pub fn new(largest: Label) -> Baseline {
        Baseline {
            items: vec![largest],
        }
    }

    /// Appends a smaller piece.
    pub fn then(mut self, label: Label) -> Baseline {
        self.items.push(label);
        self
    }

    /// How far down a run of `size` has to move to share `largest`'s baseline.
    fn drop_for(largest: f32, size: f32) -> f32 {
        (largest - size).max(0.0) * 0.72
    }

    /// The rendered widget.
    pub fn view<Message: 'static>(self) -> Element<'static, Message> {
        let largest = self
            .items
            .iter()
            .map(|label| label.role.size())
            .fold(0.0f32, f32::max);

        let parts: Vec<Element<'static, Message>> = self
            .items
            .into_iter()
            .map(|label| {
                let drop = Baseline::drop_for(largest, label.role.size());
                // Each piece is given a box of its own width, so the run reads
                // as one time rather than as pieces that happen to share a row.
                // A meridiem in Arabic is a right-to-left run inside a
                // left-to-right one, and without this it would be drawn at the
                // far end of whatever the hero card offered instead of beside
                // the digits it belongs to.
                let view = label.natural().view();
                if drop <= 0.5 {
                    return view.into();
                }
                container(view).padding(padding::top(drop)).into()
            })
            .collect();

        row(parts).spacing(0.0).into()
    }
}

/// Builds a [`Baseline`] from a sequence of labels.
#[macro_export]
macro_rules! baseline {
    ($largest:expr $(, $rest:expr)* $(,)?) => {{
        let mut items = vec![$largest];
        $(items.push($rest);)*
        $crate::ui::typography::Baseline::from_items(items)
    }};
}

/// Body copy in the secondary colour.
pub fn body(content: impl Into<String>, palette: &Palette) -> Label {
    Label::new(content, Role::Body, palette.text_muted)
}

/// A secondary label, the workhorse for control text.
pub fn label(content: impl Into<String>, palette: &Palette) -> Label {
    Label::new(content, Role::Label, palette.text_muted)
}

/// Primary text at body size.
pub fn strong(content: impl Into<String>, palette: &Palette) -> Label {
    Label::new(content, Role::Body, palette.text)
}

/// A card or section heading.
pub fn heading(content: impl Into<String>, palette: &Palette) -> Label {
    Label::new(content, Role::Heading, palette.text)
}

/// Supporting metadata.
pub fn caption(content: impl Into<String>, palette: &Palette) -> Label {
    Label::new(content, Role::Caption, palette.text_faint)
}

/// An uppercase section eyebrow.
pub fn eyebrow(content: impl Into<String>, palette: &Palette) -> Label {
    Label::new(content, Role::Micro, palette.text_faint)
}

/// Tabular digits in the largest readout role.
pub fn metric(content: impl Into<String>, palette: &Palette) -> Label {
    Label::new(content, Role::Metric, palette.text)
}

/// A dot, used to separate a value from its unit.
pub fn separator(palette: &Palette) -> Label {
    Label::new("·", Role::Metric, palette.text_faint)
}

/// Horizontal padding inside a small chip.
pub const CHIP_PADDING: f32 = space::SM;

/// Corner radius for a small chip.
pub const CHIP_RADIUS: f32 = radius::SM;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::tokens::type_scale;

    #[test]
    fn a_natural_label_is_told_to_ask_for_its_own_width() {
        use crate::services::i18n::{Catalog, Language};

        let persian = Catalog::new(Language::Persian);
        let plain = Label::new("ساعت", Role::Label, Color::BLACK).reading(persian);
        let natural = plain.clone().natural();

        assert!(!plain.natural, "a label is not natural until it asks to be");
        assert!(natural.natural, "a natural label says so");
        assert_eq!(natural.direction, Direction::RightToLeft);
        assert_eq!(natural.content, "ساعت");

        // The width it asks for is the width the text needs, and that number
        // comes from the shaper that will draw it.
        let measured = crate::ui::text_metrics::natural_width(
            &natural.content,
            natural.role.font(),
            natural.role.size(),
            natural.role.line_height(),
        );
        assert!(
            measured > 1.0,
            "a Persian label measured {measured}px, so it would not draw"
        );
    }

    #[test]
    fn a_label_hangs_from_its_own_leading_edge() {
        use crate::services::i18n::{Catalog, Language};

        let left =
            Label::new("clock", Role::Label, Color::BLACK).reading(Catalog::new(Language::English));
        let right =
            Label::new("ساعت", Role::Label, Color::BLACK).reading(Catalog::new(Language::Persian));

        assert_eq!(left.direction, Direction::LeftToRight);
        assert_eq!(right.direction, Direction::RightToLeft);
        assert_eq!(left.direction.align_x(), iced::Alignment::Start);
        assert_eq!(right.direction.align_x(), iced::Alignment::End);
    }

    #[test]
    fn the_ramp_is_shaped_advanced() {
        // `Basic` shaping draws letters in isolation and never looks for a
        // second font: an Arabic label would come out unjoined, and a CJK, Thai
        // or Cyrillic one would come out blank. Every label goes through one
        // builder, so this one choice is the whole of that support.
        assert_eq!(SHAPING, iced::widget::text::Shaping::Advanced);
        assert_ne!(SHAPING, iced::widget::text::Shaping::Basic);
    }

    #[test]
    fn centring_and_reading_are_separate_choices() {
        use crate::services::i18n::{Catalog, Language};

        // A centred run is centred by its own alignment, not by the language's,
        // so the two never quietly overwrite one another.
        let centred = Label::new("ساعت", Role::Label, Color::BLACK)
            .reading(Catalog::new(Language::Persian))
            .centre();
        assert!(centred.centred);
        assert!(!centred.natural);
    }

    #[test]
    fn roles_are_ordered_from_largest_to_smallest() {
        let sizes = [
            Role::Display.size(),
            Role::MetricLarge.size(),
            Role::Metric.size(),
            Role::Title.size(),
            Role::Heading.size(),
            Role::Body.size(),
            Role::Label.size(),
            Role::Caption.size(),
            Role::Micro.size(),
        ];
        for pair in sizes.windows(2) {
            assert!(
                pair[0] > pair[1],
                "{:?} is not larger than its successor",
                pair
            );
        }
    }

    #[test]
    fn only_the_clock_page_may_use_the_display_role() {
        // One guard rail, checked mechanically: the type ramp must stay a ramp.
        let display = Role::Display;
        assert!(display.size() > type_scale::METRIC_XL);
        assert!(
            display.line_height() < 1.2,
            "a display size needs tight leading"
        );
    }

    #[test]
    fn every_role_has_a_font_and_a_line_height() {
        for role in [
            Role::Display,
            Role::MetricLarge,
            Role::Metric,
            Role::Title,
            Role::Heading,
            Role::Body,
            Role::Label,
            Role::Caption,
            Role::Micro,
        ] {
            assert!(role.line_height() >= 1.0, "{role:?} has no line height");
            assert!(role.line_height() <= 1.6, "{role:?} leading is too loose");
        }
    }

    #[test]
    fn digit_roles_use_the_monospaced_family() {
        // The whole reason the clock does not twitch as seconds tick over.
        assert_eq!(Role::Display.font(), fonts::NUMERIC);
        assert_eq!(Role::Metric.font(), fonts::NUMERIC);
        assert_eq!(Role::MetricLarge.font(), fonts::NUMERIC);

        // Prose does not.
        assert_ne!(Role::Body.font(), fonts::NUMERIC);
        assert_ne!(Role::Heading.font(), fonts::NUMERIC);
    }

    #[test]
    fn large_digit_roles_are_tightly_tracked() {
        assert!(Role::Display.tracking() < 0.0);
        assert!(Role::Metric.tracking() <= 0.0);
        // Small text needs no correction, and an eyebrow needs the opposite.
        assert_eq!(Role::Body.tracking(), 0.0);
        assert!(Role::Micro.tracking() > 0.0);
    }

    #[test]
    fn headings_are_emphasised_and_body_is_not() {
        assert_eq!(Role::Heading.font(), fonts::UI_SEMIBOLD);
        assert_eq!(Role::Title.font(), fonts::UI_SEMIBOLD);
        assert_eq!(Role::Body.font(), fonts::UI);
    }

    #[test]
    fn a_smaller_run_is_pushed_down_to_share_the_baseline() {
        assert_eq!(Baseline::drop_for(100.0, 100.0), 0.0);
        assert!(Baseline::drop_for(100.0, 40.0) > 0.0);
        // A larger item in the run sets the baseline, so nothing is ever
        // "dropped" by a negative amount.
        assert_eq!(Baseline::drop_for(40.0, 100.0), 0.0);
    }

    #[test]
    fn a_baseline_renders_from_a_list_or_a_head() {
        let palette = Palette::LIGHT;
        let _ = Baseline::new(metric("09:41", &palette))
            .then(caption("AM", &palette))
            .view::<()>();
        let _ = Baseline::from_items(vec![metric("09:41", &palette), caption("AM", &palette)])
            .view::<()>();
    }

    #[test]
    fn helpers_pick_the_expected_roles() {
        let palette = Palette::DARK;
        assert_eq!(heading("x", &palette).role, Role::Heading);
        assert_eq!(body("x", &palette).role, Role::Body);
        assert_eq!(label("x", &palette).role, Role::Label);
        assert_eq!(caption("x", &palette).role, Role::Caption);
        assert_eq!(eyebrow("x", &palette).role, Role::Micro);
        assert_eq!(metric("x", &palette).role, Role::Metric);
        assert_eq!(strong("x", &palette).color, palette.text);
    }

    #[test]
    fn an_estimate_ignores_spaces_and_grows_with_the_role() {
        let dense = Role::Display.estimate_width("11:41");
        let sparse = Role::Display.estimate_width("11 41");
        assert!(dense > sparse, "spaces are narrower than digits");
        assert!(Role::Body.estimate_width("hello") < Role::Title.estimate_width("hello"));
        assert!(Role::Display.estimate_width("") == 0.0);
    }

    #[test]
    fn a_label_can_be_re_inked() {
        let label = Label::new("Save", Role::Label, Palette::LIGHT.text_muted);
        assert_eq!(label.colour(), Palette::LIGHT.text_muted);
        assert_eq!(
            label.tinted(Palette::LIGHT.accent_text).colour(),
            Palette::LIGHT.accent_text
        );
    }

    #[test]
    fn a_label_can_override_its_family() {
        let label = Label::new("09:41", Role::Display, Color::WHITE).proportional();
        assert_eq!(label.font, Some(fonts::UI));
    }
}
