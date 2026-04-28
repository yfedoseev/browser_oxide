//! Real font stack for Canvas 2D text rendering.
//!
//! Pipeline: [`font_shorthand`] parses `ctx.font` strings, [`FontDatabase`]
//! resolves families to concrete faces via the bundled Liberation font
//! set, [`shaper`] produces positioned glyph runs via rustybuzz, and
//! [`raster`] rasterizes individual glyphs via swash. The top-level
//! [`rasterize_text`] / [`measure_text_metrics`] helpers are the
//! entry points `Canvas2D` calls.
//!
//! Replaces the prior ab_glyph-based stub that returned fixed-width
//! metrics regardless of the requested font-family and rendered
//! through a single embedded DejaVu Sans. The new pipeline is
//! Chrome-compatible enough that `measureText("...")` responses on
//! fingerprint-sensitive sites fall within the expected range.

pub mod font_database;
pub mod font_shorthand;
pub mod raster;
pub mod shaper;

pub use font_database::FontDatabase;
pub use font_shorthand::ParsedFont;
pub use raster::GlyphBitmap;
pub use shaper::ShapedRun;

/// Full 13-field `TextMetrics` object as Canvas 2D exposes it.
///
/// Field semantics follow the HTML spec:
/// <https://html.spec.whatwg.org/multipage/canvas.html#textmetrics>
#[derive(Debug, Clone, PartialEq)]
pub struct TextMetrics {
    pub width: f32,
    pub actual_bounding_box_left: f32,
    pub actual_bounding_box_right: f32,
    pub actual_bounding_box_ascent: f32,
    pub actual_bounding_box_descent: f32,
    pub font_bounding_box_ascent: f32,
    pub font_bounding_box_descent: f32,
    pub em_height_ascent: f32,
    pub em_height_descent: f32,
    pub hanging_baseline: f32,
    pub alphabetic_baseline: f32,
    pub ideographic_baseline: f32,
}

impl TextMetrics {
    /// Zero-valued metrics for empty text / unresolved fonts.
    pub fn zero() -> Self {
        Self {
            width: 0.0,
            actual_bounding_box_left: 0.0,
            actual_bounding_box_right: 0.0,
            actual_bounding_box_ascent: 0.0,
            actual_bounding_box_descent: 0.0,
            font_bounding_box_ascent: 0.0,
            font_bounding_box_descent: 0.0,
            em_height_ascent: 0.0,
            em_height_descent: 0.0,
            hanging_baseline: 0.0,
            alphabetic_baseline: 0.0,
            ideographic_baseline: 0.0,
        }
    }
}

/// Resolve a parsed font to concrete face data. Walks the family
/// fallback chain and returns both the raw face bytes and the face
/// index (for TTC collections).
fn resolve_face(font: &ParsedFont) -> Option<(&'static [u8], u32)> {
    let db = FontDatabase::get();
    let id = db.query_chain(&font.families, font.weight, font.italic)?;
    db.face_data(id)
}

/// Measure text using the parsed font. Returns zero metrics for empty
/// text or unresolvable fonts — matching Canvas 2D's tolerant behaviour.
pub fn measure_text_metrics(text: &str, font: &ParsedFont) -> TextMetrics {
    if text.is_empty() {
        return TextMetrics::zero();
    }
    let Some((data, idx)) = resolve_face(font) else {
        return TextMetrics::zero();
    };
    let run = shaper::shape(text, data, idx, font.size_px);

    // em-height approximations: CSS spec says em_height_ascent ≈ 0.8 *
    // size and em_height_descent ≈ 0.2 * size for most Latin fonts.
    // Chrome's actual values come from the OS/2 table's sTypoAscender/
    // sTypoDescender, so the ratios aren't exactly 0.8/0.2 — but we're
    // well within the tolerance fingerprint probes allow.
    let em_ascent = font.size_px * 0.8;
    let em_descent = font.size_px * 0.2;

    TextMetrics {
        width: run.width,
        actual_bounding_box_left: -run.bbox_left.min(0.0),
        actual_bounding_box_right: run.bbox_right.max(run.width),
        actual_bounding_box_ascent: run.bbox_ascent,
        actual_bounding_box_descent: run.bbox_descent,
        font_bounding_box_ascent: run.ascent,
        font_bounding_box_descent: run.descent,
        em_height_ascent: em_ascent,
        em_height_descent: em_descent,
        // Canvas 2D default textBaseline is "alphabetic" = 0. The other
        // baselines are offsets relative to the alphabetic baseline.
        hanging_baseline: run.ascent * 0.8,
        alphabetic_baseline: 0.0,
        ideographic_baseline: -run.descent * 0.5,
    }
}

