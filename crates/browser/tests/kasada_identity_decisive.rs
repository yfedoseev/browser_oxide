//! Master plan §4 Phase 2 — the decisive Kasada differential-identity
//! experiment, core mechanism test (doc 04 §(f), made network-free).
//!
//! Kasada's disassembled VM (doc 26 §2, `j()` primitive; `0x12 GET
//! WINDOW PROP` ×1051) re-derives the global four ways —
//! `window` / `globalThis` / `(0,eval)('this')` / `Function('return
//! this')()` — and compares the intrinsics it reads (`Array.prototype.
//! slice/concat`, `Function.prototype.apply`, `Math.clz32`) **by
//! identity**. In a real same-realm browser those four acquisition
//! paths resolve the *same* global object, so every intrinsic is
//! `===` across all four — this is a **spec invariant**, not something
//! that needs a live-Chrome capture to establish. If our engine
//! fragments any of them, an `0x12`/`j()` identity branch in the VM
//! diverges → register-file divergence → the `undefined`-operand throw
//! / `bot1225.b:1`.
//!
//! Pre-registered outcomes (doc 04 §(f) Step 4), discriminated here:
//!   * **A** — all four paths `===` for every builtin ⇒ identity model
//!     matches Chrome; the realm/sentinel line is NOT the bug; Kasada
//!     block is the holistic Root-2 tail. Close that line.
//!   * **B** — any path differs where Chrome has them equal ⇒ identity
//!     fragmentation CONFIRMED; the fix is the G2 main-window realm
//!     identity unification (`globalThis[X]===window[X]===eval-global[X]
//!     ===Function-this[X]` for every intrinsic).
//!
//! This is network-free and deterministic because the Chrome reference
//! pattern is the ECMAScript "one realm, one set of intrinsics"
//! invariant. The live-canadagoose ips.js run (the in-the-wild
//! confirmation) is the separate `#[ignore]` test below.

use browser::Page;
use stealth;

async fn eval_main(js: &str) -> String {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

/// The core discriminator: for each builtin Kasada reads, are all four
/// global-acquisition paths the *same object* (Chrome invariant)?
#[tokio::test]
async fn kasada_global_identity_invariant_holds() {
    // Returns "slice:WGEF concat:WGEF apply:WGEF clz32:WGEF" where each
    // letter is present iff that path === the `window` path.
    let probe = r#"(() => {
        let W, G, E, F;
        try { W = window; } catch (e) { return 'NO_WINDOW:' + e; }
        try { G = globalThis; } catch (e) { return 'NO_GT:' + e; }
        try { E = (0, eval)('this'); } catch (e) { return 'NO_EVAL:' + e; }
        try { F = Function('return this')(); } catch (e) { return 'NO_FN:' + e; }
        const get = (g, path) => {
            try {
                return path === 'slice'  ? g.Array.prototype.slice
                     : path === 'concat' ? g.Array.prototype.concat
                     : path === 'apply'  ? g.Function.prototype.apply
                     :                      g.Math.clz32;
            } catch (e) { return ('ERR:' + e); }
        };
        const out = [];
        for (const p of ['slice', 'concat', 'apply', 'clz32']) {
            const w = get(W, p), g = get(G, p), e = get(E, p), f = get(F, p);
            let s = p + ':';
            s += 'W';                       // window is the reference
            s += (g === w) ? 'G' : 'g';
            s += (e === w) ? 'E' : 'e';
            s += (f === w) ? 'F' : 'f';
            out.push(s);
        }
        // Also surface the four globals' mutual identity (Chrome: all ===).
        let gid = 'globals:';
        gid += 'W';
        gid += (G === W) ? 'G' : 'g';
        gid += (E === W) ? 'E' : 'e';
        gid += (F === W) ? 'F' : 'f';
        out.push(gid);
        return out.join(' ');
    })()"#;

    let r = eval_main(probe).await;
    let r = r.trim().trim_matches('"').to_string();
    eprintln!("KASADA-IDENTITY-DECISIVE (ours): {r}");
    std::fs::write("kasada_identity_decisive_ours.txt", &r).ok();

    if r.starts_with("NO_") || r.contains("ERR:") {
        panic!(
            "OUTCOME D (methodology): a global-acquisition path threw — \
             cannot evaluate identity. raw={r:?}"
        );
    }

    // Each space-separated token is `name:FLAGS`. Parse ONLY the FLAGS
    // segment (after the last ':') — the label text itself contains
    // letters like the 'e' in "slice" / 'g' in "globals" and must not
    // be scanned. Chrome (one-realm spec invariant): every flag is
    // uppercase (each path === the `window` path). A lowercase flag =
    // that acquisition path fragmented (outcome B).
    let mut fragmented: Vec<String> = Vec::new();
    for tok in r.split_whitespace() {
        let (name, flags) = tok.rsplit_once(':').unwrap_or((tok, ""));
        if flags.chars().any(|c| c.is_ascii_lowercase()) {
            fragmented.push(format!("{name}({flags})"));
        }
    }
    assert!(
        fragmented.is_empty(),
        "OUTCOME B — Kasada identity fragmentation CONFIRMED: \
         acquisition path(s) {fragmented:?} resolve a DIFFERENT object \
         than `window` where real Chrome (one-realm spec invariant) has \
         them all ===. raw={r:?}. Fix = G2 main-window realm identity \
         unification (master plan §4 Phase 2.9 outcome B / Gap 1)."
    );
    // Reaching here = OUTCOME A: identity model matches Chrome; the
    // realm/sentinel line is not the Kasada bug — pivot to the holistic
    // Root-2 tail (allow-but-blocked `bot1225.b:1` is server holistic,
    // per Phase 0.3).
    eprintln!(
        "KASADA-IDENTITY-DECISIVE: OUTCOME A — all four acquisition \
         paths identical for every builtin; identity model matches \
         Chrome. Realm/sentinel line CLOSED as not-the-bug."
    );
}

