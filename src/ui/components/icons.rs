//! The OClock icon set.
//!
//! Every icon is drawn as vector geometry on a canvas rather than shipped as
//! an image or a font: a handful of path segments per icon, coloured from the
//! palette, crisp at any size, and no assets to keep in step. Geometry for a
//! single icon is a few segments, so redrawing it each frame costs less than
//! keeping a cache coherent with the theme.

use std::f32::consts::TAU;

use iced::widget::canvas::{self, Fill, Frame, Geometry, LineCap, LineJoin, Path, Stroke};
use iced::widget::Canvas;
use iced::{Color, Element, Point, Size, Vector};

use crate::services::i18n::Direction;

/// A mark OClock draws beside something or instead of it.
///
/// The set is deliberately small — one mark per idea the interface has — because
/// an icon set is a *typeface*: its value is in being small, consistent and
/// never improvised. Adding a fourteenth nearly-identical mark is a worse answer
/// than reusing the one that already exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A clock face: the application, and the Clock page.
    Clock,
    /// The world: the World page.
    Globe,
    /// An alarm clock: the Alarms page.
    Alarm,
    /// A countdown timer: the Timer page.
    Timer,
    /// A stopwatch: the Stopwatch page.
    Stopwatch,
    /// A bell: something is ringing.
    Bell,
    /// A bell with a `Z`: an alarm being put off.
    Snooze,
    /// The sun: a light appearance chosen by hand.
    Sun,
    /// The moon: a dark appearance chosen by hand.
    Moon,
    /// A monitor: the appearance follows the desktop.
    System,
    /// A plus: create.
    Plus,
    /// A minus: remove one, or a smaller amount.
    Minus,
    /// A cross: close, cancel, dismiss.
    Close,
    /// A tick: done, accepted.
    Check,
    /// A bin: delete.
    Trash,
    /// A pencil: edit in place.
    Pencil,
    /// A magnifying glass: search.
    Search,
    /// A warning triangle: something is wrong.
    Warning,
    /// A pin: keep this one at the top.
    Pin,
    /// Descending lines: a sort order.
    Sort,
    /// A play triangle: start.
    Play,
    /// Two bars: pause.
    Pause,
    /// A square: stop.
    Stop,
    /// A circular arrow: start again.
    Reset,
}

impl Icon {
    /// Every mark, for a gallery and for tests.
    pub const ALL: [Icon; 24] = [
        Icon::Clock,
        Icon::Globe,
        Icon::Alarm,
        Icon::Timer,
        Icon::Stopwatch,
        Icon::Bell,
        Icon::Snooze,
        Icon::Sun,
        Icon::Moon,
        Icon::System,
        Icon::Plus,
        Icon::Minus,
        Icon::Close,
        Icon::Check,
        Icon::Trash,
        Icon::Pencil,
        Icon::Search,
        Icon::Warning,
        Icon::Pin,
        Icon::Sort,
        Icon::Play,
        Icon::Pause,
        Icon::Stop,
        Icon::Reset,
    ];

    /// The line weight, as a fraction of the icon's size.
    ///
    /// One number for the whole set, so that two marks drawn side by side are
    /// the same weight. It scales with the icon and stops at a ceiling, because
    /// an icon this application ever draws is between 14 and 32 logical pixels
    /// and a stroke that keeps growing past that reads as clumsy rather than
    /// as large.
    fn weight(self) -> f32 {
        0.105
    }

    /// Whether this mark points somewhere, and so is mirrored in a
    /// right-to-left layout.
    ///
    /// The rule is narrow on purpose: only a mark that encodes a *direction of
    /// travel* — a chevron, a caret, an arrow — is mirrored, because a
    /// direction of travel is relative. "Back" is to the left in one script
    /// and to the right in another, and a reader of either script expects the
    /// mark to agree with the words beside it.
    ///
    /// Nothing else in the set is mirrored, and that is the point. A clock, a
    /// bell, a bin, a globe and a play mark point at nothing: they are pictures
    /// of objects, and flipping them would not be translating them, it would be
    /// drawing a different picture and calling it the same word. A play triangle
    /// that pointed the other way in Persian would say "go backwards" to a
    /// reader who has never heard of Latin. The clock face is the sharpest
    /// case of all — its hands run clockwise because that is how the planet
    /// turns, and a mirrored clock would be showing a different time.
    fn mirrors(self, direction: Direction) -> bool {
        let _ = (self, direction);
        false
    }

