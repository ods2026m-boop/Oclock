//! Drawn indicators: the progress ring, the analog dial and the sweep.
//!
//! These are the parts of OClock that are genuinely a drawing rather than a
//! layout, and they are what give the application its instrument-like feel.
//!
//! Every one of them is a path built from the face's own proportions rather
//! than from a picture, so it stays a drawing at any size and any scale factor.
//! The dial goes further and is *live*: it reads the wall clock as it is drawn
//! instead of being handed a frozen instant, which is what lets its second hand
//! sweep at its true speed and stay right after a resume.

use std::f32::consts::TAU;
use std::time::Duration;

use iced::widget::canvas::{self, Fill, Frame, Geometry, LineCap, Path, Stroke};
use iced::widget::Canvas;
use iced::{Element, Point, Rectangle, Size, Vector};

use crate::core::tz::Zoned;
use crate::design::motion::{ease, FrameClock};
use crate::design::palette::Palette;
use crate::ui::components::interaction::Interaction;

/// A countdown ring.
///
/// The arc is drawn from twelve o'clock, clockwise, and a second thinner arc
/// shows the whole cycle behind it, so the remaining time reads as a quantity
/// rather than a decoration.
pub struct Ring {
    palette: Palette,
    progress: f32,
    clock: FrameClock,
    interaction: Interaction,
    size: f32,
    thickness: f32,
    tint: Option<iced::Color>,
    /// Sweeping highlight added at the head of the arc, for completion.
    glow: f32,
}

impl Ring {
    /// A ring showing `progress` (`0.0..=1.0`).
    pub fn new(
        palette: Palette,
        progress: f32,
        clock: FrameClock,
        interaction: Interaction,
    ) -> Ring {
        Ring {
            palette,
            progress: progress.clamp(0.0, 1.0),
            clock,
            interaction,
            size: 240.0,
            thickness: 14.0,
            tint: None,
            glow: 0.0,
        }
    }

    /// Sets the outer size.
    pub fn size(mut self, size: f32) -> Ring {
        self.size = size;
        self
    }

    /// Sets the stroke thickness.
    pub fn thickness(mut self, thickness: f32) -> Ring {
        self.thickness = thickness;
        self
    }

    /// Draws the arc in `tint` instead of the accent.
    pub fn tint(mut self, tint: iced::Color) -> Ring {
        self.tint = Some(tint);
        self
    }

    /// Adds a highlight that sweeps once when the timer completes.
    pub fn glow(mut self, glow: f32) -> Ring {
        self.glow = glow.clamp(0.0, 1.0);
        self
    }

    /// The rendered widget.
    pub fn view<M: 'static>(self) -> Element<'static, M> {
        let size = self.size;
        let canvas: Element<'static, ()> = Canvas::new(self).width(size).height(size).into();
        canvas.map(|()| unreachable!("a ring never sends a message"))
    }
}

