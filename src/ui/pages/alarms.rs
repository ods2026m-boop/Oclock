//! The Alarms page, its editor, and the ringing presentation.

use chrono::{Datelike, Weekday};
use iced::widget::{column, container, Column, Space};
use iced::{Background, Element, Length};

use crate::app::focus::dialog as dialog_focus;
use crate::app::message::Message;
use crate::app::view::AlarmDraft;
use crate::app::OClock;
use crate::design::palette::Palette;
use crate::design::tokens::space;
use crate::domain::alarm::{Repeat, Weekdays};
use crate::domain::sound::Sound;
use crate::services::i18n::{Arg, Text};
use crate::ui::components::controls::{
    self, ButtonSpec, ChipSpec, Emphasis, IconButtonSpec, SwitchSpec,
};
use crate::ui::components::dialog::{self, EmptyKind};
use crate::ui::components::icons::Icon;
use crate::ui::components::inputs::{weekday_row_with, FieldSpec, StepperSpec};
use crate::ui::components::surfaces::{Card, Fill};
use crate::ui::typography::{self, Role};

/// The page.
pub fn view(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let mut stack: Vec<Element<'static, Message>> = Vec::new();

    // What is coming, above the list: an alarm app that only lists the past is
    // an alarm app nobody trusts.
    stack.push(next_up(app, palette));

    if !app.view.missed.is_empty() {
        stack.push(missed(app, palette));
    }

    if app.alarms.is_empty() {
        return dialog::empty::<Message>(
            EmptyKind::Nothing,
            palette,
            app.tr(Text::EmptyAlarmsTitle),
            app.tr(Text::EmptyAlarmsDetail),
            Some(
                ButtonSpec::new(
                    app.interactions.get("alarms.empty.add"),
                    app.clock.clone(),
                    palette,
                    Emphasis::Primary,
                    typography::label(app.tr(Text::ActionNewAlarm), &palette).reading(app.catalog),
                    Message::NewAlarm,
                )
                .icon(Icon::Plus)
                .view(),
            ),
        );
    }

    let mut rows = Vec::with_capacity(app.alarms.len());
    for (index, alarm) in app.alarms.alarms().iter().enumerate() {
        // The row's own indices, from the same list the key activation reads: the
        // missed-alarm notice above the list takes one index while it is on screen,
        // and nothing else does.
        let base = app.alarm_row_base() + index * 3;
        rows.push(alarm_row(app, palette, alarm.id, base));
    }

    stack.push(column(rows).spacing(space::MD).width(Length::Fill).into());
    stack.push(Space::with_height(space::SM).into());
    column(stack).spacing(space::LG).into()
}

/// The next alarm, and how long until it.
fn next_up(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let Some(next) = app.alarms.next_up() else {
        return Space::with_height(0.0).into();
    };

    let remaining = (next - app.snapshot.now)
        .to_std()
        .unwrap_or(std::time::Duration::ZERO);

    let clock: Element<'static, Message> =
        typography::Label::new(app.catalog.time_24h(next), Role::Metric, palette.text)
            .reading(app.catalog)
            .natural()
            .view()
            .into();
    let when: Element<'static, Message> = typography::Label::new(
        format!(
            "{} {}",
            app.catalog.weekday_short(next.date_naive().weekday()),
            app.catalog.date_short(next)
        ),
        Role::Body,
        palette.text_muted,
    )
    .reading(app.catalog)
    .proportional()
    .natural()
    .view()
    .into();
    let until: Element<'static, Message> = typography::Label::new(
        app.tr_with(
            Text::InDuration,
            &[Arg::from(app.catalog.duration(remaining))],
        ),
        Role::Label,
        palette.accent_text,
    )
    .reading(app.catalog)
    .natural()
    .view()
    .into();

    // The time leads, the date follows, and the wait until then is a status the
    // reader is looking for, so it comes last — the reading order of the card.
    let parts: Vec<Element<'static, Message>> = vec![
        clock,
        Space::with_width(space::MD).into(),
        when,
        Space::with_width(Length::Fill).into(),
        until,
    ];

    Card::new(palette, Fill::Accent)
        .pad_xy(space::XXL, space::XL)
        .spacing(space::XXS)
        .fill_width()
        .push(
            typography::eyebrow(app.tr(Text::EyebrowNextAlarm), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(crate::ui::ordered(app.catalog.direction(), parts).align_y(iced::Alignment::End))
        .view()
}

/// The alarms whose moment passed unnoticed.
fn missed(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let mut items: Column<'_, Message> = column![].spacing(space::XXS);
    for (alarm, late) in &app.view.missed {
        items = items.push(
            typography::Label::new(
                app.tr_with(
                    Text::AlarmsMissedLine,
                    &[
                        Arg::from(app.catalog.alarm_time(alarm, app.display().hour_format)),
                        Arg::from(&alarm.label),
                        Arg::from(app.catalog.duration(*late)),
                    ],
                ),
                Role::Caption,
                palette.text_muted,
            )
            .proportional()
            .view(),
        );
    }

    let dismiss: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("alarms.missed.dismiss"),
        app.clock.clone(),
        palette,
        Emphasis::Secondary,
        typography::label(app.tr(Text::ActionGotIt), &palette).reading(app.catalog),
        Message::AcknowledgeAllMissed,
    )
    // The notice sits between the page's header button and its list, so it
    // takes the index between them — while it is on screen, which is the only
    // time the control exists.
    .focused(app.view.focus == app.page_focus(1))
    .view();

    dialog::notice::<Message>(
        palette,
        palette.caution,
        app.tr(Text::AlarmsMissedHeadline),
        &app.tr_with(
            Text::AlarmsMissedDetail,
            &[Arg::from(app.view.missed.len())],
        ),
        Some(dismiss),
    )
}

