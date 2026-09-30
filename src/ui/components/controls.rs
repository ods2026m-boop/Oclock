//! Buttons, switches and chips.
//!
//! Every control follows the same three rules:
//!
//! * it is styled from the palette, never from a theme constant;
//! * it reports hover, press and state through
//!   [`crate::ui::components::interaction`], so its response is animated
//!   rather than a hard cut;
//! * its label is real text from the type ramp, so it scales with the rest of
//!   the interface and stays legible.

use iced::widget::button::{self, Button, Status};
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path, Stroke};
use iced::widget::{container, row, Canvas, Space};
use iced::{Background, Border, Color, Element, Length, Shadow};

use crate::design::motion::{ease, FrameClock, Spec};
use crate::design::palette::Palette;
use crate::design::tokens::{radius, space, stroke, FOCUS_RING};
use crate::ui::components::icons::{Icon, IconView};
use crate::ui::components::interaction::Interaction;
use crate::ui::components::surfaces::{Card, Fill as CardFill};
use crate::ui::typography::{Label, Role};

/// Composites a translucent wash over the surface it will sit on.
///
/// The palette's hover and active colours are washes, not colours, so they
/// only mean something against a known background. A quiet button and a
/// destructive one are always drawn on a card, so the card is the background
/// that gets used — which also means the result is opaque and its contrast can
/// be reasoned about rather than guessed at.
fn wash_over(wash: Color, surface: Color) -> Color {
    crate::design::palette::overlay(wash, surface)
}

/// Padding on both axes, in the order the design system states it.
fn pad_xy(horizontal: f32, vertical: f32) -> iced::Padding {
    iced::padding::top(vertical)
        .bottom(vertical)
        .left(horizontal)
        .right(horizontal)
}

/// How prominent a control is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emphasis {
    /// The one action a view wants taken. At most one per view.
    Primary,
    /// An ordinary action.
    Secondary,
    /// A quiet action beside another.
    Ghost,
    /// A destructive action.
    Danger,
}

impl Emphasis {
    /// The fill behind the control, given the animated hover and press.
    fn fill(self, palette: Palette, hover: f32, press: f32) -> Color {
        match self {
            Emphasis::Primary => {
                let lifted = ease::color(palette.accent_hover, palette.accent_active, press);
                ease::color(palette.accent, lifted, hover)
            }
            Emphasis::Secondary => ease::color(
                palette.surface,
                palette.sunken,
                (hover * 0.85 + press * 0.15).clamp(0.0, 1.0),
            ),
            Emphasis::Ghost => {
                // Pressing pushes further than hovering, so the two read as
                // different movements rather than one.
                let strength = (hover * 0.7 + press).clamp(0.0, 1.0);
                wash_over(
                    ease::color(palette.hover, palette.active, strength),
                    palette.surface,
                )
            }
            Emphasis::Danger => {
                // The wash stays light enough that the danger-coloured label
                // keeps its contrast at every point of the response.
                let strength = (hover * 0.6 + press * 0.9).clamp(0.0, 1.0);
                wash_over(
                    ease::color(palette.danger_soft, palette.danger, strength * 0.18),
                    palette.surface,
                )
            }
        }
    }

    /// The text or icon colour.
    fn ink(self, palette: Palette) -> Color {
        match self {
            Emphasis::Primary => palette.on_accent(),
            Emphasis::Danger => palette.danger,
            _ => palette.text,
        }
    }

    /// The outline, if any.
    fn outline(self, palette: Palette) -> Color {
        match self {
            Emphasis::Secondary => palette.outline,
            _ => Color::TRANSPARENT,
        }
    }

    /// The lift under the control, which grows on hover.
    fn shadow(self, palette: Palette, hover: f32) -> Shadow {
        if !matches!(self, Emphasis::Primary) {
            return Shadow::default();
        }
        let base = palette.shadow(crate::design::palette::ShadowLevel::Low);
        Shadow {
            offset: base.offset * (1.0 + hover * 0.6),
            blur_radius: base.blur_radius * (1.0 + hover * 1.2),
            ..base
        }
    }

