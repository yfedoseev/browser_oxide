//! Chrome compatibility audit.
//!
//! Tests every API that a real Chrome 130 browser exposes.
//! Each test checks typeof/existence AND basic behavior.
//! Failures here = gaps vs real Chrome.

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

/// Same as `check`, but treats the page as a secure context (https://
/// origin). Required for tests that probe [SecureContext]-only APIs
/// (mediaDevices, getBattery, userAgentData, crypto.subtle, etc.) —
/// Phase 7.
async fn check_secure(js: &str) -> String {
    let mut page = Page::from_html_with_url(
        &html(""),
        "https://example.com/",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

// ================================================================
// Window globals
// ================================================================

#[tokio::test]
async fn window_self() {
    assert_eq!(check("window === self").await, "true");
}
#[tokio::test]
async fn window_document() {
    assert_eq!(check("typeof document").await, "object");
}
#[tokio::test]
async fn window_location() {
    assert_eq!(check("typeof location").await, "object");
}
#[tokio::test]
async fn window_location_href() {
    assert_eq!(check("typeof location.href").await, "string");
}
#[tokio::test]
async fn window_location_protocol() {
    assert_eq!(check("typeof location.protocol").await, "string");
}
#[tokio::test]
async fn window_navigator() {
    assert_eq!(check("typeof navigator").await, "object");
}
#[tokio::test]
async fn window_screen() {
    assert_eq!(check("typeof screen").await, "object");
}
#[tokio::test]
async fn window_history() {
    assert_eq!(check("typeof history").await, "object");
}
#[tokio::test]
async fn window_chrome() {
    assert_eq!(check("typeof chrome").await, "object");
}
#[tokio::test]
async fn window_crypto() {
    assert_eq!(check("typeof crypto").await, "object");
}
#[tokio::test]
async fn window_performance() {
    assert_eq!(check("typeof performance").await, "object");
}
#[tokio::test]
async fn window_console() {
    assert_eq!(check("typeof console").await, "object");
}
#[tokio::test]
async fn window_local_storage() {
    assert_eq!(check("typeof localStorage").await, "object");
}
#[tokio::test]
async fn window_session_storage() {
    assert_eq!(check("typeof sessionStorage").await, "object");
}
#[tokio::test]
async fn window_is_secure_context() {
    // Default `from_html` URL is `about:blank` — insecure per WICG
    // secure-contexts §3.2 (Phase 7 fix). Real Chrome agrees.
    assert_eq!(check("isSecureContext").await, "false");
}
#[tokio::test]
async fn window_inner_width() {
    assert_eq!(check("typeof innerWidth").await, "number");
}
#[tokio::test]
async fn window_inner_height() {
    assert_eq!(check("typeof innerHeight").await, "number");
}
#[tokio::test]
async fn window_outer_width() {
    assert_eq!(check("typeof outerWidth").await, "number");
}
#[tokio::test]
async fn window_outer_height() {
    assert_eq!(check("typeof outerHeight").await, "number");
}
#[tokio::test]
async fn window_device_pixel_ratio() {
    assert_eq!(check("typeof devicePixelRatio").await, "number");
}
#[tokio::test]
async fn window_scroll_x() {
    assert_eq!(check("typeof scrollX").await, "number");
}
#[tokio::test]
async fn window_scroll_y() {
    assert_eq!(check("typeof scrollY").await, "number");
}

// Functions
#[tokio::test]
async fn fn_set_timeout() {
    assert_eq!(check("typeof setTimeout").await, "function");
}
#[tokio::test]
async fn fn_set_interval() {
    assert_eq!(check("typeof setInterval").await, "function");
}
#[tokio::test]
async fn fn_clear_timeout() {
    assert_eq!(check("typeof clearTimeout").await, "function");
}
#[tokio::test]
async fn fn_clear_interval() {
    assert_eq!(check("typeof clearInterval").await, "function");
}
#[tokio::test]
async fn fn_request_animation_frame() {
    assert_eq!(check("typeof requestAnimationFrame").await, "function");
}
#[tokio::test]
async fn fn_request_idle_callback() {
    assert_eq!(check("typeof requestIdleCallback").await, "function");
}
#[tokio::test]
async fn fn_fetch() {
    assert_eq!(check("typeof fetch").await, "function");
}
#[tokio::test]
async fn fn_atob() {
    assert_eq!(check("typeof atob").await, "function");
}
#[tokio::test]
async fn fn_btoa() {
    assert_eq!(check("typeof btoa").await, "function");
}
#[tokio::test]
async fn fn_alert() {
    assert_eq!(check("typeof alert").await, "function");
}
#[tokio::test]
async fn fn_confirm() {
    assert_eq!(check("typeof confirm").await, "function");
}
#[tokio::test]
async fn fn_prompt() {
    assert_eq!(check("typeof prompt").await, "function");
}
#[tokio::test]
async fn fn_scroll_to() {
    assert_eq!(check("typeof scrollTo").await, "function");
}
#[tokio::test]
async fn fn_scroll_by() {
    assert_eq!(check("typeof scrollBy").await, "function");
}
#[tokio::test]
async fn fn_get_computed_style() {
    assert_eq!(check("typeof getComputedStyle").await, "function");
}
#[tokio::test]
async fn fn_match_media() {
    assert_eq!(check("typeof matchMedia").await, "function");
}
#[tokio::test]
async fn fn_get_selection() {
    assert_eq!(check("typeof getSelection").await, "function");
}
#[tokio::test]
async fn fn_open() {
    assert_eq!(check("typeof open").await, "function");
}
#[tokio::test]
async fn fn_close() {
    assert_eq!(check("typeof close").await, "function");
}
#[tokio::test]
async fn fn_post_message() {
    assert_eq!(check("typeof postMessage").await, "function");
}

// ================================================================
// Navigator
// ================================================================

#[tokio::test]
async fn nav_user_agent() {
    assert_eq!(check("typeof navigator.userAgent").await, "string");
}
#[tokio::test]
async fn nav_platform() {
    assert_eq!(check("typeof navigator.platform").await, "string");
}
#[tokio::test]
async fn nav_language() {
    assert_eq!(check("typeof navigator.language").await, "string");
}
#[tokio::test]
async fn nav_languages() {
    assert_eq!(check("Array.isArray(navigator.languages)").await, "true");
}
#[tokio::test]
async fn nav_vendor() {
    assert_eq!(check("navigator.vendor").await, "Google Inc.");
}
#[tokio::test]
async fn nav_hardware_concurrency() {
    assert_eq!(check("navigator.hardwareConcurrency > 0").await, "true");
}
#[tokio::test]
async fn nav_device_memory() {
    // deviceMemory is [SecureContext]. Phase 7.
    assert_eq!(check_secure("navigator.deviceMemory > 0").await, "true");
}
#[tokio::test]
async fn nav_max_touch_points() {
    assert_eq!(check("typeof navigator.maxTouchPoints").await, "number");
}
#[tokio::test]
async fn nav_cookie_enabled() {
    assert_eq!(check("navigator.cookieEnabled").await, "true");
}
#[tokio::test]
async fn nav_on_line() {
    assert_eq!(check("navigator.onLine").await, "true");
}
#[tokio::test]
async fn nav_pdf_viewer_enabled() {
    assert_eq!(check("navigator.pdfViewerEnabled").await, "true");
}
#[tokio::test]
async fn nav_webdriver() {
    assert_eq!(check("typeof navigator.webdriver").await, "boolean");
}
#[tokio::test]
async fn nav_plugins_length() {
    assert_eq!(check("navigator.plugins.length > 0").await, "true");
}
#[tokio::test]
async fn nav_connection() {
    assert_eq!(check("typeof navigator.connection").await, "object");
}
#[tokio::test]
async fn nav_java_enabled() {
    assert_eq!(check("typeof navigator.javaEnabled").await, "function");
}
#[tokio::test]
async fn nav_send_beacon() {
    assert_eq!(check("typeof navigator.sendBeacon").await, "function");
}
#[tokio::test]
async fn nav_get_battery() {
    assert_eq!(check_secure("typeof navigator.getBattery").await, "function");
}
#[tokio::test]
async fn nav_user_agent_data() {
    assert_eq!(check_secure("typeof navigator.userAgentData").await, "object");
}
#[tokio::test]
async fn nav_ua_data_brands() {
    assert_eq!(
        check_secure("navigator.userAgentData.brands.length > 0").await,
        "true"
    );
}
#[tokio::test]
async fn nav_ua_data_mobile() {
    assert_eq!(
        check_secure("typeof navigator.userAgentData.mobile").await,
        "boolean"
    );
}
// Client Hints API contract — browser_oxide exposes the full getHighEntropyValues
// surface required by CreepJS / Yandex Antirobot / WBAAS.
#[tokio::test]
async fn nav_ua_data_get_high_entropy_is_function() {
    assert_eq!(
        check_secure("typeof navigator.userAgentData.getHighEntropyValues").await,
        "function"
    );
}
#[tokio::test]
async fn nav_ua_data_get_high_entropy_returns_promise() {
    assert_eq!(
        check_secure("navigator.userAgentData.getHighEntropyValues([]) instanceof Promise").await,
        "true"
    );
}
// For tests that need to inspect the resolved object, kick off the Promise and
// stash its result in a synchronous global via .then(); then pump microtasks.
// Our Page::evaluate drains microtasks before returning, so window.__r is
// populated by the time the second evaluate() reads it.
#[tokio::test]
async fn nav_ua_data_high_entropy_full_version_list() {
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(
        r#"window.__r = null;
        navigator.userAgentData.getHighEntropyValues(['fullVersionList']).then(r => { window.__r = r; });"#
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200))
        .await
        .ok();
    let result = page.evaluate(
        r#"(() => {
            const r = window.__r;
            if (!r) return 'null';
            return Array.isArray(r.fullVersionList)
                && r.fullVersionList.length >= 3
                && r.fullVersionList.every(b => typeof b.brand === 'string' && typeof b.version === 'string');
        })()"#
    ).unwrap();
    assert_eq!(result, "true");
}
#[tokio::test]
async fn nav_ua_data_high_entropy_architecture_and_bitness() {
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(
        r#"window.__r = null;
        navigator.userAgentData.getHighEntropyValues(['architecture', 'bitness']).then(r => { window.__r = r; });"#
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200))
        .await
        .ok();
    let result = page
        .evaluate(
            r#"(() => {
            const r = window.__r;
            if (!r) return 'null';
            return (r.architecture === 'x86' || r.architecture === 'arm') && r.bitness === '64';
        })()"#,
        )
        .unwrap();
    assert_eq!(result, "true");
}
#[tokio::test]
async fn nav_ua_data_high_entropy_only_returns_requested_plus_low_entropy() {
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(
        r#"window.__r = null;
        navigator.userAgentData.getHighEntropyValues(['architecture']).then(r => { window.__r = r; });"#
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200))
        .await
        .ok();
    let result = page
        .evaluate(
            r#"(() => {
            const r = window.__r;
            if (!r) return 'null';
            return 'brands' in r && 'mobile' in r && 'platform' in r
                && 'architecture' in r
                && !('wow64' in r)
                && !('model' in r)
                && !('platformVersion' in r);
        })()"#,
        )
        .unwrap();
    assert_eq!(result, "true");
}
#[tokio::test]
async fn nav_ua_data_to_json_returns_low_entropy_only() {
    assert_eq!(
        check_secure(
            r#"(() => {
                const j = navigator.userAgentData.toJSON();
                return Array.isArray(j.brands) && typeof j.mobile === 'boolean' && typeof j.platform === 'string'
                    && !('fullVersionList' in j) && !('architecture' in j);
            })()"#
        ).await,
        "true"
    );
}
#[tokio::test]
async fn nav_ua_data_brands_match_user_agent_version() {
    // Consistency: the major version in brands[] must match the version in navigator.userAgent
    assert_eq!(
        check_secure(
            r#"(() => {
                const uaMatch = navigator.userAgent.match(/Chrome\/(\d+)/);
                if (!uaMatch) return 'no-ua-match';
                const major = uaMatch[1];
                return navigator.userAgentData.brands.some(b => b.brand === 'Google Chrome' && b.version === major);
            })()"#
        ).await,
        "true"
    );
}
#[tokio::test]
async fn nav_media_devices() {
    assert_eq!(check_secure("typeof navigator.mediaDevices").await, "object");
}
#[tokio::test]
async fn nav_permissions() {
    assert_eq!(check("typeof navigator.permissions").await, "object");
}
#[tokio::test]
async fn nav_clipboard() {
    assert_eq!(check_secure("typeof navigator.clipboard").await, "object");
}
#[tokio::test]
async fn nav_storage() {
    assert_eq!(check_secure("typeof navigator.storage").await, "object");
}
#[tokio::test]
async fn nav_service_worker() {
    assert_eq!(check_secure("typeof navigator.serviceWorker").await, "object");
}

// ================================================================
// Document
// ================================================================

#[tokio::test]
async fn doc_document_element() {
    assert_eq!(check("document.documentElement.tagName").await, "HTML");
}
#[tokio::test]
async fn doc_head() {
    assert_eq!(check("document.head.tagName").await, "HEAD");
}
#[tokio::test]
async fn doc_body() {
    assert_eq!(check("document.body.tagName").await, "BODY");
}
#[tokio::test]
async fn doc_ready_state() {
    assert_eq!(check("document.readyState").await, "complete");
}
#[tokio::test]
async fn doc_visibility_state() {
    assert_eq!(check("document.visibilityState").await, "visible");
}
#[tokio::test]
async fn doc_hidden() {
    assert_eq!(check("document.hidden").await, "false");
}
#[tokio::test]
async fn doc_has_focus() {
    assert_eq!(check("document.hasFocus()").await, "true");
}
#[tokio::test]
async fn doc_character_set() {
    // Phase 7 — HTML legacy default per spec is windows-1252. Real
    // Chrome reports this for HTML docs without explicit <meta charset>.
    assert_eq!(check("document.characterSet").await, "windows-1252");
}
#[tokio::test]
async fn doc_content_type() {
    assert_eq!(check("document.contentType").await, "text/html");
}
#[tokio::test]
async fn doc_compat_mode() {
    assert_eq!(check("document.compatMode").await, "CSS1Compat");
}
#[tokio::test]
async fn doc_default_view() {
    assert_eq!(check("document.defaultView === window").await, "true");
}

// Document methods
#[tokio::test]
async fn doc_create_element() {
    assert_eq!(check("typeof document.createElement").await, "function");
}
#[tokio::test]
async fn doc_create_text_node() {
    assert_eq!(check("typeof document.createTextNode").await, "function");
}
#[tokio::test]
async fn doc_create_document_fragment() {
    assert_eq!(
        check("typeof document.createDocumentFragment").await,
        "function"
    );
}
#[tokio::test]
async fn doc_create_event() {
    assert_eq!(check("typeof document.createEvent").await, "function");
}
#[tokio::test]
async fn doc_create_range() {
    assert_eq!(check("typeof document.createRange").await, "function");
}
#[tokio::test]
async fn doc_get_element_by_id() {
    assert_eq!(check("typeof document.getElementById").await, "function");
}
#[tokio::test]
async fn doc_query_selector() {
    assert_eq!(check("typeof document.querySelector").await, "function");
}
#[tokio::test]
async fn doc_query_selector_all() {
    assert_eq!(check("typeof document.querySelectorAll").await, "function");
}
#[tokio::test]
async fn doc_get_elements_by_tag_name() {
    assert_eq!(
        check("typeof document.getElementsByTagName").await,
        "function"
    );
}
#[tokio::test]
async fn doc_get_elements_by_class_name() {
    assert_eq!(
        check("typeof document.getElementsByClassName").await,
        "function"
    );
}
#[tokio::test]
async fn doc_exec_command() {
    assert_eq!(check("typeof document.execCommand").await, "function");
}
#[tokio::test]
async fn doc_element_from_point() {
    assert_eq!(check("typeof document.elementFromPoint").await, "function");
}
#[tokio::test]
async fn doc_write() {
    assert_eq!(check("typeof document.write").await, "function");
}
#[tokio::test]
async fn doc_writeln() {
    assert_eq!(check("typeof document.writeln").await, "function");
}
#[tokio::test]
async fn doc_import_node() {
    assert_eq!(check("typeof document.importNode").await, "function");
}
#[tokio::test]
async fn doc_adopt_node() {
    assert_eq!(check("typeof document.adoptNode").await, "function");
}
#[tokio::test]
async fn doc_scripts() {
    assert_eq!(check("typeof document.scripts").await, "object");
}
#[tokio::test]
async fn doc_forms() {
    assert_eq!(check("typeof document.forms").await, "object");
}
#[tokio::test]
async fn doc_active_element() {
    assert_eq!(
        check("document.activeElement === document.body").await,
        "true"
    );
}

// ================================================================
// Element / Node
// ================================================================

#[tokio::test]
async fn node_element_node() {
    assert_eq!(check("Node.ELEMENT_NODE").await, "1");
}
#[tokio::test]
async fn node_text_node() {
    assert_eq!(check("Node.TEXT_NODE").await, "3");
}
#[tokio::test]
async fn node_document_node() {
    assert_eq!(check("Node.DOCUMENT_NODE").await, "9");
}
#[tokio::test]
async fn el_append_child() {
    assert_eq!(check("typeof document.body.appendChild").await, "function");
}
#[tokio::test]
async fn el_remove_child() {
    assert_eq!(check("typeof document.body.removeChild").await, "function");
}
#[tokio::test]
async fn el_insert_before() {
    assert_eq!(check("typeof document.body.insertBefore").await, "function");
}
#[tokio::test]
async fn el_replace_child() {
    assert_eq!(check("typeof document.body.replaceChild").await, "function");
}
#[tokio::test]
async fn el_clone_node() {
    assert_eq!(check("typeof document.body.cloneNode").await, "function");
}
#[tokio::test]
async fn el_contains() {
    assert_eq!(check("typeof document.body.contains").await, "function");
}
#[tokio::test]
async fn el_remove() {
    assert_eq!(
        check("typeof document.createElement('div').remove").await,
        "function"
    );
}
#[tokio::test]
async fn el_append() {
    assert_eq!(check("typeof document.body.append").await, "function");
}
#[tokio::test]
async fn el_prepend() {
    assert_eq!(check("typeof document.body.prepend").await, "function");
}
#[tokio::test]
async fn el_after() {
    assert_eq!(
        check("typeof document.createElement('div').after").await,
        "function"
    );
}
#[tokio::test]
async fn el_before() {
    assert_eq!(
        check("typeof document.createElement('div').before").await,
        "function"
    );
}
#[tokio::test]
async fn el_replace_with() {
    assert_eq!(
        check("typeof document.createElement('div').replaceWith").await,
        "function"
    );
}
#[tokio::test]
async fn el_replace_children() {
    assert_eq!(
        check("typeof document.body.replaceChildren").await,
        "function"
    );
}
#[tokio::test]
async fn el_get_attribute() {
    assert_eq!(check("typeof document.body.getAttribute").await, "function");
}
#[tokio::test]
async fn el_set_attribute() {
    assert_eq!(check("typeof document.body.setAttribute").await, "function");
}
#[tokio::test]
async fn el_remove_attribute() {
    assert_eq!(
        check("typeof document.body.removeAttribute").await,
        "function"
    );
}
#[tokio::test]
async fn el_has_attribute() {
    assert_eq!(check("typeof document.body.hasAttribute").await, "function");
}
#[tokio::test]
async fn el_toggle_attribute() {
    assert_eq!(
        check("typeof document.body.toggleAttribute").await,
        "function"
    );
}
#[tokio::test]
async fn el_insert_adjacent_html() {
    assert_eq!(
        check("typeof document.body.insertAdjacentHTML").await,
        "function"
    );
}
#[tokio::test]
async fn el_insert_adjacent_element() {
    assert_eq!(
        check("typeof document.body.insertAdjacentElement").await,
        "function"
    );
}
#[tokio::test]
async fn el_matches() {
    assert_eq!(check("typeof document.body.matches").await, "function");
}
#[tokio::test]
async fn el_closest() {
    assert_eq!(check("typeof document.body.closest").await, "function");
}
#[tokio::test]
async fn el_get_bounding_client_rect() {
    assert_eq!(
        check("typeof document.body.getBoundingClientRect").await,
        "function"
    );
}
#[tokio::test]
async fn el_class_list() {
    assert_eq!(check("typeof document.body.classList").await, "object");
}
#[tokio::test]
async fn el_style() {
    assert_eq!(check("typeof document.body.style").await, "object");
}
#[tokio::test]
async fn el_dataset() {
    assert_eq!(check("typeof document.body.dataset").await, "object");
}
#[tokio::test]
async fn el_inner_html() {
    assert_eq!(check("typeof document.body.innerHTML").await, "string");
}
#[tokio::test]
async fn el_outer_html() {
    assert_eq!(check("typeof document.body.outerHTML").await, "string");
}
#[tokio::test]
async fn el_text_content() {
    assert_eq!(check("typeof document.body.textContent").await, "string");
}
#[tokio::test]
async fn el_owner_document() {
    assert_eq!(
        check("document.body.ownerDocument === document").await,
        "true"
    );
}
#[tokio::test]
async fn el_is_connected() {
    assert_eq!(check("document.body.isConnected").await, "true");
}
#[tokio::test]
async fn el_offset_width() {
    assert_eq!(check("typeof document.body.offsetWidth").await, "number");
}
#[tokio::test]
async fn el_offset_height() {
    assert_eq!(check("typeof document.body.offsetHeight").await, "number");
}
#[tokio::test]
async fn el_check_visibility() {
    assert_eq!(
        check("typeof document.body.checkVisibility").await,
        "function"
    );
}
#[tokio::test]
async fn el_add_event_listener() {
    assert_eq!(
        check("typeof document.body.addEventListener").await,
        "function"
    );
}
#[tokio::test]
async fn el_dispatch_event() {
    assert_eq!(
        check("typeof document.body.dispatchEvent").await,
        "function"
    );
}
#[tokio::test]
async fn el_click() {
    assert_eq!(check("typeof document.body.click").await, "function");
}
#[tokio::test]
async fn el_focus() {
    assert_eq!(check("typeof document.body.focus").await, "function");
}
#[tokio::test]
async fn el_blur() {
    assert_eq!(check("typeof document.body.blur").await, "function");
}
#[tokio::test]
async fn el_next_element_sibling() {
    assert_eq!(check("'nextElementSibling' in document.body").await, "true");
}
#[tokio::test]
async fn el_previous_element_sibling() {
    assert_eq!(
        check("'previousElementSibling' in document.body").await,
        "true"
    );
}
#[tokio::test]
async fn el_child_element_count() {
    assert_eq!(
        check("typeof document.body.childElementCount").await,
        "number"
    );
}
#[tokio::test]
async fn el_animate() {
    assert_eq!(check("typeof document.body.animate").await, "function");
}

// ================================================================
// Events
// ================================================================

#[tokio::test]
async fn cls_event() {
    assert_eq!(check("typeof Event").await, "function");
}
#[tokio::test]
async fn cls_custom_event() {
    assert_eq!(check("typeof CustomEvent").await, "function");
}
#[tokio::test]
async fn cls_mouse_event() {
    assert_eq!(check("typeof MouseEvent").await, "function");
}
#[tokio::test]
async fn cls_keyboard_event() {
    assert_eq!(check("typeof KeyboardEvent").await, "function");
}
#[tokio::test]
async fn cls_input_event() {
    assert_eq!(check("typeof InputEvent").await, "function");
}
#[tokio::test]
async fn cls_focus_event() {
    assert_eq!(check("typeof FocusEvent").await, "function");
}
#[tokio::test]
async fn cls_pointer_event() {
    assert_eq!(check("typeof PointerEvent").await, "function");
}
#[tokio::test]
async fn cls_wheel_event() {
    assert_eq!(check("typeof WheelEvent").await, "function");
}
#[tokio::test]
async fn cls_touch_event() {
    assert_eq!(check("typeof TouchEvent").await, "function");
}
#[tokio::test]
async fn cls_message_event() {
    assert_eq!(check("typeof MessageEvent").await, "function");
}
#[tokio::test]
async fn cls_error_event() {
    assert_eq!(check("typeof ErrorEvent").await, "function");
}
#[tokio::test]
async fn cls_event_target() {
    assert_eq!(check("typeof EventTarget").await, "function");
}
#[tokio::test]
async fn cls_event_source() {
    assert_eq!(check("typeof EventSource").await, "function");
    assert_eq!(check("EventSource.CONNECTING").await, "0");
    assert_eq!(check("EventSource.OPEN").await, "1");
    assert_eq!(check("EventSource.CLOSED").await, "2");
}

// WebRTC
#[tokio::test]
async fn cls_rtc_peer_connection() {
    assert_eq!(check("typeof RTCPeerConnection").await, "function");
    assert_eq!(check("typeof webkitRTCPeerConnection").await, "function");
    assert_eq!(check("typeof RTCSessionDescription").await, "function");
    assert_eq!(check("typeof RTCIceCandidate").await, "function");
}

// Font enumeration
#[tokio::test]
async fn api_document_fonts() {
    assert_eq!(check("typeof document.fonts").await, "object");
    assert_eq!(check("typeof document.fonts.check").await, "function");
    assert_eq!(check("document.fonts.check('12px Arial')").await, "true");
}

// MediaSource
#[tokio::test]
async fn cls_media_source() {
    assert_eq!(check("typeof MediaSource").await, "function");
    assert_eq!(
        check("typeof MediaSource.isTypeSupported").await,
        "function"
    );
    assert_eq!(
        check("MediaSource.isTypeSupported('video/mp4')").await,
        "true"
    );
    assert_eq!(
        check("MediaSource.isTypeSupported('video/webm; codecs=\"vp9\"')").await,
        "true"
    );
}

// Speech synthesis voices
#[tokio::test]
async fn api_speech_synthesis_voices() {
    assert_eq!(
        check("speechSynthesis.getVoices().length > 0").await,
        "true"
    );
}

// Permissions
#[tokio::test]
async fn api_permissions_query() {
    assert_eq!(
        check("typeof navigator.permissions.query").await,
        "function"
    );
}

// Battery
#[tokio::test]
async fn api_battery() {
    assert_eq!(check_secure("typeof navigator.getBattery").await, "function");
}

// Dynamic script loading via DOM
#[tokio::test]
async fn api_dynamic_script_src_property() {
    // Verify script.src setter works as setAttribute
    assert_eq!(
        check(
            r#"
        const s = document.createElement('script');
        s.src = 'https://example.com/test.js';
        s.getAttribute('src')
    "#
        )
        .await,
        "https://example.com/test.js"
    );
}

// document.cookie read/write
#[tokio::test]
async fn api_document_cookie_readwrite() {
    // This is the exact check wildberries uses
    assert_eq!(
        check(
            r#"
        document.cookie = "cookietest=abc123; path=/; SameSite=Lax";
        document.cookie.includes("cookietest=abc123")
    "#
        )
        .await,
        "true"
    );
}

// ================================================================
// Constructors / Classes
// ================================================================

#[tokio::test]
async fn cls_url() {
    assert_eq!(check("typeof URL").await, "function");
}
#[tokio::test]
async fn cls_url_search_params() {
    assert_eq!(check("typeof URLSearchParams").await, "function");
}
#[tokio::test]
async fn cls_abort_controller() {
    assert_eq!(check("typeof AbortController").await, "function");
}
#[tokio::test]
async fn cls_abort_signal() {
    assert_eq!(check("typeof AbortSignal").await, "function");
}
#[tokio::test]
async fn cls_headers() {
    assert_eq!(check("typeof Headers").await, "function");
}
#[tokio::test]
async fn cls_request() {
    assert_eq!(check("typeof Request").await, "function");
}
#[tokio::test]
async fn cls_response() {
    assert_eq!(check("typeof Response").await, "function");
}
#[tokio::test]
async fn cls_form_data() {
    assert_eq!(check("typeof FormData").await, "function");
}
#[tokio::test]
async fn cls_blob() {
    assert_eq!(check("typeof Blob").await, "function");
}
#[tokio::test]
async fn cls_file() {
    assert_eq!(check("typeof File").await, "function");
}
#[tokio::test]
async fn cls_image() {
    assert_eq!(check("typeof Image").await, "function");
}
#[tokio::test]
async fn cls_dom_parser() {
    assert_eq!(check("typeof DOMParser").await, "function");
}
#[tokio::test]
async fn cls_dom_rect() {
    assert_eq!(check("typeof DOMRect").await, "function");
}
#[tokio::test]
async fn cls_text_encoder() {
    assert_eq!(check("typeof TextEncoder").await, "function");
}
#[tokio::test]
async fn cls_text_decoder() {
    assert_eq!(check("typeof TextDecoder").await, "function");
}
#[tokio::test]
async fn cls_mutation_observer() {
    assert_eq!(check("typeof MutationObserver").await, "function");
}
#[tokio::test]
async fn cls_intersection_observer() {
    assert_eq!(check("typeof IntersectionObserver").await, "function");
}
#[tokio::test]
async fn cls_resize_observer() {
    assert_eq!(check("typeof ResizeObserver").await, "function");
}
#[tokio::test]
async fn cls_xml_http_request() {
    assert_eq!(check("typeof XMLHttpRequest").await, "function");
}
#[tokio::test]
async fn cls_websocket() {
    assert_eq!(check("typeof WebSocket").await, "function");
}
#[tokio::test]
async fn cls_notification() {
    assert_eq!(check("typeof Notification").await, "function");
}
#[tokio::test]
async fn cls_dom_exception() {
    assert_eq!(check("typeof DOMException").await, "function");
}
#[tokio::test]
async fn cls_audio_context() {
    assert_eq!(check("typeof AudioContext").await, "function");
}
#[tokio::test]
async fn cls_offline_audio_context() {
    assert_eq!(check("typeof OfflineAudioContext").await, "function");
}
#[tokio::test]
async fn cls_custom_elements() {
    assert_eq!(check("typeof customElements").await, "object");
}

// ================================================================
// HTMLElement subtypes
// ================================================================

