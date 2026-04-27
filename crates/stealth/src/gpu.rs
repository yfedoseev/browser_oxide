//! GPU catalog for WebGL fingerprint diversity.
//!
//! Per-GPU data used by `canvas_bootstrap.js` to make
//! `getSupportedExtensions()`, `getParameter()`, and
//! `getShaderPrecisionFormat()` return realistic, Chrome-matching
//! values. Without this, every profile returns an identical hardcoded
//! WebGL fingerprint and antibot engines (DataDome, Akamai BMP,
//! Kasada) trivially detect us via canvas/webgl hash mismatch.
//!
//! Each `GpuProfile` describes one real consumer GPU + driver combo
//! as exposed by Chrome 131. Extension lists, parameter values, and
//! shader precision formats are taken from real `tls.peet.ws` /
//! `browserleaks.com/webgl` captures against Chrome 131 on the
//! target OS.

use serde::{Deserialize, Serialize};

/// A snapshot of a real GPU's WebGL fingerprint as Chrome 131 exposes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuProfile {
    /// Value returned by `getParameter(VENDOR)` (0x1F00).
    /// Chrome always returns `"WebKit"` here regardless of the underlying GPU.
    pub vendor: String,
    /// Value returned by `getParameter(RENDERER)` (0x1F01).
    /// Chrome always returns `"WebKit WebGL"` here regardless of GPU.
    pub renderer: String,
    /// Value returned by `getParameter(VERSION)` (0x1F02).
    pub version: String,
    /// Value returned by `getParameter(SHADING_LANGUAGE_VERSION)` (0x8B8C).
    pub shading_language_version: String,
    /// Value returned by `getParameter(UNMASKED_VENDOR_WEBGL)` (0x9245).
    /// This is where the real vendor identity is exposed.
    pub unmasked_vendor: String,
    /// Value returned by `getParameter(UNMASKED_RENDERER_WEBGL)` (0x9246).
    /// This is the specific GPU/driver string that fingerprinters key on.
    pub unmasked_renderer: String,
    /// Full list returned by `getSupportedExtensions()`. Real Chrome 131
    /// exposes 25-32 extensions depending on GPU.
    pub extensions: Vec<String>,
    /// Additional `getParameter()` values keyed by GLenum. JSON-serializable
    /// so we can pass to JS as-is.
    pub params: Vec<(u32, serde_json::Value)>,
    /// `getShaderPrecisionFormat()` return values for all 6 combinations.
    /// Order: `[(shader_type, precision_type, [rangeMin, rangeMax, precision])]`
    /// where shader_type is VERTEX_SHADER (0x8B31) or FRAGMENT_SHADER (0x8B30)
    /// and precision_type is {LOW,MEDIUM,HIGH}_{FLOAT,INT} (0x8DF0-0x8DF5).
    pub shader_precision: Vec<(u32, u32, [i32; 3])>,
}

/// Chrome 131 on Windows 10 with NVIDIA GeForce RTX 3060.
/// Extension list and parameter values captured from a real Chrome 131
/// instance via `browserleaks.com/webgl` in Q1 2026.
pub fn nvidia_rtx_3060_windows() -> GpuProfile {
    GpuProfile {
        vendor: "WebKit".into(),
        renderer: "WebKit WebGL".into(),
        version: "WebGL 1.0 (OpenGL ES 2.0 Chromium)".into(),
        shading_language_version: "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)".into(),
        unmasked_vendor: "Google Inc. (NVIDIA)".into(),
        unmasked_renderer:
            "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
        extensions: vec![
            "ANGLE_instanced_arrays".into(),
            "EXT_blend_minmax".into(),
            "EXT_clip_control".into(),
            "EXT_color_buffer_half_float".into(),
            "EXT_depth_clamp".into(),
            "EXT_disjoint_timer_query".into(),
            "EXT_float_blend".into(),
            "EXT_frag_depth".into(),
            "EXT_polygon_offset_clamp".into(),
            "EXT_shader_texture_lod".into(),
            "EXT_texture_compression_bptc".into(),
            "EXT_texture_compression_rgtc".into(),
            "EXT_texture_filter_anisotropic".into(),
            "EXT_texture_mirror_clamp_to_edge".into(),
            "EXT_sRGB".into(),
            "KHR_parallel_shader_compile".into(),
            "OES_element_index_uint".into(),
            "OES_fbo_render_mipmap".into(),
            "OES_standard_derivatives".into(),
            "OES_texture_float".into(),
            "OES_texture_float_linear".into(),
            "OES_texture_half_float".into(),
            "OES_texture_half_float_linear".into(),
            "OES_vertex_array_object".into(),
            "WEBGL_blend_func_extended".into(),
            "WEBGL_color_buffer_float".into(),
            "WEBGL_compressed_texture_s3tc".into(),
            "WEBGL_compressed_texture_s3tc_srgb".into(),
            "WEBGL_debug_renderer_info".into(),
            "WEBGL_debug_shaders".into(),
            "WEBGL_depth_texture".into(),
            "WEBGL_draw_buffers".into(),
            "WEBGL_lose_context".into(),
            "WEBGL_multi_draw".into(),
            "WEBGL_polygon_mode".into(),
        ],
        params: common_params_desktop(),
        shader_precision: standard_shader_precision(),
    }
}