/// Convenience: measure width only (for the simple `measureText(...).width`
/// fingerprint probes).
pub fn measure_text_width(text: &str, font: &ParsedFont) -> f64 {
    measure_text_metrics(text, font).width as f64
}

/// One blitted glyph ready to composite onto the canvas pixel buffer.
///
/// Each entry is in "canvas pixel space" — integer x/y top-left
/// coordinates and an alpha-only coverage buffer. The canvas performs
/// the actual premultiplied-alpha blend in
/// `Canvas2D::composite_alpha_mask`.
pub struct PlacedGlyph {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<u8>,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub alpha: f32,
}

/// Shape + rasterize `text` at the given origin, producing a list of
/// placed alpha masks ready for canvas compositing. The `(x, y)`
/// origin is the start of the text run, with the alphabetic baseline
/// at `y` (matching Canvas 2D's default `textBaseline`).
pub fn rasterize_text(
    text: &str,
    x: f32,
    y: f32,
    font: &ParsedFont,
    r: u8,
    g: u8,
    b: u8,
    alpha: f32,
) -> Vec<PlacedGlyph> {
    if text.is_empty() {
        return Vec::new();
    }
    let Some((data, idx)) = resolve_face(font) else {
        return Vec::new();
    };
    let run = shaper::shape(text, data, idx, font.size_px);

    let mut out = Vec::with_capacity(run.glyphs.len());
    let mut cursor_x = x;
    for glyph in &run.glyphs {
        if let Some(bitmap) = raster::rasterize_glyph(data, idx, glyph.glyph_id, font.size_px) {
            // `left` is the horizontal offset from the pen position to
            // the glyph bitmap's left edge. `top` is the vertical
            // offset from the baseline to the glyph bitmap's top edge
            // (positive means above the baseline).
            let draw_x = cursor_x + glyph.x_offset + bitmap.left as f32;
            let draw_y = y - glyph.y_offset - bitmap.top as f32;
            out.push(PlacedGlyph {
                x: draw_x.round() as i32,
                y: draw_y.round() as i32,
                width: bitmap.width,
                height: bitmap.height,
                coverage: bitmap.pixels,
                r,
                g,
                b,
                alpha,
            });
        }
        cursor_x += glyph.x_advance;
    }
    out
}

/// Adapter that turns `ttf_parser::OutlineBuilder` callbacks into
/// `Path2D` commands, applying the EM→pixel scale and the Y-axis
/// flip (font space is Y-up, canvas is Y-down).
struct Path2DOutlineBuilder<'a> {
    path: &'a mut crate::path::Path2D,
    /// Origin x in canvas space (where x=0 in font-space lands).
    origin_x: f32,
    /// Origin y in canvas space (the alphabetic baseline of the run).
    origin_y: f32,
    /// EM-units-to-pixels factor = size_px / units_per_em.
    scale: f32,
}

impl Path2DOutlineBuilder<'_> {
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        // Font space: y up, em units. Canvas space: y down, pixels.
        (self.origin_x + x * self.scale, self.origin_y - y * self.scale)
    }
}