/// One alarm in the list.
fn alarm_row(app: &OClock, palette: Palette, id: u64, base: usize) -> Element<'static, Message> {
    let Some(alarm) = app.alarms.get(id) else {
        return Space::with_height(0.0).into();
    };

    let pending = app.alarms.pending(id);
    let armed = alarm.is_armed();

    let time = app.catalog.alarm_time(alarm, app.display().hour_format);
    let next = pending.map(|entry| entry.at);

    let schedule = match next {
        Some(at) if armed => {
            let day = format!(
                "{} {}",
                app.catalog.weekday_short(at.date_naive().weekday()),
                app.catalog.date_short(at)
            );
            format!("{} · {}", day, app.catalog.time_24h(at))
        }
        Some(_) => app.tr(Text::StatusNotScheduled).to_string(),
        None if armed => app.tr(Text::StatusWaiting).to_string(),
        None => app.tr(Text::StatusOff).to_string(),
    };

    let toggle: Element<'static, Message> = SwitchSpec::new(
        app.interactions.get("alarms.toggle"),
        app.clock.clone(),
        palette,
        armed,
        Message::ToggleAlarm(id),
    )
    .focused(app.view.focus == app.page_focus(base))
    .view();

    Card::new(palette, if armed { Fill::Surface } else { Fill::Sunken })
        .pad_xy(space::XL, space::LG)
        .spacing(space::MD)
        .fill_width()
        .push(
            crate::ui::sequence(
                app.catalog.direction(),
                column![
                    typography::Label::new(&alarm.label, Role::Heading, palette.text)
                        .reading(app.catalog)
                        .proportional()
                        .natural()
                        .view(),
                    Space::with_height(space::XXS),
                    typography::Label::new(
                        app.catalog.repeat(alarm.repeat),
                        Role::Caption,
                        palette.text_muted
                    )
                    .reading(app.catalog)
                    .proportional()
                    .natural()
                    .view(),
                ]
                .spacing(0.0),
                Space::with_width(Length::Fill),
                crate::ui::sequence(
                    app.catalog.direction(),
                    Space::with_width(space::XL),
                    column![
                        typography::Label::new(&time, Role::Metric, palette.text)
                            .reading(app.catalog)
                            .natural()
                            .view(),
                        typography::Label::new(schedule, Role::Caption, palette.text_faint)
                            .reading(app.catalog)
                            .proportional()
                            .natural()
                            .view(),
                    ]
                    .spacing(0.0),
                    toggle,
                ),
            )
            .align_y(iced::Alignment::Center),
        )
        .push(
            crate::ui::ends(
                app.catalog.direction(),
                crate::ui::ends(
                    app.catalog.direction(),
                    controls::badge(app.catalog.sound_name(alarm.sound), palette.accent_text),
                    crate::ui::ends(
                        app.catalog.direction(),
                        Space::with_width(space::SM),
                        controls::badge(
                            app.tr_with(Text::SnoozeLower, &[Arg::from(alarm.snooze_minutes)]),
                            palette.text_faint,
                        ),
                    ),
                ),
                Space::with_width(Length::Fill),
            )
            .width(Length::Fill),
        )
        .push(
            container(
                crate::ui::ordered(
                    app.catalog.direction(),
                    vec![
                        IconButtonSpec::new(
                            app.interactions.get("alarms.edit"),
                            app.clock.clone(),
                            palette,
                            Emphasis::Ghost,
                            Icon::Pencil,
                            Message::EditAlarm(id),
                        )
                        .focused(app.view.focus == app.page_focus(base + 1))
                        .view(),
                        IconButtonSpec::new(
                            app.interactions.get("alarms.delete"),
                            app.clock.clone(),
                            palette,
                            Emphasis::Ghost,
                            Icon::Trash,
                            Message::DeleteAlarm(id),
                        )
                        .focused(app.view.focus == app.page_focus(base + 2))
                        .view(),
                    ],
                )
                .spacing(space::XXS)
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .align_x(crate::ui::leading(app.catalog.direction())),
        )
        .into()
}

