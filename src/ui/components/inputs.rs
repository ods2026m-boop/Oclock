//! Text entry, steppers and segmented choices.
//!
//! Time entry is the fiddly part of a clock application, so it gets its own
//! control: a stepper that steps by sensible amounts (an hour at a time for
//! hours, five minutes for minutes, one minute for seconds) and a plain
//! numeric field where a whole duration is typed at once.

use iced::widget::text_input::{self, TextInput};
use iced::widget::Column;
use iced::{Background, Border, Color, Element, Length};

use chrono::Weekday;

use crate::design::motion::{FrameClock, Spec};
use crate::design::palette::Palette;
use crate::design::tokens::{fonts, radius, space, stroke, FOCUS_RING};
use crate::services::i18n::{Catalog, Direction, Text};
use crate::ui::components::icons::Icon;
use crate::ui::components::interaction::Interaction;
use crate::ui::typography::{self, Label, Role};

/// A left-to-right mark: invisible, zero-width, and only ever saying "this line
/// starts here".
const LEFT_TO_RIGHT_MARK: char = '\u{200e}';

/// Prefixes `text` with a left-to-right mark.
///
/// See the field's own comment: iced lays a text input's contents out at an
/// infinite width, and a right-to-left line is then started at the far end of
/// that width, which puts the glyphs out of range of anything that can draw
/// them. The mark gives the line a left-to-right base direction; everything
/// after it keeps its own direction.
fn anchored(text: &str) -> String {
    let mut anchored = String::with_capacity(text.len() + LEFT_TO_RIGHT_MARK.len_utf8());
    anchored.push(LEFT_TO_RIGHT_MARK);
    anchored.push_str(text);
    anchored
}

/// A single-line text field.
pub struct FieldSpec<M> {
    /// The interaction slot this control animates through.
    pub interaction: Interaction,
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The colours to draw with.
    pub palette: Palette,
    /// A visible caption above the field.
    pub caption: &'static str,
    /// A hint shown when the field is empty and unfocused.
    pub placeholder: String,
    /// The note shown under the field while it has the keyboard focus.
    ///
    /// Defaults to the English [`Text::HintEditing`], so a caller that does not
    /// care about translation needs to do nothing. Pass an empty string to draw
    /// no note at all, or [`FieldSpec::hint`] with the active catalogue's text
    /// to translate it.
    pub hint: String,
    /// The current value.
    pub value: String,
    /// Called on every keystroke.
    pub on_input: Box<dyn Fn(String) -> M>,
    /// Whether to draw the keyboard focus ring.
    pub focused: bool,
    /// Which way the field's own text is read.
    pub direction: Direction,
}

impl<M: 'static + Clone> FieldSpec<M> {
    /// A field with a caption, showing `value`.
    pub fn new(
        interaction: Interaction,
        clock: FrameClock,
        palette: Palette,
        caption: &'static str,
        value: String,
        on_input: impl Fn(String) -> M + 'static,
    ) -> FieldSpec<M> {
        FieldSpec {
            interaction,
            clock,
            palette,
            caption,
            placeholder: String::new(),
            hint: Catalog::english().text(Text::HintEditing).to_string(),
            value,
            on_input: Box::new(on_input),
            focused: false,
            direction: Direction::LeftToRight,
        }
    }

    /// Reads the caption, the hint and the typed text from the leading edge of
    /// the language in `catalog`.
    pub fn reading(mut self, catalog: Catalog) -> FieldSpec<M> {
        self.direction = catalog.direction();
        self
    }

    /// Sets the placeholder.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> FieldSpec<M> {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the note shown while the field is focused.
    ///
    /// Pass an empty string to draw no note.
    pub fn hint(mut self, hint: impl Into<String>) -> FieldSpec<M> {
        self.hint = hint.into();
        self
    }

    /// Draws the focus ring.
    pub fn focused(mut self, focused: bool) -> FieldSpec<M> {
        self.focused = focused;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let FieldSpec {
            interaction,
            clock,
            palette,
            caption,
            placeholder,
            hint,
            value,
            on_input,
            focused,
            direction,
        } = self;

        let hover = interaction.hover;
        let style = {
            let clock = clock.clone();
            move |_theme: &iced::Theme, status: text_input::Status| {
                let hovered = matches!(
                    status,
                    text_input::Status::Hovered | text_input::Status::Focused
                );
                let wash = hover.track(hovered, &clock, Spec::HOVER);

                text_input::Style {
                    value: palette.text,
                    placeholder: palette.text_faint,
                    selection: palette.accent,

                    // A filled well rather than an outlined box: it matches the
                    // sunken surfaces used elsewhere, so an input reads as
                    // part of the same system as a panel.
                    background: Background::Color(if focused {
                        palette.sunken
                    } else {
                        crate::design::palette::overlay(
                            crate::design::motion::ease::fade(palette.hover, wash),
                            palette.sunken,
                        )
                    }),
                    border: Border {
                        color: if focused {
                            palette.accent_text
                        } else {
                            crate::design::motion::ease::fade(palette.outline, 0.4 + wash * 0.6)
                        },
                        width: if focused {
                            FOCUS_RING
                        } else {
                            stroke::HAIRLINE
                        },
                        radius: radius::MD.into(),
                    },
                    icon: palette.text_faint,
                }
            }
        };

        // A field is typed into from its leading edge, so the text starts where
        // the language starts rather than always at the left.
        //
        // The mark is the reason the string is not handed over as it stands. Iced
        // lays a text field's contents out at an *infinite* width, and cosmic-text
        // then starts a right-to-left line at the far end of that width: the
        // glyphs end up at positions no finite box can hold, and rendering one
        // panics. A single left-to-right mark at the front gives the line a
        // left-to-right base direction, so it is drawn from the start; the
        // Arabic inside it is still shaped and ordered right to left, and the
        // mark itself has no width. It is invisible in a left-to-right language
        // too, and harmless there: it only ever asserts what is already true.
        let field = TextInput::new(&anchored(&placeholder), &anchored(&value))
            .size(typography::Role::Body.size())
            .font(fonts::NUMERIC)
            .align_x(direction.align_x())
            .style(style)
            .on_input(on_input);

        let mut column = Column::new()
            .spacing(space::XS)
            .push(
                typography::eyebrow(caption, &palette)
                    .written_in(direction)
                    .view(),
            )
            .push(field);

        if focused && !hint.is_empty() {
            column = column.push(
                typography::Label::new(hint, Role::Micro, palette.text_faint)
                    .written_in(direction)
                    .natural()
                    .view(),
            );
        }

        column.width(Length::Fill).into()
    }
}

