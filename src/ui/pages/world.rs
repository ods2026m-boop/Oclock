//! The World Clock page.
//!
//! Every city is a card with an analog face, because a world clock is about
//! *places*, and a face reads as a place in a way a line of digits does not.
//! The city's own dial, its offset and its name stay in one object, and the
//! cards are laid out in a wrapping row so a wider window simply adds a column.

use iced::widget::{column, container, row, Space};
use iced::{Background, Element, Length};

use crate::app::focus::dialog as dialog_focus;
use crate::app::message::Message;
use crate::app::OClock;
use crate::design::palette::Palette;
use crate::design::tokens::space;
use crate::domain::world::WorldRow;
use crate::services::i18n::{Arg, Text};
use crate::ui::components::controls::{self, ChipSpec, IconButtonSpec};
use crate::ui::components::dialog::{self, EmptyKind};
use crate::ui::components::icons::Icon;
use crate::ui::components::indicators::Dial;
use crate::ui::components::inputs::FieldSpec;
use crate::ui::components::surfaces::{self, Card, Fill};
use crate::ui::typography::{self, Role};

fn leading(app: &OClock) -> iced::Alignment {
    crate::ui::leading(app.catalog.direction())
}

/// The width of a city card.
///
/// Sized from the widest reading the catalogue can produce rather than picked:
/// a card whose time has to wrap puts the meridiem on a line of its own, and the
/// widest meridiem in the catalogue is not in the language the card was designed
/// in — it is Thai, at nearly half as wide again as the English "PM" that a
/// fixed width would be measured against. The test
/// `the_card_is_wide_enough_for_every_reading` keeps the two in step, and
/// `the_card_is_no_wider_than_it_needs` says how little slack there is.
pub const CARD_WIDTH: f32 = 372.0;

/// The sort order and the number of cities, in reading order: the count is a
/// status the reader is looking for, and it leads the pair.
fn summary_row(app: &OClock, palette: Palette, count: usize) -> Element<'static, Message> {
    crate::ui::ends(
        app.catalog.direction(),
        crate::ui::ends(
            app.catalog.direction(),
            controls::badge(
                app.catalog.plural_format(Text::CountCities, count as u32),
                palette.text_muted,
            ),
            Space::with_width(space::MD),
        ),
        typography::Label::new(
            app.catalog.sort_mode(app.world.sort()),
            Role::Caption,
            palette.text_faint,
        )
        .reading(app.catalog)
        .natural()
        .view(),
    )
    .align_y(iced::Alignment::Center)
    .into()
}

/// The page.
pub fn view(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let rows = &app.snapshot.world;

    if rows.is_empty() {
        return dialog::empty::<Message>(
            EmptyKind::Nothing,
            palette,
            app.tr(Text::EmptyWorldTitle),
            app.tr(Text::EmptyWorldDetail),
            None,
        );
    }

    let mut cards = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        // The card's own indices, from the same list the key activation reads:
        // a card whose zone does not resolve draws one button rather than two,
        // and its neighbours have to move in rather than be numbered past it.
        cards.push(city_card(app, palette, row, app.world_card_base(index)));
    }

    // The grid is dealt into rows by a wrapped row, which packs from the left
    // and has no mirror of its own, so the list is left in the order it is
    // configured. Reversing it would put the *last* city in the first row
    // whenever the cards wrap, which is a worse reading order than the one it
    // replaces; the cards themselves are laid out for their language, and it is
    // the row of cards that fills from the left.

    let summary = container(summary_row(app, palette, rows.len()))
        .width(Length::Fill)
        .align_x(leading(app));

    column![
        summary,
        Space::with_height(space::MD),
        surfaces::flow(cards, space::LG),
    ]
    .width(Length::Fill)
    .into()
}

