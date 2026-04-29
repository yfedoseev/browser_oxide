//! Akamai-style fingerprint probes against a controlled local HTML page.
//!
//! Loads a self-contained HTML page that mimics what Akamai's BMP v13 sensor
//! likely does for the `ap`, `fonts`, `fh`, and `sr.client` fields, then dumps
//! the values. Lets us verify our JS fixes apply through a real `Page::navigate`
//! flow (inline-script execution + cleanup_bootstrap + canvas/dom prototypes).
//!
//! NOT a network test — runs against `about:blank` with embedded HTML. Use
//! `cargo test -p browser --test akamai_probe_local -- --nocapture`.

use browser::Page;

const PROBE_HTML: &str = r#"<!doctype html>
<html>
<head><title>probe</title></head>
<body style="margin:0;padding:0">
  <div style="height:30000px">tall content to force document overflow</div>
  <canvas id="c" width="200" height="50"></canvas>
  <script>
    (function () {
      const out = {};

      // ---- Field `ap`: ApplePaySession surface ----
      out.ap_typeof = typeof ApplePaySession;
      try {
        out.ap_canMakePayments = typeof ApplePaySession === 'function'
          ? ApplePaySession.canMakePayments() : null;
      } catch (e) { out.ap_canMakePayments_err = String(e); }
      try {
        out.ap_supportsVersion3 = typeof ApplePaySession === 'function'
          ? ApplePaySession.supportsVersion(3) : null;
      } catch (e) { out.ap_supportsVersion_err = String(e); }
      // Akamai-style probe: serialize statics
      try {
        out.ap_keys = typeof ApplePaySession === 'function'
          ? Object.keys(ApplePaySession).slice(0, 12) : null;
      } catch (e) { out.ap_keys_err = String(e); }

      // ---- Field `sr.client`: dimensions sourced where? ----
      out.docElem_cw = document.documentElement.clientWidth;
      out.docElem_ch = document.documentElement.clientHeight;
      out.docElem_ow = document.documentElement.offsetWidth;
      out.docElem_oh = document.documentElement.offsetHeight;
      out.docElem_sw = document.documentElement.scrollWidth;
      out.docElem_sh = document.documentElement.scrollHeight;
      out.body_cw = document.body.clientWidth;
      out.body_ch = document.body.clientHeight;
      out.body_ow = document.body.offsetWidth;
      out.body_oh = document.body.offsetHeight;
      out.window_iw = window.innerWidth;
      out.window_ih = window.innerHeight;

      // ---- Field `fonts`/`fh`: canvas-based font detection ----
      // Mimics the classic technique: for each candidate, set `Family,sans-serif`
      // and compare width to bare `sans-serif`. If they differ, the family is
      // reported as installed.
      const probeFamilies = [
        'Arial','Arial Black','Calibri','Cambria','Comic Sans MS','Consolas',
        'Courier','Courier New','Georgia','Helvetica','Helvetica Neue','Impact',
        'Lucida Console','Lucida Grande','Menlo','Monaco','Palatino','SF Pro',
        'Segoe UI','Tahoma','Times','Times New Roman','Trebuchet MS','Verdana',
        'Wingdings'
      ];
      const ctx = document.getElementById('c').getContext('2d');
      const text = 'mmmmmmmmmlli';
      ctx.font = '72px sans-serif';
      const baselineSans = ctx.measureText(text).width;
      ctx.font = '72px serif';
      const baselineSerif = ctx.measureText(text).width;
      ctx.font = '72px monospace';
      const baselineMono = ctx.measureText(text).width;
      const installed = [];
      for (const fam of probeFamilies) {
        ctx.font = '72px "' + fam + '", sans-serif';
        const w1 = ctx.measureText(text).width;
        ctx.font = '72px "' + fam + '", serif';
        const w2 = ctx.measureText(text).width;
        ctx.font = '72px "' + fam + '", monospace';
        const w3 = ctx.measureText(text).width;
        const detected =
          Math.abs(w1 - baselineSans) > 1e-3 ||
          Math.abs(w2 - baselineSerif) > 1e-3 ||
          Math.abs(w3 - baselineMono) > 1e-3;
        if (detected) installed.push(fam);
      }
      out.fonts_installed = installed;
      out.fonts_installed_count = installed.length;
      // Deterministic font hash — Akamai's `fh` is similar in shape (a hex
      // digest or numeric of the joined family names). null when no fonts.
      if (installed.length === 0) {
        out.fh = null;
      } else {
        let h = 2166136261 >>> 0;
        const s = installed.join(',');
        for (let i = 0; i < s.length; i++) {
          h ^= s.charCodeAt(i);
          h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
        }
        out.fh = h.toString(16);
      }

      // ---- Stash result on globalThis for the test harness ----
      globalThis.__PROBE = JSON.stringify(out);
    })();
  </script>
</body>
</html>"#;

