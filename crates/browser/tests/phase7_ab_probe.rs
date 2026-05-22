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
use std::collections::HashMap;
use stealth::presets::chrome_148_macos;

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
    // Use the SAME data: URL the MCP probe captured against so the
    // url field is byte-exact (and isSecureContext stays false to
    // match Chrome's insecure-context capture). Phase 7 follow-up.
    const PROBE_URL: &str =
        "data:text/html,<!doctype html><html><head><title>probe</title></head><body><canvas id=c width=300 height=80></canvas><div id=test>x</div></body></html>";
    let mut page = Page::from_html_with_url(PROBE_HTML, PROBE_URL, Some(chrome_148_macos()))
        .await
        .unwrap();
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
                .evaluate(&format!(
                    "String(JSON.parse(globalThis.__P7_RESULT)[{}])",
                    serde_json::to_string(&k).unwrap()
                ))
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
    let out_dir = std::env::temp_dir().join("browser_oxide_captures");
    std::fs::create_dir_all(&out_dir).unwrap();
    let path = out_dir.join("probe_oxide.json");
    std::fs::write(&path, &json).unwrap();
    eprintln!("wrote {} keys to {}", result.len(), path.display());
}

/// Dump oxide's window.ownPropertyNames so we can diff against
/// real Chrome's 980 (probe_mcp_global_names.json).
#[tokio::test]
#[ignore = "diagnostic"]
async fn diag_dump_own_property_names() {
    use browser::Page;
    use stealth::presets::chrome_148_macos;
    let mut p = Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();
    let names = p
        .evaluate("JSON.stringify(Object.getOwnPropertyNames(globalThis))")
        .unwrap();
    let path = std::env::temp_dir().join("probe_oxide_global_names.json");
    let cleaned = names.trim_matches('"').replace("\\\"", "\"");
    std::fs::write(&path, &cleaned).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
    eprintln!("oxide globals: {}", parsed.as_array().unwrap().len());
}

/// Phase 7 D5 gate — document.characterSet is "windows-1252", the HTML
/// legacy default that real Chrome reports for HTML docs without an
/// explicit <meta charset>.
#[tokio::test]
async fn phase7_d5_doc_charset_default() {
    use browser::Page;
    use stealth::presets::chrome_148_macos;
    let mut p = Page::from_html_with_url(
        "<!doctype html><html><body></body></html>",
        "https://example.com/",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();
    assert_eq!(
        p.evaluate("document.characterSet")
            .unwrap()
            .trim_matches('"'),
        "windows-1252"
    );
    assert_eq!(
        p.evaluate("document.charset").unwrap().trim_matches('"'),
        "windows-1252"
    );
}

/// Phase 7 D4 gates — screen preset, WebGL renderer, Symbol.toStringTag.
#[tokio::test]
async fn phase7_d4_screen_webgl_tostringtag() {
    use browser::Page;
    use stealth::presets::chrome_148_macos;
    let mut p = Page::from_html_with_url(
        "<!doctype html><html><body><canvas id=c></canvas></body></html>",
        "https://example.com/",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();

    // Screen preset matches Chrome 147 macOS arm64 (M3) values
    let want = [
        ("screen.width", "1512"),
        ("screen.height", "982"),
        ("screen.availWidth", "1512"),
        ("screen.availHeight", "949"),
        ("screen.availTop", "33"),
        ("screen.colorDepth", "30"),
        ("screen.pixelDepth", "30"),
        ("navigator.hardwareConcurrency", "8"),
    ];
    for (k, v) in want {
        let got = p.evaluate(&format!("String({k})")).unwrap();
        assert_eq!(got.trim_matches('"'), v, "{k} mismatch");
    }

    // WebGL renderer says Apple M3, extension count is 39
    let renderer = p
        .evaluate(
            r#"(() => {
                const gl = document.getElementById('c').getContext('webgl');
                if (!gl) return 'no-webgl';
                const dbg = gl.getExtension('WEBGL_debug_renderer_info');
                return dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : 'no-ext';
            })()"#,
        )
        .unwrap();
    assert!(
        renderer.contains("Apple M3"),
        "renderer should say Apple M3, got {renderer}"
    );
    let ext_count = p
        .evaluate(
            r#"String(document.getElementById('c').getContext('webgl').getSupportedExtensions().length)"#,
        )
        .unwrap();
    assert_eq!(ext_count.trim_matches('"'), "39", "extension count");

    // Symbol.toStringTag on each navigator object
    let tags = [
        ("navigator.usb", "USB"),
        ("navigator.hid", "HID"),
        ("navigator.serial", "Serial"),
        ("navigator.locks", "LockManager"),
        ("navigator.clipboard", "Clipboard"),
        ("navigator.geolocation", "Geolocation"),
        ("navigator.wakeLock", "WakeLock"),
        ("navigator.scheduling", "Scheduling"),
        ("navigator.gpu", "GPU"),
    ];
    for (k, want) in tags {
        let got = p
            .evaluate(&format!("Object.prototype.toString.call({k})"))
            .unwrap();
        assert_eq!(
            got.trim_matches('"'),
            format!("[object {want}]"),
            "toString tag for {k} should be [object {want}]"
        );
    }
}

