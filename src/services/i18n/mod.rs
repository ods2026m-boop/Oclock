//! Localization: every word OClock says, in every language it speaks.
//!
//! # Where this sits
//!
//! `services::i18n` is a **service**, the same kind of thing as
//! [`crate::services::notify`] or [`crate::services::storage`]: it is a
//! resource the application owns, it is constructed once and shared, and it
//! knows nothing about widgets. That placement is what keeps the layering
//! intact:
//!
//! * [`crate::domain`] does not depend on this module. `Alarm`, `Timer`,
//!   `Stopwatch` and `WorldClocks` are pure data and pure rules, and they
//!   still are — a language is a presentation concern, and the domain has none.
//! * [`crate::ui`] depends on this module, because a view has to say something
//!   and what it says is the whole point of a view.
//! * [`crate::app`] owns the active [`Catalog`] and hands it down, so switching
//!   language is one assignment rather than a search for every string.
//!
//! # What the domain gives up
//!
//! Presentation used to live beside the data it described: `Sound::name()`,
//! `Repeat::label()`, `Weekdays::label()`, `SortMode::label()` and the rest
//! each returned English. Those are now [`Catalog`] methods, which is what
//! makes "no user-visible text outside the catalogue" a property of the type
//! system rather than a convention. The values themselves did not change: a
//! `Sound` is still a `Sound`, an alarm still fires when it should, and the
//! timing code is untouched.
//!
//! # Text that is deliberately not translated
//!
//! * **Proper nouns a user typed.** Alarm names, timer names and city names are
//!   the user's own words, stored as they were entered.
//! * **Machine identifiers.** Timezone names, notification action ids and
//!   storage keys are contracts with the outside world, not prose.
//! * **The product name.** `OClock` is a name.

pub mod catalog;
pub mod compose;
pub mod language;
pub mod locales;

pub use catalog::{Arg, Catalog, Locale, PluralForm, PluralText, Text};
pub use language::{Direction, Language};

/// Builds a translated string with substituted arguments.
///
/// Shorthand for [`Catalog::format`], which is what almost every caller wants:
///
/// ```
/// use oclock::services::i18n::{Catalog, Language, Text};
/// use oclock::t;
///
/// let catalog = Catalog::new(Language::English);
/// assert_eq!(t!(catalog, Text::ToastStarted, "Tea"), "Tea started");
/// ```
#[macro_export]
macro_rules! t {
    ($catalog:expr, $key:expr) => {
        $crate::services::i18n::Catalog::format($catalog, $key, &[])
    };
    ($catalog:expr, $key:expr, $($arg:expr),+ $(,)?) => {{
        let args = [
            $($crate::services::i18n::Arg::from($arg)),+
        ];
        $crate::services::i18n::Catalog::format($catalog, $key, &args)
    }};
}
