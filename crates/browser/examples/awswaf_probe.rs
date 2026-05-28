//! AWS WAF challenge.js probe oracle.
//!
//! Loads a captured AWS WAF challenge HTML into BO's V8 isolate (with the
//! same stealth profile production sweeps use) and dumps the per-property
//! access trace recorded by an instrumentation Proxy injected before
//! challenge.js loads. Identifies which fingerprint surfaces challenge.js
//! reads and which one triggers its silent bailout — without burning live
//! IP probes.
//!
//! Usage:
//!   awswaf_probe <html_file> <url> [out_file]
//!
//! The `html_file` must already contain the instrumentation snippet that
//! installs `window.__awswafProbe = {accesses: [], errors: [], events: []}`
//! and wraps `navigator` / `screen` / `window.chrome` / `Function.prototype.toString`
//! / `console.*` with logging Proxies. See `/tmp/awswaf_probe/probe_inject.js`
//! for a reference implementation; the prep script is documented in
//! `docs/releases/v0.1.0-parity/audit/14_AWS_WAF_CORRELATION.md`.
//!
//! `url` is the location-bar URL the page should believe it's loaded from
//! (matters for CSP, sec-fetch-site, same-origin checks). For AWS WAF
//! challenges captured from amazon.com use `https://www.amazon.com/`.

use std::fs;
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let html_path = args.next().unwrap_or_else(|| {
        eprintln!("usage: awswaf_probe <html_file> <url> [out_file]");
        std::process::exit(2);
    });
    let url = args.next().unwrap_or_else(|| {
        eprintln!("missing url");
        std::process::exit(2);
    });
    let out_path = args.next();

    let html = fs::read_to_string(&html_path).expect("read html");
    let profile = stealth::presets::chrome_148_macos();

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            eprintln!(
                "[awswaf_probe] loading {} bytes from {} (url={})",
                html.len(),
                html_path,
                url
            );
            let mut page = browser::Page::from_html_with_url(&html, &url, Some(profile))
                .await
                .expect("Page::from_html_with_url");

            // challenge.js installs AwsWafIntegration synchronously then
            // the inline script calls getToken() which is async. Run the
            // event loop until idle (or 5s ceiling) so promise chains and
            // fetches inside challenge.js complete.
            let _ = page
                .event_loop()
                .run_until_idle(Duration::from_secs(5))
                .await;

            // Dump access trace. Keep the output bounded — last 200 accesses
            // is usually enough to see the bailout context.
            let dump_script = r#"
                (function(){
                    const p = window.__awswafProbe;
                    const awi = (typeof AwsWafIntegration !== 'undefined') ? AwsWafIntegration : null;
                    return JSON.stringify({
                        awswafintegration_exists: !!awi,
                        awswafintegration_methods: awi ? Object.keys(awi) : null,
                        probe_installed: !!p,
                        access_count: p ? p.accesses.length : 0,
                        error_count: p ? p.errors.length : 0,
                        events: p ? p.events : null,
                        errors: p ? p.errors.slice(0, 20) : null,
                        // What did challenge.js touch? Bucketed by (label, prop) with counts.
                        access_summary: p ? (function(){
                            const counts = {};
                            for (const a of p.accesses) {
                                const k = a.l + ':' + a.p;
                                counts[k] = (counts[k] || 0) + 1;
                            }
                            return Object.entries(counts)
                                .sort((a,b)=>b[1]-a[1])
                                .slice(0, 80)
                                .map(([k,c])=>({k,c}));
                        })() : null,
                        first_50: p ? p.accesses.slice(0, 50) : null,
                        last_50: p ? p.accesses.slice(-50) : null,
                    }, null, 2);
                })()
            "#;
            let trace = page
                .evaluate(dump_script)
                .unwrap_or_else(|e| format!("{{\"eval_error\": {:?}}}", e.to_string()));

            println!("{trace}");
            if let Some(out) = out_path {
                fs::write(&out, &trace).expect("write out");
                eprintln!("[awswaf_probe] wrote trace to {out}");
            }
        })
        .await;
}
