//! The five pages, one per module.
//!
//! A page is a function from the application and a palette to an element. It
//! reads state and sends messages; it changes nothing itself. That is what lets
//! the shell compose them freely, including drawing two at once during a
//! page transition.

pub mod alarms;
pub mod clock;
pub mod stopwatch;
pub mod timer;
pub mod world;

/// The tallest a dialog may be, as a fraction of the window.
///
/// A dialog is measured against the window rather than a constant, so it stays
/// inside a short one and scrolls instead of running off the bottom.
pub fn dialog_limit(app: &crate::app::OClock) -> f32 {
    (app.view.window.1 * 0.78).clamp(320.0, 900.0)
}

/// The widest a dialog may be, so it never exceeds the window.
pub fn dialog_width(app: &crate::app::OClock, preferred: f32) -> f32 {
    (app.view.window.0 - 80.0).min(preferred)
}