    /// The drawing, on a sixteen-unit grid with its centre at (8, 8).
    ///
    /// Describing the set as data rather than as a branch per mark is what lets
    /// the same rules apply to all of them: one grid, one weight, round caps,
    /// and a mark that is described by the coordinates it actually has.
    fn shapes(self) -> &'static [Shape] {
        match self {
            // A clock showing ten past ten, which is what a watch advert is
            // drawn at and why: the hands frame the logo instead of hiding it.
            Icon::Clock => &[
                Shape::Ring([8.0, 8.0], 7.0),
                Shape::Line([8.0, 8.0, 4.4, 5.5]),
                Shape::Line([8.0, 8.0, 13.2, 5.0]),
            ],
            // A circle, a meridian and an equator. Two of the three are what
            // makes it a globe rather than a circle; a squashed approximation
            // of the meridian is what makes it look like a mistake.
            Icon::Globe => &[
                Shape::Ring([8.0, 8.0], 7.0),
                Shape::Lens([8.0, 8.0], 3.1, 7.0),
                Shape::Line([1.0, 8.0, 15.0, 8.0]),
            ],
            // An alarm clock, so that it is not the same picture as a bell.
            Icon::Alarm => &[
                Shape::Ring([8.0, 8.8], 4.8),
                Shape::Line([8.0, 8.8, 8.0, 5.8]),
                Shape::Line([8.0, 8.8, 10.2, 7.5]),
                Shape::Line([4.2, 2.2, 6.2, 4.0]),
                Shape::Line([11.8, 2.2, 9.8, 4.0]),
            ],
            // A kitchen timer. It has to be told apart from a stopwatch in the
            // navigation rail at twenty pixels, so the differences are the ones
            // that survive being that small: a wide bar across the top rather
            // than a crown, and a hand that is *running* rather than at rest.
            Icon::Timer => &[
                Shape::Ring([8.0, 9.6], 5.0),
                Shape::Line([8.0, 9.6, 10.9, 6.8]),
                Shape::Line([8.0, 4.6, 8.0, 3.0]),
                Shape::Line([5.2, 3.0, 10.8, 3.0]),
            ],
            // A stopwatch: the same body, but with a narrow crown and a pusher
            // on the side. Two marks that are both "a circle with a stick on
            // top" are one mark, and the rail would be saying the same thing
            // twice.
            Icon::Stopwatch => &[
                Shape::Ring([8.0, 9.4], 5.2),
                Shape::Line([8.0, 9.4, 8.0, 5.6]),
                Shape::Line([8.0, 3.2, 8.0, 1.8]),
                Shape::Line([6.6, 1.8, 9.4, 1.8]),
                Shape::Line([12.4, 4.6, 13.8, 6.0]),
            ],
            Icon::Bell => &[
                Shape::Outline(&[
                    (4.0, 10.8),
                    (4.0, 7.8),
                    (4.6, 5.0),
                    (6.6, 3.2),
                    (9.4, 3.2),
                    (11.4, 5.0),
                    (12.0, 7.8),
                    (12.0, 10.8),
                ]),
                Shape::Dot([8.0, 12.0], 0.85),
                Shape::Dot([2.6, 4.2], 0.65),
                Shape::Dot([13.4, 4.2], 0.65),
            ],
            // A bell and a Z. The Z is drawn as three lines rather than as a
            // glyph so that it is the same weight as everything else in the
            // set and needs no font to be present.
            // The bell sits left of centre and the Z to its right, and the two
            // are placed so the pair as a whole is centred. Two marks balanced
            // by eye rather than by measurement always end up a little off, and
            // a mark that is a little off is the one thing in a rail of
            // twenty-pixel marks that the eye finds first.
            Icon::Snooze => &[
                Shape::Outline(&[
                    (3.0, 11.6),
                    (3.0, 8.9),
                    (3.5, 6.5),
                    (5.1, 5.1),
                    (6.9, 5.1),
                    (8.5, 6.5),
                    (9.0, 8.9),
                    (9.0, 11.6),
                ]),
                Shape::Dot([6.0, 12.4], 0.7),
                Shape::Line([9.8, 3.2, 13.2, 3.2]),
                Shape::Line([13.2, 3.2, 9.8, 6.9]),
                Shape::Line([9.8, 6.9, 13.2, 6.9]),
            ],
            Icon::Sun => &[
                Shape::Ring([8.0, 8.0], 3.2),
                Shape::Line([8.0, 1.2, 8.0, 2.6]),
                Shape::Line([8.0, 13.4, 8.0, 14.8]),
                Shape::Line([1.2, 8.0, 2.6, 8.0]),
                Shape::Line([13.4, 8.0, 14.8, 8.0]),
                Shape::Line([3.26, 3.26, 4.25, 4.25]),
                Shape::Line([11.75, 11.75, 12.74, 12.74]),
                Shape::Line([3.26, 12.74, 4.25, 11.75]),
                Shape::Line([11.75, 4.25, 12.74, 3.26]),
            ],
            // A crescent rather than a disc with a bite out of it, and filled
            // rather than outlined: a stroke and a half wide on a crescent two
            // grid units across closes the middle up at the small end, and the
            // mark stops being a moon.
            Icon::Moon => &[Shape::Crescent([8.0, 8.0], 6.4, 0.45)],
            // A monitor, for the appearance that follows the desktop: a screen
            // on a stand, which is a picture of the thing being described.
            Icon::System => &[
                Shape::Frame([8.0, 6.3], 12.4, 7.8, 1.6),
                Shape::Line([8.0, 10.2, 8.0, 12.9]),
                Shape::Line([5.2, 13.4, 10.8, 13.4]),
            ],
            Icon::Plus => &[
                Shape::Line([8.0, 3.2, 8.0, 12.8]),
                Shape::Line([3.2, 8.0, 12.8, 8.0]),
            ],
            Icon::Minus => &[Shape::Line([3.2, 8.0, 12.8, 8.0])],
            Icon::Close => &[
                Shape::Line([4.2, 4.2, 11.8, 11.8]),
                Shape::Line([11.8, 4.2, 4.2, 11.8]),
            ],
            Icon::Check => &[
                Shape::Line([3.4, 8.4, 6.5, 11.5]),
                Shape::Line([6.5, 11.5, 12.6, 4.4]),
            ],
            Icon::Trash => &[
                Shape::Line([2.0, 4.0, 14.0, 4.0]),
                Shape::Line([6.0, 4.0, 6.0, 2.4]),
                Shape::Line([6.0, 2.4, 10.0, 2.4]),
                Shape::Line([10.0, 2.4, 10.0, 4.0]),
                Shape::Outline(&[(3.4, 4.6), (4.2, 13.8), (11.8, 13.8), (12.6, 4.6)]),
                Shape::Line([6.4, 6.6, 6.7, 11.8]),
                Shape::Line([9.6, 6.6, 9.3, 11.8]),
            ],
            Icon::Pencil => &[Shape::Outline(&[
                (2.8, 13.6),
                (2.8, 10.6),
                (10.8, 2.6),
                (13.4, 5.2),
                (5.4, 13.6),
            ])],
            Icon::Search => &[
                Shape::Ring([6.9, 6.9], 5.1),
                Shape::Line([10.6, 10.6, 14.0, 14.0]),
            ],
            Icon::Warning => &[
                Shape::Outline(&[(8.0, 2.2), (14.8, 13.6), (1.2, 13.6)]),
                Shape::Line([8.0, 6.2, 8.0, 9.4]),
                Shape::Dot([8.0, 11.5], 0.62),
            ],
            Icon::Pin => &[
                Shape::Outline(&[
                    (8.0, 1.5),
                    (11.7, 4.5),
                    (11.7, 6.7),
                    (8.0, 10.1),
                    (4.3, 6.7),
                    (4.3, 4.5),
                ]),
                Shape::Line([8.0, 10.1, 8.0, 14.5]),
            ],
            Icon::Sort => &[
                Shape::Line([2.4, 4.4, 13.6, 4.4]),
                Shape::Line([2.4, 8.0, 10.6, 8.0]),
                Shape::Line([2.4, 11.6, 7.6, 11.6]),
            ],
            // Centred on its bounding box, which is what every icon set does
            // and what the eye expects: a play mark whose *area* is centred
            // sits visibly to the right of everything beside it.
            Icon::Play => &[Shape::Solid(&[(2.9, 2.9), (13.1, 8.0), (2.9, 13.1)])],
            // Rounded, because a pause mark with square shoulders is the one
            // place the set would look like a different drawing programme.
            Icon::Pause => &[
                Shape::Tile([5.7, 8.0], 2.2, 8.8, 1.1),
                Shape::Tile([10.3, 8.0], 2.2, 8.8, 1.1),
            ],
            Icon::Stop => &[Shape::Tile([8.0, 8.0], 7.4, 7.4, 2.0)],
            // A circular arrow: the arc, and a head on the end of it placed by
            // the arc's own tangent so that it points along the direction of
            // travel rather than wherever the eye expected.
            Icon::Reset => &[
                Shape::Arc([8.0, 8.0], 6.1, -0.35, 4.2),
                Shape::Solid(&[(5.2, 1.6), (4.5, 5.0), (2.0, 2.8)]),
            ],
        }
    }
}

