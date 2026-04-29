//! Phase 7 A/B probe — run the SAME comprehensive probe that captured
//! `.playwright-mcp/captures/probe_mcp.json` (insecure data: URL) and
//! `.playwright-mcp/captures/probe_mcp_secure.json` (https://example.com)
//! through our engine, dump the result, and produce a byte-exact diff
//! catalogue.
//!
//! Run with:
//!   cargo test -p browser --test phase7_ab_probe -- --test-threads=1 --nocapture phase7_ab_probe_insecure
//!   cargo test -p browser --test phase7_ab_probe -- --test-threads=1 --nocapture phase7_ab_probe_secure

use browser::Page;
use stealth::presets::chrome_130_macos;
use std::collections::HashMap;

const PROBE_HTML: &str = r#"<!doctype html><html><head><title>probe</title></head><body><canvas id=c width=300 height=80></canvas><div id=test>x</div><script>
globalThis.__P7_RESULT = null;
(async () => {
  const out = {};
  const ts = (label, fn) => { try { out[label] = fn(); } catch (e) { out[label] = `THROW:${e.name}:${e.message}`.slice(0, 120); } };
  const tsa = async (label, fn) => { try { out[label] = await fn(); } catch (e) { out[label] = `THROW:${e.name}:${e.message}`.slice(0, 120); } };

  out.url = location.href;
  out.isSecureContext = isSecureContext;

  for (const k of [
    'userAgent','appName','appCodeName','appVersion','product','productSub','vendor','vendorSub',
    'platform','language','cookieEnabled','doNotTrack','webdriver','hardwareConcurrency','deviceMemory',
    'maxTouchPoints','onLine','pdfViewerEnabled','geolocation','mediaDevices','permissions','plugins',
    'mimeTypes','serviceWorker','clipboard','credentials','keyboard','locks','presentation','wakeLock',
    'usb','bluetooth','hid','serial','virtualKeyboard','devicePosture','windowControlsOverlay',
    'mediaSession','storage','contacts','scheduling','xr','gpu','userAgentData','getBattery',
  ]) ts(`nav.${k}`, () => {
    const v = navigator[k];
    if (v == null) return v === null ? 'null' : 'undefined';
    if (typeof v === 'function') return 'function';
    if (typeof v === 'object') return Object.prototype.toString.call(v);
    return v;
  });
  ts('nav.languages', () => JSON.stringify(navigator.languages));
  ts('nav.plugins.length', () => navigator.plugins.length);
  ts('nav.plugins.names', () => JSON.stringify(Array.from(navigator.plugins).map(p => p.name)));
  ts('nav.userAgentData.brands', () => JSON.stringify(navigator.userAgentData?.brands));
  ts('nav.userAgentData.mobile', () => navigator.userAgentData?.mobile);
  ts('nav.userAgentData.platform', () => navigator.userAgentData?.platform);

  for (const g of ['caches','cookieStore','IdleDetector','EyeDropper','WebTransport']) {
    ts(`global.${g}`, () => typeof globalThis[g]);
  }
  ts('crypto.subtle.typeof', () => typeof crypto.subtle);
  ts('crypto.randomUUID.typeof', () => typeof crypto.randomUUID);
  ts('Notification.permission', () => Notification.permission);

  ts('descr.WindowProto.scrollX', () => JSON.stringify(Object.keys(Object.getOwnPropertyDescriptor(Object.getPrototypeOf(window), 'scrollX') || {})));
  ts('descr.win.scrollX', () => {
    const d = Object.getOwnPropertyDescriptor(window, 'scrollX');
    return d ? JSON.stringify(Object.keys(d)) : 'undefined';
  });
  ts('NavProto.userAgent.descr', () => {
    const d = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(navigator), 'userAgent');
    return d ? JSON.stringify({ hasGet: typeof d.get === 'function', hasSet: typeof d.set === 'function', enumerable: d.enumerable, configurable: d.configurable }) : 'null';
  });

  ts('eventCounts.size', () => performance.eventCounts.size);
  ts('eventCounts.first10keys', () => JSON.stringify(Array.from(performance.eventCounts.keys()).slice(0, 10)));
  ts('eventCounts.click', () => performance.eventCounts.get('click'));

  ts('doc.characterSet', () => document.characterSet);
  ts('doc.compatMode', () => document.compatMode);
  ts('doc.visibilityState', () => document.visibilityState);
  ts('doc.hasFocus', () => document.hasFocus());
  ts('window.ownPropertyNames.length', () => Object.getOwnPropertyNames(globalThis).length);

  for (const k of ['width','height','availWidth','availHeight','availLeft','availTop','colorDepth','pixelDepth']) {
    ts(`screen.${k}`, () => screen[k]);
  }
  ts('screen.orientation.type', () => screen.orientation ? screen.orientation.type : 'undefined');
  ts('screen.orientation.angle', () => screen.orientation ? screen.orientation.angle : 'undefined');

  // WebGL
  const gl = document.getElementById('c').getContext('webgl');
  if (gl) {
    const dbg = gl.getExtension('WEBGL_debug_renderer_info');
    ts('webgl.VENDOR', () => gl.getParameter(gl.VENDOR));
    ts('webgl.RENDERER', () => gl.getParameter(gl.RENDERER));
    ts('webgl.VERSION', () => gl.getParameter(gl.VERSION));
    ts('webgl.SHADING_LANGUAGE_VERSION', () => gl.getParameter(gl.SHADING_LANGUAGE_VERSION));
    ts('webgl.UNMASKED_VENDOR_WEBGL', () => dbg ? gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) : 'no-ext');
    ts('webgl.UNMASKED_RENDERER_WEBGL', () => dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : 'no-ext');
    ts('webgl.extensions.length', () => gl.getSupportedExtensions().length);
  }

  // matchMedia
  for (const q of [
    '(prefers-color-scheme: light)','(prefers-color-scheme: dark)',
    '(pointer: fine)','(pointer: coarse)','(hover: hover)','(forced-colors: none)',
    '(orientation: landscape)','(min-width: 100px)',
  ]) ts(`mql.${q}`, () => matchMedia(q).matches);

  // chrome.*
  ts('chrome.runtime.in', () => 'runtime' in chrome);
  ts('chrome.app.in', () => 'app' in chrome);
  ts('chrome.csi.in', () => 'csi' in chrome);
  ts('chrome.loadTimes.in', () => 'loadTimes' in chrome);
  ts('chrome.csi().keys', () => { try { return JSON.stringify(Object.keys(chrome.csi()).sort()); } catch(e){ return `THROW:${e.name}`; } });
  ts('chrome.loadTimes().keys', () => { try { return JSON.stringify(Object.keys(chrome.loadTimes()).sort()); } catch(e){ return `THROW:${e.name}`; } });

  ts('Date.toString.epoch', () => new Date(0).toString());

  // Async
  await tsa('battery.proto', async () => Object.prototype.toString.call(await navigator.getBattery()));
  await tsa('storage.estimate.quota', async () => (await navigator.storage.estimate()).quota);
  await tsa('perm.geolocation.state', async () => (await navigator.permissions.query({name:'geolocation'})).state);

  globalThis.__P7_RESULT = JSON.stringify(out, null, 2);
})();
</script></body></html>"#;