    /// Vertical padding, applied to the content.
    ///
    /// Iced 0.13's button style has no padding field, so the padding is part of
    /// the content's layout — which also means the control's height comes from
    /// the type ramp rather than from a hard-coded constant.
    fn content_padding(self) -> f32 {
        match self {
            Emphasis::Primary => space::MD + 2.0,
            _ => space::SM + 2.0,
        }
    }

    /// How far the fill shifts on hover, before any easing is applied.
    fn response(self) -> f32 {
        match self {
            Emphasis::Ghost | Emphasis::Danger => 0.55,
            _ => 1.0,
        }
    }
}

/// A label button, with an optional leading icon.
pub struct ButtonSpec<M> {
    /// The interaction slot this control animates through.
    pub interaction: Interaction,
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The colours to draw with.
    pub palette: Palette,
    /// How prominent the control is.
    pub emphasis: Emphasis,
    /// What the button says.
    pub label: Label,
    /// An icon before the text.
    pub icon: Option<Icon>,
    /// The message to send when pressed.
    pub message: M,
    /// Whether to draw the keyboard focus ring.
    pub focused: bool,
    /// False disables the control: it stops responding and dims.
    pub enabled: bool,
    /// Whether the button fills the space its row offers.
    grow: bool,
}

impl<M: 'static + Clone> ButtonSpec<M> {
    /// A button labelled `label`.
    pub fn new(
        interaction: Interaction,
        clock: FrameClock,
        palette: Palette,
        emphasis: Emphasis,
        label: Label,
        message: M,
    ) -> ButtonSpec<M> {
        ButtonSpec {
            interaction,
            clock,
            palette,
            emphasis,
            label,
            icon: None,
            message,
            focused: false,
            enabled: true,
            grow: false,
        }
    }

    /// Adds a leading icon.
    pub fn icon(mut self, icon: Icon) -> ButtonSpec<M> {
        self.icon = Some(icon);
        self
    }

    /// Draws the focus ring.
    pub fn focused(mut self, focused: bool) -> ButtonSpec<M> {
        self.focused = focused;
        self
    }

    /// Disables the control.
    pub fn enabled(mut self, enabled: bool) -> ButtonSpec<M> {
        self.enabled = enabled;
        self
    }

    /// Makes the button take the space its row offers.
    ///
    /// A dialog's confirming action is built this way rather than by putting a
    /// `Length::Fill` spacer beside it: a filling sibling in a row takes its
    /// share from *everyone*, which squeezes both buttons, while a button that
    /// fills takes exactly the space that is left.
    pub fn grow(mut self) -> ButtonSpec<M> {
        self.grow = true;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let ButtonSpec {
            interaction,
            clock,
            palette,
            emphasis,
            label,
            icon,
            message,
            focused,
            enabled,
            grow,
        } = self;

        let icon_size = (label.size() * 0.9).clamp(14.0, 22.0);
        let mut content = row![].spacing(space::SM).align_y(iced::Alignment::Center);
        if let Some(icon) = icon {
            content =
                content.push(IconView::new(icon, icon_size, emphasis.ink(palette)).view::<M>());
        }
        // A filled control inks its own label. The style's `text_color` is
        // only a default for text that does not choose its own colour, and
        // every label here does — so without this a primary button would draw
        // a dark label on the accent fill, at the one moment the interface
        // most wants to be read.
        //
        // A disabled one dims its own label too, for the same reason: the
        // style can only dim a colour it is not being asked to use.
        let label = label.tinted(if enabled {
            emphasis.ink(palette)
        } else {
            emphasis.ink(palette).scale_alpha(0.35)
        });
        // A button's label has to be as wide as the label: the button shrink-wraps
        // around it, and a right-to-left run in a wider box would be drawn at the
        // far end of that box, outside the button.
        content = content.push(label.natural().view());

        let width = if grow { Length::Fill } else { Length::Shrink };
        let content = container(content)
            .padding(pad_xy(space::SM, emphasis.content_padding()))
            .width(width)
            .height(Length::Shrink);
        let button = Button::new(content).width(width).height(Length::Shrink);
        let button = button.style(disabled_style(
            style_for(interaction, clock, palette, emphasis, focused),
            palette,
            enabled,
        ));

        // A disabled control does not take the press at all, rather than
        // accepting it and ignoring it, so the cursor and the focus order both
        // behave the way a user expects.
        if enabled {
            button.on_press(message).into()
        } else {
            button.into()
        }
    }
}