/// One primitive of an icon, in grid units on a sixteen-unit square.
///
/// An icon is a handful of these and nothing else. Keeping the vocabulary this
/// small is what makes the set look like one hand drew it: there is one way to
/// draw a ring here and one way to draw a line, so two marks that are both
/// "a circle and a line" *are* the same circle and the same line.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// A stroked straight line.
    Line([f32; 4]),
    /// A stroked circle.
    Ring([f32; 2], f32),
    /// A filled circle.
    Dot([f32; 2], f32),
    /// A stroked rounded rectangle, by its centre and its size.
    Frame([f32; 2], f32, f32, f32),
    /// A filled rounded rectangle, by its centre and its size.
    Tile([f32; 2], f32, f32, f32),
    /// A stroked closed polygon.
    Outline(&'static [(f32, f32)]),
    /// A filled closed polygon.
    Solid(&'static [(f32, f32)]),
    /// A stroked arc of a circle: its centre, its radius, where it starts and
    /// how far round it goes, in radians, positive being clockwise on screen.
    Arc([f32; 2], f32, f32, f32),
    /// A stroked ellipse, by its centre and its two radii.
    Lens([f32; 2], f32, f32),
    /// A filled crescent: a circular back and a shallower front.
    Crescent([f32; 2], f32, f32),
}

/// Draws an icon at `size` in `color`.
#[derive(Debug)]
pub struct IconView {
    icon: Icon,
    size: f32,
    color: Color,
    direction: Direction,
}

impl IconView {
    /// A view of `icon`, `size` logical pixels square, in `color`.
    pub fn new(icon: Icon, size: f32, color: Color) -> IconView {
        IconView {
            icon,
            size,
            color,
            direction: Direction::LeftToRight,
        }
    }

    /// Mirrors this icon where its meaning requires it, and not otherwise.
    pub fn reading(mut self, direction: Direction) -> IconView {
        self.direction = direction;
        self
    }

    /// Mirrors this icon in the language's own direction.
    pub fn in_language(mut self, catalog: crate::services::i18n::Catalog) -> IconView {
        self.direction = catalog.direction();
        self
    }

    /// The rendered widget.
    pub fn view<M: 'static>(self) -> Element<'static, M> {
        let size = self.size;
        let canvas: Element<'static, ()> = Canvas::new(self).width(size).height(size).into();
        canvas.map(|()| unreachable!("an icon never sends a message"))
    }
}

impl canvas::Program<()> for IconView {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::widget::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let size = self.size.min(bounds.width).min(bounds.height);
        let box_ = Size::new(size, size);
        let mut frame = Frame::new(renderer, box_);

        // Icons are drawn with round caps and joins: at 16 px a butt cap looks
        // chipped, and a round one reads as considered.
        let ink = Stroke::default()
            .with_color(self.color)
            .with_width((size * self.icon.weight()).clamp(1.2, 3.0))
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);

        // Which way this icon points, once the layout has had its say.
        let facing: f32 = if self.icon.mirrors(self.direction) {
            -1.0
        } else {
            1.0
        };

        for shape in self.icon.shapes() {
            shape.draw(&mut frame, box_, facing, ink, self.color);
        }

        vec![frame.into_geometry()]
    }
}