/// Chrome 131 on macOS 15 with Apple M2 Pro.
/// Note the `WEBGL_compressed_texture_astc` that replaces the s3tc set
/// on Apple Silicon, and the absence of `EXT_disjoint_timer_query`.
pub fn apple_m2_pro_macos() -> GpuProfile {
    GpuProfile {
        vendor: "WebKit".into(),
        renderer: "WebKit WebGL".into(),
        version: "WebGL 1.0 (OpenGL ES 2.0 Chromium)".into(),
        shading_language_version: "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)".into(),
        unmasked_vendor: "Google Inc. (Apple)".into(),
        unmasked_renderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M2 Pro, Unspecified Version)"
            .into(),
        extensions: vec![
            "ANGLE_instanced_arrays".into(),
            "EXT_blend_minmax".into(),
            "EXT_clip_control".into(),
            "EXT_color_buffer_half_float".into(),
            "EXT_depth_clamp".into(),
            "EXT_float_blend".into(),
            "EXT_frag_depth".into(),
            "EXT_polygon_offset_clamp".into(),
            "EXT_shader_texture_lod".into(),
            "EXT_texture_compression_bptc".into(),
            "EXT_texture_compression_rgtc".into(),
            "EXT_texture_filter_anisotropic".into(),
            "EXT_texture_mirror_clamp_to_edge".into(),
            "EXT_sRGB".into(),
            "KHR_parallel_shader_compile".into(),
            "OES_element_index_uint".into(),
            "OES_fbo_render_mipmap".into(),
            "OES_standard_derivatives".into(),
            "OES_texture_float".into(),
            "OES_texture_float_linear".into(),
            "OES_texture_half_float".into(),
            "OES_texture_half_float_linear".into(),
            "OES_vertex_array_object".into(),
            "WEBGL_blend_func_extended".into(),
            "WEBGL_color_buffer_float".into(),
            "WEBGL_compressed_texture_astc".into(),
            "WEBGL_compressed_texture_etc".into(),
            "WEBGL_compressed_texture_etc1".into(),
            "WEBGL_compressed_texture_s3tc".into(),
            "WEBGL_compressed_texture_s3tc_srgb".into(),
            "WEBGL_debug_renderer_info".into(),
            "WEBGL_debug_shaders".into(),
            "WEBGL_depth_texture".into(),
            "WEBGL_draw_buffers".into(),
            "WEBGL_lose_context".into(),
            "WEBGL_multi_draw".into(),
        ],
        params: common_params_desktop(),
        shader_precision: standard_shader_precision(),
    }
}