/// One city.
fn city_card(
    app: &OClock,
    palette: Palette,
    row: &WorldRow,
    base: usize,
) -> Element<'static, Message> {
    let Some(at) = row.at else {
        // A zone this build cannot resolve: still shown, with the problem
        // stated rather than a blank card.
        return Card::new(palette, Fill::Danger)
            .pad_xy(space::XL, space::LG)
            .spacing(space::SM)
            .push(
                typography::Label::new(&row.location.city, Role::Heading, palette.text)
                    .proportional()
                    .view(),
            )
            .push(
                typography::Label::new(
                    app.tr_with(Text::WorldUnknownZone, &[Arg::from(&row.location.zone)]),
                    Role::Caption,
                    palette.danger,
                )
                .proportional()
                .view(),
            )
            .push(
                IconButtonSpec::new(
                    app.interactions.get("world.remove"),
                    app.clock.clone(),
                    palette,
                    controls::Emphasis::Danger,
                    Icon::Trash,
                    Message::RemoveCity(row.location.id),
                )
                .focused(app.view.focus == app.page_focus(base))
                .view(),
            )
            .width(Length::Fixed(280.0))
            .into();
    };

    let dial: Element<'static, Message> = Dial::frozen(palette, at, 0.0).size(96.0).bare().view();

    let difference = match row.offset_from_local {
        0 => app.tr(Text::WorldSameTime).to_string(),
        minutes if minutes > 0 => app.tr_with(
            Text::WorldAhead,
            &[Arg::from(app.catalog.span_minutes(minutes))],
        ),
        minutes => app.tr_with(
            Text::WorldBehind,
            &[Arg::from(app.catalog.span_minutes(minutes))],
        ),
    };

    let actions = row![
        IconButtonSpec::new(
            app.interactions.get("world.pin"),
            app.clock.clone(),
            palette,
            if row.location.pinned {
                controls::Emphasis::Primary
            } else {
                controls::Emphasis::Ghost
            },
            Icon::Pin,
            Message::TogglePin(row.location.id),
        )
        .focused(app.view.focus == app.page_focus(base))
        .view(),
        IconButtonSpec::new(
            app.interactions.get("world.remove"),
            app.clock.clone(),
            palette,
            controls::Emphasis::Ghost,
            Icon::Trash,
            Message::RemoveCity(row.location.id),
        )
        .focused(app.view.focus == app.page_focus(base + 1))
        .view(),
    ]
    .spacing(space::XXS);

    // The city and its zone, which lead the card, and the local date and day
    // difference, which follow them. Each is measured so the two columns sit
    // side by side rather than on top of one another.
    let details: Element<'static, Message> = column![
        typography::Label::new(&row.location.city, Role::Heading, palette.text)
            .reading(app.catalog)
            .proportional()
            .natural()
            .view(),
        Space::with_height(space::XXS),
        typography::Label::new(&row.location.zone, Role::Caption, palette.text_faint)
            .reading(app.catalog)
            .proportional()
            .natural()
            .view(),
    ]
    .spacing(0.0)
    .width(Length::Fill)
    .into();

    let now: Element<'static, Message> = column![
        typography::Label::new(
            format!(
                "{} · {}",
                app.catalog.date_short(at),
                app.catalog.day_relation(row.day_relation)
            ),
            Role::Caption,
            palette.text_muted,
        )
        .reading(app.catalog)
        .natural()
        .view(),
        typography::Label::new(difference, Role::Micro, palette.text_faint)
            .reading(app.catalog)
            .proportional()
            .natural()
            .view(),
    ]
    .spacing(0.0)
    .width(Length::Fill)
    .into();

    Card::new(
        palette,
        if row.location.pinned {
            Fill::Accent
        } else {
            Fill::Surface
        },
    )
    .pad_xy(space::XL, space::LG)
    .spacing(space::SM)
    .push(
        container(
            crate::ui::ends(app.catalog.direction(), dial, details)
                .spacing(space::MD)
                .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(leading(app)),
    )
    // The reading gets a line of its own. Squeezed beside a long time and a
    // timezone name, three short pieces of text share one row and each is
    // given so little room that it wraps to one character per line.
    .push(
        container(
            typography::Label::new(
                app.catalog
                    .reading(&crate::domain::clock::format_time(&at, app.display())),
                Role::Metric,
                palette.text,
            )
            .reading(app.catalog)
            .natural()
            .view(),
        )
        .width(Length::Fill)
        .align_x(leading(app)),
    )
    .push(
        container(
            crate::ui::sequence(
                app.catalog.direction(),
                if row.daylight_saving {
                    controls::badge(app.tr(Text::DaylightSaving), palette.caution)
                } else {
                    Space::with_width(0.0).into()
                },
                Space::with_width(space::SM),
                crate::ui::ends(
                    app.catalog.direction(),
                    typography::Label::new(&row.abbreviation, Role::Caption, palette.accent_text)
                        .reading(app.catalog)
                        .natural()
                        .view(),
                    crate::ui::ends(
                        app.catalog.direction(),
                        Space::with_width(space::XS),
                        typography::Label::new(&row.offset, Role::Caption, palette.text_muted)
                            .reading(app.catalog)
                            .natural()
                            .view(),
                    ),
                ),
            )
            .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(leading(app)),
    )
    .push(
        container(
            crate::ui::ends(app.catalog.direction(), now, actions)
                .spacing(space::SM)
                .align_y(iced::Alignment::Center),
        )
        .width(Length::Fill)
        .align_x(leading(app)),
    )
    .width(Length::Fixed(CARD_WIDTH))
    .into()
}

/// The city picker.
pub fn picker(app: &OClock, palette: Palette, presence: f32) -> Element<'static, Message> {
    let query = app.view.query.as_str();
    let results = crate::domain::world::search(query, crate::services::tzdata::index());
    let shown: Vec<&crate::domain::world::City> = results.into_iter().take(40).collect();

    let search: Element<'static, Message> = FieldSpec::new(
        app.interactions.get("world.search"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionSearch),
        query.to_string(),
        Message::SearchCities,
    )
    .placeholder(app.tr(Text::PlaceholderCityCountryZone))
    .hint(app.tr(Text::HintEditing))
    .reading(app.catalog)
    .view();

    let mut rows: Vec<Element<'static, Message>> = Vec::new();
    if shown.is_empty() {
        rows.push(
            typography::Label::new(
                app.tr_with(Text::WorldNoMatch, &[Arg::from(query)]),
                Role::Body,
                palette.text_muted,
            )
            .reading(app.catalog)
            .proportional()
            .view()
            .into(),
        );
    }

    for city in shown.iter() {
        let already = app
            .world
            .locations()
            .iter()
            .any(|location| location.zone == city.zone && location.city == city.name);

        let add: Element<'static, Message> = controls::ButtonSpec::new(
            app.interactions.get("world.result"),
            app.clock.clone(),
            palette,
            controls::Emphasis::Secondary,
            typography::Label::new(
                app.tr(if already {
                    Text::ActionAdded
                } else {
                    Text::ActionAdd
                }),
                Role::Label,
                palette.accent_text,
            )
            .reading(app.catalog),
            Message::AddCity(city.zone.clone()),
        )
        .icon(if already { Icon::Check } else { Icon::Plus })
        .enabled(!already)
        .view();

        rows.push(
            crate::ui::ends(
                app.catalog.direction(),
                column![
                    typography::Label::new(&city.name, Role::Label, palette.text)
                        .reading(app.catalog)
                        .proportional()
                        .view(),
                    typography::Label::new(city.subtitle(), Role::Micro, palette.text_faint)
                        .reading(app.catalog)
                        .proportional()
                        .view(),
                ]
                .spacing(0.0),
                crate::ui::ends(
                    app.catalog.direction(),
                    Space::with_width(Length::Fill),
                    add,
                ),
            )
            .spacing(space::MD)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
            .into(),
        );
    }

    let list = column(rows).spacing(space::XXS);

    let close: Element<'static, Message> = dialog::sole_action(
        app.clock.clone(),
        app.interactions.get("world.picker.close"),
        palette,
        app.tr(Text::ActionDone),
        Message::ToggleCityPicker,
        app.view.focus == Some(dialog_focus::PICKER_CLOSE),
    );

    dialog::Dialog::<Message>::new(palette, presence)
        .reading(app.catalog)
        .limit(crate::ui::pages::dialog_limit(app))
        .width(crate::ui::pages::dialog_width(app, 520.0))
        .push(typography::heading(app.tr(Text::AddCityTitle), &palette).view())
        .push(search)
        .push(list)
        .footer(close)
        .view()
}

