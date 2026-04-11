//! End-to-end test that the JS-side OfflineAudioContext.startRendering()
//! returns the Blink-compatible audio samples via the Rust op
//! `op_offline_audio_render`. Mirrors the exact JS call pattern the adidas
//! Akamai sensor VM uses.
//!
//! Run: cargo test -p js_runtime --test audio_fingerprint -- --test-threads=1 --nocapture

use js_runtime::BrowserJsRuntime;
use std::time::Duration;

const CHROME_REFERENCE_SUM: f64 = 124.04347527516074;

#[test]
fn offline_audio_context_renders_via_rust_op() {
    let dom =
        html_parser::parse_html("<html><head></head><body><div id=\"out\"></div></body></html>");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let mut runtime = BrowserJsRuntime::new(dom);

        // Reproduce the exact CreepJS/FingerprintJS probe.
        runtime
            .execute_script(
                r#"
                (async function () {
                    // Matches the CreepJS / FingerprintJS / adidas Akamai
                    // audio probe: default compressor parameters, 10 kHz
                    // triangle oscillator, 5000-sample mono render at 44100.
                    const ctx = new OfflineAudioContext(1, 5000, 44100);
                    const osc = ctx.createOscillator();
                    osc.type = "triangle";
                    osc.frequency.value = 10000;
                    const comp = ctx.createDynamicsCompressor();
                    // Don't override comp params — Blink defaults are
                    // threshold=-24, knee=30, ratio=12, attack=0.003, release=0.25.
                    osc.connect(comp);
                    comp.connect(ctx.destination);
                    osc.start();
                    const buf = await ctx.startRendering();
                    const data = buf.getChannelData(0);
                    let sum = 0;
                    for (let i = 4500; i < 5000; i++) sum += Math.abs(data[i]);
                    globalThis.__audio_len = data.length;
                    globalThis.__audio_sum = sum;
                    globalThis.__audio_sample_0 = data[0];
                    globalThis.__audio_sample_4500 = data[4500];
                    globalThis.__audio_sample_4999 = data[4999];
                })();
                "#,
            )
            .unwrap();

        // Drain the microtask/promise queue.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if std::time::Instant::now() >= deadline {
                break;
            }
            let fut = Box::pin(runtime.run_event_loop());
            let _ = tokio::time::timeout(Duration::from_millis(100), fut).await;
            let sum_str = runtime
                .execute_script("String(globalThis.__audio_sum || 0)")
                .unwrap_or_default();
            if sum_str != "0" {
                break;
            }
        }

        let len: usize = runtime
            .execute_script("String(globalThis.__audio_len || 0)")
            .unwrap_or_default()
            .parse()
            .unwrap_or(0);
        let sum: f64 = runtime
            .execute_script("String(globalThis.__audio_sum || 0)")
            .unwrap_or_default()
            .parse()
            .unwrap_or(0.0);
        let s0 = runtime
            .execute_script("String(globalThis.__audio_sample_0)")
            .unwrap_or_default();
        let s4500 = runtime
            .execute_script("String(globalThis.__audio_sample_4500)")
            .unwrap_or_default();
        let s4999 = runtime
            .execute_script("String(globalThis.__audio_sample_4999)")
            .unwrap_or_default();

        println!("[audio] len={len}");
        println!("[audio] sum(abs(data[4500..5000])) = {sum}");
        println!("[audio] chrome reference            = {CHROME_REFERENCE_SUM}");
        println!(
            "[audio] delta                       = {}",
            (sum - CHROME_REFERENCE_SUM).abs()
        );
        println!("[audio] data[0]    = {s0}");
        println!("[audio] data[4500] = {s4500}");
        println!("[audio] data[4999] = {s4999}");

        assert_eq!(len, 5000, "buffer length should be 5000");
        // The Rust port lands within ~60 ppm of Chrome's reference; allow
        // 0.5 for float rounding / profile variation.
        let delta = (sum - CHROME_REFERENCE_SUM).abs();
        assert!(
            delta < 0.5,
            "sum {sum} is more than 0.5 off from Chrome reference"
        );
    });
}
