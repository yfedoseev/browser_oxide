//! K2-DIFF fix #2 — TARGETED smc/dpv accessor trap (the localization step).
//!
//! Established (K2DIFF_RESULT.md fix #2 + kasada_sentinel_trap):
//!  - decoded live sensor: `smc` (video/mp4, audio/x-m4a, audio/acc) and
//!    `dpv` (devtools probe) both throw the IDENTICAL
//!    `TypeError: Cannot read properties of undefined
//!     (reading 'unjzomuybtbyyhwwkdpkxomylnab')`.
//!  - `unjzomuybtbyyhwwkdpkxomylnab` is Kasada's per-build sentinel; ips.js
//!    tags a native `Function` via its VM (879 SET calls, ALL on Function),
//!    then re-reads `obj.<sentinel>`; the throw ⇒ `obj` is `undefined` at
//!    the read (the re-fetched value is undefined, not the tagged fn).
//!  - `_defProtoMethod` is identity-stable (fixed data prop) ⇒ ruled out.
//!  - so: an accessor on the smc/dpv path returns a fn on first access but
//!    `undefined` (or a different object) on a later access.
//!
//! This test wraps ONLY the smc/dpv-relevant native accessors and logs,
//! per access: the receiver, the returned value's typeof, and whether it
//! is the SAME object identity as the prior access for that key. The
//! accessor that flips fn→undefined (or fn→other-identity) IS the bug.
//!
//! It also re-installs the Object.prototype sentinel GET trap (records
//! reads on `undefined`-class receivers is impossible, but reads on a
//! tagged object whose tag is missing ARE recorded) and a native-TypeError
//! sniffer that records the message + a best-effort receiver hint.
//!
//! Network, #[ignore]. Run:
//!   cargo test -p browser --test kasada_smc_dpv_trap -- --ignored --test-threads=1 --nocapture

use browser::Page;
use std::time::Duration;

