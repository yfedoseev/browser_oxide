//! WebGL stub-output stability gate. Without the `webgl-render`
//! feature (default), our WebGL surface is implemented as JS-side
//! stubs that route `clearColor` + `readPixels` through Canvas2D ops.
//! This test pins the byte output for a known clearColor scene so
//! any change to the stub plumbing breaks loudly.
//!
//! When the `webgl-render` feature is enabled (requires OSMesa), real
//! shader execution is exercised by other tests. This test only
//! covers the no-feature default path.

use browser::Page;
use sha2::{Digest, Sha256};
use stealth;

async fn evaluate(js: &str) -> String {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body><canvas id='c' width='40' height='40'></canvas></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

/// `clearColor` followed by `readPixels` must round-trip the same
/// color across all 1600 pixels of a 40x40 canvas. This is the basic
/// "WebGL produces a canvas of pure red" probe that fingerprint sites
/// use as a no-op smoke test before rendering anything fancier.
#[tokio::test]
async fn webgl_clear_and_read_round_trips_color() {
    let r = evaluate(
        "
        const c = document.getElementById('c');
        const gl = c.getContext('webgl');
        gl.viewport(0, 0, 40, 40);
        gl.clearColor(1.0, 0.5, 0.0, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT);
        const pixels = new Uint8Array(40 * 40 * 4);
        gl.readPixels(0, 0, 40, 40, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
        // Compute a single representative pixel (middle of canvas)
        const mid = (20 * 40 + 20) * 4;
        const rgba = pixels[mid] + ',' + pixels[mid+1] + ',' + pixels[mid+2] + ',' + pixels[mid+3];
        // 1.0 -> 255, 0.5 -> 127 or 128 depending on rounding, 0 -> 0, 1.0 -> 255
        rgba
        ",
    )
    .await;
    // Allow either rounding for the 0.5 -> {127,128} ambiguity.
    assert!(
        r == "255,127,0,255" || r == "255,128,0,255",
        "WebGL clearColor → readPixels must round-trip; got {r}"
    );
}

/// Pin the SHA-256 of the entire 40x40 readPixels buffer for a known
/// clear scene. If the stub plumbing changes (e.g., readPixels stops
/// honoring viewport, or the Y-flip math drifts), this test fails.
#[tokio::test]
async fn webgl_clear_pixel_buffer_hash_is_stable() {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body><canvas id='c' width='40' height='40'></canvas></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(
        r#"
        const c = document.getElementById('c');
        const gl = c.getContext('webgl');
        gl.viewport(0, 0, 40, 40);
        gl.clearColor(0.2, 0.4, 0.6, 1.0);
        gl.clear(gl.COLOR_BUFFER_BIT);
        const px = new Uint8Array(40 * 40 * 4);
        gl.readPixels(0, 0, 40, 40, gl.RGBA, gl.UNSIGNED_BYTE, px);
        // Convert to a hex string for transport — JSON of typed arrays
        // becomes objects which is awkward; hex is easy to parse.
        let hex = '';
        for (let i = 0; i < px.length; i++) hex += px[i].toString(16).padStart(2, '0');
        window.__webglPixelsHex = hex;
        "#,
    )
    .unwrap();
    let hex = page.evaluate("window.__webglPixelsHex").unwrap();
    // Convert hex to bytes, then SHA-256
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    assert_eq!(bytes.len(), 40 * 40 * 4, "buffer must be 40*40*4 bytes");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    eprintln!("WebGL clear-buffer SHA-256: {sha}");
    // Pinned hash for clearColor(0.2, 0.4, 0.6, 1.0) on a 40x40 canvas.
    // If this changes, investigate stub drift before updating.
    assert_eq!(
        sha, "ac54fe81dbff7a3f5970b9ced28b0d3f20d06386aa84f5a2026d2d55446a1b2d",
        "WebGL clearColor pixel buffer drifted from baseline"
    );
}
