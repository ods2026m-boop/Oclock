//! The Timer page: presets, running countdowns, and the completion state.

use iced::widget::{column, Space};
use iced::{Color, Element, Length};

use crate::app::focus::dialog as dialog_focus;
use crate::app::message::Message;
use crate::app::view::TimerDraft;
use crate::app::OClock;
use crate::design::motion::Spec;
use crate::design::palette::Palette;
use crate::design::tokens::space;
use crate::domain::fmt;
use crate::domain::sound::Sound;
use crate::domain::timer::{Timer, TimerPreset, TimerState};
use crate::services::i18n::Text;
use crate::ui::components::controls::{ButtonSpec, ChipSpec, Emphasis, IconButtonSpec};
use crate::ui::components::dialog::{self, EmptyKind};
use crate::ui::components::icons::Icon;
use crate::ui::components::indicators::Ring;
use crate::ui::components::inputs::{FieldSpec, StepperSpec};
use crate::ui::components::surfaces::{Card, Fill};
use crate::ui::typography::{self, Role};

/// The page.
pub fn view(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let mut column = column![].spacing(space::XXL).width(Length::Fill);

    column = column.push(presets(app, palette));

    if app.timers.is_empty() {
        column = column.push(dialog::empty::<Message>(
            EmptyKind::Nothing,
            palette,
            app.tr(Text::EmptyTimersTitle),
            app.tr(Text::EmptyTimersDetail),
            None,
        ));
    } else {
        let mut cards = Vec::with_capacity(app.timers.len());
        for (index, timer) in app.timers.timers().iter().enumerate() {
            // The card's own indices, from the same list the key activation reads: the
            // preset strip above the cards takes one index per chip.
            let base = app.timer_row_base() + index * 3;
            cards.push(timer_card(app, palette, timer, base));
        }
        column = column.push(crate::ui::components::surfaces::flow(cards, space::LG));
    }

    column.width(Length::Fill).into()
}