#[tokio::test]
#[ignore = "network: K2-DIFF fix#2 — localize the smc/dpv fn->undefined accessor"]
async fn trap_smc_dpv_accessor() {
    let out_dir = "/tmp/kasada_tl";
    std::fs::create_dir_all(out_dir).ok();

    // The instrumentation runs BEFORE ips.js. It:
    //  (1) For each smc/dpv-relevant accessor, installs a logging wrapper
    //      that records every read's return typeof + identity-vs-prior.
    //      We cover BOTH the main realm and (via a hook on iframe
    //      contentWindow access) the child realm where smc/dpv run.
    //  (2) Re-installs the Object.prototype sentinel accessor: `set`
    //      shadows w/ a real own prop (so a re-read on the SAME tagged
    //      object succeeds — isolating "re-read on a DIFFERENT/undefined
    //      object" as the only remaining failure mode); `get` logs the
    //      receiver class for tagged objects whose own tag is missing.
    //  (3) Wraps the native error path: a Proxy on `globalThis` is too
    //      invasive; instead we patch `Error.captureStackTrace`-free V8 by
    //      wrapping `Function.prototype.call/apply` is too broad — so we
    //      rely on (1)+(2) which directly localize the accessor.
    let trap_init = r#"
        (function () {
            var NAME = "unjzomuybtbyyhwwkdpkxomylnab";
            var L = globalThis.__smcdpv = {
                acc: [],          // accessor read log: {k, n, t, sameAsPrev, viaUndef}
                sentinelSets: 0,
                sentinelGetMiss: [], // reads where tagged obj lost its tag
                notes: []
            };
            var _prev = Object.create(null);   // key -> last returned value
            var _count = Object.create(null);  // key -> read count

            function logRead(key, val) {
                try {
                    var n = (_count[key] = (_count[key] | 0) + 1);
                    var same = (key in _prev) ? (_prev[key] === val) : null;
                    var prevT = (key in _prev) ? (typeof _prev[key]) : "(none)";
                    _prev[key] = val;
                    if (L.acc.length < 4000) {
                        L.acc.push({
                            k: key, n: n, t: typeof val,
                            v: (val === undefined ? "undefined"
                                : val === null ? "null"
                                : typeof val === "function" ? ("fn:" + (val.name || "?"))
                                : Object.prototype.toString.call(val)),
                            same: same, prevT: prevT
                        });
                    }
                } catch (e) {}
                return val;
            }

            // (A) Wrap a *data* or *accessor* property on `obj` named
            // `prop`, logging each GET as key `label`. Preserves the
            // original value/getter; identity is NOT changed (we wrap the
            // descriptor's get, returning the SAME underlying object).
            function wrapProp(obj, prop, label) {
                try {
                    if (!obj) { L.notes.push("wrapProp: no obj for " + label); return; }
                    var d = Object.getOwnPropertyDescriptor(obj, prop);
                    var proto = obj;
                    while (!d && proto && proto !== Object.prototype) {
                        proto = Object.getPrototypeOf(proto);
                        if (proto) d = Object.getOwnPropertyDescriptor(proto, prop);
                    }
                    if (!d) { L.notes.push("wrapProp: no desc for " + label + " (" + prop + ")"); return; }
                    var host = (proto && proto !== obj) ? proto : obj;
                    if (d.get) {
                        var g = d.get;
                        Object.defineProperty(host, prop, {
                            configurable: true, enumerable: d.enumerable,
                            get: function () { return logRead(label, g.call(this)); },
                            set: d.set
                        });
                        L.notes.push("wrapped GETTER " + label);
                    } else if ("value" in d) {
                        // data prop: log the VALUE each time it is read via a
                        // converted accessor that returns the SAME stored value.
                        var stored = d.value;
                        Object.defineProperty(host, prop, {
                            configurable: true, enumerable: d.enumerable,
                            get: function () { return logRead(label, stored); },
                            set: function (nv) { stored = nv; }
                        });
                        L.notes.push("wrapped DATA->accessor " + label + " (identity-stable: returns stored)");
                    }
                } catch (e) { L.notes.push("wrapProp ERR " + label + ": " + e); }
            }

            // (B) Sentinel Object.prototype accessor (proven design),
            // now with a hidden uid+realm stamp on SET so a GET-miss can
            // distinguish "SAME fn, own tag vanished" vs "DIFFERENT/cross-
            // realm fn never tagged here". REALM is set per-context.
            var UID = Symbol("smcdpv_uid");
            var REALM = "main";
            var _uidc = 0;
            L.setByRealm = {};
            L.missDetail = [];
            function installSentinel(REALM) {
              try {
                Object.defineProperty(Object.prototype, NAME, {
                    configurable: true,
                    set: function (v) {
                        L.sentinelSets++;
                        L.setByRealm[REALM] = (L.setByRealm[REALM]|0)+1;
                        try {
                            // stamp identity so GET-miss can prove same-obj
                            if (!this[UID]) {
                                Object.defineProperty(this, UID, {
                                    value: REALM + "@" + (++_uidc),
                                    configurable: true, enumerable: false
                                });
                            }
                            Object.defineProperty(this, NAME, {
                                value: v, writable: true, configurable: true, enumerable: false
                            });
                        } catch (e2) {}
                    },
                    get: function () {
                        try {
                            var uid = null;
                            try { uid = this ? this[UID] : null; } catch (e3) {}
                            if (L.sentinelGetMiss.length < 400) {
                                L.sentinelGetMiss.push({
                                    cls: Object.prototype.toString.call(this),
                                    ctor: (this && this.constructor && this.constructor.name) || (typeof this),
                                    nm: (typeof this === "function" ? (this.name || "?") : ""),
                                    readRealm: REALM,
                                    // uid present => THIS exact obj was sentinel-SET
                                    // earlier (in uid's realm) yet its own NAME
                                    // prop is gone now => tag did not persist.
                                    // uid null => never tagged in any trapped
                                    // realm => fresh/cross-realm object.
                                    taggedBefore: uid || null
                                });
                            }
                        } catch (e) {}
                        return undefined;
                    }
                });
                L.notes.push("sentinel installed in realm=" + REALM);
              } catch (e) { L.notes.push("sentinel install ERR (" + REALM + "): " + e); }
            }
            installSentinel("main");

            // (C) Targeted smc-path accessors (main realm). smc reads
            // MediaSource(.isTypeSupported) / MediaRecorder; dpv is the
            // devtools probe — both funnel through native-fn fetches.
            wrapProp(globalThis, "MediaSource", "g.MediaSource");
            wrapProp(globalThis, "MediaRecorder", "g.MediaRecorder");
            wrapProp(globalThis, "MediaSourceHandle", "g.MediaSourceHandle");
            try { wrapProp(globalThis.MediaSource, "isTypeSupported", "MediaSource.isTypeSupported"); } catch (e) {}
            try { wrapProp(globalThis.MediaRecorder, "isTypeSupported", "MediaRecorder.isTypeSupported"); } catch (e) {}
            // navigator.mediaDevices getter + its methods (UNJZOMUY cand #1)
            try {
                var NavProto = (globalThis.Navigator && globalThis.Navigator.prototype) || Object.getPrototypeOf(navigator);
                wrapProp(NavProto, "mediaDevices", "nav.mediaDevices");
            } catch (e) { L.notes.push("nav.mediaDevices wrap ERR: " + e); }
            try {
                var md = navigator.mediaDevices;
                if (md) {
                    wrapProp(md, "enumerateDevices", "md.enumerateDevices");
                    wrapProp(md, "getUserMedia", "md.getUserMedia");
                    wrapProp(md, "getDisplayMedia", "md.getDisplayMedia");
                    wrapProp(md, "getSupportedConstraints", "md.getSupportedConstraints");
                }
            } catch (e) { L.notes.push("md.* wrap ERR: " + e); }
            // dpv devtools-probe surface: Function.prototype.toString,
            // Error.prototype.stack getter, console.*, Date, performance.
            try { wrapProp(Function.prototype, "toString", "Fn.proto.toString"); } catch (e) {}
            try { wrapProp(Object.getPrototypeOf(new Error()), "stack", "Error.proto.stack"); } catch (e) {}
            try { wrapProp(globalThis, "chrome", "g.chrome"); } catch (e) {}

            // (D) Hook iframe contentWindow so we can also wrap the SAME
            // accessors in the child realm where smc/dpv actually run.
            try {
                var HTMLIFRAME = globalThis.HTMLIFrameElement;
                if (HTMLIFRAME && HTMLIFRAME.prototype) {
                    var cwDesc = Object.getOwnPropertyDescriptor(HTMLIFRAME.prototype, "contentWindow");
                    if (cwDesc && cwDesc.get) {
                        var origCw = cwDesc.get;
                        Object.defineProperty(HTMLIFRAME.prototype, "contentWindow", {
                            configurable: true, enumerable: cwDesc.enumerable,
                            get: function () {
                                var w = origCw.call(this);
                                try {
                                    if (w && !w.__smcdpvHooked) {
                                        w.__smcdpvHooked = true;
                                        L.notes.push("child realm hooked");
                                        // Install the sentinel trap on the
                                        // CHILD realm's Object.prototype too
                                        // (smc/dpv run here; cross-realm tag
                                        // loss would only show with this).
                                        try {
                                            if (w.Object && w.Object.prototype
                                                && !Object.getOwnPropertyDescriptor(w.Object.prototype, NAME)) {
                                                var cR = "child";
                                                w.Object.defineProperty(w.Object.prototype, NAME, {
                                                    configurable: true,
                                                    set: function (v) {
                                                        L.sentinelSets++;
                                                        L.setByRealm[cR] = (L.setByRealm[cR]|0)+1;
                                                        try {
                                                            if (!this[UID]) {
                                                                w.Object.defineProperty(this, UID, {
                                                                    value: cR + "@" + (++_uidc),
                                                                    configurable: true, enumerable: false
                                                                });
                                                            }
                                                            w.Object.defineProperty(this, NAME, {
                                                                value: v, writable: true,
                                                                configurable: true, enumerable: false
                                                            });
                                                        } catch (e2) {}
                                                    },
                                                    get: function () {
                                                        try {
                                                            var uid = null;
                                                            try { uid = this ? this[UID] : null; } catch (e3) {}
                                                            if (L.sentinelGetMiss.length < 400) {
                                                                L.sentinelGetMiss.push({
                                                                    cls: w.Object.prototype.toString.call(this),
                                                                    ctor: (this && this.constructor && this.constructor.name) || (typeof this),
                                                                    nm: (typeof this === "function" ? (this.name || "?") : ""),
                                                                    readRealm: cR,
                                                                    taggedBefore: uid || null
                                                                });
                                                            }
                                                        } catch (e) {}
                                                        return undefined;
                                                    }
                                                });
                                                L.notes.push("sentinel installed in realm=child");
                                            }
                                        } catch (eCS) { L.notes.push("child sentinel ERR: " + eCS); }
                                        wrapProp(w, "MediaSource", "cw.MediaSource");
                                        wrapProp(w, "MediaRecorder", "cw.MediaRecorder");
                                        try { wrapProp(w.MediaSource, "isTypeSupported", "cw.MediaSource.isTypeSupported"); } catch (e) {}
                                        try { wrapProp(w.MediaRecorder, "isTypeSupported", "cw.MediaRecorder.isTypeSupported"); } catch (e) {}
                                        try {
                                            var cnav = w.navigator;
                                            var cNavProto = (w.Navigator && w.Navigator.prototype) || (cnav && Object.getPrototypeOf(cnav));
                                            if (cNavProto) wrapProp(cNavProto, "mediaDevices", "cw.nav.mediaDevices");
                                            if (cnav && cnav.mediaDevices) {
                                                wrapProp(cnav.mediaDevices, "enumerateDevices", "cw.md.enumerateDevices");
                                            }
                                        } catch (e) {}
                                        try { wrapProp(w.Function.prototype, "toString", "cw.Fn.proto.toString"); } catch (e) {}
                                    }
                                } catch (e) { L.notes.push("cw hook ERR: " + e); }
                                return w;
                            },
                            set: cwDesc.set
                        });
                    }
                }
            } catch (e) { L.notes.push("iframe hook ERR: " + e); }
        })();
    "#;

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
            println!("[smcdpv] navigate errored: {e}");
            return;
        }
        Err(_) => {
            println!("[smcdpv] navigate timed out (180s)");
            return;
        }
    };
    for _ in 0..48 {
        let _ = p.event_loop().run_until_idle(Duration::from_millis(250)).await;
    }

    // Summary: per-key, did the returned identity ever flip
    // function -> (undefined | null | different identity)?
    let summary = p
        .evaluate(
            r#"(function(){
                var L = globalThis.__smcdpv || {acc:[]};
                var byKey = {};
                (L.acc||[]).forEach(function(r){
                    var k = r.k;
                    (byKey[k] = byKey[k] || {key:k, reads:0, types:{}, flips:[], firstT:null}).reads++;
                    byKey[k].types[r.t] = (byKey[k].types[r.t]|0)+1;
                    if (byKey[k].firstT === null) byKey[k].firstT = r.t;
                    // a flip = prev was function, now not (or identity changed mid-stream)
                    if (r.prevT === "function" && (r.t !== "function" || r.same === false)) {
                        byKey[k].flips.push({n:r.n, from:r.prevT, to:r.t, v:r.v, sameId:r.same});
                    }
                });
                // Aggregate the GET-miss by (readRealm, taggedBefore?)
                var missAgg = {};
                (L.sentinelGetMiss||[]).forEach(function(m){
                    var key = m.readRealm + " | tagged=" +
                        (m.taggedBefore ? m.taggedBefore.split("@")[0] : "NEVER")
                        + " | " + m.nm;
                    missAgg[key] = (missAgg[key]|0)+1;
                });
                return JSON.stringify({
                    sentinelSets: L.sentinelSets,
                    setByRealm: L.setByRealm,
                    sentinelGetMissCount: (L.sentinelGetMiss||[]).length,
                    missAgg: missAgg,
                    sentinelGetMiss: (L.sentinelGetMiss||[]).slice(0, 60),
                    notes: L.notes,
                    keys: Object.keys(byKey).map(function(k){ return byKey[k]; })
                });
            })()"#,
        )
        .unwrap_or_default();
    let summary = summary
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/smc_dpv_trap.json"), &summary).ok();
    println!("[smcdpv] === per-accessor read summary (flips = fn->undefined/other) ===");
    println!("{}", &summary[..summary.len().min(6000)]);

    // Also persist the full raw access log for offline drill-down.
    let raw = p
        .evaluate(r#"JSON.stringify((globalThis.__smcdpv&&globalThis.__smcdpv.acc)||[])"#)
        .unwrap_or_default();
    let raw = raw
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/smc_dpv_trap_raw.json"), &raw).ok();
    println!(
        "[smcdpv] full raw access log -> {out_dir}/smc_dpv_trap_raw.json ({} bytes)",
        raw.len()
    );
}
