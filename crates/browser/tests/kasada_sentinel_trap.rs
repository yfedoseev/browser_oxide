//! K2-DIFF fix #2 — sentinel-trap capture (the decisive RE step).
//!
//! The decoded sensor showed `smc`/`dpv` throw `TypeError: Cannot read
//! properties of undefined (reading 'unjzomuybtbyyhwwkdpkxomylnab')` —
//! ips.js tags a native via its VM with that per-build sentinel, then
//! re-reads `obj.<sentinel>`; `obj` is `undefined` in our engine.
//!
//! We cannot trap a property read on `undefined`, but we CAN trap every
//! WRITE/READ of that exact property name via an `Object.prototype`
//! accessor: that records *which objects ips.js sentinel-tags* and the
//! stack where it does — naming the object whose identity our
//! `_maskFunction`/`_defProtoMethod` per-access recreation fails to
//! preserve (UNJZOMUY candidate #3). Network, #[ignore].
//!
//! Run: cargo test -p browser --test kasada_sentinel_trap -- --ignored --test-threads=1 --nocapture

use browser::Page;
use std::time::Duration;

#[tokio::test]
#[ignore = "network: K2-DIFF fix#2 — trap who tags the Kasada sentinel"]
async fn trap_kasada_sentinel_writes() {
    let out_dir = "/tmp/kasada_tl";
    std::fs::create_dir_all(out_dir).ok();

    // Install an Object.prototype accessor for the exact sentinel name.
    // `set` records {receiver tag, ctor, where} then shadows itself with
    // a normal own value (so later reads on THAT object return the tag —
    // identity-faithful, doesn't perturb ips.js's own logic). `get`
    // (for objects never written) returns undefined like before.
    let trap_init = r#"
        (function () {
            var NAME = "unjzomuybtbyyhwwkdpkxomylnab";
            globalThis.__sent = { sets: [], firstThrowStacks: [] };
            try {
                Object.defineProperty(Object.prototype, NAME, {
                    configurable: true,
                    set: function (v) {
                        try {
                            var ctor = (this && this.constructor && this.constructor.name) || (typeof this);
                            var tag = Object.prototype.toString.call(this);
                            var st = (new Error()).stack || "";
                            globalThis.__sent.sets.push({
                                ctor: String(ctor),
                                tag: String(tag),
                                where: st.split("\n").slice(1, 7).join(" | ").slice(0, 600)
                            });
                        } catch (e) {}
                        // shadow with a real own prop so re-reads work
                        try {
                            Object.defineProperty(this, NAME, {
                                value: v, writable: true, configurable: true, enumerable: false
                            });
                        } catch (e2) {}
                    },
                    get: function () { return undefined; }
                });
            } catch (e) {
                globalThis.__sent.installError = String(e);
            }
            // Also capture a richer stack at the first TypeError mentioning
            // the sentinel (ips.js catches it internally, so wrap Error).
            var _OE = globalThis.Error;
            globalThis.Error = function (m) {
                var e = new _OE(m);
                try {
                    if (typeof m === "string" && m.indexOf("unjzomuybtbyyhwwkdpkxomylnab") !== -1
                        && globalThis.__sent.firstThrowStacks.length < 5) {
                        globalThis.__sent.firstThrowStacks.push(String(e.stack || "").slice(0, 800));
                    }
                } catch (_) {}
                return e;
            };
            globalThis.Error.prototype = _OE.prototype;
            try { Object.defineProperty(globalThis.Error, "name", { value: "Error" }); } catch (_) {}
        })();
    "#;

    let res = tokio::time::timeout(
        Duration::from_secs(120),
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
            println!("[trap] navigate errored: {e}");
            return;
        }
        Err(_) => {
            println!("[trap] navigate timed out (120s)");
            return;
        }
    };
    for _ in 0..40 {
        let _ = p.event_loop().run_until_idle(Duration::from_millis(250)).await;
    }

    let dump = p
        .evaluate("JSON.stringify(globalThis.__sent || {})")
        .unwrap_or_default();
    let dump = dump.trim_matches('"').replace("\\\"", "\"").replace("\\\\", "\\");
    std::fs::write(format!("{out_dir}/sentinel_trap.json"), &dump).ok();
    println!("[trap] === sentinel SET calls (objects ips.js tags) + throw stacks ===");
    println!("{}", &dump[..dump.len().min(4000)]);
    println!("[trap] full -> {out_dir}/sentinel_trap.json ({} bytes)", dump.len());
}