/// Fades a control that is at a limit.
///
/// A stepper's buttons have to keep their place in the layout whether or not
/// they can do anything, so disabling one dims it rather than removing it —
/// which also means the value next to it never shifts.
fn disabled_style<F>(
    style: F,
    palette: Palette,
    enabled: bool,
) -> impl Fn(&iced::Theme, Status) -> button::Style
where
    F: Fn(&iced::Theme, Status) -> button::Style,
{
    move |theme, status| {
        let mut resolved = style(theme, status);
        if enabled {
            return resolved;
        }
        resolved.text_color = resolved.text_color.scale_alpha(0.35);
        if let Some(Background::Color(colour)) = resolved.background {
            resolved.background = Some(Background::Color(crate::design::palette::overlay(
                crate::design::motion::ease::fade(palette.text_faint, 0.06),
                colour,
            )));
        }
        resolved
    }
}

/// The style closure shared by every button-like control.
fn style_for(
    interaction: Interaction,
    clock: FrameClock,
    palette: Palette,
    emphasis: Emphasis,
    focused: bool,
) -> impl Fn(&iced::Theme, Status) -> button::Style {
    let hover = interaction.hover;
    let press = interaction.press;

    move |_theme: &iced::Theme, status: Status| {
        let hovered = matches!(status, Status::Hovered | Status::Pressed);
        let hover_value =
            (hover.track(hovered, &clock, Spec::HOVER) * emphasis.response()).clamp(0.0, 1.0);
        let press_value = press
            .track(matches!(status, Status::Pressed), &clock, Spec::PRESS)
            .clamp(0.0, 1.0);

        button::Style {
            background: Some(Background::Color(emphasis.fill(
                palette,
                hover_value,
                press_value,
            ))),
            text_color: emphasis.ink(palette),
            border: Border {
                color: if focused {
                    palette.accent_text
                } else {
                    emphasis.outline(palette)
                },
                width: if focused {
                    FOCUS_RING
                } else {
                    stroke::HAIRLINE
                },
                radius: radius::LG.into(),
            },
            shadow: emphasis.shadow(palette, hover_value),
        }
    }
}

/// An icon-only button, for toolbars and card actions.
pub struct IconButtonSpec<M> {
    /// The interaction slot this control animates through.
    pub interaction: Interaction,
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The colours to draw with.
    pub palette: Palette,
    /// How prominent the control is.
    pub emphasis: Emphasis,
    /// Which mark to draw.
    pub icon: Icon,
    /// The message to send when pressed.
    pub message: M,
    /// Whether to draw the keyboard focus ring.
    pub focused: bool,
    /// False disables the control.
    pub enabled: bool,
    /// Whether the button fills the space its row offers.
    grow: bool,
}

impl<M: 'static + Clone> IconButtonSpec<M> {
    /// A button showing `icon`.
    pub fn new(
        interaction: Interaction,
        clock: FrameClock,
        palette: Palette,
        emphasis: Emphasis,
        icon: Icon,
        message: M,
    ) -> IconButtonSpec<M> {
        IconButtonSpec {
            interaction,
            clock,
            palette,
            emphasis,
            icon,
            message,
            focused: false,
            enabled: true,
            grow: false,
        }
    }

    /// Makes the button fill the space its row offers.
    pub fn grow(mut self) -> IconButtonSpec<M> {
        self.grow = true;
        self
    }

    /// Draws the focus ring.
    pub fn focused(mut self, focused: bool) -> IconButtonSpec<M> {
        self.focused = focused;
        self
    }

    /// Disables the control.
    pub fn enabled(mut self, enabled: bool) -> IconButtonSpec<M> {
        self.enabled = enabled;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let IconButtonSpec {
            interaction,
            clock,
            palette,
            emphasis,
            icon,
            message,
            focused,
            enabled,
            grow,
        } = self;

        let mark: Element<'static, M> =
            IconView::new(icon, 18.0, emphasis.ink(palette)).view::<M>();
        let width = if grow { Length::Fill } else { Length::Shrink };
        // One padded row, and deliberately not a padding-less container holding
        // a padded one. Iced hands a container's child the container's own
        // bounds, so a *shrink* container around a *shrink* container inside a
        // shrink button resolves to nothing: the button still paints its fill,
        // so the control is there and looks pressable, and the mark inside it is
        // laid out at zero and never drawn. The symptom is an empty button, and
        // it only shows in the layouts where this control is the whole control —
        // the compact rail — which is why it survives a look at the wide one.
        let content = row![mark]
            .padding(pad_xy(space::SM, emphasis.content_padding()))
            .width(width)
            .height(Length::Shrink);
        let button = Button::new(content).width(width).height(Length::Shrink);
        let button = button.style(disabled_style(
            style_for(interaction, clock, palette, emphasis, focused),
            palette,
            enabled,
        ));

        if enabled {
            button.on_press(message).into()
        } else {
            button.into()
        }
    }
}

