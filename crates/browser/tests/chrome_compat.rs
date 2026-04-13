//! Chrome compatibility audit.
//!
//! Tests every API that a real Chrome 130 browser exposes.
//! Each test checks typeof/existence AND basic behavior.
//! Failures here = gaps vs real Chrome.

use browser::Page;

fn html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head></head><body>{}</body></html>",
        body
    )
}

async fn check(js: &str) -> String {
    let mut page = Page::from_html(&html("")).await.unwrap();
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
    assert_eq!(check("isSecureContext").await, "true");
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
    assert_eq!(check("navigator.deviceMemory > 0").await, "true");
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
    assert_eq!(check("typeof navigator.getBattery").await, "function");
}
#[tokio::test]
async fn nav_user_agent_data() {
    assert_eq!(check("typeof navigator.userAgentData").await, "object");
}
#[tokio::test]
async fn nav_ua_data_brands() {
    assert_eq!(
        check("navigator.userAgentData.brands.length > 0").await,
        "true"
    );
}
#[tokio::test]
async fn nav_ua_data_mobile() {
    assert_eq!(
        check("typeof navigator.userAgentData.mobile").await,
        "boolean"
    );
}
// Client Hints API contract — browser_oxide exposes the full getHighEntropyValues
// surface required by CreepJS / Yandex Antirobot / WBAAS.
#[tokio::test]
async fn nav_ua_data_get_high_entropy_is_function() {
    assert_eq!(
        check("typeof navigator.userAgentData.getHighEntropyValues").await,
        "function"
    );
}
#[tokio::test]
async fn nav_ua_data_get_high_entropy_returns_promise() {
    assert_eq!(
        check("navigator.userAgentData.getHighEntropyValues([]) instanceof Promise").await,
        "true"
    );
}
// For tests that need to inspect the resolved object, kick off the Promise and
// stash its result in a synchronous global via .then(); then pump microtasks.
// Our Page::evaluate drains microtasks before returning, so window.__r is
// populated by the time the second evaluate() reads it.
#[tokio::test]
async fn nav_ua_data_high_entropy_full_version_list() {
    let mut page = Page::from_html(&html("")).await.unwrap();
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
    let mut page = Page::from_html(&html("")).await.unwrap();
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
    let mut page = Page::from_html(&html("")).await.unwrap();
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
        check(
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
        check(
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
    assert_eq!(check("typeof navigator.mediaDevices").await, "object");
}
#[tokio::test]
async fn nav_permissions() {
    assert_eq!(check("typeof navigator.permissions").await, "object");
}
#[tokio::test]
async fn nav_clipboard() {
    assert_eq!(check("typeof navigator.clipboard").await, "object");
}
#[tokio::test]
async fn nav_storage() {
    assert_eq!(check("typeof navigator.storage").await, "object");
}
#[tokio::test]
async fn nav_service_worker() {
    assert_eq!(check("typeof navigator.serviceWorker").await, "object");
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
    assert_eq!(check("document.characterSet").await, "UTF-8");
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
    assert_eq!(check("typeof navigator.getBattery").await, "function");
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
    // macOS profile should get the Apple M2 Pro renderer.
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
        r.contains("Apple M2"),
        "macOS profile should report Apple M2, got: {r}"
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
    assert_eq!(check("typeof chrome.runtime").await, "object");
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
    assert_eq!(check("Object.getOwnPropertyNames(screen).length").await, "0");
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
        check(
            "JSON.stringify(Object.getOwnPropertyNames(TextEncoder.prototype).sort())"
        )
        .await,
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

// --- Crypto / SubtleCrypto / Performance ---

#[tokio::test]
async fn crypto_instanceof_crypto() {
    assert_eq!(check("crypto instanceof Crypto").await, "true");
}

#[tokio::test]
async fn crypto_subtle_instanceof_subtle_crypto() {
    assert_eq!(check("crypto.subtle instanceof SubtleCrypto").await, "true");
}

#[tokio::test]
async fn crypto_has_no_own_properties() {
    assert_eq!(check("Object.getOwnPropertyNames(crypto).length").await, "0");
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
        check("crypto.randomUUID.toString().includes('[native code]')").await,
        "true"
    );
}

#[tokio::test]
async fn crypto_random_uuid_format() {
    assert_eq!(
        check("/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(crypto.randomUUID())").await,
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
    assert_eq!(check("typeof crypto.subtle.digest").await, "function");
}

#[tokio::test]
async fn crypto_subtle_digest_is_native() {
    assert_eq!(
        check("crypto.subtle.digest.toString().includes('[native code]')").await,
        "true"
    );
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
