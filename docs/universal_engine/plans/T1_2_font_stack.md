# T1.2 — Real font stack: cosmic-text + fontdb + rustybuzz + swash

**Task**: #63
**Priority**: P1 — **highest ROI of the Tier-1 capability items**
**Effort**: 50-70 hours
**Dependencies**: Sprint 0 refactor must be done first (architectural
cleanliness). No dependency on T1.1.

## Goal

Replace our current stub font implementation with a real text shaping +
rasterization pipeline that produces Chrome-compatible output for
`fillText`, `measureText`, `strokeText`, and the 13-field `TextMetrics`
object. Ship with a bundled Chrome-like font set so fingerprint-sensitive
APIs (`measureText("The quick brown fox").width`, font list enumeration)
return the same values real Chrome does.

## Why this is highest priority

Per `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md`, the
adidas sensor VM calls `fillText("SomeCanvasFingerPrint.65@345876", 2,
15)` twice (at slightly offset positions). Our current font stub
returns fixed-width metrics for every character, which is a dead
giveaway on `measureText`. Font selection also affects `fillText` paint
output which flows through whichever canvas hash path the VM uses
(task #69 investigation didn't definitively identify the extraction
method — font differences could still be the signal).

Beyond adidas, font fingerprinting is one of the strongest signals in
general. FingerprintJS, CreepJS, amiunique, and every commercial
anti-bot hash `measureText` widths for ~30-100 specific strings and
compare against their Chrome reference database. Without a real font
stack, we'd need per-site calibration — which violates the
architectural principle.

## Current state

**Files**:
- `crates/canvas/src/text.rs` — current stub text rendering. Returns
  fixed-width boxes. Has the `(255.0 * a) as u8` unused variable
  warning that's been there forever.
- `crates/canvas/src/canvas2d.rs` — wraps the text operations for
  canvas context use.
- `crates/js_runtime/src/js/canvas_bootstrap.js` — JS side of
  `CanvasRenderingContext2D.fillText` and `measureText`. Routes to
  Rust ops.
- `crates/js_runtime/src/extensions/canvas_ext.rs` — op definitions.

**What's wrong with the stub**:
- Every character measured as 8px wide (or similar). Real Chrome
  varies wildly: `fillText('.')` → ~3px, `fillText('W')` → ~14px for
  16px Arial.
- `measureText` returns zeros or placeholder values for all 13 fields
  (width, actualBoundingBoxLeft, actualBoundingBoxRight,
  actualBoundingBoxAscent, actualBoundingBoxDescent,
  fontBoundingBoxAscent, fontBoundingBoxDescent, emHeightAscent,
  emHeightDescent, hangingBaseline, alphabeticBaseline,
  ideographicBaseline, width).
- Canvas `fillText` paint produces a solid-color rectangle where the
  text should be. Any site that extracts pixels via `toDataURL` sees
  obviously-wrong glyphs.
- Font-family string parsing is nominal (we read the font property
  from the context) but nothing is selected — all fonts render the
  same.

## Target architecture

```
┌───────────────────────────────────────────────────────────────┐
│                   crates/canvas/src/text.rs                    │
│                                                                 │
│ 1. Font loading (fontdb)                                        │
│    - Bundled assets in crates/canvas/assets/fonts/              │
│    - Optional host OS fonts via fontdb's system scan            │
│    - FontFamily → Face handle lookup                            │
│                                                                 │
│ 2. Text shaping (rustybuzz)                                     │
│    - Input: utf-8 string + font face + font size                │
│    - Output: glyph run with glyph IDs, x-advances, cluster map  │
│    - Handles kerning, ligatures, bidi via cosmic-text           │
│                                                                 │
│ 3. Rasterization (swash)                                        │
│    - Input: glyph ID + font face + pixel size + subpixel pos    │
│    - Output: alpha mask (u8 grid) for that glyph                │
│                                                                 │
│ 4. Composition                                                   │
│    - For each glyph in run:                                     │
│      - Advance x by glyph.x_advance                             │
│      - Blit alpha mask onto tiny_skia canvas (or skia-safe      │
│        after T1.1) at (cursor_x, cursor_y + baseline) with      │
│        fillStyle color premultiplied through the alpha mask     │
└───────────────────────────────────────────────────────────────┘
```

## Step-by-step implementation

### Step 1 — Add dependencies (30 min)

**File**: `crates/canvas/Cargo.toml`

