# SITE: douyin (`https://www.douyin.com/`) — `__ac_signature` / acrawler VM gate

**Status:** open. BO `L3-RENDERED` **6327 bytes, deterministic across all 4 profiles**;
Camoufox v135/v150 ~1.0 MB. Patchright (Chromium-stealth) also **fails** → Firefox-class asymmetry.
**Vendor:** ByteDance in-house `byted_acrawler` (NOT a third-party Akamai/Kasada/DataDome/Cloudflare/AWS stack).
**Classification:** signature-compute gate executed inside an obfuscated JS VM that runs in the SPA shell itself.
**Bar to flip:** `tag == "L3-RENDERED"` AND `len > 100000`.

---

## 1. What the existing repo docs already concluded

Cross-referenced from the in-repo research (cite-and-extend, do not re-derive):

- **`docs/vNext/05_R-SPA-DOUYIN-SIG.md`** — the canonical ticket. Concludes: douyin
  returns HTTP 200 with a ~72 KB normal SPA shell (no vendor antibot markers); the
  block happens inside the SPA's own `__ac_signature` computation; BO's V8 produces a
  value the server-side verifier rejects; Camoufox (Firefox) passes, Patchright
  (Chromium) fails ⇒ **Firefox-vs-Chromium asymmetric**. Lists candidate inputs:
  UA, screen, timezone, AudioContext, mouse/key sequence, `performance.now()` drift,
  `crypto.getRandomValues`. Status: deferred to v0.3.0+, 1–2 weeks open-ended, "tail
  end of value" (single site, Chrome-positioning conflict). Known tokens:
  `__ac_signature` (load-bearing), `ttwid`, `mssdk_`, `msToken`.

- **`docs/releases/v0.1.0-parity/05_SPA_HYDRATION_CLUSTER.md` §4** — places douyin in the
  SPA-hydration cluster. Notes the 6327-byte body + short uniform time "suggests this
  site terminates fast on a sentinel." Hypothesis tree: H1 missing `crypto.subtle`
  primitive (~35%, "`__ac_signature` is commonly HMAC-SHA256 of (ua+path+ts+iv)"), H2
  missing ByteDance SDK shim `byted_acrawler`/`acrwt` (~25%), H3 missing fingerprint
  primitive (~20%), H4 TLS/HTTP2 mismatch (~10%). §4.3 explicitly recommends **defer**
  ("China-specific anti-bot... unlikely to transfer"). Effort table §8: "1 week ...
  open-ended ... HIGH risk."

- **`docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` §4** (lines 128–137) — first capture:
  L3 6327→1.0 MB gap, "custom anti-bot called ttwid + `__ac_signature` cookie. The
  6327-byte body may include their JS that tries to compute the signature." First
  debug step = search body for `__ac_signature`/`ttwid`/`mssdk_`.

- **`docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` §R-SPA-DOUYIN-SIG** (lines 53–63)
  — confirmed this session: `curl` returns 72,914 bytes, no vendor markers; detection
  is via the SPA's own `__ac_signature`; "reads `crypto.getRandomValues` + AudioContext
  + UA + screen"; deferred because "obfuscated JS rotates daily" and it doesn't unblock
  a vendor cluster.

- **`docs/HANDOFF_2026_05_28b.md` §3** — trustworthy same-IP baseline: douyin BO 0/5,
  **v150 = ERR (driver crash)** ⇒ *the head-to-head is currently inconclusive vs v150*
  (v150's Playwright/Firefox driver crashes on douyin per §2's per-site-isolation note).
  We *believe* v150 renders douyin (prior v135/v150 1.0 MB captures) but the same-IP
  delta harness cannot currently confirm it.

**Net of prior conclusions:** correctly identified as a `byted_acrawler` signature gate,
correctly deferred on ROI grounds, but **the prior docs never captured the exact gate
mechanism, never confirmed whether the acrawler VM runs to completion in BO's V8, and
guessed wrong on the primitive** ("missing `crypto.subtle`" — see §3, BO *has* digest;
"HMAC-SHA256 of ua+path+ts" — the real flow is a VM-internal sign of `__ac_nonce`). This
doc closes those gaps.

---

## 2. New external findings (the actual `__ac_signature` / acrawler / webmssdk mechanism)

### 2.1 The two distinct ByteDance layers — do not conflate them

ByteDance web properties ship **two** separate protection systems, and prior BO docs
blurred them:

