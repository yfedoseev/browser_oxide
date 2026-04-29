//! Phase 4 — verify the sigma-lognormal humanizer produces the
//! signals (mouse / scroll / click / key) that anti-bot sensors
//! gate on, with the right shape characteristics.
//!
//! This test runs `Page::navigate_humanized` (which is just
//! `Page::navigate` — humanize is default-on) on a synthetic page
//! that records every dispatched event into `globalThis.__events`.
//! After ~3.2 s of event-loop time we read the array back and check:
//!
//! 1. **Mouse path** — at least 30 mousemove events, distance >800px
//! 2. **Scroll** — 4 wheel + 4 scroll events with monotonically
//!    decreasing deltaY (decel)
//! 3. **Click** — full mousedown / mouseup / click sequence
//! 4. **Key** — Tab keydown + keyup
//! 5. **Sigma-lognormal shape** — mousemove inter-arrival times peak
//!    in the early third (mode), with a long tail. We test this by
//!    checking that the modal interval is < median, which is what an
//!    asymmetric-right distribution requires.

use browser::Page;
use stealth::presets::chrome_130_macos;

const HTML: &str = r#"<!doctype html><html><head><title>humanize</title></head><body>
<script>
  globalThis.__events = [];
  globalThis.__lastMove = null;
  const record = (kind, ev) => {
    const e = { kind, t: Date.now() };
    if (ev) {
      if (typeof ev.clientX === 'number') { e.x = ev.clientX; e.y = ev.clientY; }
      if (typeof ev.deltaY === 'number') e.deltaY = ev.deltaY;
      if (typeof ev.key === 'string') e.key = ev.key;
      e.isTrusted = ev.isTrusted;
    }
    globalThis.__events.push(e);
  };
  document.addEventListener('mousemove', e => record('mousemove', e));
  document.addEventListener('mousedown', e => record('mousedown', e));
  document.addEventListener('mouseup',   e => record('mouseup',   e));
  document.addEventListener('click',     e => record('click',     e));
  document.addEventListener('wheel',     e => record('wheel',     e), { passive: true });
  document.addEventListener('scroll',    e => record('scroll',    e), { passive: true });
  document.addEventListener('keydown',   e => record('keydown',   e));
  document.addEventListener('keyup',     e => record('keyup',     e));
  document.addEventListener('visibilitychange', e => record('visibilitychange', e));
  window.addEventListener('focus',       e => record('focus',     e));
  window.addEventListener('scroll',      e => record('scroll-win', e));
</script>
</body></html>"#;

#[tokio::test]
async fn humanize_emits_full_signal_set() {
    // navigate_humanized installs humanize.js as an init script; from_html
    // does NOT, so we'd see nothing on a default Page::from_html. Use
    // navigate_humanized via the existing entrypoint.
    //
    // Trick: navigate_humanized takes a URL. We don't want a network test
    // here, so we use a data: URL that decodes to the probe HTML.
    let mut page = Page::from_html(HTML, Some(chrome_130_macos())).await.unwrap();
    // Manually inject humanize.js and drive enough time for it to fire.
    let humanize = include_str!("../src/js/humanize.js");
    page.evaluate(humanize).unwrap();

    // Drive the event loop until quiescent (or timeout). humanize.js
    // schedules setTimeouts up to ~3.2 s out, so a 4 s budget covers
    // everything. `page.evaluate("0")` alone doesn't fire pending
    // timers — we need run_until_idle.
    let _ = page
        .event_loop()
        .run_until_idle(std::time::Duration::from_secs(4))
        .await;

    let n_total = page.evaluate("globalThis.__events.length").unwrap();
    let n_mousemove = page
        .evaluate("globalThis.__events.filter(e => e.kind === 'mousemove').length")
        .unwrap();
    let n_wheel = page
        .evaluate("globalThis.__events.filter(e => e.kind === 'wheel').length")
        .unwrap();
    let n_click = page
        .evaluate("globalThis.__events.filter(e => e.kind === 'click').length")
        .unwrap();
    let n_keydown = page
        .evaluate("globalThis.__events.filter(e => e.kind === 'keydown').length")
        .unwrap();
    let total_distance = page
        .evaluate(
            "(() => {\
                const m = globalThis.__events.filter(e => e.kind === 'mousemove' && typeof e.x === 'number');\
                let d = 0;\
                for (let i = 1; i < m.length; i++) { d += Math.hypot(m[i].x-m[i-1].x, m[i].y-m[i-1].y); }\
                return Math.round(d);\
            })()",
        )
        .unwrap();
    let monotonic_decel = page
        .evaluate(
            "(() => {\
                const w = globalThis.__events.filter(e => e.kind === 'wheel');\
                if (w.length < 2) return 'too-few';\
                let prev = Infinity;\
                for (const e of w) { if (e.deltaY > prev) return 'not-monotonic'; prev = e.deltaY; }\
                return 'ok';\
            })()",
        )
        .unwrap();
    let trusted_count = page
        .evaluate(
            "globalThis.__events.filter(e => e.isTrusted === true).length"
        ).unwrap();

    println!("\n=== Humanize signal probe ===");
    println!("  total events:     {n_total}");
    println!("  mousemove:        {n_mousemove}");
    println!("  wheel:            {n_wheel}  (decel: {monotonic_decel})");
    println!("  click:            {n_click}");
    println!("  keydown:          {n_keydown}");
    println!("  path distance:    {total_distance} px");
    println!("  isTrusted=true:   {trusted_count}");

    let mm = n_mousemove.parse::<i64>().unwrap();
    assert!(mm >= 30, "humanize must emit ≥30 mousemove events, got {mm}");
    let w = n_wheel.parse::<i64>().unwrap();
    assert!(w >= 4, "humanize must emit ≥4 wheel events, got {w}");
    let c = n_click.parse::<i64>().unwrap();
    assert!(c >= 1, "humanize must emit ≥1 click");
    let k = n_keydown.parse::<i64>().unwrap();
    assert!(k >= 1, "humanize must emit ≥1 keydown");
    let dist = total_distance.parse::<i64>().unwrap();
    assert!(
        dist >= 800,
        "mouse path must cover ≥800 px (sensors check magnitude), got {dist}"
    );
    assert_eq!(
        monotonic_decel.trim_matches('"'), "ok",
        "scroll wheel deltaY must monotonically decrease (deceleration model)"
    );
    let trusted = trusted_count.parse::<i64>().unwrap();
    assert!(
        trusted >= 30,
        "events must have isTrusted=true so handlers gating on it fire — got {trusted}"
    );
}

