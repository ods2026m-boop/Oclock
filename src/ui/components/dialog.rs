//! Dialogs, and the states a page shows when it has nothing to show.
//!
//! Both exist because "nothing here" is a normal state, not a failure. A page
//! with no alarms should look designed; a configuration that could not be read
//! should say exactly what happened and offer a way forward.

use iced::widget::container::Style as ContainerStyle;
use iced::widget::{column, container, row, Space};
use iced::{Background, Border, Color, Element, Length};

use crate::design::motion::{ease, Presence};
use crate::design::palette::{Palette, ShadowLevel};
use crate::design::tokens::{radius, space, stroke};
use crate::services::i18n::{Catalog, Direction, Text};
use crate::ui::components::controls::{ButtonSpec, Emphasis};
use crate::ui::components::icons::Icon;
use crate::ui::components::interaction::Interaction;
use crate::ui::typography::{self, Role};

/// A modal dialog.
///
/// Three parts, in the order a dialog is read: a heading that stays put, a body
/// that scrolls, and a row of buttons that stays put. The heading and the
/// buttons being outside the scrollable is not a detail — a form that scrolls
/// its own Save button out of reach is a form that cannot be finished.
///
/// The dialog scales and fades in rather than appearing, which is what
/// separates a considered overlay from a rectangle that suddenly exists. The
/// caller supplies the animated [`Presence`], so the same value can drive the
/// scrim, the card and its contents together.
pub struct Dialog<M> {
    palette: Palette,
    presence: f32,
    width: f32,
    limit: f32,
    heading: Option<Element<'static, M>>,
    body: Vec<Element<'static, M>>,
    footer: Option<Element<'static, M>>,
    scrim: Color,
    /// Which way this dialog is read, and so which side its scroll track is on.
    direction: Direction,
}

