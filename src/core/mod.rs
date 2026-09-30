//! Platform and timing foundations shared by every feature.
//!
//! Nothing in here knows about Iced or about OClock's features; it is the
//! layer that knows about clocks and the host operating system.

pub mod ids;
pub mod time;
pub mod tz;

pub use ids::Id;
pub use time::{Discontinuity, SystemTimeline, Timeline, Watchdog};
pub use tz::Zoned;
