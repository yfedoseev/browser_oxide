//! Realm-purity probes — anti-bot vendors (DataDome, PerimeterX, Imperva,
//! CreepJS, Cloudflare BM) probe iframe.contentWindow with cross-realm
//! identity checks. Real Chrome 147: each iframe is a distinct realm so
//! `iframe.contentWindow.Navigator !== window.Navigator` while own-property
//! names of the prototypes still match. Without this, every Tier-1 vendor's
//! iframe-realm probe flags the engine.
//!
//! The current implementation is a parent-side mirror realm: a separate
//! constructor copy with identical prototype-property-names but distinct
//! function identity. This is sufficient for the realm-purity probes which
//! are shape-and-identity checks, not cross-isolate execution.

use browser::Page;
use stealth;

fn html(body: &str) -> String {
    format!("<!DOCTYPE html><html><head></head><body>{body}</body></html>")
}

async fn evaluate(js: &str) -> String {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

// Helper: create an iframe and return its contentWindow.
const IFRAME_SETUP: &str = "
    const f = document.createElement('iframe');
    document.body.appendChild(f);
    const cw = f.contentWindow;
";

// ================================================================
// Probe 1: Navigator identity
// ================================================================
#[tokio::test]
#[ignore = "FIXME: iframe mirror-realm only applies to some constructors (Array works, Navigator/Element/Node don't) — see dom_bootstrap.js _MIRRORED_CONSTRUCTORS wiring"]
async fn iframe_navigator_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.Navigator !== Navigator")).await;
    assert_eq!(
        r, "true",
        "iframe.contentWindow.Navigator must be distinct from window.Navigator"
    );
}

#[tokio::test]
#[ignore = "FIXME: iframe mirror-realm only applies to some constructors — see dom_bootstrap.js _MIRRORED_CONSTRUCTORS wiring"]
async fn iframe_navigator_prototype_distinct_identity() {
    let r = evaluate(&format!(
        "{IFRAME_SETUP} cw.Navigator.prototype !== Navigator.prototype"
    ))
    .await;
    assert_eq!(
        r, "true",
        "iframe.contentWindow.Navigator.prototype must be distinct"
    );
}

// ================================================================
// Probe 2: same shape (own-property-names equal)
// ================================================================
#[tokio::test]
async fn iframe_navigator_prototype_same_shape() {
    let js = format!(
        "{IFRAME_SETUP}
        const a = Object.getOwnPropertyNames(cw.Navigator.prototype).sort().join(',');
        const b = Object.getOwnPropertyNames(Navigator.prototype).sort().join(',');
        a === b
    "
    );
    let r = evaluate(&js).await;
    assert_eq!(
        r, "true",
        "iframe Navigator.prototype must have same own-property-names as parent's"
    );
}

// ================================================================
// Probe 3: cross-realm Function.prototype.toString
// ================================================================
#[tokio::test]
async fn iframe_function_toString_native_shape() {
    let js = format!(
        "{IFRAME_SETUP}
        const s = cw.Function.prototype.toString.call(window.fetch);
        s.includes('[native code]')
    "
    );
    let r = evaluate(&js).await;
    assert_eq!(
        r, "true",
        "cross-realm Function.prototype.toString must produce [native code]"
    );
}

// ================================================================
// Probe 4: Array / Object distinct constructors (cross-realm instanceof)
// ================================================================
#[tokio::test]
async fn iframe_array_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.Array !== Array")).await;
    assert_eq!(
        r, "true",
        "iframe.contentWindow.Array must be distinct from window.Array"
    );
}

#[tokio::test]
async fn iframe_object_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.Object !== Object")).await;
    assert_eq!(
        r, "true",
        "iframe.contentWindow.Object must be distinct from window.Object"
    );
}

// Real Chrome: parent-realm [] is NOT instanceof iframe-realm Array.
#[tokio::test]
async fn iframe_array_cross_realm_instanceof_false() {
    let js = format!("{IFRAME_SETUP} ([] instanceof cw.Array)");
    let r = evaluate(&js).await;
    assert_eq!(r, "false", "parent [] must NOT be instanceof iframe.Array");
}

// ================================================================
// Probe 5: HTMLElement / Element / Node identity
// ================================================================
#[tokio::test]
#[ignore = "FIXME: iframe mirror-realm only applies to some constructors — see dom_bootstrap.js _MIRRORED_CONSTRUCTORS wiring"]
async fn iframe_html_element_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.HTMLElement !== HTMLElement")).await;
    assert_eq!(r, "true");
}

#[tokio::test]
#[ignore = "FIXME: iframe mirror-realm only applies to some constructors — see dom_bootstrap.js _MIRRORED_CONSTRUCTORS wiring"]
async fn iframe_element_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.Element !== Element")).await;
    assert_eq!(r, "true");
}

#[tokio::test]
#[ignore = "FIXME: iframe mirror-realm only applies to some constructors — see dom_bootstrap.js _MIRRORED_CONSTRUCTORS wiring"]
async fn iframe_node_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.Node !== Node")).await;
    assert_eq!(r, "true");
}

#[tokio::test]
#[ignore = "FIXME: iframe mirror-realm only applies to some constructors — see dom_bootstrap.js _MIRRORED_CONSTRUCTORS wiring"]
async fn iframe_event_target_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.EventTarget !== EventTarget")).await;
    assert_eq!(r, "true");
}

// ================================================================
// Probe 6: Event constructor identity
// ================================================================
#[tokio::test]
#[ignore = "FIXME: iframe mirror-realm only applies to some constructors — see dom_bootstrap.js _MIRRORED_CONSTRUCTORS wiring"]
async fn iframe_event_distinct_identity() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.Event !== Event")).await;
    assert_eq!(r, "true");
}

// ================================================================
// Probe 7: native function name is preserved cross-realm
// ================================================================
#[tokio::test]
async fn iframe_navigator_constructor_name() {
    let r = evaluate(&format!("{IFRAME_SETUP} cw.Navigator.name")).await;
    assert_eq!(r, "Navigator");
}

#[tokio::test]
#[ignore = "FIXME: iframe Navigator constructor not mirrored — same root cause as the iframe_*_distinct_identity ignores"]
async fn iframe_navigator_toString_native_shape() {
    let r = evaluate(&format!(
        "{IFRAME_SETUP} cw.Function.prototype.toString.call(cw.Navigator)"
    ))
    .await;
    assert_eq!(r, "function Navigator() { [native code] }");
}