/// A switch.
///
/// Built from a `Button` and a canvas rather than from Iced's `toggler`, whose
/// track and knob are not parameterisable enough to animate. The knob's travel
/// and the track's fill come from one animated value, so the two can never
/// disagree halfway through the flip.
pub struct SwitchSpec<M> {
    /// The interaction slot this control animates through.
    pub interaction: Interaction,
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The colours to draw with.
    pub palette: Palette,
    /// The current state.
    pub on: bool,
    /// The message to send when pressed.
    pub message: M,
    /// Whether to draw the keyboard focus ring.
    pub focused: bool,
}

/// The switch's dimensions, in logical pixels.
pub const SWITCH_SIZE: (f32, f32) = (46.0, 26.0);

impl<M: 'static + Clone> SwitchSpec<M> {
    /// A switch showing `on`.
    pub fn new(
        interaction: Interaction,
        clock: FrameClock,
        palette: Palette,
        on: bool,
        message: M,
    ) -> SwitchSpec<M> {
        SwitchSpec {
            interaction,
            clock,
            palette,
            on,
            message,
            focused: false,
        }
    }

    /// Draws the focus ring.
    pub fn focused(mut self, focused: bool) -> SwitchSpec<M> {
        self.focused = focused;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let SwitchSpec {
            interaction,
            clock,
            palette,
            on,
            message,
            focused,
        } = self;

        let (width, height) = SWITCH_SIZE;
        let drawing = SwitchDrawing {
            palette,
            clock: clock.clone(),
            interaction: interaction.clone(),
            on,
        };

        let style = {
            let hover = interaction.hover;
            let press = interaction.press;
            move |_theme: &iced::Theme, status: Status| {
                let _ = hover.track(
                    matches!(status, Status::Hovered | Status::Pressed),
                    &clock,
                    Spec::HOVER,
                );
                let _ = press.track(matches!(status, Status::Pressed), &clock, Spec::PRESS);

                button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    text_color: palette.text_muted,
                    border: Border {
                        color: if focused {
                            palette.accent_text
                        } else {
                            Color::TRANSPARENT
                        },
                        // A transparent border of a real width keeps the ring
                        // from shifting the switch when focus arrives.
                        width: if focused { FOCUS_RING } else { 0.0 },
                        radius: radius::PILL.into(),
                    },
                    ..button::Style::default()
                }
            }
        };

        let canvas: Element<'static, ()> = Canvas::new(drawing).width(width).height(height).into();
        let track: Element<'static, M> =
            canvas.map(|()| unreachable!("a switch never sends a message"));

        Button::new(track)
            .width(Length::Fixed(width + 8.0))
            .style(style)
            .on_press(message)
            .into()
    }
}

struct SwitchDrawing {
    palette: Palette,
    clock: FrameClock,
    interaction: Interaction,
    on: bool,
}

impl canvas::Program<()> for SwitchDrawing {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::widget::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = self.palette;
        let (width, height) = SWITCH_SIZE;
        let radius = height / 2.0;

        // One value drives the fill and the knob, so the control is never
        // caught between two states.
        let progress = self
            .interaction
            .state
            .drive(self.on as u8 as f32, &self.clock, Spec::TINT)
            .clamp(0.0, 1.0);

        let mut frame = Frame::new(renderer, bounds.size());

