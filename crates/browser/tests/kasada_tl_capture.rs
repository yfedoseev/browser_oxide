//! K2-DIFF (Kasada decisive experiment): capture OUR engine's
//! pre-encryption `/tl` sensor for hyatt.com and dump it for a
//! field-by-field audit against the Kasada sensor taxonomy
//! (docs/research_2026_05_14/01_KASADA.md §6). The real-Chrome
//! reference `ab_harness/tl/hyatt.tl_body.bin` is the *encrypted*
//! per-session POST — a raw byte-diff is invalid; the tractable form
//! is: capture our PLAINTEXT sensor (the >500-char string ips.js
//! passes to TextEncoder.encode right before it XOR/encrypts + POSTs
//! to /tl) and audit it vs the documented taxonomy + expected
//! real-Chrome values. K1 (parallel Rust cd) is already deferred so
//! this captures ips.js's authored payload, not a race.
//!
//! Network, #[ignore]. Run:
//!   cargo test -p browser --test kasada_tl_capture -- --ignored --test-threads=1 --nocapture

use browser::Page;
use std::time::Duration;

#[tokio::test]
#[ignore = "network: K2-DIFF — capture our hyatt /tl plaintext sensor"]
async fn capture_hyatt_tl_sensor() {
    let out_dir = "/tmp/kasada_tl";
    std::fs::create_dir_all(out_dir).ok();

    // Wrap TextEncoder.encode (catches the sensor JSON BEFORE encrypt)
    // + fetch/XHR to /tl|kpsdk|cdndex (the POST + its headers). Every
    // >500-char encoded string is stashed; the largest is the sensor.
    let capture_init = r#"
        (function() {
            globalThis.__k2 = { texts: [], posts: [] };
            const oldEncode = TextEncoder.prototype.encode;
            TextEncoder.prototype.encode = function(str) {
                try {
                    if (typeof str === 'string' && str.length > 500) {
                        globalThis.__k2.texts.push({ len: str.length, text: str });
                    }
                } catch (e) {}
                return oldEncode.apply(this, arguments);
            };
            function isTl(u){ u = String(u||''); return /\/tl(\?|$|\/)|x-kpsdk|kpsdk|\/149e9513|cdndex\.io|\/ips\.js/.test(u); }
            const _f = globalThis.fetch;
            globalThis.fetch = function(input, init) {
                try {
                    const url = typeof input === 'string' ? input : (input && input.url) || '';
                    if (isTl(url)) {
                        const b = init && init.body;
                        let b64 = '';
                        try {
                            if (typeof b === 'string') b64 = btoa(unescape(encodeURIComponent(b)));
                            else if (b instanceof Uint8Array) {
                                let s=''; for (let i=0;i<b.length;i+=8192) s+=String.fromCharCode.apply(null,b.subarray(i,i+8192));
                                b64 = btoa(s);
                            } else if (b instanceof ArrayBuffer) {
                                const u=new Uint8Array(b); let s='';
                                for (let i=0;i<u.length;i+=8192) s+=String.fromCharCode.apply(null,u.subarray(i,i+8192));
                                b64 = btoa(s);
                            }
                        } catch(e){}
                        globalThis.__k2.posts.push({ url: String(url).slice(0,160),
                            method: (init && init.method) || 'POST',
                            blen: (typeof b === 'string' ? b.length : (b && (b.length||b.byteLength)) || 0),
                            b64: b64 });
                    }
                } catch (e) {}
                return _f.apply(this, arguments);
            };
            const oOpen = XMLHttpRequest.prototype.open;
            XMLHttpRequest.prototype.open = function(m,u){ this.__u=u; return oOpen.apply(this,arguments); };
            const oSend = XMLHttpRequest.prototype.send;
            XMLHttpRequest.prototype.send = function(b){
                try { if (isTl(this.__u)) globalThis.__k2.posts.push({ url:String(this.__u).slice(0,160),
                    method:'XHR', blen:(b && (b.length||b.byteLength))||0 }); } catch(e){}
                return oSend.apply(this,arguments);
            };
        })();
    "#;

    let res = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.hyatt.com/",
            stealth::presets::chrome_130_macos(),
            2,
            vec![capture_init.to_string()],
        ),
    )
    .await;

    let mut p = match res {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            println!("[k2] navigate errored: {e}");
            return;
        }
        Err(_) => {
            println!("[k2] navigate timed out (120s)");
            return;
        }
    };
    for _ in 0..40 {
        let _ = p.event_loop().run_until_idle(Duration::from_millis(250)).await;
    }

    let posts = p
        .evaluate(
            "JSON.stringify(((globalThis.__k2&&globalThis.__k2.posts)||[]).map(x=>({url:x.url,method:x.method,blen:x.blen})))",
        )
        .unwrap_or_default();
    println!("[k2] /tl-class POSTs observed: {}", posts.trim_matches('"').replace("\\\"", "\""));

    // Persist the largest Kasada POST body (the cdndex.io/error blob —
    // ips.js bails here instead of /tl; this blob carries WHY) as b64
    // for offline decode (outer-b64 → JSON .data → b64 → XOR omgtopkek).
    let blob_b64 = p
        .evaluate(
            r#"(function(){var p=(globalThis.__k2&&globalThis.__k2.posts)||[];p=p.filter(x=>x.b64&&x.b64.length>0);if(!p.length)return '';p.sort((a,b)=>b.blen-a.blen);return p[0].b64;})()"#,
        )
        .unwrap_or_default();
    let blob_b64 = blob_b64.trim_matches('"');
    if blob_b64.len() > 50 {
        let path = format!("{out_dir}/ours_hyatt_error_blob.b64");
        std::fs::write(&path, blob_b64).ok();
        println!("[k2] wrote largest Kasada POST body ({} b64 chars) -> {path}", blob_b64.len());
    } else {
        println!("[k2] NO Kasada POST body b64 captured");
    }

    // Dump every captured pre-encrypt text, largest first; the biggest
    // is the /tl sensor JSON.
    let dump = r#"(function(){
        var t=(globalThis.__k2&&globalThis.__k2.texts)||[];
        t.sort((a,b)=>b.len-a.len);
        return JSON.stringify(t.slice(0,6).map(x=>({len:x.len, head:x.text.slice(0,400)})));
    })()"#;
    let texts = p.evaluate(dump).unwrap_or_default();
    println!("[k2] top pre-encrypt texts (len, head):\n{}", texts.trim_matches('"').replace("\\\"", "\""));

    // Persist the single largest plaintext (the sensor) verbatim.
    let biggest = p
        .evaluate(
            r#"(function(){var t=(globalThis.__k2&&globalThis.__k2.texts)||[];if(!t.length)return '';t.sort((a,b)=>b.len-a.len);return t[0].text;})()"#,
        )
        .unwrap_or_default();
    let biggest = biggest.trim_matches('"');
    if biggest.len() > 50 {
        let path = format!("{out_dir}/ours_hyatt_sensor.txt");
        std::fs::write(&path, biggest.replace("\\\"", "\"").replace("\\\\", "\\")).ok();
        println!("[k2] wrote our largest pre-encrypt sensor ({} chars) -> {path}", biggest.len());
    } else {
        println!("[k2] NO >500-char pre-encrypt text captured — ips.js may not have reached the /tl sensor build (check navigate/CSP/ips.js load)");
    }
}