/// The alarm editor.
pub fn editor(
    app: &OClock,
    palette: Palette,
    draft: &AlarmDraft,
    presence: f32,
) -> Element<'static, Message> {
    let format = app.display().hour_format;

    let name: Element<'static, Message> = FieldSpec::new(
        app.interactions.get("alarms.name"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionName),
        draft.alarm.label.clone(),
        Message::SetAlarmName,
    )
    .placeholder(app.tr(Text::PlaceholderAlarm))
    .hint(app.tr(Text::HintEditing))
    .reading(app.catalog)
    .view();

    let hour: Element<'static, Message> = StepperSpec::new(
        app.interactions.get("alarms.hour"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionHour),
        format!("{:02}", draft.alarm.hour),
        Message::AlarmHourStep(-1),
        Message::AlarmHourStep(1),
    )
    .reading(app.catalog)
    .focused(app.view.focus == Some(dialog_focus::ALARM_HOUR))
    .view();

    let minute: Element<'static, Message> = StepperSpec::new(
        app.interactions.get("alarms.minute"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionMinute),
        format!("{:02}", draft.alarm.minute),
        Message::AlarmMinuteStep(-5),
        Message::AlarmMinuteStep(5),
    )
    .reading(app.catalog)
    .focused(app.view.focus == Some(dialog_focus::ALARM_MINUTE))
    .view();

    let time = Card::new(palette, Fill::Accent)
        .pad_xy(space::XXL, space::LG)
        .spacing(space::XXS)
        .fill_width()
        .push(
            typography::eyebrow(app.tr(Text::EyebrowTime), &palette)
                .reading(app.catalog)
                .view(),
        )
        // The digits and the meridiem share a baseline rather than being one
        // string. A 12-hour reading in Persian, Arabic or Urdu ends in a word
        // rather than in "AM", and at the same size as the digits that word is
        // wide enough to wrap the whole form onto a second line — which is why
        // the dialog is taller in those three languages than in the other nine.
        .push(time_reading::<Message>(
            &draft.time_parts(format, app.catalog),
            &palette,
            app.catalog,
        ));

    let repeat: Element<'static, Message> = crate::ui::ordered(
        app.catalog.direction(),
        vec![
            ChipSpec::new(
                app.interactions.get("alarms.repeat.once"),
                app.clock.clone(),
                palette,
                typography::Label::new(app.tr(Text::RepeatOnce), Role::Label, palette.text_muted)
                    .reading(app.catalog),
                matches!(draft.alarm.repeat, Repeat::Once),
                Message::SetRepeat(Repeat::Once),
            )
            .view(),
            ChipSpec::new(
                app.interactions.get("alarms.repeat.daily"),
                app.clock.clone(),
                palette,
                typography::Label::new(app.tr(Text::RepeatDaily), Role::Label, palette.text_muted)
                    .reading(app.catalog),
                matches!(draft.alarm.repeat, Repeat::Daily),
                Message::SetRepeat(Repeat::Daily),
            )
            .view(),
            ChipSpec::new(
                app.interactions.get("alarms.repeat.weekly"),
                app.clock.clone(),
                palette,
                typography::Label::new(
                    app.tr(Text::RepeatSomeDays),
                    Role::Label,
                    palette.text_muted,
                )
                .reading(app.catalog),
                matches!(draft.alarm.repeat, Repeat::Weekly(_)),
                Message::SetRepeat(Repeat::Weekly(Weekdays::WORKDAYS)),
            )
            .view(),
        ],
    )
    .spacing(space::SM)
    .width(Length::Fill)
    .into();

    let weekdays = if matches!(draft.alarm.repeat, Repeat::Weekly(_)) {
        weekday_row_with::<Message>(
            palette,
            app.catalog,
            app.clock.clone(),
            app.interactions.get("alarms.weekday"),
            match draft.alarm.repeat {
                Repeat::Weekly(days) => days,
                _ => Weekdays::WORKDAYS,
            },
            (0..7).map(Message::ToggleWeekday).collect(),
        )
    } else {
        Space::with_height(0.0).into()
    };

    let mut sounds = Vec::with_capacity(Sound::ALL.len());
    for sound in Sound::ALL {
        sounds.push(
            ChipSpec::new(
                app.interactions.get("alarms.sound"),
                app.clock.clone(),
                palette,
                typography::Label::new(
                    app.catalog.sound_name(sound),
                    Role::Label,
                    palette.text_muted,
                )
                .reading(app.catalog),
                draft.alarm.sound == sound,
                Message::SetAlarmSound(sound),
            )
            .view(),
        );
    }

    let snooze: Element<'static, Message> = StepperSpec::new(
        app.interactions.get("alarms.snooze"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionSnooze),
        app.tr_with(Text::MinutesShort, &[Arg::from(draft.alarm.snooze_minutes)]),
        Message::SetSnoozeMinutes(0), // replaced below
        Message::SetSnoozeMinutes(draft.alarm.snooze_minutes.saturating_add(1)),
    )
    .reading(app.catalog)
    .view();

    let footer = dialog::dialog_footer_with(
        app.clock.clone(),
        &app.interactions,
        palette,
        app.catalog,
        app.tr(Text::ActionSave),
        Message::SaveAlarm,
        Message::CancelAlarmEdit,
        app.view
            .focus
            .map(|index| index == dialog_focus::ALARM_CONFIRM),
    );

    dialog::Dialog::<Message>::new(palette, presence)
        .reading(app.catalog)
        .limit(crate::ui::pages::dialog_limit(app))
        .width(crate::ui::pages::dialog_width(app, 520.0))
        .push(
            typography::heading(
                app.tr(if draft.is_new {
                    Text::NewAlarmTitle
                } else {
                    Text::EditAlarmTitle
                }),
                &palette,
            )
            .reading(app.catalog)
            .view(),
        )
        .push(name)
        .push(time)
        // The two steppers are stacked rather than set side by side. Side by
        // side they need about four hundred and thirty pixels at the width this
        // dialog is given, and a clock face that clips its own last control is
        // worse than one that is a little taller — and in German and Russian
        // the captions are longer still, so it would clip further.
        .push(column![hour, minute].spacing(space::SM).width(Length::Fill))
        .push(
            typography::eyebrow(app.tr(Text::EyebrowRepeat), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(repeat)
        .push(weekdays)
        .push(
            typography::eyebrow(app.tr(Text::EyebrowSound), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(surfaces_flow(sounds))
        .push(
            typography::Label::new(
                app.catalog.sound_description(draft.alarm.sound),
                Role::Caption,
                palette.text_faint,
            )
            .reading(app.catalog)
            .proportional()
            .view(),
        )
        .push(snooze)
        .footer(footer)
        .view()
}

/// The deletion confirmation.
pub fn confirm_delete(
    app: &OClock,
    palette: Palette,
    id: u64,
    presence: f32,
) -> Element<'static, Message> {
    let name = app
        .alarms
        .get(id)
        .map(|alarm| alarm.label.clone())
        .unwrap_or_default();
    let time = app
        .alarms
        .get(id)
        .map(|alarm| app.catalog.alarm_time(alarm, app.display().hour_format))
        .unwrap_or_default();

    let _style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(crate::design::palette::overlay(
            crate::design::motion::ease::fade(palette.danger, 0.12),
            palette.elevated,
        ))),
        ..iced::widget::container::Style::default()
    };

    let footer = dialog::dialog_footer_with(
        app.clock.clone(),
        &app.interactions,
        palette,
        app.catalog,
        app.tr(Text::ActionDelete),
        Message::ConfirmDeleteAlarm,
        Message::CancelDeleteAlarm,
        app.view
            .focus
            .map(|index| index == dialog_focus::DELETE_CANCEL),
    );

    dialog::Dialog::<Message>::new(palette, presence)
        .reading(app.catalog)
        .limit(crate::ui::pages::dialog_limit(app))
        .width(crate::ui::pages::dialog_width(app, 420.0))
        .push(
            typography::heading(app.tr(Text::DeleteAlarmTitle), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(
            typography::Label::new(
                app.tr_with(Text::DeleteAlarmBody, &[Arg::from(&name), Arg::from(&time)]),
                Role::Body,
                palette.text_muted,
            )
            .reading(app.catalog)
            .proportional()
            .view(),
        )
        .footer(footer)
        .view()
}

/// The ringing presentation, which takes over the window.
pub fn ringing(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let Some(alarm) = app.view.ringing.first() else {
        return Space::with_width(1.0).into();
    };
    let alarm = &alarm.alarm;
    let ringing = &app.view.ringing[0];

    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(crate::design::palette::overlay(
            crate::design::motion::ease::fade(palette.danger, 0.10),
            palette.elevated,
        ))),
        border: iced::Border {
            color: crate::design::motion::ease::fade(palette.danger, 0.40),
            width: 1.0,
            radius: 24.0.into(),
        },
        shadow: palette.shadow(crate::design::palette::ShadowLevel::High),
        ..iced::widget::container::Style::default()
    };

    let snooze: Element<'static, Message> = if ringing.snoozable {
        ButtonSpec::new(
            app.interactions.get("ringing.snooze"),
            app.clock.clone(),
            palette,
            Emphasis::Primary,
            typography::label(
                app.tr_with(Text::SnoozeUpper, &[Arg::from(alarm.snooze_minutes)]),
                &palette,
            )
            .reading(app.catalog),
            Message::SnoozeAlarm(alarm.id),
        )
        .icon(Icon::Snooze)
        .view()
    } else {
        Space::with_width(0.0).into()
    };

    let dismiss: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("ringing.dismiss"),
        app.clock.clone(),
        palette,
        Emphasis::Secondary,
        typography::label(app.tr(Text::ActionDismiss), &palette).reading(app.catalog),
        Message::DismissAlarm(alarm.id),
    )
    .icon(Icon::Check)
    .view();

    let extra: Element<'static, Message> = if app.view.ringing.len() > 1 {
        typography::Label::new(
            app.tr_with(
                Text::AndMore,
                &[Arg::from(app.view.ringing.len().saturating_sub(1))],
            ),
            Role::Caption,
            palette.text_muted,
        )
        .reading(app.catalog)
        .natural()
        .view()
        .into()
    } else {
        Space::with_height(0.0).into()
    };

    container(
        column![
            typography::Label::new(&alarm.label, Role::Title, palette.text)
                .reading(app.catalog)
                .proportional()
                .natural()
                .view(),
            Space::with_height(space::SM),
            typography::Label::new(
                app.catalog.alarm_time(alarm, app.display().hour_format),
                Role::MetricLarge,
                palette.text,
            )
            .reading(app.catalog)
            .natural()
            .view(),
            Space::with_height(space::XS),
            typography::Label::new(
                app.catalog.repeat(alarm.repeat),
                Role::Body,
                palette.text_muted
            )
            .reading(app.catalog)
            .proportional()
            .natural()
            .view(),
            Space::with_height(space::MD),
            extra,
            Space::with_height(space::XXL),
            crate::ui::ends(
                app.catalog.direction(),
                dismiss,
                crate::ui::ends(
                    app.catalog.direction(),
                    Space::with_width(space::MD),
                    snooze,
                ),
            )
            .spacing(space::MD)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(0.0)
        .align_x(iced::Alignment::Center),
    )
    .style(style)
    .padding(iced::padding::all(space::XXXL))
    .width(Length::Fixed(440.0))
    .into()
}

