//! Canvas 2D rendering (tiny-skia), WebGL parameter stubs, AudioContext fingerprint.
//!
//! MIT/Apache-2.0 licensed. Part of the browser_oxide project.

pub mod audio;
pub mod canvas2d;
pub mod path;
pub mod periodic_wave;
pub mod text;
pub mod webgl;

#[cfg(feature = "webgl-render")]
pub mod osmesa_ffi;
#[cfg(feature = "webgl-render")]
pub mod webgl_render;

pub use audio::{AudioFingerprint, AudioParams, WaveType};
pub use canvas2d::Canvas2D;
pub use path::Path2D;
pub use webgl::WebGLParams;
