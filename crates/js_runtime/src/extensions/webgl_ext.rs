//! WebGL rendering ops bridging JS to real OpenGL via OSMesa.
//!
//! When the `webgl-render` feature is enabled on the canvas crate and
//! libOSMesa is available, these ops provide real shader compilation,
//! draw calls, and pixel readback. Otherwise, the existing JS stubs are used.

use deno_core::op2;

#[cfg(feature = "webgl-render")]
use canvas::webgl_render::WebGLContext;
#[cfg(feature = "webgl-render")]
use std::collections::HashMap;

/// State for WebGL contexts, stored in OpState.
pub struct WebGLState {
    #[cfg(feature = "webgl-render")]
    contexts: HashMap<i32, WebGLContext>,
    #[cfg(feature = "webgl-render")]
    next_id: i32,
    available: bool,
}

impl WebGLState {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "webgl-render")]
            contexts: HashMap::new(),
            #[cfg(feature = "webgl-render")]
            next_id: 1,
            available: cfg!(feature = "webgl-render"),
        }
    }
}

/// Check if real WebGL rendering is available.
#[op2(fast)]
pub fn op_webgl_available(#[state] state: &WebGLState) -> bool {
    state.available
}

/// Create a new WebGL context. Returns context ID or -1 on failure.
#[op2(fast)]
#[smi]
pub fn op_webgl_create_context(
    #[state] _state: &mut WebGLState,
    #[smi] _width: i32,
    #[smi] _height: i32,
    #[bigint] _canvas_seed: u64,
) -> i32 {
    #[cfg(feature = "webgl-render")]
    {
        if let Some(ctx) = WebGLContext::new(_width as u32, _height as u32, _canvas_seed) {
            let id = _state.next_id;
            _state.next_id += 1;
            _state.contexts.insert(id, ctx);
            return id;
        }
    }
    -1
}

/// Create a shader. Returns shader handle ID.
#[op2(fast)]
#[smi]
pub fn op_webgl_create_shader(
    #[state] _state: &mut WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _shader_type: u32,
) -> i32 {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get_mut(&_ctx_id) {
        return ctx.create_shader(_shader_type) as i32;
    }
    0
}

/// Set shader source code.
#[op2(fast)]
pub fn op_webgl_shader_source(
    #[state] _state: &mut WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _shader_id: i32,
    #[string] _source: &str,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get_mut(&_ctx_id) {
        ctx.shader_source(_shader_id as u32, _source);
    }
}

/// Compile a shader.
#[op2(fast)]
pub fn op_webgl_compile_shader(
    #[state] _state: &mut WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _shader_id: i32,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get_mut(&_ctx_id) {
        ctx.compile_shader(_shader_id as u32);
    }
}

/// Get shader parameter (e.g. COMPILE_STATUS).
#[op2(fast)]
#[smi]
pub fn op_webgl_get_shader_parameter(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _shader_id: i32,
    #[smi] _pname: u32,
) -> i32 {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        return ctx.get_shader_parameter(_shader_id as u32, _pname);
    }
    1 // Default: success
}

/// Create a program.
#[op2(fast)]
#[smi]
pub fn op_webgl_create_program(#[state] _state: &mut WebGLState, #[smi] _ctx_id: i32) -> i32 {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get_mut(&_ctx_id) {
        return ctx.create_program() as i32;
    }
    0
}

/// Attach shader to program.
#[op2(fast)]
pub fn op_webgl_attach_shader(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _program_id: i32,
    #[smi] _shader_id: i32,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.attach_shader(_program_id as u32, _shader_id as u32);
    }
}

/// Link program.
#[op2(fast)]
pub fn op_webgl_link_program(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _program_id: i32,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.link_program(_program_id as u32);
    }
}

/// Use program.
#[op2(fast)]
pub fn op_webgl_use_program(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _program_id: i32,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.use_program(_program_id as u32);
    }
}

/// Create buffer.
#[op2(fast)]
#[smi]
pub fn op_webgl_create_buffer(#[state] _state: &mut WebGLState, #[smi] _ctx_id: i32) -> i32 {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get_mut(&_ctx_id) {
        return ctx.create_buffer() as i32;
    }
    0
}

/// Bind buffer.
#[op2(fast)]
pub fn op_webgl_bind_buffer(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _target: u32,
    #[smi] _buffer_id: i32,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.bind_buffer(_target, _buffer_id as u32);
    }
}

/// Draw arrays.
#[op2(fast)]
pub fn op_webgl_draw_arrays(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _mode: u32,
    #[smi] _first: i32,
    #[smi] _count: i32,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.draw_arrays(_mode, _first, _count);
    }
}

/// Clear color.
#[op2(fast)]
pub fn op_webgl_clear_color(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    r: f64,
    g: f64,
    b: f64,
    a: f64,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.clear_color(r as f32, g as f32, b as f32, a as f32);
    }
}

/// Clear.
#[op2(fast)]
pub fn op_webgl_clear(#[state] _state: &WebGLState, #[smi] _ctx_id: i32, #[smi] _mask: u32) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.clear(_mask);
    }
}

/// Viewport.
#[op2(fast)]
pub fn op_webgl_viewport(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _x: i32,
    #[smi] _y: i32,
    #[smi] _w: i32,
    #[smi] _h: i32,
) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.viewport(_x, _y, _w, _h);
    }
}

/// Read pixels — returns RGBA bytes.
#[op2]
#[serde]
pub fn op_webgl_read_pixels(
    #[state] _state: &WebGLState,
    #[smi] _ctx_id: i32,
    #[smi] _x: i32,
    #[smi] _y: i32,
    #[smi] _w: i32,
    #[smi] _h: i32,
    #[smi] _format: u32,
    #[smi] _type: u32,
) -> Vec<u8> {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        return ctx.read_pixels(_x, _y, _w, _h, _format, _type);
    }
    vec![]
}

/// Enable capability.
#[op2(fast)]
pub fn op_webgl_enable(#[state] _state: &WebGLState, #[smi] _ctx_id: i32, #[smi] _cap: u32) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.enable(_cap);
    }
}

/// Disable capability.
#[op2(fast)]
pub fn op_webgl_disable(#[state] _state: &WebGLState, #[smi] _ctx_id: i32, #[smi] _cap: u32) {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        ctx.disable(_cap);
    }
}

/// Get error.
#[op2(fast)]
#[smi]
pub fn op_webgl_get_error(#[state] _state: &WebGLState, #[smi] _ctx_id: i32) -> u32 {
    #[cfg(feature = "webgl-render")]
    if let Some(ctx) = _state.contexts.get(&_ctx_id) {
        return ctx.get_error();
    }
    0 // GL_NO_ERROR
}

deno_core::extension!(
    webgl_extension,
    ops = [
        op_webgl_available,
        op_webgl_create_context,
        op_webgl_create_shader,
        op_webgl_shader_source,
        op_webgl_compile_shader,
        op_webgl_get_shader_parameter,
        op_webgl_create_program,
        op_webgl_attach_shader,
        op_webgl_link_program,
        op_webgl_use_program,
        op_webgl_create_buffer,
        op_webgl_bind_buffer,
        op_webgl_draw_arrays,
        op_webgl_clear_color,
        op_webgl_clear,
        op_webgl_viewport,
        op_webgl_read_pixels,
        op_webgl_enable,
        op_webgl_disable,
        op_webgl_get_error,
    ],
);
