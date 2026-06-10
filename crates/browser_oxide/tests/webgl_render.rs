//! WebGL rendering tests — verify clearColor/clear/readPixels produce
//! real pixels.
//!
//! These tests require the `webgl-render` Cargo feature (off by default)
//! which links OSMesa + glow for software GL rendering. With the feature
//! off, getContext('webgl') returns the property-shape stub and these
//! assertions correctly fail. Gated rather than `#[ignore]`'d because
//! the *whole file* depends on the feature — gating at the file level
//! keeps default `cargo test --workspace` green and lets
//! `cargo test --features webgl-render` exercise the real path.

#![cfg(feature = "webgl-render")]

use browser_oxide::stealth;
use browser_oxide::Page;

fn html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head></head><body>{}</body></html>",
        body
    )
}

#[tokio::test]
async fn webgl_clear_color_red() {
    let mut page = Page::from_html(
        &html(
            r#"
        <canvas id="c" width="100" height="100"></canvas>
        <script>
            const gl = document.getElementById('c').getContext('webgl');
            gl.clearColor(1.0, 0.0, 0.0, 1.0);
            gl.clear(gl.COLOR_BUFFER_BIT);
            const pixels = new Uint8Array(4);
            gl.readPixels(50, 50, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
            globalThis.r = pixels[0];
            globalThis.g = pixels[1];
            globalThis.b = pixels[2];
            globalThis.a = pixels[3];
        </script>
    "#,
        ),
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(page.evaluate("r").unwrap(), "255", "red channel");
    assert_eq!(page.evaluate("g").unwrap(), "0", "green channel");
    assert_eq!(page.evaluate("b").unwrap(), "0", "blue channel");
    assert_eq!(page.evaluate("a").unwrap(), "255", "alpha channel");
}

#[tokio::test]
async fn webgl_clear_color_blue() {
    let mut page = Page::from_html(
        &html(
            r#"
        <canvas id="c" width="50" height="50"></canvas>
        <script>
            const gl = document.getElementById('c').getContext('webgl');
            gl.clearColor(0.0, 0.0, 1.0, 1.0);
            gl.clear(gl.COLOR_BUFFER_BIT);
            const pixels = new Uint8Array(4);
            gl.readPixels(25, 25, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
            globalThis.r = pixels[0];
            globalThis.b = pixels[2];
        </script>
    "#,
        ),
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(page.evaluate("r").unwrap(), "0");
    assert_eq!(page.evaluate("b").unwrap(), "255");
}

#[tokio::test]
async fn webgl_to_data_url_has_content() {
    let mut page = Page::from_html(
        &html(
            r#"
        <canvas id="c" width="100" height="100"></canvas>
        <script>
            const gl = document.getElementById('c').getContext('webgl');
            gl.clearColor(0.5, 0.5, 0.5, 1.0);
            gl.clear(gl.COLOR_BUFFER_BIT);
            globalThis.url = document.getElementById('c').toDataURL();
        </script>
    "#,
        ),
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let url = page.evaluate("url").unwrap();
    assert!(url.starts_with("data:image/png;base64,"));
    assert!(
        url.len() > 200,
        "WebGL canvas should have real pixel data, got len={}",
        url.len()
    );
}

#[tokio::test]
async fn webgl_constants_exist() {
    let mut page = Page::from_html(
        &html(
            r#"
        <canvas id="c"></canvas>
        <script>
            const gl = document.getElementById('c').getContext('webgl');
            globalThis.hasCBB = gl.COLOR_BUFFER_BIT === 0x4000;
            globalThis.hasTRI = gl.TRIANGLES === 4;
            globalThis.hasRGBA = gl.RGBA === 0x1908;
            globalThis.hasUB = gl.UNSIGNED_BYTE === 0x1401;
        </script>
    "#,
        ),
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(page.evaluate("hasCBB").unwrap(), "true");
    assert_eq!(page.evaluate("hasTRI").unwrap(), "true");
    assert_eq!(page.evaluate("hasRGBA").unwrap(), "true");
    assert_eq!(page.evaluate("hasUB").unwrap(), "true");
}
