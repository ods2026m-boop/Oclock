//! Surfaces: the cards, panels and containers the interface is built from.
//!
//! The visual language is a stack of surfaces. The window canvas is the
//! background, a card lifts off it, and an inset well sinks into a card. Each
//! level has exactly one fill, one outline and one shadow in the palette, so
//! depth comes from a small set of decisions rather than from ad-hoc colours.

use iced::widget::container::{Container, Style as ContainerStyle};
use iced::widget::{column, row, scrollable, Space, Stack};
use iced::{Background, Border, Color, Element, Length};

use crate::design::motion::ease;
use crate::design::palette::{Palette, ShadowLevel};
use crate::design::tokens::{radius, space, stroke};
use crate::services::i18n::Direction;

/// How a surface is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// The ordinary card: the surface colour with a hairline outline.
    Surface,
    /// A surface that recedes: the sunken colour, no outline.
    Sunken,
    /// A wash of the accent, for a selected row.
    Accent,
    /// A wash of the accent for an active or ringing state.
    AccentStrong,
    /// A wash of the danger colour.
    Danger,
    /// Fully transparent: for layout, not for content.
    None,
}

impl Fill {
    fn colour(self, palette: Palette) -> Color {
        match self {
            Fill::Surface => palette.surface,
            Fill::Sunken => palette.sunken,
            Fill::Accent => palette.accent_soft,
            Fill::AccentStrong => palette.accent.scale_alpha(0.22),
            Fill::Danger => palette.danger_soft,
            Fill::None => Color::TRANSPARENT,
        }
    }

    fn outline(self, palette: Palette) -> Color {
        match self {
            Fill::Surface => palette.outline,
            Fill::Accent | Fill::AccentStrong => palette.accent.scale_alpha(0.35),
            Fill::Danger => palette.danger.scale_alpha(0.35),
            Fill::Sunken | Fill::None => Color::TRANSPARENT,
        }
    }

    fn corner(self) -> f32 {
        match self {
            Fill::Sunken => radius::MD,
            Fill::None => 0.0,
            _ => radius::XL,
        }
    }
}

/// A surface with a stack of children.
pub struct Card<M> {
    palette: Palette,
    fill: Fill,
    padding_x: f32,
    padding_y: f32,
    corner: f32,
    children: Vec<Element<'static, M>>,
    spacing: f32,
    shadow: Option<ShadowLevel>,
    /// A vertical gradient, for the one hero surface per view.
    gradient: Option<Background>,
    width: Length,
}

