//! Regression tests for the deno_core 0.408 tokio-context requirement (#37)
//! and for env-tunable V8 heap limits.
//!
//! Deliberately **one** `#[test]` fn. Each check needs its own `JsRuntime`, and
//! V8 requires `OwnedIsolate`s be dropped in reverse creation order; splitting
//! these across test fns left isolates interleaving badly enough to abort the
//! binary. Keeping them in one fn, each scoped and dropped before the next, makes
//! the ordering explicit. It also keeps the `#[test]` (not `#[tokio::test]`)
//! property that the #37 regression depends on.

use browser_oxide::js_runtime::BrowserJsRuntime;

fn dom() -> browser_oxide::dom::Dom {
    browser_oxide::html_parser::parse_html("<html><body></body></html>")
}

/// The #37 regression, in the exact shape that aborted: a plain `#[test]`, so
/// the harness enters no tokio runtime.
///
/// As of deno_core 0.408, `JsRuntime` construction captures
/// `tokio::runtime::Handle::try_current()` and V8's delayed foreground tasks (GC
/// memory-reducer work) are spawned on it. With no handle, deno_core prints a
/// diagnostic and calls `std::process::abort()` the first time V8 posts one.
///
/// That abort is **not** debug-gated — release only passes while V8 happens not
/// to post a delayed task in the window under test, which is why 0.408 looked
/// green locally in release and died in CI. The heap churn below provokes GC
/// scheduling so a latent abort becomes a reproducible one.
///
/// On regression the whole binary dies with SIGABRT instead of reporting a
/// failure — look for "V8 posted a delayed task ... outside of a tokio runtime
/// context" as the last line before the abort.
#[test]
fn runtime_env_and_tokio_context() {
    assert!(
        tokio::runtime::Handle::try_current().is_err(),
        "precondition: must run with NO tokio runtime entered, or this cannot \
         catch the #37 regression"
    );

    // 1. Construct with no runtime entered, and churn to provoke GC delayed tasks.
    {
        let mut rt = BrowserJsRuntime::new(dom());
        assert_eq!(rt.execute_script("1 + 2", None).unwrap(), "3");
        for i in 0..25 {
            let src = format!(
                "globalThis.k{i} = new Array(100000).fill('{i}'); globalThis.k{i} = null; 0"
            );
            rt.execute_script(&src, None).unwrap();
        }
        assert_eq!(rt.execute_script("1 + 1", None).unwrap(), "2");
    }

    // 2. Heap limits are read from the environment.
    unsafe {
        std::env::set_var("BROWSER_OXIDE_HEAP_MAX_MB", "512");
        std::env::set_var("BROWSER_OXIDE_HEAP_INITIAL_MB", "64");
    }
    {
        let mut rt = BrowserJsRuntime::new(dom());
        assert_eq!(rt.execute_script("1 + 2", None).unwrap(), "3");
    }

    // 3. Garbage must fall back to the default, not panic — a typo in an env
    //    var should not take down a scrape.
    unsafe {
        std::env::set_var("BROWSER_OXIDE_HEAP_MAX_MB", "not-a-number");
        std::env::set_var("BROWSER_OXIDE_HEAP_INITIAL_MB", "0");
    }
    {
        let mut rt = BrowserJsRuntime::new(dom());
        assert_eq!(
            rt.execute_script("1 + 2", None).unwrap(),
            "3",
            "unparseable/zero heap limits should fall back to defaults"
        );
    }

    // 4. Initial above the ceiling must be clamped — V8 rejects that pairing.
    unsafe {
        std::env::set_var("BROWSER_OXIDE_HEAP_MAX_MB", "512");
        std::env::set_var("BROWSER_OXIDE_HEAP_INITIAL_MB", "4096");
    }
    {
        let mut rt = BrowserJsRuntime::new(dom());
        assert_eq!(rt.execute_script("1 + 2", None).unwrap(), "3");
    }

    unsafe {
        std::env::remove_var("BROWSER_OXIDE_HEAP_MAX_MB");
        std::env::remove_var("BROWSER_OXIDE_HEAP_INITIAL_MB");
    }
}
