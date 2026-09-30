//! The navigation rail and its sliding indicator.
//!
//! The indicator is the one piece of motion in the shell that runs
//! continuously: it travels from one item to the next rather than blinking, so
//! the movement tells the user these are five views of one application rather
//! than five unrelated screens.
//!
//! Each item draws its own slice of the indicator, weighted by how close the
//! marker is to it. That gives a true slide with no dependence on the layout
//! having told anyone where the items are — the indicator cannot get out of
//! step with the list, because there is only one number and the list is derived
//! from it.

use iced::widget::button::{self, Button, Status};
use iced::widget::canvas::{self, Fill, Frame, Geometry, Path};
use iced::widget::{row, Canvas, Container};
use iced::{Background, Border, Color, Element, Length};

use crate::design::motion::{ease, FrameClock, Spec, Tween};
use crate::design::palette::Palette;
use crate::design::tokens::{radius, space, stroke, FOCUS_RING};
use crate::services::i18n::{Catalog, Text};
use crate::ui::components::icons::{Icon, IconView};
use crate::ui::components::interaction::{Interaction, Interactions};
use crate::ui::typography::{Label, Role};

/// The width of the indicator bar.
const MARKER_WIDTH: f32 = 3.0;

/// One destination in the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Destination {
    /// A stable name, used for the interaction key and for diagnostics.
    pub name: &'static str,
    /// The mark shown beside the label.
    pub icon: Icon,
    /// The label, shown when there is room for it.
    ///
    /// A catalogue key rather than a string: the rail is built from a list
    /// that has no access to the application's language, so the name of the
    /// thing is stored and the word for it is looked up where the rail is
    /// drawn.
    pub label: Text,
    /// A line of explanation, shown in the expanded rail.
    pub summary: Text,
}

impl Destination {
    /// The interaction key for this destination's button.
    pub fn key(&self) -> &'static str {
        self.name
    }

    /// The label in the language being read.
    pub fn label_text(self, catalog: Catalog) -> &'static str {
        catalog.text(self.label)
    }

    /// The summary line in the language being read.
    pub fn summary_text(self, catalog: Catalog) -> &'static str {
        catalog.text(self.summary)
    }

    /// The keyboard shortcut digit for this destination.
    pub fn shortcut(&self) -> u8 {
        match self.name {
            "clock" => 1,
            "world" => 2,
            "alarms" => 3,
            "timer" => 4,
            "stopwatch" => 5,
            _ => 0,
        }
    }
}

/// A group of destinations and the marker's position among them.
#[derive(Clone, Debug)]
pub struct Navigation {
    entries: Vec<Destination>,
    selected: usize,
    /// 0..1 across the current slide.
    progress: Tween,
    /// Where the slide started.
    from: f32,
    /// Where it is heading.
    to: f32,
}

impl Navigation {
    /// A navigation group over `entries`, starting on `selected`.
    pub fn new(entries: Vec<Destination>, selected: usize) -> Navigation {
        let at = selected.min(entries.len().saturating_sub(1)) as f32;
        Navigation {
            entries,
            selected,
            progress: Tween::at(1.0),
            from: at,
            to: at,
        }
    }

    /// The selected index.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The selected destination.
    pub fn destination(&self) -> Option<&Destination> {
        self.entries.get(self.selected)
    }

    /// Selects `index`, starting the indicator's slide.
    pub fn select(&mut self, index: usize, clock: &FrameClock) {
        if index >= self.entries.len() || index == self.selected {
            return;
        }
        let now = clock.now();
        // Start the new slide from wherever the marker actually is, so a rapid
        // second click continues rather than jumping.
        self.from = self.position(now);
        self.to = index as f32;
        self.progress.jump(0.0);
        self.progress.animate(1.0, now, Spec::MOVE);
        self.selected = index;
        clock.request();
    }

    /// True while the indicator is still sliding.
    pub fn is_moving(&self, clock: &FrameClock) -> bool {
        !self.progress.settled(clock.now())
    }

    /// The indicator's position, `0.0..entries.len()`.
    pub fn position(&self, now: std::time::Duration) -> f32 {
        ease::lerp(self.from, self.to, self.progress.value(now))
    }

    /// How strongly item `index` is marked, `0.0..=1.0`.
    ///
    /// Smoothly zero everywhere except at the marker, so the bar appears to
    /// travel. Above one entry's width the two neighbouring bars share it, which
    /// reads as the marker being between them rather than on them.
    pub fn weight_of(&self, index: usize, now: std::time::Duration) -> f32 {
        let distance = (self.position(now) - index as f32).abs();
        (1.0 - distance).clamp(0.0, 1.0)
    }

    /// The navigation items.
    pub fn entries(&self) -> &[Destination] {
        &self.entries
    }

