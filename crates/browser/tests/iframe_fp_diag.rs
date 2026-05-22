//! Diagnostic: child iframe contentWindow fingerprint surface vs real
//! Chrome 147. Apples-to-apples with ab_harness/probe_title.sh output.
//! Kasada's ips.js runs FP probes inside a Kasada-created child iframe;
//! if our engine's contentWindow realm diverges from real Chrome's, the
//! VM bails before POSTing /tl. This test prints exactly what our
//! child realm exposes so we can diff the gap.

use browser::Page;

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
    let profile = stealth::presets::chrome_148_macos();
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

#[tokio::test]
#[ignore = "diagnostic: deep ifw probe — what does cw look like and why instanceof fails"]
async fn kasada_ifw_deep_probe() {
    let profile = stealth::presets::chrome_148_macos();
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        Some(profile),
    )
    .await
    .unwrap();
    let r = page.evaluate(r#"(function(){
  try {
    var f = document.createElement('iframe');
    f.style.display = 'none';
    (document.body || document.documentElement).appendChild(f);
    var cw = f.contentWindow;
    if (!cw) return JSON.stringify({err: 'contentWindow null'});
    var cwProto = Object.getPrototypeOf(cw);
    var mainProto = Object.getPrototypeOf(window);
    return JSON.stringify({
      // instanceof checks
      i_ciw: (cw instanceof Window),
      i_ciw_type: typeof cw,
      // prototype comparison
      proto_same: (cwProto === mainProto),
      cw_proto_str: cwProto ? String(cwProto) : 'null',
      win_proto_str: mainProto ? String(mainProto) : 'null',
      // Window constructor
      main_win_proto: Object.getPrototypeOf(Window.prototype) === null ? 'null-parent' : 'has-parent',
      cw_ctor: (cw.constructor && cw.constructor.name) || '?',
      main_ctor: (window.constructor && window.constructor.name) || '?',
      // check if cw.Window === Window
      cw_win_same: (cw.Window === Window),
      cw_win_type: typeof cw.Window,
      // Can we use cw's own Window?
      i_ciw_inner: (cw instanceof (cw.Window || {})),
    });
  } catch(e) { return JSON.stringify({PROBE_ERR: String(e), stack: e.stack}); }
})()"#).unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
    println!("KASADA-IFW-DEEP-PROBE: {r}");
}

#[tokio::test]
#[ignore = "diagnostic: frame index access (window[0], frames[0], window.length)"]
async fn kasada_frame_index_probe() {
    let profile = stealth::presets::chrome_148_macos();
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        Some(profile),
    )
    .await
    .unwrap();
    // Mirrors Kasada's actual probe: access child iframe via window[0] / frames[0],
    // NOT via iframe.contentWindow. If window[0] is undefined, all ifw/spd/dpi probes fail.
    let r = page
        .evaluate(
            r#"(function(){
  try {
    var f = document.createElement('iframe');
    f.style.display = 'none';
    (document.body || document.documentElement).appendChild(f);
    var cw0 = window[0];
    var fr0 = frames[0];
    var len = window.length;
    var cwDirect = f.contentWindow;
    return JSON.stringify({
      win0_type: typeof cw0,
      fr0_type: typeof fr0,
      win_length: len,
      win0_eq_cw: (cw0 === cwDirect),
      fr0_eq_cw: (fr0 === cwDirect),
      win0_nav_wd: (cw0 && cw0.navigator) ? cw0.navigator.webdriver : 'NO_NAV',
      win0_plat: (cw0 && cw0.navigator) ? cw0.navigator.platform : 'NO_NAV',
      win0_is_window: (cw0 instanceof Window),
      win0_self_eq: (cw0 && cw0.self === cw0),
    });
  } catch(e) { return JSON.stringify({PROBE_ERR: String(e), stack: e.stack}); }
})()"#,
        )
        .unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
    println!("KASADA-FRAME-INDEX-PROBE: {r}");
}