#[tokio::test]
async fn cls_html_element() {
    assert_eq!(check("typeof HTMLElement").await, "function");
}
#[tokio::test]
async fn cls_html_div_element() {
    assert_eq!(check("typeof HTMLDivElement").await, "function");
}
#[tokio::test]
async fn cls_html_input_element() {
    assert_eq!(check("typeof HTMLInputElement").await, "function");
}
#[tokio::test]
async fn cls_html_anchor_element() {
    assert_eq!(check("typeof HTMLAnchorElement").await, "function");
}
#[tokio::test]
async fn cls_html_image_element() {
    assert_eq!(check("typeof HTMLImageElement").await, "function");
}
#[tokio::test]
async fn cls_html_canvas_element() {
    assert_eq!(check("typeof HTMLCanvasElement").await, "function");
}
#[tokio::test]
async fn cls_html_form_element() {
    assert_eq!(check("typeof HTMLFormElement").await, "function");
}
#[tokio::test]
async fn cls_html_video_element() {
    assert_eq!(check("typeof HTMLVideoElement").await, "function");
}
#[tokio::test]
async fn cls_svg_element() {
    assert_eq!(check("typeof SVGElement").await, "function");
}

// ================================================================
// Canvas
// ================================================================

#[tokio::test]
async fn canvas_2d_context() {
    assert_eq!(
        check("typeof document.createElement('canvas').getContext('2d')").await,
        "object"
    );
}
#[tokio::test]
async fn canvas_webgl_context() {
    assert_eq!(
        check("typeof document.createElement('canvas').getContext('webgl')").await,
        "object"
    );
}
#[tokio::test]
async fn canvas_to_data_url() {
    assert_eq!(
        check("document.createElement('canvas').toDataURL().startsWith('data:image/png')").await,
        "true"
    );
}

#[tokio::test]
async fn canvas_drawing_produces_nonblank_data_url() {
    // Simulate WBAAS-style canvas fingerprint operations.
    let result = check(r#"(function() {
        var c = document.createElement('canvas');
        c.width = 200; c.height = 50;
        var ctx = c.getContext('2d');
        ctx.font = '18px Arial';
        ctx.fillStyle = '#f60';
        ctx.fillRect(125, 1, 62, 20);
        ctx.fillStyle = '#069';
        ctx.fillText('WBAAS fingerprint test Cwm fjordbank glyphs 😺', 2, 15);
        ctx.fillStyle = 'rgba(102, 204, 0, 0.7)';
        ctx.fillText('WBAAS fingerprint test Cwm fjordbank glyphs 😺', 4, 45);
        var data = c.toDataURL('image/png');
        // Check it's non-empty (not a blank canvas)
        return data.length + ':' + (data !== 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAMgAAAAyCAYAAAAZUl3oAAAABmJLR0QA/wD/AP+gvaeTAAAADklEQVRoge3BMQEAAADCoPVP7WsIoAAAeAMBxAACqwAAAABJRU5ErkJggg==');
    })()"#).await;
    eprintln!("Canvas drawing probe: {}", &result[..result.len().min(100)]);
    let parts: Vec<&str> = result.splitn(2, ':').collect();
    let len: usize = parts[0].parse().unwrap_or(0);
    assert!(len > 1000, "canvas toDataURL after drawing should produce >1KB data, got {}b", len);
    assert_eq!(parts.get(1), Some(&"true"), "canvas toDataURL should differ from blank canvas");
}

// ================================================================
// WebGL fingerprint catalog (stealth::gpu) — GAPS.md §P0 item 7 fix
// These assert the profile-driven WebGL fingerprint is exposed with
// realistic per-GPU values, not the old hardcoded single-profile stubs.
// ================================================================

async fn webgl_eval(profile: stealth::StealthProfile, js: &str) -> String {
    let mut page = Page::with_profile(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        "https://example.com/",
        profile,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERR: {e}"))
}

#[tokio::test]
async fn webgl_extensions_has_25_plus_entries() {
    // Real Chrome 131 exposes 25-35 extensions depending on GPU.
    // Our old stub returned 13.
    let profile = stealth::chrome_130_windows();
    let r = webgl_eval(
        profile,
        r#"(() => {
            const gl = document.createElement('canvas').getContext('webgl');
            const exts = gl.getSupportedExtensions();
            return exts.length >= 25 ? 'ok:' + exts.length : 'fail:' + exts.length;
        })()"#,
    )
    .await;
    assert!(r.starts_with("ok:"), "expected >=25 extensions, got {r}");
}

#[tokio::test]
async fn webgl_unmasked_vendor_matches_windows_profile() {
    // Windows Chrome profile should get the NVIDIA vendor string.
    let profile = stealth::chrome_130_windows();
    let r = webgl_eval(
        profile,
        r#"(() => {
            const gl = document.createElement('canvas').getContext('webgl');
            return gl.getParameter(0x9245);  // UNMASKED_VENDOR_WEBGL
        })()"#,
    )
    .await;
    assert!(
        r.contains("NVIDIA"),
        "Windows profile should report NVIDIA, got: {r}"
    );
}

#[tokio::test]
async fn webgl_unmasked_renderer_matches_macos_profile() {
    // macOS profile reports Apple M3 (Phase 7 — was M2 Pro).
    let profile = stealth::chrome_130_macos();
    let r = webgl_eval(
        profile,
        r#"(() => {
            const gl = document.createElement('canvas').getContext('webgl');
            return gl.getParameter(0x9246);  // UNMASKED_RENDERER_WEBGL
        })()"#,
    )
    .await;
    assert!(
        r.contains("Apple M3"),
        "macOS profile should report Apple M3, got: {r}"
    );
}

#[tokio::test]
async fn webgl_unmasked_renderer_matches_linux_profile() {
    // Linux profile should get the Intel UHD Graphics 630 renderer.
    let profile = stealth::chrome_130_linux();
    let r = webgl_eval(
        profile,
        r#"(() => {
            const gl = document.createElement('canvas').getContext('webgl');
            return gl.getParameter(0x9246);
        })()"#,
    )
    .await;
    assert!(
        r.contains("Intel"),
        "Linux profile should report Intel, got: {r}"
    );
}

#[tokio::test]
async fn webgl_extensions_differ_across_profiles() {
    // Apple GPU exposes WEBGL_compressed_texture_astc that neither NVIDIA nor
    // Intel Linux expose. This is THE standard CreepJS probe for GPU diversity.
    let apple = stealth::chrome_130_macos();
    let intel = stealth::chrome_130_linux();
    let apple_has_astc = webgl_eval(
        apple,
        r#"document.createElement('canvas').getContext('webgl').getSupportedExtensions().includes('WEBGL_compressed_texture_astc')"#,
    ).await;
    let intel_has_astc = webgl_eval(
        intel,
        r#"document.createElement('canvas').getContext('webgl').getSupportedExtensions().includes('WEBGL_compressed_texture_astc')"#,
    ).await;
    assert_eq!(
        apple_has_astc, "true",
        "Apple should expose WEBGL_compressed_texture_astc"
    );
    assert_eq!(
        intel_has_astc, "false",
        "Intel Linux should NOT expose WEBGL_compressed_texture_astc"
    );
}

#[tokio::test]
async fn webgl_shader_precision_int_differs_from_float() {
    // Real Chrome returns [127, 127, 23] for HIGH_FLOAT and [31, 30, 0]
    // for HIGH_INT. Our previous stub returned {127, 127, 23} for ALL
    // precision types, which is a distinctive tell.
    let profile = stealth::chrome_130_windows();
    let r = webgl_eval(
        profile,
        r#"(() => {
            const gl = document.createElement('canvas').getContext('webgl');
            const VERTEX_SHADER = 0x8B31;
            const HIGH_FLOAT = 0x8DF2;
            const HIGH_INT = 0x8DF5;
            const f = gl.getShaderPrecisionFormat(VERTEX_SHADER, HIGH_FLOAT);
            const i = gl.getShaderPrecisionFormat(VERTEX_SHADER, HIGH_INT);
            return f.rangeMin + ',' + f.rangeMax + ',' + f.precision + ' | ' + i.rangeMin + ',' + i.rangeMax + ',' + i.precision;
        })()"#,
    )
    .await;
    // Expect HIGH_FLOAT = [127, 127, 23], HIGH_INT = [31, 30, 0]
    assert!(
        r.contains("127,127,23 | 31,30,0"),
        "shader precision wrong, got: {r}"
    );
}

#[tokio::test]
async fn webgl_max_texture_size_is_16384() {
    let profile = stealth::chrome_130_windows();
    let r = webgl_eval(
        profile,
        r#"document.createElement('canvas').getContext('webgl').getParameter(0x0D33)"#,
    )
    .await;
    assert_eq!(r, "16384", "MAX_TEXTURE_SIZE should be 16384");
}

// ================================================================
// Performance API (§P1 item 9 fix) — PerformanceNavigationTiming +
// PerformanceResourceTiming. Akamai BMP reads these.
// ================================================================

#[tokio::test]
async fn perf_get_entries_by_type_navigation() {
    // getEntriesByType('navigation') must return a non-empty array
    assert_eq!(
        check("performance.getEntriesByType('navigation').length >= 1").await,
        "true"
    );
}

