# T1.1 — Replace tiny_skia with skia-safe for Canvas 2D

**Task**: #62
**Priority**: P1
**Effort**: 25-35 hours
**Dependencies**: Sprint 0 refactor. T1.2 (fonts) should ideally ship
first so we can use Skia's native text rendering via swash-loaded
faces, but it's not strictly required.

## Goal

Replace the current `tiny_skia` (MIT, pure-Rust, limited) canvas backend
with `skia-safe` (MIT, Chrome's actual Skia library via C++ bindings).
Produce pixel-identical output to real Chrome's Skia for the same paint
operations, eliminating canvas fingerprint mismatches.

## Why

The adidas sensor VM probe (task #69) showed that the VM calls 11 Ctx2D
paint methods but never extracts pixels via standard APIs. This means
T1.1 may NOT be on the adidas critical path. However, for the broader
stealth browser goal, T1.1 is load-bearing:

- Every fingerprint-sensitive site that hashes `canvas.toDataURL()` or
  `ctx.getImageData()` compares against Chrome's Skia output. CreepJS,
  FingerprintJS, amiunique, pixelscan all do this.
- `tiny_skia` has known gaps that Chrome's Skia doesn't:
  - No conical/two-circle gradient support
  - Different anti-aliasing algorithm
  - Different Bezier curve discretization
  - Limited blend mode coverage
- The existing `canvas2d.rs` has an active bug where radial gradient
  construction drops `x0, y0, r0` — documented in the session but not
  fixed because the tiny_skia API can't express it.

Per-pixel Chrome fidelity on canvas is one of the largest single items
separating browser_oxide from "passes every fingerprinting site".

## Current state

**Files**:
- `crates/canvas/Cargo.toml` — `tiny_skia = "0.11"`
- `crates/canvas/src/canvas2d.rs` — the Canvas2D struct and paint ops.
  Has the known radial-gradient bug at the conversion from our Gradient
  enum to tiny_skia's RadialGradient (drops x0, y0, r0).
- `crates/canvas/src/path.rs` — Path2D wrapping tiny_skia's PathBuilder.
- `crates/canvas/src/text.rs` — stub text, replaced by T1.2.
- `crates/js_runtime/src/extensions/canvas_ext.rs` — ops that call
  Canvas2D methods.

**What tiny_skia can't do** (tested and documented):
- `createRadialGradient(x0, y0, r0, x1, y1, r1)` with two distinct
  circles (the "conical" form). tiny_skia only supports two-point
  gradients where `r0 == 0`.
- `CanvasFilter` (`ctx.filter = "blur(5px) sepia()"`). No filter
  pipeline at all.
- `globalCompositeOperation` beyond `source-over`, `destination-over`,
  `source-in`, `destination-in`, `source-out`, `destination-out`,
  `source-atop`, `destination-atop`, `xor`. Chrome supports ~26 modes
  including `multiply`, `screen`, `overlay`, `darken`, `lighten`,
  `color-dodge`, etc.
- `shadowBlur`, `shadowColor`, `shadowOffsetX`, `shadowOffsetY`.
  tiny_skia has no shadow support; we'd need to manually blur an alpha
  buffer.
- Pattern fills (`ctx.createPattern(image, repetition)`) are stubbed.

## skia-safe overview

- **Crate**: `skia-safe = "0.78"` (current as of 2026-04)
- **License**: BSD-3 (Skia itself) + MIT (skia-safe bindings). Clean
  for our MIT/Apache-2.0 policy.
- **Binary size**: ~50 MB for precompiled Linux x86_64 bindings
  (download on first build, cached in `target/skia-binaries`).
- **API**: close-to-one-to-one with Skia C++, wrapped in safe Rust.
- **Maintained**: active, 1k+ stars, regular releases tracking Skia
  upstream.

## Step-by-step implementation

### Step 1 — Dependency swap (30 min)

**File**: `crates/canvas/Cargo.toml`

```toml
[dependencies]
# Remove: tiny_skia = "0.11"
skia-safe = { version = "0.78", features = ["textlayout"] }
```

The `textlayout` feature enables `SkParagraph` which we'll use for
text shaping integration with T1.2 (if it landed first).

Run `cargo build -p canvas` — first build downloads ~50 MB of
precompiled Skia. Add `target/skia-binaries` to `.gitignore` if not
already there.

### Step 2 — Rewrite Canvas2D (8-12h)

**File**: `crates/canvas/src/canvas2d.rs`

Replace the tiny_skia-based struct:

```rust
use skia_safe::{
    Canvas, ColorType, Data, ImageInfo, Paint, PaintStyle, Path, PathFillType,
    Point, Rect, Surface,
};

pub struct Canvas2D {
    surface: Surface,
    width: u32,
    height: u32,
    // Current state (Canvas 2D state stack)
    state_stack: Vec<CanvasState>,
    current_state: CanvasState,
}

#[derive(Clone)]
struct CanvasState {
    fill_paint: Paint,
    stroke_paint: Paint,
    line_width: f32,
    line_cap: LineCap,
    line_join: LineJoin,
    miter_limit: f32,
    line_dash: Vec<f32>,
    line_dash_offset: f32,
    global_alpha: f32,
    global_composite_operation: BlendMode,
    font: String,
    text_align: TextAlign,
    text_baseline: TextBaseline,
    direction: TextDirection,
    transform: skia_safe::Matrix,
    shadow_blur: f32,
    shadow_color: Color,
    shadow_offset_x: f32,
    shadow_offset_y: f32,
    filter: Vec<FilterOp>,
    image_smoothing_enabled: bool,
    image_smoothing_quality: ImageSmoothingQuality,
}

impl Canvas2D {
    pub fn new(width: u32, height: u32) -> Self {
        let info = ImageInfo::new(
            (width as i32, height as i32),
            ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let surface = Surface::new_raster(&info, None, None)
            .expect("failed to create Skia surface");
        // ... initialize state ...
    }

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let rect = Rect::from_xywh(x, y, w, h);
        self.surface.canvas().draw_rect(rect, &self.current_state.fill_paint);
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let rect = Rect::from_xywh(x, y, w, h);
        self.surface.canvas().draw_rect(rect, &self.current_state.stroke_paint);
    }

    pub fn fill_path(&mut self, path: &Path) {
        self.surface.canvas().draw_path(path, &self.current_state.fill_paint);
    }

    pub fn stroke_path(&mut self, path: &Path) {
        self.surface.canvas().draw_path(path, &self.current_state.stroke_paint);
    }

    // ... all other methods ...
}
```

**Key translations from tiny_skia to skia-safe**:

| tiny_skia | skia-safe |
|---|---|
| `Pixmap::new(w, h)` | `Surface::new_raster(info, ...)` |
| `PathBuilder` | `Path::new()` + `moveTo`/`lineTo`/etc. |
| `Paint { shader: Shader::SolidColor(..) }` | `Paint::new(Color4f::from_rgba_f32, None)` |
| `RadialGradient::new(start, end, ...)` | `GradientShader::two_point_conical(...)` |
| `Paint::blend_mode` | `Paint::set_blend_mode(BlendMode::...)` |
| `Pixmap::fill_path(path, paint, fill_rule)` | `Canvas::draw_path(path, paint)` |

### Step 3 — Fix the radial gradient bug (1h)

**File**: `crates/canvas/src/canvas2d.rs`

The current bug (dropped `x0, y0, r0`) exists because tiny_skia has no
API for two-circle gradients. skia-safe does:

```rust
use skia_safe::gradient_shader::{two_point_conical, GradientShaderColors};

fn build_radial_gradient(
    x0: f32, y0: f32, r0: f32,
    x1: f32, y1: f32, r1: f32,
    stops: &[(f32, Color)],
) -> skia_safe::Shader {
    let colors: Vec<skia_safe::Color> = stops.iter()
        .map(|(_, c)| skia_safe::Color::from_argb(c.a, c.r, c.g, c.b))
        .collect();
    let positions: Vec<f32> = stops.iter().map(|(p, _)| *p).collect();

    two_point_conical(
        Point::new(x0, y0), r0,     // start circle — NOW USED
        Point::new(x1, y1), r1,     // end circle
        GradientShaderColors::Colors(&colors),
        Some(&positions[..]),
        skia_safe::TileMode::Clamp,
        None, None,
    ).expect("two_point_conical returned None")
}
```

Add a test:

```rust
#[test]
fn radial_gradient_uses_x0_y0_r0() {
    let mut canvas = Canvas2D::new(200, 200);
    canvas.set_fill_style_radial_gradient(
        10.0, 10.0, 5.0,    // inner circle at (10,10) r=5
        100.0, 100.0, 80.0, // outer circle at (100,100) r=80
        &[(0.0, Color::RED), (1.0, Color::BLUE)],
    );
    canvas.fill_rect(0.0, 0.0, 200.0, 200.0);
    // Verify pixel at (10, 10) is red-ish, pixel at (100, 100) is
    // blue-ish, and pixel at (5, 5) is transparent (outside both
    // circles before the gradient starts).
    // ...
}
```

### Step 4 — Full path2d rewrite (3-5h)

**File**: `crates/canvas/src/path.rs`

```rust
use skia_safe::Path as SkPath;

pub struct Path2D {
    inner: SkPath,
}

impl Path2D {
    pub fn new() -> Self {
        Self { inner: SkPath::new() }
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.inner.move_to((x, y));
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.inner.line_to((x, y));
    }

    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32,
                           cp2x: f32, cp2y: f32, x: f32, y: f32) {
        self.inner.cubic_to((cp1x, cp1y), (cp2x, cp2y), (x, y));
    }

    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.inner.quad_to((cpx, cpy), (x, y));
    }

    pub fn arc(&mut self, x: f32, y: f32, radius: f32,
               start: f32, end: f32, counterclockwise: bool) {
        let rect = skia_safe::Rect::from_ltrb(
            x - radius, y - radius, x + radius, y + radius,
        );
        let start_deg = start.to_degrees();
        let sweep_deg = if counterclockwise {
            (start - end).to_degrees()
        } else {
            (end - start).to_degrees()
        };
        self.inner.arc_to(&rect, start_deg, sweep_deg, false);
    }

    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) {
        self.inner.arc_to_tangent((x1, y1), (x2, y2), radius);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.inner.add_rect(
            skia_safe::Rect::from_xywh(x, y, w, h),
            None,
        );
    }

    pub fn ellipse(&mut self, x: f32, y: f32, rx: f32, ry: f32,
                   rotation: f32, start: f32, end: f32,
                   counterclockwise: bool) {
        // SkPath doesn't have a direct "ellipse from center + 2 radii +
        // rotation + angle range" API. Build via addOval + transforms.
        // ...
    }

    pub fn close_path(&mut self) {
        self.inner.close();
    }

    pub fn as_skia(&self) -> &SkPath {
        &self.inner
    }
}
```

### Step 5 — Gradients and patterns (2-4h)

Implement `createLinearGradient`, `createRadialGradient`,
`createConicGradient` (Chrome has this!), and `createPattern`. All
map to `skia_safe::gradient_shader` and `skia_safe::Shader`.

### Step 6 — Text via Skia (if T1.2 shipped) (2-4h)

If T1.2 shipped, use swash-loaded font faces to build `SkFont` and
call `canvas.draw_text_blob` or similar. If T1.2 didn't ship, use the
existing stub temporarily and plan to retrofit after T1.2 lands.

```rust
use skia_safe::{Font, TextBlob, Typeface};

pub fn fill_text(&mut self, text: &str, x: f32, y: f32) {
    let typeface = Typeface::from_name("Arial", skia_safe::FontStyle::default())
        .unwrap_or_else(|| Typeface::default());
    let font = Font::from_typeface_with_params(typeface, 16.0, 1.0, 0.0);
    if let Some(blob) = TextBlob::from_str(text, &font) {
        self.surface.canvas().draw_text_blob(
            &blob, (x, y), &self.current_state.fill_paint,
        );
    }
}
```

### Step 7 — getImageData / toDataURL (2-3h)

```rust
pub fn get_image_data(&self, x: i32, y: i32, w: u32, h: u32) -> Vec<u8> {
    let mut buffer = vec![0u8; (w * h * 4) as usize];
    let info = ImageInfo::new(
        (w as i32, h as i32),
        ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    self.surface.canvas().read_pixels(
        &info,
        &mut buffer,
        (w * 4) as usize,
        (x, y),
    );
    buffer
}

pub fn to_data_url(&mut self, format: &str, quality: f32) -> String {
    let image = self.surface.image_snapshot();
    let format = match format {
        "image/jpeg" => skia_safe::EncodedImageFormat::JPEG,
        "image/webp" => skia_safe::EncodedImageFormat::WEBP,
        _ => skia_safe::EncodedImageFormat::PNG,
    };
    let data = image.encode_to_data(format).unwrap();
    let base64 = base64::encode(data.as_bytes());
    format!("data:{};base64,{}", format_mime_type(format), base64)
}
```

### Step 8 — Blend modes and filters (2-4h)

Map the 26 CSS blend modes to `skia_safe::BlendMode`:

```rust
fn parse_composite_operation(s: &str) -> BlendMode {
    match s {
        "source-over" => BlendMode::SrcOver,
        "source-in" => BlendMode::SrcIn,
        "source-out" => BlendMode::SrcOut,
        "source-atop" => BlendMode::SrcATop,
        "destination-over" => BlendMode::DstOver,
        "destination-in" => BlendMode::DstIn,
        "destination-out" => BlendMode::DstOut,
        "destination-atop" => BlendMode::DstATop,
        "lighter" => BlendMode::Plus,
        "copy" => BlendMode::Src,
        "xor" => BlendMode::Xor,
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "color-dodge" => BlendMode::ColorDodge,
        "color-burn" => BlendMode::ColorBurn,
        "hard-light" => BlendMode::HardLight,
        "soft-light" => BlendMode::SoftLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        "hue" => BlendMode::Hue,
        "saturation" => BlendMode::Saturation,
        "color" => BlendMode::Color,
        "luminosity" => BlendMode::Luminosity,
        _ => BlendMode::SrcOver,
    }
}
```

Filters (`ctx.filter = "blur(5px) grayscale(50%)"`) map to
`skia_safe::ImageFilter`:

```rust
fn parse_filter(s: &str) -> Option<skia_safe::ImageFilter> {
    // ... parse CSS filter syntax, build filter chain ...
}
```

### Step 9 — Update canvas_ext.rs ops (1-2h)

**File**: `crates/js_runtime/src/extensions/canvas_ext.rs`

All the op signatures stay the same; the implementations now call
skia-safe under the hood. Most edits are mechanical.

### Step 10 — Tests (3-5h)

**File**: `crates/canvas/tests/skia_canvas.rs` (new)

Port the existing tiny_skia tests and add new ones that exercise
skia-specific features:

```rust
#[test]
fn conical_gradient_renders() {
    let mut canvas = Canvas2D::new(100, 100);
    canvas.set_fill_style_conic_gradient(
        50.0, 50.0, 0.0,
        &[(0.0, Color::RED), (0.5, Color::GREEN), (1.0, Color::BLUE)],
    );
    canvas.fill_rect(0.0, 0.0, 100.0, 100.0);
    // Verify the gradient is rotated correctly.
}

#[test]
fn shadow_blur_works() {
    let mut canvas = Canvas2D::new(100, 100);
    canvas.set_shadow_blur(10.0);
    canvas.set_shadow_color(Color::BLACK);
    canvas.set_fill_style(Color::RED);
    canvas.fill_rect(40.0, 40.0, 20.0, 20.0);
    // Verify there's a blurred shadow around the red rect.
}

#[test]
fn blend_mode_multiply() {
    // Draw two overlapping shapes with multiply blend mode.
    // Verify the overlap is darker than either input.
}
```

**Regression test**: `crates/canvas/tests/canvas_regression.rs`.
Compare output of a fixed drawing sequence against a golden image
(committed PNG). Regenerate the golden any time the rendering changes
intentionally.

## Acceptance criteria

1. **Workspace green**: `cargo test --workspace -- --test-threads=1`.
2. **All existing canvas tests still pass** (after porting from
   tiny_skia to skia-safe).
3. **New conical gradient, shadow, blend mode, and filter tests
   pass**.
4. **Radial gradient bug fixed**: `x0, y0, r0` are honored.
5. **toDataURL output is stable across runs** for the same input
   (deterministic).
6. **Binary size**: `cargo build --release` adds ~50 MB (Skia
   precompiled). Total under 500 MB budget.
7. **Fingerprint verification**: manually run CreepJS via a probe and
   verify canvas hash moves closer to Chrome's reference. Our hash
   won't be bit-identical (Chrome compiles Skia with different
   optimization flags and on different architectures) but should
   match for the "canonical" test strings.

## Risks and mitigations

**Risk 1: Binary size explosion.** 50 MB of precompiled Skia is a lot.
Mitigation: skia-safe supports building from source with selective
features. Disable WebGL, Metal, Vulkan, GPU in general — we only
need the raster backend. That cuts the binary significantly.
`skia-safe = { version = "0.78", default-features = false,
features = ["textlayout"] }`.

**Risk 2: Cross-compilation pain.** skia-safe requires a C++ toolchain
for source builds and prebuilt binaries only exist for common
targets (x86_64-linux, aarch64-linux, x86_64-windows, x86_64-apple,
aarch64-apple). Mitigation: document supported targets explicitly
and gate the canvas crate behind a feature flag for unsupported
targets, falling back to tiny_skia (with reduced fidelity).

**Risk 3: Not pixel-identical to real Chrome.** Chrome uses a specific
Skia build with custom flags. skia-safe uses upstream Skia defaults.
Some pixel-level differences will persist even after T1.1. Mitigation:
calibrate against specific known test strings (the ones CreepJS uses)
rather than aiming for bit-perfect parity.

**Risk 4: Regressions in tests that used to work with tiny_skia.**
Port carefully. Run the full test suite after each major function
port, not just at the end.

## After this ships

- Re-run `blocker_rigorous_probe` to see if any stochastic passes
  appear.
- Manually check CreepJS / FingerprintJS / pixelscan trust scores.
- Verify the existing `docs/fingerprint_scorers.md` scores improve.
- Consider landing T1.3b (full PeriodicWave wavetable) next, since
  audio is the only other T1.x that has a specific site targeting
  it.

## Related

- `docs/CANVAS.md` — existing canvas docs
- `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md` —
  adidas doesn't extract canvas pixels per probe, so T1.1 may not
  move adidas (but will move many other sites).
- Task #69 — completed investigation of adidas canvas extraction
  path.