/// A numeric stepper: a caption, a value, and two buttons.
pub struct StepperSpec<M> {
    /// The interaction slot this control animates through.
    pub interaction: Interaction,
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The colours to draw with.
    pub palette: Palette,
    /// A caption above the value.
    pub caption: &'static str,
    /// The value, already formatted.
    pub value: String,
    /// The message to send for a decrement.
    pub down: M,
    /// The message to send for an increment.
    pub up: M,
    /// Whether the decrement button is available.
    pub can_decrease: bool,
    /// Whether the increment button is available.
    pub can_increase: bool,
    /// Whether to draw the keyboard focus ring.
    pub focused: bool,
    /// Which way the caption and the value are read.
    pub direction: Direction,
}

impl<M: 'static + Clone> StepperSpec<M> {
    /// A stepper showing `value`.
    pub fn new(
        interaction: Interaction,
        clock: FrameClock,
        palette: Palette,
        caption: &'static str,
        value: String,
        down: M,
        up: M,
    ) -> StepperSpec<M> {
        StepperSpec {
            interaction,
            clock,
            palette,
            caption,
            value,
            down,
            up,
            can_decrease: true,
            can_increase: true,
            focused: false,
            direction: Direction::LeftToRight,
        }
    }

    /// Reads the caption and the value from the leading edge of the language in
    /// `catalog`.
    pub fn reading(mut self, catalog: Catalog) -> StepperSpec<M> {
        self.direction = catalog.direction();
        self
    }

    /// Disables the decrement button.
    pub fn at_minimum(mut self) -> StepperSpec<M> {
        self.can_decrease = false;
        self
    }

    /// Disables the increment button.
    pub fn at_maximum(mut self) -> StepperSpec<M> {
        self.can_increase = false;
        self
    }

    /// Draws the focus ring.
    pub fn focused(mut self, focused: bool) -> StepperSpec<M> {
        self.focused = focused;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let StepperSpec {
            interaction,
            clock,
            palette,
            caption,
            value,
            down,
            up,
            can_decrease,
            can_increase,
            focused,
            direction,
        } = self;

        let decrease = crate::ui::components::controls::IconButtonSpec::new(
            interaction.clone(),
            clock.clone(),
            palette,
            crate::ui::components::controls::Emphasis::Ghost,
            Icon::Minus,
            down,
        )
        .focused(focused)
        .enabled(can_decrease)
        .view();

        let increase = crate::ui::components::controls::IconButtonSpec::new(
            interaction.clone(),
            clock.clone(),
            palette,
            crate::ui::components::controls::Emphasis::Ghost,
            Icon::Plus,
            up,
        )
        .focused(focused)
        .enabled(can_increase)
        .view();

        // The value is monospaced, so stepping it does not make the controls
        // jump sideways.
        let readout: Element<'static, M> =
            crate::ui::components::controls::readout(palette, caption, &value);

        // A stepper counts down on one side and up on the other. The two sides
        // swap in a right-to-left language, so that the button that lowers the
        // value is on the side the reader is coming from, as it is for a person
        // used to left-to-right numbers.
        // A stepper is a group — a button, the value, and the other button —
        // so the row is sized to the group and the group is centred in whatever
        // space the control is given. Letting the two buttons fly out to the
        // edges of a wide row would leave two separate buttons with a number
        // stranded in the middle, which reads as three controls rather than one.
        let body = crate::ui::ordered(direction, vec![decrease, readout, increase])
            .spacing(space::SM)
            .align_y(iced::Alignment::Center);

        Column::new()
            .spacing(space::XS)
            .push(
                typography::eyebrow(caption, &palette)
                    .written_in(direction)
                    .view(),
            )
            .push(body)
            .align_x(iced::Alignment::Center)
            .width(Length::Fill)
            .into()
    }
}