impl canvas::Program<()> for Ring {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        _event: canvas::Event,
        _bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> (iced::event::Status, Option<()>) {
        (iced::event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::widget::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        // Everything is positioned in the frame's own space, whose origin is
        // the widget's top-left corner. A canvas translates what it is given to
        // the widget's position, so geometry built in the parent's coordinates
        // would land outside the widget and never be seen.
        let size = bounds.width.min(bounds.height);
        let center = Point::new(size / 2.0, size / 2.0);
        // The ring catches the pointer, the way a physical instrument's ring
        // would catch the light. The pointer position is available here, so the
        // response needs no event plumbing at all — and because the value is
        // time-parameterised, a slow drag across the ring animates smoothly
        // rather than stepping.
        let hovered = cursor.position().is_some_and(|at| bounds.contains(at));
        let thickness = self.thickness
            * (1.0
                + self.interaction.hover.track(
                    hovered,
                    &self.clock,
                    crate::design::motion::Spec::HOVER,
                ) * 0.14);
        let radius = (size - thickness) / 2.0;
        let accent = self.tint.unwrap_or(self.palette.accent);

        let mut frame = Frame::new(renderer, bounds.size());

        // The track: a full ring in a quiet colour.
        frame.stroke(
            &Path::circle(center, radius),
            Stroke::default()
                .with_color(self.palette.track)
                .with_width(thickness)
                .with_line_cap(LineCap::Round),
        );

        if self.progress <= 0.0 {
            return vec![frame.into_geometry()];
        }

        // The arc, from twelve o'clock clockwise.
        let sweep = self.progress * TAU;
        frame.stroke(
            &arc_path(center, radius, -std::f32::consts::FRAC_PI_2, sweep),
            Stroke::default()
                .with_color(accent)
                .with_width(thickness)
                .with_line_cap(LineCap::Round),
        );

        // A rounded cap sits slightly beyond the end of the arc, which is what
        // makes a partial ring look deliberate rather than cut off.
        let end = Point::new(
            center.x + (sweep - std::f32::consts::FRAC_PI_2).cos() * radius,
            center.y + (sweep - std::f32::consts::FRAC_PI_2).sin() * radius,
        );
        frame.fill(&Path::circle(end, thickness / 2.0), Fill::from(accent));

        if self.glow > 0.001 {
            // The completion flourish: a brighter cap travelling once around.
            let travel = (self.clock.now().as_secs_f32() * 1.6) % 1.0;
            let angle = -std::f32::consts::FRAC_PI_2 + travel * TAU;
            let at = Point::new(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            );
            frame.fill(
                &Path::circle(at, thickness * 0.9),
                Fill::from(ease::fade(
                    self.palette.on_accent(),
                    self.glow * 0.8 * (1.0 - travel),
                )),
            );
        }

        vec![frame.into_geometry()]
    }
}

/// The points along an arc, shared by the ring and the dial.
fn arc_points(center: Point, radius: f32, start: f32, sweep: f32) -> Vec<Point> {
    // One segment per 4°, clamped: smooth at 240 px without a thousand points.
    let steps = ((sweep.abs() / 0.07).ceil() as usize).clamp(2, 96);
    (0..=steps)
        .map(|step| {
            let angle = start + sweep * step as f32 / steps as f32;
            Point::new(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            )
        })
        .collect()
}

fn arc_path(center: Point, radius: f32, start: f32, sweep: f32) -> Path {
    let points = arc_points(center, radius, start, sweep);
    Path::new(move |path| {
        for (index, at) in points.iter().enumerate() {
            if index == 0 {
                path.move_to(*at);
            } else {
                path.line_to(*at);
            }
        }
    })
}

/// An analog clock face.
///
/// Everything is derived from the same instant, so the hour and minute hands
/// and the second hand can never disagree — including across a daylight-saving
/// change, because the hands are computed from the local wall time rather than
/// from an accumulated count.
///
/// The face is a live instrument rather than a picture of one. It reads the
/// wall clock as it draws rather than taking its angles from the per-second
/// snapshot, so the second hand sweeps at its true speed instead of stepping in
/// whole seconds, and it is correct immediately after a resume or a correction
/// of the system clock.
pub struct Dial {
    palette: Palette,
    at: Zoned,
    /// How the face gets its hands.
    source: Source,
    size: f32,
    /// Whether to draw the hour markers.
    markers: bool,
    /// An extra tint for the second hand.
    hand: Option<iced::Color>,
}

/// Where a face gets the instant it is showing.
#[derive(Clone, Copy, Debug)]
enum Source {
    /// The wall clock, read as the face is drawn.
    Live,
    /// A fixed instant, with the second's progress through it.
    Fixed(f32),
}

impl Dial {
    /// A dial showing the current time in `at`'s zone.
    ///
    /// The instant comes from the system clock when the face is drawn; `at`
    /// says which zone the face belongs to.
    pub fn new(palette: Palette, at: Zoned) -> Dial {
        Dial {
            palette,
            at,
            source: Source::Live,
            size: 200.0,
            markers: true,
            hand: None,
        }
    }

    /// A dial frozen on a particular instant.
    ///
    /// For a face that is not showing the reader's own time: a world clock's
    /// small faces are read at a glance beside the city they belong to, and a
    /// face that swept with the *local* clock would quietly say the wrong thing
    /// about somewhere else in the world.
    pub fn frozen(palette: Palette, at: Zoned, second_fraction: f32) -> Dial {
        let mut dial = Dial::new(palette, at);
        dial.source = Source::Fixed(second_fraction.clamp(0.0, 1.0));
        dial
    }

    /// Sets the outer size.
    pub fn size(mut self, size: f32) -> Dial {
        self.size = size;
        self
    }

    /// Hides the hour markers, for a small face.
    pub fn bare(mut self) -> Dial {
        self.markers = false;
        self
    }

    /// Draws the second hand in `hand`.
    pub fn hand(mut self, hand: iced::Color) -> Dial {
        self.hand = Some(hand);
        self
    }

    /// The rendered widget.
    pub fn view<M: 'static>(self) -> Element<'static, M> {
        let size = self.size;
        let canvas: Element<'static, ()> = Canvas::new(self).width(size).height(size).into();
        canvas.map(|()| unreachable!("a dial never sends a message"))
    }

    /// The instant this face is showing, and how far into the second it is.
    fn now(&self) -> (Zoned, f32) {
        match self.source {
            Source::Live => {
                let at = crate::core::tz::now_in(self.at.timezone());
                let subsecond = crate::domain::clock::subsecond(&at);
                (at, subsecond)
            }
            Source::Fixed(second_fraction) => (self.at, second_fraction),
        }
    }
}

/// Where the hands sit, as a fraction of a full turn from twelve o'clock.
///
/// All three advance continuously rather than stepping, which is what makes an
/// analog face look right in motion: the minute hand should be somewhere
/// between the markers, not on one. Each is derived from the one below it, so a
/// hand is never seen arriving at its mark ahead of the hand that carries it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hands {
    /// 0.0..1.0 of a turn, clockwise from twelve.
    pub hours: f32,
    /// 0.0..1.0 of a turn.
    pub minutes: f32,
    /// 0.0..1.0 of a turn.
    pub seconds: f32,
}

