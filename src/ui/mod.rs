//! The OClock interface.
//!
//! [`typography`] and [`components`] are the design system's vocabulary;
//! [`shell`] arranges them into a window; [`pages`] fill it; [`text_metrics`]
//! measures the runs of text that typography lays out.
//!
//! Iced lays every row out left to right and offers no mirrored container, so a
//! right-to-left interface is written by naming its edges in *reading* order and
//! letting the helpers here put them on the correct sides. Writing
//! `row![trailing, leading]` into a page would work exactly once and read as a
//! mistake every other time.

use iced::widget::{row, Row};
use iced::Element;

use crate::services::i18n::Direction;

pub mod components;
pub mod pages;
pub mod shell;
pub mod text_metrics;
pub mod typography;

/// The alignment a block hangs from, which is its leading edge.
pub fn leading(direction: Direction) -> iced::Alignment {
    direction.align_x()
}

/// A row of things given in reading order.
///
/// This is the general form: a list of items is written in the order a reader
/// meets them and the row is built the other way round when the language needs
/// it. [`ends`] and [`sequence`] are the two- and three-item spellings of it.
pub fn ordered<M: 'static>(
    direction: Direction,
    items: Vec<Element<'static, M>>,
) -> Row<'static, M> {
    if direction.is_rtl() {
        row(items.into_iter().rev().collect::<Vec<_>>())
    } else {
        row(items)
    }
}

/// A row holding its two ends in reading order.
///
/// The first argument is what a reader meets first, the second what follows it.
/// Which side of the window that lands on is the language's business, not the
/// caller's: Persian puts its navigation on the right, English on the left, and
/// the same expression describes both.
pub fn ends<M: 'static>(
    direction: Direction,
    first: impl Into<Element<'static, M>>,
    then: impl Into<Element<'static, M>>,
) -> Row<'static, M> {
    ordered(direction, vec![first.into(), then.into()])
}

/// A row of three things in reading order.
pub fn sequence<M: 'static>(
    direction: Direction,
    first: impl Into<Element<'static, M>>,
    second: impl Into<Element<'static, M>>,
    third: impl Into<Element<'static, M>>,
) -> Row<'static, M> {
    ordered(direction, vec![first.into(), second.into(), third.into()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_leading_edge_is_the_start_of_the_line() {
        assert_eq!(leading(Direction::LeftToRight), iced::Alignment::Start);
        assert_eq!(leading(Direction::RightToLeft), iced::Alignment::End);
    }

    #[test]
    fn reading_order_survives_both_directions() {
        // A widget is a value that cannot be laid out twice, so this can only
        // check that both branches build, not where they land. What it pins
        // down is that a caller never has to think about which side is which.
        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
            let _ends: Row<'static, ()> = ends(direction, "a", "b");
            let _sequence: Row<'static, ()> = sequence(direction, "a", "b", "c");
        }
    }
}
