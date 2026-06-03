//! GPU catalog for WebGL fingerprint diversity.
//!
//! Per-GPU data used by `canvas_bootstrap.js` to make
//! `getSupportedExtensions()`, `getParameter()`, and
//! `getShaderPrecisionFormat()` return realistic, Chrome-matching
//! values. Without this, every profile returns an identical hardcoded
//! WebGL fingerprint, which is trivially detectable via a canvas/webgl
//! hash that does not match the claimed device.
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
    /// WebGL **1.0** surface, distinct from the fields above (which describe
    /// the WebGL **2.0** surface for this profile). FIX-D2: `getContext("webgl")`
    /// must return the WebGL 1 version string + extension list, NOT the WebGL 2
    /// one. A WebGL 1 context advertising WebGL-2-only extensions (e.g.
    /// `EXT_color_buffer_float`) or the `WebGL 2.0` version string is a
    /// deterministic cross-API fingerprint inconsistency.
    /// `None` = legacy behaviour (the shared fields are used for both contexts).
    #[serde(default)]
    pub webgl1: Option<WebGL1Surface>,
}

/// The WebGL 1.0 surface of a [`GpuProfile`]. Carried separately so a
/// `getContext("webgl")` request returns version-correct strings + the
/// WebGL-1 extension set (extensions promoted to core in WebGL 2 reappear
/// here; WebGL-2-only extensions are absent). See FIX-D2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebGL1Surface {
    /// `getParameter(VERSION)` for a WebGL 1 context.
    pub version: String,
    /// `getParameter(SHADING_LANGUAGE_VERSION)` for a WebGL 1 context.
    pub shading_language_version: String,
    /// `getSupportedExtensions()` for a WebGL 1 context.
    pub extensions: Vec<String>,
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
        webgl1: None,
    }
}

/// Chrome 147 on macOS 15 with Apple M3.
///
/// Extension list verified byte-exact against a fresh capture from real
/// Chrome 147 on an M3 MacBook Pro. 39 extensions in the
/// exact registration order Chromium emits via
/// `WebGLRenderingContextBase::getSupportedExtensions()`.
/// Chrome 147+ on macOS 15 with Apple M3.
///
/// Values aligned to the WebGL 2 surface captured from a real Chrome 147 on
/// M3 (`tests/fixtures/chrome147/captured_macos_arm64.json`).
///
/// Note: the top-level fields are the WebGL 2 surface (version "WebGL 2.0",
/// SLV "GLSL ES 3.00"). FIX-D2 (done): the `webgl1` field carries the distinct
/// WebGL 1 surface so `getContext("webgl")` no longer leaks the WebGL 2 version
/// string + WebGL-2-only extensions. See `apple_m3_webgl1_surface`.
pub fn apple_m3_macos() -> GpuProfile {
    apple_m3_family_profile("Apple M3")
}

/// Chrome 147+ on macOS 15 with Apple M3 Pro.
///
/// Same ANGLE Metal Renderer driver stack as `apple_m3_macos()` — extension
/// list, params, and shader precision are byte-identical (the driver is
/// shared across the M3 chip family). Only the `unmasked_renderer` string
/// differs. Use with [`presets::chrome_148_macos_sampled`]-class samplers
/// that vary `cpu_cores` ∈ {10, 12} to stay cross-API-consistent with the
/// chip's actual core count.
pub fn apple_m3_pro_macos() -> GpuProfile {
    apple_m3_family_profile("Apple M3 Pro")
}

/// Chrome 147+ on macOS 15 with Apple M3 Max.
///
/// Same ANGLE Metal Renderer driver stack as `apple_m3_macos()`. Use with
/// samplers that vary `cpu_cores` ∈ {14, 16} (M3 Max ships in 14-core and
/// 16-core variants).
pub fn apple_m3_max_macos() -> GpuProfile {
    apple_m3_family_profile("Apple M3 Max")
}

