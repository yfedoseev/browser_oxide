//! Thin-render diagnostic probe.
//!
//! Navigates a URL with the production `Page::navigate` path, then dumps why a
//! React/Vue SPA mounted a shell and stopped: captured script errors / unhandled
//! rejections (`window.__scriptErrors`), the mount-point child counts, readyState,
//! pending-resource hints, and a sample of the tail of `document.body`.
//!
//!   cargo run --release -p browser --example thin_probe -- <url> [profile]

use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("usage: thin_probe <url> [profile]");
    let profile_name = args
        .next()
        .unwrap_or_else(|| "chrome_148_macos".to_string());
    let profile = match profile_name.as_str() {
        "chrome_148_macos" => stealth::presets::chrome_148_macos(),
        "chrome_148_windows" => stealth::presets::chrome_148_windows(),
        "firefox_135_macos" => stealth::presets::firefox_135_macos(),
        "iphone_15_pro_safari_18" => stealth::presets::iphone_15_pro_safari_18(),
        "pixel_9_pro_chrome_148" => stealth::presets::pixel_9_pro_chrome_148(),
        other => panic!("unknown profile {other}"),
    };

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let t0 = Instant::now();
            let mut page = match browser::Page::navigate(&url, profile.clone(), 3).await {
                Ok(p) => p,
                Err(e) => {
                    println!("NAVIGATE ERROR: {e}");
                    return;
                }
            };
            let nav_ms = t0.elapsed().as_millis();
            let body = page.content();
            let ec = browser::engine_classify(&body);
            println!("== thin_probe {url} ==");
            println!("nav_ms={nav_ms} tag={} len={}", ec.tag, ec.len);
            if let Ok(path) = std::env::var("DUMP_BODY") {
                let _ = std::fs::write(&path, &body);
                println!("  dumped body -> {path}");
            }
            let bl = body.to_lowercase();
            for sig in [
                "gokuprops",
                "awswafcookiedomainlist",
                "token.awswaf.com",
                "awswafintegration",
                "checkforcerefresh",
                "challenge.js",
                "captcha.awswaf.com",
            ] {
                println!("  awssig {sig}={}", bl.contains(sig));
            }

            let diag = r#"
                JSON.stringify((function(){
                    var out = {};
                    out.readyState = document.readyState;
                    out.bodyLen = document.body ? document.body.outerHTML.length : 0;
                    out.scriptTags = document.querySelectorAll('script').length;
                    out.moduleScripts = document.querySelectorAll('script[type=module]').length;
                    var sels = ['#react-root','#__next','#app','#root','[data-reactroot]','#main-app','#mount-point','#__nuxt','main'];
                    out.mounts = {};
                    for (var i=0;i<sels.length;i++){ var el=document.querySelector(sels[i]); if(el) out.mounts[sels[i]] = el.children.length; }
                    out.errors = (window.__scriptErrors||[]).slice(0,30);
                    out.errCount = (window.__scriptErrors||[]).length;
                    // React/Vue presence sniffs
                    out.hasReact = !!(window.React||window.__REACT_DEVTOOLS_GLOBAL_HOOK__);
                    out.hasReactDOM = !!window.ReactDOM;
                    out.hasNext = !!window.__NEXT_DATA__;
                    out.hasVue = !!(window.Vue||window.__VUE__);
                    out.hasNuxt = !!window.__NUXT__;
                    // common "await" globals that gate hydration
                    var fl = (window._browser_oxide && window._browser_oxide.__fetchLog)||[];
                    out.fetchLogLen = fl.length;
                    // last 40 fetch entries (url + status) — what the SPA awaited
                    out.fetches = fl.slice(-40).map(function(e){
                        return (e.status||e.ok||'?') + ' ' + (e.url||e.input||e);
                    });
                    // #root inner sample + child count of common SPA mounts
                    var root = document.querySelector('#root')||document.querySelector('#__next')||document.querySelector('#app');
                    out.rootInner = root ? root.innerHTML.substring(0,300) : '(no root)';
                    out.rootChildren = root ? root.children.length : -1;
                    // sample tail of body (where an error boundary / spinner sits)
                    var bh = document.body ? document.body.innerHTML : '';
                    out.bodyTail = bh.substring(Math.max(0, bh.length-600));
                    return out;
                })())
            "#;
            if let Ok(js) = std::env::var("PROBE_JS") {
                // Run an arbitrary async probe: stash JSON result on a global,
                // drive the loop so the fetch resolves, then read it back.
                let kick = format!(
                    "globalThis.__probeResult='(pending)';(async()=>{{try{{globalThis.__probeResult=JSON.stringify(await ({js}))}}catch(e){{globalThis.__probeResult='PROBE_ERR '+(e&&e.stack||e)}}}})();"
                );
                let _ = page
                    .event_loop()
                    .execute_and_run(&kick, std::time::Duration::from_secs(15))
                    .await;
                let r = page
                    .event_loop()
                    .execute_script("globalThis.__probeResult")
                    .unwrap_or_default();
                println!("PROBE_RESULT: {r}");
            }
            let dump = page.event_loop().execute_script(diag).unwrap_or_default();
            // Pretty-print the JSON for readability.
            match deno_core::serde_json::from_str::<deno_core::serde_json::Value>(&dump) {
                Ok(v) => println!("{}", deno_core::serde_json::to_string_pretty(&v).unwrap()),
                Err(_) => println!("{dump}"),
            }
        })
        .await;
}