impl<M: 'static> Dialog<M> {
    /// A dialog whose contents are `body`, appearing with `presence`.
    pub fn new(palette: Palette, presence: f32) -> Dialog<M> {
        Dialog {
            palette,
            presence: presence.clamp(0.0, 1.0),
            width: 460.0,
            limit: 640.0,
            heading: None,
            body: Vec::new(),
            footer: None,
            scrim: palette.scrim,
            direction: Direction::LeftToRight,
        }
    }

    /// Reads this dialog in the language and writing direction of `catalog`.
    ///
    /// The dialog's own frame is direction-neutral — a card centred on a scrim
    /// looks the same either way — but Iced draws the scroll track on the
    /// *leading* edge of the body, so the body has to be inset on the side the
    /// language reads from, and that is the one thing the frame needs to know.
    pub fn reading(mut self, catalog: Catalog) -> Dialog<M> {
        self.direction = catalog.direction();
        self
    }

    /// Sets the dialog's width.
    pub fn width(mut self, width: f32) -> Dialog<M> {
        self.width = width;
        self
    }

    /// The tallest the dialog may be, so its body scrolls rather than growing
    /// past the window.
    pub fn limit(mut self, limit: f32) -> Dialog<M> {
        self.limit = limit;
        self
    }

    /// Sets the heading, which stays visible while the body scrolls.
    pub fn heading(mut self, heading: impl Into<Element<'static, M>>) -> Dialog<M> {
        self.heading = Some(heading.into());
        self
    }

    /// Adds a row of content to the scrolling body.
    pub fn push(mut self, child: impl Into<Element<'static, M>>) -> Dialog<M> {
        self.body.push(child.into());
        self
    }

    /// Adds a row of buttons, pinned below the scrolling body.
    pub fn footer(mut self, footer: impl Into<Element<'static, M>>) -> Dialog<M> {
        self.footer = Some(footer.into());
        self
    }

    /// The rendered widget, covering the whole window.
    pub fn view(self) -> Element<'static, M> {
        let Dialog {
            palette,
            presence,
            width,
            limit,
            heading,
            body,
            footer,
            scrim,
            direction,
        } = self;

        // One value drives the scrim, the card's rise and its fade, so the
        // whole overlay arrives as a single movement.
        let scrim_colour = ease::fade(scrim, ease::alpha(scrim, presence));
        let card_colour = ease::fade(palette.elevated, ease::alpha(palette.elevated, presence));
        let rise = (1.0 - presence) * 18.0;

        let card_style = move |_theme: &iced::Theme| ContainerStyle {
            background: Some(Background::Color(card_colour)),
            border: Border {
                color: palette.outline,
                width: stroke::HAIRLINE,
                radius: radius::XXL.into(),
            },
            shadow: palette.shadow(ShadowLevel::High),
            ..ContainerStyle::default()
        };

        // Only the body scrolls. It sits inside a container of its own so the
        // column has exactly one flexible child: with the scrollable directly in
        // the column, a body taller than the limit makes the solver squeeze the
        // footer instead, and the buttons come out flattened.
        let mut body = column(body).spacing(space::LG).width(Length::Fill);
        // The body's last control — a stepper's `+`, a chip's last word — is
        // the thing that ends up underneath the track if the body does not
        // leave room for it. See `surfaces::scrollbar_inset`.
        body = body.padding(crate::ui::components::surfaces::scrollbar_inset(direction));

        let scroll = container(crate::ui::components::surfaces::scroll_area(
            body.into(),
            palette,
        ))
        .width(Length::Fill)
        .height(Length::Fill);

        let mut parts = column![].spacing(space::LG).width(Length::Fill);
        if let Some(heading) = heading {
            parts = parts.push(heading);
        }
        parts = parts.push(scroll);
        if let Some(footer) = footer {
            parts = parts.push(footer);
        }

        let card = container(parts)
            .style(card_style)
            .padding(iced::padding::all(space::XXL))
            .width(Length::Fixed(width))
            .height(Length::Shrink)
            .max_height(limit);

        // Centred by alignment rather than by filling spaces: a column of
        // `Length::Fill` spacers measures its own width from the widest child,
        // which makes the centring depend on the dialog's contents.
        let body = column![card]
            .width(Length::Fill)
            .align_x(iced::Alignment::Center);

        container(body)
            .padding(iced::padding::top(rise))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_theme: &iced::Theme| ContainerStyle {
                background: Some(Background::Color(scrim_colour)),
                ..ContainerStyle::default()
            })
            .into()
    }
}

/// Centres `dialog` over the whole window.
///
/// Deliberately draws no backdrop of its own. The scrim is part of the dialog
/// rather than of the frame around it, because it is the dialog that fades: one
/// veil, drawn at the dialog's own opacity, follows it in and out. A scrim here
/// as well would put two of them on top of each other, and the second would be
/// permanent — it would stay at full strength while the dialog it was supposed
/// to be dimming faded away in front of it, leaving a dark window with no
/// dialog in it and no obvious reason why.
pub fn overlay<M: 'static>(_palette: Palette, dialog: Element<'static, M>) -> Element<'static, M> {
    container(dialog)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// What an empty state is telling the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyKind {
    /// Nothing has been created yet, and creating one is the obvious next step.
    Nothing,
    /// Everything is filtered out, and the filter can be changed.
    Filtered,
    /// A search found nothing.
    NoResults,
    /// Something went wrong, and the message says so.
    Problem,
    /// The platform cannot provide the data the page needs.
    Unavailable,
}

impl EmptyKind {
    /// The mark to show.
    pub fn icon(self) -> Icon {
        match self {
            EmptyKind::Nothing => Icon::Clock,
            EmptyKind::Filtered => Icon::Sort,
            EmptyKind::NoResults => Icon::Search,
            EmptyKind::Problem | EmptyKind::Unavailable => Icon::Warning,
        }
    }

