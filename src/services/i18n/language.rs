//! The languages OClock speaks, and how to choose between them.
//!
//! A language is identified by its BCP-47 primary subtag, which is also the
//! key it is stored under, so a configuration file written in one build is read
//! by the next. Nothing in this module knows what any word means: it knows only
//! which languages exist, which way they are written, and what the desktop
//! asked for.

use serde::{Deserialize, Serialize};

/// Which way a language is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    /// Left to right: English, French, Chinese, Korean, Thai, German, Spanish,
    /// Indonesian, Russian.
    #[default]
    LeftToRight,
    /// Right to left: Persian, Arabic, Urdu.
    RightToLeft,
}

impl Direction {
    /// True for the languages written from the right.
    pub fn is_rtl(self) -> bool {
        matches!(self, Direction::RightToLeft)
    }

    /// The fraction a horizontal run of text is anchored from its leading edge.
    ///
    /// Iced lays every row out left to right and offers no mirrored container,
    /// so a right-to-left interface is built by pinning its leading edge to the
    /// *right*: a 0% start becomes 100% and vice versa. This is the one place
    /// that conversion lives.
    pub fn start(self) -> f32 {
        match self {
            Direction::LeftToRight => 0.0,
            Direction::RightToLeft => 1.0,
        }
    }

    /// The mirror of [`Self::start`], for a run that is anchored at the far end.
    pub fn end(self) -> f32 {
        match self {
            Direction::LeftToRight => 1.0,
            Direction::RightToLeft => 0.0,
        }
    }

    /// Flips a `0.0..=1.0` fraction across the horizontal axis.
    pub fn mirror(self, fraction: f32) -> f32 {
        match self {
            Direction::LeftToRight => fraction,
            Direction::RightToLeft => 1.0 - fraction,
        }
    }

    /// The sign that turns a leading-edge offset into a physical one.
    ///
    /// `+1` for left to right, `-1` for right to left, so a gap written as
    /// "next to the start" lands on the correct side either way.
    pub fn sign(self) -> f32 {
        match self {
            Direction::LeftToRight => 1.0,
            Direction::RightToLeft => -1.0,
        }
    }

    /// The alignment a run of text hangs from.
    ///
    /// Iced's `Start` is the *left* edge whatever the script, so naming the
    /// leading edge for the language — rather than for the toolkit — is what
    /// puts a right-to-left sentence against the right margin instead of the
    /// left one.
    pub fn align_x(self) -> iced::Alignment {
        match self {
            Direction::LeftToRight => iced::Alignment::Start,
            Direction::RightToLeft => iced::Alignment::End,
        }
    }
}

/// A language OClock can be used in.
///
/// The twelve languages here are the complete set: the catalogue is compiled
/// in, so a new one is a variant, a table and a line of detection rather than
/// a file to ship.
///
/// Stored by its BCP-47 subtag, so the preference in a configuration file is
/// `"fa"` rather than a variant name that would have to change if it were ever
/// renamed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// English. The reference language, and the fallback for everything else.
    #[default]
    #[serde(rename = "en")]
    English,
    /// Persian — فارسی
    #[serde(rename = "fa")]
    Persian,
    /// French — français
    #[serde(rename = "fr")]
    French,
    /// Chinese — 中文
    #[serde(rename = "zh")]
    Chinese,
    /// Korean — 한국어
    #[serde(rename = "ko")]
    Korean,
    /// Thai — ไทย
    #[serde(rename = "th")]
    Thai,
    /// German — Deutsch
    #[serde(rename = "de")]
    German,
    /// Spanish — español
    #[serde(rename = "es")]
    Spanish,
    /// Indonesian — Bahasa Indonesia
    #[serde(rename = "id")]
    Indonesian,
    /// Urdu — اردو
    #[serde(rename = "ur")]
    Urdu,
    /// Russian — Русский
    #[serde(rename = "ru")]
    Russian,
    /// Arabic — العربية
    #[serde(rename = "ar")]
    Arabic,
}

impl Language {
    /// Every language, in the order the picker lists them: English first as
    /// the reference, then English-named ones alphabetically, then the rest
    /// alphabetically by endonym.
    pub const ALL: [Language; 12] = [
        Language::English,
        Language::French,
        Language::German,
        Language::Indonesian,
        Language::Spanish,
        Language::Arabic,
        Language::Chinese,
        Language::Korean,
        Language::Persian,
        Language::Russian,
        Language::Thai,
        Language::Urdu,
    ];

