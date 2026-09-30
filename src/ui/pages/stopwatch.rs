//! The Stopwatch page.
//!
//! The reading is the loudest thing on the page, centisecond-accurate, and the
//! lap list is a table beside it rather than below it — a stopwatch is read
//! while something else is happening, and the laps are what a person scans
//! after the fact.

use iced::widget::{column, container, Space};
use iced::{Background, Element, Length};

use crate::app::message::Message;
use crate::app::OClock;
use crate::design::palette::Palette;
use crate::design::tokens::{radius, space, stroke};
use crate::domain::fmt;
use crate::domain::stopwatch::{Lap, LapRank};
use crate::services::i18n::{Arg, Text};
use crate::ui::components::controls::{ButtonSpec, Emphasis, IconButtonSpec};
use crate::ui::components::dialog::EmptyKind;
use crate::ui::components::icons::Icon;
use crate::ui::components::surfaces::Card;
use crate::ui::typography::{self, Role};

/// The page.
pub fn view(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let now = app.elapsed();
    let elapsed = app.stopwatch.elapsed(now);
    let running = app.stopwatch.is_running();

    let mut column = column![].spacing(space::XXL).width(Length::Fill);

    column = column.push(reading(app, palette, elapsed, running));
    column = column.push(controls_row(app, palette, running));
    column = column.push(laps(app, palette, now));

    column.width(Length::Fill).into()
}

/// The elapsed time, and the current lap.
fn reading(
    app: &OClock,
    palette: Palette,
    elapsed: std::time::Duration,
    running: bool,
) -> Element<'static, Message> {
    let state = app.tr(if running {
        Text::StatusRunning
    } else {
        Text::StatusStopped
    });

    Card::hero(palette)
        .pad_xy(space::XXXL, space::XXL)
        .spacing(space::MD)
        .fill_width()
        .push(
            column![
                typography::Label::new(fmt::stopwatch(elapsed), Role::Display, palette.text)
                    .reading(app.catalog)
                    .view(),
                Space::with_height(space::XS),
                crate::ui::ends(
                    app.catalog.direction(),
                    typography::Label::new(state, Role::Label, palette.accent_text)
                        .reading(app.catalog)
                        .natural()
                        .view(),
                    crate::ui::ends(
                        app.catalog.direction(),
                        Space::with_width(space::LG),
                        typography::Label::new(
                            app.tr_with(
                                Text::LapNumber,
                                &[Arg::from(fmt::stopwatch(
                                    app.stopwatch.current_lap(app.elapsed()),
                                ))],
                            ),
                            Role::Label,
                            palette.text_muted,
                        )
                        .reading(app.catalog)
                        .natural()
                        .view(),
                    ),
                )
                .align_y(iced::Alignment::Center),
            ]
            .width(Length::Fill)
            .align_x(iced::Alignment::Center),
        )
        .view()
}

/// Start, lap and reset.
fn controls_row(app: &OClock, palette: Palette, running: bool) -> Element<'static, Message> {
    let start: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("stopwatch.play"),
        app.clock.clone(),
        palette,
        if running {
            Emphasis::Secondary
        } else {
            Emphasis::Primary
        },
        typography::label(
            app.tr(if running {
                Text::ActionPause
            } else {
                Text::ActionStart
            }),
            &palette,
        )
        .reading(app.catalog),
        Message::ToggleStopwatch,
    )
    .icon(if running { Icon::Pause } else { Icon::Play })
    .focused(app.view.focus == app.page_focus(0))
    .view();

    let lap: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("stopwatch.lap"),
        app.clock.clone(),
        palette,
        Emphasis::Secondary,
        typography::label(app.tr(Text::ActionLap), &palette).reading(app.catalog),
        Message::LapStopwatch,
    )
    .icon(Icon::Plus)
    .focused(app.view.focus == app.page_focus(1))
    .enabled(running)
    .view();

    let reset: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("stopwatch.reset"),
        app.clock.clone(),
        palette,
        Emphasis::Ghost,
        typography::label(app.tr(Text::ActionReset), &palette).reading(app.catalog),
        Message::ResetStopwatch,
    )
    .icon(Icon::Reset)
    .focused(app.view.focus == app.page_focus(2))
    .view();

    crate::ui::ordered(
        app.catalog.direction(),
        vec![
            start,
            Space::with_width(space::MD).into(),
            lap,
            Space::with_width(space::MD).into(),
            reset,
            Space::with_width(Length::Fill).into(),
        ],
    )
    .spacing(0.0)
    .align_y(iced::Alignment::Center)
    .into()
}