/// Shared GpuProfile constructor for the M3 chip family (base / Pro / Max).
/// All three share the ANGLE Metal Renderer stack — same extension list,
/// same getParameter values, same shader precision. Only the
/// `unmasked_renderer` string differs per chip.
fn apple_m3_family_profile(chip_name: &str) -> GpuProfile {
    GpuProfile {
        vendor: "WebKit".into(),
        renderer: "WebKit WebGL".into(),
        version: "WebGL 2.0 (OpenGL ES 3.0 Chromium)".into(),
        shading_language_version: "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)".into(),
        unmasked_vendor: "Google Inc. (Apple)".into(),
        unmasked_renderer: format!(
            "ANGLE (Apple, ANGLE Metal Renderer: {chip_name}, Unspecified Version)"
        ),
        // Captured WebGL 2 extension list, alphabetically sorted to match
        // Chrome's emission order (the WebIDL declaration order on this
        // driver). Identical across M3 / M3 Pro / M3 Max — they share the
        // ANGLE Metal driver stack.
        extensions: vec![
            "EXT_clip_control".into(),
            "EXT_color_buffer_float".into(),
            "EXT_color_buffer_half_float".into(),
            "EXT_conservative_depth".into(),
            "EXT_depth_clamp".into(),
            "EXT_disjoint_timer_query_webgl2".into(),
            "EXT_float_blend".into(),
            "EXT_polygon_offset_clamp".into(),
            "EXT_render_snorm".into(),
            "EXT_texture_compression_bptc".into(),
            "EXT_texture_compression_rgtc".into(),
            "EXT_texture_filter_anisotropic".into(),
            "EXT_texture_mirror_clamp_to_edge".into(),
            "EXT_texture_norm16".into(),
            "KHR_parallel_shader_compile".into(),
            "NV_shader_noperspective_interpolation".into(),
            "OES_draw_buffers_indexed".into(),
            "OES_sample_variables".into(),
            "OES_shader_multisample_interpolation".into(),
            "OES_texture_float_linear".into(),
            "WEBGL_blend_func_extended".into(),
            "WEBGL_clip_cull_distance".into(),
            "WEBGL_compressed_texture_astc".into(),
            "WEBGL_compressed_texture_etc".into(),
            "WEBGL_compressed_texture_etc1".into(),
            "WEBGL_compressed_texture_pvrtc".into(),
            "WEBGL_compressed_texture_s3tc".into(),
            "WEBGL_compressed_texture_s3tc_srgb".into(),
            "WEBGL_debug_renderer_info".into(),
            "WEBGL_debug_shaders".into(),
            "WEBGL_lose_context".into(),
            "WEBGL_multi_draw".into(),
            "WEBGL_polygon_mode".into(),
            "WEBGL_provoking_vertex".into(),
            "WEBGL_render_shared_exponent".into(),
            "WEBGL_stencil_texturing".into(),
        ],
        params: apple_m3_params(),
        shader_precision: standard_shader_precision(),
        webgl1: Some(apple_m3_webgl1_surface()),
    }
}

/// The WebGL **1.0** surface for the Apple M3 family (ANGLE Metal Renderer).
///
/// Derived from the WebGL 2 extension list in `apple_m3_family_profile` by the
/// spec-defined WebGL1↔WebGL2 delta: extensions promoted to *core* in WebGL 2
/// (e.g. `OES_texture_float`, `ANGLE_instanced_arrays`, `WEBGL_depth_texture`)
/// reappear as WebGL-1 extensions, and WebGL-2-only extensions (e.g.
/// `EXT_color_buffer_float`, `OES_draw_buffers_indexed`) are removed. The
/// `EXT_disjoint_timer_query_webgl2` form is replaced by its WebGL-1 form
/// `EXT_disjoint_timer_query`, and `WEBGL_color_buffer_float` is the WebGL-1
/// counterpart to WebGL 2's `EXT_color_buffer_float`. The delta set was
/// cross-checked against a reference capture's Apple row of WebGL 1 vs
/// WebGL 2 supportedExtensions. Alphabetically ordered to match
/// the WebGL 2 list's convention. 39 extensions.
fn apple_m3_webgl1_surface() -> WebGL1Surface {
    WebGL1Surface {
        version: "WebGL 1.0 (OpenGL ES 2.0 Chromium)".into(),
        shading_language_version: "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)".into(),
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
            "EXT_sRGB".into(),
            "EXT_shader_texture_lod".into(),
            "EXT_texture_compression_bptc".into(),
            "EXT_texture_compression_rgtc".into(),
            "EXT_texture_filter_anisotropic".into(),
            "EXT_texture_mirror_clamp_to_edge".into(),
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
            "WEBGL_compressed_texture_pvrtc".into(),
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
    }
}

