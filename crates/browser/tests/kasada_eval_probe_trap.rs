//! K2-DIFF fix #2 — eval'd probe-source trap (final localization).
//!
//! Precise decoded set (K2DIFF_RESULT.md fix #2): EXACTLY 3 sensor fields
//! carry `TypeError: Cannot read properties of undefined (reading
//! 'unjzomuybtbyyhwwkdpkxomylnab')` — `smc`, `dpv`, `esd.cpt`. The
//! fullest stack (`esd.cpt`) is:
//!   at eval (<anonymous>:3:66)  ←  at U (/…/ips.js…)
//! i.e. ips.js's helper `U` `eval`s a tiny generated function and the
//! throw is at col 66 of that eval'd `<anonymous>` source. The earlier
//! sentinel/accessor traps proved NO host accessor flips fn→undefined
//! (candidates #1/#3 ruled out) and the 551 sentinel SETs are ips.js's
//! OWN VM fns (main realm). So the `r` register that is `undefined` at
//! `r[sentinel]` is set from a host symbol the eval'd probe references
//! that our engine resolves to `undefined`.
//!
//! This test records EVERY string ips.js passes to `eval` / the
//! `Function` constructor (main + child realm), tagged with whether
//! evaluating it threw the sentinel TypeError, so we can read the exact
//! `smc`/`dpv`/`cpt` probe source and name the missing host symbol.
//!
//! Network, #[ignore]. Run:
//!   cargo test -p browser --test kasada_eval_probe_trap -- --ignored --test-threads=1 --nocapture

use browser::Page;
use std::time::Duration;