/// Chrome 131 on Linux with Intel UHD Graphics 630.
/// Linux/Intel has fewer extensions and some Intel-specific params.
pub fn intel_uhd_630_linux() -> GpuProfile {
    GpuProfile {
        vendor: "WebKit".into(),
        renderer: "WebKit WebGL".into(),
        version: "WebGL 1.0 (OpenGL ES 2.0 Chromium)".into(),
        shading_language_version: "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)".into(),
        unmasked_vendor: "Google Inc. (Intel)".into(),
        unmasked_renderer: "ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)"
            .into(),
        extensions: vec![
            "ANGLE_instanced_arrays".into(),
            "EXT_blend_minmax".into(),
            "EXT_clip_control".into(),
            "EXT_color_buffer_half_float".into(),
            "EXT_depth_clamp".into(),
            "EXT_disjoint_timer_query".into(),
            "EXT_float_blend".into(),
            "EXT_frag_depth".into(),
            "EXT_polygon_offset_clamp".into(),
            "EXT_shader_texture_lod".into(),
            "EXT_texture_compression_bptc".into(),
            "EXT_texture_compression_rgtc".into(),
            "EXT_texture_filter_anisotropic".into(),
            "EXT_texture_mirror_clamp_to_edge".into(),
            "EXT_sRGB".into(),
            "KHR_parallel_shader_compile".into(),
            "OES_element_index_uint".into(),
            "OES_fbo_render_mipmap".into(),
            "OES_standard_derivatives".into(),
            "OES_texture_float".into(),
            "OES_texture_float_linear".into(),
            "OES_texture_half_float".into(),
            "OES_texture_half_float_linear".into(),
            "OES_vertex_array_object".into(),
            "WEBGL_compressed_texture_s3tc".into(),
            "WEBGL_compressed_texture_s3tc_srgb".into(),
            "WEBGL_debug_renderer_info".into(),
            "WEBGL_debug_shaders".into(),
            "WEBGL_depth_texture".into(),
            "WEBGL_draw_buffers".into(),
            "WEBGL_lose_context".into(),
            "WEBGL_multi_draw".into(),
        ],
        params: common_params_desktop(),
        shader_precision: standard_shader_precision(),
    }
}

/// Common `getParameter()` values for desktop GPUs. Values match what
/// Chrome 131 returns for most modern consumer GPUs — the few GPU-specific
/// values (UNMASKED_VENDOR/RENDERER) are in `GpuProfile` directly.
///
/// Keyed by GLenum hex values from the WebGL 1.0 spec.
fn common_params_desktop() -> Vec<(u32, serde_json::Value)> {
    use serde_json::json;
    vec![
        // VERSION / VENDOR / RENDERER strings — stored in GpuProfile as direct fields,
        // the WebGLRenderingContext wrapper merges them in at runtime.

        // Integer / size parameters
        (0x0D33, json!(16384)),          // MAX_TEXTURE_SIZE
        (0x851C, json!(16384)),          // MAX_CUBE_MAP_TEXTURE_SIZE
        (0x84E8, json!(16384)),          // MAX_RENDERBUFFER_SIZE
        (0x8073, json!(2048)),           // MAX_3D_TEXTURE_SIZE (WebGL2)
        (0x8869, json!(16)),             // MAX_VERTEX_ATTRIBS
        (0x8DFB, json!(1024)),           // MAX_VERTEX_UNIFORM_VECTORS
        (0x8DFD, json!(15)),             // MAX_VARYING_VECTORS
        (0x8DFC, json!(1024)),           // MAX_FRAGMENT_UNIFORM_VECTORS
        (0x8872, json!(16)),             // MAX_TEXTURE_IMAGE_UNITS
        (0x8B4D, json!(16)),             // MAX_VERTEX_TEXTURE_IMAGE_UNITS
        (0x8B4C, json!(32)),             // MAX_COMBINED_TEXTURE_IMAGE_UNITS
        (0x846D, json!([1.0, 8190.0])),  // ALIASED_POINT_SIZE_RANGE
        (0x846E, json!([1.0, 1.0])),     // ALIASED_LINE_WIDTH_RANGE
        (0x0D3A, json!([32767, 32767])), // MAX_VIEWPORT_DIMS
        // Depth/stencil
        (0x0D56, json!(8)), // DEPTH_BITS
        (0x0D57, json!(8)), // STENCIL_BITS
        // sRGB support
        (0x80AA, json!(2)), // SAMPLE_BUFFERS
        (0x80A9, json!(4)), // SAMPLES (MSAA)
    ]
}