```toml
[dependencies]
# Existing: tiny_skia = "0.11", ...
cosmic-text = "0.12"
fontdb = "0.18"
rustybuzz = "0.18"
swash = "0.1"
```

Verify licenses:
- cosmic-text: MIT
- fontdb: MIT/Apache-2.0
- rustybuzz: MIT (port of HarfBuzz, which is MIT)
- swash: Apache-2.0

All clean for our MIT/Apache-2.0 policy. Run `cargo tree | grep -i
mpl` to double-check no transitive MPL.

### Step 2 — Bundle a font set (1-2h)

**Directory**: `crates/canvas/assets/fonts/`

Chrome ships with different font sets on Windows, macOS, Linux. The
minimum credible Chrome-on-Linux font list (per
`docs/CAPABILITY_GAPS_2026.md` §3.4):

- DejaVu Sans, Sans Mono, Serif (default Linux)
- Liberation Sans, Mono, Serif
- Noto Sans (for Unicode coverage)
- Arial, Arial Bold (or the open-source equivalent — Liberation Sans
  handles `Arial` font-family requests in Linux Chrome)
- Times New Roman (or Liberation Serif)
- Courier New (or Liberation Mono)

**Licensing**: Liberation fonts are SIL Open Font License 1.1.
DejaVu is a public-domain derivative. Noto fonts are SIL OFL.
None require source disclosure or modification tracking. All
compatible with our MIT/Apache-2.0 project policy.

Download:
- https://github.com/liberationfonts/liberation-fonts/releases (TTF)
- https://dejavu-fonts.github.io/Download.html
- https://github.com/googlefonts/noto-fonts (Noto Sans at minimum)

Bundled total: ~15 MB if you include Noto Sans which covers CJK and
RTL. Skip Noto if binary size is a concern; you lose Cyrillic/CJK
fingerprint accuracy but save 10 MB.

**Decision point**: bundle vs use host fonts via fontdb system scan.
- Bundle: reproducible across machines, larger binary.
- System scan: smaller binary, machine-dependent (BAD for
  fingerprinting — our "Chrome on Windows" profile shouldn't pick
  up macOS system fonts).

**Recommendation**: bundle. Match Chrome's bundled fonts per OS
profile. Store them in `assets/fonts/linux/`, `/windows/`, `/macos/`
subdirectories and select based on `StealthProfile::os_name`.

### Step 3 — Implement `FontDatabase` (4-6h)

**File**: `crates/canvas/src/text/font_database.rs` (new)

```rust
use fontdb::{Database, ID};
use std::sync::OnceLock;

pub struct FontDatabase {
    inner: Database,
}

impl FontDatabase {
    pub fn get() -> &'static FontDatabase {
        static INSTANCE: OnceLock<FontDatabase> = OnceLock::new();
        INSTANCE.get_or_init(Self::init_bundled)
    }

    fn init_bundled() -> FontDatabase {
        let mut db = Database::new();
        // Load bundled fonts.
        const LIBERATION_SANS: &[u8] = include_bytes!(
            "../../assets/fonts/LiberationSans-Regular.ttf"
        );
        const LIBERATION_SANS_BOLD: &[u8] = include_bytes!(
            "../../assets/fonts/LiberationSans-Bold.ttf"
        );
        const LIBERATION_SERIF: &[u8] = include_bytes!(
            "../../assets/fonts/LiberationSerif-Regular.ttf"
        );
        const LIBERATION_MONO: &[u8] = include_bytes!(
            "../../assets/fonts/LiberationMono-Regular.ttf"
        );
        const DEJAVU_SANS: &[u8] = include_bytes!(
            "../../assets/fonts/DejaVuSans.ttf"
        );
        const NOTO_SANS: &[u8] = include_bytes!(
            "../../assets/fonts/NotoSans-Regular.ttf"
        );
        db.load_font_data(LIBERATION_SANS.to_vec());
        db.load_font_data(LIBERATION_SANS_BOLD.to_vec());
        db.load_font_data(LIBERATION_SERIF.to_vec());
        db.load_font_data(LIBERATION_MONO.to_vec());
        db.load_font_data(DEJAVU_SANS.to_vec());
        db.load_font_data(NOTO_SANS.to_vec());

        // Set up Chrome-compatible family aliases so `Arial` maps to
        // Liberation Sans, `Times New Roman` maps to Liberation Serif,
        // etc. This matches Linux Chrome's default fallback behavior.
        db.set_family_alias("Arial", "Liberation Sans");
        db.set_family_alias("Helvetica", "Liberation Sans");
        db.set_family_alias("Times", "Liberation Serif");
        db.set_family_alias("Times New Roman", "Liberation Serif");
        db.set_family_alias("Courier", "Liberation Mono");
        db.set_family_alias("Courier New", "Liberation Mono");

        FontDatabase { inner: db }
    }

    pub fn query(&self, family: &str, weight: u16, italic: bool) -> Option<ID> {
        // Use fontdb's match() to find the best face for the family.
        // Handle fallback chain: requested family → generic family →
        // DejaVu Sans (always-available final fallback).
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            weight: fontdb::Weight(weight),
            stretch: fontdb::Stretch::Normal,
            style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
        };
        self.inner.query(&query)
            .or_else(|| self.inner.query(&fontdb::Query {
                families: &[fontdb::Family::SansSerif],
                ..query
            }))
    }

    pub fn get_face_data(&self, id: ID) -> Option<(&[u8], u32)> {
        self.inner.face(id).and_then(|face| {
            match &face.source {
                fontdb::Source::Binary(data) => Some((data.as_ref(), face.index)),
                _ => None,
            }
        })
    }
}
```

