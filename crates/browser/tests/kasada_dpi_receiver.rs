//! Diagnostic: WHICH object/realm does Kasada's ips.js read
//! `devicePixelRatio`'s descriptor from? Main window and
//! document.createElement('iframe').contentWindow are both verified
//! Chrome-faithful now, yet the decoded /error `dpi` field is still
//! {"getter":"n/a","setter":"n/a"}. So Kasada reads it off a realm our
//! `_getIframeWindow` hook doesn't cover. This wraps the descriptor +
//! property-access paths to log the RECEIVER identity, so we can find
//! and fix that realm-creation path.

use browser::Page;
use std::time::Duration;

const INIT: &str = r#"(function(){
  globalThis.__dpiProbes = [];
  const rec = (how, recv) => {
    try {
      let ctor='?', isMain=false, hasOwn=false, descKind='none', proto='?';
      try { ctor = (recv && recv.constructor && recv.constructor.name) || typeof recv; } catch(e){ ctor='THREW:'+e; }
      try { isMain = (recv === globalThis); } catch(e){}
      try {
        const d = Object.getOwnPropertyDescriptor(recv, 'devicePixelRatio');
        descKind = d ? (d.get||d.set ? 'accessor['+(d.get?'g':'')+(d.set?'s':'')+']' : 'data') : 'absent-own';
        hasOwn = !!d;
      } catch(e){ descKind='THREW:'+e; }
      try { const p = Object.getPrototypeOf(recv); proto = (p && p.constructor && p.constructor.name) || String(p); } catch(e){}
      if (globalThis.__dpiProbes.length < 60)
        globalThis.__dpiProbes.push({how, ctor, isMain, hasOwn, descKind, proto});
    } catch(e){ try{globalThis.__dpiProbes.push({how, err:String(e)});}catch(_){} }
  };
  // (1) descriptor-based dpi probe
  const _gopd = Object.getOwnPropertyDescriptor;
  Object.getOwnPropertyDescriptor = function(o, p){
    if (p === 'devicePixelRatio') rec('getOwnPropertyDescriptor', o);
    return _gopd.apply(this, arguments);
  };
  try { Object.defineProperty(Object.getOwnPropertyDescriptor,'name',{value:'getOwnPropertyDescriptor',configurable:true}); } catch(_){}
  // (2) plain read of window.devicePixelRatio via a global getter trap is
  //     impossible without knowing the realm; instead trap Reflect.get +
  //     Object.getOwnPropertyNames-style isn't needed — descriptor probe
  //     is what `dpi` (getter/setter probe) uses per 01_KASADA §6.5.
})()"#;

#[tokio::test]
#[ignore = "network: find which realm Kasada reads devicePixelRatio from"]
async fn kasada_dpi_receiver_probe() {
    let profile = stealth::presets::chrome_130_macos();
    match tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init(
            "https://www.canadagoose.com/",
            profile,
            2,
            vec![INIT.to_string()],
        ),
    )
    .await
    {
        Ok(Ok(mut p)) => {
            for _ in 0..30 {
                let _ = p.evaluate("0");
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            let dump = p
                .evaluate("JSON.stringify({n:(globalThis.__dpiProbes||[]).length, probes:(globalThis.__dpiProbes||[]).slice(0,40)})")
                .unwrap_or_else(|e| format!("ERR {e}"));
            println!("KASADA-DPI-RECEIVERS:\n{dump}");
        }
        Ok(Err(e)) => println!("nav err: {e}"),
        Err(_) => println!("timeout"),
    }
}