/// In-the-wild confirmation (doc 04 §(f) Step 1): the same identity
/// probe but with Kasada's real `ips.js` running on canadagoose, so
/// the snapshot is taken in the live VM context. Network; manual.
#[tokio::test]
#[ignore = "network: live canadagoose ips.js identity snapshot"]
async fn kasada_identity_decisive_live_canadagoose() {
    use std::time::Duration;
    let init = r#"(function(){
        globalThis.__idsnap = (function(){
            try {
                const W=window,G=globalThis,E=(0,eval)('this'),
                      F=Function('return this')();
                const g=(x,p)=> p==='slice'?x.Array.prototype.slice
                    :p==='concat'?x.Array.prototype.concat
                    :p==='apply'?x.Function.prototype.apply:x.Math.clz32;
                const o=[];
                for(const p of ['slice','concat','apply','clz32']){
                    const w=g(W,p);
                    o.push(p+':W'+(g(G,p)===w?'G':'g')+(g(E,p)===w?'E':'e')
                        +(g(F,p)===w?'F':'f'));
                }
                o.push('globals:W'+(G===W?'G':'g')+(E===W?'E':'e')
                    +(F===W?'F':'f'));
                return o.join(' ');
            } catch(e){ return 'ERR:'+e; }
        })();
    })();"#;
    let page = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            stealth::presets::chrome_130_macos(),
            3,
            vec![init.to_string()],
        ),
    )
    .await;
    match page {
        Ok(Ok(mut p)) => {
            for _ in 0..25 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            let r = p
                .evaluate("String(globalThis.__idsnap)")
                .unwrap_or_default();
            eprintln!("KASADA-IDENTITY-DECISIVE (live canadagoose): {r}");
            std::fs::write("kasada_identity_decisive_canadagoose.txt", r.trim_matches('"')).ok();
        }
        Ok(Err(e)) => eprintln!("nav err: {e}"),
        Err(_) => eprintln!("timeout"),
    }
}