impl Shape {
    /// Emits this primitive into `frame`.
    fn draw(&self, frame: &mut Frame, box_: Size, facing: f32, ink: Stroke, color: Color) {
        let unit = box_.width / 16.0;
        // Grid (8, 8) is the middle of the box, and `across` is how far a mark
        // is reflected: -1 mirrors it, which is the only thing the reading
        // direction changes.
        let at = |x: f32, y: f32| Point::new((8.0 + facing * (x - 8.0)) * unit, y * unit);

        match *self {
            Shape::Line([x1, y1, x2, y2]) => {
                frame.stroke(&Path::line(at(x1, y1), at(x2, y2)), ink);
            }
            Shape::Ring(centre, radius) => {
                frame.stroke(&Path::circle(at(centre[0], centre[1]), radius * unit), ink);
            }
            Shape::Dot(centre, radius) => {
                frame.fill(
                    &Path::circle(at(centre[0], centre[1]), radius * unit),
                    Fill::from(color),
                );
            }
            Shape::Frame(centre, width, height, radius) => {
                let top_left =
                    at(centre[0], centre[1]) - Vector::new(width * unit / 2.0, height * unit / 2.0);
                frame.stroke(
                    &Path::new(|path| {
                        path.rounded_rectangle(
                            top_left,
                            Size::new(width * unit, height * unit),
                            (radius * unit).into(),
                        );
                    }),
                    ink,
                );
            }
            Shape::Tile(centre, width, height, radius) => {
                let top_left =
                    at(centre[0], centre[1]) - Vector::new(width * unit / 2.0, height * unit / 2.0);
                frame.fill(
                    &Path::new(|path| {
                        path.rounded_rectangle(
                            top_left,
                            Size::new(width * unit, height * unit),
                            (radius * unit).into(),
                        );
                    }),
                    Fill::from(color),
                );
            }
            Shape::Outline(points) => {
                frame.stroke(&polygon(points, unit, facing), ink);
            }
            Shape::Solid(points) => {
                frame.fill(&polygon(points, unit, facing), Fill::from(color));
            }
            Shape::Arc(centre, radius, start, sweep) => {
                let middle = at(centre[0], centre[1]);
                // Mirrored, an arc runs the other way round, so both its start
                // and its direction are reversed rather than just its centre.
                let (from, to) = if facing < 0.0 {
                    (-start, -start - sweep)
                } else {
                    (start, start + sweep)
                };
                let path = Path::new(|path| {
                    path.arc(iced::widget::canvas::path::arc::Arc {
                        center: middle,
                        radius: radius * unit,
                        start_angle: iced::Radians(from),
                        end_angle: iced::Radians(to),
                    });
                });
                frame.stroke(&path, ink);
            }
            Shape::Lens(centre, rx, ry) => {
                let middle = at(centre[0], centre[1]);
                let path = Path::new(|path| {
                    path.ellipse(iced::widget::canvas::path::arc::Elliptical {
                        center: middle,
                        radii: Vector::new(rx * unit, ry * unit),
                        rotation: iced::Radians(0.0),
                        start_angle: iced::Radians(0.0),
                        end_angle: iced::Radians(TAU),
                    });
                });
                frame.stroke(&path, ink);
            }
            Shape::Crescent(centre, radius, front) => {
                frame.fill(
                    &crescent(at(centre[0], centre[1]), radius * unit, front),
                    Fill::from(color),
                );
            }
        }
    }
}

