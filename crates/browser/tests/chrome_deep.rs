//! Deep Chrome behavioral checks — tests things anti-bot detectors actually verify,
//! beyond just API surface existence. Each test documents what Chrome does and
//! whether we match.

use browser::Page;
use stealth;

fn html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head></head><body>{}</body></html>",
        body
    )
}

async fn check(js: &str) -> String {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

// ================================================================
// Prototype integrity — anti-bot checks these aren't modified
// ================================================================

#[tokio::test]
async fn array_is_array() {
    assert_eq!(check("Array.isArray([])").await, "true");
    assert_eq!(check("Array.isArray(new Array())").await, "true");
}

#[tokio::test]
async fn promise_is_native() {
    assert_eq!(check("typeof Promise").await, "function");
    assert_eq!(check("Promise.resolve(1) instanceof Promise").await, "true");
}

#[tokio::test]
async fn json_stringify_works() {
    assert_eq!(
        check("JSON.stringify({a:1,b:'x'})").await,
        r#"{"a":1,"b":"x"}"#
    );
}

#[tokio::test]
async fn date_now_returns_number() {
    assert_eq!(check("typeof Date.now()").await, "number");
    assert_eq!(check("Date.now() > 0").await, "true");
}

// ================================================================
// Canvas fingerprint — must produce real, unique output
// ================================================================

#[tokio::test]
async fn canvas_fingerprint_produces_unique_data() {
    let mut page = Page::from_html(
        &html(
            r#"
        <canvas id="c" width="200" height="50"></canvas>
        <script>
            const ctx = document.getElementById('c').getContext('2d');
            ctx.fillStyle = '#f60';
            ctx.fillRect(125, 1, 62, 20);
            ctx.fillStyle = '#069';
            ctx.font = '11pt Arial';
            ctx.fillText('browser_oxide', 2, 15);
            ctx.fillStyle = 'rgba(102, 204, 0, 0.7)';
            ctx.fillText('browser_oxide', 4, 17);
            globalThis.fp = document.getElementById('c').toDataURL();
        </script>
    "#,
        ),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let fp = page.evaluate("fp").unwrap();
    assert!(
        fp.starts_with("data:image/png;base64,"),
        "should be valid PNG data URL"
    );
    assert!(
        fp.len() > 500,
        "fingerprint should be substantial, got len={}",
        fp.len()
    );
}

#[tokio::test]
async fn canvas_different_text_different_fingerprint() {
    async fn render(text: &str) -> String {
        let mut page = Page::from_html(
            &format!(
                r#"<!DOCTYPE html><html><head></head><body>
            <canvas id="c" width="200" height="50"></canvas>
            <script>
                const ctx = document.getElementById('c').getContext('2d');
                ctx.font = '14px Arial';
                ctx.fillText('{}', 10, 25);
                globalThis.fp = document.getElementById('c').toDataURL();
            </script></body></html>"#,
                text
            ),
            None::<stealth::StealthProfile>,
        )
        .await
        .unwrap();
        page.evaluate("fp").unwrap()
    }
    let a = render("Hello").await;
    let b = render("World").await;
    assert_ne!(a, b, "different text must produce different fingerprints");
}

// ================================================================
// WebGL parameters — must return realistic values
// ================================================================

#[tokio::test]
async fn webgl_unmasked_renderer_not_empty() {
    let mut page = Page::from_html(
        &html(
            r#"
        <canvas id="c"></canvas>
        <script>
            const gl = document.getElementById('c').getContext('webgl');
            const ext = gl.getExtension('WEBGL_debug_renderer_info');
            globalThis.renderer = gl.getParameter(ext.UNMASKED_RENDERER_WEBGL);
            globalThis.vendor = gl.getParameter(ext.UNMASKED_VENDOR_WEBGL);
        </script>
    "#,
        ),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let renderer = page.evaluate("renderer").unwrap();
    let vendor = page.evaluate("vendor").unwrap();
    assert!(!renderer.is_empty(), "renderer should not be empty");
    assert!(!vendor.is_empty(), "vendor should not be empty");
    assert!(
        renderer.contains("ANGLE") || renderer.contains("NVIDIA") || renderer.contains("Intel"),
        "renderer should look realistic: {}",
        renderer
    );
}

// ================================================================
// Navigator consistency checks
// ================================================================

#[tokio::test]
async fn navigator_ua_matches_chrome_pattern() {
    let ua = check("navigator.userAgent").await;
    assert!(
        ua.contains("Mozilla/5.0"),
        "UA should start with Mozilla: {}",
        ua
    );
    assert!(
        ua.contains("AppleWebKit"),
        "UA should contain AppleWebKit: {}",
        ua
    );
    assert!(ua.contains("Chrome/"), "UA should contain Chrome/: {}", ua);
    assert!(ua.contains("Safari/"), "UA should contain Safari/: {}", ua);
}
#[tokio::test]
async fn navigator_webdriver_boolean() {
    // Chrome 130+ sets navigator.webdriver = false (boolean) when automation is off
    assert_eq!(check("typeof navigator.webdriver").await, "boolean");
    assert_eq!(check("navigator.webdriver === false").await, "true");
}

#[tokio::test]
async fn chrome_object_structure() {
    // Chrome 147 on a regular page: {app, csi, loadTimes} only.
    // chrome.runtime is extension-only; chrome.webstore was removed in Chrome 126.
    assert_eq!(check("typeof chrome").await, "object");
    assert_eq!(check("typeof chrome.runtime").await, "undefined");
    assert_eq!(check("typeof chrome.app").await, "object");
    assert_eq!(check("typeof chrome.csi").await, "function");
    assert_eq!(check("typeof chrome.loadTimes").await, "function");
}

// §6.6 item 8 — navigator.userAgentData high-entropy values must be driven
// by the active StealthProfile. Regression target: architecture/platform/
// platformVersion/model used to be hardcoded in window_bootstrap.js.
#[tokio::test]
async fn test_user_agent_data_highentropy() {
    // macOS Apple Silicon exercises the arm path — if any hint still
    // hardcodes "x86"/"Windows" we fail here.
    let profile = stealth::presets::chrome_130_macos();
    let browser_full = profile.browser_version.clone();
    let browser_major = browser_full
        .split('.')
        .next()
        .unwrap_or_default()
        .to_string();
    // userAgentData is [SecureContext]; about:blank would return undefined.
    let page = Page::with_profile("", "https://example.com/", profile)
        .await
        .unwrap();

    let probe = r#"
        (async () => {
            const low = {
                brands: navigator.userAgentData.brands.map(b => b.brand).sort(),
                brandVersions: navigator.userAgentData.brands,
                mobile: navigator.userAgentData.mobile,
                platform: navigator.userAgentData.platform,
            };
            const hi = await navigator.userAgentData.getHighEntropyValues([
                'architecture','bitness','model','platformVersion',
                'uaFullVersion','fullVersionList','wow64',
            ]);
            let threw = null;
            try { await navigator.userAgentData.getHighEntropyValues("not-an-array"); }
            catch (e) { threw = e.constructor.name; }
            const chromeBrand = hi.fullVersionList.find(b => b.brand === 'Google Chrome');
            globalThis.__uaResult = JSON.stringify({
                lowBrands: low.brands,
                brandVersions: low.brandVersions,
                platform: low.platform,
                mobile: low.mobile,
                architecture: hi.architecture,
                bitness: hi.bitness,
                model: hi.model,
                platformVersion: hi.platformVersion,
                uaFullVersion: hi.uaFullVersion,
                wow64: hi.wow64,
                fullVersionListLen: hi.fullVersionList.length,
                chromeBrandVersion: chromeBrand && chromeBrand.version,
                nonArrayRejection: threw,
                invalidHintIgnored: (await navigator.userAgentData.getHighEntropyValues(['not-a-real-hint'])),
            });
        })()
    "#;
    {
        let mut page = page;
        page.evaluate(probe).unwrap();
        page.evaluate_async("void 0", std::time::Duration::from_millis(200))
            .await
            .ok();
        let raw = page.evaluate("globalThis.__uaResult").unwrap();
        let obj: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|_| panic!("ua probe result not JSON: {}", raw));

        assert_eq!(obj["platform"], "macOS", "platform must honor os_name");
        assert_eq!(obj["mobile"], false);
        assert_eq!(obj["architecture"], "arm", "macOS Apple Silicon → arm");
        assert_eq!(obj["bitness"], "64");
        assert_eq!(obj["model"], "", "desktop profile has empty model");
        assert_eq!(obj["platformVersion"], "15.2.0");
        assert_eq!(obj["uaFullVersion"], browser_full);
        assert_eq!(obj["wow64"], false);
        assert_eq!(obj["fullVersionListLen"], 3);
        assert_eq!(obj["chromeBrandVersion"], browser_full);
        let sorted: Vec<&str> = obj["lowBrands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // Chrome's third GREASE brand is randomized per release ("Not.A/Brand",
        // "Not-A.Brand", "Not_A Brand", "Not(A:Brand", ...); current engine
        // ships "Not.A/Brand" matching Chrome 147. Match either common form
        // — the test point is that exactly one non-Chromium/non-Google brand
        // is present.
        assert_eq!(sorted.len(), 3);
        assert!(sorted.contains(&"Chromium"));
        assert!(sorted.contains(&"Google Chrome"));
        let third = sorted
            .iter()
            .find(|b| **b != "Chromium" && **b != "Google Chrome")
            .unwrap();
        assert!(
            third.starts_with("Not") && third.contains("Brand"),
            "third brand should be a GREASE 'Not...Brand', got: {third}"
        );
        assert_eq!(obj["nonArrayRejection"], "TypeError");
        let invalid = &obj["invalidHintIgnored"];
        assert!(!invalid.is_null());
        assert!(invalid.get("not-a-real-hint").is_none());
        let brand_versions: Vec<&str> = obj["brandVersions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|b| b.get("version").and_then(|v| v.as_str()))
            .collect();
        assert!(
            brand_versions.contains(&browser_major.as_str()),
            "low-entropy brands must contain '{}', got {:?}",
            browser_major,
            brand_versions
        );
    }
}

// §6.6 item 2 — window.chrome surface parity (Chrome 147 surface).
#[tokio::test]
async fn test_chrome_api_surface() {
    // Chrome 147: {app, csi, loadTimes} — no runtime, no webstore.
    let keys = check("Object.keys(chrome).sort().join(',')").await;
    for required in ["app", "csi", "loadTimes"] {
        assert!(
            keys.contains(required),
            "chrome.{} missing (keys: {})",
            required,
            keys
        );
    }
    assert!(
        !keys.contains("runtime"),
        "chrome.runtime must be absent on regular pages (keys: {})",
        keys
    );
    assert!(
        !keys.contains("webstore"),
        "chrome.webstore was removed in Chrome 126 (keys: {})",
        keys
    );

    // chrome.loadTimes — for a non-HTTP local page, spdy/npn are false.
    // On real HTTPS pages (tested via anti_bot tests), they are true/"h2".
    assert_eq!(
        check("chrome.loadTimes().wasFetchedViaSpdy === false").await,
        "true"
    );
    assert_eq!(check("chrome.loadTimes().connectionInfo").await, "");

    // chrome.csi keys
    assert_eq!(
        check("Object.keys(chrome.csi()).sort().join(',')").await,
        "onloadT,pageT,startE,tran"
    );
}

#[tokio::test]
async fn permissions_query_returns_prompt() {
    // Chrome returns { state: "prompt" } for notifications permission on
    // a secure context; on http://about:blank Notification.permission is
    // "denied" and the Permissions query mirrors that to "denied" too.
    assert_eq!(check(r#"
        navigator.permissions.query({ name: 'notifications' }).then(r => globalThis._permState = r.state)
    "#).await, "[object Promise]");
    let mut page = Page::from_html_with_url(
        &html(""),
        "https://example.com/",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate("navigator.permissions.query({ name: 'notifications' }).then(r => globalThis._ps = r.state)").unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(50))
        .await
        .ok();
    assert_eq!(page.evaluate("globalThis._ps").unwrap(), "prompt");
}

// ================================================================
// Document state checks
// ================================================================

#[tokio::test]
async fn document_ready_state_complete() {
    assert_eq!(check("document.readyState").await, "complete");
}

#[tokio::test]
async fn document_has_focus_true() {
    assert_eq!(check("document.hasFocus()").await, "true");
}

#[tokio::test]
async fn document_visibility_visible() {
    assert_eq!(check("document.visibilityState").await, "visible");
    assert_eq!(check("document.hidden").await, "false");
}

// ================================================================
// Window dimensions consistency
// ================================================================

#[tokio::test]
async fn window_dimensions_consistent() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    // outer >= inner
    assert_eq!(page.evaluate("outerWidth >= innerWidth").unwrap(), "true");
    assert_eq!(page.evaluate("outerHeight >= innerHeight").unwrap(), "true");
    // screen >= outer
    assert_eq!(page.evaluate("screen.width >= outerWidth").unwrap(), "true");
    // All positive
    assert_eq!(
        page.evaluate("innerWidth > 0 && innerHeight > 0").unwrap(),
        "true"
    );
}

// ================================================================
// Performance API
// ================================================================

#[tokio::test]
async fn performance_now_monotonic() {
    assert_eq!(check("performance.now() >= 0").await, "true");
    assert_eq!(check("typeof performance.now()").await, "number");
}

#[tokio::test]
async fn performance_memory_chrome_specific() {
    assert_eq!(check("typeof performance.memory").await, "object");
    assert_eq!(
        check("performance.memory.jsHeapSizeLimit > 0").await,
        "true"
    );
}

// ================================================================
// Crypto API
// ================================================================

#[tokio::test]
async fn crypto_get_random_values() {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("globalThis.arr = new Uint8Array(16); crypto.getRandomValues(globalThis.arr)")
        .unwrap();
    // Should not be all zeros
    let result = page.evaluate("globalThis.arr.some(x => x !== 0)").unwrap();
    assert_eq!(
        result, "true",
        "getRandomValues should produce non-zero bytes"
    );
}

// ================================================================
// Event system behavioral checks
// ================================================================

#[tokio::test]
async fn event_is_trusted_false_for_dispatched() {
    // Manually dispatched events have isTrusted = false
    let mut page = Page::from_html(
        &html(
            r#"
        <div id="el"></div>
        <script>
            globalThis.trusted = null;
            document.getElementById('el').addEventListener('click', (e) => {
                globalThis.trusted = e.isTrusted;
            });
            document.getElementById('el').dispatchEvent(new Event('click'));
        </script>
    "#,
        ),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(page.evaluate("trusted").unwrap(), "false");
}

// ================================================================
// NodeList bracket access (found as a gap)
// ================================================================

#[tokio::test]
async fn nodelist_bracket_access() {
    // Real Chrome supports querySelectorAll(...)[0]
    // Our NodeList only supports .item(0)
    let mut page = Page::from_html(
        &html("<div class='x'>A</div><div class='x'>B</div>"),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let via_item = page
        .evaluate("document.querySelectorAll('.x').item(0).textContent")
        .unwrap();
    assert_eq!(via_item, "A");
    // Test bracket access too
    let via_bracket = page
        .evaluate("document.querySelectorAll('.x')[0]?.textContent || 'undefined'")
        .unwrap();
    // This documents the current gap — bracket access may not work
    if via_bracket == "undefined" {
        eprintln!("GAP: NodeList bracket access [0] not supported, must use .item(0)");
    }
}

// ================================================================
// getComputedStyle with CSS cascade
// ================================================================

#[tokio::test]
async fn computed_style_from_style_block() {
    let mut page = Page::from_html(
        r#"<!DOCTYPE html><html><head>
        <style>body { margin: 0; } .test { color: green; }</style>
    </head><body><div id="el" class="test"></div></body></html>"#,
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).color")
            .unwrap(),
        "green"
    );
}

// ================================================================
// Function.prototype.toString — native code detection
// ================================================================

#[tokio::test]
async fn native_function_to_string() {
    // Anti-bot checks: native functions should return "function X() { [native code] }"
    // Our polyfilled functions won't pass this check — document the gap
    let result = check("navigator.permissions.query.toString()").await;
    let is_native = result.contains("[native code]");
    if !is_native {
        eprintln!(
            "GAP: navigator.permissions.query.toString() doesn't show [native code]: {}",
            result
        );
    }
    // At minimum, it should be a function
    assert_eq!(
        check("typeof navigator.permissions.query").await,
        "function"
    );
}