impl<M: 'static> Card<M> {
    /// A surface filled with `fill`.
    pub fn new(palette: Palette, fill: Fill) -> Card<M> {
        Card {
            palette,
            fill,
            padding_x: space::XL,
            padding_y: space::XL,
            corner: fill.corner(),
            children: Vec::new(),
            spacing: space::MD,
            shadow: Some(ShadowLevel::Low),
            gradient: None,
            width: Length::Shrink,
        }
    }

    /// The ordinary card.
    pub fn standard(palette: Palette) -> Card<M> {
        Card::new(palette, Fill::Surface)
    }

    /// A surface that sits back: an input well, a table header.
    pub fn subtle(palette: Palette) -> Card<M> {
        Card {
            shadow: None,
            ..Card::new(palette, Fill::Sunken)
        }
    }

    /// A card with no shadow, for a page that should not float.
    pub fn flat(palette: Palette) -> Card<M> {
        Card {
            shadow: None,
            ..Card::new(palette, Fill::Surface)
        }
    }

    /// The hero surface: a card with the accent's wash across the top.
    pub fn hero(palette: Palette) -> Card<M> {
        Card {
            shadow: Some(ShadowLevel::Medium),
            gradient: Some(palette.hero_gradient()),
            ..Card::new(palette, Fill::Surface)
        }
    }

    /// Sets the inner padding on every side.
    pub fn padding(mut self, padding: iced::padding::Padding) -> Card<M> {
        self.padding_x = padding.left;
        self.padding_y = padding.top;
        self
    }

    /// Sets uniform inner padding.
    pub fn pad(mut self, padding: f32) -> Card<M> {
        self.padding_x = padding;
        self.padding_y = padding;
        self
    }

    /// Sets horizontal and vertical inner padding separately.
    pub fn pad_xy(mut self, horizontal: f32, vertical: f32) -> Card<M> {
        self.padding_x = horizontal;
        self.padding_y = vertical;
        self
    }

    /// Adds a child.
    pub fn push(mut self, child: impl Into<Element<'static, M>>) -> Card<M> {
        self.children.push(child.into());
        self
    }

    /// Sets the outer width, which a card needs when it is a column inside
    /// another card rather than a row beside one.
    pub fn width(mut self, width: Length) -> Card<M> {
        self.width = width;
        self
    }

    /// Sets the gap between children.
    pub fn spacing(mut self, spacing: f32) -> Card<M> {
        self.spacing = spacing;
        self
    }

    /// Stretches to the full width of its parent.
    pub fn fill_width(mut self) -> Card<M> {
        self.width = Length::Fill;
        self
    }

    /// The rendered widget.
    pub fn view(self) -> Element<'static, M> {
        let Card {
            palette,
            fill,
            padding_x,
            padding_y,
            corner,
            children,
            spacing,
            shadow,
            gradient,
            width,
        } = self;

        let outline = fill.outline(palette);
        let shadow = shadow
            .map(|level| palette.shadow(level))
            .unwrap_or_default();
        let background = gradient.unwrap_or_else(|| Background::Color(fill.colour(palette)));

        let body = Container::new(column(children).spacing(spacing)).padding(
            iced::padding::top(padding_y)
                .bottom(padding_y)
                .left(padding_x)
                .right(padding_x),
        );

        let style = move |_theme: &iced::Theme| ContainerStyle {
            background: Some(background),
            border: Border {
                color: outline,
                width: stroke::HAIRLINE,
                radius: corner.into(),
            },
            shadow,
            ..ContainerStyle::default()
        };

        Container::new(body).style(style).width(width).into()
    }
}

/// Lets a card be used wherever an element is expected, so panels nest.
impl<M: 'static> From<Card<M>> for Element<'static, M> {
    fn from(card: Card<M>) -> Self {
        card.view()
    }
}

/// A one-pixel rule, for separating list rows.
pub fn divider<M: 'static>(palette: Palette, vertical: bool) -> Element<'static, M> {
    let style = move |_theme: &iced::Theme| ContainerStyle {
        background: Some(Background::Color(palette.outline)),
        ..ContainerStyle::default()
    };

    let thin = Length::from(1);
    let rule = if vertical {
        Container::new(Space::new(thin, Length::Fill))
    } else {
        Container::new(Space::new(Length::Fill, thin))
    };

    rule.style(style).into()
}

/// A row that leaves room on the right, the standard way two things sit side by
/// side with one pushed to the end.
pub fn spread<M: 'static>(
    leading: Element<'static, M>,
    trailing: Element<'static, M>,
) -> Element<'static, M> {
    row![leading, Space::with_width(Length::Fill), trailing]
        .align_y(iced::Alignment::Center)
        .into()
}

/// A stack of children with a gap.
pub fn stack<M: 'static>(children: Vec<Element<'static, M>>, spacing: f32) -> Element<'static, M> {
    column(children).spacing(spacing).into()
}

/// A row of children with a gap, wrapping when it runs out of room.
pub fn flow<M: 'static>(children: Vec<Element<'static, M>>, spacing: f32) -> Element<'static, M> {
    // A wrapping row is the responsive answer for chips and preset strips: a
    // window that grows adds a column, and one that shrinks loses one, with no
    // breakpoint to get wrong.
    row(children)
        .spacing(spacing)
        .align_y(iced::Alignment::Center)
        .wrap()
        .into()
}

