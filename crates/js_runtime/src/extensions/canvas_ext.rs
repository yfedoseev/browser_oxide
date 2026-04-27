use canvas::Canvas2D;
use deno_core::op2;
use serde::Serialize;
use std::collections::HashMap;

/// Decoded image data stored for drawImage.
pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Canvas state stored in OpState.
pub struct CanvasState {
    canvases: HashMap<i32, Canvas2D>,
    images: HashMap<i32, DecodedImage>,
    next_id: i32,
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            canvases: HashMap::new(),
            images: HashMap::new(),
            next_id: 1,
        }
    }
}

#[op2(fast)]
#[smi]
pub fn op_canvas_create(
    #[state] state: &mut CanvasState,
    #[smi] width: i32,
    #[smi] height: i32,
) -> i32 {
    tracing::debug!(width = width, height = height, "Canvas created");
    let id = state.next_id;
    state.next_id += 1;
    if let Some(canvas) = Canvas2D::new(width.max(1) as u32, height.max(1) as u32) {
        state.canvases.insert(id, canvas);
        id
    } else {
        -1
    }
}

#[op2(fast)]
pub fn op_canvas_fill_rect(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.fill_rect(x as f32, y as f32, w as f32, h as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_stroke_rect(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.stroke_rect(x as f32, y as f32, w as f32, h as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_clear_rect(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.clear_rect(x as f32, y as f32, w as f32, h as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_begin_path(#[state] state: &mut CanvasState, #[smi] id: i32) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.begin_path();
    }
}

#[op2(fast)]
pub fn op_canvas_move_to(#[state] state: &mut CanvasState, #[smi] id: i32, x: f64, y: f64) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.move_to(x as f32, y as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_line_to(#[state] state: &mut CanvasState, #[smi] id: i32, x: f64, y: f64) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.line_to(x as f32, y as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_fill(#[state] state: &mut CanvasState, #[smi] id: i32) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.fill();
    }
}

#[op2(fast)]
pub fn op_canvas_stroke(#[state] state: &mut CanvasState, #[smi] id: i32) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.stroke();
    }
}

#[op2(fast)]
pub fn op_canvas_fill_text(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    #[string] text: &str,
    x: f64,
    y: f64,
) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.fill_text(text, x as f32, y as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_set_fill_style(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    #[string] color: &str,
) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.set_fill_color_str(color);
    }
}

#[op2(fast)]
pub fn op_canvas_set_stroke_style(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    #[string] color: &str,
) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.set_stroke_color_str(color);
    }
}

#[op2(fast)]
pub fn op_canvas_set_font(#[state] state: &mut CanvasState, #[smi] id: i32, #[string] font: &str) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.set_font(font);
    }
}

#[op2(fast)]
pub fn op_canvas_set_line_width(#[state] state: &mut CanvasState, #[smi] id: i32, width: f64) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.set_line_width(width as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_set_global_alpha(#[state] state: &mut CanvasState, #[smi] id: i32, alpha: f64) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.set_global_alpha(alpha as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_save(#[state] state: &mut CanvasState, #[smi] id: i32) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.save();
    }
}

#[op2(fast)]
pub fn op_canvas_restore(#[state] state: &mut CanvasState, #[smi] id: i32) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.restore();
    }
}

#[op2(fast)]
pub fn op_canvas_translate(#[state] state: &mut CanvasState, #[smi] id: i32, x: f64, y: f64) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.translate(x as f32, y as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_rotate(#[state] state: &mut CanvasState, #[smi] id: i32, angle: f64) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.rotate(angle as f32);
    }
}

#[op2(fast)]
pub fn op_canvas_scale(#[state] state: &mut CanvasState, #[smi] id: i32, x: f64, y: f64) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.scale(x as f32, y as f32);
    }
}

#[op2]
#[string]
pub fn op_canvas_to_data_url(#[state] state: &CanvasState, #[smi] id: i32) -> String {
    tracing::debug!("Canvas to_data_url called");
    state
        .canvases
        .get(&id)
        .map(|c| {
            let mut pixels = c.get_image_data(0, 0, c.width(), c.height());
            // Add tiny, invisible jitter to the lowest bit of random pixels
            // to break deterministic canvas fingerprinting.
            if !pixels.is_empty() {
                let mut rng = 0x9e3779b9u32; // Deterministic-ish seed
                for i in (0..pixels.len()).step_by(4) {
                    rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
                    if (rng % 100) < 5 {
                        // Jitter 5% of pixels
                        pixels[i] = pixels[i].wrapping_add((rng & 1) as u8);
                        pixels[i + 1] = pixels[i + 1].wrapping_sub(((rng >> 1) & 1) as u8);
                        pixels[i + 2] = pixels[i + 2].wrapping_add(((rng >> 2) & 1) as u8);
                    }
                }
            }

            // Encode the jittered pixels to PNG base64
            // (Note: This requires a PNG encoder that can take raw RGBA)
            // For now, we'll use the existing to_data_url which uses tiny-skia's encoder.
            // To be truly SOTA we should encode our jittered buffer.

            // Falling back to standard for now as tiny-skia's Canvas2D doesn't
            // expose the raw buffer easily for re-encoding without extra crates.
            // Wait, Canvas2D is our own struct!
            c.to_data_url_with_jitter()
        })
        .unwrap_or_default()
}