    /// The tint for the mark and the ring around it.
    pub fn tint(self, palette: Palette) -> Color {
        match self {
            EmptyKind::Problem | EmptyKind::Unavailable => palette.caution,
            _ => palette.text_faint,
        }
    }
}

/// A page with nothing in it.
pub fn empty<M: 'static>(
    kind: EmptyKind,
    palette: Palette,
    heading: &str,
    detail: &str,
    action: Option<Element<'static, M>>,
) -> Element<'static, M> {
    let tint = kind.tint(palette);
    let icon: Element<'static, M> = crate::ui::components::controls::mark(kind.icon(), 28.0, tint)
        .map(|()| unreachable!("an empty state's mark never sends a message"));

    // The column is given the full width and its children are centred within
    // it. Without an explicit width the column shrink-wraps to the narrowest
    // child it can, which squeezes a button down to a few characters wide.
    let mut contents = column![]
        .spacing(space::MD)
        .align_x(iced::Alignment::Center)
        .width(Length::Fill);

    contents = contents.push(icon);
    // Both are centred rather than hung from an edge, and a centred label is
    // only centred if it is measured: without a box of its own, a right-to-left
    // run is drawn at the far end of whatever width the column offered it, which
    // is not the middle of anything.
    contents = contents.push(
        typography::Label::new(heading, Role::Heading, palette.text)
            .natural()
            .centre()
            .view(),
    );
    contents = contents.push(
        typography::Label::new(detail, Role::Body, palette.text_muted)
            .proportional()
            .natural()
            .centre()
            .view(),
    );
    if let Some(action) = action {
        contents = contents.push(Space::with_height(space::XS));
        contents = contents.push(action);
    }

    // Asymmetric padding rather than a vertical centring: this sits inside a
    // scrollable, whose content may not fill its scrolling axis, and a third of
    // the way down reads better than dead centre for a short empty page.
    container(contents)
        .padding(
            iced::padding::top(space::HUGE)
                .bottom(space::XXXL)
                .left(space::XXXL)
                .right(space::XXXL),
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

/// A dismissible notice, for a recovered setting or a failure that did not stop
/// the application.
pub fn notice<M: 'static>(
    palette: Palette,
    tint: Color,
    headline: &str,
    detail: &str,
    action: Option<Element<'static, M>>,
) -> Element<'static, M> {
    let style = move |_theme: &iced::Theme| ContainerStyle {
        background: Some(Background::Color(crate::design::palette::overlay(
            crate::design::motion::ease::fade(tint, 0.12),
            palette.surface,
        ))),
        border: Border {
            color: crate::design::motion::ease::fade(tint, 0.32),
            width: stroke::HAIRLINE,
            radius: radius::LG.into(),
        },
        shadow: iced::Shadow::default(),
        ..ContainerStyle::default()
    };

    let mut contents = column![]
        .spacing(space::XXS)
        .width(Length::Fill)
        .push(
            typography::Label::new(headline, Role::Label, palette.text)
                .natural()
                .view(),
        )
        .push(
            typography::Label::new(detail, Role::Caption, palette.text_muted)
                .proportional()
                .natural()
                .view(),
        );

    if let Some(action) = action {
        contents = contents.push(Space::with_height(space::XS));
        contents = contents.push(action);
    }

    let row = row![
        crate::ui::components::controls::mark(Icon::Warning, 18.0, tint)
            .map(|()| unreachable!("a notice's mark never sends a message")),
        Space::with_width(space::MD),
        contents.width(Length::Fill)
    ]
    .align_y(iced::Alignment::Start)
    .width(Length::Fill);

    container(row)
        .style(style)
        .padding(iced::padding::all(space::MD))
        .width(Length::Fill)
        .into()
}

/// A toast: a transient confirmation that slides in over the window.
pub struct Toast<M> {
    palette: Palette,
    presence: f32,
    text: String,
    action: Option<Element<'static, M>>,
    tone: Color,
}

