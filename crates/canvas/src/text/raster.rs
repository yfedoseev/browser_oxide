//! Glyph rasterization via `swash`.
//!
//! Given a font face + glyph ID + pixel size, produces an 8-bit alpha
//! coverage mask. This is what the canvas then composites onto its
//! pixel buffer for `fillText`.
//!
//! We use swash's Alpha renderer with hinting enabled. Hinting
//! matters for small sizes (where pixel grid fitting substantially
//! changes bitmap width) and makes our output closer to a real
//! Chrome+FreeType session than unhinted outlines would.

use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;
use swash::{FontRef, GlyphId};

/// A rasterized glyph coverage mask.
#[derive(Debug, Clone)]
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    /// Pixel offset from the glyph's pen position (Canvas 2D "fillText"
    /// origin). `left` is in pixels to the right of the pen; `top` is
    /// in pixels above the baseline. Both can be negative for glyphs
    /// that extend to the left of the origin or below the baseline.
    pub left: i32,
    pub top: i32,
    /// Alpha-only pixel data, row-major, 1 byte per pixel.
    pub pixels: Vec<u8>,
}

/// Rasterize a single glyph. Returns `None` if the face is invalid, the
/// glyph has no outline (e.g. whitespace), or swash refuses to render.
pub fn rasterize_glyph(
    face_data: &[u8],
    face_index: u32,
    glyph_id: u32,
    size_px: f32,
) -> Option<GlyphBitmap> {
    let font = FontRef::from_index(face_data, face_index as usize)?;
    let mut context = ScaleContext::new();
    let mut scaler = context.builder(font).size(size_px).hint(true).build();

    let image = Render::new(&[
        // Try outline first — that's what text rendering wants for
        // vector fonts. Colour / bitmap sources are there as fallbacks
        // for emoji-like glyphs, not for core Latin.
        Source::Outline,
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
    ])
    .format(Format::Alpha)
    .render(&mut scaler, GlyphId::from(glyph_id as u16))?;

    Some(GlyphBitmap {
        width: image.placement.width,
        height: image.placement.height,
        left: image.placement.left,
        top: image.placement.top,
        pixels: image.data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::font_database::FontDatabase;
    use crate::text::shaper;

    fn shape_single(ch: &str, size_px: f32) -> (shaper::Glyph, &'static [u8], u32) {
        let db = FontDatabase::get();
        let id = db.query("Arial", 400, false).unwrap();
        let (data, idx) = db.face_data(id).unwrap();
        let run = shaper::shape(ch, data, idx, size_px);
        assert!(
            !run.glyphs.is_empty(),
            "shaper produced no glyphs for {ch:?}"
        );
        (run.glyphs[0].clone(), data, idx)
    }

    #[test]
    fn rasterizes_latin_glyph() {
        let (glyph, data, idx) = shape_single("A", 24.0);
        let bitmap = rasterize_glyph(data, idx, glyph.glyph_id, 24.0).expect("A should rasterize");
        assert!(bitmap.width > 0 && bitmap.height > 0);
        // 24px Latin capital should be 12-30 px tall.
        assert!(
            (10..=35).contains(&(bitmap.height as i32)),
            "24px A height {} out of range",
            bitmap.height
        );
        // Must have some non-zero coverage.
        let nonzero = bitmap.pixels.iter().filter(|&&p| p > 0).count();
        assert!(nonzero > 0, "rasterized glyph has all-zero coverage");
    }

    #[test]
    fn space_has_no_outline() {
        let (glyph, data, idx) = shape_single(" ", 24.0);
        // Space glyph exists (has advance) but has no ink. Accept either
        // "no bitmap returned" or "bitmap with zero non-zero pixels".
        let bitmap = rasterize_glyph(data, idx, glyph.glyph_id, 24.0);
        match bitmap {
            None => { /* fine */ }
            Some(b) => {
                let nonzero = b.pixels.iter().filter(|&&p| p > 0).count();
                assert_eq!(nonzero, 0, "space glyph has ink");
            }
        }
    }

    #[test]
    fn bitmap_height_scales_with_size() {
        let (small_glyph, data, idx) = shape_single("H", 14.0);
        let small = rasterize_glyph(data, idx, small_glyph.glyph_id, 14.0).unwrap();
        let (big_glyph, _, _) = shape_single("H", 48.0);
        let big = rasterize_glyph(data, idx, big_glyph.glyph_id, 48.0).unwrap();
        assert!(
            big.height > small.height * 2,
            "48/14 should be ~3.4x; got {} / {}",
            big.height,
            small.height
        );
    }
}