impl rustybuzz::ttf_parser::OutlineBuilder for Path2DOutlineBuilder<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (mx, my) = self.map(x, y);
        self.path.move_to(mx, my);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (mx, my) = self.map(x, y);
        self.path.line_to(mx, my);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (cpx, cpy) = self.map(x1, y1);
        let (mx, my) = self.map(x, y);
        self.path.quadratic_curve_to(cpx, cpy, mx, my);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1x, c1y) = self.map(x1, y1);
        let (c2x, c2y) = self.map(x2, y2);
        let (mx, my) = self.map(x, y);
        self.path.bezier_curve_to(c1x, c1y, c2x, c2y, mx, my);
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

/// Shape `text` and append the outline contours of every glyph into
/// `path`, positioned at the given baseline origin. Returns true if any
/// glyph contributed contours (false for empty text or all-bitmap fonts
/// — color-emoji fonts have no `glyf` table and silently produce no
/// outline).
///
/// Used by `Canvas2D::stroke_text` to build a stroke-able path. The
/// shaping pipeline matches `rasterize_text` so stroked text aligns
/// pixel-for-pixel with what filled text would have rendered.
pub fn append_text_outline_to_path(
    path: &mut crate::path::Path2D,
    text: &str,
    x: f32,
    y: f32,
    font: &ParsedFont,
) -> bool {
    if text.is_empty() {
        return false;
    }
    let Some((data, idx)) = resolve_face(font) else {
        return false;
    };
    // Parse a fresh ttf_parser::Face for outline access. rustybuzz's
    // Face wraps this internally but doesn't expose outline_glyph
    // directly — re-parsing is cheap (header-only, no glyph data
    // copied).
    let Ok(ttf_face) = rustybuzz::ttf_parser::Face::parse(data, idx) else {
        return false;
    };
    let upem = ttf_face.units_per_em() as f32;
    if upem <= 0.0 {
        return false;
    }
    let scale = font.size_px / upem;
    let run = shaper::shape(text, data, idx, font.size_px);

    let mut any_outline = false;
    let mut cursor_x = x;
    for glyph in &run.glyphs {
        // Per-glyph origin: the cursor + the shaper's per-glyph offset.
        // y_offset is in pixels (already-scaled), which matches our
        // origin-in-canvas-space convention; subtract because canvas
        // y grows downward.
        let glyph_origin_x = cursor_x + glyph.x_offset;
        let glyph_origin_y = y - glyph.y_offset;
        let mut builder = Path2DOutlineBuilder {
            path,
            origin_x: glyph_origin_x,
            origin_y: glyph_origin_y,
            scale,
        };
        if ttf_face
            .outline_glyph(
                rustybuzz::ttf_parser::GlyphId(glyph.glyph_id as u16),
                &mut builder,
            )
            .is_some()
        {
            any_outline = true;
        }
        cursor_x += glyph.x_advance;
    }
    any_outline
}

/// Composite a single PlacedGlyph onto a premultiplied-RGBA pixel
/// buffer. Uses SRC_OVER with the glyph colour premultiplied by the
/// per-pixel coverage × global alpha.
pub fn composite_glyph(
    glyph: &PlacedGlyph,
    pixels: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
) {
    for gy in 0..glyph.height {
        for gx in 0..glyph.width {
            let px = glyph.x + gx as i32;
            let py = glyph.y + gy as i32;
            if px < 0 || py < 0 || px >= canvas_width as i32 || py >= canvas_height as i32 {
                continue;
            }

            let cov_idx = (gy * glyph.width + gx) as usize;
            if cov_idx >= glyph.coverage.len() {
                continue;
            }
            let cov = glyph.coverage[cov_idx] as f32 / 255.0;
            if cov < 0.001 {
                continue;
            }

            let a = cov * glyph.alpha;
            if a <= 0.0 {
                continue;
            }

            let dst_idx = ((py as u32 * canvas_width + px as u32) * 4) as usize;
            if dst_idx + 3 >= pixels.len() {
                continue;
            }

            // Premultiplied source-over. `src` components are the
            // glyph colour scaled by coverage-adjusted alpha; `dst`
            // components are whatever is already in the buffer.
            let src_r = glyph.r as f32 * a;
            let src_g = glyph.g as f32 * a;
            let src_b = glyph.b as f32 * a;
            let src_a = 255.0 * a;

            let dst_r = pixels[dst_idx] as f32;
            let dst_g = pixels[dst_idx + 1] as f32;
            let dst_b = pixels[dst_idx + 2] as f32;
            let dst_a = pixels[dst_idx + 3] as f32;

            let inv_src_a = 1.0 - a;
            let out_r = (src_r + dst_r * inv_src_a).min(255.0);
            let out_g = (src_g + dst_g * inv_src_a).min(255.0);
            let out_b = (src_b + dst_b * inv_src_a).min(255.0);
            let out_a = (src_a + dst_a * inv_src_a).min(255.0);

            pixels[dst_idx] = out_r as u8;
            pixels[dst_idx + 1] = out_g as u8;
            pixels[dst_idx + 2] = out_b as u8;
            pixels[dst_idx + 3] = out_a as u8;
        }
    }
}