/// `getParameter()` overrides specific to Apple M3.
///
/// Diverges from `common_params_desktop()` on:
/// - `MAX_VIEWPORT_DIMS`  = `[16384, 16384]` (common says `[32767, 32767]`)
/// - `ALIASED_POINT_SIZE_RANGE` = `[1, 511]` (common says `[1, 8190]`)
///
/// Source: `tests/fixtures/chrome147/captured_macos_arm64.json`.
fn apple_m3_params() -> Vec<(u32, serde_json::Value)> {
    use serde_json::json;
    let mut params = common_params_desktop();
    // Replace the GPU-specific entries that diverge from common.
    for (pname, value) in params.iter_mut() {
        match *pname {
            0x0D3A => *value = json!([16384, 16384]), // MAX_VIEWPORT_DIMS
            0x846D => *value = json!([1.0, 511.0]),   // ALIASED_POINT_SIZE_RANGE
            _ => {}
        }
    }
    params
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
        webgl1: None,
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
        webgl1: None,
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

    /// FIX-D2: apple_m3 must carry a distinct WebGL 1 surface whose extension
    /// set is the spec-correct delta of the WebGL 2 list — no WebGL-2-only
    /// extensions (the cross-API bot tell), and the core-promoted WebGL-1-only
    /// extensions present.
    #[test]
    fn apple_m3_webgl1_surface_is_spec_correct() {
        let gpu = apple_m3_macos();
        let w1 = gpu
            .webgl1
            .as_ref()
            .expect("apple_m3 must have a webgl1 surface");
        assert_eq!(w1.version, "WebGL 1.0 (OpenGL ES 2.0 Chromium)");
        assert_eq!(
            w1.shading_language_version,
            "WebGL GLSL ES 1.0 (OpenGL ES GLSL ES 1.0 Chromium)"
        );
        // WebGL-2-only extensions must be ABSENT from the WebGL 1 surface.
        for banned in [
            "EXT_color_buffer_float",
            "OES_draw_buffers_indexed",
            "EXT_disjoint_timer_query_webgl2",
            "WEBGL_clip_cull_distance",
            "WEBGL_provoking_vertex",
        ] {
            assert!(
                !w1.extensions.iter().any(|e| e == banned),
                "WebGL 1 surface must not expose WebGL-2-only ext {banned}"
            );
        }
        // Core-promoted WebGL-1-only extensions must be PRESENT.
        for required in [
            "OES_texture_float",
            "ANGLE_instanced_arrays",
            "WEBGL_depth_texture",
            "EXT_disjoint_timer_query",
            "WEBGL_color_buffer_float",
        ] {
            assert!(
                w1.extensions.iter().any(|e| e == required),
                "WebGL 1 surface must expose core-promoted ext {required}"
            );
        }
        // The WebGL 1 and WebGL 2 surfaces must NOT be identical.
        assert_ne!(
            w1.extensions, gpu.extensions,
            "webgl1 must differ from webgl2"
        );
        assert_ne!(w1.version, gpu.version);
    }

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

    /// Snapshot: `apple_m3_macos` MUST match the WebGL 2 fingerprint
    /// captured from a real Chrome 147 on M3
    /// (`tests/fixtures/chrome147/captured_macos_arm64.json`). Any
    /// drift between this preset and the captured ground truth is a
    /// regression — fingerprinting scripts cross-check these fields and
    /// reject on mismatch.
    #[test]
    fn apple_m3_matches_captured_chrome_147_fixture() {
        let gpu = apple_m3_macos();
        // Identity strings
        assert_eq!(gpu.vendor, "WebKit");
        assert_eq!(gpu.renderer, "WebKit WebGL");
        assert_eq!(gpu.version, "WebGL 2.0 (OpenGL ES 3.0 Chromium)");
        assert_eq!(
            gpu.shading_language_version,
            "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)"
        );
        assert_eq!(gpu.unmasked_vendor, "Google Inc. (Apple)");
        assert_eq!(
            gpu.unmasked_renderer,
            "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)"
        );
        // Extension count exactly matches the fixture
        assert_eq!(
            gpu.extensions.len(),
            36,
            "Apple M3 WebGL 2 extension list must be exactly 36 (captured fixture). \
             Add to or remove from the preset only with an updated capture."
        );
        // Spot-check a couple of WebGL 2-specific entries that
        // distinguish the surface from WebGL 1
        for required in &[
            "EXT_disjoint_timer_query_webgl2",
            "WEBGL_clip_cull_distance",
            "WEBGL_provoking_vertex",
        ] {
            assert!(
                gpu.extensions.iter().any(|e| e == required),
                "missing WebGL 2 extension {required}"
            );
        }
        // Per-GPU param overrides
        let viewport_dims = gpu
            .params
            .iter()
            .find(|(k, _)| *k == 0x0D3A)
            .expect("MAX_VIEWPORT_DIMS present")
            .1
            .clone();
        assert_eq!(
            viewport_dims,
            serde_json::json!([16384, 16384]),
            "Apple M3 MAX_VIEWPORT_DIMS must be [16384, 16384], not the \
             common_params_desktop [32767, 32767] default"
        );
        let point_size = gpu
            .params
            .iter()
            .find(|(k, _)| *k == 0x846D)
            .expect("ALIASED_POINT_SIZE_RANGE present")
            .1
            .clone();
        assert_eq!(
            point_size,
            serde_json::json!([1.0, 511.0]),
            "Apple M3 ALIASED_POINT_SIZE_RANGE must be [1, 511]"
        );
        // The high_float shader precision in the fixture
        let high_float = gpu
            .shader_precision
            .iter()
            .find(|(s, p, _)| *s == 0x8B30 && *p == 0x8DF2)
            .expect("FRAGMENT_SHADER HIGH_FLOAT present")
            .2;
        assert_eq!(high_float, [127, 127, 23]);
    }
}