/// The lap history.
fn laps(app: &OClock, palette: Palette, _now: std::time::Duration) -> Element<'static, Message> {
    let recorded = app.stopwatch.laps();

    if recorded.is_empty() {
        return Card::flat(palette)
            .pad_xy(space::XXL, space::XXL)
            .spacing(space::MD)
            .fill_width()
            .push(
                typography::eyebrow(app.tr(Text::EyebrowLaps), &palette)
                    .reading(app.catalog)
                    .view(),
            )
            .push(dialog_empty(
                EmptyKind::Nothing,
                palette,
                app.tr(Text::EmptyLapsTitle),
                app.tr(Text::EmptyLapsDetail),
            ))
            .view();
    }

    // Newest first, the way a stopwatch reads: the mark just made is the one the
    // reader wants, and the top of the list is where they look for it. The index
    // in the message is the lap's own, so removing a row takes the lap that row
    // is showing; where it is drawn is a different number, and the focus ring
    // follows the drawing.
    let mut rows = Vec::with_capacity(recorded.len());
    for (position, (index, lap)) in recorded.iter().enumerate().rev().enumerate() {
        rows.push(lap_row(app, palette, lap, index, position));
    }

    let fastest = fastest_of(recorded);
    let slowest = slowest_of(recorded);

    let eyebrow: Element<'static, Message> =
        typography::eyebrow(app.tr(Text::EyebrowLaps), &palette)
            .reading(app.catalog)
            .view()
            .into();
    let summary: Element<'static, Message> = typography::Label::new(
        app.tr_with(
            Text::LapsSummary,
            &[
                Arg::from(
                    app.catalog
                        .plural_format(Text::CountLaps, recorded.len() as u32),
                ),
                Arg::from(fmt::stopwatch(fastest)),
                Arg::from(fmt::stopwatch(slowest)),
            ],
        ),
        Role::Caption,
        palette.text_faint,
    )
    .reading(app.catalog)
    .view()
    .into();

    // The name of the section leads; the summary of the figures is the status
    // that follows it.
    let header = crate::ui::ends(
        app.catalog.direction(),
        eyebrow,
        crate::ui::ends(
            app.catalog.direction(),
            Space::with_width(Length::Fill),
            summary,
        ),
    );

    Card::flat(palette)
        .pad_xy(space::XXL, space::XL)
        .spacing(space::MD)
        .fill_width()
        .push(header.align_y(iced::Alignment::Center))
        .push(column(rows).spacing(space::XXS).width(Length::Fill))
        .view()
}

/// One lap.
fn lap_row(
    app: &OClock,
    palette: Palette,
    lap: &Lap,
    index: usize,
    position: usize,
) -> Element<'static, Message> {
    let rank = app.stopwatch.rank_of(index);
    let (tint, note) = match rank {
        Some(LapRank::Fastest) => (palette.positive, app.tr(Text::RankFastest)),
        Some(LapRank::Slowest) => (palette.caution, app.tr(Text::RankSlowest)),
        None => (palette.text_faint, ""),
    };

    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(match rank {
            Some(LapRank::Fastest) => crate::design::palette::overlay(
                crate::design::motion::ease::fade(palette.positive, 0.10),
                palette.sunken,
            ),
            Some(LapRank::Slowest) => crate::design::palette::overlay(
                crate::design::motion::ease::fade(palette.caution, 0.10),
                palette.sunken,
            ),
            None => Color::TRANSPARENT,
        })),
        border: iced::Border {
            color: iced::Color::TRANSPARENT,
            width: 0.0,
            radius: radius::MD.into(),
        },
        ..iced::widget::container::Style::default()
    };

    container(
        crate::ui::ordered(
            app.catalog.direction(),
            vec![
                typography::Label::new(
                    format!("{:02}", lap.index),
                    Role::Label,
                    palette.text_muted,
                )
                .reading(app.catalog)
                .natural()
                .view()
                .into(),
                Space::with_width(space::LG).into(),
                typography::Label::new(fmt::stopwatch(lap.split), Role::Metric, palette.text)
                    .reading(app.catalog)
                    .natural()
                    .view()
                    .into(),
                Space::with_width(Length::Fill).into(),
                if note.is_empty() {
                    Space::with_width(0.0).into()
                } else {
                    crate::ui::components::controls::badge(note, tint)
                },
                Space::with_width(space::MD).into(),
                typography::Label::new(
                    app.tr_with(Text::LapTotal, &[Arg::from(fmt::stopwatch(lap.total))]),
                    Role::Caption,
                    palette.text_faint,
                )
                .reading(app.catalog)
                .natural()
                .view()
                .into(),
                Space::with_width(space::MD).into(),
                IconButtonSpec::new(
                    app.interactions.get("stopwatch.remove"),
                    app.clock.clone(),
                    palette,
                    Emphasis::Ghost,
                    Icon::Close,
                    Message::RemoveLap(index),
                )
                .focused(app.view.focus == app.page_focus(app.lap_row_base() + position))
                .view(),
            ],
        )
        .align_y(iced::Alignment::Center),
    )
    .style(style)
    .padding(iced::padding::all(space::SM))
    .width(Length::Fill)
    .into()
}

