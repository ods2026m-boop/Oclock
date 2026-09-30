//! OClock — the ODS-os clock application.
//!
//! This file is deliberately thin: it opens a window, hands the window to
//! [`oclock::app::OClock`], and gets out of the way. The application's state,
//! its rules and its interface all live in the library, where they can be
//! tested without a display.

use iced::window;

use oclock::app::{icon, OClock};
use oclock::design::tokens::layout;

/// The window, sized and decorated once.
fn window_settings() -> window::Settings {
    let (width, height) = layout::WINDOW;
    let (min_width, min_height) = layout::WINDOW_MIN;

    let size = icon::ICON_SIZE as u32;
    let window_icon = window::icon::from_rgba(icon::pixels(), size, size).ok();

    window::Settings {
        size: iced::Size::new(width as f32, height as f32),
        min_size: Some(iced::Size::new(min_width as f32, min_height as f32)),
        icon: window_icon,
        ..window::Settings::default()
    }
}

/// The view, as the runtime's view trait requires it.
///
/// That trait is `Fn(&'a State) -> impl Into<Element<'a, ..>>`, which a
/// function item with a *concrete* element lifetime cannot satisfy: the
/// element has to be free to borrow the state. Declaring the element's
/// lifetime as the state's own lets the `'static` window be borrowed down,
/// which is the right thing — the widget tree holds no reference to the
/// application, it is built from copies.
fn view<'a>(app: &'a OClock) -> iced::Element<'a, oclock::app::message::Message> {
    app.view()
}

fn main() -> iced::Result {
    // The application is built before the window opens, so a first launch that
    // finds a broken configuration reports it inside the window rather than
    // vanishing into a console nobody is looking at.
    let app = OClock::start();

    iced::application(oclock::APP_NAME, OClock::update, view)
        .subscription(OClock::subscription)
        .theme(OClock::theme)
        // Every mark OClock draws is a canvas path — the dial hands, the icons,
        // the progress rings, the navigation marker — and a path rendered into
        // a single-sample target has stair-stepped edges. Four samples is what
        // makes them look drawn rather than chopped, at any window size and any
        // scale factor.
        .antialiasing(true)
        .window(window_settings())
        .centered()
        .run_with(|| (app, iced::Task::none()))
}