### Step 4 — Font shorthand parser (3-4h)

**File**: `crates/canvas/src/text/font_shorthand.rs` (new)

Canvas `ctx.font = "bold italic 14px Arial"` needs parsing. Real
Chrome follows the CSS `font` shorthand spec:

```
[ <font-style> || <font-variant-css21> || <font-weight> || <font-stretch> ]?
<font-size> [ / <line-height> ]? <font-family>
```

Implement a minimal parser:

```rust
pub struct ParsedFont {
    pub weight: u16,       // 100..900
    pub italic: bool,
    pub size_px: f32,
    pub families: Vec<String>,
}

impl ParsedFont {
    pub fn parse(s: &str) -> Option<ParsedFont> {
        // Split into tokens, walk them left to right.
        // Reference: https://developer.mozilla.org/en-US/docs/Web/CSS/font
        //
        // Example inputs:
        //   "14px Arial"
        //   "bold 14px 'Comic Sans MS'"
        //   "italic 14px / 1.4 Arial, sans-serif"
        //   "16px Arial, Helvetica, sans-serif"
        //
        // The font-size must appear exactly once and is followed by the
        // font-family. Everything before is optional prefix.
        // ...
    }

    pub fn default() -> ParsedFont {
        ParsedFont {
            weight: 400,
            italic: false,
            size_px: 10.0,
            families: vec!["sans-serif".to_string()],
        }
    }
}
```

**Tests** (in the same file or `text/tests.rs`):

```rust
#[test]
fn parse_simple() {
    let p = ParsedFont::parse("14px Arial").unwrap();
    assert_eq!(p.size_px, 14.0);
    assert_eq!(p.families, vec!["Arial"]);
    assert_eq!(p.weight, 400);
    assert!(!p.italic);
}

#[test]
fn parse_bold_italic() {
    let p = ParsedFont::parse("bold italic 16px 'Times New Roman'").unwrap();
    assert_eq!(p.weight, 700);
    assert!(p.italic);
    assert_eq!(p.families, vec!["Times New Roman"]);
}

#[test]
fn parse_multi_family() {
    let p = ParsedFont::parse("14px Arial, Helvetica, sans-serif").unwrap();
    assert_eq!(p.families, vec!["Arial", "Helvetica", "sans-serif"]);
}
```

### Step 5 — Text shaping with rustybuzz (8-12h)

**File**: `crates/canvas/src/text/shaper.rs` (new)