/// Shifts `content` horizontally by `x` pixels.
///
/// Iced 0.13 has no translation widget, so the shift is made with the layout:
/// a spacer before the content pushes it right, and a trailing spacer after a
/// filling one pushes it left. Both keep the content at its natural size, which
/// matters because the thing being shifted is a whole page.
///
/// The caller passes the shift already resolved for the language, because which
/// way a page should come *from* is a question about the reading direction and
/// not about geometry. See [`crate::ui::shell`].
pub fn slide<M: 'static>(content: Element<'static, M>, x: f32) -> Element<'static, M> {
    if x.abs() < 0.5 {
        return content;
    }

    let aligned = row![content].width(Length::Fill);

    if x > 0.0 {
        row![Space::with_width(x), aligned].into()
    } else {
        row![
            Space::with_width(Length::Fill),
            aligned,
            Space::with_width(-x)
        ]
        .into()
    }
}

/// Makes `content` recede toward the canvas.
///
/// Iced 0.13 has no per-widget opacity, and the page transition needs one:
/// the incoming page has to be able to appear out of the background rather than
/// snap into it. Laying the canvas colour over the content at increasing
/// strength produces exactly that, costs one rectangle, and — because the colour
/// being laid over is the one the content already sits on — it cannot shift a
/// single hue.
pub fn recede<M: 'static>(
    palette: Palette,
    content: Element<'static, M>,
    amount: f32,
) -> Element<'static, M> {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= 0.002 {
        return content;
    }

    let veil: Element<'static, M> = scrim(
        crate::design::palette::overlay(
            crate::design::motion::ease::fade(palette.canvas, amount),
            palette.canvas,
        ),
        Space::with_width(Length::Fill).into(),
    );

    Stack::with_children(vec![content, veil])
        .width(Length::Fill)
        .into()
}

/// The padding a scrollable's content needs so its track does not land on it.
///
/// Iced draws a vertical scroll's bar against the *right* edge of the viewport in
/// every language, with no way to move it. That is the page's trailing edge in a
/// left-to-right language and its leading edge in a right-to-left one, so the
/// side the inset goes on is the same either way and only what it costs changes:
/// in a right-to-left page it is the side every card border and every section
/// eyebrow hangs from, and a page whose leading edge is under a track looks
/// broken.
///
/// The inset goes *inside* the scrollable rather than onto the frame around it,
/// because the track is drawn against the viewport's edge: padding the frame
/// would move that edge in by the same amount and leave the collision exactly
/// where it was.
pub fn scrollbar_inset(_direction: Direction) -> iced::Padding {
    let gutter = crate::design::tokens::scrollbar::WIDTH + crate::design::tokens::scrollbar::GAP;
    iced::padding::right(gutter)
}

/// A vertical scroll area, in OClock's own styling.
///
/// Iced's own scrollbar is a full-width, square-ended bar in the strongest of
/// its three background greys. Against a page of white cards that reads as a
/// control rather than as a hint about where there is more to read, and it is
/// the one mark on the page drawn in a colour the palette never chose. This one
/// is a hairline, rounded at both ends, drawn in the outline colour, and only
/// steps forward when the pointer is actually over it — which is the one moment
/// a reader is looking for it.
///
/// The rail is transparent rather than a recessed groove: a groove is a second
/// vertical line running the height of the page, and a page that already has a
/// card edge down each side does not need a third.
pub fn scroll_area<'a, M: 'static>(
    content: Element<'a, M>,
    palette: Palette,
) -> scrollable::Scrollable<'a, M> {
    let style = move |_theme: &iced::Theme, status: scrollable::Status| {
        let engaged = matches!(
            status,
            scrollable::Status::Hovered {
                is_vertical_scrollbar_hovered: true,
                ..
            } | scrollable::Status::Dragged {
                is_vertical_scrollbar_dragged: true,
                ..
            }
        );
        let colour = ease::color(palette.outline, palette.text_faint, f32::from(engaged));

        let rail = |scroller: Color| scrollable::Rail {
            background: None,
            border: Border {
                radius: radius::PILL.into(),
                ..Border::default()
            },
            scroller: scrollable::Scroller {
                color: scroller,
                // The radius is what rounds the bar; the width is zero, because
                // the bar is one solid colour and a border would only put a
                // ring around it.
                border: Border {
                    radius: radius::PILL.into(),
                    ..Border::default()
                },
            },
        };

        scrollable::Style {
            container: ContainerStyle::default(),
            vertical_rail: rail(colour),
            horizontal_rail: rail(Color::TRANSPARENT),
            gap: None,
        }
    };

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style)
}