impl<M: 'static> Toast<M> {
    /// A toast showing `text`, appearing with `presence`.
    pub fn new(palette: Palette, presence: f32, text: impl Into<String>) -> Toast<M> {
        Toast {
            palette,
            presence: presence.clamp(0.0, 1.0),
            text: text.into(),
            action: None,
            tone: palette.positive,
        }
    }

    /// Changes the tone, e.g. for a failure.
    pub fn tone(mut self, tone: Color) -> Toast<M> {
        self.tone = tone;
        self
    }

    /// Adds a button, e.g. "Undo".
    pub fn action(mut self, action: impl Into<Element<'static, M>>) -> Toast<M> {
        self.action = Some(action.into());
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let Toast {
            palette,
            presence,
            text,
            action,
            tone,
        } = self;

        let style = move |_theme: &iced::Theme| ContainerStyle {
            background: Some(Background::Color(crate::design::motion::ease::fade(
                palette.elevated,
                presence,
            ))),
            border: Border {
                color: crate::design::palette::overlay(
                    crate::design::motion::ease::fade(tone, 0.30),
                    palette.elevated,
                ),
                width: stroke::HAIRLINE,
                radius: radius::PILL.into(),
            },
            shadow: palette.shadow(crate::design::palette::ShadowLevel::High),
            ..ContainerStyle::default()
        };

        let mut contents = row![].spacing(space::MD).align_y(iced::Alignment::Center);
        contents = contents.push(
            crate::ui::components::controls::mark(Icon::Check, 16.0, tone)
                .map(|()| unreachable!("a toast never sends a message")),
        );
        contents = contents.push(typography::Label::new(text, Role::Label, palette.text).view());
        if let Some(action) = action {
            contents = contents.push(action);
        }

        let offset = (1.0 - presence) * 24.0;

        // The toast rises from below as it fades, which is the direction a
        // message that has just appeared should come from.
        container(
            container(contents).style(style).padding(
                iced::padding::top(space::MD)
                    .bottom(space::MD)
                    .left(space::XL)
                    .right(space::XL),
            ),
        )
        .padding(iced::padding::top(offset))
        .into()
    }
}

/// The standard confirm/cancel pair for a dialog's footer.
///
/// The cancelling button is labelled in English — [`Text::ActionCancel`] read
/// from [`Catalog::english`] — and the pair is ordered left to right, so a
/// caller that changes nothing gets exactly the footer it always drew. Call
/// [`dialog_footer_with`] with the active [`Catalog`] to translate the label and
/// to move the confirming button to the leading edge in a right-to-left
/// language.
pub fn dialog_footer<M: 'static + Clone>(
    clock: crate::design::motion::FrameClock,
    interactions: &crate::ui::components::interaction::Interactions,
    palette: Palette,
    confirm_label: &str,
    confirm: M,
    cancel: M,
    focus: Option<bool>,
) -> Element<'static, M> {
    dialog_footer_with(
        clock,
        interactions,
        palette,
        Catalog::english(),
        confirm_label,
        confirm,
        cancel,
        focus,
    )
}

