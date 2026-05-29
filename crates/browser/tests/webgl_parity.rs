//! WebGL parity vs captured Chrome 147 on macOS arm64.
//! Real Chrome values from tests/fixtures/chrome147/captured_macos_arm64.json.

use browser::Page;

async fn evaluate(js: &str) -> String {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body><canvas id='c' width='100' height='100'></canvas></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

/// Same, but with the chrome_148_macos stealth profile active so WebGL reads
/// the real apple_m3 surfaces (incl. the FIX-D2 WebGL 1 surface).
async fn evaluate_macos(js: &str) -> String {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body><canvas id='c' width='100' height='100'></canvas></body></html>",
        Some(stealth::presets::chrome_148_macos()),
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

/// FIX-D2: `getContext("webgl")` must NOT return the WebGL 2 surface. Pre-fix,
/// a WebGL 1 context reported the WebGL 2 version string and advertised
/// WebGL-2-only extensions (e.g. EXT_color_buffer_float) — a deterministic
/// cross-API bot tell. Guards version strings, distinct classes, and the
/// extension-set delta on both the no-profile fallback and the macOS profile.
#[tokio::test]
async fn webgl1_webgl2_surfaces_are_distinct_fix_d2() {
    let probe = "
        const gl1 = document.createElement('canvas').getContext('webgl');
        const gl2 = document.createElement('canvas').getContext('webgl2');
        const e1 = gl1.getSupportedExtensions();
        JSON.stringify({
            v1: gl1.getParameter(gl1.VERSION),
            v2: gl2.getParameter(gl2.VERSION),
            classes_distinct: WebGLRenderingContext !== WebGL2RenderingContext,
            ctor1: gl1.constructor.name,
            ctor2: gl2.constructor.name,
            gl1_has_webgl2_only: e1.includes('EXT_color_buffer_float') || e1.includes('OES_draw_buffers_indexed'),
            gl1_has_webgl1_only: e1.includes('OES_texture_float') && e1.includes('ANGLE_instanced_arrays'),
            tag2: Object.prototype.toString.call(gl2),
        })
    ";
    for (label, r) in [
        ("no-profile", evaluate(probe).await),
        ("macos", evaluate_macos(probe).await),
    ] {
        let v: serde_json::Value = serde_json::from_str(&r)
            .unwrap_or_else(|_| panic!("{label}: probe returned non-JSON: {r}"));
        assert_eq!(
            v["v1"], "WebGL 1.0 (OpenGL ES 2.0 Chromium)",
            "{label}: webgl1 VERSION must be WebGL 1.0"
        );
        assert_eq!(
            v["v2"], "WebGL 2.0 (OpenGL ES 3.0 Chromium)",
            "{label}: webgl2 VERSION must be WebGL 2.0"
        );
        assert_eq!(
            v["classes_distinct"], true,
            "{label}: WebGLRenderingContext must != WebGL2RenderingContext"
        );
        assert_eq!(v["ctor1"], "WebGLRenderingContext", "{label}: webgl1 ctor");
        assert_eq!(v["ctor2"], "WebGL2RenderingContext", "{label}: webgl2 ctor");
        assert_eq!(
            v["gl1_has_webgl2_only"], false,
            "{label}: webgl1 must NOT advertise WebGL-2-only extensions (the bot tell)"
        );
        assert_eq!(
            v["gl1_has_webgl1_only"], true,
            "{label}: webgl1 must advertise WebGL-1 core-promoted extensions"
        );
        assert_eq!(
            v["tag2"], "[object WebGL2RenderingContext]",
            "{label}: webgl2 Symbol.toStringTag"
        );
    }
}

const GL_SETUP: &str = "
const c = document.getElementById('c');
const gl = c.getContext('webgl2') || c.getContext('webgl');
";

#[tokio::test]
async fn webgl_unmasked_renderer_is_angle_format() {
    let r = evaluate(&format!(
        "{GL_SETUP}
        const ext = gl.getExtension('WEBGL_debug_renderer_info');
        gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)"
    ))
    .await;
    assert!(
        r.contains("ANGLE"),
        "UNMASKED_RENDERER_WEBGL must be ANGLE-format, got: {r}"
    );
}

#[tokio::test]
async fn webgl_unmasked_vendor_is_google() {
    let r = evaluate(&format!(
        "{GL_SETUP}
        const ext = gl.getExtension('WEBGL_debug_renderer_info');
        gl.getParameter(ext.UNMASKED_VENDOR_WEBGL)"
    ))
    .await;
    assert!(
        r.starts_with("Google Inc."),
        "UNMASKED_VENDOR_WEBGL must start with 'Google Inc.', got: {r}"
    );
}

#[tokio::test]
async fn webgl_max_texture_size_chrome_value() {
    let r = evaluate(&format!("{GL_SETUP}gl.getParameter(gl.MAX_TEXTURE_SIZE)")).await;
    assert_eq!(
        r, "16384",
        "MAX_TEXTURE_SIZE must match Chrome 147 captured value"
    );
}

#[tokio::test]
async fn webgl_max_renderbuffer_size_chrome_value() {
    let r = evaluate(&format!(
        "{GL_SETUP}gl.getParameter(gl.MAX_RENDERBUFFER_SIZE)"
    ))
    .await;
    assert_eq!(r, "16384");
}

#[tokio::test]
async fn webgl_max_vertex_attribs_chrome_value() {
    let r = evaluate(&format!("{GL_SETUP}gl.getParameter(gl.MAX_VERTEX_ATTRIBS)")).await;
    assert_eq!(r, "16");
}

#[tokio::test]
async fn webgl_aliased_line_width_chrome_value() {
    // Chrome ANGLE on every OS: [1, 1]
    let r = evaluate(&format!(
        "{GL_SETUP}Array.from(gl.getParameter(gl.ALIASED_LINE_WIDTH_RANGE)).join(',')"
    ))
    .await;
    assert_eq!(r, "1,1");
}

#[tokio::test]
async fn webgl_max_viewport_dims_chrome_value() {
    let r = evaluate(&format!(
        "{GL_SETUP}Array.from(gl.getParameter(gl.MAX_VIEWPORT_DIMS)).join(',')"
    ))
    .await;
    assert_eq!(r, "16384,16384");
}

#[tokio::test]
async fn webgl_supported_extensions_count_chrome147() {
    let r = evaluate(&format!("{GL_SETUP}gl.getSupportedExtensions().length")).await;
    let n: usize = r.parse().unwrap_or(0);
    assert!(
        n >= 30,
        "supportedExtensions count should be ≥30 (Chrome 147 captured: 36); got {n}"
    );
}

#[tokio::test]
async fn webgl_supported_extensions_includes_chrome147_set() {
    let r = evaluate(&format!(
        "{GL_SETUP}
        const exts = gl.getSupportedExtensions();
        const required = ['WEBGL_debug_renderer_info','EXT_texture_filter_anisotropic',
            'WEBGL_compressed_texture_s3tc','WEBGL_lose_context','OES_texture_float_linear',
            'KHR_parallel_shader_compile'];
        required.every(e => exts.includes(e))"
    ))
    .await;
    assert_eq!(
        r, "true",
        "WebGL must support Chrome 147 baseline extensions"
    );
}

#[tokio::test]
async fn webgl_shader_precision_high_float_chrome_values() {
    // Chrome ANGLE: {rangeMin:127, rangeMax:127, precision:23}
    let r = evaluate(&format!(
        "{GL_SETUP}
        const f = gl.getShaderPrecisionFormat(gl.FRAGMENT_SHADER, gl.HIGH_FLOAT);
        f.rangeMin + ',' + f.rangeMax + ',' + f.precision"
    ))
    .await;
    assert_eq!(r, "127,127,23");
}

#[tokio::test]
async fn webgl_debug_renderer_info_extension_present() {
    let r = evaluate(&format!(
        "{GL_SETUP}
        const ext = gl.getExtension('WEBGL_debug_renderer_info');
        ext !== null && typeof ext.UNMASKED_VENDOR_WEBGL === 'number'"
    ))
    .await;
    assert_eq!(r, "true");
}
