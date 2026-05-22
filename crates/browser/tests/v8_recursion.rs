//! V8 recursion handling — fingerprint sites (sannysoft, creepjs) trigger
//! deep recursion in shim code or in their own VMs. Real Chrome catches
//! this as a JS `RangeError`; a misconfigured isolate segfaults instead
//! (issue #60 in the project tracker).
//!
//! These tests exercise progressively deeper recursion to validate that
//! our isolate cleanly throws RangeError before C-stack exhaustion.

use browser::Page;

async fn page() -> Page {
    Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap()
}

/// Direct recursion: a function calling itself. V8 should catch this as
/// RangeError before the C stack overflows.
#[tokio::test]
async fn direct_recursion_throws_range_error() {
    let mut p = page().await;
    let r = p
        .evaluate(
            r#"
        try {
            (function f() { f(); })();
            "no-error"
        } catch (e) {
            e instanceof RangeError ? "RangeError" : ("Other:" + e.message)
        }
        "#,
        )
        .unwrap();
    assert_eq!(
        r, "RangeError",
        "deep direct recursion must surface as JS RangeError, not crash"
    );
}

/// Mutual recursion through a shim-style wrapper. This is the shape that
/// sannysoft/creepjs probes use — chained `Function.prototype.toString`
/// calls that look benign individually but compose into a deep call stack.
#[tokio::test]
async fn shim_style_mutual_recursion_throws_range_error() {
    let mut p = page().await;
    let r = p
        .evaluate(
            r#"
        try {
            const a = function() { return b(); };
            const b = function() { return a(); };
            a();
            "no-error"
        } catch (e) {
            e instanceof RangeError ? "RangeError" : ("Other:" + e.message)
        }
        "#,
        )
        .unwrap();
    assert_eq!(r, "RangeError");
}

/// Recursion via `Function.prototype.apply` — Tier-1 anti-bot scripts
/// often probe identity by calling a wrapped function via apply, which
/// adds C-frames per call. Our shim wrappers must not amplify this.
#[tokio::test]
async fn apply_chain_recursion_throws_range_error() {
    let mut p = page().await;
    let r = p
        .evaluate(
            r#"
        try {
            const f = function() { return f.apply(this, arguments); };
            f();
            "no-error"
        } catch (e) {
            e instanceof RangeError ? "RangeError" : ("Other:" + e.message)
        }
        "#,
        )
        .unwrap();
    assert_eq!(r, "RangeError");
}

/// After RangeError, the isolate must remain usable. A fingerprint scene
/// that triggers recursion early shouldn't poison the rest of the page.
#[tokio::test]
async fn isolate_recovers_after_range_error() {
    let mut p = page().await;
    p.evaluate(
        r#"
        try { (function f() { f(); })(); } catch (e) {}
        "#,
    )
    .unwrap();
    let r = p.evaluate("1 + 1").unwrap();
    assert_eq!(
        r, "2",
        "isolate must execute new scripts after a RangeError"
    );
}

/// Proxy-trap recursion: a Proxy whose `get` trap returns the proxy
/// itself, accessed in a chain. CreepJS uses this exact pattern in its
/// "lies" detector. V8 must surface RangeError, not segfault.
#[tokio::test]
async fn proxy_trap_recursion_throws_range_error() {
    let mut p = page().await;
    let r = p
        .evaluate(
            r#"
        try {
            const p = new Proxy({}, { get(t, k) { return p[k]; } });
            p.foo;
            "no-error"
        } catch (e) {
            e instanceof RangeError ? "RangeError" : ("Other:" + e.message)
        }
        "#,
        )
        .unwrap();
    assert_eq!(r, "RangeError");
}

/// Getter-chain recursion: an accessor whose getter accesses itself.
/// Akamai sensor v3 probes this on Element.prototype getters.
#[tokio::test]
async fn getter_chain_recursion_throws_range_error() {
    let mut p = page().await;
    let r = p
        .evaluate(
            r#"
        try {
            const o = {};
            Object.defineProperty(o, 'x', { get() { return o.x; } });
            o.x;
            "no-error"
        } catch (e) {
            e instanceof RangeError ? "RangeError" : ("Other:" + e.message)
        }
        "#,
        )
        .unwrap();
    assert_eq!(r, "RangeError");
}

/// Verify the test thread has at least 32 MB of stack — proves the
/// `.cargo/config.toml` `RUST_MIN_STACK = "67108864"` setting is
/// reaching the libtest-spawned thread. If RUST_MIN_STACK weren't
/// applied, this test would run on a ~2 MB tokio default stack.
///
/// We use a Rust-side recursive function (not JS) because V8 throws
/// RangeError before the OS stack gets near its limit. This test
/// drives the C-stack directly: each frame consumes a 4 KB local
/// array, and we recurse 4096 times = 16 MB. A 2 MB stack overflows
/// way before reaching that depth; a 64 MB stack reaches it easily.
#[test]
fn test_thread_stack_is_at_least_16mb() {
    fn recurse(depth: u32, max: u32) -> u32 {
        // 4 KB per frame
        let _local: [u8; 4096] = [0; 4096];
        if depth == max {
            std::hint::black_box(_local[0]) as u32
        } else {
            recurse(depth + 1, max)
        }
    }
    let r = recurse(0, 4096);
    assert_eq!(
        r, 0,
        "recurse to 16 MB stack depth must succeed — proves RUST_MIN_STACK env reached this thread"
    );
}

/// `toString` recursion via Function.prototype patching — the exact
/// pattern that fingerprint scripts use to detect monkey-patched
/// natives. If our shim's Function.prototype.toString wrapper accidentally
/// invokes itself, this is the crash path.
#[tokio::test]
#[allow(non_snake_case)] // mirrors JS API name under test
async fn function_toString_recursion_throws_range_error() {
    let mut p = page().await;
    let r = p
        .evaluate(
            r#"
        try {
            const orig = Function.prototype.toString;
            Function.prototype.toString = function() {
                return Function.prototype.toString.call(this);  // recurses through patched version
            };
            try {
                (function dummy() {}).toString();
            } finally {
                Function.prototype.toString = orig;
            }
            "no-error"
        } catch (e) {
            // Restore in case the catch is reached before `finally`
            "Caught:" + (e instanceof RangeError ? "RangeError" : e.message)
        }
        "#,
        )
        .unwrap();
    assert_eq!(r, "Caught:RangeError");
}
