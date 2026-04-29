//! Akamai BMP v13 pixel sensor — per-field parity check.
//!
//! Probe code is transcribed VERBATIM from the deobfuscated bootstrap at
//! `docs/akamai_sensor_analysis/samsclub_akam13_bootstrap.deob.js`. The
//! point is to know — field by field — whether browser_oxide currently
//! produces the same value as real Chrome on macOS does on a live
//! Akamai-protected page (captured at `/tmp/walmart_posts/003.body`).
//!
//! When a probe gap surfaces, fix at the engine and update the expected
//! constant here. This is the regression-lock for v13/pixel parity.

use browser::Page;

const PROBE_HTML: &str = r#"<!doctype html>
<html>
<head><title>v13 probe</title></head>
<body style="margin:0;padding:0">
  <script>
    (function () {
      const out = {};
      const TIMEOUT = 2000;

      // ---- nap: navigator.permissions.query over the v13 fixed list ----
      // Direct transcription of `M(e)` from samsclub_akam13_bootstrap.deob.js
      // (lines 400-435). Joined digit string; `4` for not-valid-PermissionName.
      out.nap = "PENDING";
      const NAP_NAMES = ['geolocation','notifications','push','midi','camera','microphone',
        'speaker','device-info','background-sync','bluetooth','persistent-storage',
        'ambient-light-sensor','accelerometer','gyroscope','magnetometer',
        'clipboard','accessibility-events','clipboard-read','clipboard-write','payment-handler'];
      try {
        if (!navigator.permissions) {
          out.nap = "6";
        } else {
          const t = [];
          const probe = (name, idx) => navigator.permissions.query({name}).then(r => {
            switch (r.state) {
              case 'prompt':  t[idx] = 1; break;
              case 'granted': t[idx] = 2; break;
              case 'denied':  t[idx] = 0; break;
              default:        t[idx] = 5;
            }
          }).catch(e => {
            t[idx] = String(e.message || e).indexOf('is not a valid enum value of type PermissionName') !== -1 ? 4 : 3;
          });
          Promise.all(NAP_NAMES.map((n,i) => probe(n,i))).then(() => {
            out.nap = t.join('');
          });
        }
      } catch (e) { out.nap = "7"; }

      // ---- bp: plugin × MIME java-hashCode list (line 381-388) ----
      const javaHash = (s) => {
        let h = 0;
        for (let i = 0; i < s.length; i++) { h = ((h << 5) - h + s.charCodeAt(i)) | 0; }
        return h;
      };
      const bp = [];
      const plugs = navigator.plugins;
      if (plugs) {
        for (let r = 0; r < plugs.length; r++) {
          for (let o = 0; o < plugs[r].length; o++) {
            const arr = [
              plugs[r].name, plugs[r].description, plugs[r].filename,
              plugs[r][o].description, plugs[r][o].type, plugs[r][o].suffixes
            ];
            bp.push(javaHash(arr.toString()));
          }
        }
      }
      out.bp = bp.join(',');
      out.bp_count = bp.length;
      out.plugins_layout = (() => {
        const dump = [];
        if (!plugs) return null;
        for (let i = 0; i < plugs.length; i++) {
          const p = plugs[i];
          const mimes = [];
          for (let j = 0; j < p.length; j++) {
            mimes.push({type: p[j].type, suffixes: p[j].suffixes, description: p[j].description});
          }
          dump.push({name: p.name, description: p.description, filename: p.filename, mimes});
        }
        return dump;
      })();

      // ---- dp: 24-property presence map (line 115-122) ----
      // Encoding: undefined→0, falsy primitive → primitive, function/object→1
      const enc = (t, n) => {
        if (typeof t[n] === "undefined") return 0;
        const a = t[n];
        const r = typeof a;
        if (!a || Array.isArray(a) || (r !== 'object' && r !== 'function')) return a;
        return 1;
      };
      const dp = {};
      const winProps = ['XDomainRequest','createPopup','removeEventListener','globalStorage',
        'openDatabase','indexedDB','attachEvent','ActiveXObject','dispatchEvent',
        'addBehavior','addEventListener','detachEvent','fireEvent','MutationObserver',
        'HTMLMenuItemElement','Int8Array','postMessage','querySelector'];
      for (const n of winProps) dp[n] = enc(window, n);
      const docProps = ['getElementsByClassName','querySelector','images','compatMode','documentMode'];
      for (const n of docProps) dp[n] = enc(document, n);
      dp.all = +(typeof document.all !== 'undefined');
      if (window.performance) dp.now = enc(window.performance, 'now');
      dp.contextMenu = enc(document.documentElement, 'contextMenu');
      out.dp = JSON.stringify(dp);

      // ---- sr: screen + viewport (line 356-379) ----
      out.sr = JSON.stringify({
        inner: [innerWidth, innerHeight],
        outer: [outerWidth, outerHeight],
        screen: [screenX, screenY],
        pageOffset: [pageXOffset, pageYOffset],
        avail: [screen.availWidth, screen.availHeight],
        size: [screen.width, screen.height],
        client: document.body ? [document.body.clientWidth, document.body.clientHeight] : -1,
        colorDepth: screen.colorDepth,
        pixelDepth: screen.pixelDepth,
      });

      // ---- crc: window.chrome JSON dump (line 437-441) ----
      out.crc = JSON.stringify({"window.chrome": window.chrome || '-not-existent'});

      // ---- ps: storage availability (line 314-326) ----
      const probeStorage = (name) => {
        try {
          const t = window[name];
          t.setItem('__akfp_storage_test__', '__akfp_storage_test__');
          t.removeItem('__akfp_storage_test__');
          return true;
        } catch (e) { return false; }
      };
      out.ps = probeStorage('localStorage') + ',' + probeStorage('sessionStorage');

      // ---- br: browser family detection (line 262-276) ----
      out.br = (() => {
        const ua = navigator.userAgent;
        const opera = (window.opera || ua.indexOf(' OPR/') >= 0) ? 'Opera' : 0;
        const firefox = (typeof InstallTrigger !== 'undefined') ? 'Firefox' : 0;
        let safariRaw = (Object.prototype.toString.call(window.HTMLElement).indexOf('Constructor') > 0
          || (window.safari && window.safari.pushNotification && window.safari.pushNotification.toString && window.safari.pushNotification.toString().indexOf('[native code]') !== -1)
          || window.ApplePaySession);
        const safari = safariRaw ? 'Safari' : 0;
        const chromeIOS = (safari && ua.match('CriOS')) ? 'Chrome IOS' : 0;
        const chrome = (window.chrome && !opera) ? 'Chrome' : 0;
        const ie = ((window.ActiveXObject && 'ActiveXObject' in window) || document.documentMode) ? 'IE' : 0;
        const edge = (!ie && window.StyleMedia) ? 'Edge' : 0;
        return opera || firefox || edge || ie || chrome || chromeIOS || safari || '';
      })();

      // ---- nav: navigator JSON (line 64-77) ----
      const navProps = ['userAgent','appName','appCodeName','appVersion','appMinorVersion',
        'product','productSub','vendor','vendorSub','buildID','platform','oscpu',
        'hardwareConcurrency','language','languages','systemLanguage','userLanguage',
        'doNotTrack','msDoNotTrack','cookieEnabled','geolocation','vibrate','maxTouchPoints',
        'webdriver'];
      const navObj = {};
      for (const k of navProps) {
        navObj[k] = enc(navigator, k);
      }
      // plugins is added as an array of names
      if (navigator.plugins) {
        const names = [];
        for (let i = 0; i < navigator.plugins.length; i++) names.push(navigator.plugins[i].name);
        navObj.plugins = names;
      }
      out.nav = JSON.stringify(navObj);

      // Stash everything for the harness, plus a small wait so async nap settles.
      globalThis.__V13 = out;
      globalThis.__V13_READY = false;
      setTimeout(() => { globalThis.__V13_READY = true; }, 100);
    })();
  </script>