| Layer | Where | Token(s) | Job |
|---|---|---|---|
| **acrawler** (`byted_acrawler`, the `_$jsvmprt` VM) | inline in the **first page load** (the document shell) | `__ac_nonce` → `__ac_signature` cookie | A **JS-challenge gate**: prove a real browser can run the VM, sign the nonce, set the cookie, and reload. Until this passes you only ever see the ~6 KB gate shell. |
| **webmssdk** (`window.webmssdk`, the "VM obfuscation" + bytecode interpreter) | external `webmssdk.es5.js`, loaded **after** the gate clears | `msToken`, `X-Bogus`/`X-Gnarly`/`frontierSign`, `s_v_web_id` | Signs **subsequent XHR/fetch API calls** (the FYP feed, etc.) so the SPA can hydrate. |

douyin's failure for BO is at **Layer 1 (acrawler)** — BO never gets past the
`__ac_signature` reload gate, so Layer 2 never even runs. This matches the deterministic
6327-byte body. Sources:
[autodev.blog TikTok web protection](https://autodev.blog/posts/tiktok-research-article/),
[nullpt.rs Reverse Engineering TikTok's VM](https://nullpt.rs/reverse-engineering-tiktok-vm-1),
[xugj520 webmssdk VM RE](https://www.xugj520.cn/en/archives/tiktok-vm-reverse-engineering-webmssdk.html),
[davidteather/TikTok-Api #157 "what is window.byted_acrawler.sign"](https://github.com/davidteather/TikTok-Api/issues/157).

### 2.2 The acrawler gate flow (captured verbatim, 2026-05-28)

`curl https://www.douyin.com/` (Chrome-148 UA) returns 72,914 bytes containing exactly
**two `<script>` blocks**:

- **Block 1 (~71.7 KB):** `(glb=window)._$jsvmprt=function(b,e,f){...}` — the **acrawler
  bytecode VM interpreter**. It begins with an environment-integrity probe:
  `if("undefined"==typeof Reflect||!Reflect.construct)return!1; ... if("function"==typeof Proxy)return!0; try{return Date.prototype.toString.call(Reflect.construct(Date,[]...))...}`.
  This block defines `window.byted_acrawler` (`.init`, `.sign`). Because it is a
  bytecode VM, the fingerprint reads (canvas, WebGL VENDOR/RENDERER, AudioContext,
  screen, `navigator.*`) live **inside the bytecode string pool**, not as plaintext JS
  (confirmed: grepping the block for `AudioContext`/`getRandomValues`/`webgl` yields
  zero plaintext hits; only `toString`×2 and `fromCharCode`×9 are visible).

- **Block 2 (~1.1 KB):** the gate driver (verbatim):
  ```js
  function _f1(e,t){/* parse cookie e from string t */}
  function _f2(e){return _f1(e,document.cookie)}
  function _f3(e,t,o){ /* o => sessionStorage+localStorage.setItem(e,t); */
      document.cookie=e+"=; expires=...1970...; path=/; SameSite=None; Secure;";
      document.cookie=e+"="+t+"; expires=<+1yr>; path=/; SameSite=None; Secure;"}
  window.byted_acrawler.init({aid:99999999,dfp:0});
  var __ac_nonce=_f2("__ac_nonce"),
      __ac_signature=window.byted_acrawler.sign("",__ac_nonce);
  _f3("__ac_signature",__ac_signature);
  _f3("__ac_referer",document.referrer||"__ac_blank",!0);
  try{sessionStorage.setItem("__ac_ns",performance.timing.navigationStart)}catch(e){}
  window.location.reload();
  ```

**The full flow:** server sets `__ac_nonce` via `Set-Cookie` on the gate response →
inline script reads it from `document.cookie` → `byted_acrawler.sign("", __ac_nonce)`
runs the VM to produce `__ac_signature` → writes it to `document.cookie` → `location.reload()`
→ the *reload* request carries `Cookie: __ac_nonce=...; __ac_signature=...` → server
validates and returns the real ~1 MB SPA. (`ttwid` is **server-set in Set-Cookie**,
confirmed in headers — not the blocker.) Sources:
[今日头条 __ac_signature / __ac_nonce 逆向](https://blog.csdn.net/qq_39802740/article/details/106059888),
[頭條 _signature/__ac_nonce/__ac_signature 参数](https://cloud.tencent.com/developer/article/1773707),
[carcabot/tiktok-signature #131 .sign→.frontierSign](https://github.com/carcabot/tiktok-signature/issues/131).

> Note: our local `curl` returned **HTTP 404** with `set-cookie: ttwid=...` but **no
> `__ac_nonce`** (404 path serves no nonce). The live BO nav with full Chrome headers
> hits the **200 gate** that *does* issue `__ac_nonce` — that is the 6327-byte body BO
> measures. The 72 KB curl body is the JS-VM-bearing variant; the 6327 B body is the
> minimal gate. Either way the load-bearing call is the same: `byted_acrawler.sign`.

### 2.3 Why the VM rejects BO / why Firefox passes and Chromium-stealth fails

The community consensus on `byted_acrawler.sign` returning an **empty or rejected
signature** is that `init()` runs **environment-integrity checks** and the VM either
(a) refuses to emit a valid signature, or (b) emits one that fails server verification,
when it detects an automated/inconsistent environment
([davidteather/TikTok-Api #157](https://github.com/davidteather/TikTok-Api/issues/157),
[dobizz/TikTok #6 "sign is not a function"](https://github.com/dobizz/TikTok/issues/6),
[loadchange/amemv-crawler #56 "signature无效"](https://github.com/loadchange/amemv-crawler/issues/56)).

The Firefox-passes / Chromium-stealth-fails asymmetry (Camoufox passes, Patchright fails)
points to a **Chromium-family tell** the acrawler VM keys on — i.e. something that is
*correct on a real Firefox*, *wrong on instrumented Chromium*, and **wrong-by-construction
on a Chrome-mimicking engine like BO**. The high-probability candidates (ranked, §4):

1. **Native-function-source / `toString` integrity of patched builtins.** The VM head
   already calls `Date.prototype.toString.call(Reflect.construct(...))` and the
   webmssdk RE confirms heavy reliance on `Function.prototype.toString` and native-code
   detection. Any BO builtin whose `.toString()` does NOT read `function x() { [native code] }`,
   or whose prototype shape differs from real Chrome, is a tell. Camoufox inherits
   *genuine* Firefox builtins (real native code), so it passes; BO synthesizes builtins
   in JS bootstrap, so a single mismatched `toString`/getter-descriptor fails the probe.
2. **AudioContext / OfflineAudio fingerprint distribution.** The VM samples audio (per
   webmssdk RE + decision-log). BO renders a *single deterministic* offline-audio buffer
   (see §3.4) — same bytes across all 4 profiles ⇒ exactly the "6327 deterministic across
   all profiles" signature BO produces. A real browser's audio fp varies with OS/codec;
   BO's constant value is a tell.
3. **`crypto.subtle` surface beyond `digest`.** BO stubs `sign/importKey/...` to *reject*
   (§3.2). If the VM probes `crypto.subtle.importKey`/`sign` (even feature-detect via
   `try{}`), a rejecting stub differs from Chrome's resolving one.
4. **WebGL `getParameter(VENDOR/RENDERER)` / extension list consistency** with the rest
   of the fingerprint — but per HANDOFF_2026_05_28b §4 the AWS oracle showed BO's WebGL
   surface *proceeds* through challenge.js, so this is lower-probability here.

---

## 3. BO code-level analysis (file:line)

### 3.1 The plumbing for the gate ALREADY EXISTS — this is encouraging

The acrawler gate is a **cookie-set-then-`location.reload()`** pattern, which BO already
supports:

- `location.reload()` → `crates/js_runtime/src/js/window_bootstrap.js:1390` sets
  `__pendingNavigation = { url: _locationData.href, kind: "reload" }` and calls
  `op_set_pending_nav` (masked native via `_maskAsNative(..., 'reload', ...)` at :1398).
- The navigate loop picks this up: `crates/browser/src/page.rs:2617-2638` parses the
  pending nav, resolves `next_url`, and re-fetches. The reload re-fetch builds headers via
  `net::headers::nav_headers_reload` (`page.rs:2595`); **Cookie is attached by the
  HttpClient layer** (`crates/net/src/headers.rs:770` — "Cookie is added by the HttpClient
  layer").
- `document.cookie = "__ac_signature=..."` → `crates/js_runtime/src/js/dom_bootstrap.js:1562-1563`
  calls `ops.op_cookie_set(url, val)` → `crates/js_runtime/src/extensions/fetch_ext.rs:401`
  writes to the **shared cookie jar** that the reload GET reads from
  (`op_cookie_get`, fetch_ext.rs:389).

**Conclusion:** if `byted_acrawler.sign(...)` produced a *correct* `__ac_signature`, BO's
existing reload+cookie path would carry it and the gate would clear. The blocker is the
**signature value**, not the navigation plumbing. (Contrast with AWS-WAF in
HANDOFF_2026_05_28b §4, which IS a drain/execution blocker — douyin is **not** that class:
the acrawler sign is *synchronous*, no Web Worker / async drain involved.)

### 3.2 `crypto.subtle` is digest-only — `sign`/`importKey`/etc. REJECT

`crates/js_runtime/src/js/shared_apis_bootstrap.js:112-125` (and the duplicate in
`window_bootstrap.js:3160-3181`):
```js
_defProtoMethod(_SubtleProto, 'digest', function digest(...) { ...op_crypto_digest... });
const _subtleNotImplemented = (name) => function (...args) {
    return Promise.reject(new DOMException(`${name} not implemented`, "NotSupportedError")); };
for (const m of ['sign','verify','encrypt','decrypt','generateKey','importKey',
                 'exportKey','deriveKey','deriveBits','wrapKey','unwrapKey'])
    _defProtoMethod(_SubtleProto, m, _subtleNotImplemented(m));
```
The Rust backing (`crates/js_runtime/src/extensions/crypto_ext.rs`) implements **only**
`op_crypto_digest` (SHA-1/256/384/512) + `op_crypto_random_fill`. So:
- `crypto.subtle.digest` works (real bytes) — prior doc's "missing crypto.subtle (~35%)"
  hypothesis is **FALSE**; BO has it.
- `crypto.subtle.importKey/sign/deriveBits` **reject** — a feature-probe difference vs Chrome.
- `crypto.getRandomValues` returns *non-deterministic* bytes (`rand::rng()`,
  crypto_ext.rs:39) — good for entropy but means the signature isn't deterministic *because
  of* crypto. The fact BO's body is **deterministic 6327 B across profiles** indicates the
  VM is **not even reaching a getRandomValues-dependent code path** — it bails early (env
  probe) or the server rejects a constant-shaped signature.

### 3.3 Native-function masking — the most likely tell

The acrawler VM's integrity probe (§2.2 head) and webmssdk's documented reliance on
`Function.prototype.toString` mean BO must present builtins whose source reads
`[native code]` AND whose descriptor shape matches Chrome. BO does mask via `_maskAsNative`
(`window_bootstrap.js:1398` etc.), but coverage is per-method and easy to miss. The VM
walks many builtins; **any single un-masked or wrong-shaped property** on the objects the
VM touches (e.g. a getter that should be on the prototype but is an own data property,
or a class that `toString`s as `class X{}` instead of native) is a discriminator. This is
exactly the class of bug that makes a *Chromium-mimicking* engine fail where a *genuine
Firefox* (Camoufox) passes — Camoufox never synthesizes these; BO synthesizes ~all of them
in `*_bootstrap.js`.

### 3.4 AudioContext fingerprint is deterministic (a tell)

`crates/js_runtime/src/extensions/audio_ext.rs:32` `op_offline_audio_render` renders a
fixed triangle-oscillator buffer; the result is **constant** for a given profile. Real
Chrome's audio fp has hardware/OS-dependent micro-variation. A constant audio fp that is
*identical across all 4 BO profiles* is consistent with the "deterministic 6327 B across
all profiles" observation and is a plausible VM discriminator (the VM is documented to
sample audio). NOTE: this is a *parity* gap, not a missing-API gap.

### 3.5 What we have NOT yet confirmed (instrumentation needed)

We do **not** yet know whether, inside BO's V8:
- `byted_acrawler.sign("", nonce)` **throws** (caught by the top-level trap at
  `page.rs:3406-3412` → `__scriptErrors`, leaving `__ac_signature` undefined/""), or
- returns a **non-empty but server-rejected** value.

This is the single decisive experiment (§4 R1) and must be run before any fix.

---

## 4. Ranked fix list (ROI order)

> Reality check carried from prior docs + HANDOFF §3: douyin is **1 site, Firefox-only
> known solve, v150 head-to-head currently inconclusive (driver crash)**. It is a poor ROI
> target versus the AWS live-nav drain (7–8 sites) and homedepot hardening. The ranking
> below is *within douyin*; the whole effort is correctly low-priority for v0.2.0.

### R1 — Instrument the acrawler VM: does `sign()` throw or return-and-reject? (DECISIVE, do first)
- **What:** Capture BO's live 6327 B gate (`sweep_metrics chrome_148_macos` with
  `BROWSER_OXIDE_DEBUG_NAV=1`), then inject a probe that wraps `byted_acrawler.sign` and
  dumps: return value, `__scriptErrors` after the inline block, and whether `iter=1`
  (the reload) fires. Reuse the `awswaf_probe`-style offline oracle
  (`crates/browser/examples/awswaf_probe.rs`) to feed the captured gate HTML and trace
  every `navigator.*`/`crypto.*`/`AudioContext`/`toString` access the VM makes (it already
  traps these for AWS — point it at douyin's gate).
- **Effort:** 1 day. **Confidence:** high (tooling exists). **Impact:** 0 sites directly;
  unblocks R2–R4 by telling us which branch we're in. **Public engine.**

### R2 — Close native-function / builtin-integrity tells the VM probes (most likely real lever)
- **What:** From R1's access trace, enumerate every builtin the VM `toString`s or
  descriptor-inspects; fix any that don't read `[native code]` or whose
  prototype/own-property shape differs from real Chrome 148. Extend `_maskAsNative`
  coverage; verify with an in-VM dump diffed against a real-Chrome capture.
- **Effort:** 2–4 days (iterative, depends on R1 surface count). **Confidence:** medium
  (this is the canonical Chromium-stealth-vs-genuine-Firefox discriminator and matches the
  Patchright-fails / Camoufox-passes asymmetry). **Impact:** douyin (1 site) *if* the VM
  bails on an integrity probe; possible slow-burn value for other ByteDance properties
  (capcut, toutiao) and any `Function.toString`-probing vendor. **Public engine.**

### R3 — Make AudioContext / OfflineAudio fingerprint profile-varied (parity, not stub)
- **What:** Derive the offline-audio render seed from the active stealth profile (so the
  audio fp differs per profile and matches a real hardware distribution) rather than a
  single constant in `op_offline_audio_render` (audio_ext.rs:32). Camoufox does exactly
  this (per-fingerprint audio noise — cross-check `crates/stealth/fixtures/camoufox_*`).
- **Effort:** 2–3 days. **Confidence:** low-medium (audio is *a* VM input but unconfirmed
  as *the* gate). **Impact:** douyin + general fingerprint parity (helps any audio-fp
  vendor, e.g. CreepJS-class). **Public engine** (it's fingerprint fidelity, not bypass).

### R4 — Flesh out `crypto.subtle` (`importKey`/`sign`/`deriveBits`) to real impls
- **What:** Replace the rejecting stubs (shared_apis_bootstrap.js:120-125) with real
  HMAC/ECDSA/AES via Rust ops, so a `crypto.subtle` feature-probe resolves like Chrome.
- **Effort:** 3–5 days. **Confidence:** low for douyin specifically (the acrawler VM is
  bytecode-internal crypto, unlikely to call WebCrypto), but **medium general value**
  (many SPAs/PoW challenges use `crypto.subtle.sign`/`deriveBits`; this is a real Web API
  parity gap that helps beyond douyin). **Public engine.**

### R5 — Per-vendor `byted_acrawler` signature emulation (LAST RESORT — vendor_solvers only)
- **What:** If R2/R3 do not flip douyin, the residual is reproducing the acrawler VM's
  exact signing semantics (or driving the genuine VM to a verifiable signature). This is
  **per-vendor bypass code** → forbidden in public crates by CLAUDE.md; belongs in the
  private **`vendor_solvers`** crate via the `browser::ChallengeSolver` trait /
  `Page::navigate_with_solvers` hook.
- **Effort:** 1–2 weeks, open-ended, rotates daily (decision-log §R-SPA-DOUYIN-SIG).
  **Confidence:** medium-to-flip / low-to-keep-working. **Impact:** douyin (1) + ByteDance
  family. **NOT public engine — `vendor_solvers`.**

---

## 5. Bottom line

douyin is a **Layer-1 acrawler JS-challenge gate** (`__ac_nonce` → `byted_acrawler.sign`
→ `__ac_signature` cookie → `location.reload()`), **not** an SPA-hydration-drain problem
and **not** a third-party vendor stack. BO's reload+cookie plumbing already works
(§3.1), so the blocker is purely the signature *value*. The prior "missing crypto.subtle"
hypothesis is falsified (BO has digest). The Firefox-passes/Chromium-fails asymmetry most
strongly implicates **native-builtin-integrity tells** (R2) and **deterministic audio
fingerprint** (R3) that a Chrome-*mimicking* engine gets wrong but a genuine Firefox gets
right. The decisive next step is the cheap R1 instrumentation to learn whether
`sign()` throws or merely returns a rejected value. Whole-effort ROI remains low (1 site,
v150 parity unconfirmed) — keep it behind the AWS live-nav drain and homedepot hardening.
Any irreducible signature-emulation residual (R5) must live in `vendor_solvers`, never the
public crates.