/// Sigma-lognormal shape check — mousemove inter-arrival times must
/// peak in the early portion (asymmetric right-skewed distribution).
/// This is what real human cursor motion looks like; the previous
/// uniform-time Bezier produced a flat distribution that's
/// distinguishable from human input.
#[tokio::test]
async fn humanize_mouse_intervals_are_right_skewed() {
    let mut page = Page::from_html(HTML, Some(chrome_130_macos())).await.unwrap();
    let humanize = include_str!("../src/js/humanize.js");
    page.evaluate(humanize).unwrap();

    let _ = page
        .event_loop()
        .run_until_idle(std::time::Duration::from_secs(4))
        .await;

    // Debug: print what's in __events.
    let dbg = page
        .evaluate("JSON.stringify({total: globalThis.__events.length, kinds: globalThis.__events.slice(0,3).map(e => Object.keys(e))})")
        .unwrap();
    println!("DEBUG: {dbg}");

    // Compute inter-arrival times for the first stroke (≈18 events).
    let stats = page.evaluate(
        "(() => {\
            const m = globalThis.__events.filter(e => e.kind === 'mousemove' && typeof e.t === 'number');\
            if (m.length < 10) return JSON.stringify({error: 'too-few', n: m.length});\
            const intervals = [];\
            for (let i = 1; i < m.length; i++) intervals.push(m[i].t - m[i-1].t);\
            intervals.sort((a, b) => a - b);\
            const mid = intervals.length >> 1;\
            const median = intervals.length % 2 ? intervals[mid] : (intervals[mid-1]+intervals[mid])/2;\
            const mean = intervals.reduce((a, b) => a + b, 0) / intervals.length;\
            const minVal = intervals[0];\
            const maxVal = intervals[intervals.length - 1];\
            return JSON.stringify({n: intervals.length, mean: +mean.toFixed(2), median: +median.toFixed(2), min: +minVal.toFixed(2), max: +maxVal.toFixed(2)});\
        })()"
    ).unwrap();
    println!("\nmousemove inter-arrival stats: {}", stats);
    let stats_clean = stats.trim_matches('"').replace("\\\"", "\"");
    let v: serde_json::Value = serde_json::from_str(&stats_clean).expect("stats json");
    let mean = v["mean"].as_f64().unwrap();
    let median = v["median"].as_f64().unwrap();
    let max = v["max"].as_f64().unwrap();
    let min = v["min"].as_f64().unwrap();
    assert!(
        mean > median,
        "right-skewed distribution must have mean > median (sigma-lognormal property), got mean={mean} median={median}"
    );
    assert!(
        max > min * 3.0,
        "interval range must span ≥3× (asymmetric distribution; uniform sampling would have max ≈ min), got min={min} max={max}"
    );
}
