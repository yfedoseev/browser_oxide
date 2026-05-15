//! Diagnostic: child iframe contentWindow fingerprint surface vs real
//! Chrome 147. Apples-to-apples with ab_harness/probe_title.sh output.
//! Kasada's ips.js runs FP probes inside a Kasada-created child iframe;
//! if our engine's contentWindow realm diverges from real Chrome's, the
//! VM bails before POSTing /tl. This test prints exactly what our
//! child realm exposes so we can diff the gap.

use browser::Page;
use stealth;

const PROBE: &str = r#"(function(){
  try {
    var f=document.createElement('iframe');
    f.style.display='none';
    (document.body||document.documentElement).appendChild(f);
    var w=f.contentWindow;
    if(!w) return JSON.stringify({cw:'NULL'});
    var d=Object.getOwnPropertyDescriptor(w,'devicePixelRatio');
    var proto=Object.getPrototypeOf(w);
    var pd=proto?Object.getOwnPropertyDescriptor(proto,'devicePixelRatio'):null;
    return JSON.stringify({
      cw_type: typeof w,
      cw_ctor: (w.constructor&&w.constructor.name)||'?',
      has_nav: ('navigator' in w),
      dpr: w.devicePixelRatio,
      inst_keys: d?Object.keys(d):null,
      inst_get: d&&d.get?'fn':(d?'absent':'no-desc'),
      inst_set: d&&d.set?'fn':(d?'absent':'no-desc'),
      inst_get_ts: (d&&d.get)?String(d.get).slice(0,46):'n/a',
      inst_set_ts: (d&&d.set)?String(d.set).slice(0,46):'n/a',
      proto_desc: pd?Object.keys(pd):null,
      wd: String(w.navigator&&w.navigator.webdriver),
      plat: (w.navigator&&w.navigator.platform)||'?',
      ua28: (w.navigator&&w.navigator.userAgent||'').slice(0,28),
      sliceTS: w.Array&&w.Array.prototype.slice.toString(),
      fetchTS: (w.fetch!==undefined)?(''+w.fetch).slice(0,42):'fetch-undefined',
      same_fetch: (w.fetch===window.fetch),
      same_arrproto: (w.Array&&w.Array.prototype===window.Array.prototype),
      same_window: (w===window),
      hairline: (function(){try{return Object.getOwnPropertyDescriptor(w,'devicePixelRatio')&&Object.getOwnPropertyDescriptor(w,'devicePixelRatio').set?(function(){w.devicePixelRatio=99;return w.devicePixelRatio;})():'no-set';}catch(e){return 'SET_THREW:'+e;}})()
    });
  } catch(e){ return JSON.stringify({PROBE_ERR:String(e)}); }
})()"#;

#[tokio::test]
#[ignore = "diagnostic: prints child-iframe FP surface, compare vs real Chrome"]
async fn iframe_fp_surface_macos() {
    let profile = stealth::presets::chrome_130_macos();
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        Some(profile),
    )
    .await
    .unwrap();
    let main = page
        .evaluate("JSON.stringify({mainCtor:(window.constructor&&window.constructor.name)||'?',typeofWindow:typeof Window,winInstanceofWindow:(typeof Window==='function')?(window instanceof Window):'n/a',selfCtor:(self.constructor&&self.constructor.name)||'?'})")
        .unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
    println!("OUR-ENGINE-MAIN-WINDOW: {main}");
    let r = page
        .evaluate(PROBE)
        .unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
    println!("OUR-ENGINE-IFRAME-FP (macos profile):\n{r}");
    println!(
        "REAL-CHROME-147-REF:\n{{\"dpr\":1.25,\"inst_keys\":[\"get\",\"set\",\"enumerable\",\"configurable\"],\"inst_get\":\"fn\",\"inst_set\":\"fn\",\"proto_desc\":null,\"wd\":\"false\",\"plat\":\"Linux x86_64\",\"sliceTS\":\"function slice() {{ [native code] }}\",\"fetchTS\":\"function fetch() {{ [native code] }}\",\"same_fetch\":false,\"same_arrproto\":false}}"
    );
}

#[tokio::test]
#[ignore = "diagnostic: same probe, NO profile (raw engine baseline)"]
async fn iframe_fp_surface_noprofile() {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let r = page
        .evaluate(PROBE)
        .unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
    println!("OUR-ENGINE-IFRAME-FP (no profile):\n{r}");
}