/// A card for a city that could not be resolved, exposed for the tests.
pub fn broken_card(palette: Palette, city: &str, zone: &str) -> Element<'static, Message> {
    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(crate::design::palette::overlay(
            crate::design::motion::ease::fade(palette.danger, 0.12),
            palette.surface,
        ))),
        border: iced::Border {
            color: crate::design::motion::ease::fade(palette.danger, 0.35),
            width: 1.0,
            radius: 18.0.into(),
        },
        ..iced::widget::container::Style::default()
    };

    container(
        column![
            typography::Label::new(city, Role::Heading, palette.text)
                .proportional()
                .view(),
            typography::Label::new(zone, Role::Caption, palette.danger)
                .proportional()
                .view(),
        ]
        .spacing(space::XXS),
    )
    .style(style)
    .padding(iced::padding::all(space::XL))
    .width(Length::Fixed(280.0))
    .into()
}

/// The search field, exposed so the header can offer one.
pub fn search_field(app: &OClock, palette: Palette) -> Element<'static, Message> {
    FieldSpec::new(
        app.interactions.get("world.search.field"),
        app.clock.clone(),
        palette,
        app.tr(Text::CaptionSearchCities),
        app.view.query.clone(),
        Message::SearchCities,
    )
    .placeholder(app.tr(Text::PlaceholderCityZone))
    .hint(app.tr(Text::HintEditing))
    .reading(app.catalog)
    .view()
}

