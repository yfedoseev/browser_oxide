//! Real WebGL rendering via OSMesa + glow.
//!
//! Provides software OpenGL rendering for WebGL operations,
//! enabling real shader compilation, draw calls, and pixel readback.
//!
//! # Safety
//!
//! This module wraps two `unsafe` APIs and re-exposes them through a
//! single-threaded, owned context (`WebGLContext`):
//!
//! 1. **OSMesa FFI** (`crate::osmesa_ffi`). C entry points that take
//!    raw pointers (context handles, framebuffer buffers, name
//!    strings). Each `unsafe` call in this file invokes one of these
//!    with arguments whose validity is enforced by the surrounding
//!    Rust types: framebuffer is a live `Vec<u8>` owned by the same
//!    struct as the context; CStrings are constructed inline and live
//!    for the duration of the call.
//! 2. **`glow::Context`**. Every GL call is `unsafe` in `glow` because
//!    OpenGL state can be set inconsistently from safe Rust (e.g.
//!    binding a deleted buffer). We hold a single `glow::Context` per
//!    `WebGLContext` and an `Rc`-style invariant: methods take
//!    `&mut self` so concurrent or interleaved access from other code
//!    is statically prevented.
//!
//! The OSMesa context is made current exactly once in
//! `WebGLContext::new` (line ~53). Subsequent GL calls assume that
//! current context — they MUST execute on the same thread that made
//! it current. `WebGLContext` is `!Send` and `!Sync` (via the raw
//! pointer field) so the type system enforces this.
//!
//! Per-call `// SAFETY:` annotations would be noise; the invariant is
//! file-wide. The three foundational `unsafe` blocks (context
//! creation, MakeCurrent, glow loader) carry full SAFETY comments
//! because they establish the file-wide invariant.

use crate::osmesa_ffi;
use glow::HasContext;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_void;

/// A software-rendered WebGL context backed by OSMesa + glow.
pub struct WebGLContext {
    osmesa_ctx: *mut c_void,
    gl: glow::Context,
    _framebuffer: Vec<u8>, // Must stay alive while OSMesa context is current
    pub width: u32,
    pub height: u32,
    seed: u64,
    next_id: u32,
    shaders: HashMap<u32, glow::Shader>,
    programs: HashMap<u32, glow::Program>,
    buffers: HashMap<u32, glow::Buffer>,
    textures: HashMap<u32, glow::Texture>,
    framebuffers: HashMap<u32, glow::Framebuffer>,
    /// Uniform locations, keyed by the integer handle we return to JS.
    /// glow 0.14 made `UniformLocation` opaque (no longer constructible
    /// from an integer), so we store the actual objects and dispense
    /// integer handles that proxy into this map.
    uniform_locations: HashMap<u32, glow::UniformLocation>,
}