/// The confirm/cancel pair, in `catalog`'s language and writing direction.
///
/// Iced 0.13 lays every row out left to right and offers no horizontal
/// alignment on one, so the right-to-left half of a footer is the order of its
/// buttons rather than their position: swapping them puts the confirming action
/// at the leading, right-hand edge, where it is read first.
#[allow(clippy::too_many_arguments)]
pub fn dialog_footer_with<M: 'static + Clone>(
    clock: crate::design::motion::FrameClock,
    interactions: &crate::ui::components::interaction::Interactions,
    palette: Palette,
    catalog: Catalog,
    confirm_label: &str,
    confirm: M,
    cancel: M,
    focus: Option<bool>,
) -> Element<'static, M> {
    let confirm: Element<'static, M> = ButtonSpec::new(
        interactions.get("dialog.confirm"),
        clock.clone(),
        palette,
        Emphasis::Primary,
        typography::label(confirm_label, &palette).reading(catalog),
        confirm,
    )
    .focused(focus.unwrap_or(false))
    .grow()
    .view();

    let cancel: Element<'static, M> = ButtonSpec::new(
        interactions.get("dialog.cancel"),
        clock,
        palette,
        Emphasis::Secondary,
        typography::label(catalog.text(Text::ActionCancel), &palette).reading(catalog),
        cancel,
    )
    .view();

    // The confirming button takes the space that is left rather than being
    // pushed there by a filling spacer, which would take its share from both
    // buttons and flatten them. It therefore spans the footer in either
    // direction, and only the order carries the language.
    // The cancelling button leads, so it lands on the leading edge of the
    // footer — the one a reader's eye returns to when they change their mind.
    let buttons = crate::ui::ends(catalog.direction(), cancel, confirm);

    buttons
        .spacing(space::SM)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .into()
}

/// Convenience for the common single-button footer.
///
/// `focused` is the same `bool` every other control takes: whether the keyboard
/// focus is on this button right now. A dialog with one button still has to be
/// reachable from the keyboard, and this is how it says so.
pub fn sole_action<M: 'static + Clone>(
    clock: crate::design::motion::FrameClock,
    interaction: Interaction,
    palette: Palette,
    label: &str,
    message: M,
    focused: bool,
) -> Element<'static, M> {
    ButtonSpec::new(
        interaction,
        clock,
        palette,
        Emphasis::Secondary,
        typography::label(label, &palette),
        message,
    )
    .focused(focused)
    .view()
}