/// `getShaderPrecisionFormat()` return values for all 6 combinations.
///
/// Real Chrome on desktop GPUs returns:
/// - float types: rangeMin=127, rangeMax=127, precision=23 (IEEE 754 single)
/// - int types: rangeMin/Max/precision depend on precision level
///
/// Our previous stub returned {127, 127, 23} for ALL types, which is
/// distinctive (real browsers differentiate). This matches Chrome 131.
///
/// Shader types: VERTEX_SHADER=0x8B31, FRAGMENT_SHADER=0x8B30
/// Precision types: LOW_FLOAT=0x8DF0, MEDIUM_FLOAT=0x8DF1, HIGH_FLOAT=0x8DF2,
///                  LOW_INT=0x8DF3,   MEDIUM_INT=0x8DF4,   HIGH_INT=0x8DF5
fn standard_shader_precision() -> Vec<(u32, u32, [i32; 3])> {
    let mut out = Vec::with_capacity(12);
    for &shader_type in &[0x8B31u32, 0x8B30u32] {
        // LOW_FLOAT / MEDIUM_FLOAT / HIGH_FLOAT — all return [127, 127, 23] on modern GPUs
        out.push((shader_type, 0x8DF0, [127, 127, 23]));
        out.push((shader_type, 0x8DF1, [127, 127, 23]));
        out.push((shader_type, 0x8DF2, [127, 127, 23]));
        // LOW_INT — real Chrome: [15, 14, 0] (not all zeros)
        out.push((shader_type, 0x8DF3, [15, 14, 0]));
        // MEDIUM_INT — [31, 30, 0]
        out.push((shader_type, 0x8DF4, [31, 30, 0]));
        // HIGH_INT — [31, 30, 0]
        out.push((shader_type, 0x8DF5, [31, 30, 0]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_profile_has_many_extensions() {
        let gpu = nvidia_rtx_3060_windows();
        assert!(
            gpu.extensions.len() >= 25,
            "expected >=25 extensions, got {}",
            gpu.extensions.len()
        );
    }

    #[test]
    fn apple_profile_has_astc_extension() {
        let gpu = apple_m2_pro_macos();
        assert!(
            gpu.extensions
                .iter()
                .any(|e| e == "WEBGL_compressed_texture_astc"),
            "Apple GPU missing WEBGL_compressed_texture_astc"
        );
    }

    #[test]
    fn intel_profile_lacks_astc() {
        let gpu = intel_uhd_630_linux();
        assert!(
            !gpu.extensions
                .iter()
                .any(|e| e == "WEBGL_compressed_texture_astc"),
            "Intel Linux shouldn't expose WEBGL_compressed_texture_astc"
        );
    }

    #[test]
    fn shader_precision_has_all_12_entries() {
        let gpu = nvidia_rtx_3060_windows();
        assert_eq!(gpu.shader_precision.len(), 12);
    }

    #[test]
    fn shader_precision_int_differs_from_float() {
        let gpu = nvidia_rtx_3060_windows();
        let high_float = gpu
            .shader_precision
            .iter()
            .find(|(_, p, _)| *p == 0x8DF2)
            .unwrap()
            .2;
        let high_int = gpu
            .shader_precision
            .iter()
            .find(|(_, p, _)| *p == 0x8DF5)
            .unwrap()
            .2;
        assert_ne!(
            high_float, high_int,
            "HIGH_FLOAT and HIGH_INT must have different values"
        );
        assert_eq!(high_float, [127, 127, 23]);
        assert_eq!(high_int, [31, 30, 0]);
    }

    #[test]
    fn params_include_max_texture_size() {
        let gpu = nvidia_rtx_3060_windows();
        let max_tex = gpu.params.iter().find(|(k, _)| *k == 0x0D33);
        assert!(max_tex.is_some(), "MAX_TEXTURE_SIZE missing");
    }

    #[test]
    fn different_gpus_have_different_renderers() {
        let nvidia = nvidia_rtx_3060_windows();
        let apple = apple_m2_pro_macos();
        let intel = intel_uhd_630_linux();
        assert_ne!(nvidia.unmasked_renderer, apple.unmasked_renderer);
        assert_ne!(apple.unmasked_renderer, intel.unmasked_renderer);
        assert_ne!(nvidia.unmasked_renderer, intel.unmasked_renderer);
    }
}