#[op2(fast)]
pub fn op_canvas_measure_text(
    #[state] state: &CanvasState,
    #[smi] id: i32,
    #[string] text: &str,
) -> f64 {
    state
        .canvases
        .get(&id)
        .map(|c| c.measure_text(text))
        .unwrap_or(0.0)
}

/// Serialized 13-field `TextMetrics` shape for
/// `CanvasRenderingContext2D.measureText`.
#[derive(serde::Serialize)]
pub struct JsTextMetrics {
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

impl JsTextMetrics {
    fn from_canvas(m: canvas::text::TextMetrics) -> Self {
        Self {
            width: m.width,
            actual_bounding_box_left: m.actual_bounding_box_left,
            actual_bounding_box_right: m.actual_bounding_box_right,
            actual_bounding_box_ascent: m.actual_bounding_box_ascent,
            actual_bounding_box_descent: m.actual_bounding_box_descent,
            font_bounding_box_ascent: m.font_bounding_box_ascent,
            font_bounding_box_descent: m.font_bounding_box_descent,
            em_height_ascent: m.em_height_ascent,
            em_height_descent: m.em_height_descent,
            hanging_baseline: m.hanging_baseline,
            alphabetic_baseline: m.alphabetic_baseline,
            ideographic_baseline: m.ideographic_baseline,
        }
    }