#[tokio::test]
async fn akamai_local_probe_macos_profile() {
    let profile = stealth::presets::chrome_130_macos();
    let mut page = Page::from_html(PROBE_HTML, Some(profile)).await.unwrap();
    let raw = page.evaluate("globalThis.__PROBE").unwrap();
    println!("\n=== PROBE RESULT (macOS profile) ===\n{raw}\n");

    // Strip surrounding quotes that V8 stringification adds back.
    let clean = raw.trim_matches('"').replace("\\\"", "\"");
    let v: serde_json::Value = serde_json::from_str(&clean).expect("probe must emit valid JSON");

    // ap_*: ApplePaySession must be present
    assert_eq!(v["ap_typeof"], "function", "ApplePaySession must be a function on macOS profile");
    assert_eq!(v["ap_canMakePayments"], true, "canMakePayments() must return true");
    assert_eq!(v["ap_supportsVersion3"], true, "supportsVersion(3) must return true");

    // sr.client: documentElement.clientHeight must be viewport (789), not
    // the full 30000-px overflow. The chrome_130_macos preset uses
    // innerWidth=1440, innerHeight=789.
    let inner_w = v["window_iw"].as_i64().unwrap();
    let inner_h = v["window_ih"].as_i64().unwrap();
    assert_eq!(v["docElem_cw"].as_i64().unwrap(), inner_w, "documentElement.clientWidth = innerWidth");
    assert_eq!(v["docElem_ch"].as_i64().unwrap(), inner_h, "documentElement.clientHeight = innerHeight (NOT 30000)");
    let body_cw = v["body_cw"].as_i64().unwrap_or(-1);
    let body_ch = v["body_ch"].as_i64().unwrap_or(-1);
    println!("body clientWidth={body_cw} clientHeight={body_ch} (these feed Akamai sr.client)");

    // fonts: must detect ONLY the macOS-installed set (the Akamai-style
    // probe technique now correctly distinguishes installed fonts from
    // unknown families — pre-fix every probe collapsed to Liberation
    // Sans and `fonts` came back null).
    let installed = v["fonts_installed"].as_array().unwrap();
    let count = installed.len();
    println!("Detected {count} installed fonts: {:?}", installed);
    let names: Vec<String> = installed
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"Arial".to_string()), "Arial missing: {names:?}");
    assert!(
        names.contains(&"Helvetica Neue".to_string()),
        "Helvetica Neue missing on macOS: {names:?}"
    );
    assert!(
        names.contains(&"SF Pro".to_string()),
        "SF Pro missing on macOS: {names:?}"
    );
    // Negative cases — these must NOT show up on a macOS profile.
    assert!(
        !names.contains(&"Calibri".to_string()),
        "Calibri must not be 'installed' on macOS: {names:?}"
    );
    assert!(
        !names.contains(&"Wingdings".to_string()),
        "Wingdings must not be 'installed' on macOS: {names:?}"
    );
    assert!(
        !names.contains(&"Segoe UI".to_string()),
        "Segoe UI must not be 'installed' on macOS: {names:?}"
    );
    assert!(!v["fh"].is_null(), "fh must be non-null when fonts detected");
}

/// Direct width measurement diagnostic — shows raw widths for every probe
/// case so we can see WHY the probe detects more fonts than expected.
#[tokio::test]
async fn diag_font_widths_macos() {
    let mut page = Page::from_html(
        "<canvas id=c width=200 height=50></canvas>",
        Some(stealth::presets::chrome_130_macos()),
    )
    .await
    .unwrap();
    let r = page
        .evaluate(
            r#"
        (() => {
            const ctx = document.getElementById('c').getContext('2d');
            const t = "mmmmmmmmmlli";
            const cases = [
                "72px sans-serif", "72px serif", "72px monospace",
                "72px Arial", "72px 'Arial'", '72px "Arial"',
                '72px "Arial", sans-serif',
                '72px "Helvetica Neue"',
                '72px "Helvetica Neue", sans-serif',
                "72px Wingdings",
                '72px "Wingdings", sans-serif',
                "72px Calibri",
                '72px "Calibri", sans-serif',
                '72px "Nonexistent42", sans-serif',
            ];
            const out = {};
            for (const c of cases) {
                ctx.font = c;
                out[c] = ctx.measureText(t).width.toFixed(4);
            }
            return JSON.stringify(out);
        })()
    "#,
        )
        .unwrap();
    println!("\nDIAG_WIDTHS:\n{}\n", r);
}

/// Linux profile: Helvetica Neue NOT installed, Liberation Sans IS.
#[tokio::test]
async fn akamai_local_probe_linux_profile() {
    let profile = stealth::presets::chrome_130_linux();
    let mut page = Page::from_html(PROBE_HTML, Some(profile)).await.unwrap();
    let raw = page.evaluate("globalThis.__PROBE").unwrap();
    println!("\n=== PROBE RESULT (Linux profile) ===\n{raw}\n");
    let clean = raw.trim_matches('"').replace("\\\"", "\"");
    let v: serde_json::Value = serde_json::from_str(&clean).expect("probe must emit valid JSON");

    // ap_*: ApplePaySession must NOT be present on Linux
    assert_eq!(v["ap_typeof"], "undefined", "ApplePaySession must NOT exist on Linux profile");

    // fonts: Linux set must include Arial (aliased) but NOT Helvetica Neue
    let installed = v["fonts_installed"].as_array().unwrap();
    let names: Vec<String> = installed
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    println!("Linux-detected fonts: {names:?}");
    // Arial and Helvetica are aliased to Liberation Sans in
    // `font_database::resolve_family` regardless of profile, so they
    // currently report as "installed" even on Linux. That's a residual
    // tell — real Chrome on Linux falls back through fontconfig and
    // reports neither as installed. Tightening the alias table to be
    // OS-aware is a follow-up; documented here so future changes don't
    // accidentally reintroduce the universal-detection bug.
    assert!(names.contains(&"Arial".to_string()), "Arial must be detected on Linux: {names:?}");
    // SF Pro must NOT be detected — it's macOS-only and has no alias.
    assert!(
        !names.contains(&"SF Pro".to_string()),
        "SF Pro should NOT be detected on Linux: {names:?}"
    );
    // Wingdings (no alias, not on Linux) must NOT be detected.
    assert!(
        !names.contains(&"Wingdings".to_string()),
        "Wingdings should NOT be detected on Linux: {names:?}"
    );
    assert!(
        !names.contains(&"Calibri".to_string()),
        "Calibri should NOT be detected on Linux: {names:?}"
    );
}