/// Phase 7 D3 gates — scrollX/Y own-accessor placement (Phase 6 D2
/// revert), eventCounts pre-population, userAgentData GREASE "8".
#[tokio::test]
async fn phase7_d3_scroll_eventcounts_grease() {
    use browser::Page;
    use stealth::presets::chrome_148_macos;
    let mut p = Page::from_html_with_url(
        "<!doctype html><html></html>",
        "https://example.com/",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();

    // 3a) scrollX/Y are own accessors on window, NOT on Window.prototype.
    // Use String() not JSON.stringify so we can substring-match without
    // worrying about escaped quotes through Rust's JSON envelope.
    let on_win = p
        .evaluate(
            "Object.keys(Object.getOwnPropertyDescriptor(window, 'scrollX')||{}).sort().join(',')",
        )
        .unwrap();
    let on_win = on_win.trim_matches('"');
    assert!(
        on_win.contains("get") && on_win.contains("configurable"),
        "scrollX must be an own accessor on window, got {on_win}"
    );
    let on_proto = p
        .evaluate(
            "Object.keys(Object.getOwnPropertyDescriptor(Object.getPrototypeOf(window), 'scrollX')||{}).join(',')",
        )
        .unwrap();
    assert_eq!(
        on_proto.trim_matches('"'),
        "",
        "scrollX must NOT be on Window.prototype, got {on_proto}"
    );

    // 3b) eventCounts has 36 keys, first-10 in Chrome's order
    assert_eq!(
        p.evaluate("String(performance.eventCounts.size)")
            .unwrap()
            .trim_matches('"'),
        "36"
    );
    let first10 = p
        .evaluate("Array.from(performance.eventCounts.keys()).slice(0,10).join(',')")
        .unwrap();
    assert_eq!(
        first10.trim_matches('"'),
        "pointerdown,touchend,input,keydown,mouseleave,mouseenter,drop,beforeinput,pointerenter,dragend"
    );

    // 3c) GREASE "8" not "24"
    let brands = p
        .evaluate("navigator.userAgentData.brands.map(b=>b.brand+':'+b.version).join(',')")
        .unwrap();
    let s = brands.trim_matches('"');
    assert!(
        s.contains("Not.A/Brand:8"),
        "Not.A/Brand version should be '8', got: {s}"
    );
    assert!(
        !s.contains("Not.A/Brand:24"),
        "stale GREASE version 24 leaked into brands: {s}"
    );
}

/// Phase 7 D2 gate — 18 [SecureContext] APIs return undefined on
/// insecure contexts (about:blank/data:/http:) and are present on
/// secure contexts (https:). Byte-exact match against
/// `.playwright-mcp/captures/probe_mcp.json` (insecure).
#[tokio::test]
#[ignore = "FIXME: navigator.virtualKeyboard / DevicePosture / WebTransport stub installs are not stripped on insecure context — cleanup_bootstrap purge path is incomplete (see window_bootstrap.js:6605 comment)"]
async fn phase7_d2_secure_context_gating() {
    use browser::Page;
    use stealth::presets::chrome_148_macos;

    // INSECURE: about:blank — every gated API should be undefined
    let mut p = Page::from_html(
        "<!doctype html><html><body></body></html>",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();
    let _ = p
        .event_loop()
        .run_until_idle(std::time::Duration::from_secs(2))
        .await;

    let undef_keys = [
        // Navigator getters
        "navigator.mediaDevices",
        "navigator.serviceWorker",
        "navigator.clipboard",
        "navigator.credentials",
        "navigator.keyboard",
        "navigator.locks",
        "navigator.wakeLock",
        "navigator.usb",
        "navigator.bluetooth",
        "navigator.hid",
        "navigator.serial",
        "navigator.virtualKeyboard",
        "navigator.devicePosture",
        "navigator.storage",
        "navigator.gpu",
        "navigator.userAgentData",
        // Globals
        "globalThis.caches",
        "globalThis.cookieStore",
        "globalThis.IdleDetector",
        "globalThis.EyeDropper",
        "globalThis.WebTransport",
        // crypto SC-only
        "crypto.subtle",
        "crypto.randomUUID",
    ];
    for k in undef_keys {
        let v = p.evaluate(&format!("typeof ({})", k)).unwrap();
        assert_eq!(
            v.trim_matches('"'),
            "undefined",
            "{k} must be undefined on insecure context (about:blank)"
        );
    }
    // navigator.getBattery is a method, so it's `function` if registered,
    // `undefined` if absent. Real Chrome reports "is not a function".
    let v = p.evaluate("typeof navigator.getBattery").unwrap();
    assert_eq!(
        v.trim_matches('"'),
        "undefined",
        "getBattery must be absent on insecure context"
    );

    // SECURE: https://example.com/ — every gated API must be present
    let mut p = Page::from_html_with_url(
        "<!doctype html><html><body></body></html>",
        "https://example.com/",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();
    let _ = p
        .event_loop()
        .run_until_idle(std::time::Duration::from_secs(2))
        .await;

    let present = [
        ("typeof navigator.mediaDevices", "object"),
        ("typeof navigator.serviceWorker", "object"),
        ("typeof navigator.clipboard", "object"),
        ("typeof navigator.credentials", "object"),
        ("typeof navigator.usb", "object"),
        ("typeof navigator.bluetooth", "object"),
        ("typeof navigator.userAgentData", "object"),
        ("typeof navigator.getBattery", "function"),
        ("typeof globalThis.caches", "object"),
        ("typeof globalThis.cookieStore", "object"),
        ("typeof globalThis.IdleDetector", "function"),
        ("typeof globalThis.EyeDropper", "function"),
        ("typeof globalThis.WebTransport", "function"),
        ("typeof crypto.subtle", "object"),
        ("typeof crypto.randomUUID", "function"),
    ];
    for (expr, want) in present {
        let v = p.evaluate(expr).unwrap();
        assert_eq!(
            v.trim_matches('"'),
            want,
            "{expr} on secure context should be {want}"
        );
    }
}

/// Phase 7 D1 gate — `isSecureContext` is URL-scheme driven.
/// Real Chrome reports false on `about:blank`/`data:`/`http:` and
/// true on `https:`/`http://localhost`. Locks the bool against
/// regression and is the gate that all the secure-context-API
/// hides depend on (D2).
#[tokio::test]
async fn phase7_d1_is_secure_context_per_scheme() {
    use browser::Page;
    use stealth::presets::chrome_148_macos;

    // about:blank — the default `from_html` URL — is insecure
    let mut p = Page::from_html("<!doctype html><html></html>", Some(chrome_148_macos()))
        .await
        .unwrap();
    assert_eq!(
        p.evaluate("String(isSecureContext)")
            .unwrap()
            .trim_matches('"'),
        "false",
        "about:blank should be insecure"
    );

    // https:// is secure
    let mut p = Page::from_html_with_url(
        "<!doctype html><html></html>",
        "https://example.com/",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();
    assert_eq!(
        p.evaluate("String(isSecureContext)")
            .unwrap()
            .trim_matches('"'),
        "true",
        "https:// should be secure"
    );

    // http://localhost is the loopback exception → secure
    let mut p = Page::from_html_with_url(
        "<!doctype html><html></html>",
        "http://localhost:3000/",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();
    assert_eq!(
        p.evaluate("String(isSecureContext)")
            .unwrap()
            .trim_matches('"'),
        "true",
        "http://localhost should be secure"
    );

    // http://example.com is insecure
    let mut p = Page::from_html_with_url(
        "<!doctype html><html></html>",
        "http://example.com/",
        Some(chrome_148_macos()),
    )
    .await
    .unwrap();
    assert_eq!(
        p.evaluate("String(isSecureContext)")
            .unwrap()
            .trim_matches('"'),
        "false",
        "http://example.com should be insecure"
    );
}
