//! Task#2 diagnostic: which of duolingo's 7 client-side capability
//! predicates is FALSE in our engine (any false ⇒ the homepage JS
//! self-redirects to /errors/not-supported.html). Verbatim from the
//! 2026-05-17 www.duolingo.com inline gate. `#[ignore]` (diagnostic).
//!
//! Run: cargo test -p browser --test spa_caps_diag -- --ignored --nocapture

use browser_oxide::Page;

// Task#2 regression (network-free): all 7 duolingo capability
// predicates must be TRUE in our engine, else the homepage JS
// self-redirects to /errors/not-supported.html. Was failing on
// supportsAbortController (Request.prototype.signal) +
// supportsIntersectionObserver (IntersectionObserverEntry); both
// fixed in fetch_bootstrap.js / window_bootstrap.js.
#[tokio::test]
async fn spa_capability_predicates() {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body><div id=root></div></body></html>",
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    // Each entry: (name, the verbatim duolingo predicate expression).
    let preds: &[(&str, &str)] = &[
        ("supportsWebAssembly", r#"("WebAssembly" in window)"#),
        (
            "supportsAbortController",
            r#"("AbortController" in window && "Request" in window && Object.hasOwnProperty.call(Request.prototype,"signal"))"#,
        ),
        (
            "supportsElementAnimate",
            r#"("animate" in Element.prototype)"#,
        ),
        (
            "supportsEs2019",
            r#"("flat" in Array.prototype && "flatMap" in Array.prototype && "fromEntries" in Object && "trimStart" in String.prototype && "trimEnd" in String.prototype && "description" in Symbol.prototype)"#,
        ),
        (
            "supportsIntersectionObserver",
            r#"("IntersectionObserver" in window && "IntersectionObserverEntry" in window && "intersectionRatio" in window.IntersectionObserverEntry.prototype && "isIntersecting" in window.IntersectionObserverEntry.prototype)"#,
        ),
        ("supportsResizeObserver", r#"("ResizeObserver" in window)"#),
        (
            "supportsES2015",
            r#"(function(){if("undefined"==typeof Symbol||"undefined"==typeof Proxy)return!1;try{return new Function("(a = 0) => a")(),new Function("class MyEvent extends Event{}")(),!1===new Function("return new Boolean(Symbol.match)")()?!1:(new Function("new.target")(),new Function('class C extends Array {constructor(j = "a", ...c) {const q = (({u: e}) => {return { [`s${c}`]: Symbol(j) };})({});super(j, q, ...c);}}new Promise((f) => {const a = function* (){return "x".match(/./u)[0].length === 1 || true;};for (let vre of a()) {const [uw, as, he, re] = [new Set(), new WeakSet(), new Map(), new WeakMap()];break;}f(new Proxy({}, {get: (han, h) => h in han ? han[h] : "42".repeat(0)}));}).then(bi => new C(bi.rd));')(),!!new Function("return (a, b,) => a.padStart(5, '0') === '0000x' && Object.values(b).length === 2")()("x",{a:1,b:2}))}catch(e){return e.toString()}})()"#,
        ),
    ];

    let mut any_false = false;
    for (name, expr) in preds {
        let r = page
            .evaluate(&format!("String({expr})"))
            .unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
        let ok = r.trim_matches('"') == "true";
        if !ok {
            any_false = true;
        }
        println!(
            "[duo-cap] {:<28} = {}{}",
            name,
            r.trim_matches('"'),
            if ok {
                ""
            } else {
                "   <-- FAILS (triggers not-supported redirect)"
            }
        );
    }
    assert!(
        !any_false,
        "a duolingo capability predicate is FALSE ⇒ the homepage would \
         self-redirect to /errors/not-supported.html (see [duo-cap] lines)"
    );
}
