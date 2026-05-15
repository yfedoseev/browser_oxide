//! API completeness — Chrome 147 ships ~1080 globals + ~57 Navigator.prototype
//! entries. CreepJS `features` and fp-collect `navigatorPrototype` walks hash
//! the constructor list. Missing constructors are tells. This suite asserts
//! the constructors browser_oxide stubs to land.

use browser::Page;
use stealth;

async fn evaluate(js: &str) -> String {
    let mut page = Page::from_html_with_url(
        "<!DOCTYPE html><html><body></body></html>",
        "https://example.com",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

#[tokio::test]
async fn total_window_properties_count() {
    let r = evaluate("Object.getOwnPropertyNames(globalThis).length").await;
    println!("Total window properties: {}", r);
}

/// Helper: assert a constructor exists and has the expected name.
async fn assert_ctor(name: &str) {
    let r = evaluate(&format!("typeof globalThis.{name}")).await;
    assert_eq!(r, "function", "missing globalThis.{name}");
    let n = evaluate(&format!("globalThis.{name}.name")).await;
    assert_eq!(n, name, "globalThis.{name}.name mismatch: got {n}");
}

// ================================================================
// Core DOM / Web Platform constructors that real Chrome 147 ships
// ================================================================

#[tokio::test]
async fn ctor_navigator_exists() {
    assert_ctor("Navigator").await
}
#[tokio::test]
async fn ctor_location_exists() {
    assert_ctor("Location").await
}
#[tokio::test]
async fn ctor_history_exists() {
    assert_ctor("History").await
}
#[tokio::test]
async fn ctor_screen_exists() {
    assert_ctor("Screen").await
}
#[tokio::test]
async fn ctor_event_target_exists() {
    assert_ctor("EventTarget").await
}
#[tokio::test]
async fn ctor_event_exists() {
    assert_ctor("Event").await
}

// ================================================================
// New / non-trivial Chrome 147 constructors
// ================================================================
#[tokio::test]
async fn ctor_css_style_sheet_exists() {
    assert_ctor("CSSStyleSheet").await
}
#[tokio::test]
async fn ctor_highlight_exists() {
    assert_ctor("Highlight").await
}
#[tokio::test]
async fn ctor_highlight_registry_exists() {
    assert_ctor("HighlightRegistry").await
}
#[tokio::test]
async fn ctor_css_pseudo_element_exists() {
    assert_ctor("CSSPseudoElement").await
}
#[tokio::test]
async fn ctor_static_range_exists() {
    assert_ctor("StaticRange").await
}
#[tokio::test]
async fn ctor_xml_serializer_exists() {
    assert_ctor("XMLSerializer").await
}
#[tokio::test]
async fn ctor_xslt_processor_exists() {
    assert_ctor("XSLTProcessor").await
}

#[tokio::test]
async fn ctor_edit_context_exists() {
    assert_ctor("EditContext").await
}
#[tokio::test]
async fn ctor_cookie_store_exists() {
    assert_ctor("CookieStore").await
}
#[tokio::test]
async fn ctor_web_transport_exists() {
    assert_ctor("WebTransport").await
}
#[tokio::test]
async fn ctor_launch_queue_exists() {
    assert_ctor("LaunchQueue").await
}

#[tokio::test]
async fn ctor_file_system_handle_exists() {
    assert_ctor("FileSystemHandle").await
}
#[tokio::test]
async fn ctor_file_system_file_handle_exists() {
    assert_ctor("FileSystemFileHandle").await
}
#[tokio::test]
async fn ctor_file_system_directory_handle_exists() {
    assert_ctor("FileSystemDirectoryHandle").await
}

#[tokio::test]
async fn ctor_push_manager_exists() {
    assert_ctor("PushManager").await
}
#[tokio::test]
async fn ctor_push_subscription_exists() {
    assert_ctor("PushSubscription").await
}
#[tokio::test]
async fn ctor_background_fetch_manager_exists() {
    assert_ctor("BackgroundFetchManager").await
}

#[tokio::test]
async fn ctor_payment_request_exists() {
    assert_ctor("PaymentRequest").await
}
#[tokio::test]
async fn ctor_presentation_connection_exists() {
    assert_ctor("PresentationConnection").await
}

#[tokio::test]
async fn ctor_accelerometer_exists() {
    assert_ctor("Accelerometer").await
}
#[tokio::test]
async fn ctor_gyroscope_exists() {
    assert_ctor("Gyroscope").await
}
#[tokio::test]
async fn ctor_orientation_sensor_exists() {
    assert_ctor("OrientationSensor").await
}
#[tokio::test]
async fn ctor_absolute_orientation_sensor_exists() {
    assert_ctor("AbsoluteOrientationSensor").await
}
#[tokio::test]
async fn ctor_relative_orientation_sensor_exists() {
    assert_ctor("RelativeOrientationSensor").await
}
#[tokio::test]
async fn ctor_linear_acceleration_sensor_exists() {
    assert_ctor("LinearAccelerationSensor").await
}

#[tokio::test]
async fn ctor_battery_manager_exists() {
    assert_ctor("BatteryManager").await
}
#[tokio::test]
async fn ctor_geolocation_exists() {
    assert_ctor("Geolocation").await
}
#[tokio::test]
async fn ctor_xr_system_exists() {
    assert_ctor("XRSystem").await
}
#[tokio::test]
async fn ctor_xr_session_exists() {
    assert_ctor("XRSession").await
}

#[tokio::test]
async fn ctor_credentials_container_exists() {
    assert_ctor("CredentialsContainer").await
}

// ================================================================
// Already-existing constructors (regression coverage)
// ================================================================
#[tokio::test]
async fn ctor_abort_controller_exists() {
    assert_ctor("AbortController").await
}
#[tokio::test]
async fn ctor_abort_signal_exists() {
    assert_ctor("AbortSignal").await
}
#[tokio::test]
async fn ctor_broadcast_channel_exists() {
    assert_ctor("BroadcastChannel").await
}
#[tokio::test]
async fn ctor_message_channel_exists() {
    assert_ctor("MessageChannel").await
}
#[tokio::test]
async fn ctor_event_source_exists() {
    assert_ctor("EventSource").await
}
#[tokio::test]
async fn ctor_dom_parser_exists() {
    assert_ctor("DOMParser").await
}
#[tokio::test]
async fn ctor_range_exists() {
    assert_ctor("Range").await
}
#[tokio::test]
async fn ctor_notification_exists() {
    assert_ctor("Notification").await
}
#[tokio::test]
async fn ctor_identity_credential_exists() {
    assert_ctor("IdentityCredential").await
}

// ================================================================
// Symbol.toStringTag — every host class must respond to
// Object.prototype.toString.call() with the canonical tag.
// ================================================================
#[tokio::test]
async fn navigator_to_string_tag() {
    let r = evaluate("Object.prototype.toString.call(navigator)").await;
    assert_eq!(r, "[object Navigator]");
}

#[tokio::test]
async fn screen_to_string_tag() {
    let r = evaluate("Object.prototype.toString.call(screen)").await;
    assert_eq!(r, "[object Screen]");
}

#[tokio::test]
async fn location_to_string_tag() {
    let r = evaluate("Object.prototype.toString.call(location)").await;
    assert_eq!(r, "[object Location]");
}

// ================================================================
// Illegal-constructor pattern — `new CookieStore()` must throw with
// the Chrome-canonical TypeError message shape.
// ================================================================
#[tokio::test]
async fn cookie_store_illegal_constructor() {
    let r = evaluate(
        "(()=>{ try { new CookieStore(); return 'no-throw'; } catch(e) { return e.name + ':' + (e.message.includes('Illegal')); } })()"
    ).await;
    assert!(
        r.contains("TypeError:true"),
        "CookieStore should throw Illegal constructor: {r}"
    );
}