</body>
</html>"#;

/// Use the page's evaluate to read a single string field at a time;
/// avoids the double-stringify escaping minefield of pulling everything
/// out as JSON-of-JSON.
async fn run_probe(profile: stealth::StealthProfile) -> std::collections::HashMap<String, String> {
    let mut page = Page::from_html(PROBE_HTML, Some(profile)).await.unwrap();
    // Drain the event loop a few times so the async permissions probes settle.
    for _ in 0..5 {
        let _ = page.evaluate("0").unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let mut map = std::collections::HashMap::new();
    for key in [
        "nap", "bp", "bp_count", "dp", "sr", "crc", "ps", "br", "nav",
    ] {
        // Pull each field as a plain string. Numbers come back as their
        // toString form which is fine for our equality asserts.
        let val = page
            .evaluate(&format!("String(globalThis.__V13.{key})"))
            .unwrap();
        map.insert(key.to_string(), val.trim_matches('"').to_string());
    }
    map
}

/// Real Chrome 147 on macOS captured at https://www.walmart.com/ — see
/// `/tmp/walmart_posts/003.body`. These are the canonical reference
/// values for v13 pixel sensor parity. Locked in here so future engine
/// changes can't silently drift away from Chrome.
const CHROME_REF_NAP: &str = "11111144242222244122";
const CHROME_REF_BP: &str = "2087755996,1953464915,591862434,325835597,1068473606,-1382186647,-365096851,-1979186206,-108039040,-1906852049";
const CHROME_REF_BP_COUNT: &str = "10";
const CHROME_REF_DP: &str = r#"{"XDomainRequest":0,"createPopup":0,"removeEventListener":1,"globalStorage":0,"openDatabase":0,"indexedDB":1,"attachEvent":0,"ActiveXObject":0,"dispatchEvent":1,"addBehavior":0,"addEventListener":1,"detachEvent":0,"fireEvent":0,"MutationObserver":1,"HTMLMenuItemElement":0,"Int8Array":1,"postMessage":1,"querySelector":1,"getElementsByClassName":1,"images":1,"compatMode":"CSS1Compat","documentMode":0,"all":1,"now":1,"contextMenu":0}"#;
const CHROME_REF_PS: &str = "true,true";
const CHROME_REF_BR: &str = "Chrome";
const CHROME_REF_CRC: &str = r#"{"window.chrome":{"app":{"isInstalled":false,"InstallState":{"DISABLED":"disabled","INSTALLED":"installed","NOT_INSTALLED":"not_installed"},"RunningState":{"CANNOT_RUN":"cannot_run","READY_TO_RUN":"ready_to_run","RUNNING":"running"}}}}"#;

#[tokio::test]
async fn akamai_v13_parity_macos() {
    let m = run_probe(stealth::presets::chrome_130_macos()).await;
    println!("\n=== Akamai v13 probe (chrome_130_macos profile) ===");
    for k in ["nap", "bp_count", "bp", "dp", "sr", "crc", "ps", "br"] {
        println!("  {k}: {}", m.get(k).unwrap());
    }

    // Per-field byte-for-byte match against the captured Chrome reference.
    // Each assertion is a discrete parity gate — when one fails, the
    // engine has drifted from Chrome on that specific Akamai probe.
    assert_eq!(m["nap"], CHROME_REF_NAP, "nap (permissions table)");
    assert_eq!(m["bp_count"], CHROME_REF_BP_COUNT, "bp entry count");
    assert_eq!(m["bp"], CHROME_REF_BP, "bp (plugin × MIME java-hashCodes)");
    assert_eq!(m["dp"], CHROME_REF_DP, "dp (DOM property presence map)");
    assert_eq!(m["ps"], CHROME_REF_PS, "ps (storage availability)");
    assert_eq!(m["br"], CHROME_REF_BR, "br (browser family detection)");
    assert_eq!(m["crc"], CHROME_REF_CRC, "crc (window.chrome JSON dump)");
}
