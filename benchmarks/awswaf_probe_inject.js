// AWS WAF challenge.js fingerprint-access instrumentation.
//
// Prepended (as an inline <script>) to a captured AWS WAF stub BEFORE the
// challenge.js <script src> so it is in place when challenge.js fingerprints
// the browser. Records every read of the high-value fingerprint surfaces into
// window.__awswafProbe.accesses as {l: <label>, p: <prop>}. awswaf_probe.rs
// dumps a summary + first/last accesses + whether AwsWafIntegration.getToken
// was ever called (the bailout signal: challenge.js fingerprints, then EITHER
// calls getToken (proceed) OR silently returns (bail)).
//
// Reconstructed 2026-05-28 (the original /tmp copy was wiped). Kept in-repo.
(function () {
  const P = (window.__awswafProbe = {
    accesses: [],
    errors: [],
    events: [],
    getTokenCalled: false,
    checkForceRefreshCalled: false,
  });
  const MAX = 4000;
  function rec(label, prop) {
    if (P.accesses.length < MAX) P.accesses.push({ l: label, p: String(prop) });
  }

  // Replace each property of `obj` (own + a few proto levels) with a logging
  // accessor that delegates to the original value/getter. This lets us see
  // exactly which props challenge.js reads on the REAL object (challenge.js
  // reads the genuine navigator/screen/etc., so a shadow Proxy wouldn't work).
  function instrument(obj, label, depth) {
    if (!obj || depth < 0) return;
    let cur = obj;
    let level = 0;
    while (cur && cur !== Object.prototype && level <= depth) {
      for (const key of Object.getOwnPropertyNames(cur)) {
        if (key === "constructor" || key === "__proto__") continue;
        const d = Object.getOwnPropertyDescriptor(cur, key);
        if (!d || d.configurable === false) continue;
        try {
          if (typeof d.value === "function") {
            const fn = d.value;
            Object.defineProperty(cur, key, {
              configurable: true,
              writable: true,
              enumerable: d.enumerable,
              value: function (...a) {
                rec(label, key + "()");
                return fn.apply(this, a);
              },
            });
          } else if (d.get) {
            const g = d.get;
            Object.defineProperty(cur, key, {
              configurable: true,
              enumerable: d.enumerable,
              get() {
                rec(label, key);
                return g.call(this);
              },
              set: d.set,
            });
          } else {
            // Data property: convert to a logging getter returning the value.
            const v = d.value;
            Object.defineProperty(cur, key, {
              configurable: true,
              enumerable: d.enumerable,
              get() {
                rec(label, key);
                return v;
              },
            });
          }
        } catch (e) {
          P.errors.push(label + "." + key + ": " + e);
        }
      }
      cur = Object.getPrototypeOf(cur);
      level++;
    }
  }

  try { instrument(navigator, "navigator", 1); } catch (e) { P.errors.push("nav: " + e); }
  try { instrument(screen, "screen", 0); } catch (e) { P.errors.push("screen: " + e); }
  try { if (window.chrome) instrument(window.chrome, "chrome", 1); } catch (e) {}

  // WebGL — log getParameter(pname) / extensions (the surface FIX-D2 touched).
  try {
    for (const C of [window.WebGLRenderingContext, window.WebGL2RenderingContext]) {
      if (!C || !C.prototype) continue;
      const lbl = C === window.WebGL2RenderingContext ? "webgl2" : "webgl";
      for (const m of ["getParameter", "getExtension", "getSupportedExtensions", "getShaderPrecisionFormat", "getContextAttributes"]) {
        const orig = C.prototype[m];
        if (typeof orig !== "function") continue;
        Object.defineProperty(C.prototype, m, {
          configurable: true, writable: true, enumerable: false,
          value: function (...a) {
            rec(lbl, m + (a.length ? "(0x" + (a[0] >>> 0).toString(16) + ")" : "()"));
            return orig.apply(this, a);
          },
        });
      }
    }
  } catch (e) { P.errors.push("webgl: " + e); }

  // AudioContext / OfflineAudioContext construction + key methods.
  try {
    for (const name of ["AudioContext", "webkitAudioContext", "OfflineAudioContext"]) {
      const C = window[name];
      if (typeof C !== "function") continue;
      window[name] = new Proxy(C, {
        construct(t, a) { rec("audio", name + "()"); return Reflect.construct(t, a); },
      });
    }
  } catch (e) { P.errors.push("audio: " + e); }

  // document.fonts.check / Intl / timing — common fingerprint inputs.
  try { if (document.fonts) instrument(document.fonts, "fonts", 0); } catch (e) {}
  try {
    const pn = performance.now;
    performance.now = function () { rec("perf", "now()"); return pn.apply(this, arguments); };
  } catch (e) {}
  try {
    const ts = Function.prototype.toString;
    Function.prototype.toString = function () { rec("fn", "toString()"); return ts.apply(this, arguments); };
  } catch (e) {}

  // Trap AwsWafIntegration assignment so we can wrap getToken/checkForceRefresh
  // to record the bailout decision.
  try {
    let _awi;
    Object.defineProperty(window, "AwsWafIntegration", {
      configurable: true,
      get() { return _awi; },
      set(v) {
        _awi = v;
        try {
          for (const m of ["getToken", "checkForceRefresh", "forceRefreshToken", "saveReferrer"]) {
            if (typeof v[m] === "function") {
              const orig = v[m];
              v[m] = function (...a) {
                rec("awsWaf", m + "()");
                if (m === "getToken") P.getTokenCalled = true;
                if (m === "checkForceRefresh") P.checkForceRefreshCalled = true;
                return orig.apply(this, a);
              };
            }
          }
        } catch (e) { P.errors.push("awi-wrap: " + e); }
      },
    });
  } catch (e) { P.errors.push("awi: " + e); }
})();
