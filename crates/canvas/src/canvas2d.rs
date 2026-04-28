//! CPU-based Canvas 2D rendering context backed by Skia via `skia-safe`.
//!
//! Design: the canvas owns a raw premultiplied-RGBA8 pixel buffer
//! (`Vec<u8>`). Every paint operation creates an ephemeral
//! `skia_safe::Surface` via `Surface::new_raster_direct` that borrows the
//! buffer for the duration of the call. This keeps the backing pixels
//! directly accessible for text rasterization (via `text::rasterize_text`)
//! and for `get_image_data` / `put_image_data` without self-referential
//! lifetimes between the Vec and the Surface.
//!
//! Replaces the previous `tiny_skia` backend. Known tiny_skia limitations
//! are now lifted: two-circle conical gradients, blend modes, shadows, and
//! filters are all available via Skia (though only a subset is wired up in
//! Phase A — see T1.1 Phase B for the rest).

use crate::path::Path2D;
use crate::text::{self, ParsedFont, TextMetrics};
use skia_safe::{
    gradient_shader, image_filters, surfaces, AlphaType, BlendMode, Canvas as SkCanvas, Color4f,
    ColorFilter, ColorType, ImageInfo, Matrix, Paint, PaintStyle, Point, Rect as SkRect, TileMode,
};

/// Simple 0-255 RGBA color. This is the public color type exposed to
/// `canvas_ext.rs` via `make_color` — it intentionally avoids leaking any
/// `skia_safe` type across the crate boundary so the rest of the workspace
/// doesn't need to depend on skia-safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn red(self) -> f32 {
        self.r as f32 / 255.0
    }
    pub fn green(self) -> f32 {
        self.g as f32 / 255.0
    }
    pub fn blue(self) -> f32 {
        self.b as f32 / 255.0
    }
    pub fn alpha(self) -> f32 {
        self.a as f32 / 255.0
    }

    fn to_color4f(self) -> Color4f {
        Color4f::new(self.red(), self.green(), self.blue(), self.alpha())
    }
}

/// A gradient definition with color stops.
#[derive(Debug, Clone)]
pub enum Gradient {
    Linear {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        stops: Vec<(f32, Color)>,
    },
    Radial {
        x0: f32,
        y0: f32,
        r0: f32,
        x1: f32,
        y1: f32,
        r1: f32,
        stops: Vec<(f32, Color)>,
    },
    /// Canvas 2D `createConicGradient(startAngle, x, y)` — a sweep
    /// gradient centred at `(cx, cy)` starting at angle `start_angle`
    /// (in radians, measured clockwise from positive x-axis).
    Conic {
        cx: f32,
        cy: f32,
        start_angle: f32,
        stops: Vec<(f32, Color)>,
    },
}

impl Gradient {
    fn to_shader(&self) -> Option<skia_safe::Shader> {
        match self {
            Gradient::Linear {
                x0,
                y0,
                x1,
                y1,
                stops,
            } => {
                if stops.len() < 2 {
                    return None;
                }
                let (colors, positions) = split_stops(stops);
                gradient_shader::linear(
                    (Point::new(*x0, *y0), Point::new(*x1, *y1)),
                    gradient_shader::GradientShaderColors::ColorsInSpace(&colors, None),
                    Some(&positions[..]),
                    TileMode::Clamp,
                    None,
                    None,
                )
            }
            Gradient::Radial {
                x0,
                y0,
                r0,
                x1,
                y1,
                r1,
                stops,
            } => {
                if stops.len() < 2 {
                    return None;
                }
                let (colors, positions) = split_stops(stops);
                // `two_point_conical` is Skia's general two-circle radial
                // gradient — this is the primitive that maps directly to
                // Canvas 2D's `createRadialGradient(x0, y0, r0, x1, y1, r1)`.
                // The previous tiny_skia backend could not express this;
                // we now honour `(x0, y0, r0)` exactly.
                gradient_shader::two_point_conical(
                    Point::new(*x0, *y0),
                    *r0,
                    Point::new(*x1, *y1),
                    *r1,
                    gradient_shader::GradientShaderColors::ColorsInSpace(&colors, None),
                    Some(&positions[..]),
                    TileMode::Clamp,
                    None,
                    None,
                )
            }
            Gradient::Conic {
                cx,
                cy,
                start_angle,
                stops,
            } => {
                if stops.len() < 2 {
                    return None;
                }
                let (colors, positions) = split_stops(stops);
                // Canvas 2D spec: conic gradients start at `startAngle`
                // measured clockwise from the positive x-axis. Skia's
                // sweep_gradient also starts at 0° east and sweeps
                // clockwise, so we just convert radians to degrees.
                let start_deg = start_angle.to_degrees();
                gradient_shader::sweep(
                    (*cx, *cy),
                    gradient_shader::GradientShaderColors::ColorsInSpace(&colors, None),
                    Some(&positions[..]),
                    TileMode::Clamp,
                    (start_deg, start_deg + 360.0),
                    None,
                    None,
                )
            }
        }
    }
}

fn split_stops(stops: &[(f32, Color)]) -> (Vec<Color4f>, Vec<f32>) {
    let mut colors = Vec::with_capacity(stops.len());
    let mut positions = Vec::with_capacity(stops.len());
    for (offset, color) in stops {
        colors.push(color.to_color4f());
        positions.push(*offset);
    }
    (colors, positions)
}

/// Fill or stroke style — solid color, gradient, or image pattern.
#[derive(Clone)]
enum FillStyle {
    Solid(Color),
    Gradient(Gradient),
    Pattern(Pattern),
}

/// CSS `createPattern(image, repetition)`.
#[derive(Debug, Clone)]
pub struct Pattern {
    /// Raw RGBA8 pixels of the source image (non-premultiplied).
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub repetition: PatternRepetition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternRepetition {
    Repeat,
    RepeatX,
    RepeatY,
    NoRepeat,
}

impl PatternRepetition {
    pub fn parse(s: &str) -> Self {
        match s {
            "repeat" | "" => Self::Repeat,
            "repeat-x" => Self::RepeatX,
            "repeat-y" => Self::RepeatY,
            "no-repeat" => Self::NoRepeat,
            _ => Self::Repeat,
        }
    }

    fn tile_modes(self) -> (TileMode, TileMode) {
        match self {
            PatternRepetition::Repeat => (TileMode::Repeat, TileMode::Repeat),
            PatternRepetition::RepeatX => (TileMode::Repeat, TileMode::Decal),
            PatternRepetition::RepeatY => (TileMode::Decal, TileMode::Repeat),
            PatternRepetition::NoRepeat => (TileMode::Decal, TileMode::Decal),
        }
    }
}

impl Pattern {
    fn to_shader(&self) -> Option<skia_safe::Shader> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let info = ImageInfo::new(
            (self.width as i32, self.height as i32),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        );
        let row_bytes = self.width as usize * 4;
        let data = skia_safe::Data::new_copy(&self.rgba);
        let image = skia_safe::images::raster_from_data(&info, data, row_bytes)?;
        let (tx, ty) = self.repetition.tile_modes();
        image.to_shader(Some((tx, ty)), skia_safe::SamplingOptions::default(), None)
    }
}