        let track = ease::color(palette.sunken, palette.accent, progress);
        frame.fill(
            &Path::new(|path| {
                path.rounded_rectangle(
                    iced::Point::new(0.0, 0.0),
                    iced::Size::new(width, height),
                    radius.into(),
                );
            }),
            Fill::from(track),
        );

        let inset = 3.0;
        let travel = width - height;
        frame.fill(
            &Path::circle(
                iced::Point::new(inset + travel * progress, radius),
                radius - inset,
            ),
            Fill::from(if progress > 0.5 {
                palette.on_accent()
            } else {
                palette.text_muted
            }),
        );

        frame.stroke(
            &Path::rounded_rectangle(
                iced::Point::new(0.0, 0.0),
                iced::Size::new(width, height),
                radius.into(),
            ),
            Stroke::default()
                .with_color(palette.outline)
                .with_width(stroke::HAIRLINE),
        );

        vec![frame.into_geometry()]
    }
}

/// A selectable chip, for weekdays, sounds and presets.
pub struct ChipSpec<M> {
    /// The interaction slot this control animates through.
    pub interaction: Interaction,
    /// Shared animation clock.
    pub clock: FrameClock,
    /// The colours to draw with.
    pub palette: Palette,
    /// The chip's text.
    pub label: Label,
    /// Whether the chip is selected.
    pub selected: bool,
    /// The message to send when pressed.
    pub message: M,
    /// Whether to draw the keyboard focus ring.
    pub focused: bool,
    /// An optional leading mark.
    pub icon: Option<Icon>,
}

impl<M: 'static + Clone> ChipSpec<M> {
    /// A chip labelled `label`.
    pub fn new(
        interaction: Interaction,
        clock: FrameClock,
        palette: Palette,
        label: Label,
        selected: bool,
        message: M,
    ) -> ChipSpec<M> {
        ChipSpec {
            interaction,
            clock,
            palette,
            label,
            selected,
            message,
            focused: false,
            icon: None,
        }
    }

    /// Adds a leading icon.
    pub fn icon(mut self, icon: Icon) -> ChipSpec<M> {
        self.icon = Some(icon);
        self
    }

    /// Draws the focus ring.
    pub fn focused(mut self, focused: bool) -> ChipSpec<M> {
        self.focused = focused;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let ChipSpec {
            interaction,
            clock,
            palette,
            label,
            selected,
            message,
            focused,
            icon,
        } = self;

        let mut content = row![]
            .spacing(space::XS + 2.0)
            .align_y(iced::Alignment::Center);

        // The chip's ink follows its fill, so it is resolved here, once, rather
        // than left to the style — which, for a label that has picked its own
        // colour, would never be consulted. A selected chip with an unselected
        // chip's ink is a chip whose state you have to read twice.
        let state = interaction.state;
        let clock_for_state = clock.clone();
        let selected_value = state
            .drive(selected as u8 as f32, &clock_for_state, Spec::TINT)
            .clamp(0.0, 1.0);
        let ink = ease::color(palette.text_muted, palette.accent_text, selected_value);

        if let Some(icon) = icon {
            content = content.push(IconView::new(icon, 14.0, ink).view::<M>());
        }
        // A chip's label is measured, so a right-to-left one is drawn inside its
        // own pill rather than at the far end of the row the pills share.
        content = content.push(label.tinted(ink).natural().view());

        let hover = interaction.hover;
        let press = interaction.press;

        let style = move |_theme: &iced::Theme, status: Status| {
            let hover_value = hover
                .track(
                    matches!(status, Status::Hovered | Status::Pressed),
                    &clock,
                    Spec::HOVER,
                )
                .clamp(0.0, 1.0);
            let press_value = press
                .track(matches!(status, Status::Pressed), &clock, Spec::PRESS)
                .clamp(0.0, 1.0);
            let selected_value = selected_value;

            // A selected chip is filled with a wash of the accent; hovering an
            // unselected one deepens its own surface instead, so the two states
            // never read as the same control in different shades.
            let resting = ease::color(palette.surface, palette.accent_soft, selected_value);
            let base = ease::color(
                resting,
                palette.sunken,
                hover_value * (1.0 - selected_value),
            );
            let fill = ease::color(base, palette.active, press_value * 0.5);
            let ink = ease::color(palette.text_muted, palette.accent_text, selected_value);

            button::Style {
                background: Some(Background::Color(fill)),
                text_color: ink,
                border: Border {
                    color: if focused {
                        palette.accent_text
                    } else {
                        ease::color(palette.outline, Color::TRANSPARENT, selected_value)
                    },
                    width: if focused {
                        FOCUS_RING
                    } else {
                        stroke::HAIRLINE
                    },
                    radius: radius::PILL.into(),
                },
                shadow: Shadow::default(),
            }
        };

        Button::new(content)
            .width(Length::Shrink)
            .style(style)
            .on_press(message)
            .into()
    }
}

