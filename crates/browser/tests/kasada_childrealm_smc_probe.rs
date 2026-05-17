//! K2-DIFF fix #2 — child-realm smc-operation micro-probe.
//!
//! Localized so far: ONLY `smc`, `dpv`, `esd.cpt` throw the sentinel
//! TypeError; the Kasada CALL handler (disassembled, rec 111) is
//! `l=e(n); if(l[h]&&l[h].I===l){...} else l.apply(o,_)` — the throw is
//! `l[h]` when `l===undefined`, i.e. the callable the probe invokes is
//! `undefined`. No host accessor recreates (zero identity flips, proven).
//! `smc` runs MediaSource codec checks inside the iframe CHILD realm.
//! This micro-probe runs, inside the child realm, the exact host
//! operations smc/cpt need and reports which yield `undefined` (the
//! callable Kasada would then `l[h]`-throw on). Pure diagnosis.
//!
//! Network, #[ignore]. Run:
//!   cargo test -p browser --test kasada_childrealm_smc_probe -- --ignored --test-threads=1 --nocapture

use browser::Page;
use std::time::Duration;

#[tokio::test]
#[ignore = "network: K2-DIFF fix#2 — child-realm smc/cpt host-op micro-probe"]
async fn childrealm_smc_probe() {
    let out_dir = "/tmp/kasada_tl";
    std::fs::create_dir_all(out_dir).ok();

    // Create an iframe (forces our child-realm path), then run the smc /
    // cpt host operations INSIDE contentWindow and record typeof of each
    // intermediate the Kasada probe would store in a VM slot.
    let probe_init = r##"
        (function () {
            globalThis.__cr = { steps: [], err: null };
            function go() {
              try {
                var ifr = document.createElement("iframe");
                document.documentElement.appendChild(ifr);
                var w = ifr.contentWindow;
                var R = globalThis.__cr;
                function S(label, fn) {
                    try {
                        var v = fn();
                        R.steps.push({ k: label, t: typeof v,
                            v: (v === undefined ? "undefined" : v === null ? "null"
                                : typeof v === "function" ? ("fn:" + (v.name||"?"))
                                : typeof v === "object" ? Object.prototype.toString.call(v)
                                : String(v).slice(0,40)) });
                    } catch (e) {
                        R.steps.push({ k: label, t: "THROW", v: String(e && e.message || e).slice(0,120) });
                    }
                }
                // ---- smc path (child realm) ----
                S("cw", function(){ return w; });
                S("cw.MediaSource", function(){ return w.MediaSource; });
                S("cw.MediaSource.isTypeSupported", function(){ return w.MediaSource && w.MediaSource.isTypeSupported; });
                S("cw.MediaSource.isTypeSupported('video/mp4')", function(){ return w.MediaSource.isTypeSupported("video/mp4"); });
                S("new cw.MediaSource()", function(){ return new w.MediaSource(); });
                S("cw.MediaRecorder", function(){ return w.MediaRecorder; });
                S("cw.MediaRecorder.isTypeSupported", function(){ return w.MediaRecorder && w.MediaRecorder.isTypeSupported; });
                S("cw.MediaRecorder.isTypeSupported('audio/x-m4a')", function(){ return w.MediaRecorder.isTypeSupported("audio/x-m4a"); });
                S("cw.HTMLMediaElement", function(){ return w.HTMLMediaElement; });
                S("cw.HTMLMediaElement.prototype.canPlayType", function(){ return w.HTMLMediaElement && w.HTMLMediaElement.prototype && w.HTMLMediaElement.prototype.canPlayType; });
                S("cw.document", function(){ return w.document; });
                S("cw.document.createElement", function(){ return w.document && w.document.createElement; });
                S("cw.document.createElement('video')", function(){ return w.document.createElement("video"); });
                S("video.canPlayType", function(){ var v=w.document.createElement("video"); return v.canPlayType; });
                S("video.canPlayType('video/mp4')", function(){ var v=w.document.createElement("video"); return v.canPlayType("video/mp4"); });
                // ---- cpt path (canvas 2D paint) ----
                S("cw.document.createElement('canvas')", function(){ return w.document.createElement("canvas"); });
                S("canvas.getContext", function(){ var c=w.document.createElement("canvas"); return c.getContext; });
                S("canvas.getContext('2d')", function(){ var c=w.document.createElement("canvas"); return c.getContext("2d"); });
                S("ctx.measureText", function(){ var c=w.document.createElement("canvas"); var x=c.getContext("2d"); return x && x.measureText; });
                S("ctx.measureText('x')", function(){ var c=w.document.createElement("canvas"); var x=c.getContext("2d"); return x.measureText("x"); });
                S("ctx.getImageData", function(){ var c=w.document.createElement("canvas"); var x=c.getContext("2d"); return x && x.getImageData; });
                S("canvas.toDataURL", function(){ var c=w.document.createElement("canvas"); return c.toDataURL; });
                S("canvas.toDataURL()", function(){ var c=w.document.createElement("canvas"); return c.toDataURL(); });
                // ---- dpv path (devtools/debugger present heuristics) ----
                S("cw.Function.prototype.toString", function(){ return w.Function.prototype.toString; });
                S("cw.console", function(){ return w.console; });
                S("cw.console.debug", function(){ return w.console && w.console.debug; });
                S("cw.performance", function(){ return w.performance; });
                S("cw.performance.now", function(){ return w.performance && w.performance.now; });
                S("cw.Error", function(){ return w.Error; });
                S("new cw.Error().stack", function(){ return new w.Error().stack; });
                // also: same ops in MAIN realm for contrast
                S("main.MediaSource.isTypeSupported('video/mp4')", function(){ return MediaSource.isTypeSupported("video/mp4"); });
                S("main.canvas.getContext('2d')", function(){ var c=document.createElement("canvas"); return c.getContext("2d"); });
              } catch (e) { globalThis.__cr.err = String(e && e.stack || e); }
            }
            // run after DOM is ready
            if (document.readyState === "loading") {
                document.addEventListener("DOMContentLoaded", go);
            } else { go(); }
            setTimeout(go, 1500);
        })();
    "##;

    let res = tokio::time::timeout(
        Duration::from_secs(180),
        Page::navigate_with_init(
            "https://www.hyatt.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![probe_init.to_string()],
        ),
    )
    .await;

    let mut p = match res {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            println!("[cr] navigate errored: {e}");
            return;
        }
        Err(_) => {
            println!("[cr] navigate timed out (180s)");
            return;
        }
    };
    for _ in 0..40 {
        let _ = p.event_loop().run_until_idle(Duration::from_millis(250)).await;
    }

    let dump = p
        .evaluate("JSON.stringify(globalThis.__cr || {})")
        .unwrap_or_default();
    let dump = dump
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/childrealm_smc_probe.json"), &dump).ok();
    println!("[cr] === child-realm smc/cpt/dpv host-op results ===");
    println!("{}", &dump[..dump.len().min(6000)]);
}