```rust
use rustybuzz::{Face, UnicodeBuffer};

pub struct ShapedRun {
    pub glyphs: Vec<Glyph>,
    /// Total advance width in font units (scaled to pixels later).
    pub width: f32,
    /// Ascent/descent in pixels.
    pub ascent: f32,
    pub descent: f32,
}

pub struct Glyph {
    pub glyph_id: u32,
    pub cluster: u32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

pub fn shape(text: &str, face_data: &[u8], face_index: u32, size_px: f32) -> ShapedRun {
    let face = Face::from_slice(face_data, face_index)
        .expect("fontdb gave us an invalid face — this is a bug");
    let upem = face.units_per_em() as f32;
    let scale = size_px / upem;

    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();

    let glyph_buffer = rustybuzz::shape(&face, &[], buffer);
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let mut glyphs = Vec::with_capacity(infos.len());
    let mut total_advance = 0.0_f32;
    for (info, pos) in infos.iter().zip(positions.iter()) {
        let x_advance = pos.x_advance as f32 * scale;
        let y_advance = pos.y_advance as f32 * scale;
        glyphs.push(Glyph {
            glyph_id: info.glyph_id,
            cluster: info.cluster,
            x_advance,
            y_advance,
            x_offset: pos.x_offset as f32 * scale,
            y_offset: pos.y_offset as f32 * scale,
        });
        total_advance += x_advance;
    }

    let ascender = face.ascender() as f32 * scale;
    let descender = face.descender() as f32 * scale;

    ShapedRun {
        glyphs,
        width: total_advance,
        ascent: ascender,
        descent: -descender, // Chrome convention: positive descent
    }
}
```

### Step 6 — Glyph rasterization with swash (6-10h)

**File**: `crates/canvas/src/text/raster.rs` (new)

```rust
use swash::scale::*;
use swash::FontRef;

pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
    pub pixels: Vec<u8>, // alpha only
}

pub fn rasterize_glyph(
    face_data: &[u8],
    face_index: u32,
    glyph_id: u16,
    size_px: f32,
) -> Option<GlyphBitmap> {
    let font = FontRef::from_index(face_data, face_index as usize)?;
    let mut context = ScaleContext::new();
    let mut scaler = context.builder(font)
        .size(size_px)
        .hint(true)
        .build();

    let image = Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
        Source::Outline,
    ])
    .format(swash::zeno::Format::Alpha)
    .render(&mut scaler, swash::GlyphId::new(glyph_id))?;

    Some(GlyphBitmap {
        width: image.placement.width,
        height: image.placement.height,
        left: image.placement.left,
        top: image.placement.top,
        pixels: image.data,
    })
}
```

### Step 7 — Canvas fillText integration (6-10h)

**File**: `crates/canvas/src/canvas2d.rs` (modify)

Add a `fill_text` function that uses the new shaper + rasterizer:

```rust
impl Canvas2D {
    pub fn fill_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_str: &str,
        fill_color: Color,
    ) -> Result<(), TextError> {
        // 1. Parse the font shorthand.
        let font = font_shorthand::ParsedFont::parse(font_str)
            .unwrap_or_else(font_shorthand::ParsedFont::default);

        // 2. Resolve the font family through the fallback chain.
        let (face_id, _family) = self.resolve_font_family(&font)?;

        // 3. Get face data.
        let db = FontDatabase::get();
        let (face_data, face_index) = db.get_face_data(face_id)
            .ok_or(TextError::FaceNotLoaded)?;

        // 4. Shape the text.
        let run = shaper::shape(text, face_data, face_index, font.size_px);

        // 5. For each glyph, rasterize and composite onto self.pixmap.
        let mut cursor_x = x;
        let baseline_y = y; // alphabetic baseline at y
        for glyph in &run.glyphs {
            let bitmap = raster::rasterize_glyph(
                face_data, face_index, glyph.glyph_id as u16, font.size_px,
            );
            if let Some(bitmap) = bitmap {
                let draw_x = (cursor_x + glyph.x_offset + bitmap.left as f32) as i32;
                let draw_y = (baseline_y + glyph.y_offset - bitmap.top as f32) as i32;
                self.composite_alpha_mask(
                    &bitmap.pixels, bitmap.width, bitmap.height,
                    draw_x, draw_y, fill_color,
                );
            }
            cursor_x += glyph.x_advance;
        }
        Ok(())
    }

    fn composite_alpha_mask(
        &mut self,
        mask: &[u8], w: u32, h: u32,
        x: i32, y: i32, color: Color,
    ) {
        // For each pixel in the alpha mask, blend against the canvas
        // pixmap using premultiplied-alpha SOURCE_OVER.
        // ...
    }
}
```

### Step 8 — measureText (4-6h)

**File**: `crates/canvas/src/canvas2d.rs`