/// A non-interactive mark, for list bullets and units.
pub fn mark(icon: Icon, size: f32, color: Color) -> Element<'static, ()> {
    container(IconView::new(icon, size, color).view::<()>())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .center_x(Length::Shrink)
        .center_y(Length::Shrink)
        .into()
}

/// A read-only field: a label above a value, drawn like an input.
///
/// The value is set in the monospaced face, so a value that changes from one
/// second to the next does not shift sideways as it does.
pub fn readout<M: 'static>(palette: Palette, name: &str, value: &str) -> Element<'static, M> {
    readout_field(palette, name, value, Length::Shrink, false)
}

fn readout_field<'a, M: 'static>(
    palette: Palette,
    name: &str,
    value: &str,
    width: Length,
    words: bool,
) -> Element<'a, M> {
    let value = Label::new(value, Role::Metric, palette.text);
    let value = if words {
        value.font(crate::design::tokens::fonts::UI_SEMIBOLD)
    } else {
        value
    };

    Card::new(palette, CardFill::Sunken)
        .pad_xy(space::MD, space::SM)
        .spacing(space::XXS)
        .width(width)
        .push(
            crate::ui::typography::eyebrow(name, &palette)
                .natural()
                .view(),
        )
        .push(value.natural().view())
        .view()
}

/// A read-only field whose value is words rather than a number.
///
/// The same field, in the interface face instead of the monospaced one. A value
/// that is a number is set in monospaced figures on purpose — it changes from
/// one second to the next, and equal-width digits are what stop the readout
/// shifting sideways as they do. A date changes once a day and is mostly
/// letters, and set in a monospaced face it reads as though somebody had
/// spaced its letters out on purpose. The size is unchanged either way, so the
/// two kinds of field sit on the same line.
pub fn readout_text<'a, M: 'static>(palette: Palette, name: &str, value: &str) -> Element<'a, M> {
    readout_field(palette, name, value, Length::Shrink, true)
}

/// A small text badge, for state and counts.
pub fn badge<'a, M: 'static>(text: impl Into<String>, color: Color) -> Element<'a, M> {
    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(color.scale_alpha(0.14))),
        border: Border {
            color: color.scale_alpha(0.30),
            width: stroke::HAIRLINE,
            radius: radius::SM.into(),
        },
        ..iced::widget::container::Style::default()
    };

    // Measured, so the pill is exactly as wide as the words in it. Without it
    // the pill keeps its own width and the words are drawn at the far end of
    // whatever the surrounding row offered, which puts them outside it.
    container(Label::new(text, Role::Micro, color).natural().view())
        .padding(pad_xy(space::SM, 2.0))
        .style(style)
        .into()
}

/// A caption under a control, for a hint or an error.
pub fn hint<'a, M: 'static>(
    text: impl Into<String>,
    palette: Palette,
    colour: Option<Color>,
) -> Element<'a, M> {
    Label::new(text, Role::Caption, colour.unwrap_or(palette.text_faint))
        .natural()
        .view()
        .into()
}

