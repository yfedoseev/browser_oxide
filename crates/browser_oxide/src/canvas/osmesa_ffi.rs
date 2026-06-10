//! Raw FFI bindings to Mesa's OSMesa (Off-Screen Mesa) library.
//!
//! OSMesa provides software OpenGL rendering without a display server.
//! Requires `libosmesa6-dev` on Debian/Ubuntu.

#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    reason = "FFI bindings mirror the OSMesa C symbol names"
)]

use std::os::raw::{c_char, c_int, c_void};

pub const OSMESA_RGBA: u32 = 0x1908;
pub const GL_UNSIGNED_BYTE: u32 = 0x1401;

extern "C" {
    pub fn OSMesaCreateContextExt(
        format: u32,
        depth_bits: c_int,
        stencil_bits: c_int,
        accum_bits: c_int,
        sharelist: *mut c_void,
    ) -> *mut c_void;

    pub fn OSMesaMakeCurrent(
        ctx: *mut c_void,
        buffer: *mut c_void,
        type_: u32,
        width: c_int,
        height: c_int,
    ) -> c_int;

    pub fn OSMesaDestroyContext(ctx: *mut c_void);

    pub fn OSMesaGetProcAddress(func_name: *const c_char) -> *mut c_void;
}