/// A wrapping row, re-exported so the pages agree on the rhythm.
fn surfaces_flow(parts: Vec<Element<'static, Message>>) -> Element<'static, Message> {
    crate::ui::components::surfaces::flow(parts, space::SM)
}

/// The big reading at the top of the alarm form: the digits, and the meridiem
/// beside them at a smaller size if the language spells one out.
fn time_reading<M: 'static>(
    parts: &(String, Option<&'static str>),
    palette: &Palette,
    catalog: crate::services::i18n::Catalog,
) -> Element<'static, M> {
    let (digits, meridiem) = parts;
    let digits =
        typography::Label::new(digits.clone(), Role::MetricLarge, palette.text).reading(catalog);

    let Some(meridiem) = meridiem else {
        return digits.view().into();
    };

    let meridiem = typography::Label::new(*meridiem, Role::Title, palette.text_muted)
        .reading(catalog)
        .proportional();
    crate::baseline![digits, meridiem].view()
}

/// The weekday a number refers to, exposed for the tests.
pub fn weekday_of(index: u8) -> Option<Weekday> {
    Weekday::try_from(index).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_renders_empty_and_full() {
        let app = crate::app::tests::sample();
        let _: Element<'static, Message> = view(&app, Palette::LIGHT);

        let mut app = crate::app::tests::sample_with_alarm();
        let _: Element<'static, Message> = view(&app, Palette::DARK);
        app.view.missed.push((
            app.alarms.alarms()[0].clone(),
            std::time::Duration::from_secs(600),
        ));
        let _: Element<'static, Message> = view(&app, Palette::DARK);
    }

    #[test]
    fn the_editor_renders_for_new_and_existing_alarms() {
        let app = crate::app::tests::sample_with_alarm();
        let draft = AlarmDraft::new(1, 7, 30);
        let _: Element<'static, Message> = editor(&app, Palette::LIGHT, &draft, 1.0);

        let existing = AlarmDraft::existing(app.alarms.alarms()[0].clone());
        let _: Element<'static, Message> = editor(&app, Palette::DARK, &existing, 0.5);
    }

    #[test]
    fn the_editor_shows_the_weekly_row_only_for_a_weekly_rule() {
        let app = crate::app::tests::sample_with_alarm();
        let mut daily = AlarmDraft::new(1, 7, 30);
        let _: Element<'static, Message> = editor(&app, Palette::LIGHT, &daily, 1.0);

        daily.alarm.repeat = Repeat::Weekly(Weekdays::from_days([Weekday::Mon, Weekday::Thu]));
        let _: Element<'static, Message> = editor(&app, Palette::LIGHT, &daily, 1.0);
    }

    #[test]
    fn the_delete_confirmation_renders() {
        let app = crate::app::tests::sample_with_alarm();
        let id = app.alarms.alarms()[0].id;
        let _: Element<'static, Message> = confirm_delete(&app, Palette::LIGHT, id, 1.0);
    }

    #[test]
    fn a_ringing_alarm_offers_a_snooze_and_a_dismiss() {
        let mut app = crate::app::tests::sample_with_alarm();
        let alarm = app.alarms.alarms()[0].clone();
        app.view.ringing.push(crate::app::view::Ringing {
            alarm: alarm.clone(),
            due: app.snapshot.now,
            late_by: std::time::Duration::from_secs(2),
            snoozable: true,
        });
        let _: Element<'static, Message> = ringing(&app, Palette::DARK);

        // And not a snooze when it would be useless.
        app.view.ringing[0].snoozable = false;
        let _: Element<'static, Message> = ringing(&app, Palette::DARK);
    }

    #[test]
    fn weekdays_map_from_their_numbers() {
        assert_eq!(weekday_of(0), Some(Weekday::Mon));
        assert_eq!(weekday_of(6), Some(Weekday::Sun));
        assert_eq!(weekday_of(7), None);
    }

    #[test]
    fn a_presence_animates_the_dialog() {
        use crate::design::motion::Presence;

        let clock = crate::design::motion::FrameClock::new();
        let mut presence = Presence::hidden();
        presence.show(clock.now(), crate::design::motion::Spec::ENTER);
        let value = presence.value(clock.now());
        assert!((0.0..=1.0).contains(&value));
    }
}