impl Hands {
    /// The hand positions for a local time, given the second's progress
    /// through its minute.
    pub fn at(at: &Zoned, second_fraction: f32) -> Hands {
        use chrono::Timelike;

        let seconds = at.second() as f32 + second_fraction.clamp(0.0, 1.0);
        let minutes = at.minute() as f32 + seconds / 60.0;
        let hours = (at.hour() % 12) as f32 + minutes / 60.0;

        Hands {
            hours: hours / 12.0,
            minutes: minutes / 60.0,
            seconds: seconds / 60.0,
        }
    }
}

/// The measures of a face, every one of them a fraction of its radius.
///
/// A clock face is a set of proportions, and describing it as proportions is
/// what keeps it a clock face at every size: nothing here is an absolute
/// measurement, so the same drawing code produces the same *clock* at 120px and
/// at 400px, with no stretching and no hand that quietly becomes a paddle.
#[derive(Clone, Copy, Debug)]
struct Face {
    /// The distance from the centre to the rim.
    radius: f32,
    /// The nominal line weight, the unit every other measure is read against.
    weight: f32,
}

impl Face {
    fn new(size: f32) -> Face {
        // Below about a hundred pixels a hairline is the difference between a
        // marker and a smudge, so the weight needs a floor as much as a ceiling.
        let weight = (size * 0.030).clamp(1.4, 5.0);
        Face {
            radius: size * 0.5,
            weight,
        }
    }

    /// The radius of the pivot cap, in fractions of the face radius.
    ///
    /// Measured against the radius and not against the line weight, because it
    /// has to cover the *hands* — and the hands are drawn as fractions of the
    /// face. A cap sized from the weight would be right at one size and lost
    /// under the roots at another.
    const CAP: f32 = 0.062;
    /// The radius of the dot in the middle of the cap, in radii.
    const CENTRE: f32 = 0.024;
}

/// A hand's silhouette, in fractions of the face radius.
///
/// A hand is not a stroke. A stroke is one width from end to end, which gives a
/// clock hand the shape of a drinking straw: as thick at the tip, where it needs
/// to look fine, as at the pivot, where it needs to look solid. A real hand is
/// *drawn* — it leaves the centre at one width, tapers to another at the tip,
/// and carries a tail of its own behind it — and that is what this describes, so
/// the taper is part of the outline rather than something a stroke width has to
/// approximate.
#[derive(Clone, Copy, Debug)]
struct Hand {
    /// How far the tail reaches behind the centre, in radii.
    tail: f32,
    /// How far the tip reaches ahead of the centre, in radii.
    reach: f32,
    /// Half the width of the tail, where it is cut off behind the centre.
    tail_half: f32,
    /// Half the width where the hand leaves the centre.
    root_half: f32,
    /// Half the width at the end of the taper.
    tip_half: f32,
    /// The round on the end of the tip, in radii.
    tip_round: f32,
}

impl Hand {
    /// The hour hand: short and broad, and unmistakably the slowest.
    ///
    /// Roughly thirteen times as long as it is wide at the pivot, which is the
    /// proportion a real hand has. The width matters more than it looks: a hand
    /// twice this thick reads as a wedge, and two wedges meeting at the centre
    /// read as a blot rather than as a clock.
    const HOUR: Hand = Hand {
        tail: 0.115,
        reach: 0.55,
        tail_half: 0.030,
        root_half: 0.042,
        tip_half: 0.019,
        tip_round: 0.019,
    };

    /// The minute hand: longer and finer, reaching almost to the track.
    const MINUTE: Hand = Hand {
        tail: 0.135,
        reach: 0.77,
        tail_half: 0.024,
        root_half: 0.031,
        tip_half: 0.013,
        tip_round: 0.013,
    };

    /// The second hand: a needle with a counterweight, not a third baton.
    ///
    /// A second hand earns its colour because it is the only one saying
    /// something the others do not: that the clock is running. It stays a
    /// hairline so that claim stays legible, and it widens into a counterweight
    /// behind the centre so that it reads as one object — a needle with a
    /// weight on it — rather than as a line with a disc floating near it.
    const SECOND: Hand = Hand {
        tail: 0.19,
        reach: 0.83,
        tail_half: 0.044,
        root_half: 0.014,
        tip_half: 0.005,
        tip_round: 0.0,
    };

    /// A point on this hand, in the hand's own frame turned to `angle`.
    ///
    /// `along` runs from the tail, through the centre, to the tip; `across` is
    /// measured to the hand's own right. Rotating the *corners* rather than a
    /// bounding box is what keeps the pivot on the exact centre of the face at
    /// every angle, and it is why the silhouette at three o'clock is the
    /// silhouette at twelve turned by ninety degrees and not a different shape.
    fn point(self, centre: Point, angle: f32, radius: f32, along: f32, across: f32) -> Point {
        let (sin, cos) = (angle.sin(), angle.cos());
        Point::new(
            centre.x + (along * sin + across * cos) * radius,
            centre.y + (across * sin - along * cos) * radius,
        )
    }