/// A segmented control: two or three mutually exclusive choices.
pub struct Segmented<M> {
    /// The colours to draw with.
    pub palette: Palette,
    /// The choices.
    pub options: Vec<(&'static str, Option<Icon>)>,
    /// Which one is selected.
    pub selected: usize,
    /// The message for each choice.
    pub messages: Vec<M>,
    /// The interaction slot for the whole control.
    pub interaction: Interaction,
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The index of the keyboard-focused choice.
    pub focus: Option<usize>,
    /// Which way the choices are read.
    pub direction: Direction,
}

impl<M: 'static + Clone> Segmented<M> {
    /// A control over `options`, starting on `selected`.
    pub fn new(
        palette: Palette,
        options: Vec<(&'static str, Option<Icon>)>,
        selected: usize,
        messages: Vec<M>,
        interaction: Interaction,
        clock: FrameClock,
    ) -> Segmented<M> {
        Segmented {
            palette,
            options,
            selected,
            messages,
            interaction,
            clock,
            focus: None,
            direction: Direction::LeftToRight,
        }
    }

    /// Reads the choices from the leading edge of the language in `catalog`.
    pub fn reading(mut self, catalog: Catalog) -> Segmented<M> {
        self.direction = catalog.direction();
        self
    }

    /// Marks one choice as keyboard-focused.
    pub fn focus(mut self, focus: Option<usize>) -> Segmented<M> {
        self.focus = focus;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let Segmented {
            palette,
            options,
            selected,
            messages,
            interaction,
            clock,
            focus,
            direction,
        } = self;

        let well = crate::ui::components::surfaces::Card::new(
            palette,
            crate::ui::components::surfaces::Fill::Sunken,
        )
        .pad_xy(space::XXS, space::XXS)
        .spacing(0.0)
        .fill_width();

        let mut parts = Vec::with_capacity(options.len());
        for (index, (text, icon)) in options.iter().enumerate() {
            let message = messages.get(index).cloned().unwrap_or_else(|| {
                unreachable!("a segmented control has a message for every choice")
            });
            let label = Label::new(*text, Role::Label, palette.text_muted).written_in(direction);
            let chip = crate::ui::components::controls::ChipSpec::new(
                interaction.clone(),
                clock.clone(),
                palette,
                label,
                index == selected,
                message,
            )
            .focused(focus == Some(index))
            .icon(icon.unwrap_or(Icon::Clock))
            .view();
            parts.push(chip);
        }

        well.push(crate::ui::components::surfaces::flow(parts, space::XXS))
            .view()
    }
}

/// A labelled row of weekday toggles, for the alarm editor.
///
/// The day names are English — [`Catalog::english`] — so a caller that changes
/// nothing gets the labels it always drew. Call [`weekday_row_with`] with the
/// active [`Catalog`] to name the days in the reader's own language.
pub fn weekday_row<M: 'static + Clone>(
    palette: Palette,
    clock: FrameClock,
    interaction: Interaction,
    selected: crate::domain::alarm::Weekdays,
    messages: Vec<M>,
) -> Element<'static, M> {
    weekday_row_with(
        palette,
        Catalog::english(),
        clock,
        interaction,
        selected,
        messages,
    )
}

/// A row of weekday toggles named in `catalog`'s language.
///
/// The chips wrap, so a narrower window loses a column rather than truncating
/// a day, and nothing about the row depends on which way the week is written.
pub fn weekday_row_with<M: 'static + Clone>(
    palette: Palette,
    catalog: Catalog,
    clock: FrameClock,
    interaction: Interaction,
    selected: crate::domain::alarm::Weekdays,
    messages: Vec<M>,
) -> Element<'static, M> {
    let mut parts = Vec::with_capacity(7);
    for index in 0..7u8 {
        let day = Weekday::try_from(index).expect("0..7 is a weekday");
        let message = messages
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| unreachable!("a weekday row has a message for every day"));
        parts.push(
            crate::ui::components::controls::ChipSpec::new(
                interaction.clone(),
                clock.clone(),
                palette,
                Label::new(catalog.weekday_short(day), Role::Label, palette.text_muted),
                selected.contains(day),
                message,
            )
            .view(),
        );
    }

    crate::ui::components::surfaces::flow(parts, space::SM)
}

