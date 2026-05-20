//! Anti-bot detection test suite.
//!
//! Verifies that browser_oxide's JS environment passes common
//! anti-bot fingerprint checks (sannysoft, creepjs, areyouheadless).

use browser::Page;

fn html(body: &str) -> String {
    format!("<html><head></head><body>{}</body></html>", body)
}

async fn eval(js: &str) -> String {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(js).unwrap()
}

/// Same as `eval`, but treats the page as a secure context (https://
/// origin). Required for tests that probe [SecureContext]-only APIs
/// (mediaDevices, getBattery, userAgentData, etc.) — Phase 7.
async fn eval_secure(js: &str) -> String {
    let mut page = Page::from_html_with_url(
        &html(""),
        "https://example.com/",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap()
}

// === navigator checks ===

#[tokio::test]
async fn webdriver_is_boolean() {
    assert_eq!(eval("typeof navigator.webdriver").await, "boolean");
}

#[tokio::test]
async fn navigator_user_agent_is_string() {
    assert_eq!(eval("typeof navigator.userAgent").await, "string");
}

#[tokio::test]
async fn navigator_platform_exists() {
    let platform = eval("navigator.platform").await;
    assert!(!platform.is_empty(), "platform should not be empty");
}

#[tokio::test]
async fn navigator_languages_nonempty() {
    assert_eq!(eval("navigator.languages.length > 0").await, "true");
}

#[tokio::test]
async fn navigator_hardware_concurrency_positive() {
    assert_eq!(eval("navigator.hardwareConcurrency > 0").await, "true");
}

#[tokio::test]
async fn navigator_device_memory_positive() {
    // deviceMemory is [SecureContext]. Phase 7.
    assert_eq!(eval_secure("navigator.deviceMemory > 0").await, "true");
}

#[tokio::test]
async fn navigator_vendor_is_google() {
    assert_eq!(eval("navigator.vendor").await, "Google Inc.");
}

#[tokio::test]
async fn navigator_plugins_count() {
    assert_eq!(eval("navigator.plugins.length > 0").await, "true");
}

#[tokio::test]
async fn navigator_cookie_enabled() {
    assert_eq!(eval("navigator.cookieEnabled").await, "true");
}

#[tokio::test]
async fn navigator_online() {
    assert_eq!(eval("navigator.onLine").await, "true");
}

#[tokio::test]
async fn navigator_pdf_viewer_enabled() {
    assert_eq!(eval("navigator.pdfViewerEnabled").await, "true");
}

// === document checks ===

#[tokio::test]
async fn document_has_focus() {
    assert_eq!(eval("document.hasFocus()").await, "true");
}

#[tokio::test]
async fn document_visibility_state() {
    assert_eq!(eval("document.visibilityState").await, "visible");
}

#[tokio::test]
async fn document_hidden_is_false() {
    assert_eq!(eval("document.hidden").await, "false");
}

#[tokio::test]
async fn document_ready_state() {
    assert_eq!(eval("document.readyState").await, "complete");
}

// === window / screen checks ===

#[tokio::test]
async fn window_inner_width_positive() {
    assert_eq!(eval("window.innerWidth > 0").await, "true");
}

#[tokio::test]
async fn window_inner_height_positive() {
    assert_eq!(eval("window.innerHeight > 0").await, "true");
}

#[tokio::test]
async fn window_outer_gte_inner() {
    assert_eq!(eval("window.outerWidth >= window.innerWidth").await, "true");
    assert_eq!(
        eval("window.outerHeight >= window.innerHeight").await,
        "true"
    );
}

#[tokio::test]
async fn screen_width_positive() {
    assert_eq!(eval("screen.width > 0").await, "true");
}

#[tokio::test]
async fn screen_height_positive() {
    assert_eq!(eval("screen.height > 0").await, "true");
}

#[tokio::test]
async fn screen_color_depth() {
    assert_eq!(eval("screen.colorDepth > 0").await, "true");
}

#[tokio::test]
async fn device_pixel_ratio_positive() {
    assert_eq!(eval("devicePixelRatio > 0").await, "true");
}

// === API existence checks ===

#[tokio::test]
async fn notification_exists() {
    assert_eq!(eval("typeof Notification").await, "function");
}

#[tokio::test]
async fn notification_permission_default() {
    // Phase 7 — "default" on secure context, "denied" on insecure.
    assert_eq!(eval_secure("Notification.permission").await, "default");
    assert_eq!(eval("Notification.permission").await, "denied");
}

#[tokio::test]
async fn performance_now_is_number() {
    assert_eq!(eval("typeof performance.now()").await, "number");
}

#[tokio::test]
async fn is_secure_context() {
    // Default `from_html` URL is `about:blank` — insecure per WICG
    // secure-contexts §3.2 (Phase 7 fix). Real Chrome agrees.
    assert_eq!(eval("isSecureContext").await, "false");
}

#[tokio::test]
async fn crypto_exists() {
    assert_eq!(eval("typeof crypto.getRandomValues").await, "function");
}

#[tokio::test]
async fn text_encoder_exists() {
    assert_eq!(eval("typeof TextEncoder").await, "function");
}

#[tokio::test]
async fn text_decoder_exists() {
    assert_eq!(eval("typeof TextDecoder").await, "function");
}

#[tokio::test]
async fn local_storage_works() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("localStorage.setItem('test', 'value')")
        .unwrap();
    assert_eq!(
        page.evaluate("localStorage.getItem('test')").unwrap(),
        "value"
    );
}