impl FillStyle {
    fn color(&self) -> Color {
        match self {
            FillStyle::Solid(c) => *c,
            FillStyle::Gradient(_) | FillStyle::Pattern(_) => Color::BLACK,
        }
    }
}

#[derive(Clone)]
struct CanvasState {
    fill_style: FillStyle,
    stroke_style: FillStyle,
    line_width: f32,
    global_alpha: f32,
    transform: Matrix,
    /// Parsed CSS `font` shorthand. Stored pre-parsed so each draw
    /// operation doesn't re-parse the same string.
    font: ParsedFont,
    /// CSS `globalCompositeOperation` — maps to skia BlendMode when
    /// building paints. Stored as the parsed enum so we don't have to
    /// re-match on a string every draw call.
    global_composite_operation: BlendMode,
    /// CSS `ctx.shadow*` — Canvas 2D shadow pipeline. When any of
    /// blur/offset is non-zero AND shadow_color has non-zero alpha,
    /// each fill/stroke operation attaches a Skia drop_shadow image
    /// filter to its paint.
    shadow_blur: f32,
    shadow_offset_x: f32,
    shadow_offset_y: f32,
    shadow_color: Color,
    /// Parsed CSS `ctx.filter` chain. Each entry is a `FilterOp`; the
    /// chain is applied to every paint via `image_filters::compose`.
    filter_chain: Vec<FilterOp>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            fill_style: FillStyle::Solid(Color::BLACK),
            stroke_style: FillStyle::Solid(Color::BLACK),
            line_width: 1.0,
            global_alpha: 1.0,
            transform: Matrix::new_identity(),
            font: ParsedFont::default_font(),
            global_composite_operation: BlendMode::SrcOver,
            shadow_blur: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_color: Color::TRANSPARENT,
            filter_chain: Vec::new(),
        }
    }
}

/// Parsed `ctx.filter` operation. Each variant maps to a Skia
/// `image_filters` or `color_filters` builder; the list is composed
/// in order when building a paint.
#[derive(Debug, Clone, Copy)]
pub enum FilterOp {
    Blur(f32),
    Grayscale(f32),
    Sepia(f32),
    Invert(f32),
    Brightness(f32),
    Contrast(f32),
    Saturate(f32),
    Opacity(f32),
}

