//! V8 natives parity with Chrome 147 — BotD, fp-collect, CreepJS check
//! that natives stringify in Chrome's exact shape and that engine markers
//! match (eval.toString().length === 33, etc.).

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
// BotD eval_length detector — Chromium V8 returns 33.
// ================================================================
#[tokio::test]
async fn eval_to_string_length_is_33() {
    let r = evaluate("eval.toString().length").await;
    assert_eq!(r, "33", "eval.toString().length must be 33 (Chromium V8 invariant)");
}

#[tokio::test]
async fn eval_to_string_native_shape() {
    let r = evaluate("eval.toString()").await;
    assert!(
        r.contains("[native code]"),
        "eval.toString() must contain [native code], got: {r}"
    );
}

// ================================================================
// Function.prototype.toString shape on natives
// ================================================================
#[tokio::test]
async fn math_sin_to_string_native_shape() {
    let r = evaluate("Function.prototype.toString.call(Math.sin)").await;
    assert!(r.contains("[native code]"), "Math.sin native toString missing [native code]: {r}");
}

#[tokio::test]
async fn fetch_to_string_native_shape() {
    let r = evaluate("Function.prototype.toString.call(window.fetch)").await;
    assert!(r.contains("[native code]"), "fetch native toString missing [native code]: {r}");
}

#[tokio::test]
async fn navigator_permissions_query_to_string_native_shape() {
    let r = evaluate("Function.prototype.toString.call(navigator.permissions.query)").await;
    assert!(r.contains("[native code]"), "permissions.query missing [native code]: {r}");
}

// ================================================================
// Error.prepareStackTrace strips deno: / ext: / bootstrap frames
// ================================================================
#[tokio::test]
async fn error_stack_does_not_leak_deno_frames() {
    let r = evaluate(
        "(()=>{ try { null.foo } catch(e) { return e.stack || ''; } })()",
    )
    .await;
    assert!(
        !r.contains("deno:") && !r.contains("ext:"),
        "Error.stack leaks engine-internal frames: {r}"
    );
}

#[tokio::test]
async fn error_stack_does_not_leak_bootstrap_frames() {
    let r = evaluate(
        "(()=>{ try { null.foo } catch(e) { return e.stack || ''; } })()",
    )
    .await;
    assert!(
        !r.contains("bootstrap"),
        "Error.stack leaks bootstrap frames: {r}"
    );
}

// ================================================================
// Engine identifiers
// ================================================================
#[tokio::test]
async fn promise_with_resolvers_exists() {
    // Chrome 147 ships Promise.withResolvers (since Chrome 119).
    let r = evaluate("typeof Promise.withResolvers").await;
    assert_eq!(r, "function");
}

#[tokio::test]
async fn structured_clone_exists() {
    let r = evaluate("typeof structuredClone").await;
    assert_eq!(r, "function");
}

#[tokio::test]
async fn request_idle_callback_exists() {
    // Chrome desktop ships requestIdleCallback; absence is a Safari signal.
    let r = evaluate("typeof requestIdleCallback").await;
    assert_eq!(r, "function");
}

// ================================================================
// Function.prototype.toString.call(Function.prototype.toString) is itself
// native-shape — CreepJS recurses on this.
// ================================================================
#[tokio::test]
async fn function_to_string_self_native_shape() {
    let r = evaluate("Function.prototype.toString.call(Function.prototype.toString)").await;
    assert!(r.contains("[native code]"), "self-toString missing [native code]: {r}");
}