#[tokio::test]
async fn mutation_observer_exists() {
    assert_eq!(eval("typeof MutationObserver").await, "function");
}

#[tokio::test]
async fn intersection_observer_exists() {
    assert_eq!(eval("typeof IntersectionObserver").await, "function");
}

#[tokio::test]
async fn resize_observer_exists() {
    assert_eq!(eval("typeof ResizeObserver").await, "function");
}

// === Batch 1 gap fixes ===

#[tokio::test]
async fn window_chrome_exists() {
    assert_eq!(eval("typeof window.chrome").await, "object");
}

#[tokio::test]
async fn window_chrome_app() {
    assert_eq!(eval("typeof window.chrome.app").await, "object");
}

#[tokio::test]
async fn window_chrome_runtime() {
    // Real Chrome 147 on a regular page has no chrome.runtime (extension-only).
    assert_eq!(eval("typeof window.chrome.runtime").await, "undefined");
}

#[tokio::test]
async fn window_chrome_csi() {
    assert_eq!(eval("typeof window.chrome.csi").await, "function");
}

#[tokio::test]
async fn window_chrome_load_times() {
    assert_eq!(eval("typeof window.chrome.loadTimes").await, "function");
}

#[tokio::test]
async fn performance_memory_exists() {
    assert_eq!(eval("performance.memory.jsHeapSizeLimit > 0").await, "true");
}

#[tokio::test]
async fn navigator_user_agent_data_exists() {
    // userAgentData is [SecureContext] — only present on https/etc.
    assert_eq!(
        eval_secure("typeof navigator.userAgentData").await,
        "object"
    );
}

#[tokio::test]
async fn navigator_user_agent_data_brands() {
    assert_eq!(
        eval_secure("navigator.userAgentData.brands.length > 0").await,
        "true"
    );
}

#[tokio::test]
async fn speech_synthesis_has_voices() {
    assert_eq!(eval("speechSynthesis.getVoices().length > 0").await, "true");
}

// === Canvas/WebGL/Audio (Batch 4 gaps) ===

#[tokio::test]
async fn canvas_context_2d_exists() {
    assert_eq!(
        eval("typeof document.createElement('canvas').getContext('2d')").await,
        "object"
    );
}

#[tokio::test]
async fn canvas_to_data_url_works() {
    let result = eval(
        r#"
        (() => {
            const c = document.createElement('canvas');
            const ctx = c.getContext('2d');
            ctx.fillStyle = 'red';
            ctx.fillRect(0, 0, 10, 10);
            return c.toDataURL().startsWith('data:image/png;base64,');
        })()
    "#,
    )
    .await;
    assert_eq!(result, "true");
}

#[tokio::test]
async fn webgl_context_exists() {
    assert_eq!(
        eval("typeof document.createElement('canvas').getContext('webgl')").await,
        "object"
    );
}

#[tokio::test]
async fn webgl_renderer_info() {
    let result = eval(
        r#"
        (() => {
            const gl = document.createElement('canvas').getContext('webgl');
            const ext = gl.getExtension('WEBGL_debug_renderer_info');
            return typeof gl.getParameter(ext.UNMASKED_RENDERER_WEBGL);
        })()
    "#,
    )
    .await;
    assert_eq!(result, "string");
}

#[tokio::test]
async fn webgl_max_texture_size() {
    let result = eval(
        r#"
        document.createElement('canvas').getContext('webgl').getParameter(0x0D33) > 0
    "#,
    )
    .await;
    assert_eq!(result, "true");
}

