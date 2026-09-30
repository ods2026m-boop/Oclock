//! The application icon.
//!
//! Drawn rather than shipped. A clock face is a handful of circles and two
//! hands, so an image file would be a binary blob to keep in step with the
//! palette for no benefit: generating the icon means it always uses the
//! current accent colour, at whatever size the platform asks for, with
//! antialiasing that is correct rather than whatever was baked into a PNG.

use crate::design::palette::Palette;

/// The size the icon is rasterised at. Large enough to look sharp in a
/// window title bar on a high-density display.
pub const ICON_SIZE: usize = 128;

/// The icon as RGBA bytes, ready for [`iced::window::icon::from_rgba`].
///
/// Supersampled 3× and then averaged down, which is what gives the edges
/// their smoothness without pulling in a graphics library.
pub fn pixels() -> Vec<u8> {
    raster(ICON_SIZE, Palette::LIGHT)
}

/// The icon as RGBA bytes, in the dark palette, for a dark window manager.
pub fn pixels_dark() -> Vec<u8> {
    raster(ICON_SIZE, Palette::DARK)
}

/// Rasterises the icon at `size` pixels square.
pub fn raster(size: usize, palette: Palette) -> Vec<u8> {
    const SUPERSAMPLE: usize = 3;

    let samples = size * SUPERSAMPLE;
    let mut accumulated = vec![0.0f32; size * size * 4];

    for sy in 0..samples {
        for sx in 0..samples {
            let x = (sx as f32 + 0.5) / SUPERSAMPLE as f32;
            let y = (sy as f32 + 0.5) / SUPERSAMPLE as f32;
            let Some(colour) = sample(x, y, size, palette) else {
                continue;
            };

            let index = (sy / SUPERSAMPLE) * size * 4 + (sx / SUPERSAMPLE) * 4;
            accumulated[index] += colour.r;
            accumulated[index + 1] += colour.g;
            accumulated[index + 2] += colour.b;
            accumulated[index + 3] += colour.a;
        }
    }

    // Average the subsamples, then scale to 8-bit: a channel value is a
    // proportion, and truncating one straight to a byte would turn pure white
    // into 1.
    let per_sample = (SUPERSAMPLE * SUPERSAMPLE) as f32;
    accumulated
        .into_iter()
        .map(|value| (value / per_sample).clamp(0.0, 1.0) * 255.0)
        .map(|value| value.round() as u8)
        .collect()
}

/// The colour at a point, or `None` where the icon is transparent.
///
/// The face is a filled disc in the accent, the hands are the text colour, and
/// everything outside is transparent — which is what a window title bar wants.
fn sample(x: f32, y: f32, size: usize, palette: Palette) -> Option<iced::Color> {
    let centre = size as f32 / 2.0;
    let dx = x - centre;
    let dy = y - centre;
    let radius = centre - size as f32 * 0.04;
    let distance = (dx * dx + dy * dy).sqrt();

    if distance > radius {
        return None;
    }

    // The rim, then the hands: two hands at the classic ten-past-ten, which
    // reads as a clock face at 16 px.
    let rim = size as f32 * 0.055;
    if distance > radius - rim {
        return Some(
            palette
                .text_muted
                .scale_alpha(edge_alpha(distance, radius, rim)),
        );
    }

    let hands = [
        ((-35.0f32).to_radians(), size as f32 * 0.30),
        ((38.0f32).to_radians(), size as f32 * 0.24),
    ];
    for (angle, length) in hands {
        let ex = angle.sin() * length;
        let ey = -angle.cos() * length;
        // Distance from the point to the hand's segment.
        let projection = (dx * ex + dy * ey) / (length * length);
        let closest = projection.clamp(0.0, 1.0);
        let gap = ((dx - ex * closest).powi(2) + (dy - ey * closest).powi(2)).sqrt();
        if gap < size as f32 * 0.035 {
            return Some(palette.text);
        }
    }

    Some(palette.accent)
}

/// Fades the outermost pixel of the rim, so the circle's edge is not a stair.
fn edge_alpha(distance: f32, radius: f32, rim: f32) -> f32 {
    let edge = radius - distance;
    (edge / rim.max(1.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::palette::Palette;

    #[test]
    fn the_icon_is_the_right_shape_for_the_window_api() {
        let pixels = pixels();
        assert_eq!(pixels.len(), ICON_SIZE * ICON_SIZE * 4);
    }

    #[test]
    fn the_corners_are_transparent_and_the_centre_is_not() {
        let pixels = pixels();
        let size = ICON_SIZE;

        let corner = 0;
        assert_eq!(pixels[corner * 4 + 3], 0, "a corner must be transparent");

        let centre = (size / 2 * size + size / 2) * 4;
        assert!(pixels[centre + 3] > 200, "the middle must be solid");
    }

    #[test]
    fn the_icon_is_mostly_the_accent() {
        let pixels = pixels();
        let opaque: usize = pixels
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 128)
            .count();
        let total = ICON_SIZE * ICON_SIZE;
        // A disc covers about a quarter of its bounding box; a little more is
        // expected because of the corners of the antialiasing.
        assert!(
            opaque * 3 > total,
            "only {opaque} of {total} pixels are opaque"
        );
    }

    #[test]
    fn the_icon_follows_the_palette() {
        let light = raster(64, Palette::LIGHT);
        let dark = raster(64, Palette::DARK);
        assert_ne!(light, dark, "the icon must follow the appearance");

        // A point on the face but clear of both hands: the accent, which
        // differs between the two appearances.
        let face = (20 * 64 + 32) * 4;
        assert_eq!(light[face + 3], 255, "the face is opaque");
        assert_ne!(
            light[face + 1],
            dark[face + 1],
            "the face is drawn in the appearance's accent"
        );
    }

    #[test]
    fn a_tiny_icon_still_renders() {
        let pixels = raster(16, Palette::LIGHT);
        assert_eq!(pixels.len(), 16 * 16 * 4);
    }

    #[test]
    fn sampling_outside_the_face_is_transparent() {
        assert!(sample(-1.0, -1.0, 128, Palette::LIGHT).is_none());
        assert!(sample(64.0, 64.0, 128, Palette::LIGHT).is_some());
    }
}