    /// The outline of this hand, pointing up from `centre` at `angle`.
    ///
    /// Six corners, walked in one direction: out along the left of the tail, out
    /// along the left of the body, round the tip, back down the right. The tail
    /// and the body taper independently, which is what lets one shape be a
    /// stubby baton in one role and a needle with a counterweight in another.
    fn path(self, centre: Point, angle: f32, radius: f32) -> Path {
        let at = |along: f32, across: f32| self.point(centre, angle, radius, along, across);
        let shoulder = self.reach - self.tip_round;

        Path::new(|path| {
            path.move_to(at(-self.tail, -self.tail_half));
            path.line_to(at(0.0, -self.root_half));
            path.line_to(at(shoulder, -self.tip_half));
            if self.tip_round > 0.0 {
                // Out to each side of the round's widest point and across them,
                // so the round belongs to the outline rather than being a disc
                // laid on top of it.
                path.line_to(at(self.reach, -self.tip_round));
                path.line_to(at(self.reach, self.tip_round));
            } else {
                path.line_to(at(self.reach, 0.0));
            }
            path.line_to(at(shoulder, self.tip_half));
            path.line_to(at(0.0, self.root_half));
            path.line_to(at(-self.tail, self.tail_half));
            path.close();
        })
    }
}

/// Where a tick on the track runs, in fractions of the face radius.
///
/// The whole track lives in the outer tenth of the face. That is what leaves
/// room for the hands to be long enough to read as hands and to stop short of
/// the marks, rather than finishing in the middle of them.
struct Tick {
    /// The inner end of the tick.
    inner: f32,
    /// The outer end of the tick.
    outer: f32,
    /// The line weight, as a multiple of the face's nominal weight.
    weight: f32,
}

impl Tick {
    /// A minute: hairline, short, and quiet enough to be read past.
    const MINUTE: Tick = Tick {
        inner: 0.906,
        outer: 0.944,
        weight: 0.30,
    };
    /// An hour: twice the weight of a minute, reaching further in.
    const HOUR: Tick = Tick {
        inner: 0.878,
        outer: 0.952,
        weight: 0.66,
    };
    /// A quarter: the strongest mark on the face, so the eye can take its
    /// bearings in a single glance.
    const QUARTER: Tick = Tick {
        inner: 0.868,
        outer: 0.958,
        weight: 0.92,
    };
}

impl canvas::Program<()> for Dial {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::widget::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        // Centred on the widget from its own origin rather than from a square
        // inscribed in it, so the pivot stays the middle of the face even if a
        // layout ever hands the canvas a rectangle that is not square.
        let size = bounds.width.min(bounds.height);
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let face = Face::new(size);
        let radius = face.radius;
        let weight = face.weight;

        let mut frame = Frame::new(renderer, bounds.size());

        // The face, then a rim that lifts it off whatever is behind it.
        frame.fill(
            &Path::circle(center, radius),
            Fill::from(self.palette.dial_face),
        );
        frame.stroke(
            &Path::circle(center, radius * 0.972),
            Stroke::default()
                .with_color(self.palette.dial_track)
                .with_width(weight * 0.55),
        );

        if self.markers {
            self.draw_track(&mut frame, center, radius, weight);
        }

        let (at, subsecond) = self.now();
        let hands = Hands::at(&at, subsecond);
        let second_ink = self.hand.unwrap_or(self.palette.dial_hand_alt);

        // Hour, then minute, then the second hand over both, then the pivot
        // over everything: the order a movement is assembled in, and the order
        // that keeps the cap from looking like a hole punched through the
        // second hand.
        frame.fill(
            &Hand::HOUR.path(center, hands.hours * TAU, radius),
            Fill::from(self.palette.dial_hand),
        );
        frame.fill(
            &Hand::MINUTE.path(center, hands.minutes * TAU, radius),
            Fill::from(self.palette.dial_hand),
        );
        frame.fill(
            &Hand::SECOND.path(center, hands.seconds * TAU, radius),
            Fill::from(second_ink),
        );

        // The pivot. Two concentric discs — an outer in the hand colour and an
        // inner in the face colour — is how a real centre is finished, and it
        // is what stops three hands meeting at a bare point from looking like an
        // accident. The outer one is wider than the hands' roots, which is what
        // a cap is for: the roots end *under* it.
        frame.fill(
            &Path::circle(center, radius * Face::CAP),
            Fill::from(self.palette.dial_hand),
        );
        frame.fill(
            &Path::circle(center, radius * Face::CENTRE),
            Fill::from(self.palette.dial_face),
        );

        vec![frame.into_geometry()]
    }
}

