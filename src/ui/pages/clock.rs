//! The Clock page.
//!
//! The current time is the largest thing OClock draws anywhere, and everything
//! else on this page is arranged to support it: the seconds get their own
//! column so the hours and minutes never move, a continuous sweep gives the
//! display life between ticks, and the supporting information is grouped into
//! small readouts rather than scattered as loose text.

use chrono::Datelike;
use iced::widget::{column, container, Space};
use iced::{Background, Element, Length};

use crate::app::message::Message;
use crate::app::OClock;
use crate::design::palette::{Palette, ShadowLevel};
use crate::design::tokens::{space, stroke};
use crate::services::i18n::{Arg, Text};
use crate::ui::components::controls::{self, ChipSpec};
use crate::ui::components::indicators::{Dial, Sweep};
use crate::ui::components::surfaces::{self, Card, Fill};
use crate::ui::typography::{self, Baseline, Label, Role};

/// The page.
pub fn view(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let _clock = &app.snapshot.clock;
    let focused = app.view.focus;

    let mut column = column![].spacing(space::XXL).width(Length::Fill);

    column = column.push(hero(app, palette, focused));

    if app.display().show_analog {
        column = column.push(analog(app, palette));
    }

    column = column.push(details(app, palette, focused));
    column = column.push(preferences(app, palette));

    column.width(Length::Fill).into()
}

/// The leading edge of the page's own alignment: the right in a right-to-left
/// language, the left otherwise.
fn leading(app: &OClock) -> iced::Alignment {
    crate::ui::leading(app.catalog.direction())
}

/// The hero: the time itself, and the sweep that keeps it alive.
fn hero(app: &OClock, palette: Palette, focused: Option<usize>) -> Element<'static, Message> {
    let clock = &app.snapshot.clock;
    let now = app.clock.now();

    // The big digits, the seconds column and the meridiem, all on one
    // baseline so they read as a single time rather than three numbers.
    //
    // The order stays the one a reader of the digits expects — hours, then
    // seconds, then the meridiem — because a time is a number, and reversing
    // the parts of a number to match the paragraph would make it wrong rather
    // than right. The block as a whole hangs from the leading edge.
    let mut reading = Baseline::new(
        Label::new(&clock.text.digits, Role::Display, palette.text).reading(app.catalog),
    );
    if let Some(seconds) = clock.text.seconds.as_ref() {
        reading = reading.then(
            Label::new(
                app.catalog
                    .format(Text::ClockSeconds, &[Arg::from(seconds.as_str())]),
                Role::Metric,
                palette.accent_text,
            )
            .reading(app.catalog),
        );
    }
    if let Some(is_pm) = clock.text.meridiem {
        reading = reading.then(
            Label::new(
                app.catalog.meridiem(is_pm),
                Role::Caption,
                palette.text_faint,
            )
            .reading(app.catalog),
        );
    }

    let sweep = Sweep::new(palette, clock.second_fraction).view::<Message>();

    // The zone, its offset, and the daylight-saving note beside them. The note
    // leads, because it is a status the reader is looking for rather than part
    // of the name of the zone; the abbreviation and the offset are one unit and
    // stay in that order, so `UTC-04:00` is never read as `00-40:UTC`.
    let zone_line = crate::ui::sequence(
        app.catalog.direction(),
        if clock.daylight_saving {
            controls::badge(app.tr(Text::DaylightSaving), palette.caution)
        } else {
            Space::with_width(0.0).into()
        },
        Space::with_width(space::MD),
        crate::ui::ends(
            app.catalog.direction(),
            typography::Label::new(&clock.zone_abbreviation, Role::Label, palette.accent_text)
                .reading(app.catalog)
                .natural()
                .view(),
            crate::ui::ends(
                app.catalog.direction(),
                Space::with_width(space::SM),
                typography::Label::new(&clock.zone_offset, Role::Label, palette.text_muted)
                    .reading(app.catalog)
                    .natural()
                    .view(),
            ),
        ),
    )
    .align_y(iced::Alignment::Center);

    let body = Card::hero(palette)
        .pad_xy(space::XXXL, space::XXL)
        .spacing(space::LG)
        .fill_width()
        .push(
            container(reading.view::<Message>())
                .width(Length::Fill)
                .align_x(leading(app)),
        )
        .push(sweep)
        .push(
            column![
                typography::Label::new(
                    app.catalog.date_full(clock.at),
                    Role::Body,
                    palette.text_muted
                )
                .reading(app.catalog)
                .proportional()
                .natural()
                .view(),
                Space::with_height(space::MD),
                zone_line,
            ]
            .width(Length::Fill)
            .align_x(leading(app)),
        );

    let _ = (now, focused);
    body.view()
}