    /// The rendered items, in order.
    ///
    /// `stacked` says whether these run down a rail or along a bar. It is not
    /// decoration: a rail's items are one above another and each fills the rail's
    /// width, so the item's own width is the width of the column. Along a bar
    /// they sit beside one another, and an item that fills whatever width it is
    /// offered takes the whole bar on the first one and leaves the rest of them
    /// with nothing at all.
    #[allow(clippy::too_many_arguments)]
    pub fn view<M: 'static + Clone>(
        &self,
        palette: Palette,
        catalog: Catalog,
        clock: &FrameClock,
        interactions: &Interactions,
        expanded: bool,
        stacked: bool,
        focus: Option<usize>,
        select: fn(usize) -> M,
    ) -> Vec<Element<'static, M>> {
        let now = clock.now();
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                item(
                    palette,
                    catalog,
                    clock.clone(),
                    interactions.get(entry.key()),
                    entry,
                    index == self.selected,
                    focus == Some(index),
                    expanded,
                    stacked,
                    self.weight_of(index, now),
                    select(index),
                )
            })
            .collect()
    }
}

/// A single navigation item.
#[allow(clippy::too_many_arguments)]
fn item<M: 'static + Clone>(
    palette: Palette,
    catalog: Catalog,
    clock: FrameClock,
    interaction: Interaction,
    entry: &Destination,
    selected: bool,
    focused: bool,
    expanded: bool,
    stacked: bool,
    weight: f32,
    message: M,
) -> Element<'static, M> {
    let state = interaction.state;
    let hover = interaction.hover;
    let style_clock = clock.clone();
    let hover_clock = clock;

    let style = move |_theme: &iced::Theme, status: Status| {
        let selected_value = state.drive(selected as u8 as f32, &style_clock, Spec::TINT);
        let hover_value = hover.track(
            matches!(status, Status::Hovered | Status::Pressed),
            &hover_clock,
            Spec::HOVER,
        );

        // The wash deepens with the selection; hovering an unselected item
        // gives it its own, quieter, response.
        let resting = ease::color(Color::TRANSPARENT, palette.accent_soft, selected_value);
        let fill = ease::color(resting, palette.hover, hover_value * (1.0 - selected_value));
        let ink = ease::color(palette.text_muted, palette.accent_text, selected_value);

        button::Style {
            background: Some(Background::Color(fill)),
            text_color: ink,
            border: Border {
                color: if focused {
                    palette.accent_text
                } else {
                    Color::TRANSPARENT
                },
                width: if focused { FOCUS_RING } else { 0.0 },
                radius: radius::LG.into(),
            },
            ..button::Style::default()
        }
    };

    // Padding lives on the content, because the button style has none of its
    // own to give.
    //
    // The mark belongs on the *leading* side of its label, so in a
    // right-to-left interface the row is built in the opposite order rather
    // than merely pushed to the other edge: the icon sits beside the start of
    // the word, which is where the eye looks for it.
    let mut text = row![].spacing(space::MD).align_y(iced::Alignment::Center);
    let mark = IconView::new(entry.icon, 20.0, palette.text_muted)
        .in_language(catalog)
        .view::<M>();
    if expanded {
        // Both runs are measured, so the caption hangs from the leading edge of
        // its own words and the shortcut sits beside it rather than at the far
        // end of the rail.
        let caption = Label::new(entry.label_text(catalog), Role::Label, palette.text_muted)
            .reading(catalog)
            .natural()
            .view();
        let digit = Label::new(
            entry.shortcut().to_string(),
            Role::Micro,
            palette.text_faint,
        )
        .reading(catalog)
        .natural()
        .view();
        text = if catalog.is_rtl() {
            row![]
                .spacing(space::MD)
                .align_y(iced::Alignment::Center)
                .push(digit)
                .push(iced::widget::horizontal_space())
                .push(caption)
                .push(mark)
        } else {
            text.push(mark)
                .push(caption)
                .push(iced::widget::horizontal_space())
                .push(digit)
        };
    } else {
        text = text.push(mark);
    }

    let button: Element<'static, M> = Button::new(
        text.padding(
            iced::padding::top(space::SM + 2.0)
                .bottom(space::SM + 2.0)
                .left(if expanded { space::MD } else { space::SM })
                .right(if expanded { space::MD } else { space::SM }),
        ),
    )
    .width(if stacked {
        Length::Fill
    } else {
        Length::Shrink
    })
    .style(style)
    .on_press(message)
    .into();

    // Each item draws the share of the indicator that belongs to it.
    let marker: Element<'static, M> = if weight <= 0.001 {
        Container::new(iced::widget::Space::new(MARKER_WIDTH, 1.0))
            .width(Length::Fixed(MARKER_WIDTH))
            .into()
    } else {
        let canvas: Element<'static, ()> = Canvas::new(Marker { palette, weight })
            .width(MARKER_WIDTH)
            .height(1.0)
            .into();
        canvas.map(|()| unreachable!("a marker never sends a message"))
    };

    // The indicator bar is drawn on the leading edge, so it moves to the other
    // side in a right-to-left interface along with everything it marks.
    let composed = if catalog.is_rtl() {
        row![button, marker]
    } else {
        row![marker, button]
    };
    composed
        .spacing(0.0)
        .align_y(iced::Alignment::Center)
        // A row of filling items is a column's worth of width, one after the
        // other. Beside one another in a bar, the first item would take the
        // whole bar and leave the rest of them with no room at all — which is
        // what the top bar showed: one visible destination, and that one
        // squeezed.
        .width(if stacked {
            Length::Fill
        } else {
            Length::Shrink
        })
        .into()
}