/// Map the CSS `globalCompositeOperation` keyword to a Skia `BlendMode`.
/// Unknown strings fall back to `SrcOver` (the Canvas 2D default).
pub fn parse_composite_operation(s: &str) -> BlendMode {
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

/// Parse a `ctx.filter` chain like `"blur(5px) grayscale(50%) brightness(1.2)"`
/// into an ordered list of `FilterOp`s. Unknown functions are silently
/// dropped, matching Chrome's forgiving behaviour on malformed filters.
pub fn parse_filter_chain(input: &str) -> Vec<FilterOp> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "none" {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut chars = trimmed.chars().peekable();
    while chars.peek().is_some() {
        // Skip whitespace between filters.
        while matches!(chars.peek(), Some(&c) if c.is_whitespace()) {
            chars.next();
        }
        // Read function name up to `(`.
        let mut name = String::new();
        while let Some(&c) = chars.peek() {
            if c == '(' {
                chars.next();
                break;
            }
            name.push(c);
            chars.next();
        }
        // Read argument up to `)`.
        let mut arg = String::new();
        while let Some(&c) = chars.peek() {
            chars.next();
            if c == ')' {
                break;
            }
            arg.push(c);
        }
        let op = match name.trim() {
            "blur" => FilterOp::Blur(parse_length_px(&arg).unwrap_or(0.0)),
            "grayscale" => FilterOp::Grayscale(parse_percent(&arg).unwrap_or(0.0)),
            "sepia" => FilterOp::Sepia(parse_percent(&arg).unwrap_or(0.0)),
            "invert" => FilterOp::Invert(parse_percent(&arg).unwrap_or(0.0)),
            "brightness" => FilterOp::Brightness(parse_percent(&arg).unwrap_or(1.0)),
            "contrast" => FilterOp::Contrast(parse_percent(&arg).unwrap_or(1.0)),
            "saturate" => FilterOp::Saturate(parse_percent(&arg).unwrap_or(1.0)),
            "opacity" => FilterOp::Opacity(parse_percent(&arg).unwrap_or(1.0)),
            _ => continue,
        };
        out.push(op);
    }
    out
}

fn parse_length_px(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(v) = t.strip_suffix("px") {
        return v.trim().parse().ok();
    }
    if let Some(v) = t.strip_suffix("pt") {
        return v.trim().parse::<f32>().ok().map(|n| n * 96.0 / 72.0);
    }
    t.parse().ok()
}

fn parse_percent(s: &str) -> Option<f32> {
    let t = s.trim();
    if let Some(v) = t.strip_suffix('%') {
        return v.trim().parse::<f32>().ok().map(|n| n / 100.0);
    }
    t.parse().ok()
}

/// Compose a `FilterOp` chain into a single `skia_safe::ImageFilter`,
/// optionally layered on top of an existing filter (typically the
/// drop-shadow from the canvas state).
fn compose_filter_chain(
    chain: &[FilterOp],
    mut inner: Option<skia_safe::ImageFilter>,
) -> Option<skia_safe::ImageFilter> {
    for op in chain {
        let next = match op {
            FilterOp::Blur(radius) => {
                // CSS `blur(r)` uses Gaussian radius; Skia takes sigma.
                // Chrome's conversion is sigma ≈ radius / 2.
                let sigma = radius / 2.0;
                if sigma > 0.0 {
                    image_filters::blur((sigma, sigma), TileMode::Decal, inner.take(), None)
                } else {
                    inner.take()
                }
            }
            FilterOp::Grayscale(amount) => {
                let cf = grayscale_filter(amount.clamp(0.0, 1.0));
                image_filters::color_filter(cf, inner.take(), None)
            }
            FilterOp::Sepia(amount) => {
                let cf = sepia_filter(amount.clamp(0.0, 1.0));
                image_filters::color_filter(cf, inner.take(), None)
            }
            FilterOp::Invert(amount) => {
                let cf = invert_filter(amount.clamp(0.0, 1.0));
                image_filters::color_filter(cf, inner.take(), None)
            }
            FilterOp::Brightness(amount) => {
                let cf = brightness_filter(amount.max(0.0));
                image_filters::color_filter(cf, inner.take(), None)
            }
            FilterOp::Contrast(amount) => {
                let cf = contrast_filter(amount.max(0.0));
                image_filters::color_filter(cf, inner.take(), None)
            }
            FilterOp::Saturate(amount) => {
                let cf = saturate_filter(amount.max(0.0));
                image_filters::color_filter(cf, inner.take(), None)
            }
            FilterOp::Opacity(amount) => {
                let cf = opacity_filter(amount.clamp(0.0, 1.0));
                image_filters::color_filter(cf, inner.take(), None)
            }
        };
        if let Some(filter) = next {
            inner = Some(filter);
        }
    }
    inner
}

/// Build a 4x5 colour matrix ColorFilter (Skia's `color_matrix`).
/// Matrix layout is row-major, rows = RGBA output components, columns
/// = (R, G, B, A, translation). Exactly matches the CSS filter spec's
/// feColorMatrix primitives.
fn matrix_color_filter(m: [f32; 20]) -> ColorFilter {
    // `None` clamp uses Skia's default (premultiplied-aware). Matches
    // the CSS filter spec's feColorMatrix primitive for the filters we
    // build (grayscale, sepia, brightness, contrast, etc.).
    skia_safe::color_filters::matrix_row_major(&m, None)
}

/// CSS `grayscale(x)` — linear interpolation between identity and the
/// NTSC luminance matrix. See
/// https://www.w3.org/TR/filter-effects-1/#grayscaleEquivalent.
fn grayscale_filter(amount: f32) -> ColorFilter {
    let a = amount;
    let r = 0.2126 + 0.7874 * (1.0 - a);
    let rg = 0.7152 - 0.7152 * (1.0 - a);
    let rb = 0.0722 - 0.0722 * (1.0 - a);
    let gr = 0.2126 - 0.2126 * (1.0 - a);
    let g = 0.7152 + 0.2848 * (1.0 - a);
    let gb = 0.0722 - 0.0722 * (1.0 - a);
    let br = 0.2126 - 0.2126 * (1.0 - a);
    let bg = 0.7152 - 0.7152 * (1.0 - a);
    let b = 0.0722 + 0.9278 * (1.0 - a);
    matrix_color_filter([
        r, rg, rb, 0.0, 0.0, gr, g, gb, 0.0, 0.0, br, bg, b, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    ])
}

/// CSS `sepia(x)` per the filter spec.
fn sepia_filter(amount: f32) -> ColorFilter {
    let a = amount;
    let r = 0.393 + 0.607 * (1.0 - a);
    let rg = 0.769 - 0.769 * (1.0 - a);
    let rb = 0.189 - 0.189 * (1.0 - a);
    let gr = 0.349 - 0.349 * (1.0 - a);
    let g = 0.686 + 0.314 * (1.0 - a);
    let gb = 0.168 - 0.168 * (1.0 - a);
    let br = 0.272 - 0.272 * (1.0 - a);
    let bg = 0.534 - 0.534 * (1.0 - a);
    let b = 0.131 + 0.869 * (1.0 - a);
    matrix_color_filter([
        r, rg, rb, 0.0, 0.0, gr, g, gb, 0.0, 0.0, br, bg, b, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    ])
}

/// CSS `invert(x)` — linear interpolation towards (1 - c).
fn invert_filter(amount: f32) -> ColorFilter {
    let a = amount;
    let diag = 1.0 - 2.0 * a;
    let off = a;
    matrix_color_filter([
        diag, 0.0, 0.0, 0.0, off, 0.0, diag, 0.0, 0.0, off, 0.0, 0.0, diag, 0.0, off, 0.0, 0.0,
        0.0, 1.0, 0.0,
    ])
}

/// CSS `brightness(x)` — scalar multiply of RGB.
fn brightness_filter(amount: f32) -> ColorFilter {
    matrix_color_filter([
        amount, 0.0, 0.0, 0.0, 0.0, 0.0, amount, 0.0, 0.0, 0.0, 0.0, 0.0, amount, 0.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
    ])
}

/// CSS `contrast(x)` — centred scalar around 0.5.
fn contrast_filter(amount: f32) -> ColorFilter {
    let a = amount;
    let t = 0.5 * (1.0 - a);
    matrix_color_filter([
        a, 0.0, 0.0, 0.0, t, 0.0, a, 0.0, 0.0, t, 0.0, 0.0, a, 0.0, t, 0.0, 0.0, 0.0, 1.0, 0.0,
    ])
}

/// CSS `saturate(x)` — feColorMatrix "saturate" primitive.
fn saturate_filter(amount: f32) -> ColorFilter {
    // Reference: https://www.w3.org/TR/filter-effects-1/#feColorMatrixElement
    let a = amount;
    let r = 0.213 + 0.787 * a;
    let rg = 0.715 - 0.715 * a;
    let rb = 0.072 - 0.072 * a;
    let gr = 0.213 - 0.213 * a;
    let g = 0.715 + 0.285 * a;
    let gb = 0.072 - 0.072 * a;
    let br = 0.213 - 0.213 * a;
    let bg = 0.715 - 0.715 * a;
    let b = 0.072 + 0.928 * a;
    matrix_color_filter([
        r, rg, rb, 0.0, 0.0, gr, g, gb, 0.0, 0.0, br, bg, b, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    ])
}

/// CSS `opacity(x)` — alpha channel scale.
fn opacity_filter(amount: f32) -> ColorFilter {
    matrix_color_filter([
        1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        amount, 0.0,
    ])
}

/// CPU-based Canvas 2D rendering context backed by Skia.
pub struct Canvas2D {
    /// Premultiplied RGBA8 pixel buffer, `width * height * 4` bytes.
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    state: CanvasState,
    state_stack: Vec<CanvasState>,
    path: Path2D,
}

impl Canvas2D {
    pub fn new(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let pixels = vec![0u8; (width as usize) * (height as usize) * 4];
        Some(Self {
            pixels,
            width,
            height,
            state: CanvasState::default(),
            state_stack: Vec::new(),
            path: Path2D::new(),
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Raw premultiplied-RGBA pixel buffer. Used for PNG encoding and
    /// direct compositing from `text::rasterize_text`.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    // --- State ---

    pub fn save(&mut self) {
        self.state_stack.push(self.state.clone());
    }

    pub fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.state = state;
        }
    }

    // --- Style ---

    pub fn set_fill_color(&mut self, r: u8, g: u8, b: u8, a: f32) {
        self.state.fill_style = FillStyle::Solid(Color::from_rgba8(
            r,
            g,
            b,
            (a.clamp(0.0, 1.0) * 255.0) as u8,
        ));
    }

    pub fn set_fill_color_str(&mut self, color_str: &str) {
        if let Some(c) = parse_css_color(color_str) {
            self.state.fill_style = FillStyle::Solid(c);
        }
    }

    pub fn set_stroke_color(&mut self, r: u8, g: u8, b: u8, a: f32) {
        self.state.stroke_style = FillStyle::Solid(Color::from_rgba8(
            r,
            g,
            b,
            (a.clamp(0.0, 1.0) * 255.0) as u8,
        ));
    }

    pub fn set_stroke_color_str(&mut self, color_str: &str) {
        if let Some(c) = parse_css_color(color_str) {
            self.state.stroke_style = FillStyle::Solid(c);
        }
    }

    pub fn set_fill_gradient(&mut self, gradient: Gradient) {
        self.state.fill_style = FillStyle::Gradient(gradient);
    }

    pub fn set_stroke_gradient(&mut self, gradient: Gradient) {
        self.state.stroke_style = FillStyle::Gradient(gradient);
    }

    pub fn set_fill_pattern(&mut self, pattern: Pattern) {
        self.state.fill_style = FillStyle::Pattern(pattern);
    }

    pub fn set_stroke_pattern(&mut self, pattern: Pattern) {
        self.state.stroke_style = FillStyle::Pattern(pattern);
    }

    pub fn set_line_width(&mut self, width: f32) {
        self.state.line_width = width;
    }

    pub fn set_global_alpha(&mut self, alpha: f32) {
        self.state.global_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Set `globalCompositeOperation` from a CSS string (e.g. `"multiply"`,
    /// `"source-over"`). Unknown values revert to `source-over`.
    pub fn set_global_composite_operation(&mut self, op: &str) {
        self.state.global_composite_operation = parse_composite_operation(op);
    }

    // --- Shadow ---

    pub fn set_shadow_blur(&mut self, blur: f32) {
        self.state.shadow_blur = blur.max(0.0);
    }

    pub fn set_shadow_offset_x(&mut self, x: f32) {
        self.state.shadow_offset_x = x;
    }

    pub fn set_shadow_offset_y(&mut self, y: f32) {
        self.state.shadow_offset_y = y;
    }

    pub fn set_shadow_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.state.shadow_color = Color::from_rgba8(r, g, b, a);
    }

    pub fn set_shadow_color_str(&mut self, color_str: &str) {
        if let Some(c) = parse_css_color(color_str) {
            self.state.shadow_color = c;
        }
    }

    // --- Filter chain ---

    /// Set `ctx.filter = "blur(5px) grayscale(50%)"`. Empty string or
    /// `"none"` clears the chain.
    pub fn set_filter(&mut self, filter_str: &str) {
        self.state.filter_chain = parse_filter_chain(filter_str);
    }

    pub fn set_font(&mut self, font_str: &str) {
        // Parse the full CSS `font` shorthand via the dedicated parser.
        // Unrecognised strings keep the previous font rather than
        // silently reverting to defaults (matches Chrome — setting
        // an invalid font leaves the current font unchanged).
        if let Some(parsed) = ParsedFont::parse(font_str) {
            self.state.font = parsed;
        }
    }

    // --- Transform ---

    pub fn translate(&mut self, x: f32, y: f32) {
        self.state.transform.pre_translate((x, y));
    }

    pub fn rotate(&mut self, angle: f32) {
        self.state.transform.pre_rotate(angle.to_degrees(), None);
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.state.transform.pre_scale((sx, sy), None);
    }

    pub fn set_transform(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) {
        // Canvas 2D's setTransform(a, b, c, d, e, f) matches Skia's
        // row-major Matrix.set_9 where the affine 2x3 lives in the top
        // two rows: [a c e / b d f / 0 0 1].
        let mut m = Matrix::new_identity();
        m.set_9(&[a, c, e, b, d, f, 0.0, 0.0, 1.0]);
        self.state.transform = m;
    }

    pub fn reset_transform(&mut self) {
        self.state.transform = Matrix::new_identity();
    }

    // --- Rectangle ops ---

    pub fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let rect = SkRect::from_xywh(x, y, w, h);
        let paint = self.build_fill_paint();
        let matrix = self.state.transform;
        self.with_canvas(|canvas| {
            canvas.save();
            canvas.concat(&matrix);
            canvas.draw_rect(rect, &paint);
            canvas.restore();
        });
    }

    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let rect = SkRect::from_xywh(x, y, w, h);
        let paint = self.build_stroke_paint();
        let matrix = self.state.transform;
        self.with_canvas(|canvas| {
            canvas.save();
            canvas.concat(&matrix);
            canvas.draw_rect(rect, &paint);
            canvas.restore();
        });
    }

    pub fn clear_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let rect = SkRect::from_xywh(x, y, w, h);
        let mut paint = Paint::new(Color4f::new(0.0, 0.0, 0.0, 0.0), None);
        paint.set_blend_mode(skia_safe::BlendMode::Clear);
        let matrix = self.state.transform;
        self.with_canvas(|canvas| {
            canvas.save();
            canvas.concat(&matrix);
            canvas.draw_rect(rect, &paint);
            canvas.restore();
        });
    }

    // --- Path ops ---

    pub fn begin_path(&mut self) {
        self.path.clear();
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(x, y);
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(x, y);
    }

    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        self.path.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, x, y);
    }

    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.path.quadratic_curve_to(cpx, cpy, x, y);
    }

    pub fn arc(&mut self, x: f32, y: f32, r: f32, start: f32, end: f32, ccw: bool) {
        self.path.arc(x, y, r, start, end, ccw);
    }

    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) {
        self.path.arc_to(x1, y1, x2, y2, radius);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rotation: f32,
        start: f32,
        end: f32,
        ccw: bool,
    ) {
        self.path.ellipse(cx, cy, rx, ry, rotation, start, end, ccw);
    }

    pub fn close_path(&mut self) {
        self.path.close_path();
    }

    pub fn fill(&mut self) {
        let Some(sk_path) = self.path.to_skia_path() else {
            return;
        };
        let paint = self.build_fill_paint();
        let matrix = self.state.transform;
        self.with_canvas(|canvas| {
            canvas.save();
            canvas.concat(&matrix);
            canvas.draw_path(&sk_path, &paint);
            canvas.restore();
        });
    }

    pub fn stroke(&mut self) {
        let Some(sk_path) = self.path.to_skia_path() else {
            return;
        };
        let paint = self.build_stroke_paint();
        let matrix = self.state.transform;
        self.with_canvas(|canvas| {
            canvas.save();
            canvas.concat(&matrix);
            canvas.draw_path(&sk_path, &paint);
            canvas.restore();
        });
    }

    // --- Pixel ops ---

    /// Get RGBA pixel data for a region (non-premultiplied, matching
    /// Canvas 2D's `getImageData` semantics).
    pub fn get_image_data(&self, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in y..(y + h).min(self.height) {
            for col in x..(x + w).min(self.width) {
                let offset = ((row * self.width + col) * 4) as usize;
                if offset + 3 >= self.pixels.len() {
                    out.extend_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                // Stored premultiplied — unpremultiply for the public API.
                let r = self.pixels[offset];
                let g = self.pixels[offset + 1];
                let b = self.pixels[offset + 2];
                let a = self.pixels[offset + 3];
                if a > 0 {
                    out.push(((r as u16 * 255 + a as u16 / 2) / a as u16) as u8);
                    out.push(((g as u16 * 255 + a as u16 / 2) / a as u16) as u8);
                    out.push(((b as u16 * 255 + a as u16 / 2) / a as u16) as u8);
                    out.push(a);
                } else {
                    out.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        out
    }

    /// Write RGBA pixel data (non-premultiplied) at a position.
    pub fn put_image_data(&mut self, data: &[u8], x: u32, y: u32, w: u32, h: u32) {
        let stride = self.width as usize * 4;
        for row in 0..h.min(self.height.saturating_sub(y)) {
            for col in 0..w.min(self.width.saturating_sub(x)) {
                let src_offset = ((row * w + col) * 4) as usize;
                let dst_offset = ((y + row) as usize * stride) + ((x + col) as usize * 4);
                if src_offset + 3 >= data.len() || dst_offset + 3 >= self.pixels.len() {
                    continue;
                }
                // Premultiply before storing.
                let r = data[src_offset];
                let g = data[src_offset + 1];
                let b = data[src_offset + 2];
                let a = data[src_offset + 3];
                self.pixels[dst_offset] = ((r as u16 * a as u16) / 255) as u8;
                self.pixels[dst_offset + 1] = ((g as u16 * a as u16) / 255) as u8;
                self.pixels[dst_offset + 2] = ((b as u16 * a as u16) / 255) as u8;
                self.pixels[dst_offset + 3] = a;
            }
        }
    }

    // --- Text (real shaping via rustybuzz + raster via swash) ---

    /// Measure text width in CSS pixels (Canvas 2D `measureText().width`).
    pub fn measure_text(&self, text: &str) -> f64 {
        text::measure_text_width(text, &self.state.font)
    }

    /// Full 13-field `TextMetrics` object for
    /// `CanvasRenderingContext2D.measureText`.
    pub fn measure_text_metrics(&self, text: &str) -> TextMetrics {
        text::measure_text_metrics(text, &self.state.font)
    }

    /// Fill text at `(x, y)`, with `y` interpreted as the alphabetic
    /// baseline (Canvas 2D's default `textBaseline`). Shapes via
    /// rustybuzz, rasterizes glyphs via swash, and composites the
    /// alpha masks onto the canvas pixel buffer using
    /// premultiplied-alpha SOURCE_OVER.
    pub fn fill_text(&mut self, text: &str, x: f32, y: f32) {
        let color = self.state.fill_style.color();
        let glyphs = text::rasterize_text(
            text,
            x,
            y,
            &self.state.font,
            color.r,
            color.g,
            color.b,
            self.state.global_alpha * color.alpha(),
        );

        let w = self.width;
        let h = self.height;
        for glyph in &glyphs {
            text::composite_glyph(glyph, &mut self.pixels, w, h);
        }
    }

    /// Stroke text at `(x, y)` (alphabetic baseline). Builds a Path2D
    /// from glyph outlines via ttf-parser, then strokes that path with
    /// the current `strokeStyle` / `lineWidth` / cap / join. Mirrors
    /// Chrome's Skia-based strokeText: glyphs are filled by tracing
    /// their contours, not by rasterizing-and-edge-detecting.
    pub fn stroke_text(&mut self, text: &str, x: f32, y: f32) {
        let mut path = crate::path::Path2D::new();
        let any = text::append_text_outline_to_path(&mut path, text, x, y, &self.state.font);
        if !any {
            return;
        }
        let Some(sk_path) = path.to_skia_path() else {
            return;
        };
        let paint = self.build_stroke_paint();
        let matrix = self.state.transform;
        self.with_canvas(|canvas| {
            canvas.save();
            canvas.concat(&matrix);
            canvas.draw_path(&sk_path, &paint);
            canvas.restore();
        });
    }

    // --- Image compositing ---

    /// Draw RGBA pixel data onto the canvas at a position.
    pub fn draw_image_pixels(&mut self, rgba: &[u8], src_w: u32, src_h: u32, dx: f32, dy: f32) {
        self.put_image_data(rgba, dx as u32, dy as u32, src_w, src_h);
    }

    /// Decode image bytes (PNG/JPEG) and return RGBA pixels + dimensions.
    pub fn decode_image(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
        let img = image::load_from_memory(bytes).ok()?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        Some((rgba.into_raw(), w, h))
    }

    // --- Encoding ---

    /// Encode the canvas as a PNG data URL.
    pub fn to_data_url(&self) -> String {
        let png_bytes = self.to_png_bytes();
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &png_bytes);
        format!("data:image/png;base64,{}", b64)
    }

    /// Encode with tiny invisible noise to break deterministic fingerprinting.
    pub fn to_data_url_with_jitter(&self) -> String {
        let mut pixels = self.get_image_data(0, 0, self.width, self.height);
        if !pixels.is_empty() {
            let mut rng = 0x9e3779b9u32;
            for i in (0..pixels.len()).step_by(4) {
                rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                if (rng % 100) < 5 {
                    // Jitter RGB channels by +/- 1
                    pixels[i] = pixels[i].wrapping_add((rng & 1) as u8);
                    pixels[i + 1] = pixels[i + 1].wrapping_sub(((rng >> 1) & 1) as u8);
                    pixels[i + 2] = pixels[i + 2].wrapping_add(((rng >> 2) & 1) as u8);
                }
            }
        }
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("PNG header write failed");
            writer
                .write_image_data(&pixels)
                .expect("PNG data write failed");
        }
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &buf);
        format!("data:image/png;base64,{}", b64)
    }

    /// Encode the canvas as PNG bytes (non-premultiplied RGBA).
    pub fn to_png_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("PNG header write failed");
            let unpremultiplied = self.get_image_data(0, 0, self.width, self.height);
            writer
                .write_image_data(&unpremultiplied)
                .expect("PNG data write failed");
        }
        buf
    }

    /// Check if any pixels have been drawn (non-transparent).
    pub fn has_content(&self) -> bool {
        self.pixels.chunks(4).any(|px| px[3] > 0)
    }

    // --- Internal helpers ---

    /// Run a closure with access to a Skia canvas that draws directly into
    /// the pixel buffer. The surface is created fresh for each call so the
    /// lifetime of the Vec and the Surface don't conflict.
    fn with_canvas<F>(&mut self, f: F)
    where
        F: FnOnce(&SkCanvas),
    {
        let info = ImageInfo::new(
            (self.width as i32, self.height as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let row_bytes = self.width as usize * 4;
        let Some(mut surface) =
            surfaces::wrap_pixels(&info, &mut self.pixels, Some(row_bytes), None)
        else {
            return;
        };
        f(surface.canvas());
    }

    fn build_fill_paint(&self) -> Paint {
        self.build_paint(&self.state.fill_style, PaintStyle::Fill, 0.0)
    }

    fn build_stroke_paint(&self) -> Paint {
        self.build_paint(
            &self.state.stroke_style,
            PaintStyle::Stroke,
            self.state.line_width,
        )
    }

    fn build_paint(&self, style: &FillStyle, paint_style: PaintStyle, stroke_width: f32) -> Paint {
        let mut paint = match style {
            FillStyle::Solid(color) => Paint::new(color.to_color4f(), None),
            FillStyle::Gradient(_) | FillStyle::Pattern(_) => {
                Paint::new(Color4f::new(0.0, 0.0, 0.0, 1.0), None)
            }
        };
        paint.set_anti_alias(true);
        paint.set_style(paint_style);
        if paint_style == PaintStyle::Stroke {
            paint.set_stroke_width(stroke_width);
        }
        match style {
            FillStyle::Gradient(grad) => {
                if let Some(shader) = grad.to_shader() {
                    paint.set_shader(shader);
                }
            }
            FillStyle::Pattern(pat) => {
                if let Some(shader) = pat.to_shader() {
                    paint.set_shader(shader);
                }
            }
            FillStyle::Solid(_) => {}
        }
        // `globalAlpha` applies on top of the per-style alpha.
        let base_alpha = paint.alpha_f();
        paint.set_alpha_f(base_alpha * self.state.global_alpha);

        // `globalCompositeOperation`.
        paint.set_blend_mode(self.state.global_composite_operation);

        // Shadow: attach a drop_shadow image filter when any shadow
        // attribute is active. Composed with the user filter chain
        // below so both effects stack cleanly.
        let shadow_active = (self.state.shadow_blur > 0.0
            || self.state.shadow_offset_x != 0.0
            || self.state.shadow_offset_y != 0.0)
            && self.state.shadow_color.a > 0;

        let mut img_filter = None;
        if shadow_active {
            let sigma = self.state.shadow_blur / 2.0;
            let shadow_col = skia_safe::Color::from_argb(
                self.state.shadow_color.a,
                self.state.shadow_color.r,
                self.state.shadow_color.g,
                self.state.shadow_color.b,
            );
            img_filter = image_filters::drop_shadow(
                (self.state.shadow_offset_x, self.state.shadow_offset_y),
                (sigma, sigma),
                shadow_col,
                None, // default color space
                None, // no input filter (draws against source)
                None, // default crop rect
            );
        }

        // Apply the filter chain on top of any shadow filter.
        if !self.state.filter_chain.is_empty() {
            img_filter = compose_filter_chain(&self.state.filter_chain, img_filter);
        }

        if let Some(f) = img_filter {
            paint.set_image_filter(f);
        }

        paint
    }
}

/// Construct a Color for external callers (canvas_ext.rs) without leaking
/// the underlying Skia type.
pub fn make_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// Parse a CSS color string into our public Color type.
fn parse_css_color(s: &str) -> Option<Color> {
    let s = s.trim();
    match s {
        "black" => Some(Color::from_rgba8(0, 0, 0, 255)),
        "white" => Some(Color::from_rgba8(255, 255, 255, 255)),
        "red" => Some(Color::from_rgba8(255, 0, 0, 255)),
        "green" => Some(Color::from_rgba8(0, 128, 0, 255)),
        "blue" => Some(Color::from_rgba8(0, 0, 255, 255)),
        "yellow" => Some(Color::from_rgba8(255, 255, 0, 255)),
        "cyan" => Some(Color::from_rgba8(0, 255, 255, 255)),
        "magenta" => Some(Color::from_rgba8(255, 0, 255, 255)),
        "transparent" => Some(Color::from_rgba8(0, 0, 0, 0)),
        s if s.starts_with('#') => parse_hex_color(s),
        s if s.starts_with("rgb") => parse_rgb_color(s),
        _ => None,
    }
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = &s[1..];
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(Color::from_rgba8(r, g, b, 255))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, 255))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, a))
        }
        _ => None,
    }
}

