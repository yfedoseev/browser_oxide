//! K2-DIFF fix #2 — child-realm prototype-surface delta probe.
//!
//! Prior micro-probe found `cw.HTMLMediaElement.prototype.canPlayType`
//! === undefined in the child realm (instance method works, prototype
//! method missing). The Kasada CALL handler throws `l[sentinel]` when
//! `l` (the callable fetched via the VM) is `undefined` — a prototype
//! method that is `undefined` is exactly such an `l`. This probe dumps,
//! for the media/canvas/devtools prototypes the smc/dpv/cpt probes
//! walk, which methods are present on the MAIN realm but `undefined`
//! on the CHILD realm prototype (the precise missing callables).
//!
//! Network, #[ignore]. Run:
//!   cargo test -p browser --test kasada_proto_surface_probe -- --ignored --test-threads=1 --nocapture

use browser::Page;
use std::time::Duration;

#[tokio::test]
#[ignore = "network: K2-DIFF fix#2 — child-realm prototype-surface delta"]
async fn proto_surface_delta() {
    let out_dir = "/tmp/kasada_tl";
    std::fs::create_dir_all(out_dir).ok();

    let probe_init = r##"
        (function () {
            globalThis.__ps = { delta: [], err: null };
            function go() {
              try {
                var ifr = document.createElement("iframe");
                document.documentElement.appendChild(ifr);
                var w = ifr.contentWindow;
                var R = globalThis.__ps;
                // For each constructor, compare own+proto method presence
                // main vs child.
                var ctors = ["MediaSource","MediaRecorder","HTMLMediaElement",
                             "HTMLVideoElement","HTMLAudioElement","HTMLCanvasElement",
                             "CanvasRenderingContext2D","Notification","Navigator",
                             "Performance","Screen","Error"];
                ctors.forEach(function(cn){
                    var mc = globalThis[cn], cc = w[cn];
                    var row = { ctor: cn,
                        main: typeof mc, child: typeof cc,
                        mainProto: (mc && mc.prototype) ? "obj" : String(mc && mc.prototype),
                        childProto: (cc && cc.prototype) ? "obj" : String(cc && cc.prototype),
                        missingOnChildProto: [], presentBothProto: 0 };
                    try {
                        if (mc && mc.prototype) {
                            var names = Object.getOwnPropertyNames(mc.prototype);
                            names.forEach(function(nm){
                                var mv;
                                try { mv = mc.prototype[nm]; } catch(e){ mv = undefined; }
                                if (typeof mv !== "function") return;
                                var cv;
                                try { cv = cc && cc.prototype ? cc.prototype[nm] : undefined; } catch(e){ cv = undefined; }
                                if (typeof cv === "function") row.presentBothProto++;
                                else row.missingOnChildProto.push(nm + "(" + typeof cv + ")");
                            });
                        }
                    } catch (e) { row.err = String(e); }
                    R.delta.push(row);
                });
                // Also: static method presence delta (MediaSource.isTypeSupported etc.)
                var staticChecks = [
                    ["MediaSource","isTypeSupported"], ["MediaRecorder","isTypeSupported"],
                    ["Notification","requestPermission"], ["Error","captureStackTrace"]
                ];
                R.staticDelta = staticChecks.map(function(p){
                    var m = globalThis[p[0]], c = w[p[0]];
                    return { ctor:p[0], m: m && typeof m[p[1]], c: c && typeof (c[p[1]]),
                             method:p[1] };
                });
              } catch (e) { globalThis.__ps.err = String(e && e.stack || e); }
            }
            if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", go);
            else go();
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
            println!("[ps] navigate errored: {e}");
            return;
        }
        Err(_) => {
            println!("[ps] navigate timed out (180s)");
            return;
        }
    };
    for _ in 0..40 {
        let _ = p.event_loop().run_until_idle(Duration::from_millis(250)).await;
    }

    let dump = p
        .evaluate("JSON.stringify(globalThis.__ps || {})")
        .unwrap_or_default();
    let dump = dump
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/proto_surface_delta.json"), &dump).ok();
    println!("[ps] === child-realm prototype-surface delta (missing callables) ===");
    println!("{}", &dump[..dump.len().min(7000)]);
}
