//! **OClock** — the ODS-os clock application.
//!
//! OClock is a first-party ODS-os system application: a clock, a world clock,
//! alarms, timers and a stopwatch in one window.
//!
//! # Layout
//!
//! The crate is layered so that each concern can be reasoned about, tested and
//! replaced on its own:
//!
//! | Layer | Modules | Knows about |
//! |---|---|---|
//! | [`design`] | tokens, palette, motion | nothing but Iced's primitives |
//! | [`core`] | clocks, timezones, ids | the operating system |
//! | [`domain`] | alarms, timers, stopwatch, world, clock | only itself |
//! | [`services`] | storage, notifications, audio, tz data | the desktop |
//! | [`ui`] | components and pages | `domain`, `services`, `design` |
//! | [`app`] | state, messages, wiring | everything |
//!
//! `domain` never imports `ui`, and `services` never imports `domain`'s UI
//! types. Business logic is therefore testable without a window, and the UI
//! layer is a projection of state rather than a second source of truth.

pub mod app;
pub mod core;
pub mod design;
pub mod domain;
pub mod services;
pub mod ui;

/// The application name, as shown in the window and the about box.
pub const APP_NAME: &str = "OClock";

/// The organisation the application ships with.
pub const APP_VENDOR: &str = "ODS-os";

/// Directory name used for the configuration and state folders.
pub const APP_SLUG: &str = "oclock";