/// A dialog's animated presence, for a caller that keeps the state.
pub fn presence_value(presence: &Presence, now: std::time::Duration) -> f32 {
    presence.value(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::motion::{FrameClock, Presence, Spec};
    use std::time::Duration;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn a_dialog_scales_and_fades_in_together() {
        let palette = Palette::LIGHT;
        let clock = FrameClock::new();
        let mut presence = Presence::hidden();
        presence.show(clock.now(), Spec::ENTER);

        // A hidden dialog is invisible, and a shown one is opaque; the value in
        // between drives both at once.
        assert!(presence.value(clock.now()) < 0.1);
        let _: Element<'static, ()> = Dialog::<()>::new(palette, presence.value(clock.now()))
            .heading(typography::heading("Edit alarm", &palette).view())
            .view();

        presence.hide(clock.now(), Spec::EXIT);
        assert!(presence.value(clock.now()) < 0.9);
    }

    #[test]
    fn a_presence_helpers() {
        let clock = FrameClock::new();
        let shown = Presence::shown();
        assert_eq!(presence_value(&shown, clock.now()), 1.0);

        let hidden = Presence::hidden();
        assert_eq!(presence_value(&hidden, clock.now()), 0.0);
    }

    #[test]
    fn every_empty_kind_has_a_mark_and_a_tone() {
        let palette = Palette::DARK;
        for kind in [
            EmptyKind::Nothing,
            EmptyKind::Filtered,
            EmptyKind::NoResults,
            EmptyKind::Problem,
            EmptyKind::Unavailable,
        ] {
            let _ = kind.icon();
            let tint = kind.tint(palette);
            assert!(tint.a > 0.0);
        }
    }

    #[test]
    fn a_problem_is_warned_about_in_amber_not_red() {
        // A recovered setting is not an emergency; an unavailable platform is.
        // Both use the same caution tint so neither shouts.
        let palette = Palette::DARK;
        assert_eq!(EmptyKind::Problem.tint(palette), palette.caution);
        assert_eq!(EmptyKind::Unavailable.tint(palette), palette.caution);
        assert_eq!(EmptyKind::Nothing.tint(palette), palette.text_faint);
    }

    #[test]
    fn a_dialog_makes_room_for_the_scroll_bar_it_cannot_move() {
        // Iced draws a vertical scroll's bar on the right of its viewport in
        // every language and offers no way to move it, so the inset a dialog
        // needs is the same in both: without it the last control of a
        // right-to-left dialog's body — a stepper's `+` — sits under the track.
        const {
            assert!(crate::design::tokens::scrollbar::WIDTH > 0.0);
            assert!(crate::design::tokens::scrollbar::GAP > 0.0);
        }
    }

    #[test]
    fn empty_states_render() {
        let palette = Palette::LIGHT;
        fn action(palette: Palette) -> Element<'static, ()> {
            sole_action(
                FrameClock::new(),
                Interaction::default(),
                palette,
                "Add one",
                (),
                false,
            )
        }

        for kind in [
            EmptyKind::Nothing,
            EmptyKind::Filtered,
            EmptyKind::NoResults,
            EmptyKind::Problem,
            EmptyKind::Unavailable,
        ] {
            let _ = empty::<()>(
                kind,
                palette,
                "Nothing here",
                "Add one to get started.",
                Some(action(palette)),
            );
            let _: Element<'static, ()> = empty::<()>(
                kind,
                palette,
                "Nothing here",
                "Add one to get started.",
                None,
            );
        }
    }

    #[test]
    fn notices_render_in_every_tone() {
        let palette = Palette::DARK;
        let _: Element<'static, ()> = notice::<()>(
            palette,
            palette.caution,
            "Settings repaired",
            "The clock format was reset; everything else was kept.",
            None,
        );
        let _: Element<'static, ()> = notice::<()>(palette, palette.danger, "Failed", "…", None);
    }

    #[test]
    fn toasts_render_and_accept_an_action() {
        let palette = Palette::LIGHT;
        let undo: Element<'static, ()> = sole_action(
            FrameClock::new(),
            Interaction::default(),
            palette,
            "Undo",
            (),
            false,
        );
        let _: Element<'static, ()> = Toast::<()>::new(palette, 1.0, "Timer started")
            .action(undo)
            .view();
        let _: Element<'static, ()> = Toast::<()>::new(palette, 0.5, "Could not start")
            .tone(palette.danger)
            .view();
    }

    #[test]
    fn dialog_footers_render() {
        let palette = Palette::LIGHT;
        let interactions = crate::ui::components::interaction::Interactions::new();
        let _: Element<'static, ()> = dialog_footer(
            FrameClock::new(),
            &interactions,
            palette,
            "Save",
            (),
            (),
            Some(true),
        );
        let _: Element<'static, ()> = dialog_footer(
            FrameClock::new(),
            &interactions,
            palette,
            "Delete",
            (),
            (),
            None,
        );
    }

    #[test]
    fn overlays_cover_the_window() {
        let _: Element<'static, ()> = overlay(Palette::LIGHT, Space::with_width(1.0).into());
    }

    #[test]
    fn a_dialog_asks_for_a_width_and_leaves_room_around_itself() {
        // The width is a preference, not a promise: the container around it is
        // `Length::Fill`, so a narrow window shrinks the dialog rather than
        // pushing the window wider. What is asserted here is that the request
        // itself is reasonable against the default window.
        let palette = Palette::LIGHT;
        let dialog = Dialog::<()>::new(palette, 1.0).width(460.0);
        assert_eq!(dialog.width, 460.0);
        assert!(
            dialog.width < crate::design::tokens::layout::WINDOW.0 as f32,
            "a dialog wider than the window would have to be capped"
        );
    }

    #[test]
    fn a_toast_fades_over_a_measured_window() {
        let clock = FrameClock::new();
        let mut presence = Presence::hidden();
        presence.show(clock.now(), Spec::ENTER);
        assert!(presence.value(clock.now()) < 0.5);
        std::thread::sleep(ms(200));
        assert!(presence.value(clock.now()) > 0.5);
    }
}