/// The analog dial, for people who read a face.
fn analog(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let clock = &app.snapshot.clock;
    let dial: Element<'static, Message> = Dial::new(palette, app.snapshot.now).size(220.0).view();

    let captions = Card::flat(palette)
        .pad_xy(space::XXL, space::XL)
        .spacing(space::XS)
        .fill_width()
        .push(
            typography::eyebrow(app.tr(Text::EyebrowAlso), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(
            crate::ui::ends(
                app.catalog.direction(),
                typography::Label::new(
                    app.catalog.weekday_long(clock.at.date_naive().weekday()),
                    Role::Heading,
                    palette.text,
                )
                .reading(app.catalog)
                .proportional()
                .natural()
                .view(),
                crate::ui::ends(
                    app.catalog.direction(),
                    Space::with_width(Length::Fill),
                    typography::Label::new(
                        app.tr_with(Text::WeekNumber, &[Arg::from(clock.week)]),
                        Role::Label,
                        palette.text_muted,
                    )
                    .reading(app.catalog)
                    .natural()
                    .view(),
                ),
            )
            .align_y(iced::Alignment::Center),
        )
        .push(
            crate::ui::ends(
                app.catalog.direction(),
                typography::Label::new(
                    app.tr_with(Text::DayOfYear, &[Arg::from(clock.day_of_year)]),
                    Role::Caption,
                    palette.text_faint,
                )
                .reading(app.catalog)
                .natural()
                .view(),
                crate::ui::ends(
                    app.catalog.direction(),
                    Space::with_width(Length::Fill),
                    typography::Label::new(&clock.zone_offset, Role::Caption, palette.text_faint)
                        .reading(app.catalog)
                        .natural()
                        .view(),
                ),
            )
            .align_y(iced::Alignment::Center),
        )
        .view();

    // The dial is what a person came to this card for, so it leads and takes the
    // leading edge of the pair.
    crate::ui::ends(app.catalog.direction(), dial, captions)
        .spacing(space::XXL)
        .align_y(iced::Alignment::Center)
        .into()
}

/// The readouts: what the hero deliberately does not show.
fn details(app: &OClock, palette: Palette, focused: Option<usize>) -> Element<'static, Message> {
    let clock = &app.snapshot.clock;
    let narrow = matches!(app.view.layout, crate::app::view::Layout::Stacked);

    // The readouts divide the reading up: the hero shows all of it, and these
    // are the parts a person might actually be looking for.
    //
    // They are laid out in reading order. Iced builds a row left to right, so a
    // right-to-left page hands the list over the other way round. A widget is a
    // value that cannot be laid out twice, so each branch builds its own.
    let hour = clock
        .text
        .digits
        .split(':')
        .next()
        .unwrap_or("")
        .to_string();
    let seconds_text = clock
        .text
        .seconds
        .clone()
        .unwrap_or_else(|| "––".to_string());
    let date_text = app.catalog.date_short(clock.at);

    let readouts = || {
        [
            controls::readout::<Message>(palette, app.tr(Text::CaptionHour), &hour),
            controls::readout::<Message>(palette, app.tr(Text::CaptionSecond), &seconds_text),
            controls::readout::<Message>(
                palette,
                app.tr(Text::CaptionZone),
                &clock.zone_abbreviation,
            ),
            controls::readout_text::<Message>(palette, app.tr(Text::CaptionDate), &date_text),
        ]
    };

    let cards: Element<'static, Message> = if narrow {
        let [hours, seconds, zone, date] = readouts();
        column![
            crate::ui::ordered(
                app.catalog.direction(),
                vec![hours, Space::with_width(space::MD).into(), seconds,]
            ),
            crate::ui::ordered(
                app.catalog.direction(),
                vec![zone, Space::with_width(space::MD).into(), date,]
            ),
        ]
        .spacing(space::MD)
        .align_x(leading(app))
        .into()
    } else {
        let [hours, seconds, zone, date] = readouts();
        let mut items: Vec<Element<'static, Message>> = Vec::with_capacity(7);
        for (index, readout) in [hours, seconds, zone, date].into_iter().enumerate() {
            if index > 0 {
                items.push(Space::with_width(space::MD).into());
            }
            items.push(readout);
        }
        crate::ui::ordered(app.catalog.direction(), items).into()
    };

    let _ = focused;
    column![
        typography::eyebrow(app.tr(Text::EyebrowDetails), &palette)
            .reading(app.catalog)
            .view(),
        Space::with_height(space::MD),
        cards
    ]
    .width(Length::Fill)
    .into()
}