    fn zero() -> Self {
        Self::from_canvas(canvas::text::TextMetrics::zero())
    }
}

/// Full 13-field TextMetrics measurement — what real Chrome returns
/// from `measureText`. Uses the shaped-glyph bounding box for the
/// `actual_bounding_box_*` fields, which is the signal fingerprinters
/// actually probe.
#[op2]
#[serde]
pub fn op_canvas_measure_text_full(
    #[state] state: &CanvasState,
    #[smi] id: i32,
    #[string] text: &str,
) -> JsTextMetrics {
    state
        .canvases
        .get(&id)
        .map(|c| JsTextMetrics::from_canvas(c.measure_text_metrics(text)))
        .unwrap_or_else(JsTextMetrics::zero)
}

/// Set fill style to a gradient.
/// gradient_type: "linear" or "radial"
/// coords: [x0, y0, x1, y1] for linear, [x0, y0, r0, x1, y1, r1] for radial
/// stops: JSON array of [offset, r, g, b, a] tuples
#[op2(fast)]
pub fn op_canvas_set_fill_gradient(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    #[string] gradient_type: &str,
    #[string] params_json: &str,
) {
    if let Some(grad) = parse_gradient(gradient_type, params_json) {
        if let Some(c) = state.canvases.get_mut(&id) {
            c.set_fill_gradient(grad);
        }
    }
}

fn parse_gradient(gradient_type: &str, json: &str) -> Option<canvas::canvas2d::Gradient> {
    let val: serde_json::Value = serde_json::from_str(json).ok()?;
    let coords = val.get("coords")?.as_array()?;
    let stops_arr = val.get("stops")?.as_array()?;

    let mut stops = Vec::new();
    for s in stops_arr {
        let offset = s.get(0)?.as_f64()? as f32;
        let r = s.get(1)?.as_f64()? as u8;
        let g = s.get(2)?.as_f64()? as u8;
        let b = s.get(3)?.as_f64()? as u8;
        let a = s.get(4).and_then(|v| v.as_f64()).unwrap_or(255.0) as u8;
        // Use canvas's parse_css_color equivalent — construct directly
        // Canvas2D uses tiny_skia::Color but we don't depend on tiny_skia here
        // Store as (offset, r, g, b, a) tuples and let Canvas2D convert
        stops.push((offset, canvas::canvas2d::make_color(r, g, b, a)));
    }

    match gradient_type {
        "linear" => Some(canvas::canvas2d::Gradient::Linear {
            x0: coords.get(0)?.as_f64()? as f32,
            y0: coords.get(1)?.as_f64()? as f32,
            x1: coords.get(2)?.as_f64()? as f32,
            y1: coords.get(3)?.as_f64()? as f32,
            stops,
        }),
        "radial" => Some(canvas::canvas2d::Gradient::Radial {
            x0: coords.get(0)?.as_f64()? as f32,
            y0: coords.get(1)?.as_f64()? as f32,
            r0: coords.get(2)?.as_f64()? as f32,
            x1: coords.get(3)?.as_f64()? as f32,
            y1: coords.get(4)?.as_f64()? as f32,
            r1: coords.get(5)?.as_f64()? as f32,
            stops,
        }),
        _ => None,
    }
}

/// Get image data (RGBA, non-premultiplied) from a canvas region.
#[op2]
#[serde]
pub fn op_canvas_get_image_data(
    #[state] state: &CanvasState,
    #[smi] id: i32,
    #[smi] x: i32,
    #[smi] y: i32,
    #[smi] w: i32,
    #[smi] h: i32,
) -> Vec<u8> {
    state
        .canvases
        .get(&id)
        .map(|c| c.get_image_data(x as u32, y as u32, w as u32, h as u32))
        .unwrap_or_default()
}

/// Put image data onto a canvas at a position.
#[op2(fast)]
pub fn op_canvas_put_image_data(
    #[state] state: &mut CanvasState,
    #[smi] id: i32,
    #[buffer] data: &[u8],
    #[smi] x: i32,
    #[smi] y: i32,
    #[smi] w: i32,
    #[smi] h: i32,
) {
    if let Some(c) = state.canvases.get_mut(&id) {
        c.put_image_data(data, x as u32, y as u32, w as u32, h as u32);
    }
}

/// Draw one canvas onto another (canvas-to-canvas compositing).
#[op2(fast)]
pub fn op_canvas_draw_image(
    #[state] state: &mut CanvasState,
    #[smi] dst_id: i32,
    #[smi] src_id: i32,
    dx: f64,
    dy: f64,
) {
    // Get source pixels
    let src_pixels = state.canvases.get(&src_id).map(|c| {
        (
            c.get_image_data(0, 0, c.width(), c.height()),
            c.width(),
            c.height(),
        )
    });
    if let Some((data, sw, sh)) = src_pixels {
        if let Some(dst) = state.canvases.get_mut(&dst_id) {
            dst.put_image_data(&data, dx as u32, dy as u32, sw, sh);
        }
    }
}

/// Decode image from a base64-encoded string, store in canvas state, return ID.
#[op2(fast)]
#[smi]
pub fn op_image_decode_base64(#[state] state: &mut CanvasState, #[string] b64: &str) -> i32 {
    let bytes = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64) {
        Ok(b) => b,
        Err(_) => return -1,
    };
    match canvas::Canvas2D::decode_image(&bytes) {
        Some((rgba, w, h)) => {
            let id = state.next_id;
            state.next_id += 1;
            state.images.insert(
                id,
                DecodedImage {
                    rgba,
                    width: w,
                    height: h,
                },
            );
            id
        }
        None => -1,
    }
}

/// Draw a decoded image onto a canvas.
#[op2(fast)]
pub fn op_canvas_draw_decoded_image(
    #[state] state: &mut CanvasState,
    #[smi] canvas_id: i32,
    #[smi] image_id: i32,
    dx: f64,
    dy: f64,
) {
    let img = match state.images.get(&image_id) {
        Some(i) => i,
        None => return,
    };
    let rgba = img.rgba.clone();
    let w = img.width;
    let h = img.height;
    if let Some(c) = state.canvases.get_mut(&canvas_id) {
        c.draw_image_pixels(&rgba, w, h, dx as f32, dy as f32);
    }
}

deno_core::extension!(
    canvas_extension,
    ops = [
        op_canvas_create,
        op_canvas_fill_rect,
        op_canvas_stroke_rect,
        op_canvas_clear_rect,
        op_canvas_begin_path,
        op_canvas_move_to,
        op_canvas_line_to,
        op_canvas_fill,
        op_canvas_stroke,
        op_canvas_fill_text,
        op_canvas_set_fill_style,
        op_canvas_set_stroke_style,
        op_canvas_set_font,
        op_canvas_set_line_width,
        op_canvas_set_global_alpha,
        op_canvas_save,
        op_canvas_restore,
        op_canvas_translate,
        op_canvas_rotate,
        op_canvas_scale,
        op_canvas_to_data_url,
        op_canvas_measure_text,
        op_canvas_measure_text_full,
        op_canvas_get_image_data,
        op_canvas_put_image_data,
        op_canvas_draw_image,
        op_canvas_set_fill_gradient,
        op_image_decode_base64,
        op_canvas_draw_decoded_image,
    ],
);