/// A chip that filters the list, exposed for the header.
pub fn sort_chip(app: &OClock, palette: Palette) -> Element<'static, Message> {
    ChipSpec::new(
        app.interactions.get("world.sort.chip"),
        app.clock.clone(),
        palette,
        typography::Label::new(
            app.catalog.sort_mode(app.world.sort()),
            Role::Label,
            palette.text_muted,
        )
        .reading(app.catalog),
        true,
        Message::CycleSort,
    )
    .icon(Icon::Sort)
    .view()
}

/// A marker, kept so the page can be built without the shell's brand.
pub fn mark(icon: Icon, size: f32, colour: iced::Color) -> Element<'static, ()> {
    controls::mark(icon, size, colour)
}

/// The page's rhythm, matching the other pages.
pub fn gap() -> f32 {
    space::XXL
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::tokens::{fonts, type_scale};
    use crate::domain::clock::ClockText;
    use crate::services::i18n::{Catalog, Language};

    /// The width every reading in the catalogue has to fit inside.
    fn reading_width(catalog: Catalog, meridiem: bool) -> f32 {
        let reading = catalog.reading(&ClockText {
            digits: "05:52".to_string(),
            seconds: Some("39".to_string()),
            meridiem: Some(meridiem),
        });
        crate::ui::text_metrics::natural_width(
            &reading,
            fonts::NUMERIC,
            type_scale::METRIC,
            type_scale::METRIC * 1.2,
        )
    }

    #[test]
    fn the_card_is_wide_enough_for_every_reading() {
        // The card holds a time, and the catalogue spells the meridiem out in
        // some languages and abbreviates it in others — a two-letter "PM" in
        // Arabic against a nine-character word in Thai. Sizing the card for the
        // reading rather than for one language is the only way all of them fit
        // on one line; a card too narrow wraps the meridiem away from the time
        // it belongs to.
        let inner = CARD_WIDTH - space::XXL * 2.0;
        for language in Language::ALL {
            for meridiem in [false, true] {
                let width = reading_width(Catalog::new(language), meridiem);
                assert!(
                    width <= inner,
                    "{language:?} reads {width:.0}px wide, and the card offers {inner:.0}px"
                );
            }
        }
    }

    #[test]
    fn the_card_is_no_wider_than_it_needs() {
        // A card measured from its text would be a different width in every
        // language, and the grid would be a different shape in each. The
        // constant is a compromise; this says how much of it is slack.
        let inner = CARD_WIDTH - space::XXL * 2.0;
        let widest = Language::ALL
            .iter()
            .flat_map(|language| [false, true].map(|m| reading_width(Catalog::new(*language), m)))
            .fold(0.0f32, f32::max);
        assert!(
            inner - widest < space::XXL,
            "{}px of slack is more than a gap",
            inner - widest
        );
    }
}