/// A progress note under a control, e.g. a sound's description.
pub fn supporting<'a, M: 'static>(text: &'a str, palette: Palette) -> Element<'a, M> {
    Label::new(text, Role::Caption, palette.text_faint)
        .proportional()
        .view()
        .into()
}

/// The colour a disabled control's contents are drawn in.
pub fn disabled_ink(palette: Palette) -> Color {
    palette.text_faint
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_is_anchored_to_the_left_of_its_line() {
        // Iced lays a text input's contents out at an infinite width, and a
        // right-to-left line is started at the far end of that width — which
        // puts the glyphs out of range of anything that can draw them. The mark
        // is what stops that.
        assert_eq!(LEFT_TO_RIGHT_MARK, '\u{200e}');
        assert_eq!(anchored("ساعت"), "\u{200e}ساعت");
        assert_eq!(anchored(""), "\u{200e}");
        assert_eq!(anchored("04:30"), "\u{200e}04:30");

        // It only ever prefixes: the value itself is untouched, so what the
        // field hands back to the domain is what was typed.
        assert!(anchored("اسم").ends_with("اسم"));
    }

    #[test]
    fn a_field_reads_its_caption_and_its_text_from_the_catalogue() {
        use crate::services::i18n::{Catalog, Language};

        for (language, expected) in [
            (Language::English, Direction::LeftToRight),
            (Language::Persian, Direction::RightToLeft),
            (Language::Arabic, Direction::RightToLeft),
            (Language::Urdu, Direction::RightToLeft),
        ] {
            let field = FieldSpec::<()>::new(
                Interaction::default(),
                FrameClock::new(),
                Palette::LIGHT,
                "Name",
                "Alarm".to_string(),
                |_| (),
            )
            .reading(Catalog::new(language));
            assert_eq!(field.direction, expected, "{language:?}");
        }
    }

    use crate::ui::components::interaction::Interactions;
    use chrono::Weekday;

    fn clock() -> FrameClock {
        FrameClock::new()
    }

    #[test]
    fn a_field_captures_its_caption_and_value() {
        let palette = Palette::LIGHT;
        let _: Element<'static, String> = FieldSpec::new(
            Interaction::default(),
            clock(),
            palette,
            "Label",
            "Alarm".to_string(),
            |value: String| value,
        )
        .placeholder("Name")
        .focused(true)
        .view();
    }

    #[test]
    fn a_stepper_renders_at_both_ends_of_its_range() {
        let palette = Palette::DARK;
        let _: Element<'static, ()> = StepperSpec::new(
            Interaction::default(),
            clock(),
            palette,
            "Hour",
            "07".to_string(),
            (),
            (),
        )
        .view();
        let _: Element<'static, ()> = StepperSpec::new(
            Interaction::default(),
            clock(),
            palette,
            "Minute",
            "00".to_string(),
            (),
            (),
        )
        .at_minimum()
        .at_maximum()
        .focused(true)
        .view();
    }

    #[test]
    fn a_segmented_control_renders() {
        let palette = Palette::LIGHT;
        let _: Element<'static, ()> = Segmented::new(
            palette,
            vec![
                ("12-hour", None),
                ("24-hour", None),
                ("System", Some(Icon::System)),
            ],
            0,
            vec![(), (), ()],
            Interaction::default(),
            clock(),
        )
        .focus(Some(1))
        .view();
    }

    #[test]
    fn the_weekday_row_offers_all_seven_days_in_order() {
        let palette = Palette::DARK;
        let selected = crate::domain::alarm::Weekdays::from_days([Weekday::Mon, Weekday::Fri]);
        let _: Element<'static, ()> = weekday_row(
            palette,
            clock(),
            Interaction::default(),
            selected,
            vec![(); 7],
        );
    }

    #[test]
    fn interactions_are_optional_for_a_control() {
        // The shell keeps one set for the whole window, so a page can ask for a
        // control it never animates and get a working, if flatter, one.
        let interactions = Interactions::new();
        let _ = interactions.get("test.field");
        assert_eq!(interactions.len(), 1);
    }

    #[test]
    fn supporting_text_and_disabled_ink() {
        let palette = Palette::LIGHT;
        let _: Element<'static, ()> = supporting::<()>("A short note", palette);
        assert_eq!(disabled_ink(palette), palette.text_faint);
    }
}