```rust
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

impl Canvas2D {
    pub fn measure_text(&self, text: &str, font_str: &str) -> TextMetrics {
        let font = font_shorthand::ParsedFont::parse(font_str)
            .unwrap_or_else(font_shorthand::ParsedFont::default);
        let (face_id, _) = self.resolve_font_family(&font).unwrap();
        let db = FontDatabase::get();
        let (face_data, face_index) = db.get_face_data(face_id).unwrap();
        let run = shaper::shape(text, face_data, face_index, font.size_px);

        // Compute all 13 fields from the run.
        // Chrome reference values for "The quick brown fox" at 16px Arial:
        //   width: 119.28 (varies by ~1px between fonts)
        //   fontBoundingBoxAscent: 14.77
        //   fontBoundingBoxDescent: 3.84
        //   actualBoundingBoxAscent: 11.59
        //   actualBoundingBoxDescent: 3.84
        //   ...

        TextMetrics {
            width: run.width,
            actual_bounding_box_left: 0.0, // depends on glyph bboxes
            actual_bounding_box_right: run.width,
            actual_bounding_box_ascent: run.ascent,
            actual_bounding_box_descent: run.descent,
            font_bounding_box_ascent: run.ascent,
            font_bounding_box_descent: run.descent,
            em_height_ascent: font.size_px * 0.8,  // approximation
            em_height_descent: font.size_px * 0.2,
            hanging_baseline: run.ascent * 0.8,
            alphabetic_baseline: 0.0,
            ideographic_baseline: -run.descent * 0.5,
        }
    }
}
```

### Step 9 — Wire through canvas_ext.rs and canvas_bootstrap.js (3-4h)

**File**: `crates/js_runtime/src/extensions/canvas_ext.rs`

Update `op_canvas_fill_text` and add `op_canvas_measure_text` to call
the new Canvas2D methods. Pass font string and fill color explicitly.

**File**: `crates/js_runtime/src/js/canvas_bootstrap.js`

Update `CanvasRenderingContext2D`:

```js
fillText(text, x, y, maxWidth) {
    const result = ops.op_canvas_fill_text(
        this.#id, text, x, y,
        this.#font,           // "14px Arial"
        this.#fillStyle,      // "#ff0000" or rgb(...)
        maxWidth || -1,
    );
}

measureText(text) {
    const metrics = ops.op_canvas_measure_text(
        this.#id, text, this.#font,
    );
    return {
        width: metrics.width,
        actualBoundingBoxLeft: metrics.actual_bounding_box_left,
        actualBoundingBoxRight: metrics.actual_bounding_box_right,
        actualBoundingBoxAscent: metrics.actual_bounding_box_ascent,
        actualBoundingBoxDescent: metrics.actual_bounding_box_descent,
        fontBoundingBoxAscent: metrics.font_bounding_box_ascent,
        fontBoundingBoxDescent: metrics.font_bounding_box_descent,
        emHeightAscent: metrics.em_height_ascent,
        emHeightDescent: metrics.em_height_descent,
        hangingBaseline: metrics.hanging_baseline,
        alphabeticBaseline: metrics.alphabetic_baseline,
        ideographicBaseline: metrics.ideographic_baseline,
    };
}
```

### Step 10 — Tests (4-6h)

**File**: `crates/canvas/tests/text_metrics.rs` (new)

```rust
#[test]
fn width_measurement_basic() {
    let canvas = canvas::Canvas2D::new(300, 150);
    let metrics = canvas.measure_text("Hello, World!", "16px Arial");
    // Real Chrome for 16px Arial on Linux returns ~92-96px depending
    // on version. Allow ±5px tolerance because Liberation Sans is
    // not pixel-identical to Arial.
    assert!((metrics.width - 94.0).abs() < 5.0,
            "width {} not within tolerance of 94.0", metrics.width);
}

#[test]
fn different_fonts_different_widths() {
    let canvas = canvas::Canvas2D::new(300, 150);
    let sans = canvas.measure_text("The quick brown fox", "16px Arial");
    let mono = canvas.measure_text("The quick brown fox", "16px 'Courier New'");
    // Mono should be wider than variable-width.
    assert!(mono.width > sans.width);
}

#[test]
fn bold_wider_than_regular() {
    let canvas = canvas::Canvas2D::new(300, 150);
    let reg = canvas.measure_text("Hello", "14px Arial");
    let bold = canvas.measure_text("Hello", "bold 14px Arial");
    assert!(bold.width >= reg.width);
}
```

**File**: `crates/canvas/tests/text_rendering.rs` (new)

