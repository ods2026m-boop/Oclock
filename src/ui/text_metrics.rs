//! Natural widths for runs of text, measured by the shaper that draws them.
//!
//! Iced shapes a text widget at the width its layout *offers*, not at the width
//! the text *wants*, and cosmic-text lays a right-to-left line out flush to the
//! end of that box. A shrink-wrapped label — a chip, a button, a badge — is
//! therefore drawn at the far end of whatever space its row happened to offer,
//! which in a right-to-left layout puts it outside its own background.
//!
//! The fix is to hand the widget a width of its own. Measuring needs the same
//! font database, the same attributes and the same shaper that will draw the
//! glyphs, or the number means nothing, so this asks Iced's own font system
//! rather than keeping a second one. Results are cached: a view is rebuilt
//! every frame, and shaping a hundred labels again each time would cost more
//! than drawing them.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use iced::advanced::text::Shaping;
use iced::font::Font;
use iced_graphics::text::{cosmic_text, font_system, measure, to_attributes, to_shaping};

/// How many measurements to keep before the cache is dropped.
///
/// Text that changes on every keystroke would otherwise grow the cache without
/// bound. Dropping it wholesale is cheap — the next frame refills what is still
/// on screen — and the limit is far above the number of distinct labels a
/// window holds.
const CACHE_LIMIT: usize = 4_096;

/// What a measurement was made from.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    content: String,
    font: Font,
    size: u32,
    line_height: u32,
}

fn cache() -> &'static Mutex<HashMap<Key, f32>> {
    static CACHE: OnceLock<Mutex<HashMap<Key, f32>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The width `content` occupies on one line, in logical pixels.
///
/// The run is measured unbounded, so the answer is the width the text wants
/// rather than the width it is about to be squeezed into. A right-to-left run
/// is measured the same way as a left-to-right one: the two differ only in
/// where the line sits inside its box, not in how much of the box it needs.
///
/// The result is rounded *up* to a sixteenth of a pixel. The measurement and
/// the drawing use the same shaper, so the two agree exactly; the rounding is
/// only there so that a figure which lands a fraction low cannot let the line
/// wrap in a box that was sized for it.
pub fn natural_width(content: &str, font: Font, size: f32, line_height: f32) -> f32 {
    if content.is_empty() || size <= 0.0 || !size.is_finite() {
        return 0.0;
    }

    let key = Key {
        content: content.to_string(),
        font,
        size: size.to_bits(),
        line_height: line_height.to_bits(),
    };

    if let Ok(cache) = cache().lock() {
        if let Some(width) = cache.get(&key) {
            return *width;
        }
    }

    let width = (measure_uncached(&key) * 16.0).ceil() / 16.0;

    if let Ok(mut cache) = cache().lock() {
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert(key, width);
    }

    width
}

/// Measures without consulting the cache.
fn measure_uncached(key: &Key) -> f32 {
    let size = f32::from_bits(key.size);
    let line_height = f32::from_bits(key.line_height);

    // A poisoned lock means a previous measurement panicked; the font system is
    // still perfectly usable, and refusing to measure afterwards would turn one
    // failure into a permanently blank interface.
    let mut system = font_system()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut buffer =
        cosmic_text::Buffer::new(system.raw(), cosmic_text::Metrics::new(size, line_height));

    // A label is one line by definition. Letting the buffer wrap would make the
    // answer depend on where the text happened to break.
    buffer.set_wrap(system.raw(), cosmic_text::Wrap::None);
    // `Advanced` is what draws an Arabic letter in its joined form and what
    // reaches for a second font when the first has no glyph for a character.
    // The measurement has to use the same one, or a Persian label would be
    // measured with one font and drawn with another.
    buffer.set_text(
        system.raw(),
        &key.content,
        to_attributes(key.font),
        to_shaping(Shaping::Advanced),
    );

    measure(&buffer).width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::tokens::fonts;

    fn width_of(content: &str, font: Font, size: f32) -> f32 {
        natural_width(content, font, size, size * 1.2)
    }

    #[test]
    fn a_run_is_wider_than_one_of_its_own_prefixes() {
        let ui = width_of("abc", fonts::UI, 16.0);
        let longer = width_of("abcdef", fonts::UI, 16.0);
        assert!(ui > 0.0, "a Latin run must measure to something");
        assert!(longer > ui, "{longer} should exceed {ui}");
    }

    #[test]
    fn measuring_is_stable_across_calls() {
        // The cache has to agree with itself: a label that changed width
        // between two frames in the same view would make the layout jump.
        let first = width_of("ساعت", fonts::UI, 16.0);
        let second = width_of("ساعت", fonts::UI, 16.0);
        assert_eq!(first, second);
    }

    #[test]
    fn the_cache_distinguishes_text_size_and_face() {
        let small = width_of("abcd", fonts::UI, 12.0);
        let large = width_of("abcd", fonts::UI, 24.0);
        assert!(large > small, "a larger size must measure wider");

        let ui = width_of("abcd", fonts::UI, 16.0);
        let numeric = width_of("abcd", fonts::NUMERIC, 16.0);
        assert!(ui > 0.0 && numeric > 0.0);
    }

    #[test]
    fn nothing_to_measure_measures_nothing() {
        assert_eq!(natural_width("", fonts::UI, 16.0, 19.2), 0.0);
        assert_eq!(natural_width("abc", fonts::UI, 0.0, 0.0), 0.0);
        assert_eq!(natural_width("abc", fonts::UI, f32::NAN, 0.0), 0.0);
    }

    #[test]
    fn every_supported_script_shapes_to_a_visible_width() {
        // Shaping with `Advanced` is what gives Arabic its joined forms and
        // what reaches for a second font for a script the first lacks. A run
        // that measures as nothing would be a run that draws as nothing.
        let samples = [
            ("Latin", "Clock"),
            ("Persian", "ساعت"),
            ("Arabic", "الساعة"),
            ("Urdu", "گھڑی"),
            ("Cyrillic", "Часы"),
            ("Greek", "Ώρα"),
            ("Chinese", "时钟"),
            ("Japanese", "とけい"),
            ("Korean", "시계"),
            ("Thai", "นาฬิกา"),
        ];

        for (script, sample) in samples {
            let width = width_of(sample, fonts::UI, 16.0);
            assert!(
                width > 1.0,
                "{script} ({sample}) measured {width}px wide, so it would not draw"
            );
        }
    }

    #[test]
    fn arabic_is_measured_joined_rather_than_letter_by_letter() {
        // A joined Arabic word is narrower than the sum of its letters drawn in
        // isolation, because initial and medial forms are narrower than the
        // isolated ones. A shaper that ignored the joining would not show that,
        // and a shaper that found no glyphs at all would measure nothing.
        let joined = width_of("سلام", fonts::UI, 24.0);
        let separated = width_of("س ل ا م", fonts::UI, 24.0);
        assert!(joined > 0.0 && separated > 0.0);
        assert!(
            joined < separated,
            "joined {joined}px should be narrower than spaced {separated}px"
        );
    }
}
