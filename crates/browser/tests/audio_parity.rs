//! Audio compressor parity vs captured Chrome 147 ground truth.
//!
//! Real Chrome 147 (macOS arm64, captured 2026-04-28):
//!   sum of |buf[4500..5000]| === 124.04348155876505
//!
//! CreepJS KnownAudio table accepts any value within a small cluster of
//! Chrome/Blink-emitted floats. We assert (a) the value is in a plausible
//! Blink-like range so a corpus lookup MAY match, and (b) the exact value
//! is reported so progress toward bit-equality can be measured.

use browser::Page;
use stealth;

#[tokio::test]
async fn audio_compressor_in_blink_range() {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    // Stash the rendered buffer on globalThis on first eval, read sum on second.
    page.evaluate(
        r#"
        const oac = new OfflineAudioContext(1, 5000, 44100);
        const osc = oac.createOscillator();
        osc.type = 'triangle'; osc.frequency.value = 10000;
        const comp = oac.createDynamicsCompressor();
        comp.threshold.value = -50; comp.knee.value = 40; comp.ratio.value = 12;
        comp.attack.value = 0; comp.release.value = 0.25;
        osc.connect(comp); comp.connect(oac.destination); osc.start(0);
        oac.startRendering().then(buf => {
            const data = buf.getChannelData(0);
            let sum = 0;
            for (let i = 4500; i < 5000; i++) sum += Math.abs(data[i]);
            globalThis.__audioSum = sum;
        });
        'rendered';
        "#,
    )
    .unwrap();
    // Drain microtasks via a separate eval (Promise.then microtask runs).
    page.evaluate("new Promise(r => r(0)); 'drained';").unwrap();
    let r = page.evaluate("String(globalThis.__audioSum)").unwrap_or_default();
    eprintln!("Engine audio compressor sum: {r}");
    eprintln!("Chrome 147 captured sum:    124.04348155876505");
    let parsed: f64 = r.parse().unwrap_or(0.0);
    assert!(parsed > 0.0, "audio sum must be > 0: {r}");
    // Blink cluster: sums seen on real Chrome are 124.043... ± small variance.
    assert!(
        parsed > 100.0 && parsed < 200.0,
        "audio sum must be in Blink cluster (100..200): got {parsed}"
    );
    let chrome_sum: f64 = 124.04348155876505;
    let delta = (parsed - chrome_sum).abs();
    eprintln!("Delta vs Chrome 147: {delta:.6e}");
}
