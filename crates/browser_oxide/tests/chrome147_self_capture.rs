//! End-to-end engine self-capture — runs browser_oxide through the same
//! probe that captured the Chrome 147 fixtures, hashes the canvas, and
//! reports the side-by-side comparison. The captured fixture is at
//! `tests/fixtures/chrome147/captured_macos_arm64.json`.
//!
//! This is the master integration test that demonstrates engine coherence
//! at the highest level: same probe code, same engine surface, side-by-side
//! comparable output.

use browser_oxide::Page;
use sha2::{Digest, Sha256};

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

  // chrome.app shape — synchronous part. installState is async (callback)
  // and is asserted separately in chrome147_parity.rs where the test can
  // await it. Here we only assert that it's a function.
  if (typeof chrome !== 'undefined' && typeof chrome.app === 'object') {
    out.chrome_app_keys = Object.getOwnPropertyNames(chrome.app).sort();
    out.chrome_app_is_installed = chrome.app.isInstalled;
    out.chrome_app_InstallState = chrome.app.InstallState;
    out.chrome_app_RunningState = chrome.app.RunningState;
    out.chrome_app_installState_typeof = typeof chrome.app.installState;
    try { out.chrome_app_getDetails_returns = chrome.app.getDetails(); } catch (e) { out.chrome_app_getDetails_returns = "ERR:" + e.message; }
    try { out.chrome_app_getIsInstalled_returns = chrome.app.getIsInstalled(); } catch (e) { out.chrome_app_getIsInstalled_returns = "ERR:" + e.message; }
    try { out.chrome_app_runningState_returns = chrome.app.runningState(); } catch (e) { out.chrome_app_runningState_returns = "ERR:" + e.message; }
  }

  return JSON.stringify(out);
})()
"##;

#[tokio::test]
#[ignore = "not yet implemented: per-realm constructor identity for iframe contexts"]
async fn engine_self_capture_succeeds() {
    // Use an https:// URL so the page is a secure context: the
    // captured Chrome 147 values for Notification.permission and
    // userAgentData were recorded on a secure page, and a few of the
    // expected values below (notification_permission == "default")
    // only hold on a secure context.
    let mut page = Page::from_html_with_url(
        "<!DOCTYPE html><html><body></body></html>",
        "https://example.com/",
        None::<browser_oxide::stealth::StealthProfile>,
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
    assert_eq!(
        parsed["navigator_product_sub"].as_str().unwrap(),
        "20030107"
    );
    assert_eq!(parsed["navigator_webdriver"], false);
    assert_eq!(
        parsed["notification_permission"].as_str().unwrap(),
        "default"
    );

    // ---- Iframe realm purity (matches captured Chrome 147) ----
    assert_eq!(parsed["iframe_navigator_distinct"], true);
    assert_eq!(parsed["iframe_array_distinct"], true);
    assert_eq!(parsed["iframe_array_instanceof"], false);
    assert_eq!(parsed["iframe_function_toString_native"], true);

    // ---- Sub-pixel layout (LayoutUnit 1/64 px) ----
    let w = parsed["rect_width_1_3px"].as_f64().unwrap();
    let h = parsed["rect_height_0_5px"].as_f64().unwrap();
    assert!(
        ((w * 64.0).round() - (w * 64.0)).abs() < 1e-9,
        "width must be 1/64-px multiple: {w}"
    );
    assert!(
        ((h * 64.0).round() - (h * 64.0)).abs() < 1e-9,
        "height must be 1/64-px multiple: {h}"
    );

    // ---- Canvas produces a non-trivial PNG ----
    let url = parsed["canvas_data_url"].as_str().unwrap();
    assert!(
        url.starts_with("data:image/png;base64,"),
        "canvas must emit PNG data URL"
    );
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
        "EditContext",
        "Highlight",
        "CookieStore",
        "WebTransport",
        "FileSystemHandle",
        "BatteryManager",
        "Geolocation",
        "XRSession",
        "Accelerometer",
        "Gyroscope",
    ] {
        let key = format!("ctor_{ctor}");
        assert_eq!(
            parsed[&key].as_str().unwrap(),
            "function",
            "constructor {ctor} must be a function"
        );
    }

    // ---- chrome.app shape (Chromium source-documented surface) ----
    // Real non-automation Chrome 147 exposes exactly these own-property
    // names on chrome.app. Cross-checked against
    // third_party/blink/renderer/extensions/chromeos/chrome.idl.
    let keys = parsed["chrome_app_keys"]
        .as_array()
        .expect("chrome_app_keys must be an array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    let expected_keys = vec![
        "InstallState",
        "RunningState",
        "getDetails",
        "getIsInstalled",
        "installState",
        "isInstalled",
        "runningState",
    ];
    assert_eq!(
        keys, expected_keys,
        "chrome.app own-property-names mismatch"
    );
    assert_eq!(parsed["chrome_app_is_installed"], false);
    assert_eq!(
        parsed["chrome_app_installState_typeof"].as_str().unwrap(),
        "function"
    );
    assert!(
        parsed["chrome_app_getDetails_returns"].is_null(),
        "getDetails() must return null"
    );
    assert_eq!(parsed["chrome_app_getIsInstalled_returns"], false);
    assert_eq!(
        parsed["chrome_app_runningState_returns"].as_str().unwrap(),
        "cannot_run"
    );

    // InstallState / RunningState enum dicts
    let install_state = &parsed["chrome_app_InstallState"];
    assert_eq!(install_state["DISABLED"].as_str().unwrap(), "disabled");
    assert_eq!(install_state["INSTALLED"].as_str().unwrap(), "installed");
    assert_eq!(
        install_state["NOT_INSTALLED"].as_str().unwrap(),
        "not_installed"
    );
    let running_state = &parsed["chrome_app_RunningState"];
    assert_eq!(running_state["CANNOT_RUN"].as_str().unwrap(), "cannot_run");
    assert_eq!(
        running_state["READY_TO_RUN"].as_str().unwrap(),
        "ready_to_run"
    );
    assert_eq!(running_state["RUNNING"].as_str().unwrap(), "running");
}
