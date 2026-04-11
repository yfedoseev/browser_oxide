/// WebGL parameter stubs for fingerprint spoofing.
///
/// Anti-bot systems query WebGL parameters to identify hardware.
/// We return consistent values from a profile rather than real GPU data.
#[derive(Debug, Clone)]
pub struct WebGLParams {
    pub vendor: String,
    pub renderer: String,
    pub version: String,
    pub shading_language_version: String,
    pub max_texture_size: i32,
    pub max_renderbuffer_size: i32,
    pub max_viewport_dims: [i32; 2],
    pub max_vertex_attribs: i32,
    pub max_varying_vectors: i32,
    pub max_combined_texture_image_units: i32,
    pub max_texture_image_units: i32,
    pub max_fragment_uniform_vectors: i32,
    pub max_vertex_uniform_vectors: i32,
    pub max_vertex_texture_image_units: i32,
    pub aliased_line_width_range: [f32; 2],
    pub aliased_point_size_range: [f32; 2],
    pub extensions: Vec<String>,
    pub high_float_precision: ShaderPrecision,
    pub medium_float_precision: ShaderPrecision,
}

#[derive(Debug, Clone, Copy)]
pub struct ShaderPrecision {
    pub range_min: i32,
    pub range_max: i32,
    pub precision: i32,
}

// WebGL parameter constants (subset used by anti-bot)
pub const VENDOR: u32 = 0x1F00;
pub const RENDERER: u32 = 0x1F01;
pub const VERSION: u32 = 0x1F02;
pub const SHADING_LANGUAGE_VERSION: u32 = 0x8B8C;
pub const MAX_TEXTURE_SIZE: u32 = 0x0D33;
pub const MAX_RENDERBUFFER_SIZE: u32 = 0x84E8;
pub const MAX_VIEWPORT_DIMS: u32 = 0x0D3A;
pub const MAX_VERTEX_ATTRIBS: u32 = 0x8869;
pub const MAX_VARYING_VECTORS: u32 = 0x8DFC;
pub const MAX_COMBINED_TEXTURE_IMAGE_UNITS: u32 = 0x8B4D;
pub const MAX_TEXTURE_IMAGE_UNITS: u32 = 0x8872;
pub const MAX_FRAGMENT_UNIFORM_VECTORS: u32 = 0x8DFD;
pub const MAX_VERTEX_UNIFORM_VECTORS: u32 = 0x8DFB;
pub const MAX_VERTEX_TEXTURE_IMAGE_UNITS: u32 = 0x8B4C;
pub const ALIASED_LINE_WIDTH_RANGE: u32 = 0x846E;
pub const ALIASED_POINT_SIZE_RANGE: u32 = 0x846D;
// WEBGL_debug_renderer_info extension
pub const UNMASKED_VENDOR_WEBGL: u32 = 0x9245;
pub const UNMASKED_RENDERER_WEBGL: u32 = 0x9246;

impl WebGLParams {
    /// Get a parameter value as a string (for getParameter).
    pub fn get_parameter_string(&self, pname: u32) -> Option<String> {
        match pname {
            VENDOR => Some("WebKit".to_string()),
            RENDERER => Some("WebKit WebGL".to_string()),
            VERSION => Some(self.version.clone()),
            SHADING_LANGUAGE_VERSION => Some(self.shading_language_version.clone()),
            UNMASKED_VENDOR_WEBGL => Some(self.vendor.clone()),
            UNMASKED_RENDERER_WEBGL => Some(self.renderer.clone()),
            _ => None,
        }
    }

    /// Get a parameter value as an integer.
    pub fn get_parameter_int(&self, pname: u32) -> Option<i32> {
        match pname {
            MAX_TEXTURE_SIZE => Some(self.max_texture_size),
            MAX_RENDERBUFFER_SIZE => Some(self.max_renderbuffer_size),
            MAX_VERTEX_ATTRIBS => Some(self.max_vertex_attribs),
            MAX_VARYING_VECTORS => Some(self.max_varying_vectors),
            MAX_COMBINED_TEXTURE_IMAGE_UNITS => Some(self.max_combined_texture_image_units),
            MAX_TEXTURE_IMAGE_UNITS => Some(self.max_texture_image_units),
            MAX_FRAGMENT_UNIFORM_VECTORS => Some(self.max_fragment_uniform_vectors),
            MAX_VERTEX_UNIFORM_VECTORS => Some(self.max_vertex_uniform_vectors),
            MAX_VERTEX_TEXTURE_IMAGE_UNITS => Some(self.max_vertex_texture_image_units),
            _ => None,
        }
    }