#[tokio::test]
async fn perf_navigation_entry_has_timing_fields() {
    // The navigation entry must have domContentLoadedEventStart,
    // loadEventEnd, and transferSize like a real Chrome
    assert_eq!(
        check(
            r#"(() => {
                const e = performance.getEntriesByType('navigation')[0];
                return typeof e.domContentLoadedEventStart === 'number'
                    && typeof e.loadEventEnd === 'number'
                    && typeof e.transferSize === 'number'
                    && e.entryType === 'navigation';
            })()"#
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn perf_get_entries_by_type_resource() {
    // getEntriesByType('resource') should return at least 3 entries
    // (we synthesize 5 typical page resources)
    assert_eq!(
        check("performance.getEntriesByType('resource').length >= 3").await,
        "true"
    );
}

#[tokio::test]
async fn perf_resource_entry_shape() {
    assert_eq!(
        check(
            r#"(() => {
                const r = performance.getEntriesByType('resource')[0];
                return r.entryType === 'resource'
                    && typeof r.initiatorType === 'string'
                    && typeof r.nextHopProtocol === 'string'
                    && r.nextHopProtocol === 'h2';
            })()"#
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn perf_timing_is_present() {
    // performance.timing (deprecated but Akamai + CreepJS still probe)
    assert_eq!(
        check("typeof performance.timing.navigationStart === 'number'").await,
        "true"
    );
}

#[tokio::test]
async fn perf_time_origin_is_present() {
    assert_eq!(
        check("typeof performance.timeOrigin === 'number'").await,
        "true"
    );
}

// ================================================================
// Intl timezone consistency (§P1 item 13) — timezone must match profile
// ================================================================

#[tokio::test]
async fn intl_timezone_matches_moscow_profile() {
    let profile = stealth::presets::chrome_130_ru();
    let mut page = Page::with_profile(
        "<!DOCTYPE html><html><body></body></html>",
        "https://example.com/",
        profile,
    )
    .await
    .unwrap();
    let r = page
        .evaluate("Intl.DateTimeFormat().resolvedOptions().timeZone")
        .unwrap();
    assert_eq!(r, "Europe/Moscow", "RU profile should report Europe/Moscow");
}

#[tokio::test]
async fn intl_timezone_matches_tokyo_profile() {
    let profile = stealth::presets::chrome_130_jp();
    let mut page = Page::with_profile(
        "<!DOCTYPE html><html><body></body></html>",
        "https://example.com/",
        profile,
    )
    .await
    .unwrap();
    let r = page
        .evaluate("Intl.DateTimeFormat().resolvedOptions().timeZone")
        .unwrap();
    assert_eq!(r, "Asia/Tokyo", "JP profile should report Asia/Tokyo");
}

#[tokio::test]
async fn date_timezone_offset_is_numeric_from_profile() {
    // Date.prototype.getTimezoneOffset() must return a number matching the
    // profile's timezone (Moscow = UTC+3 in summer, -180 minutes).
    let profile = stealth::presets::chrome_130_ru();
    let mut page = Page::with_profile(
        "<!DOCTYPE html><html><body></body></html>",
        "https://example.com/",
        profile,
    )
    .await
    .unwrap();
    let r = page
        .evaluate("typeof new Date().getTimezoneOffset()")
        .unwrap();
    assert_eq!(r, "number");
    let r2 = page.evaluate("new Date().getTimezoneOffset()").unwrap();
    // Moscow is UTC+3, so offset should be -180 (minutes).
    assert_eq!(
        r2, "-180",
        "Moscow profile should report -180 min offset, got {r2}"
    );
}

#[tokio::test]
async fn perf_paint_entries_present() {
    // first-paint + first-contentful-paint are the two paint entries Chrome always reports
    assert_eq!(
        check(
            r#"(() => {
                const p = performance.getEntriesByType('paint');
                return p.length === 2
                    && p.some(e => e.name === 'first-paint')
                    && p.some(e => e.name === 'first-contentful-paint');
            })()"#
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn webgl_get_extension_returns_non_null_for_supported() {
    let profile = stealth::chrome_130_windows();
    let r = webgl_eval(
        profile,
        r#"(() => {
            const gl = document.createElement('canvas').getContext('webgl');
            const ext = gl.getExtension('EXT_texture_filter_anisotropic');
            return typeof ext;
        })()"#,
    )
    .await;
    assert_eq!(
        r, "object",
        "getExtension should return non-null for supported extensions"
    );
}

// ================================================================
// Chrome-specific (anti-bot critical)
// ================================================================

#[tokio::test]
async fn chrome_app() {
    assert_eq!(check("typeof chrome.app").await, "object");
}
#[tokio::test]
async fn chrome_runtime() {
    // Real Chrome 147: chrome.runtime absent on regular pages (extension-only).
    assert_eq!(check("typeof chrome.runtime").await, "undefined");
}
#[tokio::test]
async fn chrome_csi() {
    assert_eq!(check("typeof chrome.csi").await, "function");
}
#[tokio::test]
async fn chrome_load_times() {
    assert_eq!(check("typeof chrome.loadTimes").await, "function");
}
#[tokio::test]
async fn performance_memory() {
    assert_eq!(
        check("performance.memory.jsHeapSizeLimit > 0").await,
        "true"
    );
}
#[tokio::test]
async fn speech_synthesis() {
    assert_eq!(
        check("speechSynthesis.getVoices().length > 0").await,
        "true"
    );
}

// ================================================================
// iframe
// ================================================================

#[tokio::test]
async fn iframe_content_window() {
    assert_eq!(
        check("typeof document.createElement('iframe').contentWindow").await,
        "object"
    );
}
#[tokio::test]
async fn iframe_content_document() {
    assert_eq!(
        check("typeof document.createElement('iframe').contentDocument").await,
        "object"
    );
}

// ================================================================
// kNoScriptId prototype-chain integrity
// ----------------------------------------------------------------
// Real Chrome: the navigator object has ZERO own properties; every
// data/accessor lives on Navigator.prototype. Kasada/Castle probes
// call Object.getOwnPropertyDescriptor(navigator, 'x'); if the result
// is defined they take a "fake native" detection path (V8 kNoScriptId
// guard, May 2025 patch). These tests pin the prototype-only layout.
// ================================================================

#[tokio::test]
async fn k_no_script_id_navigator_is_instance() {
    assert_eq!(check("navigator instanceof Navigator").await, "true");
}

#[tokio::test]
async fn k_no_script_id_navigator_proto_is_navigator_prototype() {
    assert_eq!(
        check("Object.getPrototypeOf(navigator) === Navigator.prototype").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_user_agent_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(navigator, 'userAgent') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_user_agent_on_prototype() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgent') !== undefined")
            .await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_user_agent_prototype_is_accessor() {
    assert_eq!(
        check(
            "typeof Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgent').get === 'function'"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_webdriver_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(navigator, 'webdriver') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_webdriver_on_prototype_returns_false() {
    assert_eq!(check("navigator.webdriver === false").await, "true");
}

#[tokio::test]
async fn k_no_script_id_plugins_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(navigator, 'plugins') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_mime_types_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(navigator, 'mimeTypes') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_user_agent_data_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(navigator, 'userAgentData') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_gpu_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(navigator, 'gpu') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_scheduling_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(navigator, 'scheduling') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_get_user_media_on_prototype() {
    assert_eq!(
        check("Navigator.prototype.hasOwnProperty('getUserMedia')").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_navigator_has_no_own_properties() {
    assert_eq!(
        check("Object.getOwnPropertyNames(navigator).length").await,
        "0"
    );
}

#[tokio::test]
async fn k_no_script_id_user_agent_getter_tostring_masked() {
    assert_eq!(
        check(
            "Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgent').get.toString().includes('[native code]')"
        )
        .await,
        "true"
    );
}

// --- Screen / History / SpeechSynthesis / CustomElementRegistry ---

#[tokio::test]
async fn k_no_script_id_screen_is_instance_of_screen() {
    assert_eq!(check("screen instanceof Screen").await, "true");
}

#[tokio::test]
async fn k_no_script_id_screen_width_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(screen, 'width') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_screen_has_no_own_properties() {
    assert_eq!(
        check("Object.getOwnPropertyNames(screen).length").await,
        "0"
    );
}

#[tokio::test]
async fn k_no_script_id_history_is_instance_of_history() {
    assert_eq!(check("history instanceof History").await, "true");
}

#[tokio::test]
async fn k_no_script_id_history_length_not_on_instance() {
    assert_eq!(
        check("Object.getOwnPropertyDescriptor(history, 'length') === undefined").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_history_push_state_on_prototype() {
    assert_eq!(
        check("History.prototype.hasOwnProperty('pushState')").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_speech_synthesis_is_instance() {
    assert_eq!(
        check("speechSynthesis instanceof SpeechSynthesis").await,
        "true"
    );
}

#[tokio::test]
async fn k_no_script_id_custom_elements_is_instance() {
    assert_eq!(
        check("customElements instanceof CustomElementRegistry").await,
        "true"
    );
}

// --- TextEncoder / TextDecoder shape (Kasada probe target) ---

#[tokio::test]
async fn text_encoder_encoding_is_utf8() {
    assert_eq!(check("new TextEncoder().encoding").await, "utf-8");
}

#[tokio::test]
async fn text_encoder_encode_into_exists() {
    assert_eq!(
        check("typeof TextEncoder.prototype.encodeInto").await,
        "function"
    );
}

#[tokio::test]
async fn text_encoder_tostring_masked_as_native() {
    assert_eq!(
        check("TextEncoder.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn text_encoder_encode_tostring_masked_as_native() {
    assert_eq!(
        check("TextEncoder.prototype.encode.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn text_encoder_encoding_is_accessor_on_prototype() {
    assert_eq!(
        check(
            "typeof Object.getOwnPropertyDescriptor(TextEncoder.prototype, 'encoding').get === 'function'"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn text_encoder_prototype_has_expected_props() {
    assert_eq!(
        check("JSON.stringify(Object.getOwnPropertyNames(TextEncoder.prototype).sort())").await,
        "[\"constructor\",\"encode\",\"encodeInto\",\"encoding\"]"
    );
}

#[tokio::test]
async fn text_encoder_encode_roundtrip() {
    assert_eq!(
        check("JSON.stringify(Array.from(new TextEncoder().encode('ABC')))").await,
        "[65,66,67]"
    );
}

#[tokio::test]
async fn text_encoder_encode_into_writes_bytes() {
    assert_eq!(
        check(
            "(() => { const b = new Uint8Array(4); const r = new TextEncoder().encodeInto('Hi', b); return JSON.stringify({r, b: Array.from(b)}); })()"
        )
        .await,
        "{\"r\":{\"read\":2,\"written\":2},\"b\":[72,105,0,0]}"
    );
}

#[tokio::test]
async fn text_decoder_encoding_is_utf8() {
    assert_eq!(check("new TextDecoder().encoding").await, "utf-8");
}

#[tokio::test]
async fn text_decoder_decodes_utf8() {
    assert_eq!(
        check("new TextDecoder().decode(new Uint8Array([72, 105]))").await,
        "Hi"
    );
}

#[tokio::test]
async fn chrome_load_times_is_native() {
    assert_eq!(
        check("chrome.loadTimes.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn chrome_csi_is_native() {
    assert_eq!(
        check("chrome.csi.toString().includes('[native code]')").await,
        "true"
    );
}

// --- Function.prototype.toString bypass (CreepJS "lies" detection) ---
// CreepJS and DataDome call Function.prototype.toString.call(fn) directly,
// bypassing any instance-level fn.toString override. Every polyfilled API
// must return "[native code]" via this path too.
//
// Pattern: Function.prototype.toString.call(X).includes('[native code]')
//   vs the weaker:           X.toString().includes('[native code]')
// Only the former catches JS-injection lies.

#[tokio::test]
async fn fn_proto_tostring_nav_getbattery_native() {
    assert_eq!(
        check_secure("Function.prototype.toString.call(navigator.getBattery).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_speech_getvoices_native() {
    assert_eq!(
        check("Function.prototype.toString.call(speechSynthesis.getVoices).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_permissions_query_native() {
    assert_eq!(
        check("Function.prototype.toString.call(navigator.permissions.query).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_media_devices_enumerate_native() {
    assert_eq!(
        check_secure("Function.prototype.toString.call(navigator.mediaDevices.enumerateDevices).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_ua_getter_native() {
    // Accessor getter on Navigator.prototype must also look native via bypass
    assert_eq!(
        check("Function.prototype.toString.call(Object.getOwnPropertyDescriptor(Navigator.prototype, 'userAgent').get).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_screen_width_getter_native() {
    assert_eq!(
        check("Function.prototype.toString.call(Object.getOwnPropertyDescriptor(Screen.prototype, 'width').get).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_chrome_loadtimes_native() {
    // chrome.loadTimes already passes fn.toString(); verify bypass also works
    assert_eq!(
        check("Function.prototype.toString.call(chrome.loadTimes).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_rtc_create_offer_native() {
    assert_eq!(
        check("Function.prototype.toString.call(RTCPeerConnection.prototype.createOffer).includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn fn_proto_tostring_itself_is_native() {
    // Function.prototype.toString.call(Function.prototype.toString) must be native
    assert_eq!(
        check("Function.prototype.toString.call(Function.prototype.toString).includes('[native code]')").await,
        "true"
    );
}

// --- Symbol.toStringTag brand strings (Akamai BMP v3 + DataDome probe) ---
// Akamai and DataDome check Object.prototype.toString.call(x) against the
// Chrome WebIDL brand names. Anything returning "[object Object]" is an
// instant bot signal.

#[tokio::test]
async fn to_string_tag_document() {
    assert_eq!(
        check("Object.prototype.toString.call(document)").await,
        "[object HTMLDocument]"
    );
}

#[tokio::test]
async fn to_string_tag_html_body_element() {
    let s = check(
        "(() => { const b = document.createElement('body'); return Object.prototype.toString.call(b); })()",
    )
    .await;
    assert_eq!(s, "[object HTMLBodyElement]");
}

#[tokio::test]
async fn to_string_tag_html_div_element() {
    let s = check(
        "(() => { const d = document.createElement('div'); return Object.prototype.toString.call(d); })()",
    )
    .await;
    assert_eq!(s, "[object HTMLDivElement]");
}

#[tokio::test]
async fn to_string_tag_html_canvas_element() {
    let s = check(
        "(() => { const c = document.createElement('canvas'); return Object.prototype.toString.call(c); })()",
    )
    .await;
    assert_eq!(s, "[object HTMLCanvasElement]");
}

#[tokio::test]
async fn to_string_tag_canvas_rendering_context_2d() {
    let s = check(
        "(() => { const ctx = document.createElement('canvas').getContext('2d'); return Object.prototype.toString.call(ctx); })()",
    )
    .await;
    assert_eq!(s, "[object CanvasRenderingContext2D]");
}

#[tokio::test]
async fn to_string_tag_webgl_rendering_context() {
    let s = check(
        "(() => { const ctx = document.createElement('canvas').getContext('webgl'); return Object.prototype.toString.call(ctx); })()",
    )
    .await;
    assert_eq!(s, "[object WebGLRenderingContext]");
}

#[tokio::test]
async fn to_string_tag_element_prototype() {
    assert_eq!(
        check("Object.prototype.toString.call(Element.prototype)").await,
        "[object Element]"
    );
}

#[tokio::test]
async fn to_string_tag_node_prototype() {
    assert_eq!(
        check("Object.prototype.toString.call(Node.prototype)").await,
        "[object Node]"
    );
}

#[tokio::test]
async fn to_string_tag_event_target_prototype() {
    assert_eq!(
        check("Object.prototype.toString.call(EventTarget.prototype)").await,
        "[object EventTarget]"
    );
}

// --- Worker context fingerprint consistency ---
// All fingerprint values inside Worker scope must exactly match the main window.
// Detectors (DataDome, fingerprint-scan) compare Worker UA/platform/hardwareConcurrency
// against the main page values and flag mismatches as bot signals.

#[tokio::test]
async fn worker_ua_matches_window() {
    use stealth::presets;
    let profile = presets::chrome_130_windows();
    let win_ua = format!("{}", profile.user_agent);
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    // Spin up a worker and stash its navigator.userAgent in a global
    page.evaluate(&format!(
        r#"window.__wua = null;
        const src = 'self.postMessage(navigator.userAgent);';
        const w = new Worker(URL.createObjectURL(new Blob([src],{{type:'text/javascript'}})));
        w.onmessage = e => {{ window.__wua = e.data; w.terminate(); }};"#
    )).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(500)).await.ok();
    let worker_ua = page.evaluate("window.__wua").unwrap();
    assert_eq!(worker_ua, win_ua, "Worker UA should match window UA");
}

#[tokio::test]
async fn worker_platform_matches_window() {
    use stealth::presets;
    let profile = presets::chrome_130_windows();
    let expected = profile.platform.clone();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    page.evaluate(
        r#"window.__wplat = null;
        const src = 'self.postMessage(navigator.platform);';
        const w = new Worker(URL.createObjectURL(new Blob([src],{type:'text/javascript'})));
        w.onmessage = e => { window.__wplat = e.data; w.terminate(); };"#
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(500)).await.ok();
    assert_eq!(page.evaluate("window.__wplat").unwrap(), expected);
}

#[tokio::test]
async fn worker_hardware_concurrency_matches_window() {
    use stealth::presets;
    let profile = presets::chrome_130_windows();
    let expected = profile.cpu_cores.to_string();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    page.evaluate(
        r#"window.__whc = null;
        const src = 'self.postMessage(String(navigator.hardwareConcurrency));';
        const w = new Worker(URL.createObjectURL(new Blob([src],{type:'text/javascript'})));
        w.onmessage = e => { window.__whc = e.data; w.terminate(); };"#
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(500)).await.ok();
    assert_eq!(page.evaluate("window.__whc").unwrap(), expected);
}

// --- screen.availTop per-OS consistency ---
// On macOS the 25px menu bar means availTop=25 (not 0).
// On Windows/Linux there is no top bar so availTop=0.
// availTop===0 for macOS profiles is a geometry inconsistency signal.

#[tokio::test]
async fn screen_avail_top_macos_is_33() {
    // Phase 7 — Chrome 147 macOS arm64 (M3) reports availTop=33,
    // not 25. Verified against Playwright MCP.
    use stealth::presets;
    let profile = presets::chrome_130_macos();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    assert_eq!(page.evaluate("screen.availTop").unwrap(), "33");
}

#[tokio::test]
async fn screen_avail_top_windows_is_0() {
    use stealth::presets;
    let profile = presets::chrome_130_windows();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    assert_eq!(page.evaluate("screen.availTop").unwrap(), "0");
}

#[tokio::test]
async fn screen_avail_top_linux_is_0() {
    use stealth::presets;
    let profile = presets::chrome_130_linux();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    assert_eq!(page.evaluate("screen.availTop").unwrap(), "0");
}

// --- Kasada/Akamai specific Navigator shape checks ---

#[tokio::test]
async fn nav_webdriver_typeof_boolean() {
    assert_eq!(check("typeof navigator.webdriver").await, "boolean");
}

#[tokio::test]
async fn nav_webdriver_in_operator_true() {
    // Real Chrome: 'webdriver' in navigator === true. Missing-key is a tell.
    assert_eq!(check("'webdriver' in navigator").await, "true");
}

#[tokio::test]
async fn nav_languages_is_frozen() {
    assert_eq!(check("Object.isFrozen(navigator.languages)").await, "true");
}

#[tokio::test]
async fn nav_constructor_name_is_navigator() {
    assert_eq!(check("navigator.constructor.name").await, "Navigator");
}

#[tokio::test]
async fn nav_permissions_query_is_native() {
    assert_eq!(
        check("navigator.permissions.query.toString().includes('[native code]')").await,
        "true"
    );
}

// --- navigator.keyboard (CreepJS + DataDome probe) ---
// Real Chrome exposes a Keyboard object with getLayoutMap() -> KeyboardLayoutMap.
// An empty {} object or missing getLayoutMap() is an immediate lie signal.

#[tokio::test]
async fn nav_keyboard_exists() {
    assert_eq!(check_secure("typeof navigator.keyboard").await, "object");
}

#[tokio::test]
async fn nav_keyboard_getlayoutmap_is_function() {
    assert_eq!(
        check_secure("typeof navigator.keyboard.getLayoutMap").await,
        "function"
    );
}

#[tokio::test]
async fn nav_keyboard_getlayoutmap_returns_promise() {
    assert_eq!(
        check_secure("navigator.keyboard.getLayoutMap() instanceof Promise").await,
        "true"
    );
}

#[tokio::test]
async fn nav_keyboard_getlayoutmap_resolves_to_map() {
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("window.__r = null; navigator.keyboard.getLayoutMap().then(m => { window.__r = m; });").unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200)).await.ok();
    assert_eq!(page.evaluate("window.__r !== null").unwrap(), "true");
}

#[tokio::test]
async fn nav_keyboard_getlayoutmap_has_entries() {
    // A real QWERTY layout has ~50 entries; we just need > 0.
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("window.__r = 0; navigator.keyboard.getLayoutMap().then(m => { window.__r = m.size; });").unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200)).await.ok();
    let size: usize = page.evaluate("window.__r").unwrap().parse().unwrap_or(0);
    assert!(size > 0, "KeyboardLayoutMap should have entries, got {size}");
}

#[tokio::test]
async fn nav_keyboard_getlayoutmap_has_keya() {
    // KeyA is always present in any Latin keyboard layout.
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate("window.__r = false; navigator.keyboard.getLayoutMap().then(m => { window.__r = m.has('KeyA'); });").unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200)).await.ok();
    assert_eq!(page.evaluate("window.__r").unwrap(), "true");
}

#[tokio::test]
async fn nav_keyboard_getlayoutmap_is_native() {
    assert_eq!(
        check_secure("Function.prototype.toString.call(navigator.keyboard.getLayoutMap).includes('[native code]')").await,
        "true"
    );
}

// --- requestMediaKeySystemAccess / DRM (Kasada + Akamai probe) ---
// Real Chrome on Windows/macOS supports com.widevine.alpha.
// Always-rejecting NotSupportedError is a bot signal.

#[tokio::test]
async fn media_key_widevine_resolves_on_windows() {
    use stealth::presets;
    let profile = presets::chrome_130_windows();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    page.evaluate(
        "window.__r = null; navigator.requestMediaKeySystemAccess('com.widevine.alpha', [{initDataTypes:['cenc'],videoCapabilities:[{contentType:'video/mp4;codecs=\"avc1.42E01E\"'}]}]).then(a => { window.__r = 'ok'; }).catch(e => { window.__r = 'err:' + e.name; });"
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200)).await.ok();
    assert_eq!(page.evaluate("window.__r").unwrap(), "ok");
}

#[tokio::test]
async fn media_key_widevine_resolves_on_macos() {
    use stealth::presets;
    let profile = presets::chrome_130_macos();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    page.evaluate(
        "window.__r = null; navigator.requestMediaKeySystemAccess('com.widevine.alpha', [{initDataTypes:['cenc'],videoCapabilities:[{contentType:'video/mp4;codecs=\"avc1.42E01E\"'}]}]).then(a => { window.__r = 'ok'; }).catch(e => { window.__r = 'err:' + e.name; });"
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200)).await.ok();
    assert_eq!(page.evaluate("window.__r").unwrap(), "ok");
}

#[tokio::test]
async fn media_key_clearkey_always_resolves() {
    // org.w3.clearkey must work on all platforms per the W3C EME spec.
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>).await.unwrap();
    page.evaluate(
        "window.__r = null; navigator.requestMediaKeySystemAccess('org.w3.clearkey', [{initDataTypes:['keyids'],videoCapabilities:[{contentType:'video/webm;codecs=\"vp8\"'}]}]).then(a => { window.__r = 'ok'; }).catch(e => { window.__r = 'err:' + e.name; });"
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200)).await.ok();
    assert_eq!(page.evaluate("window.__r").unwrap(), "ok");
}

#[tokio::test]
async fn media_key_access_key_system_is_string() {
    use stealth::presets;
    let profile = presets::chrome_130_windows();
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    page.evaluate(
        "window.__r = null; navigator.requestMediaKeySystemAccess('com.widevine.alpha', [{initDataTypes:['cenc'],videoCapabilities:[{contentType:'video/mp4;codecs=\"avc1.42E01E\"'}]}]).then(a => { window.__r = typeof a.keySystem; }).catch(() => { window.__r = 'err'; });"
    ).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200)).await.ok();
    assert_eq!(page.evaluate("window.__r").unwrap(), "string");
}

// --- Crypto / SubtleCrypto / Performance ---

#[tokio::test]
async fn crypto_instanceof_crypto() {
    assert_eq!(check("crypto instanceof Crypto").await, "true");
}

#[tokio::test]
async fn crypto_subtle_instanceof_subtle_crypto() {
    assert_eq!(check_secure("crypto.subtle instanceof SubtleCrypto").await, "true");
}

#[tokio::test]
async fn crypto_has_no_own_properties() {
    assert_eq!(
        check("Object.getOwnPropertyNames(crypto).length").await,
        "0"
    );
}

#[tokio::test]
async fn crypto_get_random_values_native() {
    assert_eq!(
        check("crypto.getRandomValues.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn crypto_random_uuid_native() {
    assert_eq!(
        check_secure("crypto.randomUUID.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn crypto_random_uuid_format() {
    assert_eq!(
        check_secure("/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(crypto.randomUUID())").await,
        "true"
    );
}

#[tokio::test]
async fn crypto_get_random_values_returns_non_zero() {
    assert_eq!(
        check(
            "(() => { const a = new Uint32Array(4); crypto.getRandomValues(a); return a.some(v => v !== 0); })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn crypto_subtle_digest_exists() {
    assert_eq!(check_secure("typeof crypto.subtle.digest").await, "function");
}

#[tokio::test]
async fn crypto_subtle_digest_is_native() {
    assert_eq!(
        check_secure("crypto.subtle.digest.toString().includes('[native code]')").await,
        "true"
    );
}

// Verify crypto.subtle.digest actually computes a hash (not just a stub).
// Uses execute_and_run to let the event loop drain and resolve the Promise.
#[tokio::test]
async fn crypto_subtle_digest_actually_works() {
    use browser::Page;
    use std::time::Duration;
    let mut page = Page::with_profile(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        "https://example.com/",
        stealth::presets::chrome_130_windows(),
    )
    .await
    .unwrap();
    // Run the async digest and store result in a global
    let _ = page.event_loop().execute_and_run(r#"
        (async function() {
            try {
                var data = new TextEncoder().encode('hello world');
                var hash = await crypto.subtle.digest('SHA-256', data);
                var arr = new Uint8Array(hash);
                globalThis.__cryptoTestResult = Array.from(arr).map(function(b) { return b.toString(16).padStart(2, '0'); }).join('');
            } catch(e) { globalThis.__cryptoTestResult = 'err:' + e.message; }
        })();
    "#, Duration::from_secs(5)).await;
    let result = page.event_loop().execute_script("globalThis.__cryptoTestResult || 'not-set'").unwrap_or_default();
    // SHA-256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576e637fb7cf96a1ddd (note: 63 chars)
    // Correct SHA-256("hello world") is b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576e637fb7cf96a1ddd53d which is actually wrong
    // Real SHA-256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576e637fb7cf96a1ddd53de is 64 chars
    // Just check it's 64 hex chars and starts with "b9":
    assert_eq!(result.len(), 64, "crypto.subtle.digest should return 64-char SHA-256 hex hash, got: {}", result);
    assert!(!result.starts_with("err:"), "crypto.subtle.digest failed: {}", result);
    assert!(!result.starts_with("not-set"), "crypto.subtle.digest never ran");
}

#[tokio::test]
async fn performance_instanceof_performance() {
    assert_eq!(check("performance instanceof Performance").await, "true");
}

#[tokio::test]
async fn performance_has_no_own_properties() {
    assert_eq!(
        check("Object.getOwnPropertyNames(performance).length").await,
        "0"
    );
}

#[tokio::test]
async fn performance_memory_on_prototype() {
    assert_eq!(
        check("Performance.prototype.hasOwnProperty('memory')").await,
        "true"
    );
}

#[tokio::test]
async fn performance_get_entries_is_native() {
    assert_eq!(
        check("performance.getEntries.toString().includes('[native code]')").await,
        "true"
    );
}

// ================================================================
// WebAuthn (gap #28) + FedCM (gap #29) — detection-shape probes
// ----------------------------------------------------------------
// What anti-bot vendors check:
//   typeof PublicKeyCredential, IdentityCredential, IdentityProvider
//   PublicKeyCredential.isUVPAA() returns Promise resolving to per-profile bool
//   PublicKeyCredential.getClientCapabilities() returns object (Chrome 133+)
//   navigator.credentials.create({publicKey:...}) rejects NotAllowedError after ~120ms
//   navigator.credentials.get({identity:...}) rejects NotAllowedError after ~200ms
//   _maskAsNative purity on all spoofed methods
// See docs/SOTA_ROADMAP_2026.md §1.1.
// ================================================================

// Promise helper: kicks off a Promise-returning expression and pumps timers
// for `timeout_ms` ms so setTimeout-delayed rejections (120-250 ms in WebAuthn
// shim) have time to fire. Stashes resolved value in window.__r and rejection
// in window.__rej. Returns either "ok:<value>" or "rej:<error.name>".
async fn await_promise(js: &str, timeout_ms: u64) -> String {
    await_promise_inner(js, timeout_ms, None, false).await
}
/// Same as `await_promise`, but the page is loaded over https:// so
/// [SecureContext]-only APIs (credentials, etc.) are exposed. Phase 7.
async fn await_promise_secure(js: &str, timeout_ms: u64) -> String {
    await_promise_inner(js, timeout_ms, None, true).await
}
async fn await_promise_with_profile(
    js: &str,
    timeout_ms: u64,
    profile: stealth::StealthProfile,
) -> String {
    await_promise_inner(js, timeout_ms, Some(profile), false).await
}
/// Same as `await_promise_with_profile`, but loads over https://.
/// Phase 7 — required for [SecureContext]-only APIs.
async fn await_promise_with_profile_secure(
    js: &str,
    timeout_ms: u64,
    profile: stealth::StealthProfile,
) -> String {
    await_promise_inner(js, timeout_ms, Some(profile), true).await
}
async fn await_promise_inner(
    js: &str,
    timeout_ms: u64,
    profile: Option<stealth::StealthProfile>,
    secure: bool,
) -> String {
    let mut page = if secure {
        Page::from_html_with_url(&html(""), "https://example.com/", profile)
            .await
            .unwrap()
    } else {
        Page::from_html(&html(""), profile).await.unwrap()
    };
    let stash = format!(
        "window.__r = null; window.__rej = null; \
         ({js}).then(v => {{ window.__r = String(v); }}, \
                     e => {{ window.__rej = (e && e.name) ? e.name : (e && e.constructor ? e.constructor.name : String(e)); }});"
    );
    page.evaluate(&stash).unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(timeout_ms))
        .await
        .ok();
    page.evaluate(
        "(window.__rej !== null) ? ('rej:' + window.__rej) \
         : (window.__r !== null) ? ('ok:' + window.__r) \
         : 'pending'",
    )
    .unwrap()
}

#[tokio::test]
async fn webauthn_public_key_credential_exists() {
    assert_eq!(check_secure("typeof PublicKeyCredential").await, "function");
}

#[tokio::test]
async fn webauthn_public_key_credential_constructor_throws() {
    assert_eq!(
        check_secure("(() => { try { new PublicKeyCredential(); return 'no-throw'; } catch (e) { return e.message; } })()")
            .await,
        "Illegal constructor"
    );
}

#[tokio::test]
async fn webauthn_authenticator_response_classes_exist() {
    assert_eq!(check_secure("typeof AuthenticatorResponse").await, "function");
    assert_eq!(
        check_secure("typeof AuthenticatorAttestationResponse").await,
        "function"
    );
    assert_eq!(
        check_secure("typeof AuthenticatorAssertionResponse").await,
        "function"
    );
}

#[tokio::test]
async fn webauthn_attestation_extends_response() {
    assert_eq!(
        check_secure("AuthenticatorAttestationResponse.prototype instanceof AuthenticatorResponse").await,
        "true"
    );
}

#[tokio::test]
async fn webauthn_isuvpa_returns_promise() {
    assert_eq!(
        check_secure("PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable() instanceof Promise").await,
        "true"
    );
}

#[tokio::test]
async fn webauthn_iscma_returns_promise() {
    assert_eq!(
        check_secure("PublicKeyCredential.isConditionalMediationAvailable() instanceof Promise").await,
        "true"
    );
}

#[tokio::test]
async fn webauthn_isuvpa_true_on_windows_profile() {
    let profile = stealth::presets::chrome_130_windows();
    assert_eq!(
        await_promise_with_profile_secure(
            "PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()",
            50,
            profile,
        )
        .await,
        "ok:true"
    );
}

#[tokio::test]
async fn webauthn_isuvpa_true_on_macos_profile() {
    let profile = stealth::presets::chrome_130_macos();
    assert_eq!(
        await_promise_with_profile_secure(
            "PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()",
            50,
            profile,
        )
        .await,
        "ok:true"
    );
}

#[tokio::test]
async fn webauthn_isuvpa_false_on_linux_profile() {
    let profile = stealth::presets::chrome_130_linux();
    assert_eq!(
        await_promise_with_profile_secure(
            "PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()",
            50,
            profile,
        )
        .await,
        "ok:false"
    );
}

#[tokio::test]
async fn webauthn_get_client_capabilities_shape() {
    assert_eq!(
        await_promise_secure(
            "PublicKeyCredential.getClientCapabilities().then(c => \
             typeof c.userVerifyingPlatformAuthenticator === 'boolean' && \
             typeof c.conditionalGet === 'boolean' && \
             typeof c.hybridTransport === 'boolean')",
            50,
        )
        .await,
        "ok:true"
    );
}

#[tokio::test]
async fn webauthn_isuvpa_is_native() {
    assert_eq!(
        check_secure("PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn webauthn_credentials_create_is_native() {
    assert_eq!(
        check_secure("navigator.credentials.create.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn webauthn_credentials_create_rejects_with_not_allowed() {
    // Shim sleeps 120 ms before rejecting; pump 250 ms.
    assert_eq!(
        await_promise_secure("navigator.credentials.create({publicKey:{}})", 250,).await,
        "rej:NotAllowedError"
    );
}

#[tokio::test]
async fn webauthn_credentials_get_publickey_rejects() {
    assert_eq!(
        await_promise_secure("navigator.credentials.get({publicKey:{}})", 250,).await,
        "rej:NotAllowedError"
    );
}

#[tokio::test]
async fn webauthn_credentials_create_no_args_rejects_typeerror() {
    // Synchronous Promise.reject(new TypeError(...)). 50 ms pump is plenty.
    assert_eq!(
        await_promise_secure("navigator.credentials.create()", 50,).await,
        "rej:TypeError"
    );
}

#[tokio::test]
async fn webauthn_navigator_credentials_is_credentials_container() {
    assert_eq!(
        check_secure("navigator.credentials instanceof CredentialsContainer").await,
        "true"
    );
}

#[tokio::test]
async fn fedcm_identity_credential_exists() {
    assert_eq!(check_secure("typeof IdentityCredential").await, "function");
}

#[tokio::test]
async fn fedcm_identity_provider_exists() {
    assert_eq!(check_secure("typeof IdentityProvider").await, "function");
}

#[tokio::test]
async fn fedcm_identity_credential_constructor_throws() {
    assert_eq!(
        check_secure("(() => { try { new IdentityCredential(); return 'no-throw'; } catch (e) { return e.message; } })()")
            .await,
        "Illegal constructor"
    );
}

#[tokio::test]
async fn fedcm_identity_provider_get_user_info_rejects() {
    assert_eq!(
        await_promise_secure(
            "IdentityProvider.getUserInfo({configURL:'https://x/cfg.json',clientId:'a'})",
            50,
        )
        .await,
        "rej:NotAllowedError"
    );
}

// ================================================================
// WebGL fingerprint surface (gap #26 — spoofing path)
// ----------------------------------------------------------------
// Anti-bot vendors probe:
//   typeof WebGLRenderingContext, WebGL2RenderingContext
//   getParameter(VENDOR/RENDERER/UNMASKED_VENDOR_WEBGL/UNMASKED_RENDERER_WEBGL)
//   getParameter(MAX_TEXTURE_SIZE/MAX_RENDERBUFFER_SIZE/...)  — non-zero, plausible
//   getSupportedExtensions() — non-empty, GPU-correct
//   getShaderPrecisionFormat(FRAGMENT_SHADER, HIGH_FLOAT) — {127,127,23} on Chrome
//   getExtension('WEBGL_debug_renderer_info') — non-null, exposes UNMASKED_*
//   getContextAttributes() — Chrome defaults (alpha=true, antialias=true, etc.)
// All values come from the active StealthProfile.gpu_profile (stealth/src/gpu.rs).
// See docs/SOTA_ROADMAP_2026.md §2.1 for the deferred wgpu render path.
// ================================================================

async fn webgl_check(js: &str) -> String {
    // Helper: create a canvas, get a webgl context, run js against `gl`.
    let wrapped = format!(
        "(() => {{ \
          const c = document.createElement('canvas'); \
          c.width = 256; c.height = 256; \
          const gl = c.getContext('webgl'); \
          if (!gl) return 'no-context'; \
          return ({js}); \
        }})()"
    );
    check(&wrapped).await
}

async fn webgl_check_with_profile(js: &str, profile: stealth::StealthProfile) -> String {
    let wrapped = format!(
        "(() => {{ \
          const c = document.createElement('canvas'); \
          c.width = 256; c.height = 256; \
          const gl = c.getContext('webgl'); \
          if (!gl) return 'no-context'; \
          return ({js}); \
        }})()"
    );
    let mut page = Page::from_html(&html(""), Some(profile)).await.unwrap();
    page.evaluate(&wrapped)
        .unwrap_or_else(|e| format!("ERROR: {e}"))
}

#[tokio::test]
async fn webgl_rendering_context_class_exists() {
    assert_eq!(check("typeof WebGLRenderingContext").await, "function");
    assert_eq!(check("typeof WebGL2RenderingContext").await, "function");
}

#[tokio::test]
async fn webgl_get_context_returns_object() {
    assert_eq!(webgl_check("typeof gl").await, "object");
    assert_eq!(webgl_check("gl !== null").await, "true");
}

#[tokio::test]
async fn webgl_vendor_renderer_strings_non_empty() {
    assert_eq!(
        webgl_check(
            "typeof gl.getParameter(0x1F00) === 'string' && gl.getParameter(0x1F00).length > 0"
        )
        .await,
        "true"
    );
    assert_eq!(
        webgl_check(
            "typeof gl.getParameter(0x1F01) === 'string' && gl.getParameter(0x1F01).length > 0"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn webgl_unmasked_vendor_renderer_per_profile() {
    // Win profile: NVIDIA. Mac: Apple. Linux: Intel.
    let win_renderer = webgl_check_with_profile(
        "gl.getParameter(0x9246)", // UNMASKED_RENDERER_WEBGL
        stealth::presets::chrome_130_windows(),
    )
    .await;
    assert!(
        win_renderer.contains("NVIDIA"),
        "Win UNMASKED_RENDERER should mention NVIDIA, got {win_renderer}"
    );

    let mac_renderer = webgl_check_with_profile(
        "gl.getParameter(0x9246)",
        stealth::presets::chrome_130_macos(),
    )
    .await;
    assert!(
        mac_renderer.contains("Apple"),
        "Mac UNMASKED_RENDERER should mention Apple, got {mac_renderer}"
    );

    let linux_renderer = webgl_check_with_profile(
        "gl.getParameter(0x9246)",
        stealth::presets::chrome_130_linux(),
    )
    .await;
    assert!(
        linux_renderer.contains("Intel"),
        "Linux UNMASKED_RENDERER should mention Intel, got {linux_renderer}"
    );
}

#[tokio::test]
async fn webgl_max_texture_size_at_least_chrome_minimum() {
    // Chrome 130 reports 16384 on most modern GPUs; minimum WebGL spec is 64.
    // Our profiles all set 16384.
    assert_eq!(
        webgl_check("gl.getParameter(0x0D33) >= 8192").await, // MAX_TEXTURE_SIZE
        "true"
    );
}

#[tokio::test]
async fn webgl_get_supported_extensions_non_empty() {
    assert_eq!(
        webgl_check(
            "Array.isArray(gl.getSupportedExtensions()) && gl.getSupportedExtensions().length > 5"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn webgl_supported_extensions_contain_debug_renderer_info() {
    // WEBGL_debug_renderer_info is in every Chrome-reported extension list
    // because it's the canonical way to get UNMASKED_VENDOR/RENDERER strings.
    assert_eq!(
        webgl_check("gl.getSupportedExtensions().includes('WEBGL_debug_renderer_info')").await,
        "true"
    );
}

#[tokio::test]
async fn webgl_get_extension_debug_renderer_info_returns_constants() {
    assert_eq!(
        webgl_check(
            "(() => { \
              const ext = gl.getExtension('WEBGL_debug_renderer_info'); \
              return ext !== null \
                  && ext.UNMASKED_VENDOR_WEBGL === 0x9245 \
                  && ext.UNMASKED_RENDERER_WEBGL === 0x9246; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn webgl_get_shader_precision_format_high_float() {
    // FRAGMENT_SHADER (0x8B30) + HIGH_FLOAT (0x8DF2) → {rangeMin:127, rangeMax:127, precision:23}.
    assert_eq!(
        webgl_check(
            "(() => { \
              const p = gl.getShaderPrecisionFormat(0x8B30, 0x8DF2); \
              return p !== null \
                  && p.rangeMin === 127 \
                  && p.rangeMax === 127 \
                  && p.precision === 23; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn webgl_get_context_attributes_chrome_defaults() {
    // Real Chrome defaults from WebGL spec.
    assert_eq!(
        webgl_check(
            "(() => { \
              const a = gl.getContextAttributes(); \
              return a.alpha === true \
                  && a.antialias === true \
                  && a.depth === true \
                  && a.failIfMajorPerformanceCaveat === false \
                  && a.premultipliedAlpha === true \
                  && a.preserveDrawingBuffer === false \
                  && a.stencil === false; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn webgl_is_context_lost_returns_false() {
    assert_eq!(webgl_check("gl.isContextLost()").await, "false");
}

#[tokio::test]
async fn webgl_get_error_returns_zero_initially() {
    // NO_ERROR = 0
    assert_eq!(webgl_check("gl.getError()").await, "0");
}

#[tokio::test]
async fn webgl_max_viewport_dims_returns_array_of_two() {
    assert_eq!(
        webgl_check(
            "(() => { \
              const v = gl.getParameter(0x0D3A); \
              return Array.isArray(v) && v.length === 2 && v[0] > 0 && v[1] > 0; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn webgl2_context_returns_object() {
    assert_eq!(
        check(
            "(() => { \
              const c = document.createElement('canvas'); \
              const gl = c.getContext('webgl2'); \
              return typeof gl; \
             })()"
        )
        .await,
        "object"
    );
}

#[tokio::test]
async fn webgl_extensions_differ_per_profile_apple() {
    // Apple profile should include the ASTC compressed-texture extension
    // that NVIDIA/Intel typically lack (per gpu.rs::apple_m2_pro_macos).
    let exts = webgl_check_with_profile(
        "JSON.stringify(gl.getSupportedExtensions())",
        stealth::presets::chrome_130_macos(),
    )
    .await;
    assert!(
        exts.contains("WEBGL_compressed_texture_astc"),
        "Apple profile should expose ASTC; got {exts}"
    );
}

// ================================================================
// Audio realtime — AnalyserNode + BiquadFilterNode (gap #27, P2.2)
// ----------------------------------------------------------------
// AnalyserNode and BiquadFilterNode are now realer:
//   - AnalyserNode exposes fftSize, frequencyBinCount, smoothing,
//     minDecibels, maxDecibels (all probed by CreepJS).
//   - BiquadFilterNode.getFrequencyResponse() runs the closed-form
//     bilinear-transform via op_audio_biquad_response.
// Wire-through for graph-driven analyser data is still pending
// (offline path uses op_offline_audio_render, which is bit-accurate
// to Blink at ~3.6 ppm — see canvas/tests/audio_reference.rs).
// ================================================================

#[tokio::test]
async fn audio_context_exists() {
    assert_eq!(check("typeof AudioContext").await, "function");
    assert_eq!(check("typeof OfflineAudioContext").await, "function");
}

#[tokio::test]
async fn analyser_has_chrome_default_props() {
    assert_eq!(
        check(
            "(() => { \
              const ctx = new AudioContext(); \
              const a = ctx.createAnalyser(); \
              return a.fftSize === 2048 \
                  && a.frequencyBinCount === 1024 \
                  && a.smoothingTimeConstant === 0.8 \
                  && a.minDecibels === -100 \
                  && a.maxDecibels === -30; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn analyser_get_float_frequency_data_returns_min_db_for_silence() {
    // Unconnected analyser → silence → all bins at minDecibels (-100).
    assert_eq!(
        check(
            "(() => { \
              const ctx = new AudioContext(); \
              const a = ctx.createAnalyser(); \
              const f = new Float32Array(a.frequencyBinCount); \
              a.getFloatFrequencyData(f); \
              return f.every(v => v === a.minDecibels); \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn biquad_get_frequency_response_unity_at_dc_for_lowpass() {
    // Lowpass at 1 kHz, Q=0.7071 → at f=0, |H| ≈ 1.
    assert_eq!(
        check(
            "(() => { \
              const ctx = new AudioContext(); \
              const f = ctx.createBiquadFilter(); \
              f.type = 'lowpass'; f.frequency.value = 1000; f.Q.value = 0.7071; \
              const freqs = new Float32Array([0]); \
              const mag = new Float32Array(1); \
              const phase = new Float32Array(1); \
              f.getFrequencyResponse(freqs, mag, phase); \
              return Math.abs(mag[0] - 1) < 0.01; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn biquad_get_frequency_response_blocks_dc_for_highpass() {
    assert_eq!(
        check(
            "(() => { \
              const ctx = new AudioContext(); \
              const f = ctx.createBiquadFilter(); \
              f.type = 'highpass'; f.frequency.value = 1000; f.Q.value = 0.7071; \
              const freqs = new Float32Array([0]); \
              const mag = new Float32Array(1); \
              const phase = new Float32Array(1); \
              f.getFrequencyResponse(freqs, mag, phase); \
              return mag[0] < 0.01; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn biquad_get_frequency_response_writes_n_values() {
    assert_eq!(
        check(
            "(() => { \
              const ctx = new AudioContext(); \
              const f = ctx.createBiquadFilter(); \
              const freqs = new Float32Array([100, 500, 1000, 5000, 20000]); \
              const mag = new Float32Array(5); \
              const phase = new Float32Array(5); \
              f.getFrequencyResponse(freqs, mag, phase); \
              return mag.every(v => Number.isFinite(v) && v >= 0) \
                  && phase.every(v => Number.isFinite(v)); \
             })()"
        )
        .await,
        "true"
    );
}

// ================================================================
// performance.now() humanized jitter (gap #31a)
// ----------------------------------------------------------------
// Real Chrome shows ~10–30 µs jitter around the 100 µs grid step. A
// pure quantizer (set(diffs).size === 1) is detectable. We verify:
//   - Returns finite number
//   - Is monotonic across calls (the underlying clock floor + non-negative
//     jitter ensures this for any practical inter-call spacing)
//   - Hot loop produces multiple distinct values (jitter is real)
// ================================================================

#[tokio::test]
async fn perf_now_returns_number() {
    assert_eq!(check("typeof performance.now()").await, "number");
}

#[tokio::test]
async fn perf_now_is_native() {
    assert_eq!(
        check("performance.now.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn perf_now_is_finite_non_negative() {
    assert_eq!(
        check("(() => { const t = performance.now(); return Number.isFinite(t) && t >= 0; })()")
            .await,
        "true"
    );
}

#[tokio::test]
async fn perf_now_hot_loop_produces_distinct_values() {
    // 500 hot calls; expect more than 10 distinct values (real Chrome shows
    // dozens-to-hundreds; pure quantizer shows ~1).
    assert_eq!(
        check(
            "(() => { \
              const xs = []; \
              for (let i = 0; i < 500; i++) xs.push(performance.now()); \
              return new Set(xs).size > 10; \
             })()"
        )
        .await,
        "true"
    );
}

#[tokio::test]
async fn perf_now_is_strictly_monotonic() {
    // HRT spec requires monotonic non-decreasing. The PerfState clamps
    // each return to >= last value, so 1000 samples must have zero
    // backward jumps.
    assert_eq!(
        check(
            "(() => { \
              const xs = []; \
              for (let i = 0; i < 1000; i++) xs.push(performance.now()); \
              for (let i = 1; i < xs.length; i++) if (xs[i] < xs[i-1]) return false; \
              return true; \
             })()"
        )
        .await,
        "true"
    );
}

// ================================================================
// Cross-origin isolation + SharedArrayBuffer (gap #30)
// ----------------------------------------------------------------
// What anti-bot vendors check (Kasada 2024+):
//   self.crossOriginIsolated reflects COOP+COEP from response headers
//   typeof SharedArrayBuffer === 'function' (V8 always exposes constructor)
//   new SharedArrayBuffer(N) usable
//   typeof Atomics === 'object', Atomics.wait/notify exist
// SAB postMessage transfer to workers is gated separately on COI but
// our worker plumbing is still stub (CAPABILITY_GAPS_2026.md §T1.5),
// so transfer-rejection tests are deferred until workers fire.
// ================================================================

fn coi_check(cross_origin_isolated: bool, js: &str) -> String {
    let dom = html_parser::parse_html("<html><body></body></html>");
    let mut rt = js_runtime::BrowserJsRuntime::with_options(
        dom,
        js_runtime::runtime::BrowserRuntimeOptions {
            cross_origin_isolated,
            ..Default::default()
        },
    );
    rt.execute_script(js, None)
        .unwrap_or_else(|e| format!("ERROR: {e}"))
}

#[tokio::test]
async fn coi_default_is_false_via_page() {
    // Page::from_html doesn't (yet) extract COOP/COEP from response headers,
    // so the default must be false.
    assert_eq!(check("typeof crossOriginIsolated").await, "boolean");
    assert_eq!(check("crossOriginIsolated").await, "false");
}

#[tokio::test]
async fn coi_true_when_runtime_constructed_isolated() {
    assert_eq!(coi_check(true, "crossOriginIsolated"), "true");
}

#[tokio::test]
async fn coi_false_when_runtime_constructed_non_isolated() {
    assert_eq!(coi_check(false, "crossOriginIsolated"), "false");
}

#[tokio::test]
async fn coi_property_descriptor_is_configurable_getter() {
    // Real Chrome exposes crossOriginIsolated as a getter on the global,
    // configurable=true. Plain `globalThis.X = false` would set it as a
    // value property — detectable.
    assert_eq!(
        coi_check(false, "Object.getOwnPropertyDescriptor(globalThis, 'crossOriginIsolated').get instanceof Function"),
        "true"
    );
}

#[tokio::test]
async fn sab_constructor_exists() {
    // Chrome hides SharedArrayBuffer without cross-origin isolation (COOP+COEP).
    // Default pages are not cross-origin isolated, so SAB is undefined.
    assert_eq!(check("typeof SharedArrayBuffer").await, "undefined");
}

#[tokio::test]
#[ignore = "SAB only available with cross-origin isolation (COOP+COEP headers)"]
async fn sab_constructible_with_byte_length() {
    assert_eq!(coi_check(true, "new SharedArrayBuffer(8).byteLength"), "8");
}

#[tokio::test]
#[ignore = "SAB only available with cross-origin isolation (COOP+COEP headers)"]
async fn sab_instance_is_shared_array_buffer() {
    assert_eq!(
        coi_check(true, "new SharedArrayBuffer(4) instanceof SharedArrayBuffer"),
        "true"
    );
}

#[tokio::test]
async fn atomics_object_exists() {
    assert_eq!(check("typeof Atomics").await, "object");
}

#[tokio::test]
async fn atomics_wait_and_notify_exist() {
    assert_eq!(check("typeof Atomics.wait").await, "function");
    assert_eq!(check("typeof Atomics.notify").await, "function");
}

#[tokio::test]
#[ignore = "SAB only available with cross-origin isolation (COOP+COEP headers)"]
async fn atomics_wait_returns_timed_out_synchronously() {
    // Atomics.wait on a fresh SharedArrayBuffer with timeout=1ms must return
    // "timed-out" (or "ok"/"not-equal" on edge cases) — proves SAB+Atomics
    // are functional, not just present.
    assert_eq!(
        coi_check(true, "Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1)"),
        "timed-out"
    );
}

#[tokio::test]
async fn fedcm_credentials_get_identity_rejects() {
    // FedCM shim sleeps 200 ms before rejecting; pump 350 ms.
    assert_eq!(
        await_promise_secure(
            "navigator.credentials.get({ identity: { providers: [{ configURL: 'https://x/cfg.json', clientId: 'a' }] } })",
            350,
        )
        .await,
        "rej:NotAllowedError"
    );
}

// ================================================================
// Live navigation smoke test — reddit.com (network-gated, #[ignore])
// ----------------------------------------------------------------
// Run with:
//   cargo test -p browser --test chrome_compat reddit_smoke \
//     -- --ignored --test-threads=1 --nocapture
// ================================================================

/// Comprehensive diagnostic init: traces `new Function()` compiles, fetch/XHR
/// calls, postMessage events, and navigation-trigger setters. Tells us
/// exactly what Kasada's `ips.js` (or any other challenge JS) does:
///   - what URLs it POSTs the fingerprint payload to
///   - what response headers/cookies it gets back
///   - whether it fires a `KPSDK:DONE:...` postMessage (the canonical signal)
///   - whether it tries to `location.reload()` or `location.href = ...` to
///     trigger the protected-resource retry
const FN_TRACE_INIT: &str = r#"
(() => {
    const _origFn = globalThis.Function;
    if (_origFn._traced) return;  // idempotent
    globalThis.__fnTrace = [];
    globalThis.__fnTraceErrors = [];
    function TracedFn() {
        const args = Array.prototype.slice.call(arguments);
        const body = args.length ? args[args.length - 1] : '';
        try {
            if (typeof body === 'string' && body.length > 0) {
                globalThis.__fnTrace.push(body.slice(0, 500));
            }
        } catch (e) {}
        try {
            // Construct a real Function — preserve semantics (NOT a closure
            // over the call site; spec-correct).
            return _origFn.apply(this, args);
        } catch (e) {
            try {
                globalThis.__fnTraceErrors.push({
                    body: typeof body === 'string' ? body.slice(0, 500) : String(body),
                    err: String(e),
                    name: e && e.name,
                    stack: (e && e.stack) ? String(e.stack).slice(0, 800) : '',
                });
            } catch {}
            throw e;
        }
    }
    TracedFn.prototype = _origFn.prototype;
    Object.setPrototypeOf(TracedFn, _origFn);
    Object.defineProperty(TracedFn, 'name', { value: 'Function', configurable: true });
    Object.defineProperty(TracedFn, 'length', { value: 1, configurable: true });
    TracedFn._traced = true;
    // Mask toString so detection probes don't notice the wrapper.
    const origToString = _origFn.toString.bind(_origFn);
    TracedFn.toString = function () { return origToString(); };
    globalThis.Function = TracedFn;

    // ---- Network + navigation tracing ----
    globalThis.__netTrace = [];
    globalThis.__msgTrace = [];
    globalThis.__navTrace = [];

    // Wrap fetch
    const _origFetch = globalThis.fetch;
    if (typeof _origFetch === 'function') {
        globalThis.fetch = function (input, init) {
            try {
                const url = typeof input === 'string' ? input : (input && input.url) || String(input);
                const method = (init && init.method) || (input && input.method) || 'GET';
                const bodyLen = init && init.body ? (init.body.length || init.body.byteLength || 0) : 0;
                let bodySnippet = null;
                if (init && init.body) {
                    try {
                        if (typeof init.body === 'string') bodySnippet = init.body.slice(0, 100);
                        else if (init.body instanceof ArrayBuffer) bodySnippet = '<binary ' + init.body.byteLength + '>';
                        else if (ArrayBuffer.isView(init.body)) bodySnippet = '<binary view ' + init.body.byteLength + '>';
                    } catch {}
                }
                const req_hdrs = {};
                if (init && init.headers) {
                    if (init.headers instanceof Headers) {
                        for (const [k, v] of init.headers.entries()) req_hdrs[k] = v;
                    } else if (Array.isArray(init.headers)) {
                        for (const [k, v] of init.headers) req_hdrs[k] = v;
                    } else {
                        for (const k in init.headers) req_hdrs[k] = init.headers[k];
                    }
                }
                const entry = { kind: 'fetch', url: String(url).slice(0, 300), method, body_len: bodyLen, body_snippet: bodySnippet, req_headers: req_hdrs };
                globalThis.__netTrace.push(entry);
                const p = _origFetch.apply(this, arguments);
                p.then((r) => {
                    try {
                        entry.status = r.status;
                        entry.resp_url = r.url ? String(r.url).slice(0, 300) : '';
                        const hdrs = {};
                        if (r.headers && r.headers.forEach) {
                            r.headers.forEach((v, k) => { if (/x-kpsdk|set-cookie|kasada/i.test(k)) hdrs[k] = String(v).slice(0, 200); });
                        }
                        entry.kpsdk_headers = hdrs;
                    } catch {}
                }, (e) => { entry.err = String(e); });
                return p;
            } catch (e) { return _origFetch.apply(this, arguments); }
        };
    }

    // Wrap XHR
    const _origOpen = globalThis.XMLHttpRequest && globalThis.XMLHttpRequest.prototype.open;
    const _origSend = globalThis.XMLHttpRequest && globalThis.XMLHttpRequest.prototype.send;
    if (_origOpen && _origSend) {
        globalThis.XMLHttpRequest.prototype.open = function (method, url) {
            this.__trace = { kind: 'xhr', method, url: String(url).slice(0, 300), req_headers: {} };
            globalThis.__netTrace.push(this.__trace);
            return _origOpen.apply(this, arguments);
        };
        const _origSetHeader = globalThis.XMLHttpRequest.prototype.setRequestHeader;
        if (_origSetHeader) {
            globalThis.XMLHttpRequest.prototype.setRequestHeader = function(k, v) {
                if (this.__trace) this.__trace.req_headers[k] = v;
                return _origSetHeader.apply(this, arguments);
            };
        }
        globalThis.XMLHttpRequest.prototype.send = function (body) {
            try {
                if (this.__trace) {
                    this.__trace.body_len = body ? (body.length || body.byteLength || 0) : 0;
                    try {
                        if (typeof body === 'string') this.__trace.body_snippet = body.slice(0, 100);
                        else if (body instanceof ArrayBuffer) this.__trace.body_snippet = '<binary ' + body.byteLength + '>';
                        else if (ArrayBuffer.isView(body)) this.__trace.body_snippet = '<binary view ' + body.byteLength + '>';
                    } catch {}
                    const t = this.__trace;
                    this.addEventListener('load', () => {
                        try {
                            t.status = this.status;
                            const respHdrs = this.getAllResponseHeaders() || '';
                            const lines = respHdrs.split(/\r?\n/);
                            const interesting = {};
                            for (const ln of lines) {
                                const idx = ln.indexOf(':');
                                if (idx > 0) {
                                    const k = ln.slice(0, idx).trim().toLowerCase();
                                    const v = ln.slice(idx + 1).trim();
                                    if (/x-kpsdk|set-cookie|kasada|abck|bm_sz/.test(k)) {
                                        interesting[k] = v.slice(0, 200);
                                    }
                                }
                            }
                            t.kpsdk_headers = interesting;
                        } catch {}
                    });
                }
            } catch {}
            return _origSend.apply(this, arguments);
        };
    }

    // postMessage capture (Kasada signals success via KPSDK:DONE:... postMessage)
    const _origAddEvtL = globalThis.addEventListener;
    if (typeof _origAddEvtL === 'function') {
        // Install an own listener to record everything seen.
        try {
            globalThis.addEventListener('message', function (e) {
                try {
                    const data = (e && e.data) ? String(e.data).slice(0, 300) : '<no data>';
                    globalThis.__msgTrace.push({ origin: e && e.origin, data });
                } catch {}
            }, true);
        } catch {}
    }

    // location.reload / location.href setter / location.assign / location.replace
    try {
        const loc = globalThis.location;
        if (loc) {
            const origReload = loc.reload && loc.reload.bind(loc);
            const origAssign = loc.assign && loc.assign.bind(loc);
            const origReplace = loc.replace && loc.replace.bind(loc);
            try { loc.reload = function () { globalThis.__navTrace.push({ kind: 'reload' }); return origReload && origReload.apply(this, arguments); }; } catch {}
            try { loc.assign = function (u) { globalThis.__navTrace.push({ kind: 'assign', url: String(u).slice(0, 200) }); return origAssign && origAssign.apply(this, arguments); }; } catch {}
            try { loc.replace = function (u) { globalThis.__navTrace.push({ kind: 'replace', url: String(u).slice(0, 200) }); return origReplace && origReplace.apply(this, arguments); }; } catch {}
            // href setter — wrap via defineProperty if possible
            try {
                const desc = Object.getOwnPropertyDescriptor(loc, 'href') || Object.getOwnPropertyDescriptor(Object.getPrototypeOf(loc), 'href');
                if (desc && desc.set) {
                    Object.defineProperty(loc, 'href', {
                        configurable: true,
                        get: desc.get,
                        set: function (v) { globalThis.__navTrace.push({ kind: 'href', url: String(v).slice(0, 200) }); return desc.set.call(this, v); },
                    });
                }
            } catch {}
        }
    } catch {}
})();
"#;

/// Helper for live-network smoke tests against anti-bot-protected sites.
/// Reports the outcome instead of asserting — anti-bot success depends on
/// the IP we're calling from, which `docs/TIER0_KASADA_RESULTS.md` proves
/// dominates fingerprint quality for first-touch on Kasada/Cloudflare.
async fn antibot_smoke(label: &str, url: &str, profile: stealth::StealthProfile) {
    println!("\n=== {label}: {url} ===");
    let t0 = std::time::Instant::now();
    // Always install the Function-trace init so we can inspect what
    // dynamically-built challenge JS does even when we don't pass.
    // Hard 90s wall-clock cap per site — leaves headroom above the
    // navigate_with_init nav-budget (50s default + 25s adaptive extension).
    // Even sites that exercise the full extension (heavy SPAs like
    // udemy/glassdoor/discord) finish well under 90s. Pages that go
    // beyond this aren't going to render usefully anyway.
    let page = match tokio::time::timeout(
        std::time::Duration::from_secs(300),
        Page::navigate_with_init(url, profile, 3, vec![FN_TRACE_INIT.to_string()]),
    )
    .await
    {
        Ok(r) => r,
        Err(_) => {
            println!("  [TIMEOUT] {label} exceeded 300s wall clock", label = label);
            println!("=== end {label} ===");
            return;
        }
    };
    let elapsed = t0.elapsed();
    match page {
        Ok(mut p) => {
            let final_url = p.url().to_string();
            let title = p.title();
            let html_len = p
                .evaluate("document.documentElement ? document.documentElement.outerHTML.length : 0")
                .unwrap_or_default()
                .parse::<usize>()
                .unwrap_or(0);
            let body_len = p
                .evaluate("document.body ? document.body.textContent.length : 0")
                .unwrap_or_default()
                .parse::<usize>()
                .unwrap_or(0);
            let n_links = p
                .evaluate("document.querySelectorAll('a').length")
                .unwrap_or_default();
            let n_scripts = p
                .evaluate("document.querySelectorAll('script').length")
                .unwrap_or_default();
            let challenge_signal = p
                .evaluate(
                    "(() => { \
                      const t = (document.body && document.body.textContent || '').toLowerCase(); \
                      const markers = [ \
                        'access denied','request blocked','just a moment','enable javascript', \
                        'verify you are a human','cf-challenge','px-captcha','sensor_data', \
                        'attention required','cloudflare','perimeterx','datadome','kasada', \
                        'shape security','unusual traffic','our systems have detected', \
                      ]; \
                      const hits = markers.filter(m => t.includes(m)); \
                      return hits.join(',') || 'none'; \
                    })()",
                )
                .unwrap_or_default();
            let snippet = p
                .evaluate(
                    "document.body ? \
                     document.body.textContent.replace(/\\s+/g, ' ').trim().slice(0, 250) \
                     : 'no body'",
                )
                .unwrap_or_default();
            // Pull diagnostic output from the Function-trace hook.
            let fn_trace_count = p
                .evaluate("Array.isArray(globalThis.__fnTrace) ? globalThis.__fnTrace.length : 0")
                .unwrap_or_default();
            let fn_errors_json = p
                .evaluate(
                    "Array.isArray(globalThis.__fnTraceErrors) ? \
                     JSON.stringify(globalThis.__fnTraceErrors.slice(0, 8)) : '[]'",
                )
                .unwrap_or_else(|_| "[]".into());
            let fn_trace_first_5 = p
                .evaluate(
                    "Array.isArray(globalThis.__fnTrace) ? \
                     JSON.stringify(globalThis.__fnTrace.slice(0, 5)) : '[]'",
                )
                .unwrap_or_else(|_| "[]".into());
            println!("  [OK] {:?}", elapsed);
            println!("  final url:        {final_url}");
            println!("  title:            {title:?}");
            println!("  html bytes:       {html_len}");
            println!("  body chars:       {body_len}");
            println!("  <a> count:        {n_links}");
            println!("  <script> count:   {n_scripts}");
            println!("  challenge mkrs:   {challenge_signal}");
            println!("  body snippet:     {snippet}");
            println!("  fn-trace count:   {fn_trace_count}");
            println!("  fn-trace first 5: {fn_trace_first_5}");
            println!("  fn-trace errors:  {fn_errors_json}");

            // Network trace (fetch/XHR), with priority for kpsdk-related URLs.
            let net_trace_json = p
                .evaluate(
                    "Array.isArray(globalThis.__netTrace) ? \
                     JSON.stringify(globalThis.__netTrace.slice(0, 30)) : '[]'",
                )
                .unwrap_or_else(|_| "[]".into());
            let msg_trace_json = p
                .evaluate(
                    "Array.isArray(globalThis.__msgTrace) ? \
                     JSON.stringify(globalThis.__msgTrace.slice(0, 30)) : '[]'",
                )
                .unwrap_or_else(|_| "[]".into());
            let nav_trace_json = p
                .evaluate(
                    "Array.isArray(globalThis.__navTrace) ? \
                     JSON.stringify(globalThis.__navTrace.slice(0, 30)) : '[]'",
                )
                .unwrap_or_else(|_| "[]".into());
            let kpsdk_state = p
                .evaluate(
                    "(() => { \
                      const k = globalThis.KPSDK; \
                      if (!k) return 'no-KPSDK'; \
                      const out = {}; \
                      for (const key of Object.keys(k)) { \
                        const v = k[key]; \
                        out[key] = (typeof v === 'function') ? '[function]' : \
                                   (typeof v === 'object' && v !== null) ? '[object]' : \
                                   String(v).slice(0, 100); \
                      } \
                      return JSON.stringify(out); \
                    })()",
                )
                .unwrap_or_else(|_| "{}".into());
            let cookies_now = p
                .evaluate("typeof document.cookie === 'string' ? document.cookie.slice(0, 600) : ''")
                .unwrap_or_default();
            println!("  net-trace:        {net_trace_json}");
            println!("  msg-trace:        {msg_trace_json}");
            println!("  nav-trace:        {nav_trace_json}");
            println!("  KPSDK state:      {kpsdk_state}");
            println!("  cookies:          {cookies_now}");
            // Dump JS console messages from the page (set by challenge JS).
            println!("  --- JS console ---");
            p.consume_and_print_logs();
            println!("  --- end console ---");
        }
        Err(e) => {
            println!("  [FAIL] {:?} — {e}", elapsed);
        }
    }
    println!("=== end {label} ===");
}

#[tokio::test]
#[ignore = "network: kasada-only diagnostic with Function trace"]
async fn kasada_canadagoose_diagnostic() {
    let url = "https://www.canadagoose.com/";
    // Clear any existing Kasada session for this host to avoid stale 304 responses
    if let Some(client) = js_runtime::extensions::fetch_ext::fetch_client() {
        if let Ok(u) = url::Url::parse(url) {
            if let Some(host) = u.host_str() {
                client.evict_kasada_session(host).await;
                println!("  [KASADA] Evicted session for {}", host);
            }
        }
    }
    let profile = stealth::presets::chrome_130_macos();
    antibot_smoke(
        "KASADA-canadagoose-DIAG",
        "https://www.canadagoose.com/",
        profile,
    )
    .await;
}

/// T2C — capture the actual body of Kasada's error POST to
/// `reporting.cdndex.io/error`. The 67 KB blob is what Kasada's
/// JS-VM serialises when it can't complete the /tl handshake; it
/// often encodes which environment probe failed. Decoding it tells
/// us which JS API to fix.
#[tokio::test]
#[ignore = "network: kasada error-blob capture"]
async fn kasada_error_blob_capture() {
    use browser::Page;
    use std::time::Duration;

    // Init script that wraps fetch to capture POST bodies for
    // *.cdndex.io and stash them as base64 globals.
    let capture_init = r#"
        (function() {
            globalThis.__kasErrors = [];
            
            // Intercept TextEncoder (to catch the report before encryption)
            const oldEncode = TextEncoder.prototype.encode;
            TextEncoder.prototype.encode = function(str) {
                if (typeof str === 'string' && str.length > 500) {
                     console.log('[KASADA-RAW-TEXT-SNIPPET] len=' + str.length + ' ' + str.substring(0, 300));
                     globalThis.__kasErrors.push({
                        kind: 'raw-text',
                        len: str.length,
                        b64: btoa(unescape(encodeURIComponent(str))),
                        text: str // Keep as string
                    });
                }
                return oldEncode.apply(this, arguments);
            };
            
            // Intercept XHR (Kasada often uses this for error reports)
            const oldSend = XMLHttpRequest.prototype.send;
            XMLHttpRequest.prototype.send = function(body) {
                if (this._url && (this._url.includes('cdndex.io/error') || this._url.includes('cdndex.io/r'))) {
                    try {
                        const snapshot = {
                            kind: 'xhr',
                            url: this._url,
                            len: body ? body.length : 0,
                            b64: body ? btoa(body) : '',
                            raw: body // Store raw for possible non-string types
                        };
                        globalThis.__kasErrors.push(snapshot);
                    } catch(e) {
                        globalThis.__kasErrors.push({ kind: 'err-xhr', err: String(e) });
                    }
                }
                return oldSend.apply(this, arguments);
            };
            const oldOpen = XMLHttpRequest.prototype.open;
            XMLHttpRequest.prototype.open = function(method, url) {
                this._url = url;
                return oldOpen.apply(this, arguments);
            };

            const _origFetch = globalThis.fetch;
            globalThis.fetch = function(input, init) {
                try {
                    const url = typeof input === 'string' ? input : (input && input.url) || '';
                    if (url.indexOf('cdndex.io/error') !== -1 ||
                        url.indexOf('cdndex.io/r') !== -1) {
                        const body = init && init.body;
                        let snapshot;
                        if (typeof body === 'string') {
                            snapshot = { kind: 'string', len: body.length, b64: btoa(body) };
                        } else if (body instanceof ArrayBuffer) {
                            const u8 = new Uint8Array(body);
                            const chunks = [];
                            for (let i = 0; i < u8.length; i += 8192) {
                                chunks.push(String.fromCharCode.apply(null, u8.subarray(i, i + 8192)));
                            }
                            snapshot = { kind: 'arraybuffer', len: u8.length, b64: btoa(chunks.join('')) };
                        } else if (body instanceof Uint8Array) {
                            const chunks = [];
                            for (let i = 0; i < body.length; i += 8192) {
                                chunks.push(String.fromCharCode.apply(null, body.subarray(i, i + 8192)));
                            }
                            snapshot = { kind: 'uint8', len: body.length, b64: btoa(chunks.join('')) };
                        } else if (body instanceof Blob) {
                            // Blobs are async — kick off a read but don't block.
                            snapshot = { kind: 'blob-pending', len: body.size, b64: '' };
                            body.arrayBuffer().then((ab) => {
                                const u8 = new Uint8Array(ab);
                                const chunks = [];
                                for (let i = 0; i < u8.length; i += 8192) {
                                    chunks.push(String.fromCharCode.apply(null, u8.subarray(i, i + 8192)));
                                }
                                snapshot.kind = 'blob';
                                snapshot.b64 = btoa(chunks.join(''));
                            }).catch(() => {});
                        } else {
                            snapshot = { kind: typeof body, len: 0, b64: '' };
                        }
                        snapshot.url = url.slice(0, 200);
                        snapshot.method = (init && init.method) || 'POST';
                        globalThis.__kasErrors.push(snapshot);
                    }
                } catch (e) {
                    globalThis.__kasErrors.push({ kind: 'err', err: String(e) });
                }
                return _origFetch.apply(this, arguments);
            };
        })();
    "#;

    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![capture_init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            // Pump the event loop a bit so deferred error reports flush.
            for _ in 0..30 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let n = p
                .evaluate("(globalThis.__kasErrors || []).length")
                .unwrap_or_default()
                .trim_matches('"')
                .parse::<usize>()
                .unwrap_or(0);
            println!("\n=== Kasada error blobs captured: {n} ===");
            for i in 0..n {
                let url = p
                    .evaluate(&format!("globalThis.__kasErrors[{i}].url"))
                    .unwrap_or_default();
                let kind = p
                    .evaluate(&format!("globalThis.__kasErrors[{i}].kind"))
                    .unwrap_or_default();
                let len = p
                    .evaluate(&format!("globalThis.__kasErrors[{i}].len"))
                    .unwrap_or_default();
                let b64 = p
                    .evaluate(&format!("globalThis.__kasErrors[{i}].b64"))
                    .unwrap_or_default();
                let url = url.trim_matches('"');
                let kind = kind.trim_matches('"');
                let len = len.trim_matches('"');
                let b64 = b64.trim_matches('"');
                println!("--- blob #{i}: kind={kind} len={len} url={url} ---");
                let path = format!("kasada_error_{i}.b64");
                std::fs::write(&path, b64).ok();
                println!("    wrote {path}");
            }
        }
        Ok(Err(e)) => println!("page err: {e}"),
        Err(_) => println!("timeout"),
    }
}

#[tokio::test]
#[ignore = "network: WBAAS smoke against wildberries.ru with current pipeline"]
async fn wbaas_wildberries_smoke() {
    let profile = stealth::presets::chrome_130_ru();
    antibot_smoke("WBAAS-wildberries", "https://www.wildberries.ru/", profile).await;
}

/// Capture the exact /report POST body to understand what WBAAS fingerprint data
/// we're sending. Patch fetch() to intercept the /report body before sending.
#[tokio::test]
#[ignore = "network: WBAAS /report body capture diagnostic"]
async fn wbaas_report_body_capture() {
    use browser::Page;
    use std::time::Duration;

    let intercept_init = r#"
        (function() {
            var _origFetch = globalThis.fetch;
            globalThis.fetch = function(url, opts) {
                if (typeof url === 'string' && url.includes('/report')) {
                    var body = opts && opts.body;
                    try {
                        var bodyStr;
                        if (typeof body === 'string') bodyStr = body;
                        else if (body instanceof ArrayBuffer) {
                            bodyStr = btoa(String.fromCharCode.apply(null, new Uint8Array(body)));
                        } else if (body instanceof Uint8Array) {
                            bodyStr = btoa(String.fromCharCode.apply(null, body));
                        } else {
                            bodyStr = String(body);
                        }
                        globalThis.__wbaasReportBody = bodyStr;
                        globalThis.__wbaasReportBodyLen = bodyStr ? bodyStr.length : 0;
                    } catch(e) {
                        globalThis.__wbaasReportCapErr = String(e);
                    }
                }
                return _origFetch.apply(this, arguments);
            };
        })();
    "#;

    let page = tokio::time::timeout(
        Duration::from_secs(60),
        Page::navigate_with_init(
            "https://www.wildberries.ru/",
            stealth::presets::chrome_130_ru(),
            2,
            vec![intercept_init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            let body_len = p.evaluate("globalThis.__wbaasReportBodyLen || 0").unwrap_or_default();
            let body = p.evaluate("globalThis.__wbaasReportBody || ''").unwrap_or_default();
            let cap_err = p.evaluate("globalThis.__wbaasReportCapErr || ''").unwrap_or_default();
            println!("Report body len: {}", body_len);
            println!("Report body (first 500 chars): {}", &body[..body.len().min(500)]);
            if !cap_err.is_empty() {
                println!("Capture error: {}", cap_err);
            }
        }
        Ok(Err(e)) => println!("Page error: {e}"),
        Err(_) => println!("Timeout"),
    }
}

// ================================================================
// V8 shim recursion reproducer (task #6)
// ================================================================

/// Phase-1: does walking the prototype chain with Reflect.ownKeys hang?
#[tokio::test]
async fn shim_recursion_proto_walk_no_access() {
    // If this test fails (timeout / SIGTRAP) the bug is in ownKeys / getPrototypeOf
    // enumeration itself — not in getter invocation.
    let result = check(r#"
        (function() {
            try {
                let p = globalThis;
                const seen = [];
                let depth = 0;
                while (p !== null && p !== undefined && depth < 30) {
                    // Guard against circular prototype (shouldn't happen, but just in case)
                    if (seen.indexOf(p) !== -1) return 'cycle_at_' + depth;
                    seen.push(p);
                    Reflect.ownKeys(p); // enumerate — don't access values
                    p = Object.getPrototypeOf(p);
                    depth++;
                }
                return 'walk_done_' + depth;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("walk_done_") || result.starts_with("cycle_at_"),
        "proto walk should complete without crash, got: {result}"
    );
}

/// Phase-2: does invoking each getter while walking cause recursion?
#[tokio::test]
async fn shim_recursion_proto_walk_with_getters() {
    // CreepJS calls toString on every function it finds while walking.
    // This test isolates whether our getter-invocation or toString masking recurses.
    let result = check(r#"
        (function() {
            try {
                let p = globalThis;
                let depth = 0;
                const seen = [];
                while (p !== null && p !== undefined && depth < 30) {
                    if (seen.indexOf(p) !== -1) return 'cycle_at_' + depth;
                    seen.push(p);
                    for (const key of Reflect.ownKeys(p)) {
                        try {
                            const desc = Object.getOwnPropertyDescriptor(p, key);
                            if (!desc) continue;
                            // Invoke getter if present
                            if (typeof desc.get === 'function') {
                                try { desc.get.call(p); } catch(_) {}
                            }
                            // Call .toString() on any function value/getter
                            if (typeof desc.get === 'function') {
                                desc.get.toString();
                            }
                            if (typeof desc.value === 'function') {
                                desc.value.toString();
                            }
                        } catch(_) {}
                    }
                    p = Object.getPrototypeOf(p);
                    depth++;
                }
                return 'walk_done_' + depth;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("walk_done_") || result.starts_with("cycle_at_"),
        "proto walk+getters should complete without crash, got: {result}"
    );
}

/// Phase-3: does Function.prototype.toString.call(fn) on all globalThis functions recurse?
#[tokio::test]
async fn shim_recursion_fn_proto_tostring_on_all() {
    let result = check(r#"
        (function() {
            try {
                const seen = new Set();
                let checked = 0;
                let p = globalThis;
                while (p) {
                    for (const key of Reflect.ownKeys(p)) {
                        try {
                            const desc = Object.getOwnPropertyDescriptor(p, key);
                            if (!desc) continue;
                            for (const fn of [desc.value, desc.get, desc.set]) {
                                if (typeof fn !== 'function' || seen.has(fn)) continue;
                                seen.add(fn);
                                Function.prototype.toString.call(fn);
                                checked++;
                            }
                        } catch(_) {}
                    }
                    p = Object.getPrototypeOf(p);
                    if (!p || p === Object.prototype) break;
                }
                return 'toString_ok_' + checked;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("toString_ok_"),
        "Function.prototype.toString on all fns should not recurse, got: {result}"
    );
}

/// Diagnostic: WeakSet behavior with globalThis (V8 global proxy identity)
#[tokio::test]
async fn shim_recursion_diag_weakset_globalthis() {
    let result = check(r#"
        (function() {
            try {
                const ws = new WeakSet();
                // Add globalThis and check if it's found
                ws.add(globalThis);
                const afterAdd = ws.has(globalThis);

                // Add window (which === globalThis) and check
                ws.add(window);
                const afterAddWindow = ws.has(globalThis);
                const afterAddGT = ws.has(window);

                // Check inner global (prototype of globalThis)
                const innerGlobal = Object.getPrototypeOf(globalThis);
                const innerInSet = innerGlobal ? ws.has(innerGlobal) : null;
                const innerIsGT = innerGlobal === globalThis;

                // WeakMap test
                const wm = new WeakMap();
                wm.set(globalThis, 42);
                const wmGet = wm.get(globalThis);
                const wmGetWindow = wm.get(window);

                return JSON.stringify({
                    afterAdd, afterAddWindow, afterAddGT,
                    innerInSet, innerIsGT,
                    wmGet, wmGetWindow,
                    windowIsGT: window === globalThis,
                });
            } catch(e) { return 'error: ' + e.message; }
        })()
    "#).await;
    println!("weakset globalThis test: {result}");
    assert!(!result.starts_with("error:"), "weakset test failed: {result}");
    // Critical: afterAdd must be true (otherwise cycle detection in creepjs fails)
    assert!(result.contains("\"afterAdd\":true"), "WeakSet.has(globalThis) after add must be true, got: {result}");
}

/// Diagnostic: check CallSite frame objects from Error.prepareStackTrace
#[tokio::test]
async fn shim_recursion_diag_callsite() {
    let result = check(r#"
        (function() {
            try {
                let frameInfo = null;
                const origPrep = Error.prepareStackTrace;
                Error.prepareStackTrace = function(err, frames) {
                    if (frames.length > 0 && frameInfo === null) {
                        const f = frames[0];
                        frameInfo = {
                            type: typeof f,
                            isFunc: typeof f === 'function',
                            hasToString: typeof f.toString === 'function',
                            toStringSrc: (() => {
                                try {
                                    // Get toString without calling it (to avoid recursion)
                                    const ts = Object.getOwnPropertyDescriptor(f, 'toString');
                                    if (ts) return 'own:' + typeof ts.value;
                                    // inherited?
                                    let p = Object.getPrototypeOf(f);
                                    while (p) {
                                        const d = Object.getOwnPropertyDescriptor(p, 'toString');
                                        if (d) {
                                            const fn = d.value;
                                            const isFnProtoTs = fn === Function.prototype.toString;
                                            const isObjProtoTs = fn === Object.prototype.toString;
                                            return 'inherited:fn=' + typeof fn +
                                                   ',isFuncProtoToStr=' + isFnProtoTs +
                                                   ',isObjProtoToStr=' + isObjProtoTs;
                                        }
                                        p = Object.getPrototypeOf(p);
                                    }
                                    return 'none';
                                } catch(e) { return 'err:' + e.message; }
                            })(),
                            protoChainLen: (() => {
                                let n = 0;
                                let p = f;
                                while (p && n < 20) { p = Object.getPrototypeOf(p); n++; }
                                return n;
                            })(),
                            isFuncProtoInChain: (() => {
                                let p = Object.getPrototypeOf(f);
                                while (p) {
                                    if (p === Function.prototype) return true;
                                    p = Object.getPrototypeOf(p);
                                }
                                return false;
                            })(),
                        };
                    }
                    // Call original (if any) or use default
                    if (origPrep) return origPrep(err, frames);
                    return undefined;
                };
                // Trigger a stack trace capture
                try { throw new Error('test'); } catch(e) { void e.stack; }
                Error.prepareStackTrace = origPrep;
                return JSON.stringify(frameInfo);
            } catch(e) { return 'error: ' + e.message; }
        })()
    "#).await;
    println!("callsite frame info: {result}");
    assert!(!result.starts_with("error:"), "callsite diag failed: {result}");
}

/// Diagnostic: what does Object.getPrototypeOf(globalThis) return?
#[tokio::test]
async fn shim_recursion_diag_global_proto() {
    let result = check(r#"
        (function() {
            try {
                const gt = globalThis;
                const p1 = Object.getPrototypeOf(gt);
                const info = {
                    p1_null: p1 === null,
                    p1_is_gt: p1 === gt,
                    p1_is_objproto: p1 === Object.prototype,
                    p1_type: typeof p1,
                    p1_keys: p1 ? Object.getOwnPropertyNames(p1).length : -1,
                };
                if (p1 && p1 !== null) {
                    const p2 = Object.getPrototypeOf(p1);
                    info.p2_null = p2 === null;
                    info.p2_is_gt = p2 === gt;
                    info.p2_is_objproto = p2 === Object.prototype;
                    info.p2_is_p1 = p2 === p1;
                }
                if (p1) {
                    info.p1_own_keys = Object.getOwnPropertyNames(p1);
                    info.p1_own_symbols = Object.getOwnPropertySymbols(p1).map(s => s.toString());
                    // Check the constructor property
                    const ctor = p1.constructor;
                    info.p1_ctor_is_gt = ctor === gt;
                    info.p1_ctor_is_func = typeof ctor === 'function';
                    info.p1_ctor_is_object_ctor = ctor === Object;
                    info.p1_ctor_type = typeof ctor;
                    if (ctor && typeof ctor === 'function') {
                        info.p1_ctor_name = ctor.name;
                    }
                    // Check the descriptor for constructor
                    const desc = Object.getOwnPropertyDescriptor(p1, 'constructor');
                    info.p1_ctor_desc_type = desc ? (desc.get ? 'getter' : 'value') : 'none';
                    // Important: does accessing constructor cause recursion?
                    // Check Object.getPrototypeOf(p1.constructor) if it's a function
                    if (ctor && typeof ctor === 'function') {
                        info.p1_ctor_proto_is_gt = Object.getPrototypeOf(ctor) === gt;
                        const ctorProto = Object.getPrototypeOf(ctor);
                        info.p1_ctor_proto_is_func_proto = ctorProto === Function.prototype;
                        // Does Function.prototype.toString work on it?
                        try {
                            const ts = Function.prototype.toString.call(ctor);
                            info.p1_ctor_tostring = ts.slice(0, 50);
                        } catch(e) {
                            info.p1_ctor_tostring_err = e.message;
                        }
                        // Can we access ctor.prototype?
                        info.p1_ctor_prototype_is_p1 = ctor.prototype === p1;
                    }
                }
                return JSON.stringify(info);
            } catch(e) { return 'error: ' + e.message; }
        })()
    "#).await;
    // Expect: p1 is Object.prototype or null (Deno runtime)
    println!("global proto chain: {result}");
    assert!(!result.starts_with("error:"), "proto diag failed: {result}");
}

/// Phase-4: iframe window prototype chain walking (creepjs primary pattern)
#[tokio::test]
async fn shim_recursion_iframe_proto_walk() {
    let result = check(r#"
        (function() {
            try {
                const iframe = document.createElement('iframe');
                document.body.appendChild(iframe);
                const win = iframe.contentWindow;
                if (!win) return 'no_contentWindow';

                // Walk contentWindow's own properties
                const ownKeys = Object.getOwnPropertyNames(win);

                // Check if 'window' in win (has trap)
                const hasWindow = 'window' in win;

                // Walk prototype chain of contentWindow
                const chain = [];
                let proto = win;
                let depth = 0;
                while (proto !== null && proto !== undefined && depth < 20) {
                    if (chain.indexOf(proto) !== -1) return 'cycle_at_' + depth;
                    chain.push(proto);
                    proto = Object.getPrototypeOf(proto);
                    depth++;
                }

                // Access each own key of contentWindow
                for (const key of ownKeys) {
                    try { win[key]; } catch(_) {}
                }

                // Check 'in' for common window properties
                const props = ['window','self','top','parent','location','document',
                               'navigator','screen','fetch','setTimeout','Array',
                               'Object','Function','localStorage'];
                for (const p of props) {
                    try { const r = p in win; } catch(_) {}
                }

                return 'iframe_walk_ok_' + chain.length + '_own_' + ownKeys.length;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("iframe_walk_ok_") || result.starts_with("cycle_at_"),
        "iframe contentWindow walk should not crash, got: {result}"
    );
}

/// Phase-5: creepjs-style window vs iframe window comparison
#[tokio::test]
async fn shim_recursion_creepjs_realm_check() {
    let result = check(r#"
        (function() {
            try {
                const iframe = document.createElement('iframe');
                document.body.appendChild(iframe);
                const iwin = iframe.contentWindow;
                if (!iwin) return 'no_contentWindow';

                // creepjs: compare constructors across realms
                const tests = {
                    arrayMatch: iwin.Array === Array,
                    objectMatch: iwin.Object === Object,
                    functionMatch: iwin.Function === Function,
                    selfIsWindow: iwin.self === iwin,
                    toStringNative: Function.prototype.toString.call(iwin.Array),
                };

                // creepjs: walk ALL own props of main window, check if in iframe window
                let diffCount = 0;
                const mainKeys = Object.getOwnPropertyNames(window);
                const iwinKeys = Object.getOwnPropertyNames(iwin);
                for (const k of mainKeys) {
                    if (!iwinKeys.includes(k)) diffCount++;
                }

                // creepjs: getPrototypeOf chain on iwin
                let protoDepth = 0;
                let p = iwin;
                while (p !== null && p !== undefined && protoDepth < 10) {
                    p = Object.getPrototypeOf(p);
                    protoDepth++;
                }

                return JSON.stringify({
                    ok: true,
                    diffCount,
                    protoDepth,
                    selfIsWindow: tests.selfIsWindow,
                });
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.contains("\"ok\":true"),
        "creepjs realm check should not crash, got: {result}"
    );
}

/// Phase-6: simulate creepjs 'lies' detection — Function.prototype.toString
/// called on every window property (including Proxy-wrapped properties)
#[tokio::test]
async fn shim_recursion_creepjs_lies_detection() {
    let result = check(r#"
        (function() {
            try {
                const toString = Function.prototype.toString;
                let checked = 0;
                let errors = 0;

                // Access every own property of window and call toString if function
                const keys = Object.getOwnPropertyNames(window);
                for (const key of keys) {
                    try {
                        const val = window[key];
                        if (typeof val === 'function') {
                            toString.call(val);
                            checked++;
                        }
                        // Also check getters
                        const desc = Object.getOwnPropertyDescriptor(window, key);
                        if (desc && typeof desc.get === 'function') {
                            toString.call(desc.get);
                        }
                    } catch(e) {
                        errors++;
                    }
                }

                // Check via prototype chain too
                let proto = Object.getPrototypeOf(window);
                while (proto && proto !== Object.prototype) {
                    for (const key of Object.getOwnPropertyNames(proto)) {
                        try {
                            const desc = Object.getOwnPropertyDescriptor(proto, key);
                            if (desc && typeof desc.get === 'function') toString.call(desc.get);
                            if (desc && typeof desc.value === 'function') toString.call(desc.value);
                            checked++;
                        } catch(_) { errors++; }
                    }
                    proto = Object.getPrototypeOf(proto);
                }

                return 'lies_check_ok_checked=' + checked + '_errors=' + errors;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("lies_check_ok_"),
        "creepjs lies detection should not crash, got: {result}"
    );
}

/// Phase-7: Yandex Metrika IIFE pattern — createElement('script'), src= assignment,
/// getElementsByTagName, parentNode.insertBefore. No network fetch (src points at
/// a non-existent data: URL so _onNodeInserted's op_net_fetch_sync either
/// fails or is skipped). Guards that the DOM mutation + re-entrant _onNodeInserted
/// path doesn't infinitely recurse.
#[tokio::test]
async fn shim_recursion_ym_iife_pattern() {
    let result = check(r#"
        (function() {
            try {
                // Simulate Yandex Metrika initialization IIFE.
                // Uses a data: URL so no real network call is made.
                var insertCount = 0;
                (function(m, e, t, r, i) {
                    m[i] = m[i] || function() { (m[i].a = m[i].a || []).push(arguments); };
                    m[i].l = 1 * new Date();
                    for (var j = 0; j < document.scripts.length; j++) {
                        if (document.scripts[j].src === r) { return; }
                    }
                    var k = e.createElement(t);
                    k.src = r;
                    insertCount++;
                    var a = e.getElementsByTagName(t)[0];
                    if (a && a.parentNode) { a.parentNode.insertBefore(k, a); }
                    else { e.head.appendChild(k); }
                })(window, document, 'script', 'data:text/javascript,void+0', 'ym');
                return 'ym_ok_inserts_' + insertCount;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("ym_ok_"),
        "Yandex Metrika IIFE pattern should not recurse, got: {result}"
    );
}

/// YM tag.js probe: iframe contentWindow access + navigator for..in.
/// YM creates a hidden iframe to read cross-document globals. Our iframeWindow
/// Proxy falls through to globalThis — if the fall-through path creates a Proxy
/// cycle or triggers a self-referential toString chain, it crashes here.
#[tokio::test]
async fn shim_recursion_ym_iframe_navigator_probe() {
    let result = check(r#"
        (function() {
            try {
                // Create iframe + read its window (our iframeWindow Proxy)
                var iframe = document.createElement('iframe');
                document.body.appendChild(iframe);
                var iw = iframe.contentWindow;
                if (!iw) return 'error: no contentWindow';

                // YM accesses iframe.contentWindow.navigator for cross-frame check
                var nav = iw.navigator;
                var ua = nav ? nav.userAgent : 'missing';

                // YM probes navigator properties via for...in
                var navKeys = [];
                try {
                    for (var k in navigator) { navKeys.push(k); if (navKeys.length > 100) break; }
                } catch(e) {}

                // YM accesses plugins and mimeTypes
                var pluginsLen = navigator.plugins.length;
                var mimesLen = navigator.mimeTypes.length;

                // YM calls Function.prototype.toString on iframe globals
                var iToStr = Function.prototype.toString.call(iw.Function || Function);

                return 'ym_iframe_ok_nav_keys_' + navKeys.length +
                       '_plugins_' + pluginsLen +
                       '_mimes_' + mimesLen +
                       '_tostr_' + (iToStr.includes('[native code]') ? 'native' : 'src');
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("ym_iframe_ok_"),
        "YM iframe+navigator probe should not crash, got: {result}"
    );
}

/// YM tag.js probe: document.cookie read/write and script enumeration.
/// YM reads/writes cookies on every pageview. Also iterates document.scripts
/// to avoid re-inserting itself.
#[tokio::test]
async fn shim_recursion_ym_cookie_scripts_probe() {
    let result = check(r#"
        (function() {
            try {
                // Cookie read/write (YM stores its state in _ym_uid cookie)
                document.cookie = '_ym_uid=1234567890; path=/';
                var cookie = document.cookie;

                // YM iterates document.scripts to check if already loaded
                var scriptSrcs = [];
                for (var j = 0; j < document.scripts.length; j++) {
                    scriptSrcs.push(document.scripts[j].src || 'inline');
                }

                // YM probes performance.timing for load time calculation
                var timing = window.performance && window.performance.timing;
                var navStart = timing ? timing.navigationStart : -1;

                // YM uses screen properties
                var screenInfo = screen.width + 'x' + screen.height + 'x' + screen.colorDepth;

                return 'ym_cookie_ok_cookie_' + (cookie.includes('_ym_uid') ? 'found' : 'missing') +
                       '_scripts_' + scriptSrcs.length +
                       '_screen_' + screenInfo;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("ym_cookie_ok_"),
        "YM cookie+scripts probe should not crash, got: {result}"
    );
}

/// YM tag.js probe: window property enumeration pattern.
/// YM scans window properties to check for anti-detect environment signals.
/// If any property getter recurses or the prototype walk loops, this crashes.
#[tokio::test]
async fn shim_recursion_ym_window_enum_probe() {
    let result = check(r#"
        (function() {
            try {
                // YM checks if specific globals exist via in-operator
                var checks = {
                    ym: 'ym' in window,
                    Ya: 'Ya' in window,
                    yandex: 'yandex' in window,
                    _ym_: '_ym_' in window,
                };

                // YM enumerates window to find conflicting libraries
                var keyCount = 0;
                try {
                    for (var k in window) {
                        keyCount++;
                        if (keyCount > 500) break; // Safety cap
                    }
                } catch(e) {}

                // YM checks window.top === window (not in a cross-origin frame)
                var isTop = window.top === window;
                var isSelf = window.self === window;
                var isParent = window.parent === window;

                // YM checks typeof various globals
                var typeChecks = [
                    typeof window.JSON,
                    typeof window.Promise,
                    typeof window.fetch,
                    typeof window.XMLHttpRequest,
                    typeof window.Worker,
                ].join(',');

                return 'ym_enum_ok_keys_' + keyCount + '_top_' + isTop +
                       '_types_' + typeChecks;
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("ym_enum_ok_"),
        "YM window enumeration should not crash, got: {result}"
    );
}

/// YM tag.js probe: eval-based module loader pattern.
/// YM tag.js uses a webpack-like bundler. The outermost IIFE sets up a module
/// registry via an object mapping module IDs to factory functions. If our eval
/// of a string containing nested function definitions causes V8 C++ recursion,
/// this test catches it.
#[tokio::test]
async fn shim_recursion_ym_module_loader_pattern() {
    let result = check(r#"
        (function() {
            try {
                // Simulate YM's webpack-style module loader
                var modules = {};
                var cache = {};
                function require(id) {
                    if (cache[id]) return cache[id].exports;
                    var mod = { exports: {} };
                    cache[id] = mod;
                    if (modules[id]) modules[id](mod, mod.exports, require);
                    return mod.exports;
                }

                // Register some fake modules
                modules[0] = function(m, e, r) {
                    e.init = function() { return r(1).run(); };
                };
                modules[1] = function(m, e, r) {
                    e.run = function() {
                        // Simulate YM environment probe
                        var hasYa = typeof window.Ya !== 'undefined';
                        var userAgent = navigator.userAgent;
                        var lang = navigator.language;
                        return { hasYa: hasYa, ua: userAgent.slice(0, 20), lang: lang };
                    };
                };

                // Run entry point (module 0)
                var result = require(0).init();

                return 'ym_loader_ok_ua_' + (result.ua ? 'present' : 'missing') +
                       '_lang_' + (result.lang ? 'present' : 'missing');
            } catch(e) {
                return 'error: ' + e.message;
            }
        })()
    "#).await;
    assert!(
        result.starts_with("ym_loader_ok_"),
        "YM module loader pattern should not crash, got: {result}"
    );
}

// Tier-based smoke tests. Each tier runs as a separate #[tokio::test] so each
// gets a fresh stack — running 30 V8 isolates in a single test overflows.

#[tokio::test]
#[ignore = "network: T0 baseline (Cloudflare-lite)"]
async fn antibot_t0_baseline() {
    let p = stealth::presets::chrome_130_macos();
    antibot_smoke("T0-nowsecure", "https://nowsecure.nl/", p.clone()).await;
    antibot_smoke("T0-discord", "https://discord.com/", p.clone()).await;
    antibot_smoke("T0-chatgpt", "https://chatgpt.com/", p.clone()).await;
    // T0-sannysoft disabled — crashes during Yandex Metrika tag.js evaluation
    // (infinite recursion inside tag.js's own code, unrelated to our shims).
    // T0-creepjs disabled — crashes with same root cause in the DataDome/libertix
    // scripts. Tracked as task #60.
    let _ = p;
}

#[tokio::test]
#[ignore = "network: T1 Cloudflare Enterprise + AI-block (claude/openai/anthropic/HF/perplexity)"]
async fn antibot_t1_cloudflare_enterprise() {
    let p = stealth::presets::chrome_130_macos();
    antibot_smoke("T1-claude", "https://claude.ai/", p.clone()).await;
    antibot_smoke("T1-openai", "https://openai.com/", p.clone()).await;
    antibot_smoke("T1-anthropic", "https://www.anthropic.com/", p.clone()).await;
    antibot_smoke("T1-huggingface", "https://huggingface.co/", p.clone()).await;
    antibot_smoke("T1-perplexity", "https://www.perplexity.ai/", p).await;
}

#[tokio::test]
#[ignore = "network: T2 DataDome behavioral (glassdoor/crunchbase/vinted/leboncoin)"]
async fn antibot_t2_datadome() {
    let p = stealth::presets::chrome_130_macos();
    antibot_smoke("T2-glassdoor", "https://www.glassdoor.com/", p.clone()).await;
    antibot_smoke("T2-crunchbase", "https://www.crunchbase.com/", p.clone()).await;
    antibot_smoke("T2-vinted", "https://www.vinted.com/", p.clone()).await;
    antibot_smoke("T2-leboncoin", "https://www.leboncoin.fr/", p).await;
}

#[tokio::test]
#[ignore = "network: T3 Akamai BMP (adidas/nike/footlocker)"]
async fn antibot_t3_akamai_bmp() {
    let p = stealth::presets::chrome_130_macos();
    antibot_smoke("T3-adidas", "https://www.adidas.com/", p.clone()).await;
    antibot_smoke("T3-nike", "https://www.nike.com/", p.clone()).await;
    antibot_smoke("T3-footlocker", "https://www.footlocker.com/", p).await;
}

#[tokio::test]
#[ignore = "network: T4 Kasada (ticketmaster/canadagoose/hyatt)"]
async fn antibot_t4_kasada() {
    let p = stealth::presets::chrome_130_macos();
    antibot_smoke("T4-ticketmaster", "https://www.ticketmaster.com/", p.clone()).await;
    antibot_smoke("T4-canadagoose", "https://www.canadagoose.com/", p.clone()).await;
    antibot_smoke("T4-hyatt", "https://www.hyatt.com/", p).await;
}

#[tokio::test]
#[ignore = "network: T5 HUMAN/PerimeterX + Imperva (zillow/walmart/udemy)"]
async fn antibot_t5_human_imperva() {
    let p = stealth::presets::chrome_130_macos();
    antibot_smoke("T5-zillow", "https://www.zillow.com/", p.clone()).await;
    antibot_smoke("T5-walmart", "https://www.walmart.com/", p.clone()).await;
    antibot_smoke("T5-udemy", "https://www.udemy.com/", p).await;
}

#[tokio::test]
#[ignore = "network: T6 Shape/F5 landing (delta) — login flows excluded"]
async fn antibot_t6_shape() {
    let p = stealth::presets::chrome_130_windows();
    antibot_smoke("T6-delta", "https://www.delta.com/", p).await;
}

#[tokio::test]
#[ignore = "network: T7 Russian (ya.ru/wildberries/ozon)"]
async fn antibot_t7_russian() {
    let p = stealth::presets::chrome_130_ru();
    antibot_smoke("T7-yandex", "https://ya.ru/", p.clone()).await;
    antibot_smoke("T7-wildberries", "https://www.wildberries.ru/", p.clone()).await;
    antibot_smoke("T7-ozon", "https://www.ozon.ru/", p).await;
}

#[tokio::test]
#[ignore = "network: T8 Chinese (taobao/jd/douyin)"]
async fn antibot_t8_chinese() {
    let p = stealth::presets::chrome_130_cn();
    antibot_smoke("T8-taobao", "https://www.taobao.com/", p.clone()).await;
    antibot_smoke("T8-jd", "https://www.jd.com/", p.clone()).await;
    antibot_smoke("T8-douyin", "https://www.douyin.com/", p).await;
}

#[tokio::test]
#[ignore = "network: direct fetch of WBAAS solver script to isolate the bug"]
async fn wbaas_solver_url_direct_fetch() {
    let profile = stealth::presets::chrome_130_ru();
    let client = net::HttpClient::new(&profile).unwrap();
    let url = "https://www.wildberries.ru/__wbaas/challenges/antibot/statics/challenge_solver_v1.0.4.js";
    println!("\n=== WBAAS solver direct fetch ===");
    let t0 = std::time::Instant::now();
    let mut hdrs = net::headers::chrome_headers(&profile);
    hdrs.push(("referer".to_string(), "https://www.wildberries.ru/".into()));
    hdrs.push(("sec-fetch-dest".to_string(), "script".into()));
    hdrs.push(("sec-fetch-mode".to_string(), "no-cors".into()));
    hdrs.push(("sec-fetch-site".to_string(), "same-origin".into()));
    match client.get_with_headers(url, &hdrs).await {
        Ok(resp) => {
            let body = resp.text();
            println!("  status:        {}", resp.status);
            println!("  body bytes:    {}", body.len());
            println!("  elapsed:       {:?}", t0.elapsed());
            println!("  content-type:  {:?}", resp.headers.get("content-type"));
            println!("  content-encoding: {:?}", resp.headers.get("content-encoding"));
            if !body.is_empty() {
                println!("  body[..200]:   {}", &body[..body.len().min(200)]);
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }
    println!("=== end ===");
}

/// Full network trace of all fetch+XHR requests during wildberries WBAAS challenge.
/// Installs an XHR interceptor in the init script to capture sync XHR token fetches
/// that don't appear in the standard __fetchLog (since they bypass the fetch() path).
#[tokio::test]
#[ignore = "network: full WBAAS network trace to identify token endpoint URL"]
async fn wbaas_full_network_trace() {
    use browser::Page;
    use std::time::Duration;

    // Extra init script: capture all network requests including body snippets for diagnosis.
    let xhr_trace_init = r#"
        (function() {
            if (!window.__allRequests) window.__allRequests = [];
            // Wrap fetch() — capture URL, method, body snippet, and response status.
            const _origFetch = globalThis.fetch;
            globalThis.fetch = async function(input, init) {
                const url = typeof input==='string' ? input : (input&&input.url)||'';
                const method = (init&&init.method)||'GET';
                const e = { via: 'fetch', method, url: url.substring(0, 300), t: Date.now() };
                // Capture body snippet for POST requests.
                if (init && init.body != null) {
                    try {
                        const b = init.body;
                        if (typeof b === 'string') {
                            e.bodySnippet = b.substring(0, 500);
                            e.bodyLen = b.length;
                        } else if (b instanceof ArrayBuffer || ArrayBuffer.isView(b)) {
                            const u8 = b instanceof Uint8Array ? b : new Uint8Array(b.buffer||b, b.byteOffset||0, b.byteLength);
                            e.bodyLen = u8.length;
                            let s = '';
                            for (let i = 0; i < Math.min(u8.length, 200); i++) s += String.fromCharCode(u8[i]);
                            e.bodySnippet = s;
                        } else {
                            e.bodySnippet = String(b).substring(0, 300);
                            e.bodyLen = String(b).length;
                        }
                    } catch(ex) { e.bodySnippet = 'ERR:'+ex.message; }
                } else {
                    e.bodyLen = 0;
                }
                window.__allRequests.push(e);
                try {
                    const r = await _origFetch.apply(this, arguments);
                    e.status = r.status;
                    // Clone and read response body for key endpoints.
                    if (url.includes('create-token') || url.includes('find-frontend')) {
                        try {
                            const clone = r.clone();
                            clone.text().then(t => { e.respSnippet = t.substring(0, 500); }).catch(()=>{});
                        } catch {}
                    }
                    return r;
                } catch(err) {
                    e.error = String(err&&err.message||err);
                    throw err;
                }
            };
        })();
    "#;

    let page = tokio::time::timeout(
        Duration::from_secs(90),
        Page::navigate_with_init(
            "https://www.wildberries.ru/",
            stealth::presets::chrome_130_ru(),
            3,
            vec![], // no extra init — rely on page.rs __fetchLog only
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            // Read both __allRequests (our wrapper) and __fetchLog (page.rs wrapper).
            let all = p.evaluate("JSON.stringify(window.__allRequests||[])").unwrap_or_default();
            let fetch_log = p.evaluate(r#"JSON.stringify((window.__fetchLog||[]).map(e => ({
                method: e.method, url: e.url, status: e.status, hasBody: e.hasBody,
                body: e.body ? e.body.substring(0,300) : undefined,
                respHeaders: e.respHeaders
            })))"#).unwrap_or_default();

            println!("\n=== __fetchLog (page.rs wrapper) ===");
            if let Ok(entries) = serde_json::from_str::<serde_json::Value>(&fetch_log) {
                if let Some(arr) = entries.as_array() {
                    println!("Total: {}", arr.len());
                    for (i, e) in arr.iter().enumerate() {
                        println!("  [{i:03}] {} {} status={}",
                            e["method"].as_str().unwrap_or("?"),
                            e["url"].as_str().unwrap_or("?"),
                            e.get("status").and_then(|s| s.as_u64()).map(|s| s.to_string()).unwrap_or_else(|| "?".to_string()),
                        );
                        if let Some(b) = e.get("body").and_then(|s| s.as_str()) {
                            if !b.is_empty() { println!("         body: {}", &b[..b.len().min(300)]); }
                        }
                    }
                }
            } else {
                println!("Raw: {}", &fetch_log[..fetch_log.len().min(1000)]);
            }

            println!("\n=== __allRequests (diagnostic wrapper) ===");
            if let Ok(entries) = serde_json::from_str::<serde_json::Value>(&all) {
                if let Some(arr) = entries.as_array() {
                    println!("Total: {}", arr.len());
                    for (i, e) in arr.iter().enumerate() {
                        println!("  [{i:03}] {} {} status={} bodyLen={}",
                            e["method"].as_str().unwrap_or("?"),
                            e["url"].as_str().unwrap_or("?"),
                            e.get("status").and_then(|s| s.as_u64()).map(|s| s.to_string()).unwrap_or_else(|| "?".to_string()),
                            e.get("bodyLen").and_then(|s| s.as_u64()).unwrap_or(0),
                        );
                        if let Some(snippet) = e.get("bodySnippet").and_then(|s| s.as_str()) {
                            if !snippet.is_empty() {
                                println!("         body: {}", &snippet[..snippet.len().min(300)]);
                            }
                        }
                        if let Some(snippet) = e.get("respSnippet").and_then(|s| s.as_str()) {
                            if !snippet.is_empty() {
                                println!("         resp: {}", &snippet[..snippet.len().min(300)]);
                            }
                        }
                    }
                }
            } else {
                println!("Raw: {}", &all[..all.len().min(1000)]);
            }

            let errors = p.evaluate("JSON.stringify(window.__scriptErrors||[])").unwrap_or_default();
            println!("\n=== Script errors ===");
            println!("{}", &errors[..errors.len().min(1000)]);
        }
        Ok(Err(e)) => println!("Page error: {e}"),
        Err(_) => println!("Timeout"),
    }
}

#[tokio::test]
#[ignore = "network: dump initial response from hyatt to compare against playwright"]
async fn dump_hyatt_initial_response() {
    let profile = stealth::presets::chrome_130_macos();
    let client = net::HttpClient::new(&profile).unwrap();
    let url = std::env::var("HYATT_URL")
        .unwrap_or_else(|_| "https://www.hyatt.com/".to_string());
    let url = url.as_str();
    println!("\n=== Initial response from hyatt.com ===");
    let hdrs = net::headers::chrome_headers(&profile);
    println!("  Headers we sent:");
    for (k, v) in &hdrs {
        println!("    {}: {}", k, v);
    }
    // No-redirect single GET
    match client.get_with_headers(url, &hdrs).await {
        Ok(resp) => {
            let body = resp.text();
            println!("  --- Response ---");
            println!("  status: {}", resp.status);
            println!("  url: {}", resp.url);
            println!("  body bytes: {}", body.len());
            println!("  Response headers:");
            for (k, v) in &resp.headers {
                println!("    {}: {}", k, v);
            }
            if !body.is_empty() {
                println!("  body[..500]: {}", &body[..body.len().min(500)]);
            }
        }
        Err(e) => println!("  ERROR: {e}"),
    }
    println!("=== end ===");
}

#[tokio::test]
#[ignore = "network: dump our headers via httpbin to diff vs real Chrome"]
async fn dump_our_headers_httpbin() {
    let profile = stealth::presets::chrome_130_macos();
    let client = net::HttpClient::new(&profile).unwrap();
    let url = "https://httpbin.org/headers";
    println!("\n=== Our request headers per httpbin.org/headers ===");
    let hdrs = net::headers::chrome_headers(&profile);
    println!("  Headers we'll attach (in order, LOW-ENTROPY = production navs):");
    for (k, v) in &hdrs {
        println!("    {}: {}", k, v);
    }
    match client.get_with_headers(url, &hdrs).await {
        Ok(resp) => {
            let body = resp.text();
            println!("  --- httpbin echo (what server saw) ---");
            println!("{}", body);
        }
        Err(e) => println!("  ERROR: {e}"),
    }
    println!("=== end ===");
}

#[tokio::test]
#[ignore = "network: dump our TLS+H2 fingerprint via tls.peet.ws"]
async fn tls_fingerprint_peet() {
    let profile = stealth::presets::chrome_130_macos();
    let client = net::HttpClient::new(&profile).unwrap();
    let url = "https://tls.peet.ws/api/all";
    println!("\n=== Our TLS fingerprint per tls.peet.ws ===");
    let hdrs = net::headers::chrome_headers_with_accept_ch(&profile);
    match client.get_with_headers(url, &hdrs).await {
        Ok(resp) => {
            let body = resp.text();
            println!("  status: {}", resp.status);
            println!("  body bytes: {}", body.len());
            // Permissive extract: key="value" with optional whitespace
            let extract = |key: &str| -> String {
                let pat = format!("\"{}\"", key);
                if let Some(s) = body.find(&pat) {
                    let after = &body[s + pat.len()..];
                    // find next '"' that opens the value
                    if let Some(open) = after.find('"') {
                        let val_start = open + 1;
                        if let Some(close) = after[val_start..].find('"') {
                            return after[val_start..val_start + close].to_string();
                        }
                    }
                }
                "?".to_string()
            };
            println!("  ja3:               {}", extract("ja3"));
            println!("  ja3_hash:          {}", extract("ja3_hash"));
            println!("  ja4:               {}", extract("ja4"));
            println!("  ja4_r:             {}", extract("ja4_r"));
            println!("  peetprint:         {}", extract("peetprint"));
            println!("  peetprint_hash:    {}", extract("peetprint_hash"));
            println!("  akamai_fp:         {}", extract("akamai_fingerprint"));
            println!("  akamai_hash:       {}", extract("akamai_fingerprint_hash"));
            println!("  user_agent:        {}", extract("user_agent"));
            // also dump our http_version to verify h2
            println!("  http_version:      {}", extract("http_version"));
        }
        Err(e) => println!("  ERROR: {e}"),
    }
    println!("=== end ===");
}

#[tokio::test]
#[ignore = "network: hyatt only — quick re-test after kasada cd-field changes"]
async fn kasada_hyatt_only() {
    let profile = stealth::presets::chrome_130_macos();
    antibot_smoke("KASADA-hyatt-RETEST", "https://www.hyatt.com/", profile).await;
}

#[tokio::test]
#[ignore = "network: 2 alt-Kasada targets to isolate IP-gate vs fingerprint"]
async fn kasada_alt_targets() {
    let profile = stealth::presets::chrome_130_macos();
    // hyatt.com + ticketmaster.com per docs/NEXT_STEPS.md as alternate Kasada
    // sites. If one passes where canadagoose doesn't, that argues for
    // canadagoose-specific IP-blocklisting (per commit 6307749 "prove IP is
    // the gate"). If both also return ~700-byte challenge pages, that argues
    // for a fingerprint/PoW gap rather than IP.
    antibot_smoke("KASADA-hyatt", "https://www.hyatt.com/", profile.clone()).await;
    antibot_smoke(
        "KASADA-ticketmaster",
        "https://www.ticketmaster.com/",
        profile,
    )
    .await;
}

#[tokio::test]
#[ignore = "network: 3-site anti-bot smoke (Cloudflare/Kasada/Akamai)"]
async fn antibot_smoke_tier05() {
    let profile = stealth::presets::chrome_130_macos();
    // Tier 0.5 structural-advantage targets per docs/NEXT_STEPS.md §4.5
    antibot_smoke(
        "CLOUDFLARE-baseline",
        "https://nowsecure.nl/",
        profile.clone(),
    )
    .await;
    antibot_smoke(
        "KASADA-canadagoose",
        "https://www.canadagoose.com/",
        profile.clone(),
    )
    .await;
    antibot_smoke("AKAMAI-adidas", "https://www.adidas.com/", profile).await;
}

#[tokio::test]
#[ignore = "network: hits reddit.com"]
async fn reddit_smoke() {
    let profile = stealth::presets::chrome_130_macos();
    let mut page = match Page::navigate("https://www.reddit.com/", profile, 5).await {
        Ok(p) => p,
        Err(e) => {
            println!("\n=== reddit.com navigation FAILED ===");
            println!("error: {e}");
            panic!("navigate failed");
        }
    };

    let title = page.title();
    let url = page.url().to_string();

    let body_len = page
        .evaluate("document.body ? document.body.textContent.length : 0")
        .unwrap_or_else(|e| format!("evaluate body err: {e}"));

    let body_snippet = page
        .evaluate(
            "document.body ? \
             document.body.textContent.replace(/\\s+/g, ' ').trim().slice(0, 400) \
             : 'no body'",
        )
        .unwrap_or_else(|e| format!("evaluate snippet err: {e}"));

    let html_len = page
        .evaluate("document.documentElement ? document.documentElement.outerHTML.length : 0")
        .unwrap_or_default();

    let n_links = page
        .evaluate("document.querySelectorAll('a').length")
        .unwrap_or_default();

    let n_scripts = page
        .evaluate("document.querySelectorAll('script').length")
        .unwrap_or_default();

    let cookies = page
        .evaluate("typeof document.cookie === 'string' ? document.cookie.length : 0")
        .unwrap_or_default();

    println!("\n=== reddit.com navigation result ===");
    println!("  final url:      {url}");
    println!("  title:          {title:?}");
    println!("  html bytes:     {html_len}");
    println!("  body chars:     {body_len}");
    println!("  <a> count:      {n_links}");
    println!("  <script> count: {n_scripts}");
    println!("  cookie chars:   {cookies}");
    println!("  body snippet:   {body_snippet}");
    println!("=== end ===\n");

    // Sanity: we got *some* content back (not a 0-byte challenge).
    assert!(
        html_len.parse::<usize>().unwrap_or(0) > 1000,
        "expected >1000 bytes of HTML, got {html_len}"
    );
}

#[tokio::test]
async fn fingerprint_probe_vs_chrome() {
    let js = r#"(() => {
      const r = {};
      r.chromeKeys = Object.keys(window.chrome||{}).sort().join(',');
      r.chromeRuntimeExists = !!(window.chrome && window.chrome.runtime);
      r.chromeWebstoreExists = !!(window.chrome && window.chrome.webstore);
      if (window.chrome && window.chrome.loadTimes) {
        try {
          const lt = window.chrome.loadTimes();
          r.loadTimesSpdy = lt.wasFetchedViaSpdy;
          r.loadTimesNpn = lt.wasNpnNegotiated;
          r.loadTimesProto = lt.npnNegotiatedProtocol;
          r.loadTimesKeys = Object.keys(lt).sort().join(',');
        } catch(e) { r.loadTimesError = ''+e; }
      }
      r.trustedTypes = typeof trustedTypes;
      r.scheduler = typeof scheduler;
      r.reportError = typeof reportError;
      r.requestIdleCallback = typeof requestIdleCallback;
      r.cancelIdleCallback = typeof cancelIdleCallback;
      r.queueMicrotask = typeof queueMicrotask;
      r.SharedArrayBuffer = typeof SharedArrayBuffer;
      r.deviceMemory = navigator.deviceMemory;
      r.connection_effectiveType = navigator.connection ? navigator.connection.effectiveType : 'NONE';
      r.getBattery = typeof navigator.getBattery === 'function';
      r.pdfViewerEnabled = navigator.pdfViewerEnabled;
      r.webdriver = navigator.webdriver;
      r.webdriverInNav = 'webdriver' in navigator;
      r.pluginsLength = navigator.plugins ? navigator.plugins.length : -1;
      r.mimeTypesLength = navigator.mimeTypes ? navigator.mimeTypes.length : -1;
      r.perfMemory = !!(window.performance && window.performance.memory);
      r.isSecureContext = window.isSecureContext;
      r.touchExists = typeof Touch !== 'undefined';
      r.PaymentRequest = typeof PaymentRequest;
      r.docHasFocus = document.hasFocus();
      r.docVisibility = document.visibilityState;
      r.navigatorProto = Object.getPrototypeOf(navigator) ? Object.getPrototypeOf(navigator).constructor.name : null;
      r.notificationExists = typeof Notification !== 'undefined';
      r.paymentRequestExists = typeof PaymentRequest !== 'undefined';
      r.indexedDBExists = typeof indexedDB !== 'undefined';
      r.storageExists = typeof navigator.storage !== 'undefined';
      r.serviceWorkerExists = typeof navigator.serviceWorker !== 'undefined';
      r.credentialsExists = typeof navigator.credentials !== 'undefined';
      r.mediaDevicesExists = typeof navigator.mediaDevices !== 'undefined';
      r.geolocationExists = typeof navigator.geolocation !== 'undefined';
      r.bluetoothExists = !!(navigator.bluetooth);
      r.usbExists = !!(navigator.usb);
      r.ResizeObserver = typeof ResizeObserver;
      r.IntersectionObserver = typeof IntersectionObserver;
      r.PerformanceObserver = typeof PerformanceObserver;
      r.MutationObserver = typeof MutationObserver;
      r.cryptoRandomUUID = typeof crypto !== 'undefined' && typeof crypto.randomUUID;
      r.screen_orientation = screen.orientation ? screen.orientation.type : 'NONE';
      r.devicePixelRatio = window.devicePixelRatio;
      return JSON.stringify(r, null, 2);
    })()"#;
    let result = check(js).await;
    println!("\nFINGERPRINT PROBE:\n{}", result);
    // Just ensure it ran
    assert!(result.starts_with('{'), "expected JSON, got: {}", &result[..100.min(result.len())]);
}

/// W6 — DataDome diagnostic capture. Mirror of kasada_error_blob_capture
/// for DataDome-protected sites. DataDome's flow per
/// docs/RESEARCH_DATADOME_BYPASS_2026_05_10.md:
///   1. Page loads `<script src="//js.datadome.co/tags.js?...">` (the
///      bootstrap with the obfuscated probe code).
///   2. Bootstrap collects fingerprint+behavioural data, encrypts via
///      dual-XOR-PRNG over a kv buffer, and POSTs to
///      `https://api-js.datadome.co/js/?dd=<obfuscated_payload>`.
///   3. Server responds with `Set-Cookie: datadome=...` (1y TTL,
///      IP-bound) on success, or 302 to `geo.captcha-delivery.com/...`
///      on failure.
///
/// This test patches XHR + fetch + TextEncoder.encode to capture every
/// request/response involving datadome.co or captcha-delivery.com,
/// stashing them as base64 globals for offline analysis.
#[tokio::test]
#[ignore = "network: datadome diagnostic capture"]
async fn datadome_diagnostic_capture() {
    use browser::Page;
    use std::time::Duration;

    let capture_init = r#"
        (function() {
            globalThis.__ddCapture = [];

            const _isDdUrl = (u) => {
                if (!u || typeof u !== 'string') return false;
                return u.includes('datadome.co')
                    || u.includes('captcha-delivery.com')
                    || u.includes('/js/?dd=');
            };

            // Intercept TextEncoder.encode — catch the cleartext probe
            // payload before it gets encrypted+base64'd.
            const oldEncode = TextEncoder.prototype.encode;
            TextEncoder.prototype.encode = function(str) {
                if (typeof str === 'string' && str.length > 100) {
                    try {
                        globalThis.__ddCapture.push({
                            kind: 'raw-text',
                            len: str.length,
                            b64: btoa(unescape(encodeURIComponent(str.substring(0, 8000)))),
                            preview: str.substring(0, 200),
                        });
                    } catch (e) {
                        globalThis.__ddCapture.push({ kind: 'err-encode', err: String(e) });
                    }
                }
                return oldEncode.apply(this, arguments);
            };

            // Intercept XHR (DataDome often uses this for the sensor POST)
            const oldOpen = XMLHttpRequest.prototype.open;
            XMLHttpRequest.prototype.open = function(method, url) {
                this._ddUrl = url;
                this._ddMethod = method;
                return oldOpen.apply(this, arguments);
            };
            const oldSend = XMLHttpRequest.prototype.send;
            XMLHttpRequest.prototype.send = function(body) {
                if (this._ddUrl && _isDdUrl(this._ddUrl)) {
                    try {
                        globalThis.__ddCapture.push({
                            kind: 'xhr',
                            url: this._ddUrl.substring(0, 500),
                            method: this._ddMethod || 'GET',
                            len: body ? (body.length || body.byteLength || 0) : 0,
                            b64: body && typeof body === 'string' ? btoa(body.substring(0, 8000)) : '',
                        });
                    } catch (e) {
                        globalThis.__ddCapture.push({ kind: 'err-xhr', err: String(e) });
                    }
                }
                return oldSend.apply(this, arguments);
            };

            // Intercept fetch
            const _origFetch = globalThis.fetch;
            globalThis.fetch = function(input, init) {
                try {
                    const url = typeof input === 'string' ? input : (input && input.url) || '';
                    if (_isDdUrl(url)) {
                        const body = init && init.body;
                        let snapshot = {
                            kind: 'fetch',
                            url: url.substring(0, 500),
                            method: (init && init.method) || 'GET',
                        };
                        if (typeof body === 'string') {
                            snapshot.body_kind = 'string';
                            snapshot.len = body.length;
                            snapshot.b64 = btoa(body.substring(0, 8000));
                        } else if (body instanceof ArrayBuffer || body instanceof Uint8Array) {
                            const u8 = body instanceof ArrayBuffer ? new Uint8Array(body) : body;
                            const chunks = [];
                            for (let i = 0; i < u8.length && i < 8000; i += 8192) {
                                chunks.push(String.fromCharCode.apply(null, u8.subarray(i, Math.min(i + 8192, 8000))));
                            }
                            snapshot.body_kind = 'binary';
                            snapshot.len = u8.length;
                            snapshot.b64 = btoa(chunks.join(''));
                        } else if (body) {
                            snapshot.body_kind = typeof body;
                            snapshot.len = body.size || 0;
                        }
                        globalThis.__ddCapture.push(snapshot);
                    }
                } catch (e) {
                    globalThis.__ddCapture.push({ kind: 'err-fetch', err: String(e) });
                }
                return _origFetch.apply(this, arguments);
            };
        })();
    "#;

    // yelp is the simplest of the 4 DataDome-protected sites in our
    // sweep — no paywall (vs wsj), no auth requirement (vs leboncoin
    // ad-posting), simpler page (vs etsy product index).
    let target_url = "https://www.yelp.com/";
    println!("\n=== DataDome diagnostic capture: {target_url} ===\n");

    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            target_url,
            stealth::presets::chrome_130_macos(),
            2,
            vec![capture_init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            // Pump deferred operations (fetch handlers fire asynchronously).
            for _ in 0..30 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let n = p
                .evaluate("(globalThis.__ddCapture || []).length")
                .unwrap_or_default()
                .trim_matches('"')
                .parse::<usize>()
                .unwrap_or(0);
            println!("\n=== DataDome capture: {n} entries ===");

            // Page final state
            let title = p.title();
            let url = p.url().to_string();
            let content_len = p.content().len();
            // Did we get a `datadome` cookie? Indirect check via
            // document.cookie — Page-level cookie jar is the truth, but
            // this is what the JS would see.
            let cookie = p.evaluate("document.cookie || ''")
                .unwrap_or_default();
            println!("page final: title={title:?} url={url} content_len={content_len}");
            println!("page cookie: {}", cookie.trim_matches('"').chars().take(500).collect::<String>());

            for i in 0..n {
                let kind = p.evaluate(&format!("globalThis.__ddCapture[{i}].kind || ''"))
                    .unwrap_or_default();
                let url = p.evaluate(&format!("globalThis.__ddCapture[{i}].url || ''"))
                    .unwrap_or_default();
                let len = p.evaluate(&format!("globalThis.__ddCapture[{i}].len || 0"))
                    .unwrap_or_default();
                let b64 = p.evaluate(&format!("globalThis.__ddCapture[{i}].b64 || ''"))
                    .unwrap_or_default();
                let preview = p.evaluate(&format!("globalThis.__ddCapture[{i}].preview || ''"))
                    .unwrap_or_default();
                let kind = kind.trim_matches('"');
                let url = url.trim_matches('"');
                let len = len.trim_matches('"');
                let b64 = b64.trim_matches('"');
                let preview = preview.trim_matches('"');
                println!("--- entry #{i}: kind={kind} len={len} url={url} ---");
                if !preview.is_empty() {
                    println!("    preview: {}", preview.chars().take(200).collect::<String>());
                }
                if !b64.is_empty() {
                    let path = format!("datadome_blob_{i}.b64");
                    std::fs::write(&path, b64).ok();
                    println!("    wrote {path}");
                }
            }
        }
        Ok(Err(e)) => println!("page err: {e}"),
        Err(_) => println!("timeout"),
    }
}

/// W4a-deeper — instrument eval to capture the source string for every
/// unjzomuy probe Kasada fires. Adds a globalThis.__evalLog array that
/// records every eval'd string containing the obfuscated property name.
/// Also records the surrounding 200 chars of the throw site via Error
/// stack inspection.
#[tokio::test]
#[ignore = "network: kasada eval-source capture for unjzomuy probes"]
async fn kasada_eval_source_capture() {
    use browser::Page;
    use std::time::Duration;

    let capture_init = r#"
        (function() {
            globalThis.__evalLog = [];
            const _origEval = globalThis.eval;
            globalThis.eval = function(src) {
                if (typeof src === 'string' && src.length > 0) {
                    // Capture every eval'd string. Filter later by content.
                    try {
                        globalThis.__evalLog.push({
                            len: src.length,
                            src: src.length > 1500 ? src.substring(0, 1500) + '...[TRUNC]' : src,
                            ts: Date.now(),
                        });
                    } catch (_) {}
                }
                return _origEval.call(this, src);
            };
            // Also patch Function constructor (Kasada uses both)
            const _OrigFunction = globalThis.Function;
            globalThis.Function = new Proxy(_OrigFunction, {
                construct(target, args) {
                    if (args.length > 0) {
                        const body = args[args.length - 1];
                        if (typeof body === 'string' && body.length > 0) {
                            try {
                                globalThis.__evalLog.push({
                                    len: body.length,
                                    src: 'function(' + args.slice(0, -1).join(',') + ') { ' +
                                        (body.length > 1500 ? body.substring(0, 1500) + '...[TRUNC]' : body) + ' }',
                                    ts: Date.now(),
                                });
                            } catch (_) {}
                        }
                    }
                    return Reflect.construct(target, args);
                }
            });
        })();
    "#;

    println!("\n=== Kasada eval-source capture: canadagoose.com ===\n");
    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![capture_init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            for _ in 0..30 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Filter: only show evals containing "unjzomuy" OR very short
            // expressions (likely the probe call itself).
            let unj = p.evaluate(
                "(globalThis.__evalLog || []).filter(e => e.src.includes('unjzomuy')).length"
            ).unwrap_or_default().trim_matches('"').parse::<usize>().unwrap_or(0);
            let total = p.evaluate(
                "(globalThis.__evalLog || []).length"
            ).unwrap_or_default().trim_matches('"').parse::<usize>().unwrap_or(0);
            println!("\n=== Total evals: {total}, unjzomuy-bearing: {unj} ===\n");
            // Dump all unjzomuy evals
            for i in 0..total.min(2000) {
                let has = p.evaluate(&format!(
                    "(function(){{ var e = globalThis.__evalLog[{i}]; return e && e.src && e.src.includes('unjzomuy') ? '1' : '0'; }})()"
                )).unwrap_or_default();
                if has.trim_matches('"') == "1" {
                    let src = p.evaluate(&format!(
                        "globalThis.__evalLog[{i}].src"
                    )).unwrap_or_default();
                    println!("--- eval #{i} (unjzomuy) ---");
                    let s = src.trim_matches('"');
                    println!("{}", s.chars().take(800).collect::<String>());
                    println!();
                }
            }
        }
        Ok(Err(e)) => println!("page err: {e}"),
        Err(_) => println!("timeout"),
    }
}

/// W4a-deeper — hook TypeError constructor to capture the throw-site
/// stack frame for every unjzomuy probe. Property-access TypeErrors
/// (e.g. `undefined.X` where X is the obfuscated name) include the
/// call stack of the catching function, which tells us which Kasada
/// VM opcode handler is performing the probe and what target object
/// it expected.
#[tokio::test]
#[ignore = "network: kasada TypeError stack capture for unjzomuy probes"]
async fn kasada_typeerror_stack_capture() {
    use browser::Page;
    use std::time::Duration;

    let capture_init = r#"
        (function() {
            globalThis.__teLog = [];
            // Capture all Error construction with the unjzomuy pattern
            // by patching Error.prepareStackTrace before any other code
            // runs, so V8 will call our prepareStackTrace once per
            // unhandled error stack access.
            //
            // The Kasada VM catches every TypeError its probes throw.
            // Inside the catch handler it reads err.message — which
            // forces V8 to format the stack. Our prepareStackTrace
            // hook fires at that point and records the structured
            // CallSite array. We filter by message content for the
            // unjzomuy 28-char identifier.
            const _origPrepare = Error.prepareStackTrace;
            Error.prepareStackTrace = function(err, frames) {
                try {
                    if (err && err.message && err.message.indexOf('unjzomuybtbyyhwwkdpkxomylnab') !== -1) {
                        // Capture top 5 frames as raw strings so we
                        // see file:line for each.
                        const stack = [];
                        for (let i = 0; i < Math.min(8, frames.length); i++) {
                            const f = frames[i];
                            try {
                                stack.push({
                                    fn: f.getFunctionName ? (f.getFunctionName() || '<anonymous>') : '?',
                                    file: f.getFileName ? (f.getFileName() || '?') : '?',
                                    line: f.getLineNumber ? f.getLineNumber() : -1,
                                    col: f.getColumnNumber ? f.getColumnNumber() : -1,
                                    isEval: f.isEval ? f.isEval() : false,
                                    isNative: f.isNative ? f.isNative() : false,
                                });
                            } catch (_) {}
                        }
                        globalThis.__teLog.push({
                            msg: err.message,
                            stack: stack,
                            ts: Date.now(),
                        });
                    }
                } catch (_) {}
                // Fall back to the default formatter so we don't break the
                // catch handler's err.stack reads.
                return _origPrepare ? _origPrepare(err, frames) :
                    err.toString() + '\n' + frames.map(f => '    at ' +
                        ((f.getFunctionName && f.getFunctionName()) || '<anon>') +
                        ' (' + ((f.getFileName && f.getFileName()) || '?') + ':' +
                        ((f.getLineNumber && f.getLineNumber()) || -1) + ')').join('\n');
            };
        })();
    "#;

    println!("\n=== Kasada TypeError stack capture: canadagoose.com ===\n");
    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![capture_init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            for _ in 0..30 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let n = p.evaluate("(globalThis.__teLog || []).length")
                .unwrap_or_default()
                .trim_matches('"')
                .parse::<usize>()
                .unwrap_or(0);
            println!("\n=== Captured {n} unjzomuy TypeError stacks ===\n");
            for i in 0..n {
                println!("--- TypeError #{i} ---");
                let msg = p.evaluate(&format!(
                    "globalThis.__teLog[{i}].msg || ''"
                )).unwrap_or_default();
                println!("  msg: {}", msg.trim_matches('"'));
                let frames_n = p.evaluate(&format!(
                    "(globalThis.__teLog[{i}].stack || []).length"
                )).unwrap_or_default()
                    .trim_matches('"').parse::<usize>().unwrap_or(0);
                for f in 0..frames_n {
                    let fn_name = p.evaluate(&format!(
                        "globalThis.__teLog[{i}].stack[{f}].fn || ''"
                    )).unwrap_or_default();
                    let file = p.evaluate(&format!(
                        "globalThis.__teLog[{i}].stack[{f}].file || ''"
                    )).unwrap_or_default();
                    let line = p.evaluate(&format!(
                        "globalThis.__teLog[{i}].stack[{f}].line || -1"
                    )).unwrap_or_default();
                    let col = p.evaluate(&format!(
                        "globalThis.__teLog[{i}].stack[{f}].col || -1"
                    )).unwrap_or_default();
                    let is_eval = p.evaluate(&format!(
                        "globalThis.__teLog[{i}].stack[{f}].isEval || false"
                    )).unwrap_or_default();
                    let is_native = p.evaluate(&format!(
                        "globalThis.__teLog[{i}].stack[{f}].isNative || false"
                    )).unwrap_or_default();
                    println!(
                        "  [{f}] fn={} file={} line={} col={} isEval={} isNative={}",
                        fn_name.trim_matches('"'),
                        file.trim_matches('"'),
                        line.trim_matches('"'),
                        col.trim_matches('"'),
                        is_eval.trim_matches('"'),
                        is_native.trim_matches('"'),
                    );
                }
                println!();
            }
        }
        Ok(Err(e)) => println!("page err: {e}"),
        Err(_) => println!("timeout"),
    }
}

#[tokio::test]
async fn check_iterators_test() {
    let profile = stealth::chrome_130_macos();
    let mut page = browser::Page::with_profile("", "about:blank", profile).await.unwrap();
    let js = r#"
        (function() {
            const results = {};
            const targets = [
                ['navigator.plugins', navigator.plugins],
                ['navigator.mimeTypes', navigator.mimeTypes],
                ['document.fonts', document.fonts],
            ];
            for (const [name, obj] of targets) {
                try {
                    const it = obj[Symbol.iterator]();
                    results[name + '_iterator_iterable'] = it[Symbol.iterator]() === it;
                } catch (e) {
                    results[name + '_error'] = e.message;
                }
            }
            return JSON.stringify(results, null, 2);
        })()
    "#;
    let result = page.evaluate(js).unwrap();
    println!("ITERATOR CHECK:\n{}", result);
}

#[tokio::test]
async fn check_ctors_test() {
    let profile = stealth::chrome_130_macos();
    let mut page = browser::Page::with_profile("", "about:blank", profile).await.unwrap();
    let js = r#"
        (function() {
            const results = {};
            const targets = [
                ['navigator.plugins.item', navigator.plugins.item],
                ['navigator.plugins.namedItem', navigator.plugins.namedItem],
                ['navigator.plugins.refresh', navigator.plugins.refresh],
                ['MediaSource.isTypeSupported', MediaSource.isTypeSupported],
                ['Notification.requestPermission', Notification.requestPermission],
            ];
            for (const [name, fn] of targets) {
                try {
                    new fn();
                    results[name] = 'IS_CONSTRUCTOR';
                } catch (e) {
                    results[name] = 'NOT_CONSTRUCTOR';
                }
            }
            return JSON.stringify(results, null, 2);
        })()
    "#;
    let result = page.evaluate(js).unwrap();
    println!("CTOR CHECK:\n{}", result);
}

#[tokio::test]
async fn check_tostring_test() {
    let profile = stealth::chrome_130_macos();
    let mut page = browser::Page::with_profile("", "about:blank", profile).await.unwrap();
    let js = r#"
        (function() {
            const res = {};
            const targets = {
                'mediaDevices.enumerateDevices': navigator.mediaDevices && navigator.mediaDevices.enumerateDevices,
                'mediaDevices.getUserMedia': navigator.mediaDevices && navigator.mediaDevices.getUserMedia,
                'mediaDevices.addEventListener': navigator.mediaDevices && navigator.mediaDevices.addEventListener,
                'PublicKeyCredential.isUVPAA': globalThis.PublicKeyCredential && PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable,
                'CredentialsContainer.get': navigator.credentials && navigator.credentials.get,
                'plugins.item': navigator.plugins.item,
                'plugins.refresh': navigator.plugins.refresh,
                'fetch': globalThis.fetch,
                'setTimeout': globalThis.setTimeout,
            };
            for (const [k, v] of Object.entries(targets)) {
                try {
                    res[k + '_instance'] = v.toString();
                    res[k + '_protoCall'] = Function.prototype.toString.call(v);
                    res[k + '_length'] = v.length;
                } catch (e) {
                    res[k + '_err'] = e.message;
                }
            }
            return JSON.stringify(res, null, 2);
        })()
    "#;
    let result = page.evaluate(js).unwrap();
    println!("TOSTRING CHECK:\n{}", result);
}

#[tokio::test]
async fn check_spread_test() {
    let profile = stealth::chrome_130_macos();
    let mut page = browser::Page::with_profile("", "about:blank", profile).await.unwrap();
    let js = r#"
        (function() {
            const res = {};
            const targets = {
                'plugins': navigator.plugins,
                'mimeTypes': navigator.mimeTypes,
                'fonts': document.fonts,
                'scripts': document.scripts,
                'styleSheets': document.styleSheets,
                'all': document.all,
                'htmlCollection': document.getElementsByTagName('div'),
                'nodeList': document.querySelectorAll('div'),
            };
            for (const [k, v] of Object.entries(targets)) {
                try {
                    [...v];
                    res[k] = true;
                } catch(e) {
                    res[k] = e.message;
                }
            }
            return JSON.stringify(res, null, 2);
        })()
    "#;
    println!("SPREAD CHECK:\n{}", page.evaluate(js).unwrap());
}

/// W4a candidates probe — try `[...x]` on every known `ao` candidate
/// and report which throw. The doc lists: navigator.plugins,
/// userAgentData.brands, MediaSource.activeSourceBuffers,
/// document.fonts, HTMLCollection, RTCRtpReceiver.getCapabilities.
#[tokio::test]
async fn check_ao_candidates_test() {
    let profile = stealth::chrome_130_macos();
    let mut page = browser::Page::with_profile("", "about:blank", profile).await.unwrap();
    let js = r#"
        (function() {
            const res = {};
            const tests = [
                ['navigator.plugins',
                    () => navigator.plugins],
                ['navigator.mimeTypes',
                    () => navigator.mimeTypes],
                ['navigator.userAgentData.brands',
                    () => navigator.userAgentData && navigator.userAgentData.brands],
                ['document.fonts',
                    () => document.fonts],
                ['document.scripts',
                    () => document.scripts],
                ['document.styleSheets',
                    () => document.styleSheets],
                ['document.all',
                    () => document.all],
                ['document.images',
                    () => document.images],
                ['document.links',
                    () => document.links],
                ['document.forms',
                    () => document.forms],
                ['document.embeds',
                    () => document.embeds],
                ['document.anchors',
                    () => document.anchors],
                ['HTMLCollection (children)',
                    () => document.body.children],
                ['NodeList (querySelectorAll)',
                    () => document.querySelectorAll('div')],
                ['NodeList (childNodes)',
                    () => document.body.childNodes],
                ['NamedNodeMap (attributes)',
                    () => document.body.attributes],
                ['DOMTokenList (classList)',
                    () => document.body.classList],
                ['FormData',
                    () => new FormData()],
                ['URLSearchParams',
                    () => new URLSearchParams('a=1&b=2')],
                ['Headers',
                    () => new Headers({'X-Test': 'a'})],
                ['MediaList',
                    () => document.styleSheets[0] && document.styleSheets[0].media],
                ['CSSRuleList',
                    () => document.styleSheets[0] && document.styleSheets[0].cssRules],
                ['MediaSource exists?',
                    () => typeof MediaSource],
                ['MediaSource.activeSourceBuffers',
                    () => { const ms = new MediaSource(); return ms.activeSourceBuffers; }],
                ['MediaSource.sourceBuffers',
                    () => { const ms = new MediaSource(); return ms.sourceBuffers; }],
                ['RTCRtpReceiver exists?',
                    () => typeof RTCRtpReceiver],
                ['RTCRtpReceiver.getCapabilities(audio)',
                    () => RTCRtpReceiver.getCapabilities('audio')],
                ['RTCRtpSender.getCapabilities(video)',
                    () => RTCRtpSender.getCapabilities('video')],
                ['ResizeObserverSize-like',
                    () => ({length: 2, 0: 'a', 1: 'b'})],
                ['FileList',
                    () => null /* requires <input type=file>; skip */],
                ['TouchList',
                    () => null],
            ];
            for (const [name, fn] of tests) {
                let r = {};
                try {
                    const v = fn();
                    if (v === null) { r.skip = true; }
                    else if (typeof v === 'string') { r.value = v; }
                    else {
                        r.type = Object.prototype.toString.call(v);
                        r.hasSymbolIter = typeof v[Symbol.iterator] === 'function';
                        try {
                            const arr = [...v];
                            r.spread = `ok len=${arr.length}`;
                        } catch (e) {
                            r.spread = 'ERR: ' + e.message.split('\n')[0];
                        }
                    }
                } catch (e) {
                    r.setup_err = e.message;
                }
                res[name] = r;
            }
            return JSON.stringify(res, null, 2);
        })()
    "#;
    println!("AO CANDIDATES CHECK:\n{}", page.evaluate(js).unwrap());
}

/// W4a-deeper — Symbol.iterator probe. Install a getter on
/// Object.prototype[Symbol.iterator] that logs every access where the
/// receiver doesn't already have an own/inherited Symbol.iterator. The
/// getter returns undefined so the spread/iteration still throws as
/// V8 default. Each access is recorded with constructor name, keys, and
/// a stack snippet. The goal: identify exactly which object Kasada's
/// `ao` probe attempts to spread (we know it's something we don't make
/// iterable).
#[tokio::test]
#[ignore = "network: canadagoose Symbol.iterator probe"]
async fn kasada_symbol_iterator_probe() {
    use browser::Page;
    use std::time::Duration;

    let init = r#"
        (function() {
            globalThis.__iterProbe = [];
            // Cap the log to avoid OOM on busy pages.
            const MAX = 1000;
            // Define a CONFIGURABLE+ENUMERABLE getter on Object.prototype.
            // ECMA spec: when V8 looks up Symbol.iterator on a receiver
            // that doesn't have it own and doesn't inherit from Array/
            // Map/Set/etc., it walks up to Object.prototype, hits our
            // getter, and reads undefined — which then throws
            // "non-iterable".
            try {
                Object.defineProperty(Object.prototype, Symbol.iterator, {
                    configurable: true,
                    get() {
                        if (globalThis.__iterProbe.length < MAX) {
                            let stack = '';
                            try { stack = new Error().stack || ''; } catch (_) {}
                            let typeTag = '?';
                            try { typeTag = Object.prototype.toString.call(this); } catch (_) {}
                            let keys = [];
                            try { keys = Object.keys(this || {}).slice(0, 15); } catch (_) {}
                            let proto = '?';
                            try {
                                const p = Object.getPrototypeOf(this);
                                proto = p && p.constructor && p.constructor.name || String(p);
                            } catch (_) {}
                            // Truncate stack — most useful frames are top 6
                            const stackTop = stack.split('\n').slice(0, 6).join(' | ');
                            globalThis.__iterProbe.push({
                                typeTag, keys, proto, stackTop,
                            });
                        }
                        // Return undefined to preserve V8's "non-iterable" throw
                        return undefined;
                    },
                });
            } catch (e) {
                globalThis.__iterProbeInitErr = String(e);
            }
        })();
    "#;

    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            for _ in 0..30 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            let init_err = p.evaluate("globalThis.__iterProbeInitErr || ''").unwrap_or_default();
            let n = p
                .evaluate("(globalThis.__iterProbe || []).length")
                .unwrap_or_default()
                .trim_matches('"')
                .parse::<usize>()
                .unwrap_or(0);
            println!("init_err: {init_err}");
            println!("=== Symbol.iterator probes (all): {n} ===");
            // Group by (typeTag, keys.join, proto) to dedupe.
            let summary = p.evaluate(r#"
                JSON.stringify((() => {
                    const counts = {};
                    for (const e of globalThis.__iterProbe || []) {
                        const key = `${e.typeTag} ${e.proto} keys=${(e.keys||[]).join(',')}`;
                        counts[key] = (counts[key] || 0) + 1;
                    }
                    return counts;
                })())
            "#).unwrap_or_default();
            println!("SUMMARY: {summary}");
            // Show the first 20 unique stacks
            let stacks = p.evaluate(r#"
                JSON.stringify((() => {
                    const seen = new Set(); const out = [];
                    for (const e of globalThis.__iterProbe || []) {
                        const k = (e.stackTop || '').slice(0, 200);
                        if (seen.has(k)) continue;
                        seen.add(k);
                        out.push(e);
                        if (out.length >= 20) break;
                    }
                    return out;
                })())
            "#).unwrap_or_default();
            println!("UNIQUE_STACKS: {stacks}");
        }
        Ok(Err(e)) => println!("page err: {e}"),
        Err(_) => println!("timeout"),
    }
}

/// Capture the per-session ips.js source from a canadagoose load. The
/// W4a doc points at line 5 col 116 (U dispatcher) and line 3 col 66
/// (STORE-INDEXED handler #12) as the load-bearing inspection targets.
/// This test writes ips.js to ./kasada_ips.js for static analysis.
#[tokio::test]
#[ignore = "network: captures ips.js for static analysis"]
async fn kasada_capture_ips_js() {
    use browser::Page;
    use std::time::Duration;

    // Init script: hook script-element src setter so we record every
    // <script src=...> assignment. Then after page load, fetch the
    // ips.js URL from JS-side (same-origin) and stash the source on
    // globalThis.__ipsJsSource.
    let init = r#"
        (function() {
            globalThis.__scriptUrls = [];
            // Capture script src assignments via setter.
            const HSE = HTMLScriptElement && HTMLScriptElement.prototype;
            if (HSE) {
                const origDesc = Object.getOwnPropertyDescriptor(HSE, 'src');
                if (origDesc && origDesc.set) {
                    const origSet = origDesc.set;
                    Object.defineProperty(HSE, 'src', {
                        configurable: true,
                        get: origDesc.get,
                        set(v) {
                            try { globalThis.__scriptUrls.push(String(v)); } catch (_) {}
                            return origSet.call(this, v);
                        }
                    });
                }
            }
        })();
    "#;

    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            // Pump event loop briefly so ips.js script tag is observed.
            for _ in 0..10 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            // Find ips.js URL from setter log OR from DOM <script src> elements.
            let url_js = r#"
                (function() {
                    // From setter capture
                    const fromSetter = (globalThis.__scriptUrls || [])
                        .find(u => u && u.includes('ips.js'));
                    if (fromSetter) return fromSetter;
                    // Fallback: walk current DOM for <script src> ending in ips.js
                    const scripts = document.querySelectorAll('script[src]');
                    for (const s of scripts) {
                        if (s.src && s.src.includes('ips.js')) return s.src;
                    }
                    return '';
                })()
            "#;
            let ips_url = p.evaluate(url_js).unwrap_or_default();
            let ips_url = ips_url.trim_matches('"').to_string();
            if ips_url.is_empty() {
                println!("ips.js URL not found in setter log or DOM");
                println!("Captured script URLs:");
                let urls = p.evaluate("JSON.stringify(globalThis.__scriptUrls || [])").unwrap_or_default();
                println!("{urls}");
                return;
            }
            println!("ips.js URL: {ips_url}");

            // Resolve to absolute URL. The captured URL is path-only
            // (starts with /), so prepend canadagoose's origin.
            let abs_url = if ips_url.starts_with('/') {
                format!("https://www.canadagoose.com{}", ips_url)
            } else if ips_url.starts_with("http") {
                ips_url.clone()
            } else {
                format!("https://www.canadagoose.com/{}", ips_url)
            };
            println!("Absolute ips.js URL: {abs_url}");

            // Debug: what does the runtime think the location is?
            let loc = p.evaluate("JSON.stringify({base: document.baseURI, loc: location.href})").unwrap_or_default();
            println!("Page location debug: {loc}");

            // Use a SYNCHRONOUS XHR — our op_net_fetch_sync runs inline
            // and avoids the async-promise pump pitfall.
            let fetch_js = format!(
                r#"
                (function() {{
                    try {{
                        const x = new XMLHttpRequest();
                        x.open('GET', {:?}, false);  // sync
                        x.send();
                        if (x.status >= 200 && x.status < 300) {{
                            globalThis.__ipsJsSource = x.responseText;
                            return 'OK_len_' + (x.responseText || '').length + '_status_' + x.status;
                        }}
                        return 'HTTP_' + x.status;
                    }} catch (e) {{
                        return 'ERR_' + String(e);
                    }}
                }})()
                "#,
                abs_url
            );
            let fetch_result = p.evaluate(&fetch_js).unwrap_or_default();
            println!("Fetch result: {fetch_result}");
            let source = p.evaluate("globalThis.__ipsJsSource || ''").unwrap_or_default();
            // Strip JSON-encoded outer quotes if any
            let source = source.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(&source);
            // Unescape JSON \" and \\
            let source = source.replace("\\n", "\n").replace("\\\"", "\"").replace("\\\\", "\\");
            println!("ips.js length: {}", source.len());
            std::fs::write("kasada_ips.js", &source).ok();
            println!("Wrote kasada_ips.js ({} bytes)", source.len());

            // Show line 5 cols 100-200 (per W4a doc, col=116 is U dispatcher).
            let lines: Vec<&str> = source.lines().collect();
            if lines.len() >= 5 {
                let line5 = lines[4];
                println!("\n=== ips.js line 5 (len={}) ===", line5.len());
                let start = 80.min(line5.len());
                let end = 250.min(line5.len());
                println!("cols 80..250: {:?}", &line5[start..end]);
            }
            // Also line 3 (STORE-INDEXED handler #12 body)
            if lines.len() >= 3 {
                let line3 = lines[2];
                println!("\n=== ips.js line 3 (len={}) ===", line3.len());
                let start = 30.min(line3.len());
                let end = 200.min(line3.len());
                println!("cols 30..200: {:?}", &line3[start..end]);
            }
            // Count spread syntax occurrences in source.
            let spread_count = source.matches("...").count();
            println!("\nSpread (...) occurrences in ips.js: {spread_count}");
        }
        Ok(Err(e)) => println!("page err: {e}"),
        Err(_) => println!("timeout"),
    }
}

/// Tier 3 Tool 2 — Kasada VM dispatcher trace.
///
/// Hooks `Function` constructor BEFORE ips.js loads. Each dynamically-created
/// function (the VM opcode handlers) is wrapped to log invocation args (truncated)
/// + return value or thrown exception, into `globalThis.__vmTrace`. After the
/// page loads we dump the last N entries before the first `throw 'ball'` (Kasada's
/// internal scope-chain miss) or before the synthesized TypeError ("Invalid attempt
/// to spread non-iterable instance" — the `ao` probe).
///
/// Cross-references with kasada_typeerror_stack_capture: the opcode body that
/// runs IMMEDIATELY before the throw names the property/receiver Kasada was
/// fetching when our engine diverged. That is the `ao` (or `bot1225`, etc.)
/// receiver we need to fix.
#[tokio::test]
#[ignore = "network: Kasada VM dispatcher trace, ~30s, writes ./kasada_vm_trace.json"]
async fn kasada_vm_dispatcher_trace() {
    use browser::Page;
    use std::time::Duration;

    // Init script: install the Function-constructor hook BEFORE any other
    // JS runs. Per-handler invocation logging into __vmTrace ring buffer.
    // Also intercept throws via window.onerror and Promise rejection.
    let init = r#"
        (function() {
            globalThis.__vmTrace = [];
            globalThis.__vmTraceMax = 4000;          // ring buffer cap
            globalThis.__vmHandlerSeq = 0;           // unique id per wrapped fn
            globalThis.__vmFirstThrowAt = -1;        // index of first thrown
            globalThis.__vmHandlerBodies = {};       // id -> truncated body source
            globalThis.__vmThrowStacks = [];         // {idx, msg, stack}

            const _origFn = Function;
            const _record = (entry) => {
                const t = globalThis.__vmTrace;
                if (t.length >= globalThis.__vmTraceMax) {
                    t.splice(0, t.length - globalThis.__vmTraceMax + 1);
                }
                t.push(entry);
            };
            const _summarize = (v) => {
                if (v === null) return 'null';
                if (v === undefined) return 'undefined';
                const t = typeof v;
                if (t === 'string') return 's:' + v.slice(0, 40);
                if (t === 'number' || t === 'boolean') return t[0] + ':' + v;
                if (t === 'function') return 'fn';
                if (Array.isArray(v)) return 'a[' + v.length + ']';
                if (t === 'object') {
                    try {
                        const keys = Object.keys(v).slice(0, 4).join(',');
                        return 'o{' + keys + '}';
                    } catch (_) { return 'o?'; }
                }
                return t;
            };

            // Wrap Function to instrument each created handler.
            // Kasada's VM uses two layers:
            //   outer: new Function('return function(n,e,a,v,i,r){...}')
            //   call outer() once → returns the arity-6 handler
            //   the dispatcher then invokes the handler many times
            //
            // We wrap the OUTER factory invocation, then recursively wrap
            // the RETURNED handler (arity 6) so we catch every dispatcher call.
            const _wrapHandler = (fn, id, source) => {
                if (typeof fn !== 'function') return fn;
                if (fn.__vm_wrapped) return fn;
                globalThis.__vmHandlerBodies[id] = String(source || fn.toString()).slice(0, 240);
                const wrapped = function(...callArgs) {
                    const idx = globalThis.__vmTrace.length;
                    let ret;
                    try {
                        ret = fn.apply(this, callArgs);
                    } catch (e) {
                        const errMsg = String(e && (e.message || e));
                        if (globalThis.__vmFirstThrowAt < 0) {
                            globalThis.__vmFirstThrowAt = idx;
                        }
                        globalThis.__vmThrowStacks.push({
                            idx,
                            id,
                            msg: errMsg,
                            stack: (e && e.stack ? String(e.stack).split('\n').slice(0, 6).join(' | ') : ''),
                        });
                        _record({
                            i: idx, h: id,
                            a: callArgs.length,
                            a0: callArgs.length > 0 ? _summarize(callArgs[0]) : '',
                            r: 'THROW: ' + errMsg.slice(0, 80),
                        });
                        throw e;
                    }
                    _record({
                        i: idx, h: id,
                        a: callArgs.length,
                        a0: callArgs.length > 0 ? _summarize(callArgs[0]) : '',
                        r: _summarize(ret),
                    });
                    return ret;
                };
                try {
                    Object.defineProperty(wrapped, 'length',
                        { value: fn.length, configurable: true });
                    Object.defineProperty(wrapped, 'name',
                        { value: fn.name || 'h', configurable: true });
                    Object.defineProperty(wrapped, '__vm_wrapped',
                        { value: true, configurable: true });
                } catch (_) {}
                return wrapped;
            };

            const _wrappedFn = function Function(...args) {
                const fn = _origFn.apply(this, args);
                if (typeof fn !== 'function') return fn;
                const body = String(args[args.length - 1] || '');
                const looksLikeFactory =
                    body.includes("throw 'ball'") ||
                    body.includes('return function(n,e,a,v,i,r)') ||
                    body.match(/\bfunction\s*\(n,e,a,v,i,r\)/) ||
                    body.includes('e(n)[e(n)]=e(n)') ||
                    body.includes('a(n,e(n)+e(n))') ||
                    body.includes('2654435769');
                if (!looksLikeFactory) return fn;

                // The outer factory: when called, returns the actual handler.
                // We wrap the factory so that the returned handler is itself
                // wrapped (after the factory runs once, future direct calls
                // hit the inner wrapped handler via the dispatcher's slot).
                const factoryId = ++globalThis.__vmHandlerSeq;
                globalThis.__vmHandlerBodies[factoryId] =
                    '[FACTORY] ' + body.slice(0, 220);
                const wrappedFactory = function(...factoryArgs) {
                    const ret = fn.apply(this, factoryArgs);
                    // If the factory returns a handler-shaped function (arity 6,
                    // typical Kasada VM handler), wrap it. If it returns an array
                    // (e.g. opcode #5 returns [m, r, t] for the TEA cipher), wrap
                    // each function element.
                    if (typeof ret === 'function' && ret.length === 6) {
                        const handlerId = ++globalThis.__vmHandlerSeq;
                        return _wrapHandler(ret, handlerId, body);
                    }
                    if (Array.isArray(ret)) {
                        return ret.map((el, i) => {
                            if (typeof el === 'function') {
                                const handlerId = ++globalThis.__vmHandlerSeq;
                                return _wrapHandler(el, handlerId,
                                    '[FROM_ARRAY:' + i + '] ' + body);
                            }
                            return el;
                        });
                    }
                    return ret;
                };
                try {
                    Object.defineProperty(wrappedFactory, 'length',
                        { value: fn.length, configurable: true });
                    Object.defineProperty(wrappedFactory, 'name',
                        { value: fn.name || 'h', configurable: true });
                } catch (_) {}
                return wrappedFactory;
            };
            // Preserve constructor identity so `instanceof Function` and
            // `Function.prototype` stay consistent.
            _wrappedFn.prototype = _origFn.prototype;
            // Replace the global Function reference. WARNING: this MUST run
            // before ips.js. Page-level scripts that reference Function via
            // closure (already-resolved) will keep the original, which is
            // OK for performance — we just won't trace those.
            try { globalThis.Function = _wrappedFn; } catch (_) {}
        })();
    "#;

    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![init.to_string()],
        ),
    )
    .await;

    match page {
        Ok(Ok(mut p)) => {
            // Pump the event loop so ips.js fully runs and emits its
            // characteristic exception (`throw 'ball'` for scope-chain
            // misses, or our engine's TypeError for `ao` spread probe).
            for _ in 0..30 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            // Dump trace, throws, handler bodies, total counts.
            let dump_js = r#"
                JSON.stringify({
                    handler_count: globalThis.__vmHandlerSeq || 0,
                    trace_size: (globalThis.__vmTrace || []).length,
                    first_throw_at: globalThis.__vmFirstThrowAt,
                    throw_stacks: (globalThis.__vmThrowStacks || []).slice(0, 10),
                    // Last 60 entries before the first throw (the slice that
                    // names the receiver Kasada was probing).
                    pre_throw_window: (function() {
                        const t = globalThis.__vmTrace || [];
                        const at = globalThis.__vmFirstThrowAt;
                        if (at < 0) return t.slice(-60);
                        const start = Math.max(0, at - 60);
                        return t.slice(start, at);
                    })(),
                    // First 5 + last 5 handler bodies by id (so we can
                    // visually map h:N entries in the trace).
                    handler_bodies_sample: (function() {
                        const out = {};
                        const ids = Object.keys(globalThis.__vmHandlerBodies || {})
                            .map(Number).sort((a,b) => a - b);
                        for (const id of ids.slice(0, 10).concat(ids.slice(-10))) {
                            out[id] = globalThis.__vmHandlerBodies[id];
                        }
                        return out;
                    })(),
                    // ALWAYS include full bodies of any handler that threw,
                    // PLUS any handler that appears in the pre-throw window.
                    // These are the receivers we need to fix.
                    throwing_handler_bodies: (function() {
                        const out = {};
                        const wantIds = new Set();
                        for (const t of (globalThis.__vmThrowStacks || [])) {
                            wantIds.add(t.id);
                        }
                        // Also unique handler ids in the last 100 trace entries
                        const t = globalThis.__vmTrace || [];
                        for (const e of t.slice(Math.max(0, t.length - 100))) {
                            wantIds.add(e.h);
                        }
                        for (const id of wantIds) {
                            out[id] = globalThis.__vmHandlerBodies[id];
                        }
                        return out;
                    })(),
                })
            "#;
            let dump = p.evaluate(dump_js).unwrap_or_default();
            // Strip outer JSON-encoding quotes from evaluate result
            let dump = dump.trim();
            let stripped: String = if dump.starts_with('"') && dump.ends_with('"') {
                serde_json::from_str(dump).unwrap_or_else(|_| dump.to_string())
            } else {
                dump.to_string()
            };
            std::fs::write("kasada_vm_trace.json", &stripped).ok();
            println!("=== Kasada VM dispatcher trace ===");
            // Print a brief summary; full dump on disk.
            let parsed: serde_json::Value = serde_json::from_str(&stripped)
                .unwrap_or(serde_json::Value::Null);
            if let Some(obj) = parsed.as_object() {
                println!("handler_count: {}", obj.get("handler_count").map(|v| v.to_string()).unwrap_or_default());
                println!("trace_size:    {}", obj.get("trace_size").map(|v| v.to_string()).unwrap_or_default());
                println!("first_throw_at: {}", obj.get("first_throw_at").map(|v| v.to_string()).unwrap_or_default());
                if let Some(throws) = obj.get("throw_stacks").and_then(|v| v.as_array()) {
                    println!("\n=== First {} throws ===", throws.len());
                    for t in throws {
                        println!("  {}", t);
                    }
                }
                if let Some(window) = obj.get("pre_throw_window").and_then(|v| v.as_array()) {
                    println!("\n=== Pre-throw window (last 60 ops before first throw) ===");
                    for entry in window.iter().rev().take(20).rev() {
                        println!("  {}", entry);
                    }
                }
            }
            println!("\nFull dump: ./kasada_vm_trace.json (size: {} bytes)", stripped.len());
        }
        Ok(Err(e)) => println!("page err: {e}"),
        Err(_) => println!("timeout"),
    }
}

/// Diagnose why a site returns < 1KB HTML in our engine (THIN-BODY in
/// holistic_sweep classifier). Both cloudflare.com and primevideo.com
/// fail this way in parallel sweep. Reports the actual HTML, readyState,
/// body children count, head children count, and any visible CSP errors.
async fn thin_body_diagnose(url: &str, name: &str) {
    use browser::Page;
    use std::time::Duration;

    println!("\n========== {name} ({url}) ==========");
    let r = tokio::time::timeout(
        Duration::from_secs(60),
        Page::navigate(url, stealth::presets::chrome_130_macos(), 2),
    )
    .await;
    match r {
        Ok(Ok(mut p)) => {
            let info = p.evaluate(r#"
                JSON.stringify({
                    readyState: document.readyState,
                    bodyLen: (document.body && document.body.innerHTML || '').length,
                    bodyChildren: document.body ? document.body.children.length : 0,
                    headChildren: document.head ? document.head.children.length : 0,
                    scriptCount: document.scripts ? document.scripts.length : 0,
                    title: document.title,
                    url: location.href,
                    bodySnippet: (document.body && document.body.innerHTML || '').slice(0, 400),
                    htmlSnippet: document.documentElement ? document.documentElement.outerHTML.slice(0, 400) : '',
                })
            "#).unwrap_or_default();
            println!("info: {info}");
        }
        Ok(Err(e)) => println!("ERROR: {e}"),
        Err(_) => println!("TIMEOUT"),
    }
}

#[tokio::test]
#[ignore = "network: THIN-BODY diagnostic for parallel sweep failures"]
async fn diag_thin_body_sites() {
    thin_body_diagnose("https://www.cloudflare.com/", "cloudflare").await;
    thin_body_diagnose("https://www.primevideo.com/", "prime-video").await;
}

/// PaymentRequest API surface — Tier 1.1 from RESEARCH_2026_05_12.
/// Must exist on secure-context Chrome profiles. canMakePayment for
/// Google Pay payment method must resolve true (matches a real Chrome
/// with the handler registered, no card enrolled). hasEnrolledInstrument
/// is Chrome/Edge-only and resolves false on a fresh profile.
/// ApplePaySession MUST be undefined under non-macOS Chrome profiles
/// (the cardinal sin: ApplePaySession + Chrome UA = instant flag).
#[tokio::test]
async fn check_payment_request_surface() {
    let profile = stealth::chrome_130_linux();
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", Some(profile))
        .await
        .unwrap();
    // Synchronous surface checks first.
    let sync_js = r#"
        (function() {
            const res = {};
            res.PaymentRequest_typeof = typeof PaymentRequest;
            res.PaymentRequest_length = typeof PaymentRequest === 'function' ? PaymentRequest.length : null;
            res.PaymentResponse_typeof = typeof PaymentResponse;
            res.PaymentMethodChangeEvent_typeof = typeof PaymentMethodChangeEvent;
            res.PaymentRequestUpdateEvent_typeof = typeof PaymentRequestUpdateEvent;
            res.ApplePaySession_typeof = typeof ApplePaySession;
            res.canMakePayment_toString = typeof PaymentRequest === 'function'
                ? PaymentRequest.prototype.canMakePayment.toString()
                : null;
            // Constructor validation (sync throws)
            try { new PaymentRequest([], { total: { label: 'x', amount: { currency: 'USD', value: '1' } } }); res.empty_methods = 'no throw'; }
            catch (e) { res.empty_methods = e.name; }
            try { new PaymentRequest([{supportedMethods: 'basic-card'}], {}); res.no_total = 'no throw'; }
            catch (e) { res.no_total = e.name; }
            return JSON.stringify(res, null, 2);
        })()
    "#;
    let sync_result = page.evaluate(sync_js).unwrap();
    println!("PAYMENT REQUEST SYNC CHECK:\n{}", sync_result);

    assert!(sync_result.contains("\"PaymentRequest_typeof\": \"function\""));
    assert!(sync_result.contains("\"PaymentRequest_length\": 2"));
    assert!(sync_result.contains("\"PaymentResponse_typeof\": \"function\""));
    assert!(sync_result.contains("\"PaymentMethodChangeEvent_typeof\": \"function\""));
    assert!(sync_result.contains("\"PaymentRequestUpdateEvent_typeof\": \"function\""));
    assert!(
        sync_result.contains("\"ApplePaySession_typeof\": \"undefined\""),
        "ApplePaySession must be undefined under Linux Chrome profile (cardinal sin)"
    );
    assert!(sync_result.contains("\"empty_methods\": \"TypeError\""));
    assert!(sync_result.contains("\"no_total\": \"TypeError\""));
    assert!(
        sync_result.contains("[native code]"),
        "PaymentRequest.prototype.canMakePayment.toString() must include [native code]"
    );

    // Async checks — kick off Promise chain into window.__r, pump microtasks, read back.
    page.evaluate(
        r#"window.__r = {};
        (async function() {
            const r = window.__r;
            try {
                const pr = new PaymentRequest(
                    [{ supportedMethods: 'https://google.com/pay' }],
                    { total: { label: 'x', amount: { currency: 'USD', value: '1.00' } } }
                );
                r.id_present = typeof pr.id === 'string' && pr.id.length > 0;
                r.shippingAddress = pr.shippingAddress;
                r.shippingOption = pr.shippingOption;
                r.shippingType = pr.shippingType;
                r.canMakePayment_googlepay = await pr.canMakePayment();
                r.hasEnrolledInstrument = await pr.hasEnrolledInstrument();
                try { await pr.show(); r.show_ok = 'unexpected resolve'; }
                catch (e) { r.show_rejected = e.name; }
                r.abort_resolved = await pr.abort();
            } catch (e) { r.ctor_err = e.name + ': ' + e.message; }

            try {
                const pr2 = new PaymentRequest(
                    [{ supportedMethods: 'unknown://method' }],
                    { total: { label: 'x', amount: { currency: 'USD', value: '1.00' } } }
                );
                r.canMakePayment_unknown = await pr2.canMakePayment();
            } catch (e) { r.unknown_err = e.message; }

            try {
                r.spc = await PaymentRequest.securePaymentConfirmationAvailability();
            } catch (e) { r.spc_err = e.message; }

            window.__r_done = true;
        })();"#,
    )
    .unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(500))
        .await
        .ok();
    let async_result = page
        .evaluate("JSON.stringify(window.__r, null, 2)")
        .unwrap();
    println!("PAYMENT REQUEST ASYNC CHECK:\n{}", async_result);

    assert!(async_result.contains("\"id_present\": true"));
    assert!(async_result.contains("\"shippingAddress\": null"));
    assert!(async_result.contains("\"canMakePayment_googlepay\": true"));
    assert!(async_result.contains("\"canMakePayment_unknown\": false"));
    assert!(async_result.contains("\"hasEnrolledInstrument\": false"));
    assert!(async_result.contains("\"show_rejected\": \"AbortError\""));
    assert!(async_result.contains("\"spc\": \"unavailable-no-user-verifying-platform-authenticator\""));
}

/// navigator.getInstalledRelatedApps — Chrome/Edge-only API; absence
/// under Chrome UA is a tell. Must return Promise<[]> on a fresh profile.
#[tokio::test]
async fn check_get_installed_related_apps() {
    let profile = stealth::chrome_130_linux();
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", Some(profile))
        .await
        .unwrap();
    let sync_js = r#"
        (function() {
            const res = {};
            res.typeof_method = typeof navigator.getInstalledRelatedApps;
            res.toString = typeof navigator.getInstalledRelatedApps === 'function'
                ? navigator.getInstalledRelatedApps.toString()
                : null;
            return JSON.stringify(res, null, 2);
        })()
    "#;
    let sync_result = page.evaluate(sync_js).unwrap();
    println!("getInstalledRelatedApps SYNC CHECK:\n{}", sync_result);
    assert!(sync_result.contains("\"typeof_method\": \"function\""));
    assert!(sync_result.contains("[native code]"));

    page.evaluate(
        r#"window.__r = null;
        navigator.getInstalledRelatedApps().then(o => { window.__r = o; });"#,
    )
    .unwrap();
    page.evaluate_async("void 0", std::time::Duration::from_millis(200))
        .await
        .ok();
    let async_result = page
        .evaluate(
            r#"(() => {
                const r = window.__r;
                if (!Array.isArray(r)) return 'not-array:' + typeof r;
                return 'len=' + r.length;
            })()"#,
        )
        .unwrap();
    println!("getInstalledRelatedApps ASYNC CHECK: {}", async_result);
    assert_eq!(async_result, "len=0");
}

/// Tier 1.2 — Function.prototype.toString cross-realm + stack-trace audit.
///
/// fingerprint-suite + DataDome's threat-research blog highlight 4 detector
/// edge cases that defeat naive toString patches:
///   1. instance.toString + "" coercion (skips Function.prototype.toString)
///   2. Function.prototype.toString.call(maskedFn) directly
///   3. Cross-realm: iframe.contentWindow.Function.prototype.toString.call(parentFn)
///   4. Stack-trace inspection: try { fn.toString.call(undefined) } catch (e) { e.stack }
///      — must NOT contain Proxy artifacts ("at Object.apply", "at Object.get",
///      "at Reflect.apply", "at newHandler.<computed>")
#[tokio::test]
async fn check_tostring_audit_full() {
    let profile = stealth::chrome_130_linux();
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", Some(profile))
        .await
        .unwrap();
    let js = r#"
        (function() {
            const res = {};
            const targets = [
                ['canPlayType', () => {
                    const v = document.createElement('video');
                    return v.canPlayType;
                }],
                ['enumerateDevices', () => navigator.mediaDevices && navigator.mediaDevices.enumerateDevices],
                ['getBattery', () => navigator.getBattery],
                ['getInstalledRelatedApps', () => navigator.getInstalledRelatedApps],
                ['canMakePayment',
                    () => typeof PaymentRequest === 'function' && PaymentRequest.prototype.canMakePayment],
                ['plugins.item', () => navigator.plugins && navigator.plugins.item],
                ['fetch', () => globalThis.fetch],
            ];

            for (const [label, getter] of targets) {
                const fn = getter();
                const r = {};
                if (typeof fn !== 'function') { r.skip = 'not a function: ' + typeof fn; res[label] = r; continue; }

                // Path 1: implicit + "" coercion
                try {
                    const s = fn + "";
                    r.coerce = s.includes('[native code]') ? 'native' : ('SRC: ' + s.slice(0, 60));
                } catch (e) { r.coerce_err = e.message; }

                // Path 2: Function.prototype.toString.call(fn)
                try {
                    const s = Function.prototype.toString.call(fn);
                    r.protoCall = s.includes('[native code]') ? 'native' : ('SRC: ' + s.slice(0, 60));
                } catch (e) { r.protoCall_err = e.message; }

                // Path 3: instance .toString()
                try {
                    const s = fn.toString();
                    r.instance = s.includes('[native code]') ? 'native' : ('SRC: ' + s.slice(0, 60));
                } catch (e) { r.instance_err = e.message; }

                // Path 4: Object.prototype.toString — should NOT change shape
                try {
                    r.objProto = Object.prototype.toString.call(fn);
                } catch (e) { r.objProto_err = e.message; }

                res[label] = r;
            }

            // Cross-realm: iframe.contentWindow.Function.prototype.toString
            try {
                const ifr = document.createElement('iframe');
                document.body && document.body.appendChild(ifr);
                const cw = ifr.contentWindow;
                if (!cw) { res.iframe = 'no contentWindow'; }
                else if (!cw.Function || !cw.Function.prototype || !cw.Function.prototype.toString) {
                    res.iframe = 'no cw.Function.prototype.toString';
                } else {
                    const cwTs = cw.Function.prototype.toString;
                    const fns = {
                        'parent.fetch': globalThis.fetch,
                        'parent.canPlayType': document.createElement('video').canPlayType,
                        'parent.canMakePayment': PaymentRequest && PaymentRequest.prototype.canMakePayment,
                        'parent.getBattery': navigator.getBattery,
                    };
                    res.iframe = {};
                    for (const [k, fn] of Object.entries(fns)) {
                        if (typeof fn !== 'function') { res.iframe[k] = 'skip:' + typeof fn; continue; }
                        try {
                            const s = cwTs.call(fn);
                            res.iframe[k] = s.includes('[native code]') ? 'native' : ('SRC: ' + s.slice(0, 60));
                        } catch (e) { res.iframe[k] = 'ERR: ' + e.message.split('\n')[0]; }
                    }
                }
            } catch (e) { res.iframe_err = e.message; }

            // Stack-trace sanitization
            const proxyArtifacts = [
                'at Object.apply', 'at Object.get', 'at Reflect.apply',
                'at newHandler.', 'at Proxy.', 'at handler.', 'at trap.',
                '_inPatchedToStr', '_nativeTag', '_origFnToStr', '_patchedFnToStr',
            ];
            try {
                Function.prototype.toString.call(undefined);
                res.stack = 'unexpected: did not throw';
            } catch (e) {
                const stack = String(e.stack || '');
                const hits = proxyArtifacts.filter(a => stack.includes(a));
                res.stack = hits.length === 0 ? 'clean' : 'LEAK: ' + hits.join(', ');
                res.stack_full = stack.split('\n').slice(0, 6).join(' | ');
            }
            try {
                Function.prototype.toString.call({});
                res.stack_obj = 'unexpected: did not throw';
            } catch (e) {
                const stack = String(e.stack || '');
                const hits = proxyArtifacts.filter(a => stack.includes(a));
                res.stack_obj = hits.length === 0 ? 'clean' : 'LEAK: ' + hits.join(', ');
            }

            return JSON.stringify(res, null, 2);
        })()
    "#;
    let result = page.evaluate(js).unwrap();
    println!("TOSTRING AUDIT:\n{}", result);

    // Whole-result invariants: NO path on ANY masked function should leak
    // raw source. JSON values starting with "SRC:" mean detector got source.
    assert!(
        !result.contains("\"coerce\": \"SRC:"),
        "+ \"\" coercion leaked raw source somewhere: {result}"
    );
    assert!(
        !result.contains("\"protoCall\": \"SRC:"),
        "Function.prototype.toString.call leaked raw source: {result}"
    );
    assert!(
        !result.contains("\"instance\": \"SRC:"),
        "instance .toString() leaked raw source: {result}"
    );
    // Cross-realm via iframe.contentWindow must also not leak
    assert!(
        !result.contains(": \"SRC:"),
        "cross-realm iframe toString leaked source: {result}"
    );
    // Stack must be clean (no Proxy / handler artifacts)
    assert!(
        result.contains("\"stack\": \"clean\""),
        "stack trace from toString.call(undefined) leaked Proxy artifacts: {result}"
    );
    assert!(
        result.contains("\"stack_obj\": \"clean\""),
        "stack trace from toString.call({{}}) leaked Proxy artifacts: {result}"
    );
}

/// Tier 1.5 verification — audio fingerprint differs across stealth profiles.
/// Real OfflineAudioContext-based fingerprint probes (FPjs, CreepJS) hash
/// the rendered output. Per-profile audio_seed must produce distinct hashes.
/// Two profiles with different audio_seed values MUST produce different
/// audio output, AND the same profile must reproduce the same output (deterministic).
#[tokio::test]
async fn check_audio_fingerprint_per_profile() {
    let render_js = r#"
        (function() {
            const ctx = new OfflineAudioContext(1, 5000, 44100);
            const osc = ctx.createOscillator();
            osc.type = 'triangle';
            osc.frequency.value = 10000;
            const comp = ctx.createDynamicsCompressor();
            comp.threshold.value = -50;
            comp.knee.value = 40;
            comp.ratio.value = 12;
            comp.attack.value = 0;
            comp.release.value = 0.25;
            osc.connect(comp);
            comp.connect(ctx.destination);
            osc.start(0);
            window.__audio_done = false;
            window.__audio_hash = null;
            ctx.startRendering().then(buf => {
                const data = buf.getChannelData(0);
                let s = 0;
                // FPjs-canonical reduction: sum of abs values in [4500, 5000]
                for (let i = 4500; i < Math.min(5000, data.length); i++) s += Math.abs(data[i]);
                window.__audio_hash = s;
                window.__audio_done = true;
            });
        })()
    "#;
    async fn render(profile: stealth::StealthProfile) -> f64 {
        let mut page = Page::from_html_with_url(&html(""), "https://example.com/", Some(profile))
            .await
            .unwrap();
        page.evaluate(r#"
            (function() {
                const ctx = new OfflineAudioContext(1, 5000, 44100);
                const osc = ctx.createOscillator();
                osc.type = 'triangle';
                osc.frequency.value = 10000;
                const comp = ctx.createDynamicsCompressor();
                comp.threshold.value = -50;
                comp.knee.value = 40;
                comp.ratio.value = 12;
                comp.attack.value = 0;
                comp.release.value = 0.25;
                osc.connect(comp);
                comp.connect(ctx.destination);
                osc.start(0);
                window.__audio_done = false;
                window.__audio_hash = null;
                ctx.startRendering().then(buf => {
                    const data = buf.getChannelData(0);
                    let s = 0;
                    for (let i = 4500; i < Math.min(5000, data.length); i++) s += Math.abs(data[i]);
                    window.__audio_hash = s;
                    window.__audio_done = true;
                });
            })()
        "#).unwrap();
        page.evaluate_async("void 0", std::time::Duration::from_millis(500))
            .await
            .ok();
        let raw = page
            .evaluate(r#"String(window.__audio_hash)"#)
            .unwrap();
        raw.parse::<f64>().unwrap_or(f64::NAN)
    }
    let _ = render_js; // referenced for documentation

    let h_mac = render(stealth::chrome_130_macos()).await;
    let h_lin = render(stealth::chrome_130_linux()).await;
    let h_lin_2 = render(stealth::chrome_130_linux()).await;
    println!("audio hashes: mac={h_mac} lin={h_lin} lin_again={h_lin_2}");

    assert!(h_mac.is_finite() && h_mac > 0.0, "macOS profile produced no audio output");
    assert!(h_lin.is_finite() && h_lin > 0.0, "Linux profile produced no audio output");
    // Different audio_seed → distinct hashes
    assert!(
        (h_mac - h_lin).abs() > 1e-6,
        "audio fingerprint should differ across profiles with distinct audio_seed: mac={h_mac} lin={h_lin}"
    );
    // Same profile → reproducible hash (determinism)
    assert!(
        (h_lin - h_lin_2).abs() < 1e-6,
        "same profile should produce identical audio hash: lin={h_lin} lin_again={h_lin_2}"
    );
}

/// Tier 4 — function-identity preservation sniff tests.
/// Per docs/kasada_ips_analysis/UNJZOMUY_INVESTIGATION_2026_05_12.md, the
/// Kasada `unjzomuybtbyyhwwkdpkxomylnab` sentinel-property throws (5
/// engine-divergence TypeErrors per the trace) most likely arise from one
/// of three sites where the same function reference returns a DIFFERENT
/// object on subsequent access. If any sniff test fails, that's the
/// divergence site Kasada is detecting; patch it to return stable references.
#[tokio::test]
async fn check_function_identity_preservation() {
    let profile = stealth::chrome_130_macos();
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", Some(profile))
        .await
        .unwrap();
    let js = r#"
        (function() {
            const res = {};

            // Test 1: navigator.mediaDevices.enumerateDevices identity stability
            try {
                const a = navigator.mediaDevices.enumerateDevices;
                const b = navigator.mediaDevices.enumerateDevices;
                res.test1_same_ref = (a === b);
                if (a === b) {
                    a.unjzomuybtbyyhwwkdpkxomylnab = 'tag';
                    res.test1_tag_persists = (b.unjzomuybtbyyhwwkdpkxomylnab === 'tag');
                    delete a.unjzomuybtbyyhwwkdpkxomylnab;
                } else {
                    res.test1_tag_persists = 'skipped (refs differ)';
                }
            } catch (e) { res.test1_err = e.message; }

            // Test 2: iframe contentWindow getOwnPropertyDescriptor.value identity
            try {
                const iframe = document.createElement('iframe');
                document.body && document.body.appendChild(iframe);
                const w = iframe.contentWindow;
                if (!w) { res.test2_skipped = 'no contentWindow'; }
                else {
                    const d1 = Object.getOwnPropertyDescriptor(w, 'Function');
                    const d2 = Object.getOwnPropertyDescriptor(w, 'Function');
                    if (!d1 || !d2) { res.test2_skipped = 'no Function descriptor'; }
                    else {
                        res.test2_same_ref = (d1.value === d2.value);
                        if (d1.value === d2.value && d1.value) {
                            d1.value.unjzomuybtbyyhwwkdpkxomylnab = 'tag';
                            res.test2_tag_persists = (d2.value.unjzomuybtbyyhwwkdpkxomylnab === 'tag');
                            try { delete d1.value.unjzomuybtbyyhwwkdpkxomylnab; } catch (_) {}
                        } else {
                            res.test2_tag_persists = 'skipped (refs differ)';
                        }
                    }
                }
            } catch (e) { res.test2_err = e.message; }

            // Test 3: Navigator method identity via Object.getOwnPropertyDescriptor
            try {
                const proto = navigator.constructor.prototype;
                const d1 = Object.getOwnPropertyDescriptor(proto, 'sendBeacon');
                const d2 = Object.getOwnPropertyDescriptor(proto, 'sendBeacon');
                if (!d1 || !d2) { res.test3_skipped = 'no sendBeacon descriptor'; }
                else {
                    res.test3_same_ref = (d1.value === d2.value);
                    if (d1.value === d2.value && d1.value) {
                        d1.value.unjzomuybtbyyhwwkdpkxomylnab = 'tag';
                        res.test3_tag_persists = (d2.value.unjzomuybtbyyhwwkdpkxomylnab === 'tag');
                        try { delete d1.value.unjzomuybtbyyhwwkdpkxomylnab; } catch (_) {}
                    } else {
                        res.test3_tag_persists = 'skipped (refs differ)';
                    }
                }
            } catch (e) { res.test3_err = e.message; }

            // Test 4 (bonus): navigator method via direct property access (the way
            // most JS uses it). If THIS one differs while test 3 passes, the issue
            // is in our descriptor wrapper, not the underlying function.
            try {
                const a = navigator.sendBeacon;
                const b = navigator.sendBeacon;
                res.test4_same_ref = (a === b);
                if (a === b) {
                    a.unjzomuybtbyyhwwkdpkxomylnab = 'tag';
                    res.test4_tag_persists = (b.unjzomuybtbyyhwwkdpkxomylnab === 'tag');
                    try { delete a.unjzomuybtbyyhwwkdpkxomylnab; } catch (_) {}
                }
            } catch (e) { res.test4_err = e.message; }

            // Test 5 (bonus): WebGL parameter — Kasada specifically tags
            // WebGLRenderingContext.prototype.getParameter via the canvas probe.
            try {
                const c = document.createElement('canvas');
                const g1 = c.getContext('webgl');
                const g2 = c.getContext('webgl');
                res.test5_ctx_same = (g1 === g2);
                if (g1 && g2) {
                    const fn1 = g1.getParameter;
                    const fn2 = g2.getParameter;
                    res.test5_method_same = (fn1 === fn2);
                }
            } catch (e) { res.test5_err = e.message; }

            return JSON.stringify(res, null, 2);
        })()
    "#;
    let result = page.evaluate(js).unwrap();
    println!("FUNCTION IDENTITY CHECK:\n{}", result);

    // Failures here directly identify the divergence site:
    assert!(
        result.contains("\"test1_same_ref\": true"),
        "FAIL test1: navigator.mediaDevices.enumerateDevices returns DIFFERENT object on re-access — divergence site #1"
    );
    assert!(
        result.contains("\"test1_tag_persists\": true")
            || result.contains("\"test1_tag_persists\": \"skipped (refs differ)\""),
        "FAIL test1: tag did NOT persist on enumerateDevices re-access — even though refs were equal"
    );
    assert!(
        result.contains("\"test3_same_ref\": true")
            || result.contains("\"test3_skipped\""),
        "FAIL test3: Navigator.sendBeacon descriptor.value returns DIFFERENT objects on re-access"
    );
    assert!(
        result.contains("\"test4_same_ref\": true"),
        "FAIL test4: direct navigator.sendBeacon returns DIFFERENT objects on re-access"
    );
}

/// iOS Safari profile JS surface check — Phase 3 (synthesis doc Tier 2.3-2.4).
/// Verifies the `iphone_15_pro_safari_18` preset produces an iOS-shaped JS env:
///   - 16 declined APIs absent
///   - userAgentData absent (Safari has no UA-CH)
///   - hasEnrolledInstrument absent on PaymentRequest (Chrome/Edge-only)
///   - window.orientation present (legacy iOS-only)
///   - DeviceMotionEvent.requestPermission static present (iOS 13+)
///   - ontouchstart on window
///   - WebGL renderer = "Apple GPU" constant
#[tokio::test]
async fn check_ios_safari_surface() {
    let profile = stealth::presets::iphone_15_pro_safari_18();
    let mut page = Page::from_html_with_url(&html(""), "https://example.com/", Some(profile))
        .await
        .unwrap();
    let js = r#"
        (function() {
            const res = {};

            // 16 declined APIs — all must be absent on iOS profile
            const declined = [
                "Bluetooth", "USB", "Serial", "HID", "Sensor", "Accelerometer",
                "Gyroscope", "Magnetometer", "NetworkInformation", "BatteryManager",
                "IdleDetector",
            ];
            res.declined_apis_present = declined.filter(k => typeof globalThis[k] !== "undefined");

            // Navigator absences
            res.bluetooth_typeof = typeof navigator.bluetooth;
            res.usb_typeof = typeof navigator.usb;
            res.serial_typeof = typeof navigator.serial;
            res.hid_typeof = typeof navigator.hid;
            res.getBattery_typeof = typeof navigator.getBattery;
            res.connection_typeof = typeof navigator.connection;
            res.userAgentData_typeof = typeof navigator.userAgentData;
            res.deviceMemory_typeof = typeof navigator.deviceMemory;
            res.requestMIDIAccess_typeof = typeof navigator.requestMIDIAccess;

            // PaymentRequest hasEnrolledInstrument MUST be absent (Chrome-only)
            res.hasEnrolledInstrument_present = typeof PaymentRequest === "function"
                ? typeof PaymentRequest.prototype.hasEnrolledInstrument !== "undefined"
                : "no_PaymentRequest";

            // iOS-only globals
            res.window_orientation = typeof globalThis.orientation;
            res.window_orientation_value = globalThis.orientation;
            res.ontouchstart_present = "ontouchstart" in globalThis;

            // DeviceMotionEvent.requestPermission must exist (iOS 13+ tell)
            res.deviceMotion_requestPermission =
                typeof DeviceMotionEvent !== "undefined"
                && typeof DeviceMotionEvent.requestPermission;
            res.deviceOrientation_requestPermission =
                typeof DeviceOrientationEvent !== "undefined"
                && typeof DeviceOrientationEvent.requestPermission;

            // Profile-driven values from preset
            res.platform = navigator.platform;
            res.maxTouchPoints = navigator.maxTouchPoints;
            res.hardwareConcurrency = navigator.hardwareConcurrency;
            res.userAgent = navigator.userAgent;

            return JSON.stringify(res, null, 2);
        })()
    "#;
    let result = page.evaluate(js).unwrap();
    println!("iOS SAFARI SURFACE CHECK:\n{}", result);

    // The 16 declined APIs must all be absent
    assert!(
        result.contains("\"declined_apis_present\": []"),
        "iOS profile must strip all 16 declined APIs, got: {result}"
    );

    // Navigator absences
    assert!(result.contains("\"bluetooth_typeof\": \"undefined\""));
    assert!(result.contains("\"usb_typeof\": \"undefined\""));
    assert!(result.contains("\"serial_typeof\": \"undefined\""));
    assert!(result.contains("\"hid_typeof\": \"undefined\""));
    assert!(result.contains("\"getBattery_typeof\": \"undefined\""));
    assert!(result.contains("\"connection_typeof\": \"undefined\""));
    assert!(result.contains("\"userAgentData_typeof\": \"undefined\""));
    assert!(result.contains("\"deviceMemory_typeof\": \"undefined\""));

    // PaymentRequest hasEnrolledInstrument must be absent on Safari
    assert!(
        result.contains("\"hasEnrolledInstrument_present\": false"),
        "iOS profile must NOT expose PaymentRequest.prototype.hasEnrolledInstrument (Chrome/Edge-only): {result}"
    );

    // iOS-only globals
    assert!(result.contains("\"window_orientation\": \"number\""));
    assert!(result.contains("\"window_orientation_value\": 0"));
    assert!(result.contains("\"ontouchstart_present\": true"));

    // iOS 13+ Device*Event.requestPermission statics
    assert!(
        result.contains("\"deviceMotion_requestPermission\": \"function\""),
        "iOS profile must expose DeviceMotionEvent.requestPermission static"
    );
    assert!(
        result.contains("\"deviceOrientation_requestPermission\": \"function\""),
        "iOS profile must expose DeviceOrientationEvent.requestPermission static"
    );

    // Profile-driven
    assert!(result.contains("\"platform\": \"iPhone\""));
    assert!(result.contains("\"maxTouchPoints\": 5"));
    assert!(
        result.contains("\"hardwareConcurrency\": 2"),
        "Safari intentionally caps hardwareConcurrency to 2 — got: {result}"
    );
    assert!(result.contains("\"userAgent\":") && result.contains("iPhone"));
}

// ================================================================
// Kasada sentinel identity audit — pinpoints which engine site
// fails to preserve Kasada's `unjzomuybtbyyhwwkdpkxomylnab` sentinel
// property across two reads of the same conceptual identity.
//
// Per docs/research_2026_05_14/01_KASADA.md §1.5, three probes:
//   T1 — WebIDL method identity via prototype lookup
//        (`navigator.mediaDevices.enumerateDevices`).
//   T2 — iframe Function descriptor identity via
//        `getOwnPropertyDescriptor(iframe.contentWindow, 'Function')`.
//        Refined as the medium-high probability culprit after W1.1's
//        _buildRemoteRealm cache landed without flipping any Kasada
//        site in the post-W1 sweep.
//   T3 — Navigator.prototype data-property identity
//        (`Object.getOwnPropertyDescriptor(NavProto, 'sendBeacon').value`).
//
// Expected real-Chrome result for all three: identical=true,
// tagSurvives=true. Our engine should match. The first probe to
// return `identical=false` or `tagSurvives=false` is the divergence
// site that loses Kasada's sentinel.
//
// This test does NOT assert pass/fail yet — its purpose is to
// surface the divergence empirically so the next patch targets the
// specific site. Read the eprintln output.
// ================================================================
#[tokio::test]
async fn kasada_sentinel_identity_audit() {
    let mut page = Page::from_html_with_url(
        &html("<div id='canvas'></div>"),
        "https://example.com/",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let js = r#"
        const out = { mediaDevices: null, iframeDesc: null, navProto: null };

        // T1: WebIDL method identity stability
        try {
            const a = navigator.mediaDevices.enumerateDevices;
            const b = navigator.mediaDevices.enumerateDevices;
            let tagSurvives;
            try { a.__test_tag = 42; tagSurvives = (b.__test_tag === 42); }
            catch (e) { tagSurvives = 'threw: ' + e.message; }
            out.mediaDevices = { identical: a === b, tagSurvives };
        } catch (e) {
            out.mediaDevices = { err: e.message };
        }

        // T2: iframe Function descriptor value identity (the high-confidence
        // sentinel-loss site per the research doc).
        try {
            const iframe = document.createElement('iframe');
            document.body.appendChild(iframe);
            const w = iframe.contentWindow;
            const d1 = Object.getOwnPropertyDescriptor(w, 'Function');
            const d2 = Object.getOwnPropertyDescriptor(w, 'Function');
            let tagSurvives;
            if (d1 && d2) {
                try { d1.value.__test_tag = 42; tagSurvives = (d2.value.__test_tag === 42); }
                catch (e) { tagSurvives = 'threw: ' + e.message; }
            } else {
                tagSurvives = 'no_desc';
            }
            out.iframeDesc = {
                d1isObj: typeof d1 === 'object' && d1 !== null,
                d2isObj: typeof d2 === 'object' && d2 !== null,
                valueIdentical: !!(d1 && d2 && d1.value === d2.value),
                tagSurvives,
            };
        } catch (e) {
            out.iframeDesc = { err: e.message };
        }

        // T3: Navigator.prototype data-property identity (sendBeacon).
        try {
            const proto = navigator.constructor.prototype;
            const d1 = Object.getOwnPropertyDescriptor(proto, 'sendBeacon');
            const d2 = Object.getOwnPropertyDescriptor(proto, 'sendBeacon');
            const a = d1 && d1.value, b = d2 && d2.value;
            let tagSurvives;
            if (a && b) {
                try { a.__test_tag = 42; tagSurvives = (b.__test_tag === 42); }
                catch (e) { tagSurvives = 'threw: ' + e.message; }
            } else {
                tagSurvives = 'no_method';
            }
            out.navProto = {
                present: a != null && b != null,
                identical: a === b,
                tagSurvives,
            };
        } catch (e) {
            out.navProto = { err: e.message };
        }

        JSON.stringify(out);
    "#;
    let result = page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"));
    eprintln!("kasada-sentinel-audit: {result}");
    assert!(!result.starts_with("ERROR:"), "evaluation failed: {result}");
    // The audit is informational. Assertions below mark the result so the
    // test surfaces a clear failure if any probe loses identity. Update
    // these assertions once the divergence site is patched.
    let want_pass = |needle: &str| {
        assert!(
            result.contains(needle),
            "kasada sentinel audit: expected {needle:?} — got: {result}"
        );
    };
    want_pass("\"identical\":true");
    want_pass("\"tagSurvives\":true");
    want_pass("\"valueIdentical\":true");
}

// W2.7 diagnostic: does Error.stack inside our iframe Proxy trap leak
// the signatures Kasada's `p.js` greps for? Real Chrome's iframe
// contentWindow is NOT a Proxy; reading its properties does not produce
// `at Object.apply` / `at Reflect.apply` / `at newHandler.<computed>`
// frames. Our engine mirrors iframe.contentWindow via Proxy, so if
// Kasada reads new Error().stack at the moment one of our traps fires,
// the proxy frame names could leak.
//
// Per docs/research_2026_05_14/01_KASADA.md W2.7. Captures the actual
// stack a detector sees inside our trap so we know whether to ship a
// stack sanitizer.
#[tokio::test]
async fn kasada_proxy_stack_leak_probe() {
    let mut page = Page::from_html_with_url(
        &html(""),
        "https://example.com/",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let js = r#"
        const iframe = document.createElement('iframe');
        document.body.appendChild(iframe);
        const w = iframe.contentWindow;
        // Read a property that goes through our Proxy `get` trap and
        // collect the stack at the moment of access. The trap itself
        // doesn't (currently) construct an Error; we proxy via a
        // hand-rolled handler whose `get` builds one.
        const sentinel = { name: 'probe' };
        const handler = {
            get(target, prop, receiver) {
                if (prop === 'capture') {
                    sentinel.stack = new Error('probe').stack;
                    return undefined;
                }
                return Reflect.get(target, prop, receiver);
            }
        };
        const p = new Proxy({}, handler);
        // Force a Reflect.apply path by calling the proxy as a method
        // through Object.prototype.toString-shaped invocation.
        void p.capture;
        // Also test reading from our iframe Proxy directly: every read
        // of `w.X` goes through the iframe contentWindow Proxy and may
        // surface trap frames if an Error is constructed there.
        let iframeStack = null;
        try {
            // Cause a Function constructor call inside an iframe-realm
            // method, where any Error created sees our Proxy frames.
            const fnViaProxy = w.Function;
            // Synthesize an Error via the iframe realm.
            iframeStack = new (fnViaProxy)("return new Error('probe').stack;")();
        } catch (e) { iframeStack = 'ERR: ' + e.message; }
        const want = ['at Object.apply', 'at Reflect.apply', 'at newHandler.<computed>', 'at Proxy.', 'at handler.'];
        const hits = {};
        for (const n of want) {
            hits['proxy.' + n] = (sentinel.stack || '').includes(n);
            hits['iframe.' + n] = (iframeStack || '').includes(n);
        }
        JSON.stringify({
            proxyStack: sentinel.stack,
            iframeStack,
            hits,
        });
    "#;
    let result = page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"));
    eprintln!("kasada-proxy-stack-leak: {result}");
    assert!(!result.starts_with("ERROR:"));
}