```rust
#[test]
fn fill_text_produces_non_empty_pixels() {
    let mut canvas = canvas::Canvas2D::new(300, 150);
    canvas.fill_text("A", 10.0, 30.0, "24px Arial", Color::BLACK).unwrap();
    // Count non-transparent pixels.
    let nonempty = canvas.count_non_transparent_pixels();
    assert!(nonempty > 50, "expected rasterized glyph, got {nonempty} px");
}
```

### Step 11 — FontFace API and enumerateFonts (4-6h)

Some fingerprinters use `document.fonts.check()` or iterate
`document.fonts`. Implement a minimal `FontFaceSet` that exposes the
bundled fonts:

```js
// In dom_bootstrap.js or a new font_bootstrap.js
globalThis.FontFace = class FontFace {
    constructor(family, source, descriptors) {
        this.family = family;
        this.source = source;
        Object.assign(this, descriptors || {});
        this.status = 'unloaded';
    }
    load() { this.status = 'loaded'; return Promise.resolve(this); }
};

if (document && !document.fonts) {
    document.fonts = {
        ready: Promise.resolve(),
        status: 'loaded',
        size: 6, // our bundled count
        check(font, text) {
            // Parse the font string, return true if we have the family.
            return Deno.core.ops.op_font_check(font) || false;
        },
        entries() { /* iterator over bundled faces */ },
        values() { /* same */ },
        keys() { /* same */ },
        forEach(cb) { /* same */ },
        [Symbol.iterator]() { return this.entries(); },
    };
}
```

## Acceptance criteria

1. **Tests pass**: new `text_metrics.rs` and `text_rendering.rs` tests
   all green.
2. **Workspace still green**: `cargo test --workspace -- --test-threads=1`
   zero failures.
3. **Deep-path regression**: all 22 currently-passing sites still HOLD
   on `deep_path_validation.rs`.
4. **Blocker probe**: re-run `blocker_rigorous_probe.rs` 3 times. Look
   for: (a) POST body size changes on adidas/homedepot (should grow if
   our `fillText` output now matches Chrome more closely), (b) any
   stochastic PASSes.
5. **Fingerprint site check**: manually navigate CreepJS or
   FingerprintJS via a probe test and verify `measureText` values are
   in the expected Chrome range. Some tolerance is fine; the goal is
   "not obviously wrong" not "bit-perfect".
6. **Binary size**: `cargo build --release` should add ~15-20 MB to
   the binary (mostly bundled fonts). Still under the 500 MB budget.

## Risks and mitigations

**Risk 1: Liberation fonts aren't pixel-identical to Arial.**
Inevitable. Our measureText values will be ~1-3% off from a real
Chrome on Windows with DirectWrite. Mitigation: bundle Microsoft's
Core Fonts for the Web (Arial, Times New Roman, Courier New, Verdana,
etc.) if the license allows for this use (check — they're
redistributable under specific conditions).

**Risk 2: swash rasterization differs from Skia's text rasterizer.**
Chrome uses Skia's text rendering which has its own hinting and
subpixel positioning. swash is close but not identical. For fingerprint
sites that hash the exact canvas pixels (not just width), we'll
differ. Mitigation: after T1.1 ships skia-safe, we can use
`skia_safe::Canvas::draw_text` directly with an `SkFont` loaded from
the same `swash` face data. That should produce pixel-identical output.

**Risk 3: Binary size explosion.** Including Noto Sans + CJK + RTL
adds ~30 MB. Mitigation: subset the fonts to Latin + Cyrillic + basic
punctuation using `pyftsubset` or `fonttools`. Reduces each font file
by 70-90%.

**Risk 4: FontDB system scan pollution.** If a developer runs tests on
a machine with a non-default font set, system scan picks up their
fonts and our output varies. Mitigation: NEVER call
`db.load_system_fonts()` in production code. Only load bundled fonts.

## After this ships

- Re-run `blocker_rigorous_probe` 3 times to measure impact.
- If adidas/homedepot POST body sizes change meaningfully, keep going
  on capability work (T1.1 canvas next).
- If nothing moves for the tier-1 sites, document that fonts weren't
  the bottleneck and pivot to T1.1.
- Measure CreepJS trust score improvement (should drop).

## Related

- `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md` —
  adidas sensor VM calls `fillText` twice.
- `docs/universal_engine/05_capability_gaps.md` — T1.x tier overview.
- Task #37 (completed) — added 13-field TextMetrics shape previously.
  This task implements the VALUES; #37 only ensured the property
  names existed.