#[tokio::test]
#[ignore = "diagnostic: Kasada ifw+smc probe parity check"]
async fn kasada_ifw_smc_probe() {
    let profile = stealth::presets::chrome_148_macos();
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        Some(profile),
    )
    .await
    .unwrap();
    // Mirrors Kasada's ifw and smc probe checks exactly:
    //   i_ciw  = cw instanceof Window
    //   i_nwd  = cw.navigator.webdriver
    //   i_cwwd = cw.window === cw
    //   smc    = typeof cw.MediaSource.isTypeSupported, result for "video/mp4"
    // Also tests inner-realm perspective: what does `window` resolve to
    // INSIDE the child realm (via cw.eval)?
    let r = page.evaluate(r#"(function(){
  try {
    var f = document.createElement('iframe');
    f.style.display = 'none';
    (document.body || document.documentElement).appendChild(f);
    var cw = f.contentWindow;
    if (!cw) return JSON.stringify({err: 'contentWindow null'});
    var ms = cw.MediaSource;
    // Test inner-realm perspective via cw.eval (same-origin eval)
    var inner_wt = 'no-eval';
    var inner_ww = false;
    var inner_self = false;
    var inner_win_eq_cw = false;
    try {
      inner_wt = String(cw.eval('typeof window'));
      inner_ww = cw.eval('window.window === window');
      inner_self = cw.eval('self === window');
      inner_win_eq_cw = cw.eval('window === (' + JSON.stringify(undefined) + ')') === true ? 'UNDEFINED' : 'defined';
    } catch(e) { inner_wt = 'EVAL_ERR:' + String(e); }
    return JSON.stringify({
      i_ciw: (cw instanceof Window),
      i_nwd: (cw.navigator && cw.navigator.webdriver),
      i_cwwd: (cw.window === cw),
      i_self_eq_cw: (cw.self === cw),
      ms_type: typeof ms,
      its_type: (ms && typeof ms.isTypeSupported),
      its_mp4: (ms && ms.isTypeSupported && ms.isTypeSupported('video/mp4')),
      its_webm: (ms && ms.isTypeSupported && ms.isTypeSupported('video/webm')),
      cw_win_type: typeof cw.window,
      window_ciw: (typeof Window === 'function'),
      inner_wt: inner_wt,
      inner_ww: inner_ww,
      inner_self: inner_self,
    });
  } catch(e) { return JSON.stringify({PROBE_ERR: String(e), stack: e.stack}); }
})()"#).unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
    println!("KASADA-IFW-SMC-PROBE: {r}");
}

#[tokio::test]
#[ignore = "diagnostic: Kasada spd (screen pixel density) probe in child realm"]
async fn kasada_spd_probe() {
    let profile = stealth::presets::chrome_148_macos();
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><head></head><body></body></html>",
        Some(profile),
    )
    .await
    .unwrap();
    // Mirrors Kasada's spd probe: reads screen/viewport props from child iframe window.
    let r = page
        .evaluate(
            r#"(function(){
  try {
    var f = document.createElement('iframe');
    f.style.display = 'none';
    (document.body || document.documentElement).appendChild(f);
    var cw = f.contentWindow;
    if (!cw) return JSON.stringify({err: 'contentWindow null'});
    var scr = cw.screen;
    return JSON.stringify({
      availWidth: cw.availWidth !== undefined ? cw.availWidth : 'n/a',
      availHeight: cw.availHeight !== undefined ? cw.availHeight : 'n/a',
      width: scr ? scr.width : 'no-screen',
      height: scr ? scr.height : 'no-screen',
      innerWidth: cw.innerWidth !== undefined ? cw.innerWidth : 'n/a',
      innerHeight: cw.innerHeight !== undefined ? cw.innerHeight : 'n/a',
      outerWidth: cw.outerWidth !== undefined ? cw.outerWidth : 'n/a',
      outerHeight: cw.outerHeight !== undefined ? cw.outerHeight : 'n/a',
      dpr: cw.devicePixelRatio !== undefined ? cw.devicePixelRatio : 'n/a',
    });
  } catch(e) { return JSON.stringify({PROBE_ERR: String(e)}); }
})()"#,
        )
        .unwrap_or_else(|e| format!("EVAL_ERR: {e}"));
    println!("KASADA-SPD-PROBE: {r}");
}
