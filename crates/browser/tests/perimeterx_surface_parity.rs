//! PerimeterX (HUMAN Bot Defender) — JS surface parity check.
//!
//! Walmart's active bot detector is PerimeterX, not Akamai (Akamai is
//! CSP-blocked from JS execution at walmart). PerimeterX runs an
//! in-page sensor that fingerprints the JS surface and POSTs to
//! `collector-<appId>.px-cloud.net/api/v2/collector`. A favorable
//! response sets `_px3=<hash>:<hmac>:1000:<token>` — score 1000 = passed.
//!
//! This test asserts our JS surface matches what real Chrome (Playwright
//! captured at `.playwright-mcp/captures/walmart_v13_state.json`) shows
//! on the surface PX is publicly known to probe — see
//! `docs/ANTIBOT_RESEARCH_2026.md` and the ScrapFly write-up
//! https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping
//!
//! Probes here are intentionally focused on **detection surfaces** that
//! are documented in public anti-bot writeups, not on the encrypted
//! sensor payload itself. The goal is parity with a real Chrome JS
//! environment so that whatever PX inspects, it sees Chrome.

use browser::Page;

const PROBE_HTML: &str = r#"<!doctype html>
<html><body><script>
(() => {
  const out = {};

  // ---- Automation markers — must all be ABSENT --------------------
  // PerimeterX specifically scans `window` and `document` enumerables
  // for known automation-tool leaks. See the `z` field in Akamai BMP
  // v13 for the canonical list of SHA-1 hashes; PX uses the same set
  // plus a few CDP-specific ones.
  const AUTOMATION_MARKERS = [
    'webdriver', 'driver_evaluate', 'driver_unwrapped', '$cdc_asdjflasutopfhvcZLmcfl_',
    '$chrome_asyncScriptInfo', '__webdriver_evaluate', '__selenium_evaluate',
    '__webdriver_script_function', '__webdriver_script_func', '__webdriver_script_fn',
    '__fxdriver_evaluate', '__driver_unwrapped', '__webdriver_unwrapped',
    '__driver_evaluate', '__selenium_unwrapped', '__fxdriver_unwrapped',
    '_phantom', '__nightmare', '_selenium', 'callPhantom', 'callSelenium',
    '_Selenium_IDE_Recorder', 'domAutomation', 'domAutomationController',
    'spawn',
  ];
  out.markers_on_window = AUTOMATION_MARKERS.filter(m => m in window);
  out.markers_on_document = AUTOMATION_MARKERS.filter(m => m in document);

  // ---- Navigator surface PX inspects ------------------------------
  out.nav_webdriver = navigator.webdriver;
  out.nav_vendor = navigator.vendor;
  out.nav_plugins_length = navigator.plugins ? navigator.plugins.length : null;
  out.nav_languages = navigator.languages;
  out.nav_hardwareConcurrency = navigator.hardwareConcurrency;
  out.nav_platform = navigator.platform;

  // ---- Native-code masking — Function.prototype.toString.call(...)
  // PX/HUMAN runs this exact check on key surface APIs. The expected
  // form is: function NAME() { [native code] }
  // Any drift (no `function `, missing `[native code]`, wrong name)
  // is a bot tell.
  const nativeCheck = (fn, expectedName) => {
    try {
      const s = Function.prototype.toString.call(fn);
      const ok = s.indexOf('[native code]') !== -1;
      const nameOk = s.indexOf('function ' + expectedName) === 0
        || s.indexOf('function ' + expectedName + '(') !== -1;
      return { s: s.slice(0, 80), native: ok, nameOk };
    } catch (e) { return { err: String(e) }; }
  };
  out.tostring_query = nativeCheck(navigator.permissions.query, 'query');
  out.tostring_getUserMedia = navigator.mediaDevices ? nativeCheck(navigator.mediaDevices.getUserMedia, 'getUserMedia') : null;
  out.tostring_fetch = nativeCheck(window.fetch, 'fetch');
  out.tostring_setTimeout = nativeCheck(window.setTimeout, 'setTimeout');
  out.tostring_addEventListener = nativeCheck(window.addEventListener, 'addEventListener');
  out.tostring_canvasGetContext = nativeCheck(HTMLCanvasElement.prototype.getContext, 'getContext');

  // ---- Chrome runtime presence — PX `if (window.chrome)` branch ---
  out.has_chrome_obj = typeof window.chrome !== 'undefined';
  out.chrome_runtime_keys = window.chrome && window.chrome.runtime
    ? Object.keys(window.chrome.runtime).slice(0, 10) : null;
  out.chrome_app_keys = window.chrome && window.chrome.app
    ? Object.keys(window.chrome.app) : null;

  // ---- Iframe isolation check — PX runs APIs inside a clean iframe
  // and compares the result against the parent. If we monkey-patch
  // navigator.webdriver in the parent realm but the iframe's own
  // Navigator prototype still returns true, PX flags us.
  const iframe = document.createElement('iframe');
  iframe.style.display = 'none';
  document.body.appendChild(iframe);
  try {
    out.iframe_webdriver = iframe.contentWindow.navigator.webdriver;
    out.iframe_vendor = iframe.contentWindow.navigator.vendor;
    out.iframe_chrome_present = typeof iframe.contentWindow.chrome !== 'undefined';
    // PX's stronger check: compare prototype chains
    const parentNav = Object.getPrototypeOf(navigator);
    const iframeNav = Object.getPrototypeOf(iframe.contentWindow.navigator);
    out.iframe_navProto_ctor_match =
      parentNav.constructor.name === iframeNav.constructor.name;
  } catch (e) {
    out.iframe_err = String(e);
  } finally {
    iframe.remove();
  }

  // ---- _pxAppId hook surface — PX bootstrap sets this; if a site
  // pre-sets it (which it does on inline injection) the sensor reads
  // and validates against a server-issued challenge. Not a parity
  // assertion — just verifies no global pre-pollution.
  out.pxAppId_undefined = typeof window._pxAppId === 'undefined';
  out.pxAppId_writable = !Object.getOwnPropertyDescriptor(window, '_pxAppId');

  // ---- Five Chrome-surface parity gaps from
  // docs/CHROME_JS_SURFACE_PARITY_2026_04_29.md ---------------------

  // 1) BatteryManager class identity
  out.battery_pending = "pending";
  navigator.getBattery().then(b => {
    out.battery_ctor_name = Object.getPrototypeOf(b).constructor.name;
    out.battery_is_eventTarget = b instanceof EventTarget;
    out.battery_toStringTag = Object.prototype.toString.call(b);
    out.battery_charging = b.charging;
    out.battery_pending = "done";
  }).catch(e => { out.battery_err = String(e); out.battery_pending = "errored"; });

  // 2) navigator.storage.estimate quota plausibility
  out.storage_pending = "pending";
  navigator.storage.estimate().then(s => {
    out.storage_quota = s.quota;
    out.storage_quota_gb = (s.quota / 1024 / 1024 / 1024).toFixed(1);
    out.storage_has_usage_details = !!s.usageDetails;
    out.storage_pending = "done";
  }).catch(e => { out.storage_err = String(e); out.storage_pending = "errored"; });

  // 3) userAgentData GREASE brand literal
  if (navigator.userAgentData && navigator.userAgentData.brands) {
    out.uad_brands = navigator.userAgentData.brands.map(b => b.brand);
    out.uad_has_correct_grease = navigator.userAgentData.brands.some(b =>
      b.brand === "Not.A/Brand" || b.brand === "Not.A.Brand" || /Not.?A.?Brand/.test(b.brand)
    );
    out.uad_no_old_grease = !navigator.userAgentData.brands.some(b => b.brand === "Not-A.Brand");
  }

  // 4) Touch.prototype Symbol.toStringTag
  if (typeof Touch === 'function') {
    try {
      const t = new Touch({ identifier: 1, target: document.body });
      out.touch_toString = Object.prototype.toString.call(t);
    } catch (e) { out.touch_err = String(e); }
  }

  // 5) RTCPeerConnection emits at least one host candidate
  out.rtc_candidates = [];
  out.rtc_pending = "pending";
  try {
    const pc = new RTCPeerConnection();
    pc.onicecandidate = (ev) => {
      if (ev.candidate && ev.candidate.candidate) {
        out.rtc_candidates.push(ev.candidate.candidate);
      } else {
        out.rtc_pending = "done";
      }
    };
    pc.createDataChannel("probe");
    pc.createOffer().then(o => pc.setLocalDescription(o));
  } catch (e) { out.rtc_err = String(e); out.rtc_pending = "errored"; }

  globalThis.__PX_PROBE = JSON.stringify(out);
  // Re-stringify after async settles
  setTimeout(() => { globalThis.__PX_PROBE = JSON.stringify(out); }, 200);
})();
</script></body></html>"#;