impl Dial {
    /// Draws the tick track: sixty minutes, twelve hours, four quarters.
    ///
    /// Each group is a single path and a single draw. Sixty separate strokes is
    /// a different frame budget from three, and nothing about a clock face
    /// needs sixty.
    fn draw_track(&self, frame: &mut Frame, center: Point, radius: f32, weight: f32) {
        let group = |turn: f32, tick: &Tick| {
            let angle = turn * TAU;
            let (sin, cos) = (angle.sin(), angle.cos());
            (sin, cos, tick.inner * radius, tick.outer * radius)
        };

        let track = |ticks: Vec<(f32, f32, f32, f32)>| {
            Path::new(|path| {
                for (sin, cos, inner, outer) in ticks {
                    path.move_to(Point::new(center.x + sin * inner, center.y - cos * inner));
                    path.line_to(Point::new(center.x + sin * outer, center.y - cos * outer));
                }
            })
        };

        // The minute tick that would sit under an hour mark is left out, and so
        // is the hour mark under a quarter: each group is drawn over the one
        // below it, and stacking three passes of the same colour on the same
        // few pixels buys a smudge where a single clean stroke is what reads.
        let minutes = (0..60)
            .map(|index| index as f32 / 60.0)
            .filter(|turn| (turn * 12.0).fract() > 0.01)
            .map(|turn| group(turn, &Tick::MINUTE))
            .collect();
        let hours = (1..12)
            .filter(|hour| hour % 3 != 0)
            .map(|hour| group(hour as f32 / 12.0, &Tick::HOUR));
        let quarters = (0..4).map(|index| group(index as f32 / 4.0, &Tick::QUARTER));

        frame.stroke(
            &track(minutes),
            Stroke::default()
                .with_color(self.palette.dial_track)
                .with_width(weight * Tick::MINUTE.weight)
                .with_line_cap(LineCap::Round),
        );
        frame.stroke(
            &track(hours.collect()),
            Stroke::default()
                .with_color(self.palette.dial_marker)
                .with_width(weight * Tick::HOUR.weight)
                .with_line_cap(LineCap::Round),
        );
        frame.stroke(
            &track(quarters.collect()),
            Stroke::default()
                .with_color(self.palette.dial_marker)
                .with_width(weight * Tick::QUARTER.weight)
                .with_line_cap(LineCap::Round),
        );
    }
}

/// A bar that fills once per minute, under the hero clock.
///
/// It is the only decoration on the clock page, and it earns its place: it
/// shows the current second continuously rather than jumping, which is what
/// makes a digital clock feel alive without moving the numbers themselves.
pub struct Sweep {
    palette: Palette,
    fraction: f32,
    height: f32,
}

/// The bar's thickness, in logical pixels.
///
/// Four pixels is a deliberate choice: at three, the rounded ends leave so
/// little of the middle that a fraction below about a quarter renders as a
/// scatter of dots rather than a bar.
pub const SWEEP_HEIGHT: f32 = 4.0;

impl Sweep {
    /// A sweep showing `fraction` of a minute.
    pub fn new(palette: Palette, fraction: f32) -> Sweep {
        Sweep {
            palette,
            fraction: fraction.clamp(0.0, 1.0),
            height: SWEEP_HEIGHT,
        }
    }

    /// Sets the thickness.
    pub fn thickness(mut self, height: f32) -> Sweep {
        self.height = height;
        self
    }

    /// The rendered widget.
    pub fn view<M: 'static>(self) -> Element<'static, M> {
        let height = self.height;
        let canvas: Element<'static, ()> =
            Canvas::new(self).width(Length::Fill).height(height).into();
        canvas.map(|()| unreachable!("a sweep never sends a message"))
    }
}

/// A length, imported late to keep the module's imports tidy.
use iced::Length;

impl canvas::Program<()> for Sweep {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::widget::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        // The bar is drawn at its own thickness, anchored to the top-left of
        // the frame's space, so it cannot be squeezed by a layout that offered
        // less than it asked for.
        let height = self.height.min(bounds.size().height);
        let corner = (height / 2.0).min(height);
        let width = bounds.size().width.max(height);

        frame.fill(
            &Path::new(|path| {
                path.rounded_rectangle(
                    Point::new(0.0, 0.0),
                    Size::new(width, height),
                    corner.into(),
                );
            }),
            Fill::from(self.palette.track),
        );

        let filled = (width * self.fraction).max(height * 0.4);
        if self.fraction > 0.0 {
            frame.fill(
                &Path::new(|path| {
                    path.rounded_rectangle(
                        Point::new(0.0, 0.0),
                        Size::new(filled, height),
                        corner.into(),
                    );
                }),
                Fill::from(self.palette.accent),
            );
        }

        vec![frame.into_geometry()]
    }
}

/// A slow pulse, used to keep a running stopwatch from looking frozen between
/// its centisecond updates.
pub fn breath(elapsed: Duration, period: Duration) -> f32 {
    ease::pulse(elapsed, period)
}

/// The bounding size a drawing occupies, for tests.
pub fn box_size(size: f32) -> Size {
    Size::new(size, size)
}