/// The slice of the indicator an item is responsible for.
struct Marker {
    palette: Palette,
    weight: f32,
}

impl canvas::Program<()> for Marker {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::widget::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        // The bar is drawn in the frame's own space; the canvas translates it
        // to the widget's position, so the origin here is the widget's
        // top-left corner.
        let mut frame = Frame::new(renderer, bounds.size());
        let corner = bounds.width / 2.0;
        frame.fill(
            &Path::new(|path| {
                path.rounded_rectangle(
                    iced::Point::new(0.0, 0.0),
                    iced::Size::new(bounds.width, bounds.height),
                    corner.into(),
                );
            }),
            Fill::from(ease::fade(self.palette.accent, self.weight)),
        );
        vec![frame.into_geometry()]
    }
}

/// The positions the indicator visits, `0.0..count`.
///
/// Separated from the drawing so the geometry of the slide can be checked
/// without a window.
pub fn marker_track(count: usize) -> Vec<f32> {
    (0..count).map(|index| index as f32).collect()
}

/// The hairline that separates the rail from the page.
pub fn rail_border(palette: Palette) -> Border {
    Border {
        color: palette.outline,
        width: stroke::HAIRLINE,
        radius: 0.0.into(),
    }
}

/// The shell's destinations, in the order they appear.
pub fn destinations() -> Vec<Destination> {
    vec![
        Destination {
            name: "clock",
            icon: Icon::Clock,
            label: Text::PageClock,
            summary: Text::PageClockSummary,
        },
        Destination {
            name: "world",
            icon: Icon::Globe,
            label: Text::PageWorld,
            summary: Text::PageWorldSummary,
        },
        Destination {
            name: "alarms",
            icon: Icon::Alarm,
            label: Text::PageAlarms,
            summary: Text::PageAlarmsSummary,
        },
        Destination {
            name: "timer",
            icon: Icon::Timer,
            label: Text::PageTimer,
            summary: Text::PageTimerSummary,
        },
        Destination {
            name: "stopwatch",
            icon: Icon::Stopwatch,
            label: Text::PageStopwatch,
            summary: Text::PageStopwatchSummary,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_offers_all_five_sections() {
        let entries = destinations();
        assert_eq!(entries.len(), 5);
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.shortcut() as usize, index + 1);
            // The name and the summary are catalogue keys now, so the check is
            // that each one resolves in every language rather than that the
            // stored string is non-empty.
            for language in crate::services::i18n::Language::ALL {
                let catalog = Catalog::new(language);
                assert!(!entry.label_text(catalog).is_empty(), "{language:?}");
                assert!(!entry.summary_text(catalog).is_empty(), "{language:?}");
            }
        }
    }

    #[test]
    fn the_indicator_rests_on_the_selected_entry() {
        let clock = FrameClock::new();
        let navigation = Navigation::new(destinations(), 2);
        assert_eq!(navigation.selected(), 2);
        assert_eq!(navigation.position(clock.now()), 2.0);
        assert_eq!(navigation.weight_of(2, clock.now()), 1.0);
        assert_eq!(navigation.weight_of(0, clock.now()), 0.0);
        assert!(!navigation.is_moving(&clock));
    }

    #[test]
    fn selecting_slides_the_indicator() {
        let clock = FrameClock::new();
        let mut navigation = Navigation::new(destinations(), 0);
        navigation.select(3, &clock);

        assert!(navigation.is_moving(&clock));
        // It starts where the marker was, give or take the sliver of time the
        // call itself took.
        assert!(
            navigation.position(clock.now()) < 0.01,
            "it did not start where it was: {}",
            navigation.position(clock.now())
        );

        std::thread::sleep(Spec::MOVE.duration / 2);
        let halfway = navigation.position(clock.now());
        assert!(halfway > 0.3 && halfway < 2.9, "halfway was {halfway}");

        std::thread::sleep(Spec::MOVE.duration);
        assert_eq!(navigation.position(clock.now()), 3.0);
        assert!(!navigation.is_moving(&clock));
    }

    #[test]
    fn the_bar_belongs_to_exactly_one_item_at_a_time() {
        let clock = FrameClock::new();
        let mut navigation = Navigation::new(destinations(), 0);
        navigation.select(3, &clock);

        for _ in 0..20 {
            let now = clock.now();
            let total: f32 = (0..5).map(|index| navigation.weight_of(index, now)).sum();
            assert!(
                (total - 1.0).abs() < 0.05,
                "the indicator's weight vanished: {total}"
            );
            std::thread::sleep(Spec::MOVE.duration / 20);
        }
    }

    #[test]
    fn a_rapid_second_selection_continues_from_where_the_marker_is() {
        let clock = FrameClock::new();
        let mut navigation = Navigation::new(destinations(), 0);
        navigation.select(4, &clock);
        std::thread::sleep(Spec::MOVE.duration / 3);
        let first = navigation.position(clock.now());

        navigation.select(1, &clock);
        let after = navigation.position(clock.now());
        assert!(
            (first - after).abs() < 0.01,
            "the marker jumped from {first} to {after}"
        );
    }

    #[test]
    fn selecting_the_current_entry_does_nothing() {
        let clock = FrameClock::new();
        let mut navigation = Navigation::new(destinations(), 1);
        navigation.select(1, &clock);
        assert!(!navigation.is_moving(&clock));
        assert!(!clock.active(), "a no-op selection should cost nothing");
    }

    #[test]
    fn an_out_of_range_selection_is_ignored() {
        let clock = FrameClock::new();
        let mut navigation = Navigation::new(destinations(), 1);
        navigation.select(99, &clock);
        assert_eq!(navigation.selected(), 1);
        assert_eq!(navigation.destination().map(|d| d.name), Some("world"));
    }

    #[test]
    fn the_slide_is_monotonic() {
        let clock = FrameClock::new();
        let mut navigation = Navigation::new(destinations(), 0);
        navigation.select(4, &clock);
        let mut previous = navigation.position(clock.now());
        for _ in 0..24 {
            std::thread::sleep(Spec::MOVE.duration / 12);
            let now = clock.now();
            assert!(
                navigation.position(now) >= previous - 1.0e-4,
                "the marker went backwards"
            );
            previous = navigation.position(now);
        }
        std::thread::sleep(Spec::MOVE.duration / 8);
        assert!((navigation.position(clock.now()) - 4.0).abs() < 0.01);
    }

    #[test]
    fn every_destination_has_a_distinct_key() {
        let mut keys = std::collections::HashSet::new();
        for entry in destinations() {
            assert!(keys.insert(entry.key()), "duplicate key {}", entry.key());
        }
    }

    #[test]
    fn the_rail_renders_in_both_forms() {
        let clock = FrameClock::new();
        let navigation = Navigation::new(destinations(), 0);
        let interactions = Interactions::new();

        let expanded: Vec<Element<'static, ()>> = navigation.view(
            Palette::LIGHT,
            Catalog::english(),
            &clock,
            &interactions,
            true,
            true,
            Some(0),
            |_| (),
        );
        assert_eq!(expanded.len(), 5);

        let compact: Vec<Element<'static, String>> = navigation.view(
            Palette::DARK,
            Catalog::english(),
            &clock,
            &interactions,
            false,
            true,
            None,
            |index| {
                destinations()[index]
                    .label_text(Catalog::english())
                    .to_string()
            },
        );
        assert_eq!(compact.len(), 5);
    }

    #[test]
    fn a_bar_of_destinations_renders_as_well_as_a_rail_of_them() {
        // The bar is the same five items in a row, and the width an item takes
        // is the one thing that differs: on a rail an item fills the rail's
        // width, and in a bar it is only as wide as its own mark. Asking for
        // both here is the only way the difference stays deliberate.
        let clock = FrameClock::new();
        let navigation = Navigation::new(destinations(), 0);
        let interactions = Interactions::new();

        for language in crate::services::i18n::Language::ALL {
            let catalog = Catalog::new(language);
            let along: Vec<Element<'static, ()>> = navigation.view(
                Palette::DARK,
                catalog,
                &clock,
                &interactions,
                false,
                false,
                Some(1),
                |_| (),
            );
            let down: Vec<Element<'static, ()>> = navigation.view(
                Palette::DARK,
                catalog,
                &clock,
                &interactions,
                false,
                true,
                Some(1),
                |_| (),
            );
            assert_eq!(along.len(), 5, "{language:?}");
            assert_eq!(down.len(), 5, "{language:?}");
        }
    }

    #[test]
    fn helpers_are_neutral() {
        assert_eq!(rail_border(Palette::LIGHT).radius, 0.0.into());
        assert_eq!(marker_track(5), vec![0.0, 1.0, 2.0, 3.0, 4.0]);
        assert!(marker_track(0).is_empty());
    }
}