/// A scrim over the whole window, used by the theme transition and by dialogs.
pub fn scrim<M: 'static>(colour: Color, content: Element<'static, M>) -> Element<'static, M> {
    let style = move |_theme: &iced::Theme| ContainerStyle {
        background: Some(Background::Color(colour)),
        ..ContainerStyle::default()
    };
    Container::new(content)
        .style(style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// A panel with a heading, used for the grouped sections of a page.
pub struct Panel<M> {
    card: Card<M>,
    heading: Option<Element<'static, M>>,
    trailing: Option<Element<'static, M>>,
}

impl<M: 'static> Panel<M> {
    /// A panel with the given heading.
    pub fn new(palette: Palette, heading: impl Into<Element<'static, M>>) -> Panel<M> {
        let heading = heading.into();
        Panel {
            card: Card::flat(palette).spacing(space::LG),
            heading: Some(heading),
            trailing: None,
        }
    }

    /// Adds a control to the heading row, on the right.
    pub fn action(mut self, trailing: impl Into<Element<'static, M>>) -> Panel<M> {
        let trailing = trailing.into();
        self.trailing = Some(trailing);
        self
    }

    /// Adds body content.
    pub fn push(mut self, child: impl Into<Element<'static, M>>) -> Panel<M> {
        self.card = self.card.push(child);
        self
    }

    /// The rendered widget.
    pub fn view(mut self) -> Element<'static, M> {
        if let Some(heading) = self.heading.take() {
            let heading_row = match self.trailing.take() {
                Some(trailing) => spread(heading, trailing),
                None => heading,
            };
            self.card = self.card.push(heading_row);
        }
        self.card.view()
    }
}

/// Empty space above a panel's body, sized from the spacing scale.
pub fn gap<M: 'static>(size: f32) -> Element<'static, M> {
    Space::with_height(size).into()
}