#[tokio::test]
async fn audio_context_exists() {
    assert_eq!(eval("typeof AudioContext").await, "function");
}

#[tokio::test]
async fn offline_audio_context_exists() {
    assert_eq!(eval("typeof OfflineAudioContext").await, "function");
}

#[tokio::test]
async fn canvas_measure_text() {
    let result = eval(
        r#"
        (() => {
            const ctx = document.createElement('canvas').getContext('2d');
            return ctx.measureText('Hello').width > 0;
        })()
    "#,
    )
    .await;
    assert_eq!(result, "true");
}

// === Batch 5: Layout, getComputedStyle, XHR, WebSocket ===

#[tokio::test]
async fn get_bounding_client_rect_returns_object() {
    let result = eval(
        r#"
        typeof document.body.getBoundingClientRect().width
    "#,
    )
    .await;
    assert_eq!(result, "number");
}

#[tokio::test]
async fn offset_width_positive() {
    // Block elements should have non-zero offsetWidth
    assert_eq!(eval("document.body.offsetWidth > 0").await, "true");
}

#[tokio::test]
async fn offset_height_positive() {
    assert_eq!(eval("document.body.offsetHeight > 0").await, "true");
}

#[tokio::test]
async fn get_computed_style_exists() {
    assert_eq!(eval("typeof getComputedStyle").await, "function");
}

#[tokio::test]
async fn get_computed_style_display() {
    assert_eq!(
        eval("getComputedStyle(document.body).display").await,
        "block"
    );
}

#[tokio::test]
async fn get_computed_style_visibility() {
    assert_eq!(
        eval("getComputedStyle(document.body).visibility").await,
        "visible"
    );
}

#[tokio::test]
async fn get_computed_style_get_property_value() {
    assert_eq!(
        eval("getComputedStyle(document.body).getPropertyValue('opacity')").await,
        "1"
    );
}

#[tokio::test]
async fn check_visibility_exists() {
    assert_eq!(
        eval("typeof document.body.checkVisibility").await,
        "function"
    );
}

#[tokio::test]
async fn xmlhttprequest_exists() {
    assert_eq!(eval("typeof XMLHttpRequest").await, "function");
}

#[tokio::test]
async fn websocket_exists() {
    assert_eq!(eval("typeof WebSocket").await, "function");
}

#[tokio::test]
async fn websocket_has_constants() {
    assert_eq!(eval("WebSocket.CONNECTING").await, "0");
    assert_eq!(eval("WebSocket.OPEN").await, "1");
    assert_eq!(eval("WebSocket.CLOSED").await, "3");
}

#[tokio::test]
async fn fetch_exists_as_function() {
    assert_eq!(eval("typeof fetch").await, "function");
}

#[tokio::test]
async fn headers_class_exists() {
    assert_eq!(eval("typeof Headers").await, "function");
}

#[tokio::test]
async fn request_class_exists() {
    assert_eq!(eval("typeof Request").await, "function");
}

#[tokio::test]
async fn response_class_exists() {
    assert_eq!(eval("typeof Response").await, "function");
}

// === Batch 6: Missing APIs from real-world testing ===

#[tokio::test]
async fn navigator_java_enabled_exists() {
    assert_eq!(eval("typeof navigator.javaEnabled").await, "function");
}

#[tokio::test]
async fn navigator_java_enabled_returns_false() {
    assert_eq!(eval("navigator.javaEnabled()").await, "false");
}

#[tokio::test]
async fn navigator_send_beacon_exists() {
    assert_eq!(eval("typeof navigator.sendBeacon").await, "function");
}

#[tokio::test]
async fn navigator_send_beacon_returns_true() {
    assert_eq!(
        eval("navigator.sendBeacon('https://example.com', 'data')").await,
        "true"
    );
}

#[tokio::test]
async fn navigator_get_battery_exists() {
    // getBattery is [SecureContext] — only present on https/etc.
    assert_eq!(eval_secure("typeof navigator.getBattery").await, "function");
}

#[tokio::test]
async fn navigator_get_battery_returns_promise() {
    assert_eq!(
        eval_secure("navigator.getBattery() instanceof Promise").await,
        "true"
    );
}