/// The preset strip.
fn presets(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let mut chips = Vec::with_capacity(app.timers.presets().len());

    for (index, preset) in app.timers.presets().iter().enumerate() {
        // Every chip takes the index the key activation reads, whether it is a
        // built-in or the user's own: a chip the user saved is a control on the
        // page in exactly the way a built-in one is.
        let chip = ChipSpec::new(
            app.interactions.get("timer.preset"),
            app.clock.clone(),
            palette,
            typography::Label::new(&preset.name, Role::Label, palette.text_muted)
                .reading(app.catalog),
            false,
            Message::StartPreset(preset.id),
        )
        .icon(Icon::Play)
        .focused(app.view.focus == app.page_focus(index + 1));

        chips.push(chip.view());
    }

    // The strip wraps rather than scrolling, so there is no container to mirror;
    // giving the list over in reading order is what makes the first preset the
    // one read first.
    if app.catalog.is_rtl() {
        chips.reverse();
    }

    Card::flat(palette)
        .pad_xy(space::XXL, space::XL)
        .spacing(space::MD)
        .fill_width()
        .push(
            typography::eyebrow(app.tr(Text::EyebrowPresets), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(crate::ui::components::surfaces::flow(chips, space::SM))
        .view()
}

/// One running or ready timer.
fn timer_card(
    app: &OClock,
    palette: Palette,
    timer: &Timer,
    base: usize,
) -> Element<'static, Message> {
    let now = app.elapsed();
    let remaining = timer.remaining(now);
    let progress = timer.progress(now);
    let done = timer.state == TimerState::Done;

    // The ring's colour is the timer's state, so a finished timer is obvious
    // from across the room.
    let tint = if done {
        palette.positive
    } else if timer.state == TimerState::Paused {
        palette.caution
    } else {
        palette.accent
    };

    let ring: Element<'static, Message> = Ring::new(
        palette,
        progress,
        app.clock.clone(),
        app.interactions.get("timer.ring"),
    )
    .size(196.0)
    .thickness(11.0)
    .tint(tint)
    .glow(if done { 1.0 } else { 0.0 })
    .view();

    let reading: Element<'static, Message> = column![
        typography::Label::new(fmt::countdown(remaining), Role::MetricLarge, palette.text)
            .reading(app.catalog)
            .view(),
        Space::with_height(space::XXS),
        typography::Label::new(
            app.catalog.timer_state(timer.state),
            Role::Caption,
            palette.text_faint,
        )
        .reading(app.catalog)
        .proportional()
        .view(),
    ]
    .spacing(0.0)
    .align_x(iced::Alignment::Center)
    .into();

    let playing: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("timer.play"),
        app.clock.clone(),
        palette,
        if timer.state.is_running() {
            Emphasis::Secondary
        } else {
            Emphasis::Primary
        },
        typography::label(
            app.tr(if timer.state.is_running() {
                Text::ActionPause
            } else {
                Text::ActionStart
            }),
            &palette,
        )
        .reading(app.catalog),
        Message::ToggleTimer(timer.id),
    )
    .icon(if timer.state.is_running() {
        Icon::Pause
    } else {
        Icon::Play
    })
    .focused(app.view.focus == app.page_focus(base))
    .view();

    let reset: Element<'static, Message> = IconButtonSpec::new(
        app.interactions.get("timer.reset"),
        app.clock.clone(),
        palette,
        Emphasis::Ghost,
        Icon::Reset,
        Message::ResetTimer(timer.id),
    )
    .focused(app.view.focus == app.page_focus(base + 1))
    .view();

    let remove: Element<'static, Message> = IconButtonSpec::new(
        app.interactions.get("timer.remove"),
        app.clock.clone(),
        palette,
        Emphasis::Ghost,
        Icon::Trash,
        Message::RemoveTimer(timer.id),
    )
    .focused(app.view.focus == app.page_focus(base + 2))
    .view();

    // Iced 0.13 lays a row out left to right and offers no horizontal alignment
    // on one, so the right-to-left half of this is the filling space: moving it
    // to the front pins the controls to the leading, right-hand edge. The
    // playing button keeps its place in the pair either way, because a control
    // that jumped sides between readings would be missed.
    // The controls lead, and the filling space is on their trailing side, so
    // they sit against the leading edge of the card whichever way it is written.
    // The playing button keeps its place in the pair either way, because a
    // control that jumped sides between readings would be missed.
    let controls: Element<'static, Message> = crate::ui::ordered(
        app.catalog.direction(),
        vec![
            playing,
            Space::with_width(space::SM).into(),
            reset,
            remove,
            Space::with_width(Length::Fill).into(),
        ],
    )
    .spacing(space::XXS)
    .align_y(iced::Alignment::Center)
    .into();

    Card::new(palette, if done { Fill::Accent } else { Fill::Surface })
        .pad_xy(space::XL, space::XL)
        .spacing(space::MD)
        .push(
            column![ring, Space::with_height(space::MD), reading]
                .width(Length::Fill)
                .align_x(iced::Alignment::Center),
        )
        .push(
            typography::Label::new(&timer.label, Role::Heading, palette.text)
                .reading(app.catalog)
                .proportional()
                .centre()
                .view(),
        )
        .push(controls)
        .width(Length::Fixed(260.0))
        .into()
}