#[tokio::test]
#[ignore = "network: K2-DIFF fix#2 — capture the eval'd smc/dpv/cpt probe source"]
async fn trap_evald_probe_source() {
    let out_dir = "/tmp/kasada_tl";
    std::fs::create_dir_all(out_dir).ok();

    let trap_init = r##"
        (function () {
            var SENT = "unjzomuybtbyyhwwkdpkxomylnab";
            var L = globalThis.__evp = { evals: [], fns: [], notes: [] };

            function record(bucket, src, realm) {
                try {
                    if (typeof src !== "string") return;
                    if (bucket.length > 1200) return;
                    bucket.push({ realm: realm, len: src.length, src: src.slice(0, 1400) });
                } catch (e) {}
            }

            // Wrap eval so we see ips.js's `U`-helper generated sources.
            // We DO NOT change semantics: call the original, record around
            // it, and re-throw. We also tag the record if the thrown error
            // mentions the per-build sentinel (that IS the smc/dpv/cpt one).
            function wrapEval(g, realm) {
                try {
                    var _eval = g.eval;
                    var w = function (s) {
                        var idx = -1;
                        try {
                            if (typeof s === "string") {
                                idx = L.evals.length;
                                record(L.evals, s, realm);
                            }
                        } catch (e) {}
                        try {
                            return _eval.call(this, s);
                        } catch (err) {
                            try {
                                var m = String(err && err.message || err);
                                if (idx >= 0 && L.evals[idx] && m.indexOf(SENT) !== -1) {
                                    L.evals[idx].THREW_SENTINEL = true;
                                    L.evals[idx].errStack = String(err && err.stack || "").slice(0, 600);
                                }
                            } catch (e2) {}
                            throw err;
                        }
                    };
                    // keep it looking native-ish; ips.js may toString it
                    try { Object.defineProperty(w, "name", { value: "eval", configurable: true }); } catch (e) {}
                    try { Object.defineProperty(w, "length", { value: 1, configurable: true }); } catch (e) {}
                    g.eval = w;
                    L.notes.push("wrapped eval realm=" + realm);
                } catch (e) { L.notes.push("wrapEval ERR " + realm + ": " + e); }

                // Wrap the Function constructor too (Kasada also builds
                // probe fns via `Function("...")`).
                try {
                    var _F = g.Function;
                    var WF = function () {
                        try {
                            var a = Array.prototype.slice.call(arguments);
                            record(L.fns, a.join(" || "), realm);
                        } catch (e) {}
                        return _F.apply(this, arguments);
                    };
                    WF.prototype = _F.prototype;
                    try { Object.defineProperty(WF, "name", { value: "Function", configurable: true }); } catch (e) {}
                    g.Function = WF;
                    L.notes.push("wrapped Function realm=" + realm);
                } catch (e) { L.notes.push("wrapFunction ERR " + realm + ": " + e); }
            }

            wrapEval(globalThis, "main");

            // Hook iframe contentWindow to wrap the child realm's eval too
            // (smc runs there).
            try {
                var IF = globalThis.HTMLIFrameElement;
                if (IF && IF.prototype) {
                    var d = Object.getOwnPropertyDescriptor(IF.prototype, "contentWindow");
                    if (d && d.get) {
                        var og = d.get;
                        Object.defineProperty(IF.prototype, "contentWindow", {
                            configurable: true, enumerable: d.enumerable,
                            get: function () {
                                var w = og.call(this);
                                try {
                                    if (w && !w.__evpHooked) {
                                        w.__evpHooked = true;
                                        wrapEval(w, "child");
                                    }
                                } catch (e) { L.notes.push("cw hook ERR: " + e); }
                                return w;
                            },
                            set: d.set
                        });
                    }
                }
            } catch (e) { L.notes.push("iframe hook ERR: " + e); }
        })();
    "##;

    let res = tokio::time::timeout(
        Duration::from_secs(180),
        Page::navigate_with_init(
            "https://www.hyatt.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![trap_init.to_string()],
        ),
    )
    .await;

    let mut p = match res {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            println!("[evp] navigate errored: {e}");
            return;
        }
        Err(_) => {
            println!("[evp] navigate timed out (180s)");
            return;
        }
    };
    for _ in 0..48 {
        let _ = p.event_loop().run_until_idle(Duration::from_millis(250)).await;
    }

    // First: only the eval'd sources that THREW the sentinel TypeError —
    // these ARE the smc/dpv/cpt probe bodies.
    let threw = p
        .evaluate(
            r#"JSON.stringify(((globalThis.__evp&&globalThis.__evp.evals)||[]).filter(function(e){return e.THREW_SENTINEL;}))"#,
        )
        .unwrap_or_default();
    let threw = threw
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/evp_threw.json"), &threw).ok();
    println!("[evp] === eval'd sources that THREW the sentinel TypeError ===");
    println!("{}", &threw[..threw.len().min(8000)]);

    let summary = p
        .evaluate(
            r#"(function(){
                var L = globalThis.__evp || {evals:[],fns:[],notes:[]};
                return JSON.stringify({
                    evalCount: L.evals.length,
                    fnCount: L.fns.length,
                    threwCount: L.evals.filter(function(e){return e.THREW_SENTINEL;}).length,
                    notes: L.notes
                });
            })()"#,
        )
        .unwrap_or_default();
    println!(
        "[evp] summary: {}",
        summary.trim_matches('"').replace("\\\"", "\"")
    );

    // Persist the full eval + Function corpus for offline drill-down.
    let all = p
        .evaluate(r#"JSON.stringify((globalThis.__evp&&globalThis.__evp.evals)||[])"#)
        .unwrap_or_default();
    let all = all
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/evp_all_evals.json"), &all).ok();
    let fns = p
        .evaluate(r#"JSON.stringify((globalThis.__evp&&globalThis.__evp.fns)||[])"#)
        .unwrap_or_default();
    let fns = fns
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/evp_all_fns.json"), &fns).ok();
    println!(
        "[evp] full corpus -> {out_dir}/evp_all_evals.json ({} b), {out_dir}/evp_all_fns.json ({} b)",
        all.len(),
        fns.len()
    );
}