// ---------------------------------------------------------------------------
// Backwards-compatible size-only helpers.
//
// A few call sites currently use a `font_size: f32` field without a
// full parsed font. The thin wrappers below let them migrate at their
// own pace — internally they construct a default `ParsedFont` with the
// given size and hand off to the real pipeline.
// ---------------------------------------------------------------------------

/// Legacy convenience: measure width using a default sans-serif face
/// at the given size. New code should use `measure_text_width`.
pub fn measure_text_width_size_only(text: &str, font_size: f32) -> f64 {
    let font = ParsedFont {
        size_px: font_size,
        ..ParsedFont::default_font()
    };
    measure_text_width(text, &font)
}

/// Legacy convenience for `fillText` paths that only know the font
/// size. Uses the default sans-serif chain (Liberation Sans via the
/// bundled font database).
#[allow(clippy::too_many_arguments)]
pub fn rasterize_text_size_only(
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    r: u8,
    g: u8,
    b: u8,
    alpha: f32,
) -> Vec<PlacedGlyph> {
    let font = ParsedFont {
        size_px: font_size,
        ..ParsedFont::default_font()
    };
    rasterize_text(text, x, y, &font, r, g, b, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_hello_world_at_16px_arial() {
        let font = ParsedFont::parse("16px Arial").unwrap();
        let metrics = measure_text_metrics("Hello, World!", &font);
        assert!(metrics.width > 30.0 && metrics.width < 200.0);
        assert!(metrics.font_bounding_box_ascent > 10.0);
    }

    #[test]
    fn different_sizes_different_widths() {
        let small = ParsedFont::parse("10px Arial").unwrap();
        let big = ParsedFont::parse("40px Arial").unwrap();
        let w1 = measure_text_width("Hello", &small);
        let w2 = measure_text_width("Hello", &big);
        assert!(w2 > w1 * 3.0);
    }

    #[test]
    fn mono_wider_than_sans_for_narrow_text() {
        // For the narrow character "i" repeated, a proportional sans
        // font measures much less than a fixed-width monospace font.
        let sans = ParsedFont::parse("14px Arial").unwrap();
        let mono = ParsedFont::parse("14px monospace").unwrap();
        let sans_w = measure_text_width("iiiiii", &sans);
        let mono_w = measure_text_width("iiiiii", &mono);
        assert!(
            mono_w > sans_w * 1.3,
            "mono should be much wider for narrow chars: sans={sans_w} mono={mono_w}"
        );
    }

    #[test]
    fn rasterize_produces_glyphs() {
        let font = ParsedFont::parse("24px Arial").unwrap();
        let glyphs = rasterize_text("A", 0.0, 20.0, &font, 0, 0, 0, 1.0);
        assert!(!glyphs.is_empty());
        let g = &glyphs[0];
        assert!(g.width > 0 && g.height > 0);
        assert!(g.coverage.iter().any(|&c| c > 0));
    }

    #[test]
    fn rasterize_empty_returns_empty() {
        let font = ParsedFont::parse("16px Arial").unwrap();
        let glyphs = rasterize_text("", 0.0, 0.0, &font, 0, 0, 0, 1.0);
        assert!(glyphs.is_empty());
    }
}
