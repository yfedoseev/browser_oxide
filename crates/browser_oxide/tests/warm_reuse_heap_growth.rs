//! Quantitative regression test for issue #33 — warm reuse leaked V8 heap.
//!
//! The functional tests in `warm_reuse_reset.rs` prove specific things are
//! unbound. This one proves the *aggregate* consequence: that live V8 heap
//! stays flat across many reuses.
//!
//! It is a true A/B of the fix. `reload_html` WITHOUT `reset_for_reuse` is
//! exactly what 0.1.0's `PagePool::acquire` did, so the `retain` arm
//! reproduces 0.1.0 behaviour and the `reset` arm is HEAD. Comparing the two
//! in one build is more meaningful than comparing against a 0.1.0 checkout,
//! which has no heap-inspection API to measure with in the first place.
//!
//! Measurements are taken after `collect_garbage()`, so they reflect *live*
//! (reachable) objects. That is the whole point: the reporter observed that a
//! full GC recovered only 1-2% of the growth, which is what identified the
//! leak as live references rather than deferred garbage.

use browser_oxide::stealth::StealthProfile;
use browser_oxide::Page;

/// Each load retains ~1 MB behind a `window` listener closure — the exact
/// shape that leaked, and a signal far above GC noise.
const LEAKY_DOC: &str = r#"<html><body><script>
    (function () {
        const payload = new Array(130000).fill('xxxxxxxx');
        window.addEventListener('resize', function () { return payload.length; });
        window.__appState = payload;
    })();
</script></body></html>"#;

const ITERATIONS: usize = 25;

/// Returns live heap (bytes) after `ITERATIONS` reloads, sampled at the start
/// (post-warmup) and at the end.
async fn run_reuse_loop(reset_between: bool) -> (usize, usize) {
    let mut page = Page::from_html(LEAKY_DOC, None::<StealthProfile>)
        .await
        .unwrap();

    // Warm up: let one-off bootstrap allocations settle so the baseline
    // reading is not dominated by first-load noise.
    for _ in 0..3 {
        if reset_between {
            page.reset_for_reuse();
        }
        page.reload_html(LEAKY_DOC, "about:blank");
    }
    page.collect_garbage();
    let start = page.v8_heap_used_bytes();

    for _ in 0..ITERATIONS {
        if reset_between {
            page.reset_for_reuse();
        }
        page.reload_html(LEAKY_DOC, "about:blank");
    }
    page.collect_garbage();
    let end = page.v8_heap_used_bytes();

    (start, end)
}

/// With the reset in place, live heap must not grow meaningfully across
/// reuses. Threshold is deliberately loose (bytes-per-iteration well under the
/// ~1 MB each document retains) so this asserts "not leaking", not an exact
/// allocation profile.
#[tokio::test]
async fn heap_is_flat_across_warm_reuses() {
    let (start, end) = run_reuse_loop(true).await;
    let growth = end.saturating_sub(start);
    let per_iter = growth / ITERATIONS;

    println!("with reset:    start={start} end={end} growth={growth} ({per_iter} B/iteration)");

    assert!(
        per_iter < 200_000,
        "live V8 heap grew {per_iter} B per warm reuse (start={start}, end={end}); \
         each document retains ~1 MB, so this indicates the reset is not \
         releasing the previous page"
    );
}

/// The control arm: without the reset — i.e. 0.1.0's behaviour — the same
/// workload must leak. If this ever stops leaking, the test above has become
/// vacuous and is no longer proving anything.
#[tokio::test]
async fn heap_grows_without_reset_control_arm() {
    let (start, end) = run_reuse_loop(false).await;
    let growth = end.saturating_sub(start);
    let per_iter = growth / ITERATIONS;

    println!("without reset: start={start} end={end} growth={growth} ({per_iter} B/iteration)");

    assert!(
        per_iter > 200_000,
        "control arm did not leak ({per_iter} B/iteration) — the A/B test above \
         is no longer meaningful, because reuse-without-reset is supposed to \
         reproduce the 0.1.0 leak"
    );
}