/// The clock's own preferences.
fn preferences(app: &OClock, palette: Palette) -> Element<'static, Message> {
    // The chips are numbered by the same list the key activation reads, so a
    // ring can only ever land on a control that can then be pressed. The dial
    // chip is the same control as the analog button in the header, whichever
    // way round a narrow window has to choose, and both take one index.
    let focused = |index: usize| app.view.focus == app.page_focus(index);
    let clock = &app.snapshot.clock;

    let format: Element<'static, Message> = ChipSpec::new(
        app.interactions.get("clock.format"),
        app.clock.clone(),
        palette,
        typography::Label::new(
            clock
                .text
                .meridiem
                .as_ref()
                .map(|_| app.catalog.hour_format(app.display().hour_format))
                .unwrap_or(
                    app.catalog
                        .hour_format(crate::domain::clock::HourFormat::TwentyFour),
                ),
            Role::Label,
            palette.text_muted,
        )
        .reading(app.catalog),
        true,
        Message::ToggleHourFormat,
    )
    .focused(focused(0))
    .view();

    let seconds: Element<'static, Message> = ChipSpec::new(
        app.interactions.get("clock.seconds"),
        app.clock.clone(),
        palette,
        typography::Label::new(
            app.tr(Text::CaptionSeconds),
            Role::Label,
            palette.text_muted,
        )
        .reading(app.catalog),
        app.display().show_seconds,
        Message::ToggleSeconds,
    )
    .focused(focused(1))
    .view();

    let dial: Element<'static, Message> = ChipSpec::new(
        app.interactions.get("clock.analog"),
        app.clock.clone(),
        palette,
        typography::Label::new(app.tr(Text::CaptionDial), Role::Label, palette.text_muted)
            .reading(app.catalog),
        app.display().show_analog,
        Message::ToggleAnalog,
    )
    .focused(focused(2))
    .view();

    Card::flat(palette)
        .pad_xy(space::XXL, space::XL)
        .spacing(space::MD)
        .fill_width()
        .push(
            typography::eyebrow(app.tr(Text::EyebrowDisplay), &palette)
                .reading(app.catalog)
                .view(),
        )
        .push(
            container(
                crate::ui::ordered(
                    app.catalog.direction(),
                    vec![
                        format,
                        Space::with_width(space::SM).into(),
                        seconds,
                        Space::with_width(space::SM).into(),
                        dial,
                    ],
                )
                .spacing(space::SM)
                .align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .align_x(leading(app)),
        )
        .view()
}

/// A compact readout of the current time, for the world-clock cards.
pub fn compact(app: &OClock, _palette: Palette) -> String {
    app.catalog.reading(&app.snapshot.clock.text)
}

/// A card the page can reuse, exposed for the world page's rhythm.
pub fn section<M: 'static>(
    palette: Palette,
    heading: &str,
    body: impl Into<Element<'static, M>>,
) -> Element<'static, M> {
    Card::flat(palette)
        .pad_xy(space::XXL, space::XL)
        .spacing(space::MD)
        .fill_width()
        .push(typography::eyebrow(heading, &palette).view())
        .push(body)
        .view()
}

/// A hairline under a card, exposed for consistency with the shell.
pub fn card_rule(palette: Palette) -> Element<'static, ()> {
    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(palette.outline)),
        ..iced::widget::container::Style::default()
    };

    container(Space::with_height(1.0))
        .style(style)
        .width(Length::Fill)
        .into()
}

/// The elevation a page's cards sit at.
pub fn elevation(palette: Palette) -> iced::Shadow {
    palette.shadow(ShadowLevel::Low)
}

/// A border width, exposed so pages and the shell agree.
pub const HAIRLINE: f32 = stroke::HAIRLINE;

/// A surface helper for the world page.
pub fn surface<M: 'static>(palette: Palette) -> surfaces::Card<M> {
    Card::new(palette, Fill::Surface)
}