/// Vertical space that collapses to nothing when there is nothing to show.
pub fn optional_gap<'a, M: 'static>(visible: bool, size: f32) -> Element<'a, M> {
    if visible {
        Space::with_height(size).into()
    } else {
        Space::with_height(0.0).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::palette::contrast;

    const VARIANTS: [Emphasis; 4] = [
        Emphasis::Primary,
        Emphasis::Secondary,
        Emphasis::Ghost,
        Emphasis::Danger,
    ];

    #[test]
    fn every_emphasis_keeps_its_label_readable() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            for emphasis in VARIANTS {
                for (hover, press) in [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 1.0)] {
                    let fill = emphasis.fill(palette, hover, press);
                    let ratio = contrast(emphasis.ink(palette), fill);
                    assert!(
                        ratio >= 3.0,
                        "{emphasis:?} hover {hover} press {press}: contrast {ratio:.2}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_emphasis_answers_to_hover() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            for emphasis in VARIANTS {
                let resting = emphasis.fill(palette, 0.0, 0.0);
                let hovered = emphasis.fill(palette, 1.0, 0.0);
                let delta = (resting.r - hovered.r).abs()
                    + (resting.g - hovered.g).abs()
                    + (resting.b - hovered.b).abs();
                assert!(delta > 0.01, "{emphasis:?} gives no hover feedback");
            }
        }
    }

    #[test]
    fn a_press_goes_further_than_a_hover() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            for emphasis in VARIANTS {
                let hovered = emphasis.fill(palette, 1.0, 0.0);
                let pressed = emphasis.fill(palette, 1.0, 1.0);
                assert_ne!(hovered, pressed, "{emphasis:?} press is not distinct");
            }
        }
    }

    #[test]
    fn the_primary_action_is_the_only_one_that_lifts() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            assert_ne!(Emphasis::Primary.shadow(palette, 0.0), Shadow::default());
            for emphasis in [Emphasis::Secondary, Emphasis::Ghost, Emphasis::Danger] {
                assert_eq!(emphasis.shadow(palette, 1.0), Shadow::default());
            }
        }
    }

    #[test]
    fn a_hovered_primary_lifts_further() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            let resting = Emphasis::Primary.shadow(palette, 0.0);
            let hovered = Emphasis::Primary.shadow(palette, 1.0);
            assert!(hovered.blur_radius > resting.blur_radius);
        }
    }

    #[test]
    fn only_the_secondary_variant_is_outlined_at_rest() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            assert_ne!(Emphasis::Secondary.outline(palette), Color::TRANSPARENT);
            for emphasis in [Emphasis::Primary, Emphasis::Ghost, Emphasis::Danger] {
                assert_eq!(emphasis.outline(palette), Color::TRANSPARENT);
            }
        }
    }

    #[test]
    fn the_primary_action_is_the_tallest() {
        assert!(
            Emphasis::Primary.content_padding() > Emphasis::Secondary.content_padding(),
            "the one action a view wants taken should be reachable"
        );
    }

    #[test]
    fn the_switch_has_room_to_travel() {
        let (width, height) = SWITCH_SIZE;
        assert!(width > height * 1.5, "a switch should read as elongated");
        assert!(width - height >= 18.0, "there must be visible travel");
    }

    #[test]
    fn controls_render() {
        let clock = FrameClock::new();
        let palette = Palette::DARK;
        let interaction = Interaction::default();

        let _: Element<'static, ()> = ButtonSpec::new(
            interaction.clone(),
            clock.clone(),
            palette,
            Emphasis::Primary,
            crate::ui::typography::label("Start", &palette),
            (),
        )
        .icon(Icon::Play)
        .focused(true)
        .view();

        let _: Element<'static, ()> = IconButtonSpec::new(
            interaction.clone(),
            clock.clone(),
            palette,
            Emphasis::Ghost,
            Icon::Close,
            (),
        )
        .focused(false)
        .view();

        let _: Element<'static, ()> =
            SwitchSpec::new(interaction.clone(), clock.clone(), palette, true, ()).view();

        let _: Element<'static, ()> = ChipSpec::new(
            interaction.clone(),
            clock.clone(),
            palette,
            crate::ui::typography::label("Mon", &palette),
            true,
            (),
        )
        .icon(Icon::Check)
        .view();

        let _ = mark(Icon::Clock, 16.0, palette.text);
        let _ = readout::<()>(palette, "Zone", "CET");
        let _: Element<'static, ()> = badge("3", palette.accent);
        let _: Element<'static, ()> = hint("a hint", palette, None);
        let _: Element<'static, ()> = hint("an error", palette, Some(palette.danger));
        let _: Element<'static, ()> = optional_gap::<()>(true, 8.0);
        let _: Element<'static, ()> = optional_gap::<()>(false, 8.0);
    }
}