/// The shortest lap.
pub fn fastest_of(laps: &[Lap]) -> std::time::Duration {
    laps.iter().map(|lap| lap.split).min().unwrap_or_default()
}

/// The longest lap.
pub fn slowest_of(laps: &[Lap]) -> std::time::Duration {
    laps.iter().map(|lap| lap.split).max().unwrap_or_default()
}

/// The empty state a lap list shows before anything is recorded.
fn dialog_empty(
    kind: EmptyKind,
    palette: Palette,
    heading: &str,
    detail: &str,
) -> Element<'static, Message> {
    crate::ui::components::dialog::empty::<Message>(kind, palette, heading, detail, None)
}

/// A hairline between laps, exposed so the rhythm matches the rest.
pub fn row_rule(_palette: Palette) -> f32 {
    stroke::HAIRLINE
}

use iced::Color;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn lap(index: usize, seconds: u64) -> Lap {
        Lap {
            index,
            total: Duration::from_secs(seconds * 2),
            split: Duration::from_secs(seconds),
        }
    }

    #[test]
    fn the_page_renders_empty_and_running() {
        let app = crate::app::tests::sample();
        let _: Element<'static, Message> = view(&app, Palette::LIGHT);
    }

    #[test]
    fn the_fastest_and_slowest_laps_are_found() {
        let laps = [lap(1, 5), lap(2, 3), lap(3, 9)];
        assert_eq!(fastest_of(&laps), Duration::from_secs(3));
        assert_eq!(slowest_of(&laps), Duration::from_secs(9));
    }

    #[test]
    fn an_empty_lap_list_has_no_extremes() {
        assert_eq!(fastest_of(&[]), Duration::ZERO);
        assert_eq!(slowest_of(&[]), Duration::ZERO);
    }

    #[test]
    fn the_reading_fits_the_narrowest_window() {
        // The layout invariant this page depends on: its largest readout has to
        // fit the content column in the *narrowest* supported window, because
        // the readout cannot shrink and the card cannot scroll sideways.
        use crate::design::tokens::layout;
        use crate::ui::typography::Role;

        let window = layout::WINDOW_MIN.0 as f32;
        let rail = layout::SIDEBAR_COMPACT;
        let padding = space::XXXL * 2.0;
        let available = window - rail - padding;

        // The longest reading: hours, minutes, seconds and hundredths.
        let reading = fmt::stopwatch(Duration::from_secs(12 * 3600 + 34 * 60 + 56));
        let width = Role::MetricLarge.estimate_width(&reading);
        assert!(
            width < available,
            "the stopwatch readout needs {width:.0} px but only {available:.0} px are there"
        );
    }

    #[test]
    fn the_reading_is_centisecond_accurate() {
        let app = crate::app::tests::sample();
        let now = app.elapsed();
        let reading = fmt::stopwatch(now);
        assert!(
            reading.contains('.'),
            "a stopwatch shows hundredths: {reading}"
        );
        assert_eq!(reading.len(), 8, "mm:ss.cc is eight characters: {reading}");
    }

    #[test]
    fn helpers_are_neutral() {
        assert!(row_rule(Palette::LIGHT) > 0.0);
    }
}