impl WebGLContext {
    /// Create a new WebGL context with the given dimensions.
    /// Returns None if OSMesa initialization fails.
    pub fn new(width: u32, height: u32, seed: u64) -> Option<Self> {
        let mut framebuffer = vec![0u8; (width * height * 4) as usize];

        // SAFETY: `OSMesaCreateContextExt` takes plain enum values plus
        // a null sharelist (no context sharing). It returns either a
        // valid opaque context pointer or null; we check for null
        // immediately. No pre-existing OSMesa state is required.
        let osmesa_ctx = unsafe {
            osmesa_ffi::OSMesaCreateContextExt(
                osmesa_ffi::OSMESA_RGBA,
                24, // depth bits
                8,  // stencil bits
                0,  // accum bits
                std::ptr::null_mut(),
            )
        };

        if osmesa_ctx.is_null() {
            return None;
        }

        // SAFETY: `osmesa_ctx` is the non-null context we just created.
        // `framebuffer` is a contiguous `Vec<u8>` sized to
        // `width*height*4` (RGBA, 8 bits per channel). It lives for as
        // long as this `WebGLContext` does (we move it into `self`
        // below as `_framebuffer`), so the pointer remains valid for
        // every subsequent GL operation. `GL_UNSIGNED_BYTE` matches the
        // RGBA layout; width/height match the buffer size.
        let ok = unsafe {
            osmesa_ffi::OSMesaMakeCurrent(
                osmesa_ctx,
                framebuffer.as_mut_ptr() as *mut c_void,
                osmesa_ffi::GL_UNSIGNED_BYTE,
                width as i32,
                height as i32,
            )
        };

        if ok == 0 {
            // SAFETY: MakeCurrent failed but the context is still a
            // valid handle from the successful create above; destroy
            // it to avoid leaking.
            unsafe { osmesa_ffi::OSMesaDestroyContext(osmesa_ctx) };
            return None;
        }

        // SAFETY: `glow::Context::from_loader_function` requires that
        // the loader resolves OpenGL entry points by name and that
        // an OpenGL context is current on this thread. Both hold:
        // the closure delegates to `OSMesaGetProcAddress` (which is
        // the documented OSMesa loader), and we made the context
        // current above. The loader is invoked synchronously here;
        // the CString is owned by the closure scope and outlives the
        // single FFI call it's passed to.
        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                let c_name = CString::new(name).unwrap();
                osmesa_ffi::OSMesaGetProcAddress(c_name.as_ptr()) as *const _
            })
        };

        // SAFETY: GL context is current on this thread (established
        // by OSMesaMakeCurrent above); setting initial viewport is a
        // state-only call that mutates no buffers.
        unsafe { gl.viewport(0, 0, width as i32, height as i32) };

        Some(Self {
            osmesa_ctx,
            gl,
            _framebuffer: framebuffer,
            width,
            height,
            seed,
            next_id: 1,
            shaders: HashMap::new(),
            programs: HashMap::new(),
            buffers: HashMap::new(),
            textures: HashMap::new(),
            framebuffers: HashMap::new(),
            uniform_locations: HashMap::new(),
        })
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // --- Shader pipeline ---

    pub fn create_shader(&mut self, shader_type: u32) -> u32 {
        let id = self.alloc_id();
        let shader = unsafe { self.gl.create_shader(shader_type).ok() };
        if let Some(s) = shader {
            self.shaders.insert(id, s);
        }
        id
    }

    pub fn shader_source(&self, shader_id: u32, source: &str) {
        if let Some(s) = self.shaders.get(&shader_id) {
            unsafe { self.gl.shader_source(*s, source) };
        }
    }

    pub fn compile_shader(&self, shader_id: u32) {
        if let Some(s) = self.shaders.get(&shader_id) {
            unsafe { self.gl.compile_shader(*s) };
        }
    }

    pub fn get_shader_parameter(&self, shader_id: u32, pname: u32) -> i32 {
        // glow 0.14 removed the generic `get_shader_parameter_i32` in
        // favour of status-specific helpers. We only need COMPILE_STATUS
        // in practice (that's what `webgl_bootstrap.js` asks for); any
        // other pname returns 0 to match "unsupported" semantics.
        const GL_COMPILE_STATUS: u32 = 0x8B81;
        if pname != GL_COMPILE_STATUS {
            return 0;
        }
        if let Some(s) = self.shaders.get(&shader_id) {
            unsafe {
                if self.gl.get_shader_compile_status(*s) {
                    1
                } else {
                    0
                }
            }
        } else {
            0
        }
    }

    pub fn get_shader_info_log(&self, shader_id: u32) -> String {
        if let Some(s) = self.shaders.get(&shader_id) {
            unsafe { self.gl.get_shader_info_log(*s) }
        } else {
            String::new()
        }
    }

    pub fn create_program(&mut self) -> u32 {
        let id = self.alloc_id();
        let program = unsafe { self.gl.create_program().ok() };
        if let Some(p) = program {
            self.programs.insert(id, p);
        }
        id
    }

    pub fn attach_shader(&self, program_id: u32, shader_id: u32) {
        if let (Some(p), Some(s)) = (self.programs.get(&program_id), self.shaders.get(&shader_id)) {
            unsafe { self.gl.attach_shader(*p, *s) };
        }
    }

    pub fn link_program(&self, program_id: u32) {
        if let Some(p) = self.programs.get(&program_id) {
            unsafe { self.gl.link_program(*p) };
        }
    }

    pub fn get_program_parameter(&self, program_id: u32, pname: u32) -> i32 {
        // glow 0.14 removed the generic `get_program_parameter_i32` in
        // favour of status-specific helpers. LINK_STATUS is the only
        // one JS code uses for the "did the program link?" check.
        const GL_LINK_STATUS: u32 = 0x8B82;
        if pname != GL_LINK_STATUS {
            return 0;
        }
        if let Some(p) = self.programs.get(&program_id) {
            unsafe {
                if self.gl.get_program_link_status(*p) {
                    1
                } else {
                    0
                }
            }
        } else {
            0
        }
    }

    pub fn use_program(&self, program_id: u32) {
        let p = if program_id == 0 {
            None
        } else {
            self.programs.get(&program_id).copied()
        };
        unsafe { self.gl.use_program(p) };
    }

    // --- Uniforms ---

    pub fn get_uniform_location(&mut self, program_id: u32, name: &str) -> i32 {
        let Some(p) = self.programs.get(&program_id).copied() else {
            return -1;
        };
        let Some(loc) = (unsafe { self.gl.get_uniform_location(p, name) }) else {
            return -1;
        };
        // Allocate a new integer handle; store the opaque glow
        // UniformLocation in `uniform_locations` for later lookup.
        let id = self.alloc_id();
        self.uniform_locations.insert(id, loc);
        id as i32
    }

    fn uniform_location(&self, handle: i32) -> Option<&glow::UniformLocation> {
        if handle < 0 {
            return None;
        }
        self.uniform_locations.get(&(handle as u32))
    }

    pub fn get_attrib_location(&self, program_id: u32, name: &str) -> i32 {
        if let Some(p) = self.programs.get(&program_id) {
            unsafe {
                self.gl
                    .get_attrib_location(*p, name)
                    .map(|l| l as i32)
                    .unwrap_or(-1)
            }
        } else {
            -1
        }
    }

    pub fn uniform1f(&self, location: i32, v0: f32) {
        if let Some(loc) = self.uniform_location(location) {
            unsafe { self.gl.uniform_1_f32(Some(loc), v0) };
        }
    }

    pub fn uniform4f(&self, location: i32, v0: f32, v1: f32, v2: f32, v3: f32) {
        if let Some(loc) = self.uniform_location(location) {
            unsafe { self.gl.uniform_4_f32(Some(loc), v0, v1, v2, v3) };
        }
    }

    pub fn uniform1i(&self, location: i32, v0: i32) {
        if let Some(loc) = self.uniform_location(location) {
            unsafe { self.gl.uniform_1_i32(Some(loc), v0) };
        }
    }

    pub fn uniform_matrix4fv(&self, location: i32, transpose: bool, data: &[f32]) {
        if data.len() != 16 {
            return;
        }
        if let Some(loc) = self.uniform_location(location) {
            unsafe {
                self.gl
                    .uniform_matrix_4_f32_slice(Some(loc), transpose, data)
            };
        }
    }

    // --- Buffers ---

    pub fn create_buffer(&mut self) -> u32 {
        let id = self.alloc_id();
        let buf = unsafe { self.gl.create_buffer().ok() };
        if let Some(b) = buf {
            self.buffers.insert(id, b);
        }
        id
    }

    pub fn bind_buffer(&self, target: u32, buffer_id: u32) {
        let b = if buffer_id == 0 {
            None
        } else {
            self.buffers.get(&buffer_id).copied()
        };
        unsafe { self.gl.bind_buffer(target, b) };
    }

    pub fn buffer_data(&self, target: u32, data: &[u8], usage: u32) {
        unsafe { self.gl.buffer_data_u8_slice(target, data, usage) };
    }

    pub fn vertex_attrib_pointer(
        &self,
        index: u32,
        size: i32,
        data_type: u32,
        normalized: bool,
        stride: i32,
        offset: i32,
    ) {
        unsafe {
            self.gl
                .vertex_attrib_pointer_f32(index, size, data_type, normalized, stride, offset);
        }
    }

    pub fn enable_vertex_attrib_array(&self, index: u32) {
        unsafe { self.gl.enable_vertex_attrib_array(index) };
    }

    // --- Drawing ---

    pub fn clear_color(&self, r: f32, g: f32, b: f32, a: f32) {
        unsafe { self.gl.clear_color(r, g, b, a) };
    }

    pub fn clear(&self, mask: u32) {
        unsafe { self.gl.clear(mask) };
    }

    pub fn viewport(&self, x: i32, y: i32, w: i32, h: i32) {
        unsafe { self.gl.viewport(x, y, w, h) };
    }

    pub fn draw_arrays(&self, mode: u32, first: i32, count: i32) {
        unsafe { self.gl.draw_arrays(mode, first, count) };
    }

    pub fn draw_elements(&self, mode: u32, count: i32, element_type: u32, offset: i32) {
        unsafe { self.gl.draw_elements(mode, count, element_type, offset) };
    }

    // --- Pixel readback ---

    pub fn read_pixels(&self, x: i32, y: i32, w: i32, h: i32, format: u32, type_: u32) -> Vec<u8> {
        let size = (w * h * 4) as usize; // RGBA
        let mut pixels = vec![0u8; size];
        unsafe {
            self.gl.read_pixels(
                x,
                y,
                w,
                h,
                format,
                type_,
                glow::PixelPackData::Slice(&mut pixels),
            );
        }

        // Apply deterministic jitter based on profile seed (SOTA Phase 1)
        if !pixels.is_empty() {
            // PCG32-style PRNG seeded by the profile
            let mut state = self.seed.wrapping_add(0x9E3779B97F4A7C15);
            let mut inc = (self.seed >> 32) | 1;

            let mut next_u32 = |s: &mut u64| {
                let old_state = *s;
                *s = old_state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(inc);
                let xorshifted = (((old_state >> 18) ^ old_state) >> 27) as u32;
                let rot = (old_state >> 59) as u32;
                (xorshifted >> rot) | (xorshifted << (rot.wrapping_neg() & 31))
            };

            for i in (0..pixels.len()).step_by(4) {
                let val = next_u32(&mut state);
                if (val % 100) < 5 {
                    // Jitter 5% of pixels
                    // Perturb RGB by +/- 1 in a way that remains in [0, 255]
                    pixels[i] = if pixels[i] > 128 {
                        pixels[i].wrapping_sub(1)
                    } else {
                        pixels[i].wrapping_add(1)
                    };
                    pixels[i + 1] = if pixels[i + 1] > 128 {
                        pixels[i + 1].wrapping_sub(1)
                    } else {
                        pixels[i + 1].wrapping_add(1)
                    };
                    pixels[i + 2] = if pixels[i + 2] > 128 {
                        pixels[i + 2].wrapping_sub(1)
                    } else {
                        pixels[i + 2].wrapping_add(1)
                    };
                }
            }
        }

        pixels
    }

    // --- Textures ---

    pub fn create_texture(&mut self) -> u32 {
        let id = self.alloc_id();
        let tex = unsafe { self.gl.create_texture().ok() };
        if let Some(t) = tex {
            self.textures.insert(id, t);
        }
        id
    }

    pub fn bind_texture(&self, target: u32, texture_id: u32) {
        let t = if texture_id == 0 {
            None
        } else {
            self.textures.get(&texture_id).copied()
        };
        unsafe { self.gl.bind_texture(target, t) };
    }

    pub fn tex_parameteri(&self, target: u32, pname: u32, param: i32) {
        unsafe { self.gl.tex_parameter_i32(target, pname, param) };
    }

    pub fn tex_image_2d(
        &self,
        target: u32,
        level: i32,
        internal_format: i32,
        width: i32,
        height: i32,
        format: u32,
        type_: u32,
        data: Option<&[u8]>,
    ) {
        unsafe {
            self.gl.tex_image_2d(
                target,
                level,
                internal_format,
                width,
                height,
                0, // border
                format,
                type_,
                data,
            );
        }
    }

    // --- State ---

    pub fn enable(&self, cap: u32) {
        unsafe { self.gl.enable(cap) };
    }

    pub fn disable(&self, cap: u32) {
        unsafe { self.gl.disable(cap) };
    }

    pub fn blend_func(&self, sfactor: u32, dfactor: u32) {
        unsafe { self.gl.blend_func(sfactor, dfactor) };
    }

    pub fn depth_func(&self, func: u32) {
        unsafe { self.gl.depth_func(func) };
    }

    pub fn pixel_storei(&self, pname: u32, param: i32) {
        unsafe { self.gl.pixel_store_i32(pname, param) };
    }

    pub fn get_error(&self) -> u32 {
        unsafe { self.gl.get_error() }
    }

    // --- Framebuffers ---

    pub fn create_framebuffer(&mut self) -> u32 {
        let id = self.alloc_id();
        let fb = unsafe { self.gl.create_framebuffer().ok() };
        if let Some(f) = fb {
            self.framebuffers.insert(id, f);
        }
        id
    }

    pub fn bind_framebuffer(&self, target: u32, fb_id: u32) {
        let f = if fb_id == 0 {
            None
        } else {
            self.framebuffers.get(&fb_id).copied()
        };
        unsafe { self.gl.bind_framebuffer(target, f) };
    }

    pub fn check_framebuffer_status(&self, target: u32) -> u32 {
        unsafe { self.gl.check_framebuffer_status(target) }
    }

    pub fn framebuffer_texture_2d(
        &self,
        target: u32,
        attachment: u32,
        textarget: u32,
        texture_id: u32,
        level: i32,
    ) {
        let t = self.textures.get(&texture_id).copied();
        unsafe {
            self.gl
                .framebuffer_texture_2d(target, attachment, textarget, t, level);
        }
    }

    // --- Cleanup ---

    pub fn delete_shader(&mut self, shader_id: u32) {
        if let Some(s) = self.shaders.remove(&shader_id) {
            unsafe { self.gl.delete_shader(s) };
        }
    }

    pub fn delete_program(&mut self, program_id: u32) {
        if let Some(p) = self.programs.remove(&program_id) {
            unsafe { self.gl.delete_program(p) };
        }
    }

    pub fn delete_buffer(&mut self, buffer_id: u32) {
        if let Some(b) = self.buffers.remove(&buffer_id) {
            unsafe { self.gl.delete_buffer(b) };
        }
    }

    pub fn delete_texture(&mut self, texture_id: u32) {
        if let Some(t) = self.textures.remove(&texture_id) {
            unsafe { self.gl.delete_texture(t) };
        }
    }

    pub fn delete_framebuffer(&mut self, fb_id: u32) {
        if let Some(f) = self.framebuffers.remove(&fb_id) {
            unsafe { self.gl.delete_framebuffer(f) };
        }
    }
}

impl Drop for WebGLContext {
    fn drop(&mut self) {
        // Clean up all GL resources
        let shader_ids: Vec<u32> = self.shaders.keys().copied().collect();
        for id in shader_ids {
            self.delete_shader(id);
        }
        let program_ids: Vec<u32> = self.programs.keys().copied().collect();
        for id in program_ids {
            self.delete_program(id);
        }
        let buffer_ids: Vec<u32> = self.buffers.keys().copied().collect();
        for id in buffer_ids {
            self.delete_buffer(id);
        }
        let texture_ids: Vec<u32> = self.textures.keys().copied().collect();
        for id in texture_ids {
            self.delete_texture(id);
        }
        let fb_ids: Vec<u32> = self.framebuffers.keys().copied().collect();
        for id in fb_ids {
            self.delete_framebuffer(id);
        }

        // Destroy OSMesa context
        unsafe {
            osmesa_ffi::OSMesaDestroyContext(self.osmesa_ctx);
        }
    }
}
