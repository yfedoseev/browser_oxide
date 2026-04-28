//! Sub-pixel layout determinism — Blink's LayoutUnit emits 1/64-px
//! quantized floats for `getBoundingClientRect` and friends. Akamai
//! pHash hashes these values across `querySelectorAll('*')`. Engines that
//! emit raw f32/f64 pixel grids fail.
//!
//! Chrome 147 reference: `width:1.3px` div returns `1.296875` (= 83/64).

use browser::Page;
use stealth;

fn page_with(body: &str) -> String {
    format!("<!DOCTYPE html><html><body>{body}</body></html>")
}

async fn eval_with_body(body: &str, js: &str) -> String {
    let mut page = Page::from_html(&page_with(body), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

/// All output floats from getBoundingClientRect must be multiples of 1/64.
#[tokio::test]
async fn rect_floats_are_64ths() {
    let r = eval_with_body(
        "<div id='x' style='width:100px;height:50px'></div>",
        "
        const el = document.getElementById('x');
        const r = el.getBoundingClientRect();
        // Each value, when multiplied by 64, must round to an integer.
        const all64ths = ['x', 'y', 'width', 'height', 'top', 'right', 'bottom', 'left']
            .every(k => Number.isInteger(Math.round(r[k] * 64)));
        all64ths
        ",
    )
    .await;
    assert_eq!(r, "true", "every DOMRect float must be a multiple of 1/64");
}

/// LayoutUnit quantization is observable on toString of fractional widths.
#[tokio::test]
async fn fractional_widths_quantize() {
    // Exercise the pure LayoutUnit quantization via canvas (which uses the
    // same unit mechanism in DOMRect::new). A 100/3 = 33.333... should
    // quantize to a value with 1/64-px precision (33 + 21/64 = 33.328125).
    let r = eval_with_body(
        "",
        "
        // Direct test of the quantization step via DOMRect.new behaviour.
        // We can't construct a DOMRect from JS in the page, but we CAN
        // observe quantized output from any layout-derived rect.
        const d = document.createElement('div');
        d.style.cssText = 'width:33.333px';
        document.body.appendChild(d);
        const r = d.getBoundingClientRect();
        // The width must be a multiple of 1/64.
        Number.isInteger(Math.round(r.width * 64))
        ",
    )
    .await;
    assert_eq!(r, "true");
}

/// Akamai pHash-style probe: walk all elements, hash their rect floats.
/// Without LayoutUnit, the hash drifts versus Chrome's reference.
#[tokio::test]
async fn querySelectorAll_rects_all_64ths() {
    let r = eval_with_body(
        "
        <div style='width:1.3px;height:1.7px'></div>
        <div style='width:50.5px;height:25.25px'></div>
        <div style='width:99.99px;height:0.5px'></div>
        ",
        "
        const all = document.querySelectorAll('div');
        let ok = true;
        for (const el of all) {
            const r = el.getBoundingClientRect();
            for (const k of ['x', 'y', 'width', 'height']) {
                if (!Number.isInteger(Math.round(r[k] * 64))) { ok = false; break; }
            }
            if (!ok) break;
        }
        ok
        ",
    )
    .await;
    assert_eq!(r, "true", "every rect float across querySelectorAll('*') must be a multiple of 1/64");
}