    /// The BCP-47 primary subtag, which is also the stored form.
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Persian => "fa",
            Language::French => "fr",
            Language::Chinese => "zh",
            Language::Korean => "ko",
            Language::Thai => "th",
            Language::German => "de",
            Language::Spanish => "es",
            Language::Indonesian => "id",
            Language::Urdu => "ur",
            Language::Russian => "ru",
            Language::Arabic => "ar",
        }
    }

    /// The language's name in that language.
    ///
    /// Deliberately never translated: a person looking for their own language
    /// in a list does not recognise it written in the language they are
    /// currently reading, which is the one that is not theirs.
    pub fn endonym(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Persian => "فارسی",
            Language::French => "Français",
            Language::Chinese => "中文",
            Language::Korean => "한국어",
            Language::Thai => "ไทย",
            Language::German => "Deutsch",
            Language::Spanish => "Español",
            Language::Indonesian => "Bahasa Indonesia",
            Language::Urdu => "اردو",
            Language::Russian => "Русский",
            Language::Arabic => "العربية",
        }
    }

    /// The language's name in English, for logs and for a subtitle in the
    /// picker when the endonym is not a Latin script.
    pub fn exonym(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Persian => "Persian",
            Language::French => "French",
            Language::Chinese => "Chinese",
            Language::Korean => "Korean",
            Language::Thai => "Thai",
            Language::German => "German",
            Language::Spanish => "Spanish",
            Language::Indonesian => "Indonesian",
            Language::Urdu => "Urdu",
            Language::Russian => "Russian",
            Language::Arabic => "Arabic",
        }
    }

    /// Which way this language is written.
    pub fn direction(self) -> Direction {
        match self {
            Language::Persian | Language::Urdu | Language::Arabic => Direction::RightToLeft,
            _ => Direction::LeftToRight,
        }
    }

    /// True for the three right-to-left languages.
    pub fn is_rtl(self) -> bool {
        self.direction().is_rtl()
    }

    /// The language named by a BCP-47 tag, a POSIX locale name or a bare code.
    ///
    /// Accepts what a desktop actually puts in its environment: `fa_IR.UTF-8`,
    /// `zh_CN`, `pt-BR`, `en`. The primary subtag decides, so a region or an
    /// encoding is ignored, and an unsupported language is `None` rather than
    /// a silent English — the caller decides whether to fall back.
    pub fn from_tag(tag: &str) -> Option<Language> {
        let primary = tag
            .trim()
            .split(['-', '_', '.', '@'])
            .next()
            .filter(|part| !part.is_empty())?;

        // A handful of legacy tags still appear in `LANG` values and mean a
        // language that is present under a different tag today.
        let lowered = primary.to_ascii_lowercase();
        let primary = match lowered.as_str() {
            "in" => "id",
            other => other,
        };

        Language::ALL
            .into_iter()
            .find(|language| language.code() == primary)
    }

    /// The language the desktop asked for, or `None` if it asked for something
    /// OClock does not speak.
    ///
    /// `LC_ALL` wins over `LC_MESSAGES` wins over `LANG`, which is the
    /// precedence a POSIX shell gives them.
    pub fn from_environment() -> Option<Language> {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|variable| std::env::var(variable).ok())
            .and_then(|value| Language::from_tag(&value))
    }

    /// The language the desktop's locale asks for, falling back to English when
    /// it asks for something OClock does not speak.
    ///
    /// Reported, but deliberately **not** what a fresh installation starts in:
    /// a locale is an ambient guess made about the machine, and a first run has
    /// no user preference to honour, so it starts in
    /// [`Language::default`] instead. See [`crate::services::settings::Config::first_run`].
    /// OClock asks this nowhere on the startup path; it exists so that a caller
    /// that *wants* the desktop's guess — a settings screen's "match the
    /// system" affordance, say — does not have to write the precedence itself.
    pub fn detected() -> Language {
        Language::from_environment().unwrap_or_default()
    }

    /// The next language in the picker, for cycling with a shortcut.
    pub fn next(self) -> Language {
        let index = Language::ALL
            .iter()
            .position(|language| *language == self)
            .unwrap_or(0);
        Language::ALL[(index + 1) % Language::ALL.len()]
    }

    /// The position of this language in [`Self::ALL`].
    pub fn index(self) -> usize {
        Language::ALL
            .iter()
            .position(|language| *language == self)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_exactly_twelve_languages() {
        assert_eq!(Language::ALL.len(), 12);
    }

    #[test]
    fn the_picker_lists_every_language_exactly_once() {
        let mut seen = Language::ALL.to_vec();
        seen.sort_by_key(|language| language.index());
        assert_eq!(seen, Language::ALL, "index() must be the list order");

        for (position, language) in Language::ALL.iter().enumerate() {
            assert_eq!(language.index(), position, "{language:?} is misplaced");
        }
    }

    #[test]
    fn every_language_has_a_distinct_code_and_endonym() {
        let codes: std::collections::HashSet<&str> = Language::ALL
            .iter()
            .map(|language| language.code())
            .collect();
        let endonyms: std::collections::HashSet<&str> = Language::ALL
            .iter()
            .map(|language| language.endonym())
            .collect();
        assert_eq!(codes.len(), Language::ALL.len(), "codes must be unique");
        assert_eq!(
            endonyms.len(),
            Language::ALL.len(),
            "endonyms must be unique"
        );
    }

    #[test]
    fn a_language_round_trips_through_its_stored_form() {
        // The document says the code, not the variant's name, so a preference
        // written by one build reads in the next and reads in any tool.
        for language in Language::ALL {
            let json = serde_json::to_string(&language).expect("serializes");
            assert_eq!(json, format!("\"{}\"", language.code()));
            let back: Language = serde_json::from_str(&json).expect("parses");
            assert_eq!(back, language);
        }
    }

    #[test]
    fn an_unknown_stored_language_is_refused() {
        assert!(serde_json::from_str::<Language>("\"klingon\"").is_err());
    }

    #[test]
    fn codes_round_trip_through_the_parser() {
        for language in Language::ALL {
            assert_eq!(Language::from_tag(language.code()), Some(language));
        }
    }

    #[test]
    fn a_posix_locale_resolves_to_its_language() {
        assert_eq!(Language::from_tag("fa_IR.UTF-8"), Some(Language::Persian));
        assert_eq!(Language::from_tag("zh_CN"), Some(Language::Chinese));
        assert_eq!(Language::from_tag("pt-BR"), None, "not supported");
        assert_eq!(
            Language::from_tag("de_AT.UTF-8@euro"),
            Some(Language::German)
        );
        assert_eq!(Language::from_tag(""), None);
        assert_eq!(
            Language::from_tag("C"),
            None,
            "the C locale is not a language"
        );
        assert_eq!(Language::from_tag("POSIX"), None);
    }

    #[test]
    fn an_unsupported_language_is_refused_rather_than_guessed() {
        // The distinction matters: a caller that gets `None` can report that the
        // desktop asked for something unavailable, whereas a silent English
        // would look like the application had ignored the user.
        assert_eq!(Language::from_tag("ja_JP.UTF-8"), None);
        assert_eq!(Language::from_tag("xx"), None);
    }

    #[test]
    fn the_legacy_indonesian_tag_is_understood() {
        assert_eq!(Language::from_tag("in_ID"), Some(Language::Indonesian));
    }

    #[test]
    fn detection_without_a_preference_is_english() {
        // `detected` is a total function: a silent machine still gets a
        // language, because an application with none cannot draw its own labels.
        let detected = Language::detected();
        assert!(Language::ALL.contains(&detected));
    }

    #[test]
    fn the_three_rtl_languages_are_exactly_persian_urdu_and_arabic() {
        let rtl: Vec<Language> = Language::ALL
            .into_iter()
            .filter(|language| language.is_rtl())
            .collect();
        assert_eq!(
            rtl,
            vec![Language::Arabic, Language::Persian, Language::Urdu],
            "Persian, Arabic and Urdu are the right-to-left set"
        );
    }

    #[test]
    fn every_other_language_reads_left_to_right() {
        for language in Language::ALL {
            let expected = !matches!(
                language,
                Language::Persian | Language::Urdu | Language::Arabic
            );
            assert_eq!(
                language.is_rtl(),
                !expected,
                "{language:?} has the wrong direction"
            );
        }
    }

    #[test]
    fn direction_anchors_from_the_leading_edge() {
        assert_eq!(Direction::LeftToRight.start(), 0.0);
        assert_eq!(Direction::RightToLeft.start(), 1.0);
        assert_eq!(Direction::LeftToRight.end(), 1.0);
        assert_eq!(Direction::RightToLeft.end(), 0.0);
    }

    #[test]
    fn mirroring_is_its_own_inverse() {
        for direction in [Direction::LeftToRight, Direction::RightToLeft] {
            for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let once = direction.mirror(fraction);
                assert!((direction.mirror(once) - fraction).abs() < f32::EPSILON);
            }
        }
        assert_eq!(Direction::LeftToRight.mirror(0.25), 0.25);
        assert_eq!(Direction::RightToLeft.mirror(0.25), 0.75);
    }

    #[test]
    fn a_mirrored_sign_pushes_a_gap_to_the_correct_side() {
        assert_eq!(Direction::LeftToRight.sign(), 1.0);
        assert_eq!(Direction::RightToLeft.sign(), -1.0);
    }

    #[test]
    fn text_hangs_from_the_leading_edge() {
        // Iced's `Start` is physical, not logical: it is the left edge in every
        // language. A right-to-left run has to be told to hang from the other
        // side explicitly, which is what this does.
        assert_eq!(Direction::LeftToRight.align_x(), iced::Alignment::Start);
        assert_eq!(Direction::RightToLeft.align_x(), iced::Alignment::End);
    }

    #[test]
    fn cycling_visits_every_language_and_returns() {
        let mut language = Language::English;
        let mut seen = vec![language];
        for _ in 1..Language::ALL.len() {
            language = language.next();
            seen.push(language);
        }
        assert_eq!(seen.len(), Language::ALL.len());
        assert_eq!(language.next(), Language::English, "the cycle closes");
    }
}