/// Read a single PX probe field as a plain string. Avoids the
/// double-stringify escaping that breaks raw JSON parse.
fn pull(page: &mut Page, expr: &str) -> String {
    match page.evaluate(&format!("String({expr})")) {
        Ok(s) => s.trim_matches('"').to_string(),
        Err(e) => panic!("pull({:?}) failed: {}", expr, e),
    }
}

#[tokio::test]
async fn perimeterx_surface_macos() {
    // PerimeterX probe references [SecureContext] APIs (mediaDevices,
    // userAgentData, etc.) — load over https:// so they're exposed.
    // Phase 7.
    let mut page = Page::from_html_with_url(
        PROBE_HTML,
        "https://example.com/",
        Some(stealth::presets::chrome_130_macos()),
    )
        .await
        .unwrap();

    // Per-field reads — see akamai_v13_probe_parity for why we don't
    // pull the whole struct as a single JSON string.
    let win_markers = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).markers_on_window.join(',')");
    let doc_markers = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).markers_on_document.join(',')");
    let webdriver = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).nav_webdriver");
    let vendor = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).nav_vendor");
    let plugins_len = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).nav_plugins_length");
    let has_chrome = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).has_chrome_obj");
    let iframe_wd = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).iframe_webdriver)");
    let iframe_chrome = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).iframe_chrome_present)");
    let iframe_proto_match = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).iframe_navProto_ctor_match)");

    let native_query = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).tostring_query.native");
    let native_fetch = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).tostring_fetch.native");
    let native_setTimeout = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).tostring_setTimeout.native");
    let native_addEventListener = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).tostring_addEventListener.native");
    let native_getContext = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).tostring_canvasGetContext.native");

    println!("\n=== PerimeterX surface probe (macOS profile) ===");
    println!("  webdriver:           {webdriver}");
    println!("  vendor:              {vendor}");
    println!("  plugins.length:      {plugins_len}");
    println!("  window.chrome:       {has_chrome}");
    println!("  marker leaks (win):  '{win_markers}'");
    println!("  marker leaks (doc):  '{doc_markers}'");
    println!("  iframe webdriver:    {iframe_wd}");
    println!("  iframe chrome:       {iframe_chrome}");
    println!("  iframe proto match:  {iframe_proto_match}");
    println!("  Function.toString native-code presence:");
    println!("    permissions.query:        {native_query}");
    println!("    fetch:                    {native_fetch}");
    println!("    setTimeout:               {native_setTimeout}");
    println!("    addEventListener:         {native_addEventListener}");
    println!("    canvas.getContext:        {native_getContext}");

    // --- D10 surfaces: visualViewport / InputDeviceCapabilities / MediaSession
    let vv_typeof = pull(&mut page, "typeof visualViewport");
    let vv_w = pull(&mut page, "visualViewport.width");
    let vv_h = pull(&mut page, "visualViewport.height");
    let vv_scale = pull(&mut page, "String(visualViewport.scale)");
    let vv_proto = pull(
        &mut page,
        "Object.prototype.toString.call(visualViewport)",
    );
    let vv_is_event_target = pull(&mut page, "String(visualViewport instanceof EventTarget)");
    let idc_typeof = pull(&mut page, "typeof InputDeviceCapabilities");
    let idc_fires_touch = pull(
        &mut page,
        "String(new InputDeviceCapabilities({firesTouchEvents: true}).firesTouchEvents)",
    );
    let ms_proto = pull(
        &mut page,
        "Object.prototype.toString.call(navigator.mediaSession)",
    );
    let ms_state = pull(&mut page, "navigator.mediaSession.playbackState");
    let ms_set_action_typeof = pull(&mut page, "typeof navigator.mediaSession.setActionHandler");
    println!("  visualViewport: typeof={vv_typeof} {vv_w}x{vv_h} scale={vv_scale} proto={vv_proto} EventTarget={vv_is_event_target}");
    println!("  InputDeviceCapabilities: typeof={idc_typeof} firesTouchEvents-roundtrip={idc_fires_touch}");
    println!("  MediaSession: proto={ms_proto} playbackState={ms_state} setActionHandler={ms_set_action_typeof}");

    // --- Hard parity gates ------------------------------------------
    assert_eq!(win_markers, "", "Automation markers leaked on window");
    assert_eq!(doc_markers, "", "Automation markers leaked on document");
    assert_eq!(webdriver, "false", "navigator.webdriver must be false");
    assert_eq!(vendor, "Google Inc.");
    assert!(
        plugins_len.parse::<i64>().unwrap() >= 5,
        "navigator.plugins must enumerate ≥5 Chrome stub plugins, got {plugins_len}"
    );
    assert_eq!(has_chrome, "true", "window.chrome must exist on macOS Chrome profile");

    assert_eq!(native_query, "true", "navigator.permissions.query must serialize native");
    assert_eq!(native_fetch, "true", "fetch must serialize native");
    assert_eq!(native_setTimeout, "true", "setTimeout must serialize native");
    assert_eq!(native_addEventListener, "true", "addEventListener must serialize native");
    assert_eq!(native_getContext, "true", "HTMLCanvasElement.getContext must serialize native");

    assert_eq!(iframe_wd, "false", "iframe navigator.webdriver must be false");
    assert_eq!(iframe_chrome, "true", "iframe must also have window.chrome");
    assert_eq!(
        iframe_proto_match, "true",
        "iframe's Navigator prototype constructor must match parent"
    );

    // --- Five Chrome-surface gaps locked in --------------------------
    // Drive a few extra event-loop ticks so the async probes settle.
    for _ in 0..10 {
        let _ = page.evaluate("0").unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }

    let battery_ctor_name = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).battery_ctor_name");
    let battery_is_event_target = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).battery_is_eventTarget)");
    let battery_to_string_tag = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).battery_toStringTag");
    let storage_quota_gb = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).storage_quota_gb");
    let uad_correct = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).uad_has_correct_grease)");
    let uad_no_old = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).uad_no_old_grease)");
    let touch_to_string = pull(&mut page, "JSON.parse(globalThis.__PX_PROBE).touch_toString");
    let rtc_candidates_n = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).rtc_candidates.length)");
    let rtc_first = pull(&mut page, "String(JSON.parse(globalThis.__PX_PROBE).rtc_candidates[0]||'')");

    println!("\n--- Five Chrome-surface fixes ---");
    println!("  BatteryManager.constructor.name:   {battery_ctor_name}");
    println!("  battery instanceof EventTarget:    {battery_is_event_target}");
    println!("  toString.call(battery):            {battery_to_string_tag}");
    println!("  storage.estimate quota (GB):       {storage_quota_gb}");
    println!("  userAgentData GREASE correct:      {uad_correct}");
    println!("  userAgentData old hyphen absent:   {uad_no_old}");
    println!("  toString.call(new Touch(...)):     {touch_to_string}");
    println!("  RTC ICE candidates emitted:        {rtc_candidates_n}");
    println!("    first:                           {rtc_first}");

    assert_eq!(battery_ctor_name, "BatteryManager", "navigator.getBattery() must resolve a BatteryManager instance");
    assert_eq!(battery_is_event_target, "true", "BatteryManager must inherit EventTarget");
    assert_eq!(battery_to_string_tag, "[object BatteryManager]");
    let quota_gb: f64 = storage_quota_gb.parse().unwrap();
    assert!(quota_gb >= 50.0, "storage.estimate quota must be ≥50 GB (real Chrome ~120 GB), got {quota_gb} GB");
    assert_eq!(uad_correct, "true", "userAgentData must contain Chrome's GREASE brand 'Not.A/Brand'");
    assert_eq!(uad_no_old, "true", "userAgentData must NOT contain the old 'Not-A.Brand' literal");
    assert_eq!(touch_to_string, "[object Touch]", "Touch.prototype Symbol.toStringTag missing");
    let cand_n: i64 = rtc_candidates_n.parse().unwrap();
    assert!(cand_n >= 1, "RTCPeerConnection must emit ≥1 ICE candidate before the null terminator");
    assert!(rtc_first.contains(".local"), "RTC candidate must be mDNS-anonymized (.local), got: {rtc_first}");
    assert!(rtc_first.contains("typ host"), "RTC candidate must be host type, got: {rtc_first}");

    // --- D10 surfaces parity gates -----------------------------------
    assert_eq!(vv_typeof, "object", "window.visualViewport must exist");
    assert_eq!(vv_proto, "[object VisualViewport]", "Symbol.toStringTag missing");
    assert_eq!(vv_is_event_target, "true", "VisualViewport must extend EventTarget");
    let vv_w_int: i64 = vv_w.parse().unwrap();
    let vv_h_int: i64 = vv_h.parse().unwrap();
    assert!(vv_w_int > 0 && vv_h_int > 0, "visualViewport size must be non-zero, got {vv_w}x{vv_h}");
    assert_eq!(vv_scale, "1");
    assert_eq!(idc_typeof, "function", "InputDeviceCapabilities constructor must exist");
    assert_eq!(idc_fires_touch, "true", "firesTouchEvents must round-trip from constructor init");
    assert_eq!(ms_proto, "[object MediaSession]", "navigator.mediaSession must be a MediaSession instance");
    assert_eq!(ms_state, "none", "default playbackState must be 'none' per spec");
    assert_eq!(ms_set_action_typeof, "function", "setActionHandler must be callable");

    // --- Phase 6 D1: Date.toString TZ + matchMedia full features --
    // Real Chrome on macOS / America/Los_Angeles produces:
    //   "Tue Apr 29 2026 13:02:46 GMT-0700 (Pacific Daylight Time)"
    // Detection libraries grep `new Date().toString()` for "GMT-" or
    // `(Pacific Daylight Time)` to verify the TZ matches the
    // navigator.userAgent platform claim. UTC output is a hard tell.
    let date_to_string = pull(&mut page, "new Date('2026-04-29T20:02:46Z').toString()");
    let date_has_gmt_offset = pull(
        &mut page,
        "String(/GMT[-+]\\d{4}/.test(new Date('2026-04-29T20:02:46Z').toString()))",
    );
    let date_has_long_name = pull(
        &mut page,
        "String(/\\(.+\\)$/.test(new Date('2026-04-29T20:02:46Z').toString()))",
    );
    let date_to_time_string = pull(&mut page, "new Date('2026-04-29T20:02:46Z').toTimeString()");
    let date_to_date_string = pull(&mut page, "new Date('2026-04-29T20:02:46Z').toDateString()");
    let date_to_string_native = pull(
        &mut page,
        "Function.prototype.toString.call(Date.prototype.toString)",
    );
    println!("\n--- Phase 6 D1: Date.toString TZ format ---");
    println!("  Date.toString:                {date_to_string}");
    println!("  has GMT[+-]####:              {date_has_gmt_offset}");
    println!("  has long-name suffix:         {date_has_long_name}");
    println!("  Date.toTimeString:            {date_to_time_string}");
    println!("  Date.toDateString:            {date_to_date_string}");
    println!("  toString native-mask:         {date_to_string_native}");
    assert_eq!(
        date_has_gmt_offset, "true",
        "Date.toString must include GMT[+-]#### offset, not UTC. Got: {date_to_string}"
    );
    assert!(
        !date_to_string.contains("GMT+0000"),
        "Date.toString must not print UTC on macOS profile (timezone=America/Los_Angeles). Got: {date_to_string}"
    );
    assert_eq!(
        date_has_long_name, "true",
        "Date.toString must include long timezone name in parens. Got: {date_to_string}"
    );
    assert!(
        date_to_time_string.contains("GMT"),
        "Date.toTimeString must include GMT[+-]####. Got: {date_to_time_string}"
    );
    assert!(
        date_to_date_string.contains("2026"),
        "Date.toDateString must include the year. Got: {date_to_date_string}"
    );
    assert!(
        date_to_string_native.contains("[native code]"),
        "Date.prototype.toString must serialize as native code"
    );

    let mql_color_scheme = pull(&mut page, "matchMedia('(prefers-color-scheme: light)').matches");
    let mql_pointer_fine = pull(&mut page, "matchMedia('(pointer: fine)').matches");
    let mql_hover = pull(&mut page, "matchMedia('(hover: hover)').matches");
    let mql_forced_colors_none = pull(&mut page, "matchMedia('(forced-colors: none)').matches");
    let mql_inverted_colors_none = pull(&mut page, "matchMedia('(inverted-colors: none)').matches");
    let mql_prefers_contrast = pull(
        &mut page,
        "matchMedia('(prefers-contrast: no-preference)').matches",
    );
    let mql_orientation = pull(&mut page, "matchMedia('(orientation: landscape)').matches");
    let mql_min_width = pull(&mut page, "matchMedia('(min-width: 100px)').matches");
    let mql_proto = pull(
        &mut page,
        "Object.prototype.toString.call(matchMedia('(min-width: 0)'))",
    );
    let mql_is_event_target = pull(
        &mut page,
        "String(matchMedia('(min-width: 0)') instanceof EventTarget)",
    );
    let mql_is_class = pull(
        &mut page,
        "String(matchMedia('(min-width: 0)') instanceof MediaQueryList)",
    );
    println!("\n--- Phase 6 D1: matchMedia full features ---");
    println!("  prefers-color-scheme: light:  {mql_color_scheme}");
    println!("  pointer: fine:                {mql_pointer_fine}");
    println!("  hover: hover:                 {mql_hover}");
    println!("  forced-colors: none:          {mql_forced_colors_none}");
    println!("  inverted-colors: none:        {mql_inverted_colors_none}");
    println!("  prefers-contrast: no-pref:    {mql_prefers_contrast}");
    println!("  orientation: landscape:       {mql_orientation}");
    println!("  min-width: 100px:             {mql_min_width}");
    println!("  toStringTag:                  {mql_proto}");
    println!("  instanceof EventTarget:       {mql_is_event_target}");
    println!("  instanceof MediaQueryList:    {mql_is_class}");
    assert_eq!(mql_color_scheme, "true", "macOS profile defaults light theme");
    assert_eq!(mql_pointer_fine, "true", "desktop profile default pointer:fine");
    assert_eq!(mql_hover, "true", "desktop profile default hover:hover");
    assert_eq!(mql_forced_colors_none, "true");
    assert_eq!(mql_inverted_colors_none, "true");
    assert_eq!(mql_prefers_contrast, "true");
    assert_eq!(mql_orientation, "true", "1440x789 viewport is landscape");
    assert_eq!(mql_min_width, "true", "1440 viewport >= 100");
    assert_eq!(mql_proto, "[object MediaQueryList]");
    assert_eq!(mql_is_event_target, "true");
    assert_eq!(mql_is_class, "true");

    // --- Phase 6 D2: enumerateDevices full blanking + scroll accessors --
    // Stash the device list on a global via Promise.then, then drain a few
    // event-loop ticks so the resolution settles before we read.
    page.evaluate(
        "globalThis.__edev = null; navigator.mediaDevices.enumerateDevices().then(d => { globalThis.__edev = d.map(x => ({k: x.kind, idLen: x.deviceId.length, gidLen: x.groupId.length, labelLen: x.label.length})); });"
    ).unwrap();
    for _ in 0..5 {
        let _ = page.evaluate("0").unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let edev_pre = pull(&mut page, "JSON.stringify(globalThis.__edev)");
    let edev_count = pull(&mut page, "String(globalThis.__edev ? globalThis.__edev.length : 0)");
    println!("\n--- Phase 6 D2: enumerateDevices pre-permission blanking ---");
    println!("  count: {edev_count}");
    println!("  per-device shape: {edev_pre}");
    let edev_count_n: i64 = edev_count.parse().unwrap();
    assert!(
        edev_count_n >= 1,
        "real Chrome leaks the count of devices; we should match"
    );
    // All deviceId/groupId/label fields must be empty pre-permission.
    assert!(
        !edev_pre.contains("\"idLen\":") || edev_pre.matches("\"idLen\":0").count() == edev_count_n as usize,
        "deviceId must be blank (length 0) for every device pre-permission, got {edev_pre}"
    );
    assert!(
        !edev_pre.contains("\"gidLen\":") || edev_pre.matches("\"gidLen\":0").count() == edev_count_n as usize,
        "groupId must be blank pre-permission, got {edev_pre}"
    );
    assert!(
        !edev_pre.contains("\"labelLen\":") || edev_pre.matches("\"labelLen\":0").count() == edev_count_n as usize,
        "label must be blank pre-permission, got {edev_pre}"
    );

    // scroll/page/screen position accessors must live on Window.prototype,
    // NOT as own data properties on globalThis.
    // Phase 7 — scrollX/Y/pageX/YOffset/screenX/Y are OWN accessors
    // on the window instance (NOT on Window.prototype). The earlier
    // Phase 6 D2 placed them on the prototype based on a misreading
    // of the spec; Phase 7 verified against Playwright MCP that real
    // Chrome puts them on the instance, with descriptor
    // `{get,set,enumerable,configurable}`.
    let scroll_own_get = pull(
        &mut page,
        "String(typeof Object.getOwnPropertyDescriptor(window, 'scrollX').get)",
    );
    let page_x_offset_own_get = pull(
        &mut page,
        "String(typeof Object.getOwnPropertyDescriptor(window, 'pageXOffset').get)",
    );
    let screen_x_own_get = pull(
        &mut page,
        "String(typeof Object.getOwnPropertyDescriptor(window, 'screenX').get)",
    );
    let scroll_proto_descr = pull(
        &mut page,
        "String(Object.getOwnPropertyDescriptor(Object.getPrototypeOf(window), 'scrollX'))",
    );
    let scroll_initial = pull(&mut page, "String(window.scrollY)");
    let scroll_after = pull(
        &mut page,
        "(() => { window.scrollTo(0, 100); return String(window.scrollY); })()",
    );
    let page_y_after = pull(&mut page, "String(window.pageYOffset)");
    let scroll_by_after = pull(
        &mut page,
        "(() => { window.scrollBy(0, 50); return String(window.scrollY); })()",
    );
    println!("\n--- Phase 7 D3: scrollX/Y/pageX/YOffset/screenX/Y as own-instance accessors ---");
    println!("  scrollX own descriptor.get:      {scroll_own_get}");
    println!("  pageXOffset own descriptor.get:  {page_x_offset_own_get}");
    println!("  screenX own descriptor.get:      {screen_x_own_get}");
    println!("  Window.prototype.scrollX descr:  {scroll_proto_descr}");
    println!("  scrollY initial:                 {scroll_initial}");
    println!("  scrollY after scrollTo(0,100):   {scroll_after}");
    println!("  pageYOffset after scrollTo:      {page_y_after}");
    println!("  scrollY after scrollBy(0,50):    {scroll_by_after}");
    assert_eq!(
        scroll_own_get, "function",
        "scrollX must be an own accessor on window with a getter"
    );
    assert_eq!(
        page_x_offset_own_get, "function",
        "pageXOffset must be an own accessor on window"
    );
    assert_eq!(
        screen_x_own_get, "function",
        "screenX must be an own accessor on window"
    );
    assert_eq!(
        scroll_proto_descr, "undefined",
        "scrollX must NOT be on Window.prototype (Phase 6 D2 mistake corrected in Phase 7)"
    );
    assert_eq!(scroll_initial, "0", "initial scrollY is 0");
    assert_eq!(scroll_after, "100", "scrollTo(0, 100) updates scrollY");
    assert_eq!(page_y_after, "100", "pageYOffset mirrors scrollY");
    assert_eq!(scroll_by_after, "150", "scrollBy(0, 50) adds to scrollY");

    // --- Phase 6 D3: chrome.runtime gating + WebTransport async-reject --
    let chrome_has_runtime = pull(&mut page, "String('runtime' in chrome)");
    let chrome_has_app = pull(&mut page, "String('app' in chrome)");
    let chrome_has_csi = pull(&mut page, "String('csi' in chrome)");
    let chrome_has_load_times = pull(&mut page, "String('loadTimes' in chrome)");
    let chrome_runtime_undefined = pull(&mut page, "String(typeof chrome.runtime)");
    println!("\n--- Phase 6 D3: chrome.runtime gating ---");
    println!("  'runtime' in chrome:    {chrome_has_runtime}");
    println!("  'app' in chrome:        {chrome_has_app}");
    println!("  'csi' in chrome:        {chrome_has_csi}");
    println!("  'loadTimes' in chrome:  {chrome_has_load_times}");
    println!("  typeof chrome.runtime:  {chrome_runtime_undefined}");
    assert_eq!(
        chrome_has_runtime, "false",
        "chrome.runtime is extension-context only — must be absent on regular pages"
    );
    assert_eq!(chrome_has_app, "true");
    assert_eq!(chrome_has_csi, "true");
    assert_eq!(chrome_has_load_times, "true");
    assert_eq!(chrome_runtime_undefined, "undefined");

    let wt_typeof = pull(
        &mut page,
        "String(typeof new WebTransport('https://example.invalid/'))",
    );
    let wt_ready_is_promise = pull(
        &mut page,
        "String(new WebTransport('https://example.invalid/').ready instanceof Promise)",
    );
    let wt_closed_is_promise = pull(
        &mut page,
        "String(new WebTransport('https://example.invalid/').closed instanceof Promise)",
    );
    let wt_reliability = pull(
        &mut page,
        "new WebTransport('https://example.invalid/').reliability",
    );
    let wt_get_stats_promise = pull(
        &mut page,
        "String(new WebTransport('https://example.invalid/').getStats() instanceof Promise)",
    );
    println!("\n--- Phase 6 D3: WebTransport async-reject ---");
    println!("  typeof new WebTransport():           {wt_typeof}");
    println!("  ready instanceof Promise:            {wt_ready_is_promise}");
    println!("  closed instanceof Promise:           {wt_closed_is_promise}");
    println!("  reliability:                         {wt_reliability}");
    println!("  getStats() instanceof Promise:       {wt_get_stats_promise}");
    assert_eq!(
        wt_typeof, "object",
        "new WebTransport(badUrl) must return an object — real Chrome doesn't throw synchronously"
    );
    assert_eq!(wt_ready_is_promise, "true");
    assert_eq!(wt_closed_is_promise, "true");
    assert_eq!(wt_reliability, "pending");
    assert_eq!(wt_get_stats_promise, "true");

    // --- Phase 6 D4: 10 missing-constructor batch -------------------
    // (1) caches
    let caches_typeof = pull(&mut page, "String(typeof caches)");
    let caches_proto = pull(&mut page, "Object.prototype.toString.call(caches)");
    let caches_match_typeof = pull(&mut page, "String(typeof caches.match)");
    // (2) cookieStore
    let cs_typeof = pull(&mut page, "String(typeof cookieStore)");
    let cs_proto = pull(&mut page, "Object.prototype.toString.call(cookieStore)");
    let cs_get_typeof = pull(&mut page, "String(typeof cookieStore.get)");
    let cs_is_event_target = pull(&mut page, "String(cookieStore instanceof EventTarget)");
    // (3) performance.eventCounts
    let ec_proto = pull(&mut page, "Object.prototype.toString.call(performance.eventCounts)");
    let ec_size = pull(&mut page, "String(performance.eventCounts.size)");
    let ec_get_typeof = pull(&mut page, "String(typeof performance.eventCounts.get)");
    // (4) Notification.requestPermission
    let n_perm = pull(&mut page, "Notification.permission");
    let n_max = pull(&mut page, "String(Notification.maxActions)");
    let n_req_perm_typeof = pull(&mut page, "String(typeof Notification.requestPermission)");
    let n_req_perm_returns_promise = pull(&mut page, "String(Notification.requestPermission() instanceof Promise)");
    // (5) IdleDetector
    let id_typeof = pull(&mut page, "String(typeof IdleDetector)");
    let id_req_perm_typeof = pull(&mut page, "String(typeof IdleDetector.requestPermission)");
    // (6) EyeDropper
    let ed_typeof = pull(&mut page, "String(typeof EyeDropper)");
    let ed_open_returns_promise = pull(&mut page, "String((new EyeDropper()).open() instanceof Promise)");
    // (7) navigator.virtualKeyboard
    let vk_typeof = pull(&mut page, "String(typeof navigator.virtualKeyboard)");
    let vk_overlays = pull(&mut page, "String(navigator.virtualKeyboard.overlaysContent)");
    let vk_show_typeof = pull(&mut page, "String(typeof navigator.virtualKeyboard.show)");
    // (8) navigator.devicePosture
    let dp_typeof = pull(&mut page, "String(typeof navigator.devicePosture)");
    let dp_type = pull(&mut page, "navigator.devicePosture.type");
    // (9) navigator.windowControlsOverlay
    let wco_typeof = pull(&mut page, "String(typeof navigator.windowControlsOverlay)");
    let wco_visible = pull(&mut page, "String(navigator.windowControlsOverlay.visible)");
    let wco_rect_typeof = pull(&mut page, "String(typeof navigator.windowControlsOverlay.getTitlebarAreaRect())");
    // (10) Document.startViewTransition
    let svt_typeof = pull(&mut page, "String(typeof document.startViewTransition)");
    let svt_returns_obj = pull(&mut page, "Object.prototype.toString.call(document.startViewTransition(() => {}))");
    let svt_ready_promise = pull(&mut page, "String(document.startViewTransition(() => {}).ready instanceof Promise)");

    println!("\n--- Phase 6 D4: 10 missing-constructor batch ---");
    println!("  caches:                {caches_typeof} {caches_proto} match={caches_match_typeof}");
    println!("  cookieStore:           {cs_typeof} {cs_proto} get={cs_get_typeof} EventTarget={cs_is_event_target}");
    println!("  performance.eventCounts: {ec_proto} size={ec_size} get={ec_get_typeof}");
    println!("  Notification:          permission={n_perm} maxActions={n_max} reqPerm={n_req_perm_typeof} reqPerm()->Promise={n_req_perm_returns_promise}");
    println!("  IdleDetector:          {id_typeof} reqPerm={id_req_perm_typeof}");
    println!("  EyeDropper:            {ed_typeof} open()->Promise={ed_open_returns_promise}");
    println!("  virtualKeyboard:       {vk_typeof} overlaysContent={vk_overlays} show={vk_show_typeof}");
    println!("  devicePosture:         {dp_typeof} type={dp_type}");
    println!("  windowControlsOverlay: {wco_typeof} visible={wco_visible} rect={wco_rect_typeof}");
    println!("  startViewTransition:   {svt_typeof} returns={svt_returns_obj} ready=Promise:{svt_ready_promise}");

    assert_eq!(caches_typeof, "object");
    assert_eq!(caches_proto, "[object CacheStorage]");
    assert_eq!(caches_match_typeof, "function");
    assert_eq!(cs_typeof, "object", "cookieStore must be an instance, not a constructor");
    assert_eq!(cs_proto, "[object CookieStore]");
    assert_eq!(cs_get_typeof, "function");
    assert_eq!(cs_is_event_target, "true");
    assert_eq!(ec_proto, "[object EventCounts]");
    // Phase 7 — pre-populated with 36 known event-type keys to match
    // real Chrome 147 (which ships with eventCounts.size > 0 from page load).
    assert_eq!(ec_size, "36");
    assert_eq!(ec_get_typeof, "function");
    assert_eq!(n_perm, "default");
    assert_eq!(n_max, "2");
    assert_eq!(n_req_perm_typeof, "function");
    assert_eq!(n_req_perm_returns_promise, "true");
    assert_eq!(id_typeof, "function");
    assert_eq!(id_req_perm_typeof, "function");
    assert_eq!(ed_typeof, "function");
    assert_eq!(ed_open_returns_promise, "true");
    assert_eq!(vk_typeof, "object");
    assert_eq!(vk_overlays, "false");
    assert_eq!(vk_show_typeof, "function");
    assert_eq!(dp_typeof, "object");
    assert_eq!(dp_type, "continuous");
    assert_eq!(wco_typeof, "object");
    assert_eq!(wco_visible, "false");
    assert_eq!(wco_rect_typeof, "object");
    assert_eq!(svt_typeof, "function");
    assert_eq!(svt_returns_obj, "[object ViewTransition]");
    assert_eq!(svt_ready_promise, "true");
}