/// The custom timer editor.
pub fn editor(
    app: &OClock,
    palette: Palette,
    draft: &TimerDraft,
    presence: f32,
) -> Element<'static, Message> {
    let name: Element<'static, Message> = FieldSpec::new(
        app.interactions.get("timer.name"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionName),
        draft.name.clone(),
        Message::SetTimerName,
    )
    .placeholder(app.tr(Text::PlaceholderTimer))
    .hint(app.tr(Text::HintEditing))
    .reading(app.catalog)
    .view();

    let minutes = (draft.duration.as_secs() / 60) as u32;
    let hours = minutes / 60;
    let rest = minutes % 60;

    let hour: Element<'static, Message> = StepperSpec::new(
        app.interactions.get("timer.hours"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionHours),
        format!("{hours:02}"),
        Message::TimerDurationStep(-60),
        Message::TimerDurationStep(60),
    )
    .reading(app.catalog)
    .focused(app.view.focus == Some(dialog_focus::TIMER_HOURS))
    .view();

    let minute: Element<'static, Message> = StepperSpec::new(
        app.interactions.get("timer.minutes"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionMinutes),
        format!("{rest:02}"),
        Message::TimerDurationStep(-5),
        Message::TimerDurationStep(5),
    )
    .reading(app.catalog)
    .focused(app.view.focus == Some(dialog_focus::TIMER_MINUTES))
    .view();

    let preview: Element<'static, Message> = Card::new(palette, Fill::Accent)
        .pad_xy(space::XXL, space::LG)
        .spacing(space::XXS)
        .fill_width()
        .push(
            typography::eyebrow(app.tr(Text::EyebrowWillRunFor), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(
            typography::Label::new(
                fmt::countdown_padded(draft.duration),
                Role::Metric,
                palette.text,
            )
            .reading(app.catalog)
            .view(),
        )
        .view();

    let mut sounds = Vec::with_capacity(Sound::ALL.len());
    for sound in Sound::ALL {
        sounds.push(
            ChipSpec::new(
                app.interactions.get("timer.sound"),
                app.clock.clone(),
                palette,
                typography::Label::new(
                    app.catalog.sound_name(sound),
                    Role::Label,
                    palette.text_muted,
                )
                .reading(app.catalog),
                app.sound.timer_sound == sound,
                Message::SetTimerSound(sound),
            )
            .view(),
        );
    }

    let footer = dialog::dialog_footer_with(
        app.clock.clone(),
        &app.interactions,
        palette,
        app.catalog,
        app.tr(Text::ActionStart),
        Message::ConfirmTimer,
        Message::CancelTimerEdit,
        app.view
            .focus
            .map(|index| index == dialog_focus::TIMER_CONFIRM),
    );

    dialog::Dialog::<Message>::new(palette, presence)
        .reading(app.catalog)
        .limit(crate::ui::pages::dialog_limit(app))
        .width(crate::ui::pages::dialog_width(app, 480.0))
        .push(
            typography::heading(app.tr(Text::NewTimerTitle), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(name)
        .push(crate::ui::ends(
            app.catalog.direction(),
            hour,
            crate::ui::ends(
                app.catalog.direction(),
                Space::with_width(space::MD),
                minute,
            ),
        ))
        .push(preview)
        .push(
            typography::eyebrow(app.tr(Text::EyebrowSound), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(crate::ui::components::surfaces::flow(sounds, space::SM))
        .footer(footer)
        .view()
}

/// A preset row, exposed for the tests.
pub fn preset_row(
    app: &OClock,
    palette: Palette,
    preset: &TimerPreset,
) -> Element<'static, Message> {
    let _: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("timer.preset"),
        app.clock.clone(),
        palette,
        Emphasis::Secondary,
        typography::Label::new(&preset.name, Role::Label, palette.text_muted).reading(app.catalog),
        Message::StartPreset(preset.id),
    )
    .icon(Icon::Play)
    .view();
    Space::with_width(0.0).into()
}

/// A short description of a timer, for a toast.
pub fn describe(app: &OClock, timer: &Timer) -> String {
    format!(
        "{} · {}",
        timer.label,
        fmt::countdown(timer.remaining(app.elapsed()))
    )
}

/// The colour a finished timer is announced in.
pub fn completion_colour(palette: Palette) -> Color {
    palette.positive
}

/// The motion a completed ring uses.
pub fn completion_motion() -> Spec {
    Spec::CELEBRATE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_renders_with_and_without_timers() {
        let app = crate::app::tests::sample();
        let _: Element<'static, Message> = view(&app, Palette::LIGHT);

        let app = crate::app::tests::sample_with_timer();
        let _: Element<'static, Message> = view(&app, Palette::DARK);
    }

    #[test]
    fn a_finished_timer_is_shown_as_finished() {
        let app = crate::app::tests::sample_with_timer();
        let _: Element<'static, Message> = view(&app, Palette::DARK);
        let timer = app.timers.timers()[0].clone();
        assert!(describe(&app, &timer).starts_with(&timer.label));
    }

    #[test]
    fn the_editor_renders() {
        let app = crate::app::tests::sample();
        let draft = TimerDraft::new("Tea", std::time::Duration::from_secs(300));
        let _: Element<'static, Message> = editor(&app, Palette::LIGHT, &draft, 1.0);
        let _: Element<'static, Message> = editor(&app, Palette::LIGHT, &draft, 0.0);
    }

    #[test]
    fn helpers_are_neutral() {
        let palette = Palette::LIGHT;
        assert_eq!(completion_colour(palette), palette.positive);
        assert_eq!(completion_motion().duration, Spec::CELEBRATE.duration);
    }
}