async fn run_probe(_url_label: &str) -> HashMap<String, String> {
    let mut page = Page::from_html(PROBE_HTML, Some(chrome_130_macos())).await.unwrap();
    let _ = page
        .event_loop()
        .run_until_idle(std::time::Duration::from_secs(3))
        .await;

    // Pull each key one at a time using the same `pull(page, expr)`
    // pattern other Phase parity tests use — avoids the round-trip
    // JSON-of-JSON escape issue that broke a single-shot stringify.
    let parsed_obj = page.evaluate("JSON.parse(globalThis.__P7_RESULT)").unwrap();
    // The result of evaluate("JSON.parse(...)") is V8's serialized form
    // which deno_core stringifies as a JSON object — what we want.
    let mut map = HashMap::new();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&parsed_obj) {
        if let serde_json::Value::Object(obj) = v {
            for (k, val) in obj {
                let s = match val {
                    serde_json::Value::String(s) => s,
                    other => other.to_string(),
                };
                map.insert(k, s);
            }
        }
    } else {
        // Fallback: pull keys one at a time.
        let key_list = page
            .evaluate("JSON.stringify(Object.keys(JSON.parse(globalThis.__P7_RESULT)))")
            .unwrap();
        let cleaned = key_list.trim_matches('"').replace("\\\"", "\"");
        let keys: Vec<String> = serde_json::from_str(&cleaned).unwrap_or_default();
        for k in keys {
            let v = page
                .evaluate(&format!("String(JSON.parse(globalThis.__P7_RESULT)[{}])", serde_json::to_string(&k).unwrap()))
                .unwrap();
            map.insert(k, v.trim_matches('"').to_string());
        }
    }
    map
}

#[tokio::test]
#[ignore = "captures the comprehensive A/B probe result for diff against MCP"]
async fn phase7_ab_probe_capture_oxide() {
    let result = run_probe("oxide").await;
    let json = serde_json::to_string_pretty(&result).unwrap();
    let out_dir = "/Users/yfedoseev/Projects/browser_oxide/.playwright-mcp/captures";
    std::fs::create_dir_all(out_dir).unwrap();
    let path = format!("{out_dir}/probe_oxide.json");
    std::fs::write(&path, &json).unwrap();
    eprintln!("wrote {} keys to {path}", result.len());
}

/// Phase 7 D1 gate — `isSecureContext` is URL-scheme driven.
/// Real Chrome reports false on `about:blank`/`data:`/`http:` and
/// true on `https:`/`http://localhost`. Locks the bool against
/// regression and is the gate that all the secure-context-API
/// hides depend on (D2).
#[tokio::test]
async fn phase7_d1_is_secure_context_per_scheme() {
    use browser::Page;
    use stealth::presets::chrome_130_macos;

    // about:blank — the default `from_html` URL — is insecure
    let mut p = Page::from_html("<!doctype html><html></html>", Some(chrome_130_macos()))
        .await
        .unwrap();
    assert_eq!(p.evaluate("String(isSecureContext)").unwrap().trim_matches('"'), "false",
        "about:blank should be insecure");

    // https:// is secure
    let mut p = Page::from_html_with_url(
        "<!doctype html><html></html>",
        "https://example.com/",
        Some(chrome_130_macos()),
    )
    .await
    .unwrap();
    assert_eq!(p.evaluate("String(isSecureContext)").unwrap().trim_matches('"'), "true",
        "https:// should be secure");

    // http://localhost is the loopback exception → secure
    let mut p = Page::from_html_with_url(
        "<!doctype html><html></html>",
        "http://localhost:3000/",
        Some(chrome_130_macos()),
    )
    .await
    .unwrap();
    assert_eq!(p.evaluate("String(isSecureContext)").unwrap().trim_matches('"'), "true",
        "http://localhost should be secure");

    // http://example.com is insecure
    let mut p = Page::from_html_with_url(
        "<!doctype html><html></html>",
        "http://example.com/",
        Some(chrome_130_macos()),
    )
    .await
    .unwrap();
    assert_eq!(p.evaluate("String(isSecureContext)").unwrap().trim_matches('"'), "false",
        "http://example.com should be insecure");
}