#[tokio::test]
async fn navigator_get_battery_resolves() {
    // getBattery is [SecureContext] — page must use https://
    //
    // BatteryManager values are deliberately randomized per-session by
    // window_bootstrap.js (commit cab06c4, W2.6) to defeat the CreepJS
    // "level:1, charging:true, chargingTime:0, dischargingTime:Infinity"
    // headless fingerprint. We assert on the *shape* the spec mandates,
    // not the now-defeated default values.
    let mut page = Page::from_html_with_url(
        &html(""),
        "https://example.com/",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate("navigator.getBattery().then(b => { globalThis._bat = b; })")
        .unwrap();
    use std::time::Duration;
    page.evaluate_async("void 0", Duration::from_millis(100))
        .await
        .ok();
    // charging is a boolean.
    let charging = page.evaluate("typeof globalThis._bat.charging").unwrap();
    assert_eq!(charging, "boolean", "battery.charging should be boolean");
    // level is a number in [0, 1].
    let level_ok = page
        .evaluate(
            "(() => { const l = globalThis._bat.level; \
                return typeof l === 'number' && l >= 0 && l <= 1; })()",
        )
        .unwrap();
    assert_eq!(
        level_ok, "true",
        "battery.level should be a number in [0,1]"
    );
    // chargingTime is number-or-Infinity (>= 0).
    let ct_ok = page
        .evaluate(
            "(() => { const t = globalThis._bat.chargingTime; \
                return typeof t === 'number' && t >= 0; })()",
        )
        .unwrap();
    assert_eq!(
        ct_ok, "true",
        "battery.chargingTime should be a non-negative number"
    );
    // dischargingTime is number-or-Infinity (>= 0).
    let dt_ok = page
        .evaluate(
            "(() => { const t = globalThis._bat.dischargingTime; \
                return typeof t === 'number' && t >= 0; })()",
        )
        .unwrap();
    assert_eq!(
        dt_ok, "true",
        "battery.dischargingTime should be a non-negative number"
    );
}

#[tokio::test]
async fn window_scroll_to_exists() {
    assert_eq!(eval("typeof window.scrollTo").await, "function");
}

#[tokio::test]
async fn window_scroll_exists() {
    assert_eq!(eval("typeof window.scroll").await, "function");
}

#[tokio::test]
async fn window_scroll_by_exists() {
    assert_eq!(eval("typeof window.scrollBy").await, "function");
}

#[tokio::test]
async fn window_scroll_to_updates_position() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("window.scrollTo(100, 200)").unwrap();
    assert_eq!(page.evaluate("window.scrollX").unwrap(), "100");
    assert_eq!(page.evaluate("window.scrollY").unwrap(), "200");
}

#[tokio::test]
async fn window_scroll_by_accumulates() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("window.scrollTo(10, 20)").unwrap();
    page.evaluate("window.scrollBy(5, 10)").unwrap();
    assert_eq!(page.evaluate("window.scrollX").unwrap(), "15");
    assert_eq!(page.evaluate("window.scrollY").unwrap(), "30");
}

#[tokio::test]
async fn element_style_exists() {
    assert_eq!(eval("typeof document.body.style").await, "object");
}

#[tokio::test]
async fn element_style_set_and_read() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("document.body.style.backgroundColor = 'red'")
        .unwrap();
    assert_eq!(
        page.evaluate("document.body.style.backgroundColor")
            .unwrap(),
        "red"
    );
}

#[tokio::test]
async fn element_style_set_property() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("document.body.style.setProperty('color', 'blue')")
        .unwrap();
    assert_eq!(
        page.evaluate("document.body.style.getPropertyValue('color')")
            .unwrap(),
        "blue"
    );
}

#[tokio::test]
async fn element_style_reflects_in_attribute() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("document.body.style.opacity = '0.5'")
        .unwrap();
    let attr = page
        .evaluate("document.body.getAttribute('style')")
        .unwrap();
    assert!(
        attr.contains("opacity"),
        "style attribute should contain opacity: {}",
        attr
    );
}

#[tokio::test]
async fn document_write_exists() {
    assert_eq!(eval("typeof document.write").await, "function");
}

#[tokio::test]
async fn document_writeln_exists() {
    assert_eq!(eval("typeof document.writeln").await, "function");
}

#[tokio::test]
async fn document_write_appends_content() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("document.write('<div id=\"injected\">hello</div>')")
        .unwrap();
    let result = page
        .evaluate("document.getElementById('injected').textContent")
        .unwrap();
    assert_eq!(result, "hello");
}

// === stealth profile validation ===

#[test]
fn stealth_profiles_validate() {
    use stealth::presets::*;
    for profile in [chrome_130_windows(), chrome_130_macos(), chrome_130_linux()] {
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
    }
}
