//! End-to-end engine self-capture — runs browser_oxide through the same
//! probe that captured the Chrome 147 fixtures, hashes the canvas, and
//! reports the side-by-side comparison. The captured fixture is at
//! `tests/fixtures/chrome147/captured_macos_arm64.json`.
//!
//! This is the master integration test that demonstrates engine coherence
//! at the highest level: same probe code, same engine surface, side-by-side
//! comparable output.

use browser::Page;
use sha2::{Digest, Sha256};
use stealth;

const CAPTURE_PROBE: &str = r##"
(() => {
  const out = {};
  const r = (k, fn) => { try { out[k] = fn(); } catch (e) { out[k] = "ERR:" + e.message; } };

  r("eval_to_string_length", () => eval.toString().length);
  r("eval_to_string", () => eval.toString());
  r("function_to_string_self", () => Function.prototype.toString.call(Function.prototype.toString));
  r("function_to_string_math_sin", () => Function.prototype.toString.call(Math.sin));
  r("math_cos_13e", () => Math.cos(13 * Math.E));
  r("math_acos_0_123", () => Math.acos(0.123));
  r("math_acosh_1e308", () => Math.acosh(1e308));
  r("navigator_vendor", () => navigator.vendor);
  r("navigator_product_sub", () => navigator.productSub);
  r("navigator_webdriver", () => navigator.webdriver);
  r("notification_permission", () => Notification.permission);

  // Iframe realm purity
  const f = document.createElement('iframe');
  document.body.appendChild(f);
  r("iframe_navigator_distinct", () => f.contentWindow.Navigator !== Navigator);
  r("iframe_array_distinct", () => f.contentWindow.Array !== Array);
  r("iframe_array_instanceof", () => ([] instanceof f.contentWindow.Array));
  r("iframe_function_toString_native", () => f.contentWindow.Function.prototype.toString.call(window.fetch).includes('[native code]'));

  // Sub-pixel
  const probe = document.createElement('div');
  probe.style.cssText = 'width:1.3px;height:0.5px;position:absolute;';
  document.body.appendChild(probe);
  r("rect_width_1_3px", () => probe.getBoundingClientRect().width);
  r("rect_height_0_5px", () => probe.getBoundingClientRect().height);

  // Canvas
  const canvas = document.createElement('canvas');
  canvas.width = 220; canvas.height = 30;
  const ctx = canvas.getContext('2d');
  ctx.textBaseline = "top";
  ctx.font = "14px 'Arial'";
  ctx.textBaseline = "alphabetic";
  ctx.fillStyle = "#f60";
  ctx.fillRect(125, 1, 62, 20);
  ctx.fillStyle = "#069";
  ctx.fillText("Cwm fjordbank glyphs vext quiz", 2, 15);
  ctx.fillStyle = "rgba(102, 204, 0, 0.7)";
  ctx.fillText("Cwm fjordbank glyphs vext quiz", 4, 17);
  out.canvas_data_url = canvas.toDataURL();
  out.canvas_data_url_length = out.canvas_data_url.length;

  // Constructors that must exist
  for (const name of ["EditContext","Highlight","CookieStore","WebTransport","FileSystemHandle","BatteryManager","Geolocation","XRSession","Accelerometer","Gyroscope"]) {
    out["ctor_" + name] = typeof globalThis[name];
  }

  return JSON.stringify(out);
})()
"##;

#[tokio::test]
async fn engine_self_capture_succeeds() {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let raw = page.evaluate(CAPTURE_PROBE).expect("probe must run");
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).expect("probe output must be valid JSON");

    // ---- Things that must match Chrome 147 byte-for-byte ----
    assert_eq!(parsed["eval_to_string_length"], 33);
    assert_eq!(
        parsed["eval_to_string"].as_str().unwrap(),
        "function eval() { [native code] }"
    );
    assert_eq!(
        parsed["function_to_string_self"].as_str().unwrap(),
        "function toString() { [native code] }"
    );
    assert_eq!(
        parsed["function_to_string_math_sin"].as_str().unwrap(),
        "function sin() { [native code] }"
    );

    // ---- Math constants exact match ----
    assert_eq!(parsed["math_cos_13e"], -0.7108118501064332);
    assert_eq!(parsed["math_acos_0_123"], 1.4474840516030247);
    assert_eq!(parsed["math_acosh_1e308"], 709.889355822726);

    // ---- Navigator surface ----
    assert_eq!(parsed["navigator_vendor"].as_str().unwrap(), "Google Inc.");
    assert_eq!(parsed["navigator_product_sub"].as_str().unwrap(), "20030107");
    assert_eq!(parsed["navigator_webdriver"], false);
    assert_eq!(parsed["notification_permission"].as_str().unwrap(), "default");

    // ---- Iframe realm purity (matches captured Chrome 147) ----
    assert_eq!(parsed["iframe_navigator_distinct"], true);
    assert_eq!(parsed["iframe_array_distinct"], true);
    assert_eq!(parsed["iframe_array_instanceof"], false);
    assert_eq!(parsed["iframe_function_toString_native"], true);

    // ---- Sub-pixel layout (LayoutUnit 1/64 px) ----
    let w = parsed["rect_width_1_3px"].as_f64().unwrap();
    let h = parsed["rect_height_0_5px"].as_f64().unwrap();
    assert!(((w * 64.0).round() - (w * 64.0)).abs() < 1e-9, "width must be 1/64-px multiple: {w}");
    assert!(((h * 64.0).round() - (h * 64.0)).abs() < 1e-9, "height must be 1/64-px multiple: {h}");

    // ---- Canvas produces a non-trivial PNG ----
    let url = parsed["canvas_data_url"].as_str().unwrap();
    assert!(url.starts_with("data:image/png;base64,"), "canvas must emit PNG data URL");
    assert!(url.len() > 100, "canvas PNG must be non-trivial");

    // Hash the canvas PNG and report (does not assert match against real
    // Chrome — the libpng-sys swap is required for byte-equal PNG output).
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    eprintln!("Engine canvas SHA-256: {hex}");
    eprintln!("Chrome 147 canvas SHA-256: 2ea9a50df41d1a1d3b367aff9f9c69af8610c3f04b8f38e09ef185d5862624d6 (deferred — libpng-sys required)");

    // ---- Constructor existence (P7 surface) ----
    for ctor in [
        "EditContext", "Highlight", "CookieStore", "WebTransport",
        "FileSystemHandle", "BatteryManager", "Geolocation",
        "XRSession", "Accelerometer", "Gyroscope",
    ] {
        let key = format!("ctor_{ctor}");
        assert_eq!(
            parsed[&key].as_str().unwrap(),
            "function",
            "constructor {ctor} must be a function"
        );
    }
}
