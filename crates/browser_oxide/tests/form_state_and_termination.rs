//! Regression tests for two independent defects reported together:
//!
//! 1. **Form state was missing.** `select.value` / `select.selectedIndex`
//!    were `undefined`, `textarea.value` never reflected text, `option` state
//!    did not exist, and `checkbox.click()` fired the event without running
//!    the activation behaviour — so `.checked` never flipped. Any gate flow
//!    driven by form state stalled.
//!
//! 2. **A stale `terminate_execution()` poisoned the isolate.** The
//!    navigation deadline watcher could deliver a termination *after* the
//!    script it targeted had finished, and it stayed latched. Every later
//!    `evaluate()` then failed ("execution terminated") or hung, process-wide.
//!    The engine now clears a request that no armed watcher can still be
//!    responsible for, and the host binding discards an isolate V8 has
//!    declared dead instead of reusing it.

use browser_oxide::Page;

async fn blank_page() -> Page {
    Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .expect("blank page builds")
}

// ── Form state ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn select_value_and_selected_index_round_trip() {
    let mut p = Page::from_html(
        r#"<html><body>
             <select id="s">
               <option value="a">A</option>
               <option value="b" selected>B</option>
               <option>C</option>
             </select>
           </body></html>"#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    // Attribute-driven initial selection.
    assert_eq!(
        p.evaluate("document.getElementById('s').selectedIndex")
            .unwrap(),
        "1"
    );
    assert_eq!(
        p.evaluate("document.getElementById('s').value").unwrap(),
        "b"
    );
    // Text-content fallback for an <option> without a value attribute.
    assert_eq!(
        p.evaluate("document.getElementById('s').options[2].value")
            .unwrap(),
        "C"
    );

    // Property-driven selection writes through to state the form sees.
    let r = p
        .evaluate(
            r#"(() => {
                 const s = document.getElementById('s');
                 s.value = 'a';
                 return JSON.stringify({ idx: s.selectedIndex, val: s.value, sel: s.options[0].selected });
               })()"#,
        )
        .unwrap();
    assert_eq!(r, r#"{"idx":0,"val":"a","sel":true}"#);
}

#[tokio::test]
async fn textarea_value_reflects_text_not_attribute() {
    let mut p = Page::from_html(
        r#"<html><body><textarea id="t">initial</textarea></body></html>"#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(
        p.evaluate("document.getElementById('t').value").unwrap(),
        "initial"
    );
    let r = p
        .evaluate(
            r#"(() => {
                 const t = document.getElementById('t');
                 t.value = 'typed';
                 return JSON.stringify({ v: t.value, html: document.getElementById('t').outerHTML });
               })()"#,
        )
        .unwrap();
    assert!(
        r.contains(r#""v":"typed""#),
        "textarea.value must round-trip: {r}"
    );
    // Must not serialize as a bogus value="" attribute.
    assert!(
        !r.contains("value=\\\"typed\\\""),
        "textarea must not emit a value attribute: {r}"
    );
}

#[tokio::test]
async fn checkbox_click_runs_activation_behaviour() {
    let mut p = blank_page().await;
    let r = p
        .evaluate(
            r#"(() => {
                 const box = document.createElement('input');
                 box.setAttribute('type', 'checkbox');
                 document.body.appendChild(box);
                 const before = box.checked;
                 box.click();
                 const after = box.checked;
                 box.click();
                 return JSON.stringify({ before, after, toggledBack: box.checked });
               })()"#,
        )
        .unwrap();
    assert_eq!(
        r, r#"{"before":false,"after":true,"toggledBack":false}"#,
        "click() must toggle .checked and toggle back: {r}"
    );
}

#[tokio::test]
async fn radio_click_is_exclusive_within_group() {
    let mut p = blank_page().await;
    let r = p
        .evaluate(
            r#"(() => {
                 const mk = (v) => {
                   const el = document.createElement('input');
                   el.setAttribute('type', 'radio');
                   el.setAttribute('name', 'g');
                   el.setAttribute('value', v);
                   document.body.appendChild(el);
                   return el;
                 };
                 const a = mk('a'), b = mk('b');
                 a.click(); b.click();
                 return JSON.stringify({ a: a.checked, b: b.checked });
               })()"#,
        )
        .unwrap();
    assert_eq!(
        r, r#"{"a":false,"b":true}"#,
        "clicking a radio must clear its peers in the same group: {r}"
    );
}

#[tokio::test]
async fn form_submit_serializes_checked_and_selected_state() {
    let mut p = Page::from_html(
        r#"<html><body>
             <form id="f" method="get">
               <input name="q" value="hello">
               <input type="checkbox" name="opt" checked>
               <select name="pick"><option value="x">X</option><option value="y" selected>Y</option></select>
             </form>
           </body></html>"#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    // The serializer reads `checked` / `value`; before the fix the checkbox
    // contributed nothing and the select contributed "undefined".
    let nav = p
        .evaluate("(() => { document.getElementById('f').submit(); return JSON.stringify(globalThis.__pendingNavigation); })()")
        .unwrap();
    assert!(nav.contains("q=hello"), "input value must serialize: {nav}");
    assert!(
        nav.contains("opt="),
        "checked checkbox must serialize: {nav}"
    );
    assert!(
        nav.contains("pick=y"),
        "selected option must serialize: {nav}"
    );
}

// ── Termination lifecycle ──────────────────────────────────────────────────

/// A request that outlives the work it targeted must not kill later scripts.
///
/// `terminate_execution()` is an asynchronous *request*: V8 latches it as a
/// stack-guard interrupt until the JS thread next checks, so a request issued
/// while idle survives to abort the next (unrelated) script. This is the
/// "evaluate dies process-wide" symptom.
#[tokio::test]
async fn stale_termination_is_cleared_before_next_script() {
    let mut p = blank_page().await;
    let handle = p.event_loop().runtime_mut().isolate_handle();
    handle.terminate_execution();

    // The watcher that would have armed this deadline is already gone, so the
    // request is stale and must be discarded rather than fail every evaluate.
    assert_eq!(
        p.evaluate("1 + 1").unwrap(),
        "2",
        "a stale terminate request must not fail the next evaluate"
    );
    assert_eq!(p.evaluate("'still alive'").unwrap(), "still alive");
}

/// While a deadline watcher is armed, a termination is legitimate and must be
/// honored — the fix must not swallow a real interruption.
#[tokio::test]
async fn armed_termination_is_honored() {
    let mut p = blank_page().await;
    p.event_loop().runtime_mut().arm_deadline_watcher();
    let handle = p.event_loop().runtime_mut().isolate_handle();
    handle.terminate_execution();

    let outcome = p.evaluate("1 + 1");
    assert!(
        outcome.is_err(),
        "an armed watcher's termination must interrupt the script, got: {outcome:?}"
    );

    // Once the watcher is gone, the isolate recovers.
    p.event_loop().runtime_mut().clear_pending_termination();
    assert_eq!(p.evaluate("1 + 1").unwrap(), "2");
}
