//! Master plan §4 Phase 1 **G9** — V8 inspector / runtime parity.
//!
//! 2026 cross-vendor intel (svebaa, "How V8 Leaks Your Headless
//! Browser's Identity"; doc 03 §1.2) describes two CDP/inspector traps
//! that fire only when a DevTools `Runtime.enable` inspector is
//! attached and eagerly previews/serializes console arguments:
//!
//!   1. error-stack getter:     `console.debug(errWithStackGetter)`
//!      → inspector `descriptionForError()` calls `object->Get()` for
//!        name/stack/message, firing a user `stack` getter.
//!   2. Proxy-prototype ownKeys: `console.groupEnd(Object.create(trap))`
//!      → inspector `DebugPropertyIterator` invokes `[[OwnPropertyKeys]]`
//!        on the prototype chain (spec-mandated JS execution).
//!
//! browser_oxide runs **no CDP inspector** in production, so it should
//! be structurally immune — but "should" is not "verified". These are
//! the cheap, high-ROI "verify, don't assume" regression tests the plan
//! asks for. A real non-CDP Chrome (our reference) also passes them, so
//! matching is achievable engine-side. Kept in a dedicated file so the
//! commit stays isolated from the pre-existing uncommitted
//! `chrome_compat.rs` working changes.

use browser::Page;
use stealth;

async fn eval(js: &str) -> String {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

/// (1) Logging an Error whose `stack` is a user getter must NOT invoke
/// that getter — only a CDP inspector's eager error-preview would.
#[tokio::test]
async fn console_debug_does_not_fire_error_stack_getter() {
    let r = eval(
        r#"(() => {
            let fired = false;
            const e = new Error('probe');
            Object.defineProperty(e, 'stack', {
                configurable: true,
                get() { fired = true; return ''; }
            });
            try { console.debug(e); } catch (_) {}
            try { console.log(e); } catch (_) {}
            try { console.error(e); } catch (_) {}
            return String(fired);
        })()"#,
    )
    .await;
    assert_eq!(
        r, "false",
        "console.* eagerly read Error().stack — that is the CDP/inspector \
         error-preview tell (svebaa); a non-CDP engine must not fire it"
    );
}

/// (2) Logging `Object.create(proxyWithOwnKeysTrap)` must NOT invoke
/// the prototype Proxy's `ownKeys` trap — only a CDP inspector's
/// `DebugPropertyIterator` preview walks the prototype chain.
#[tokio::test]
async fn console_grouped_log_does_not_fire_proto_proxy_ownkeys() {
    let r = eval(
        r#"(() => {
            let fired = false;
            const trap = new Proxy({}, { ownKeys() { fired = true; return []; } });
            const o = Object.create(trap);
            try { console.groupEnd(o); } catch (_) {}
            try { console.log(o); } catch (_) {}
            try { console.dir(o); } catch (_) {}
            return String(fired);
        })()"#,
    )
    .await;
    assert_eq!(
        r, "false",
        "console.* invoked a prototype-chain Proxy ownKeys trap — that is \
         the 2026 CDP/inspector preview tell that bypasses the V8 patch"
    );
}

/// (3) `Error().stack` is lazily materialised: a never-read stack must
/// not eagerly run `Error.prepareStackTrace`; reading it then yields a
/// string. (If the engine ignores `prepareStackTrace`, the fallback
/// assertion still verifies `.stack` is a present string and that
/// construction has no eager side effect.)
#[tokio::test]
async fn error_stack_is_lazy() {
    let r = eval(
        r#"(() => {
            let calls = 0;
            const prev = Error.prepareStackTrace;
            Error.prepareStackTrace = function (_e, _s) { calls++; return 'X'; };
            const e = new Error('lazy');
            const beforeRead = calls;          // must be 0 — not eager
            const s = e.stack;                 // materialise now
            const isStr = (typeof s === 'string');
            Error.prepareStackTrace = prev;
            // beforeRead===0 always required (no eager compute).
            // honored: prepareStackTrace ran on access (calls>=1) and we
            // got our 'X'. unhonored: engine builds its own stack string
            // (calls stays 0) — still must be a present string.
            return beforeRead + '|' + isStr + '|' + (calls >= 1);
        })()"#,
    )
    .await;
    let parts: Vec<&str> = r.split('|').collect();
    assert_eq!(
        parts.first().copied(),
        Some("0"),
        "Error.prepareStackTrace ran before .stack was ever read — \
         stack is being eagerly computed at construction (got {r:?})"
    );
    assert_eq!(
        parts.get(1).copied(),
        Some("true"),
        "Error().stack did not materialise as a string on access (got {r:?})"
    );
}
