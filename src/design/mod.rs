//! The design system: tokens, colour, and motion.
//!
//! Components are written against these abstractions rather than against
//! numbers, so a change here propagates through the whole application.

pub mod motion;
pub mod palette;
pub mod theme;
pub mod tokens;

pub use motion::{FrameClock, Presence, Spec, TrackedHandle, Tween};
pub use palette::Palette;
pub use theme::{Appearance, ThemeMode, ThemeState};
pub use tokens::Easing;