/// A vector from the centre of a box, for tests.
pub fn offset(centre: Point, x: f32, y: f32) -> Vector {
    Vector::new(x - centre.x, y - centre.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tz;
    use chrono_tz::Tz;

    fn at(zone: Tz, y: i32, mo: u32, d: u32, h: u32, mi: u32) -> Zoned {
        tz::resolve_wall_time_parts(zone, y, mo, d, h, mi).expect("resolvable")
    }

    #[test]
    fn the_hands_start_at_twelve() {
        let hands = Hands::at(&at(Tz::UTC, 2026, 6, 15, 0, 0), 0.0);
        assert!((hands.hours - 0.0).abs() < 1.0e-6);
        assert!((hands.minutes - 0.0).abs() < 1.0e-6);
        assert!((hands.seconds - 0.0).abs() < 1.0e-6);
    }

    /// The centre a hand is built around, at a radius of 100.
    const CENTRE: Point = Point { x: 50.0, y: 50.0 };
    const RADIUS: f32 = 100.0;

    const EVERY_HAND: [(&str, Hand); 3] = [
        ("hour", Hand::HOUR),
        ("minute", Hand::MINUTE),
        ("second", Hand::SECOND),
    ];

    #[test]
    fn a_hand_points_straight_up_at_the_top_of_the_turn() {
        for (name, hand) in EVERY_HAND {
            let tip = hand.point(CENTRE, 0.0, RADIUS, 1.0, 0.0);
            assert!(
                (tip.x - CENTRE.x).abs() < 1.0e-4 && (tip.y - (CENTRE.y - RADIUS)).abs() < 1.0e-4,
                "the {name} hand's tip at zero was {tip:?}, not straight up"
            );
        }
    }

    #[test]
    fn a_hand_turns_clockwise_so_three_oclock_is_to_the_right() {
        for (name, hand) in EVERY_HAND {
            for (turn, expected) in [
                (
                    0.25_f32,
                    Point {
                        x: CENTRE.x + RADIUS,
                        y: CENTRE.y,
                    },
                ),
                (
                    0.5,
                    Point {
                        x: CENTRE.x,
                        y: CENTRE.y + RADIUS,
                    },
                ),
                (
                    0.75,
                    Point {
                        x: CENTRE.x - RADIUS,
                        y: CENTRE.y,
                    },
                ),
                (
                    1.0,
                    Point {
                        x: CENTRE.x,
                        y: CENTRE.y - RADIUS,
                    },
                ),
            ] {
                let tip = hand.point(CENTRE, turn * TAU, RADIUS, 1.0, 0.0);
                assert!(
                    (tip.x - expected.x).abs() < 1.0e-3 && (tip.y - expected.y).abs() < 1.0e-3,
                    "the {name} hand at {turn} of a turn was {tip:?}, not {expected:?}"
                );
            }
        }
    }

    #[test]
    fn every_hand_pivots_on_the_exact_centre_at_every_angle() {
        // The regression this guards is subtle and ugly: rotating a *bounding
        // box* instead of the corners puts the pivot off the centre by a few
        // pixels, which on a dark face looks like the hands are glued slightly
        // off-centre and nobody can say why.
        for (name, hand) in EVERY_HAND {
            for step in 0..360 {
                let angle = step as f32 * std::f32::consts::PI / 180.0;
                let pivot = hand.point(CENTRE, angle, RADIUS, 0.0, 0.0);
                assert!(
                    (pivot.x - CENTRE.x).abs() < 1.0e-3 && (pivot.y - CENTRE.y).abs() < 1.0e-3,
                    "the {name} hand's pivot at {step}° was {pivot:?}, not the centre"
                );
            }
        }
    }

    #[test]
    fn a_hand_is_symmetric_about_its_own_axis() {
        for (name, hand) in EVERY_HAND {
            for step in 0..360 {
                let angle = step as f32 * std::f32::consts::PI / 180.0;
                for (along, across) in [
                    (-hand.tail, hand.tail_half),
                    (0.0, hand.root_half),
                    (hand.reach, hand.tip_round.max(hand.tip_half)),
                ] {
                    let left = hand.point(CENTRE, angle, RADIUS, along, across);
                    let right = hand.point(CENTRE, angle, RADIUS, along, -across);
                    let axis = hand.point(CENTRE, angle, RADIUS, along, 0.0);
                    // The pair straddles the axis, and the axis is the midpoint
                    // of the pair: that is what "symmetric" means here.
                    let midpoint = Point {
                        x: (left.x + right.x) / 2.0,
                        y: (left.y + right.y) / 2.0,
                    };
                    assert!(
                        (midpoint.x - axis.x).abs() < 1.0e-3
                            && (midpoint.y - axis.y).abs() < 1.0e-3,
                        "the {name} hand at {step}° is lopsided at along={along}"
                    );
                }
            }
        }
    }

    /// The ordering the face is read by, checked where it cannot change.
    ///
    /// These are all constants, so these are really *compile-time* invariants of
    /// the geometry: the face is read by finding the shortest hand first, and if
    /// the ordering ever inverted the clock would be unreadable rather than ugly.
    /// A test that runs after the build has found out cannot.
    const _: () = {
        assert!(Hand::HOUR.reach < Hand::MINUTE.reach);
        assert!(Hand::MINUTE.reach < Hand::SECOND.reach);
        // And each is thicker than the one it outranks.
        assert!(Hand::HOUR.root_half > Hand::MINUTE.root_half);
        assert!(Hand::MINUTE.root_half > Hand::SECOND.root_half);

        // A hand whose tip finishes in the middle of the markers reads as a
        // drawing error, so every hand stops in the clear band below them, and
        // the nearest mark is the innermost one.
        assert!(Hand::SECOND.reach <= Tick::QUARTER.inner);
        assert!(Tick::QUARTER.inner <= Tick::HOUR.inner);
        assert!(Tick::HOUR.inner <= Tick::MINUTE.inner);
        assert!(Hand::HOUR.tail < 0.35);
        assert!(Hand::MINUTE.tail < 0.35);
        assert!(Hand::SECOND.tail < 0.35);

        // Every tick sits between the same two rings, or the face looks like it
        // has two different tracks on it.
        const { assert!(Tick::MINUTE.outer <= 0.97 && Tick::MINUTE.inner >= 0.85) };
        const { assert!(Tick::HOUR.outer <= 0.97 && Tick::HOUR.inner >= 0.85) };
        const { assert!(Tick::QUARTER.outer <= 0.97 && Tick::QUARTER.inner >= 0.85) };
        const { assert!(Tick::MINUTE.outer > Tick::MINUTE.inner) };
        const { assert!(Tick::HOUR.outer > Tick::HOUR.inner) };
        const { assert!(Tick::QUARTER.outer > Tick::QUARTER.inner) };

        // The cap is what makes three hands meeting at a point look assembled
        // rather than collided, which only works if it is wider than the roots
        // it hides. Both are in radii, so this holds at every size — which a cap
        // sized from the line weight would not.
        assert!(Face::CAP >= Hand::HOUR.root_half);
        assert!(Face::CAP >= Hand::MINUTE.root_half);
        assert!(Face::CAP >= Hand::SECOND.root_half);
        assert!(Face::CENTRE < Face::CAP);

        // A counterweight wider than the neck it grows out of, and further back
        // than the cap, is what makes the second hand read as a needle with a
        // weight on it rather than as a line with a disc floating beside it.
        assert!(Hand::SECOND.tail_half > Hand::SECOND.root_half * 2.0);
        assert!(Hand::SECOND.tail * 2.0 > Face::CAP);
        // The hour and minute hands are the opposite: a plain stub behind the
        // cap, not a weight.
        assert!(Hand::HOUR.tail_half < Hand::HOUR.root_half);
        assert!(Hand::MINUTE.tail_half < Hand::MINUTE.root_half);

        // A hand that widens towards its tip is a cone, not a hand.
        assert!(Hand::HOUR.tip_half <= Hand::HOUR.root_half);
        assert!(Hand::MINUTE.tip_half <= Hand::MINUTE.root_half);
        assert!(Hand::SECOND.tip_half <= Hand::SECOND.root_half);
        // And a round longer than the hand is a cap that hangs off the end.
        const { assert!(Hand::HOUR.tip_round <= Hand::HOUR.reach) };
        const { assert!(Hand::MINUTE.tip_round <= Hand::MINUTE.reach) };
        const { assert!(Hand::SECOND.tip_round <= Hand::SECOND.reach) };
    };

    #[test]
    fn the_hands_do_not_fold_back_on_themselves() {
        // Each corner has to move along the hand monotonically, or the outline
        // crosses itself and the fill comes out with a notch in it. This one
        // cannot be a constant, because the corners are reached through a
        // closure that borrows the hand.
        for (name, hand) in EVERY_HAND {
            let alongs = [-hand.tail, 0.0, hand.reach - hand.tip_round, hand.reach];
            for window in alongs.windows(2) {
                assert!(
                    window[1] >= window[0],
                    "the {name} hand's outline folds back at {}",
                    window[0]
                );
            }
            // And the round does not start before the taper does.
            assert!(
                hand.tip_round <= hand.reach / 2.0,
                "the {name} hand's tip round is longer than half of it"
            );
        }
    }

    #[test]
    fn a_dial_is_the_same_clock_at_every_size() {
        // Proportions, not measurements: the ratio between two dimensions of
        // the face must not change with its size, or a large dial is a small
        // one with something stretched.
        for size in [96.0_f32, 160.0, 220.0, 480.0] {
            let face = Face::new(size);
            assert!((face.radius - size * 0.5).abs() < 1.0e-4);
            assert!(face.weight >= 1.4 && face.weight <= 5.0);
        }
        // Every hand's reach is the same *fraction* of the face at both ends of
        // the size range, which is the property a pixel measurement cannot see
        // and a hand drawn in absolute pixels would quietly break.
        for size in [96.0_f32, 480.0] {
            let face = Face::new(size);
            for (name, hand) in EVERY_HAND {
                for step in 0..12 {
                    let angle = step as f32 * std::f32::consts::PI / 6.0;
                    let tip = hand.point(Point::new(0.0, 0.0), angle, face.radius, hand.reach, 0.0);
                    let reach = (tip.x * tip.x + tip.y * tip.y).sqrt();
                    assert!(
                        (reach - face.radius * hand.reach).abs() < 1.0e-2,
                        "at {size}px the {name} hand reached {reach} rather than {}",
                        face.radius * hand.reach
                    );
                }
            }
        }
    }

    #[test]
    fn the_hands_complete_one_turn_every_twelve_hours() {
        for (hour, expected) in [
            (0, 0.0),
            (1, 1.0 / 12.0),
            (3, 0.25),
            (6, 0.5),
            (9, 0.75),
            (11, 11.0 / 12.0),
        ] {
            let hands = Hands::at(&at(Tz::UTC, 2026, 6, 15, hour, 0), 0.0);
            assert!(
                (hands.hours - expected).abs() < 1.0e-5,
                "at {hour}:00 the hour hand was at {}",
                hands.hours
            );
        }
    }

    #[test]
    fn the_hands_move_continuously_not_in_steps() {
        // The minute hand must be between the markers while the minute passes.
        let early = Hands::at(&at(Tz::UTC, 2026, 6, 15, 0, 0), 0.0);
        let later = Hands::at(&at(Tz::UTC, 2026, 6, 15, 0, 0), 0.5);
        assert!(later.minutes > early.minutes);
        assert!(
            later.minutes < 0.01,
            "half a second is half a minute's 1/120"
        );

        // And so must the hour hand, or the face looks wrong in motion.
        let half_past = Hands::at(&at(Tz::UTC, 2026, 6, 15, 1, 30), 0.0);
        assert!(
            (half_past.hours - (1.5 / 12.0)).abs() < 1.0e-5,
            "half an hour should move the hour hand by half a mark"
        );
    }

    #[test]
    fn the_second_fraction_uses_sub_second_precision() {
        let hands = Hands::at(&at(Tz::UTC, 2026, 6, 15, 0, 0), 0.25);
        assert!((hands.seconds - 0.25 / 60.0).abs() < 1.0e-6);
        // It nudges the other hands too, which is what a real second hand does.
        assert!(hands.minutes > 0.0);
    }

    #[test]
    fn the_sweep_is_thick_enough_to_read_as_a_bar() {
        // At three pixels a fraction of a quarter left so little solid middle
        // that it rendered as a scatter of dots.
        const { assert!(SWEEP_HEIGHT >= 4.0) };
    }

    #[test]
    fn a_dial_still_draws_live_and_frozen() {
        let clock = FrameClock::new();
        // A live face reads the wall clock; a frozen one shows another zone's
        // time, which is what the world clock's small faces are for.
        let _: Element<'static, ()> = Dial::new(Palette::DARK, at(Tz::UTC, 2026, 6, 15, 9, 41))
            .size(180.0)
            .view();
        let _: Element<'static, ()> =
            Dial::frozen(Palette::DARK, at(Tz::UTC, 2026, 6, 15, 9, 41), 0.5)
                .size(180.0)
                .bare()
                .hand(iced::Color::WHITE)
                .view();

        let _: Element<'static, ()> =
            Ring::new(Palette::LIGHT, 0.42, clock, Interaction::default())
                .size(200.0)
                .thickness(12.0)
                .tint(iced::Color::from_rgb(0.9, 0.2, 0.3))
                .glow(0.5)
                .view();

        let _: Element<'static, ()> = Sweep::new(Palette::LIGHT, 0.5).thickness(6.0).view();
    }

    #[test]
    fn progress_is_clamped() {
        let clock = FrameClock::new();
        let interaction = Interaction::default();
        let over = Ring::new(Palette::LIGHT, 4.0, clock.clone(), interaction.clone());
        assert_eq!(over.progress, 1.0);
        let under = Ring::new(Palette::LIGHT, -2.0, clock, interaction);
        assert_eq!(under.progress, 0.0);
    }

    #[test]
    fn arcs_stay_on_their_radius_and_are_bounded() {
        let center = Point::new(50.0, 50.0);
        for sweep in [0.1f32, 1.0, std::f32::consts::PI, std::f32::consts::TAU] {
            let points = arc_points(center, 30.0, 0.0, sweep);
            assert!(points.len() >= 2 && points.len() <= 97);
            for point in points {
                let dx = point.x - center.x;
                let dy = point.y - center.y;
                assert!(((dx * dx + dy * dy).sqrt() - 30.0).abs() < 0.01);
            }
        }
    }

    #[test]
    fn a_zero_progress_ring_still_shows_its_track() {
        // The track has to be there before the timer starts, or the control
        // would appear to grow into existence.
        let ring = Ring::new(
            Palette::LIGHT,
            0.0,
            FrameClock::new(),
            Interaction::default(),
        );
        assert_eq!(ring.progress, 0.0);
        assert!(ring.thickness > 0.0);
    }

    #[test]
    fn helpers_behave() {
        assert_eq!(box_size(12.0), Size::new(12.0, 12.0));
        assert_eq!(
            offset(Point::new(10.0, 10.0), 4.0, 6.0),
            Vector::new(-6.0, -4.0)
        );
        assert!((breath(Duration::ZERO, Duration::from_secs(4)) - 0.0).abs() < 1.0e-3);
    }
}