fn parse_rgb_color(s: &str) -> Option<Color> {
    let inner = s
        .strip_prefix("rgba(")
        .or_else(|| s.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').collect();
    if parts.len() >= 3 {
        let r: u8 = parts[0].trim().parse().ok()?;
        let g: u8 = parts[1].trim().parse().ok()?;
        let b: u8 = parts[2].trim().parse().ok()?;
        let a = if parts.len() >= 4 {
            (parts[3].trim().parse::<f32>().ok()? * 255.0) as u8
        } else {
            255
        };
        Some(Color::from_rgba8(r, g, b, a))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_canvas() {
        let c = Canvas2D::new(200, 100).unwrap();
        assert_eq!(c.width(), 200);
        assert_eq!(c.height(), 100);
        assert!(!c.has_content());
    }

    #[test]
    fn fill_rect_produces_pixels() {
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_fill_color(255, 0, 0, 1.0);
        c.fill_rect(10.0, 10.0, 50.0, 50.0);
        assert!(c.has_content());
    }

    #[test]
    fn clear_rect_clears() {
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_fill_color(255, 0, 0, 1.0);
        c.fill_rect(0.0, 0.0, 100.0, 100.0);
        assert!(c.has_content());
        c.clear_rect(0.0, 0.0, 100.0, 100.0);
        assert!(!c.has_content());
    }

    #[test]
    fn path_fill() {
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_fill_color(0, 0, 255, 1.0);
        c.begin_path();
        c.move_to(10.0, 10.0);
        c.line_to(90.0, 10.0);
        c.line_to(50.0, 90.0);
        c.close_path();
        c.fill();
        assert!(c.has_content());
    }

    #[test]
    fn stroke_rect() {
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_stroke_color(0, 255, 0, 1.0);
        c.set_line_width(2.0);
        c.stroke_rect(10.0, 10.0, 80.0, 80.0);
        assert!(c.has_content());
    }

    #[test]
    fn get_image_data_red() {
        let mut c = Canvas2D::new(10, 10).unwrap();
        c.set_fill_color(255, 0, 0, 1.0);
        c.fill_rect(0.0, 0.0, 10.0, 10.0);
        let data = c.get_image_data(0, 0, 10, 10);
        assert_eq!(data.len(), 10 * 10 * 4);
        assert_eq!(data[0], 255);
        assert_eq!(data[1], 0);
        assert_eq!(data[2], 0);
        assert_eq!(data[3], 255);
    }

    #[test]
    fn to_data_url() {
        let mut c = Canvas2D::new(10, 10).unwrap();
        c.set_fill_color(255, 0, 0, 1.0);
        c.fill_rect(0.0, 0.0, 10.0, 10.0);
        let url = c.to_data_url();
        assert!(url.starts_with("data:image/png;base64,"));
        assert!(url.len() > 30);
    }

    #[test]
    fn to_png_bytes_magic() {
        let mut c = Canvas2D::new(10, 10).unwrap();
        c.set_fill_color(0, 0, 255, 1.0);
        c.fill_rect(0.0, 0.0, 10.0, 10.0);
        let bytes = c.to_png_bytes();
        assert_eq!(&bytes[0..4], &[0x89, 0x50, 0x4e, 0x47]);
    }

    #[test]
    fn fill_text_produces_content() {
        let mut c = Canvas2D::new(200, 50).unwrap();
        c.set_fill_color(0, 0, 0, 1.0);
        c.set_font("16px sans-serif");
        c.fill_text("Hello World", 10.0, 30.0);
        assert!(c.has_content());
    }

    #[test]
    fn measure_text_nonzero() {
        let c = Canvas2D::new(100, 100).unwrap();
        let width = c.measure_text("Hello");
        assert!(width > 0.0);
    }

    #[test]
    fn save_restore() {
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_fill_color(255, 0, 0, 1.0);
        c.save();
        c.set_fill_color(0, 0, 255, 1.0);
        c.restore();
        c.fill_rect(0.0, 0.0, 10.0, 10.0);
        let data = c.get_image_data(0, 0, 1, 1);
        assert_eq!(data[0], 255);
    }

    #[test]
    fn css_color_parsing() {
        assert!(parse_css_color("red").is_some());
        assert!(parse_css_color("#ff0000").is_some());
        assert!(parse_css_color("#f00").is_some());
        assert!(parse_css_color("rgb(255, 0, 0)").is_some());
        assert!(parse_css_color("rgba(255, 0, 0, 0.5)").is_some());
    }

    #[test]
    fn fill_text_renders_real_glyphs() {
        let mut text_canvas = Canvas2D::new(200, 50).unwrap();
        text_canvas.set_fill_color(0, 0, 0, 1.0);
        text_canvas.set_font("16px sans-serif");
        text_canvas.fill_text("Hello World", 10.0, 30.0);

        let mut rect_canvas = Canvas2D::new(200, 50).unwrap();
        rect_canvas.set_fill_color(0, 0, 0, 1.0);
        rect_canvas.fill_rect(10.0, 14.0, 80.0, 16.0);

        assert!(text_canvas.has_content());
        assert!(rect_canvas.has_content());

        let text_pixels = text_canvas.get_image_data(0, 0, 200, 50);
        let rect_pixels = rect_canvas.get_image_data(0, 0, 200, 50);
        assert_ne!(text_pixels, rect_pixels);
    }

    #[test]
    fn measure_text_varies_by_font_size() {
        let mut c1 = Canvas2D::new(100, 100).unwrap();
        c1.set_font("10px sans-serif");
        let w1 = c1.measure_text("Hello");

        let mut c2 = Canvas2D::new(100, 100).unwrap();
        c2.set_font("20px sans-serif");
        let w2 = c2.measure_text("Hello");

        assert!(w2 > w1);
    }

    #[test]
    fn measure_text_varies_by_content() {
        let c = Canvas2D::new(100, 100).unwrap();
        let short = c.measure_text("Hi");
        let long = c.measure_text("Hello World");
        assert!(long > short);
    }

    #[test]
    fn fill_text_deterministic() {
        fn render() -> Vec<u8> {
            let mut c = Canvas2D::new(200, 50).unwrap();
            c.set_fill_color(0, 0, 0, 1.0);
            c.set_font("16px sans-serif");
            c.fill_text("Fingerprint", 10.0, 30.0);
            c.to_png_bytes()
        }
        let a = render();
        let b = render();
        assert_eq!(a, b);
    }

    #[test]
    fn different_text_different_output() {
        let mut c1 = Canvas2D::new(200, 50).unwrap();
        c1.set_fill_color(0, 0, 0, 1.0);
        c1.set_font("16px sans-serif");
        c1.fill_text("Hello", 10.0, 30.0);

        let mut c2 = Canvas2D::new(200, 50).unwrap();
        c2.set_fill_color(0, 0, 0, 1.0);
        c2.set_font("16px sans-serif");
        c2.fill_text("World", 10.0, 30.0);

        assert_ne!(c1.to_png_bytes(), c2.to_png_bytes());
    }

    #[test]
    fn deterministic_output() {
        fn render() -> Vec<u8> {
            let mut c = Canvas2D::new(50, 50).unwrap();
            c.set_fill_color(128, 64, 32, 1.0);
            c.fill_rect(5.0, 5.0, 40.0, 40.0);
            c.to_png_bytes()
        }
        let a = render();
        let b = render();
        assert_eq!(a, b);
    }

    #[test]
    fn linear_gradient_fill_rect() {
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_fill_gradient(Gradient::Linear {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 0.0,
            stops: vec![
                (0.0, Color::from_rgba8(255, 0, 0, 255)),
                (1.0, Color::from_rgba8(0, 0, 255, 255)),
            ],
        });
        c.fill_rect(0.0, 0.0, 100.0, 100.0);
        assert!(c.has_content());
        let left = c.get_image_data(5, 50, 1, 1);
        let right = c.get_image_data(95, 50, 1, 1);
        assert!(
            left[0] > left[2],
            "left more red: r={} b={}",
            left[0],
            left[2]
        );
        assert!(
            right[2] > right[0],
            "right more blue: r={} b={}",
            right[0],
            right[2]
        );
    }

    #[test]
    fn radial_gradient_fill_rect() {
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_fill_gradient(Gradient::Radial {
            x0: 50.0,
            y0: 50.0,
            r0: 0.0,
            x1: 50.0,
            y1: 50.0,
            r1: 50.0,
            stops: vec![
                (0.0, Color::from_rgba8(255, 255, 0, 255)),
                (1.0, Color::from_rgba8(0, 0, 128, 255)),
            ],
        });
        c.fill_rect(0.0, 0.0, 100.0, 100.0);
        assert!(c.has_content());
        let center = c.get_image_data(50, 50, 1, 1);
        let edge = c.get_image_data(0, 0, 1, 1);
        assert!(center[0] > edge[0], "center should have more red than edge");
    }

    /// Regression test for the Phase A radial-gradient bug: the previous
    /// tiny_skia backend could not honour `(x0, y0, r0)`, so a non-zero
    /// `r0` would be silently dropped. With skia-safe's
    /// `two_point_conical` we must see distinctly different output for
    /// `r0 = 0` vs `r0 = 20` with the same outer circle.
    #[test]
    fn radial_gradient_inner_radius_changes_output() {
        fn render(r0: f32) -> Vec<u8> {
            let mut c = Canvas2D::new(100, 100).unwrap();
            c.set_fill_gradient(Gradient::Radial {
                x0: 50.0,
                y0: 50.0,
                r0,
                x1: 50.0,
                y1: 50.0,
                r1: 50.0,
                stops: vec![
                    (0.0, Color::from_rgba8(255, 0, 0, 255)),
                    (1.0, Color::from_rgba8(0, 255, 0, 255)),
                ],
            });
            c.fill_rect(0.0, 0.0, 100.0, 100.0);
            c.to_png_bytes()
        }
        let a = render(0.0);
        let b = render(20.0);
        assert_ne!(
            a, b,
            "inner radius r0 must affect the rendered gradient, otherwise the \
             Phase A tiny_skia regression has returned"
        );
    }

    #[test]
    fn radial_gradient_offset_focal() {
        // Focal point at (25, 25) with r0=0, expanding out to (75, 75) r1=60.
        // This is the two-circle form that tiny_skia could not express.
        let mut c = Canvas2D::new(100, 100).unwrap();
        c.set_fill_gradient(Gradient::Radial {
            x0: 25.0,
            y0: 25.0,
            r0: 0.0,
            x1: 75.0,
            y1: 75.0,
            r1: 60.0,
            stops: vec![
                (0.0, Color::from_rgba8(255, 255, 255, 255)),
                (1.0, Color::from_rgba8(0, 0, 0, 255)),
            ],
        });
        c.fill_rect(0.0, 0.0, 100.0, 100.0);
        // Pixel near the focal point (25, 25) should be near-white; pixel
        // far from it (90, 90) should be near-black.
        let near = c.get_image_data(25, 25, 1, 1);
        let far = c.get_image_data(90, 90, 1, 1);
        assert!(
            near[0] > far[0],
            "focal pixel should be lighter: near.r={} far.r={}",
            near[0],
            far[0]
        );
    }

    #[test]
    fn gradient_resets_on_solid_color() {
        let mut c = Canvas2D::new(50, 50).unwrap();
        c.set_fill_gradient(Gradient::Linear {
            x0: 0.0,
            y0: 0.0,
            x1: 50.0,
            y1: 0.0,
            stops: vec![
                (0.0, Color::from_rgba8(255, 0, 0, 255)),
                (1.0, Color::from_rgba8(0, 0, 255, 255)),
            ],
        });
        c.set_fill_color(0, 255, 0, 1.0);
        c.fill_rect(0.0, 0.0, 50.0, 50.0);
        let px = c.get_image_data(25, 25, 1, 1);
        assert_eq!(px[1], 255, "should be solid green, not gradient");
    }

    // ---- T1.1 Phase B tests ----

    #[test]
    fn parse_composite_op_all_26() {
        let names = [
            "source-over",
            "source-in",
            "source-out",
            "source-atop",
            "destination-over",
            "destination-in",
            "destination-out",
            "destination-atop",
            "lighter",
            "copy",
            "xor",
            "multiply",
            "screen",
            "overlay",
            "darken",
            "lighten",
            "color-dodge",
            "color-burn",
            "hard-light",
            "soft-light",
            "difference",
            "exclusion",
            "hue",
            "saturation",
            "color",
            "luminosity",
        ];
        assert_eq!(names.len(), 26);
        for name in names {
            // Just make sure each one maps; SrcOver is the safe default
            // so we check that all except "source-over" resolve to
            // SOMETHING other than SrcOver.
            let bm = parse_composite_operation(name);
            if name != "source-over" {
                assert!(
                    bm != BlendMode::SrcOver,
                    "{name} should not fall back to SrcOver"
                );
            }
        }
    }

    #[test]
    fn multiply_blend_darkens_overlap() {
        let mut c = Canvas2D::new(40, 40).unwrap();
        // Start with a red background.
        c.set_fill_color(200, 200, 0, 1.0);
        c.fill_rect(0.0, 0.0, 40.0, 40.0);
        // Multiply a yellow square on top — output should be the
        // component-wise product (red stays at 200, green multiplies
        // to ~156, blue = 0).
        c.set_global_composite_operation("multiply");
        c.set_fill_color(255, 200, 0, 1.0);
        c.fill_rect(0.0, 0.0, 40.0, 40.0);
        let px = c.get_image_data(20, 20, 1, 1);
        // Reset is important so future tests start clean — but the
        // canvas is dropped after this test anyway.
        assert!(px[1] < 210, "multiply should darken green: {:?}", px);
    }

    #[test]
    fn shadow_blur_increases_ink_footprint() {
        // Render once with shadow_blur = 0 and once with blur = 10,
        // compare the number of non-transparent pixels. The blur
        // variant must paint strictly more pixels because its drop
        // shadow spreads ink beyond the rect's rasterized footprint.
        fn render(blur: f32) -> usize {
            let mut c = Canvas2D::new(80, 80).unwrap();
            c.set_fill_color(0, 0, 0, 1.0);
            if blur > 0.0 {
                c.set_shadow_color(255, 0, 0, 255);
                c.set_shadow_offset_x(20.0);
                c.set_shadow_offset_y(20.0);
                c.set_shadow_blur(blur);
            }
            c.fill_rect(10.0, 10.0, 20.0, 20.0);
            c.pixels().chunks(4).filter(|px| px[3] > 0).count()
        }
        let no_shadow = render(0.0);
        let with_shadow = render(15.0);
        assert!(
            with_shadow > no_shadow + 50,
            "shadow should paint notably more pixels: no_shadow={no_shadow} with={with_shadow}"
        );
    }

    #[test]
    fn filter_grayscale_collapses_color() {
        let mut c = Canvas2D::new(20, 20).unwrap();
        c.set_filter("grayscale(100%)");
        c.set_fill_color(255, 0, 0, 1.0);
        c.fill_rect(0.0, 0.0, 20.0, 20.0);
        let px = c.get_image_data(10, 10, 1, 1);
        // Pure red through full grayscale should become a gray with
        // R ≈ G ≈ B. Allow tolerance for color filter rounding.
        assert!(
            (px[0] as i32 - px[1] as i32).abs() < 20,
            "grayscale red should have R≈G, got {:?}",
            px
        );
        assert!(
            (px[1] as i32 - px[2] as i32).abs() < 20,
            "grayscale red should have G≈B, got {:?}",
            px
        );
    }

    #[test]
    fn parse_filter_chain_basic() {
        let chain = parse_filter_chain("blur(5px) grayscale(50%) brightness(1.2)");
        assert_eq!(chain.len(), 3);
        match chain[0] {
            FilterOp::Blur(r) => assert!((r - 5.0).abs() < 1e-6),
            _ => panic!("expected Blur"),
        }
        match chain[1] {
            FilterOp::Grayscale(a) => assert!((a - 0.5).abs() < 1e-6),
            _ => panic!("expected Grayscale"),
        }
        match chain[2] {
            FilterOp::Brightness(a) => assert!((a - 1.2).abs() < 1e-6),
            _ => panic!("expected Brightness"),
        }
    }

    #[test]
    fn parse_filter_chain_none_and_empty() {
        assert!(parse_filter_chain("").is_empty());
        assert!(parse_filter_chain("   ").is_empty());
        assert!(parse_filter_chain("none").is_empty());
    }

    #[test]
    fn conic_gradient_renders() {
        let mut c = Canvas2D::new(60, 60).unwrap();
        c.set_fill_gradient(Gradient::Conic {
            cx: 30.0,
            cy: 30.0,
            start_angle: 0.0,
            stops: vec![
                (0.0, Color::from_rgba8(255, 0, 0, 255)),
                (0.5, Color::from_rgba8(0, 255, 0, 255)),
                (1.0, Color::from_rgba8(0, 0, 255, 255)),
            ],
        });
        c.fill_rect(0.0, 0.0, 60.0, 60.0);
        assert!(c.has_content());
        // Red should dominate to the right of centre (angle 0°).
        let right = c.get_image_data(50, 30, 1, 1);
        // Blue should dominate above centre at angle ~270°.
        let top = c.get_image_data(30, 10, 1, 1);
        // Different positions along the sweep must show different
        // dominant channels.
        assert_ne!((right[0], right[1], right[2]), (top[0], top[1], top[2]));
    }

    #[test]
    fn pattern_repeat_tiles() {
        // 2x2 checkerboard pattern: red top-left, green top-right,
        // blue bottom-left, white bottom-right.
        let rgba: Vec<u8> = vec![
            255, 0, 0, 255, // (0, 0) red
            0, 255, 0, 255, // (1, 0) green
            0, 0, 255, 255, // (0, 1) blue
            255, 255, 255, 255, // (1, 1) white
        ];
        let pattern = Pattern {
            rgba,
            width: 2,
            height: 2,
            repetition: PatternRepetition::Repeat,
        };
        let mut c = Canvas2D::new(10, 10).unwrap();
        c.set_fill_pattern(pattern);
        c.fill_rect(0.0, 0.0, 10.0, 10.0);
        assert!(c.has_content(), "pattern should fill with repeating tiles");
        // There should be multiple distinct colours across the canvas
        // (not a single solid block).
        let tl = c.get_image_data(0, 0, 1, 1);
        let tr = c.get_image_data(1, 0, 1, 1);
        let bl = c.get_image_data(0, 1, 1, 1);
        assert_ne!(tl[0..3], tr[0..3]);
        assert_ne!(tl[0..3], bl[0..3]);
    }

    #[test]
    fn pattern_repetition_parse() {
        assert_eq!(
            PatternRepetition::parse("repeat"),
            PatternRepetition::Repeat
        );
        assert_eq!(PatternRepetition::parse(""), PatternRepetition::Repeat);
        assert_eq!(
            PatternRepetition::parse("repeat-x"),
            PatternRepetition::RepeatX
        );
        assert_eq!(
            PatternRepetition::parse("repeat-y"),
            PatternRepetition::RepeatY
        );
        assert_eq!(
            PatternRepetition::parse("no-repeat"),
            PatternRepetition::NoRepeat
        );
        assert_eq!(PatternRepetition::parse("bogus"), PatternRepetition::Repeat);
    }
}
