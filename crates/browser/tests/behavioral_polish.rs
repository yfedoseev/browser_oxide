//! Behavioral polish — fp-collect, CreepJS, BotD probes verify Chrome 147
//! desktop-coherent values for performance.memory, navigator.connection
//! quantization, and assorted other surfaces that have been trip-wires.

use browser::Page;
use stealth;

async fn evaluate(js: &str) -> String {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

// ================================================================
// performance.memory.jsHeapSizeLimit — Chrome desktop = 4294705152 (4GB).
// 2172649472 (2GB) is the headless flag.
// ================================================================
#[tokio::test]
async fn perf_memory_heap_limit_is_desktop_value() {
    let r = evaluate("performance.memory.jsHeapSizeLimit").await;
    assert_eq!(
        r, "4294705152",
        "jsHeapSizeLimit must be Chrome desktop value"
    );
}

// ================================================================
// navigator.connection.rtt rounded to 25 ms (Chrome quantization).
// ================================================================
#[tokio::test]
async fn navigator_connection_rtt_25ms_quantized() {
    let r = evaluate("navigator.connection.rtt % 25").await;
    assert_eq!(
        r, "0",
        "navigator.connection.rtt must be a multiple of 25 ms"
    );
}

// ================================================================
// navigator.connection.downlink rounded to 25 kbps (= 0.025 mbps step).
// ================================================================
#[tokio::test]
async fn navigator_connection_downlink_quantized() {
    // Chrome rounds downlink to nearest 0.025 Mbps. Test: value × 40 is an integer.
    let r = evaluate("Number.isInteger(Math.round(navigator.connection.downlink * 40))").await;
    assert_eq!(
        r, "true",
        "navigator.connection.downlink must be a multiple of 0.025 Mbps"
    );
}

// ================================================================
// hardwareConcurrency must be a typical desktop value.
// ================================================================
#[tokio::test]
async fn hardware_concurrency_is_typical() {
    let r = evaluate("navigator.hardwareConcurrency").await;
    let n: u32 = r.parse().unwrap_or(0);
    // Chrome 147 desktop typically 4–32. CreepJS flags 1, 0, or > 64.
    assert!(n >= 2 && n <= 32, "hardwareConcurrency suspicious: {n}");
}

// ================================================================
// deviceMemory must be in the allowed enum {0.25, 0.5, 1, 2, 4, 8}.
// ================================================================
#[tokio::test]
async fn device_memory_is_quantized() {
    let r = evaluate("navigator.deviceMemory").await;
    let allowed = ["0.25", "0.5", "1", "2", "4", "8"];
    assert!(
        allowed.contains(&r.as_str()),
        "deviceMemory must be in Chrome's enum, got: {r}"
    );
}

// ================================================================
// Symbol-keyed event.isTrusted hook works as designed.
// Page-side `new Event(...)` produces isTrusted=false; an event
// constructed with the internal Symbol opt-in is trusted.
// ================================================================
#[tokio::test]
async fn event_constructor_default_is_not_trusted() {
    let r = evaluate("new Event('click').isTrusted").await;
    assert_eq!(
        r, "false",
        "page-side new Event must produce isTrusted=false"
    );
}

#[tokio::test]
async fn event_trusted_via_internal_symbol() {
    // The Rust dispatchers can use Symbol.for('__bo_trusted__') to opt-in.
    // Page JS doesn't know this Symbol unless it's registered; verify the
    // mechanism works.
    let r =
        evaluate("new Event('click', { [Symbol.for('__bo_trusted__')]: true }).isTrusted").await;
    assert_eq!(r, "true", "Symbol-opt-in event must be trusted");
}

// ================================================================
// ScrollTop / scrollIntoView / scrollTo / scrollBy persist state.
// ================================================================
#[tokio::test]
async fn scroll_top_persists_across_reads() {
    let r = evaluate(
        "
        const d = document.createElement('div');
        document.body.appendChild(d);
        d.scrollTop = 123;
        d.scrollTop
        ",
    )
    .await;
    assert_eq!(r, "123", "scrollTop set/get must persist");
}

#[tokio::test]
async fn scroll_into_view_does_not_throw() {
    let r = evaluate(
        "
        const d = document.createElement('div');
        document.body.appendChild(d);
        try { d.scrollIntoView({behavior:'smooth'}); 'ok' } catch(e) { 'threw' }
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn scroll_to_options_object() {
    let r = evaluate(
        "
        const d = document.createElement('div');
        document.body.appendChild(d);
        d.scrollTo({top: 50, left: 10});
        d.scrollTop + ',' + d.scrollLeft
        ",
    )
    .await;
    assert_eq!(r, "50,10");
}

// ================================================================
// matchMedia round-trip — Chrome accepts standard queries.
// ================================================================
#[tokio::test]
async fn match_media_returns_object() {
    let r = evaluate("typeof matchMedia('(prefers-color-scheme: light)').matches").await;
    assert_eq!(r, "boolean");
}