    /// NVIDIA RTX 3080 desktop profile.
    pub fn nvidia_rtx_3080() -> Self {
        Self {
            vendor: "Google Inc. (NVIDIA)".into(),
            renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)"
                .into(),
            version: "WebGL 2.0 (OpenGL ES 3.0 Chromium)".into(),
            shading_language_version: "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)".into(),
            max_texture_size: 16384,
            max_renderbuffer_size: 16384,
            max_viewport_dims: [32767, 32767],
            max_vertex_attribs: 16,
            max_varying_vectors: 30,
            max_combined_texture_image_units: 32,
            max_texture_image_units: 16,
            max_fragment_uniform_vectors: 1024,
            max_vertex_uniform_vectors: 4096,
            max_vertex_texture_image_units: 16,
            aliased_line_width_range: [1.0, 1.0],
            aliased_point_size_range: [1.0, 1024.0],
            extensions: nvidia_extensions(),
            high_float_precision: ShaderPrecision {
                range_min: 127,
                range_max: 127,
                precision: 23,
            },
            medium_float_precision: ShaderPrecision {
                range_min: 127,
                range_max: 127,
                precision: 23,
            },
        }
    }

    /// Intel Iris Xe integrated profile.
    pub fn intel_iris_xe() -> Self {
        Self {
            vendor: "Google Inc. (Intel)".into(),
            renderer: "ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)"
                .into(),
            version: "WebGL 2.0 (OpenGL ES 3.0 Chromium)".into(),
            shading_language_version: "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)".into(),
            max_texture_size: 16384,
            max_renderbuffer_size: 16384,
            max_viewport_dims: [16384, 16384],
            max_vertex_attribs: 16,
            max_varying_vectors: 30,
            max_combined_texture_image_units: 32,
            max_texture_image_units: 16,
            max_fragment_uniform_vectors: 1024,
            max_vertex_uniform_vectors: 4096,
            max_vertex_texture_image_units: 16,
            aliased_line_width_range: [1.0, 1.0],
            aliased_point_size_range: [1.0, 1024.0],
            extensions: intel_extensions(),
            high_float_precision: ShaderPrecision {
                range_min: 127,
                range_max: 127,
                precision: 23,
            },
            medium_float_precision: ShaderPrecision {
                range_min: 127,
                range_max: 127,
                precision: 23,
            },
        }
    }

    /// Apple M2 (macOS) profile.
    pub fn apple_m2() -> Self {
        Self {
            vendor: "Google Inc. (Apple)".into(),
            renderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)".into(),
            version: "WebGL 2.0 (OpenGL ES 3.0 Chromium)".into(),
            shading_language_version: "WebGL GLSL ES 3.00 (OpenGL ES GLSL ES 3.0 Chromium)".into(),
            max_texture_size: 16384,
            max_renderbuffer_size: 16384,
            max_viewport_dims: [16384, 16384],
            max_vertex_attribs: 16,
            max_varying_vectors: 30,
            max_combined_texture_image_units: 32,
            max_texture_image_units: 16,
            max_fragment_uniform_vectors: 1024,
            max_vertex_uniform_vectors: 4096,
            max_vertex_texture_image_units: 16,
            aliased_line_width_range: [1.0, 1.0],
            aliased_point_size_range: [1.0, 1024.0],
            extensions: apple_extensions(),
            high_float_precision: ShaderPrecision {
                range_min: 127,
                range_max: 127,
                precision: 23,
            },
            medium_float_precision: ShaderPrecision {
                range_min: 127,
                range_max: 127,
                precision: 23,
            },
        }
    }
}

fn nvidia_extensions() -> Vec<String> {
    vec![
        "ANGLE_instanced_arrays",
        "EXT_blend_minmax",
        "EXT_color_buffer_half_float",
        "EXT_float_blend",
        "EXT_frag_depth",
        "EXT_shader_texture_lod",
        "EXT_texture_compression_bptc",
        "EXT_texture_compression_rgtc",
        "EXT_texture_filter_anisotropic",
        "EXT_sRGB",
        "OES_element_index_uint",
        "OES_fbo_render_mipmap",
        "OES_standard_derivatives",
        "OES_texture_float",
        "OES_texture_float_linear",
        "OES_texture_half_float",
        "OES_texture_half_float_linear",
        "OES_vertex_array_object",
        "WEBGL_color_buffer_float",
        "WEBGL_compressed_texture_s3tc",
        "WEBGL_compressed_texture_s3tc_srgb",
        "WEBGL_debug_renderer_info",
        "WEBGL_debug_shaders",
        "WEBGL_depth_texture",
        "WEBGL_draw_buffers",
        "WEBGL_lose_context",
        "WEBGL_multi_draw",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn intel_extensions() -> Vec<String> {
    nvidia_extensions() // Similar for modern Intel GPUs
}

fn apple_extensions() -> Vec<String> {
    nvidia_extensions() // Similar for Apple Silicon
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvidia_profile_params() {
        let p = WebGLParams::nvidia_rtx_3080();
        assert_eq!(p.get_parameter_int(MAX_TEXTURE_SIZE), Some(16384));
        assert!(p
            .get_parameter_string(UNMASKED_RENDERER_WEBGL)
            .unwrap()
            .contains("RTX 3080"));
        assert!(p
            .get_parameter_string(UNMASKED_VENDOR_WEBGL)
            .unwrap()
            .contains("NVIDIA"));
    }

    #[test]
    fn intel_profile_params() {
        let p = WebGLParams::intel_iris_xe();
        assert!(p
            .get_parameter_string(UNMASKED_RENDERER_WEBGL)
            .unwrap()
            .contains("Iris"));
    }

    #[test]
    fn extensions_nonempty() {
        let p = WebGLParams::nvidia_rtx_3080();
        assert!(p.extensions.len() > 20);
        assert!(p
            .extensions
            .contains(&"WEBGL_debug_renderer_info".to_string()));
    }
}