/// An element that fills both axes, used to push a modal over the window.
pub fn fill<M: 'static>(child: Element<'static, M>) -> Element<'static, M> {
    Container::new(child)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::palette::contrast;

    #[test]
    fn every_surface_kind_has_a_distinct_fill_in_both_themes() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            let fills = [
                Fill::Surface,
                Fill::Sunken,
                Fill::Accent,
                Fill::AccentStrong,
                Fill::Danger,
            ];
            for (index, left) in fills.iter().enumerate() {
                for right in &fills[index + 1..] {
                    assert_ne!(
                        left.colour(palette),
                        right.colour(palette),
                        "{left:?} and {right:?} are the same colour"
                    );
                }
            }
        }
    }

    #[test]
    fn a_sunken_surface_sinks_below_its_card() {
        // Depth is the whole point: the well has to be darker than the card it
        // sits in, or the hierarchy inverts.
        for palette in [Palette::LIGHT, Palette::DARK] {
            let surface = palette.surface;
            let sunken = Fill::Sunken.colour(palette);
            let surface_luma = surface.r * 0.2126 + surface.g * 0.7152 + surface.b * 0.0722;
            let sunken_luma = sunken.r * 0.2126 + sunken.g * 0.7152 + sunken.b * 0.0722;
            assert!(
                sunken_luma < surface_luma,
                "a well should be darker than its card in {palette:?}"
            );
        }
    }

    #[test]
    fn cards_are_visible_against_the_canvas() {
        for palette in [Palette::LIGHT, Palette::DARK] {
            assert!(
                contrast(Fill::Surface.colour(palette), palette.canvas) > 1.01,
                "a card would vanish into the window"
            );
        }
    }

    #[test]
    fn only_solid_surfaces_carry_an_outline() {
        let palette = Palette::DARK;
        assert_ne!(Fill::Surface.outline(palette), Color::TRANSPARENT);
        for fill in [Fill::Sunken, Fill::None] {
            assert_eq!(fill.outline(palette), Color::TRANSPARENT);
        }
    }

    #[test]
    fn corners_step_down_through_the_hierarchy() {
        assert_eq!(Fill::Sunken.corner(), radius::MD);
        assert_eq!(Fill::Surface.corner(), radius::XL);
        assert_eq!(Fill::None.corner(), 0.0);
    }

    #[test]
    fn surfaces_render() {
        let palette = Palette::LIGHT;
        let _: Element<'static, ()> = Card::<()>::standard(palette)
            .push(Space::with_width(10))
            .view();
        let _: Element<'static, ()> = Card::<()>::subtle(palette).pad(8.0).view();
        let _: Element<'static, ()> = Card::<()>::standard(palette).pad_xy(4.0, 8.0).view();
        let _: Element<'static, ()> = Card::<()>::standard(palette)
            .padding(iced::padding::all(6.0))
            .view();
        let _: Element<'static, ()> = Card::<()>::hero(palette).fill_width().view();
        let _: Element<'static, ()> = Card::<()>::flat(palette).push(Space::with_width(10)).view();
        let _: Element<'static, ()> = divider::<()>(palette, true);
        let _: Element<'static, ()> =
            spread(Space::with_width(1).into(), Space::with_width(2).into());
        let _: Element<'static, ()> = stack(vec![], 4.0);
        let _: Element<'static, ()> = flow(vec![], 4.0);
        let _: Element<'static, ()> = scrim(Color::BLACK, Space::with_width(1).into());
        let heading: Element<'static, ()> = Space::with_width(1).into();
        let action: Element<'static, ()> = Space::with_width(2).into();
        let _: Element<'static, ()> = Panel::new(palette, heading)
            .action(action)
            .push(Space::with_width(3))
            .view();
        let _: Element<'static, ()> = gap::<()>(8.0);
        let _: Element<'static, ()> = fill(Space::with_width(1).into());
    }

    use crate::services::i18n::Direction;

    #[test]
    fn the_scroll_inset_is_on_the_side_the_track_is_drawn() {
        // Iced draws the track against the right edge of the viewport in *every*
        // language. So the inset is on the right in every language, and a
        // right-to-left page is the one that pays for it: there the right edge
        // is the page's leading one, where its card borders and its section
        // eyebrows live.
        //
        // This was measured rather than assumed. The first attempt put the inset
        // on the leading side in a right-to-left page, on the reasoning that the
        // track would be there too; the only way to know was to look at a
        // running window, where it turned out to be on the right like everywhere
        // else.
        let gutter =
            crate::design::tokens::scrollbar::WIDTH + crate::design::tokens::scrollbar::GAP;
        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
            let inset = scrollbar_inset(direction);
            assert_eq!(
                inset.right, gutter,
                "{direction:?} has to leave room on its right"
            );
            assert_eq!(
                inset.left, 0.0,
                "{direction:?} also inset the side with no track on it"
            );
        }
    }

    #[test]
    fn the_track_is_wide_enough_to_be_a_target() {
        // A track thinner than the scrollbar it represents is a hint nobody can
        // hit, and the gap has to be there for the content it is protecting.
        const {
            assert!(crate::design::tokens::scrollbar::WIDTH >= 8.0);
            assert!(crate::design::tokens::scrollbar::GAP >= 4.0);
        }
    }
}
