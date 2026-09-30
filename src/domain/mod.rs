//! Feature logic.
//!
//! Everything here is deterministic business logic: models, state machines and
//! formatting. It knows nothing about Iced, about files on disk, or about
//! notifications, which is what lets the whole feature set be tested without
//! a window, and what keeps the UI layer thin enough to read.

pub mod alarm;
pub mod clock;
pub mod fmt;
pub mod sound;
pub mod stopwatch;
pub mod timer;
pub mod world;