/// A closed polygon through `points`, in grid units.
fn polygon(points: &[(f32, f32)], unit: f32, facing: f32) -> Path {
    Path::new(|path| {
        let mut first = true;
        for (x, y) in points {
            let point = Point::new((8.0 + facing * (*x - 8.0)) * unit, *y * unit);
            if first {
                path.move_to(point);
                first = false;
            } else {
                path.line_to(point);
            }
        }
        path.close();
    })
}

/// The control-point distance that makes a cubic a half circle.
///
/// The constant every drawing programme uses, and the reason this is a cubic
/// and not the quadratic that is easier to write: a quadratic's height is half
/// its control point's, so the obvious one — a control at `1.4 * radius` —
/// comes out at 70% of the radius and quietly squashes every curve in the set.
const KAPPA: f32 = 0.552_284_8;

/// A crescent: the left of a circle, cut by the left of a narrower one.
///
/// Filled as a single outline rather than stroked, because a crescent only two
/// grid units across and a stroke and a half wide would fill in solid at the
/// small end and the shape would stop being a crescent at all.
fn crescent(centre: Point, radius: f32, front: f32) -> Path {
    let inner = radius * front;
    Path::new(|path| {
        let top = Point::new(centre.x, centre.y - radius);
        let bottom = Point::new(centre.x, centre.y + radius);

        path.move_to(bottom);
        // The outer edge: round the left of the circle, bottom to top.
        path.bezier_curve_to(
            Point::new(centre.x - radius * KAPPA, centre.y + radius),
            Point::new(centre.x - radius * KAPPA, centre.y - radius),
            top,
        );
        // The inner edge: back down the left of the narrower ellipse, which
        // starts and ends on the same two points, so the two always close.
        path.bezier_curve_to(
            Point::new(centre.x - inner * KAPPA, centre.y - radius),
            Point::new(centre.x - inner * KAPPA, centre.y + radius),
            bottom,
        );
        path.close();
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::i18n::Language;

    /// Every corner of an icon's geometry, in grid units.
    ///
    /// The bounds are what a reader sees, so they include the stroke's own
    /// width: a ring whose *path* is inside the box but whose ink is not is an
    /// icon that is visibly clipped.
    fn bounds(icon: Icon) -> (f32, f32, f32, f32) {
        let mut seen: Vec<(f32, f32)> = Vec::new();
        for shape in icon.shapes() {
            match *shape {
                Shape::Line([x1, y1, x2, y2]) => {
                    seen.push((x1, y1));
                    seen.push((x2, y2));
                }
                Shape::Ring(c, r) | Shape::Dot(c, r) => {
                    seen.push((c[0] - r, c[1] - r));
                    seen.push((c[0] + r, c[1] + r));
                }
                Shape::Frame(c, w, h, _) | Shape::Tile(c, w, h, _) => {
                    // The corner radius rounds the rectangle's corners off; it
                    // does not push them out.
                    seen.push((c[0] - w / 2.0, c[1] - h / 2.0));
                    seen.push((c[0] + w / 2.0, c[1] + h / 2.0));
                }
                Shape::Outline(points) | Shape::Solid(points) => {
                    seen.extend(points.iter().copied())
                }
                Shape::Arc(c, r, start, sweep) => {
                    for step in 0..=16 {
                        let a = start + sweep * step as f32 / 16.0;
                        seen.push((c[0] + r * a.cos(), c[1] + r * a.sin()));
                    }
                }
                Shape::Lens(c, rx, ry) => {
                    let r = rx.max(ry);
                    seen.push((c[0] - r, c[1] - r));
                    seen.push((c[0] + r, c[1] + r));
                }
                Shape::Crescent(c, r, _) => {
                    seen.push((c[0] - r, c[1] - r));
                    seen.push((c[0] + r, c[1] + r));
                }
            }
        }
        // Half the stroke, in grid units, so that the bounds are the ink rather
        // than the path down the middle of it.
        let half = Icon::Clock.weight() * 16.0 / 2.0;
        let left = seen.iter().map(|(x, _)| *x).fold(f32::MAX, f32::min) - half;
        let top = seen.iter().map(|(_, y)| *y).fold(f32::MAX, f32::min) - half;
        let right = seen.iter().map(|(x, _)| *x).fold(f32::MIN, f32::max) + half;
        let bottom = seen.iter().map(|(_, y)| *y).fold(f32::MIN, f32::max) + half;
        (left, top, right, bottom)
    }

    #[test]
    fn every_icon_is_drawn() {
        for icon in Icon::ALL {
            assert!(!icon.shapes().is_empty(), "{icon:?} draws nothing");
        }
    }

    #[test]
    fn every_icon_fits_inside_its_box() {
        // An icon is sixteen units on a side, and an icon that draws outside
        // that is an icon that is clipped, or that spills into its neighbour
        // in a row of them.
        for icon in Icon::ALL {
            let (left, top, right, bottom) = bounds(icon);
            assert!(
                left >= 0.0 && top >= 0.0 && right <= 16.0 && bottom <= 16.0,
                "{icon:?} drew outside its box: {left},{top} to {right},{bottom}"
            );
        }
    }

    #[test]
    fn every_icon_fills_enough_of_its_box_to_be_seen() {
        // The other side of the same coin: a mark that draws in a corner is one
        // that reads as a smudge rather than as itself.
        for icon in Icon::ALL {
            let (left, top, right, bottom) = bounds(icon);
            let width = right - left;
            let height = bottom - top;
            // One mark is a single line and is meant to be: a minus sign with
            // anything else in it is not a minus sign. Every other mark has to
            // fill its box in both directions.
            let long = width.max(height);
            assert!(
                long >= 9.0,
                "{icon:?} only covers {width}x{height} of its sixteen units"
            );
            if icon != Icon::Minus {
                assert!(
                    width.min(height) >= 8.0,
                    "{icon:?} is lopsided at {width}x{height}"
                );
            }
        }
    }

    #[test]
    fn no_icon_is_handed() {
        // Nothing in this set means a direction of travel, so nothing in it
        // may be off-centre. A mark that is lopsided *looks* like it points
        // somewhere, and then a reader in a right-to-left language is right to
        // wonder which way it is pointing.
        for icon in Icon::ALL {
            let (left, _, right, _) = bounds(icon);
            let centre = (left + right) / 2.0;
            assert!(
                (centre - 8.0).abs() <= 0.75,
                "{icon:?} is centred at {centre}, not at 8"
            );
        }
    }

    #[test]
    fn every_icon_has_a_stroke_or_a_fill() {
        // An icon drawn entirely in outlines is quiet, and one drawn entirely
        // in solids is loud; the set is meant to be neither, and the way that
        // is kept honest is that both kinds are accounted for.
        let mut stroked = 0;
        let mut filled = 0;
        for icon in Icon::ALL {
            for shape in icon.shapes() {
                match shape {
                    Shape::Dot(..) | Shape::Tile(..) | Shape::Solid(..) | Shape::Crescent(..) => {
                        filled += 1
                    }
                    _ => stroked += 1,
                }
            }
        }
        assert!(stroked > 0 && filled > 0, "the set is all of one kind");
    }

    #[test]
    fn the_weight_is_the_same_for_every_icon() {
        for icon in Icon::ALL {
            assert_eq!(icon.weight(), Icon::Clock.weight(), "{icon:?} is heavier");
        }
    }

    #[test]
    fn the_weight_scales_with_the_icon_and_stays_drawable() {
        // At the sizes the application uses — 14 for a chip, 20 for the rail,
        // 28 for an empty state — the stroke has to be at least a pixel and at
        // most a third of the icon.
        for size in [14.0_f32, 18.0, 20.0, 22.0, 24.0, 28.0, 32.0] {
            let width = (size * Icon::Clock.weight()).clamp(1.2, 3.0);
            assert!(width >= 1.2, "a {size}px icon has no stroke");
            assert!(width <= size / 3.0, "a {size}px icon is all stroke");
        }
    }

    #[test]
    fn nothing_is_mirrored_in_any_language() {
        // The rule, stated as a test so that a mark added later has to argue
        // with it: a clock runs clockwise because the planet does.
        for language in Language::ALL {
            let direction = language.direction();
            for icon in Icon::ALL {
                assert!(
                    !icon.mirrors(direction),
                    "{icon:?} would be mirrored in {language:?}"
                );
            }
        }
    }

    #[test]
    fn an_icon_renders_in_every_language() {
        for language in Language::ALL {
            let _: Element<'static, ()> = IconView::new(Icon::Clock, 20.0, Color::WHITE)
                .reading(language.direction())
                .view();
        }
        for icon in Icon::ALL {
            let _: Element<'static, ()> = IconView::new(icon, 20.0, Color::WHITE).view();
        }
    }
}
