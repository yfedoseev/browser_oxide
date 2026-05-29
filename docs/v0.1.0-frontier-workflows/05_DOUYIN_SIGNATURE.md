# 05 — DOUYIN `__ac_signature` / acrawler: the no-CDP re-examination

**Site:** `https://www.douyin.com/` — ByteDance in-house `byted_acrawler` JS-challenge gate.
**Vendor:** ByteDance `byted_acrawler` (the `_$jsvmprt` bytecode VM). NOT Akamai / Kasada /
DataDome / Cloudflare / AWS-WAF.
**BO status:** `L3-RENDERED`, **6327 bytes, deterministic across all 4 stealth profiles**.
Camoufox v135/v150 render ~1.0 MB. Patchright (Chromium-stealth) **fails** (~8.6 KB).
**Bar to flip:** `tag == "L3-RENDERED"` AND `len > 100000`.

> **This doc challenges the prior "Firefox-only solve ⇒ out of scope for a Chrome engine"
> verdict** (`docs/vNext/05_R-SPA-DOUYIN-SIG.md`, `docs/v0.1.0-parity-workflows/sites/SITE_douyin.md`).
> New external evidence (a working **headless-Chromium** acrawler signer) shows the gate is
> **NOT fundamentally Firefox-keyed** — it is an automation-residue + environment-consistency
> gate. That re-classifies douyin from "(a) Firefox JS-value distribution, unfixable for a
> Chrome engine" toward "(b) automation/consistency tell, **engine-addressable**", and it is
> exactly the class of problem where BO's **no-CDP architecture is a structural advantage**.

This doc cites and extends — does not re-derive — the prior in-repo research:
`docs/v0.1.0-parity-workflows/sites/SITE_douyin.md` (the canonical mechanism capture),
`docs/vNext/05_R-SPA-DOUYIN-SIG.md` (the ticket),
`docs/releases/v0.1.0-parity/05_SPA_HYDRATION_CLUSTER.md` §4,
`docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md` §4,
`docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` §R-SPA-DOUYIN-SIG.

---

## 1. The exact detection mechanism blocking BO

### 1.1 The gate flow (re-confirmed verbatim from the live capture, `/tmp/awswaf/douyin.html`, 72,914 B)

The captured douyin document shell contains **exactly two `<script>` blocks** (grep
`<script` → 2), and they implement a classic cookie-set-then-reload JS challenge:

- **Block 1 (~71.7 KB): the acrawler bytecode VM.**
  `(glb=window)._$jsvmprt=function(b,e,f){ function a(){ if("undefined"==typeof Reflect||!Reflect.construct)return!1; if(Reflect.construct.sham)return!1; if("function"==typeof Proxy)return!0; try{return Date.prototype.toString.call(Reflect.construct(Date,[],(function(){}))),!0}catch(b){return!1} } ...`
  This is a register-based bytecode interpreter (`l=function(b,e){var f=b[e++],a=b[e],d=parseInt(""+f+a,16);...}` — a hex-nibble opcode decoder over a string pool). It captures host globals into the closure: grep shows
  `..."undefined"!=typeof navigator?navigator:void 0,"undefined"!=typeof location?location:void 0...`
  (Math, navigator, location are passed in). It defines `window.byted_acrawler` with `.init` and `.sign`.
  The fingerprint reads (canvas / WebGL / AudioContext / `navigator.*`) live **inside the
  bytecode string pool**, not as plaintext — grepping Block 1 yields **zero** plaintext hits for
  `AudioContext` / `getRandomValues` / `webgl`, and only `toString`×2 + `fromCharCode`×9.

- **Block 2 (~1.1 KB): the gate driver** (captured tokens: `byted_acrawler.init({aid:99999999,dfp:0})`,
  `__ac_nonce=_f2("__ac_nonce")`, `__ac_signature=window.byted_acrawler.sign("",__ac_nonce)`,
  `__ac_referer`, `location.reload`):
  ```js
  window.byted_acrawler.init({aid:99999999, dfp:0});
  var __ac_nonce     = _f2("__ac_nonce"),                          // read Set-Cookie nonce
      __ac_signature = window.byted_acrawler.sign("", __ac_nonce); // run the VM
  _f3("__ac_signature", __ac_signature);                           // write cookie
  _f3("__ac_referer", document.referrer || "__ac_blank", !0);
  try { sessionStorage.setItem("__ac_ns", performance.timing.navigationStart) } catch(e){}
  window.location.reload();                                        // reload carries the cookie
  ```

**Full flow:** server `Set-Cookie: __ac_nonce=...` on the gate response → inline script reads it →
`byted_acrawler.sign("", __ac_nonce)` runs the VM → writes `__ac_signature` to `document.cookie`
→ `location.reload()` → the reload GET carries `Cookie: __ac_nonce=...; __ac_signature=...` →
server validates and returns the real ~1 MB SPA. `ttwid` is **server-set** (Set-Cookie), not a
blocker. This matches `SITE_douyin.md` §2.2 exactly.

### 1.2 What the signature actually keys on (corrected from prior docs)

The prior cluster doc guessed "HMAC-SHA256 of (ua+path+ts+iv)" and "missing `crypto.subtle`"
(`05_SPA_HYDRATION_CLUSTER.md` §4). **Both are falsified.** The Chinese RE corpus
(blog.csdn.net 106059888 / 128305986 / 137271970, cnblogs 18183750, tencent cloud 1773707) is
consistent: `__ac_signature` is computed **inside the VM** from a small input set —
**`__ac_nonce` + `navigator.userAgent` + the page domain**, walked with `charCodeAt`, mixed with a
timespan, bit-packed, base-converted. It is **not WebCrypto** (BO's `crypto.subtle.digest` is
real anyway — see §3.3). The VM gates *whether it will emit a valid signature at all* behind an
**environment-integrity probe**; the cryptographic core is simple, the **environment check is the
gate**.

### 1.3 What blocks BO specifically

BO renders the gate, runs Block 1+2 (the env probe at the VM head returns `!0` in V8 — see §3.1),
calls `sign()`, sets a cookie, reloads — and still receives the 6327-byte gate body on the reload.
So one of:

- **(B1) `sign()` throws** inside the VM (caught by BO's top-level trap, `page.rs:3411` →
  `__scriptErrors`), leaving `__ac_signature` `undefined`/`""`. Reload carries no/empty signature.
- **(B2) `sign()` returns a non-empty value the server rejects** because an *environment input
  the VM folds into the signature* (UA-vs-platform-vs-engine-behaviour consistency, an audio/canvas
  sample, a builtin `toString` integrity byte) differs from a genuine Chrome.

The **deterministic-across-all-4-profiles 6327 B** observation is the key tell: BO produces the
*same wrong outcome* regardless of profile seed. That is most consistent with **(B1) an early VM
bail / throw on an integrity branch** (a thrown/empty signature is profile-independent), or a
constant-shaped rejected signature. Distinguishing B1 vs B2 is the single decisive experiment (§4).

---

## 2. Does a no-CDP real browser pass it? ⇒ engine-addressable?

### 2.1 The asymmetry, restated with NEW evidence

Prior framing (`vNext/05`): "Camoufox (Firefox) passes; Patchright (Chromium) fails ⇒
Firefox-vs-Chromium asymmetric ⇒ a Chrome-mimicking engine probably can't ever flip it without
Firefox emulation, which contradicts our Chrome positioning." That framing is what made douyin
look out-of-scope.

**New evidence overturns the "Firefox-only" premise.** The most-maintained open signer,
`carcabot/tiktok-signature` (deepwiki-confirmed), produces **valid acrawler/frontier signatures in
headless _Chromium_** — `puppeteer-extra` + `puppeteer-extra-plugin-stealth`, launched with
`--disable-blink-features=AutomationControlled` and `ignoreDefaultArgs:["--enable-automation"]`,
spoofing **`navigator.platform = "MacIntel"`, a macOS UA, screen 1920×1080**, and injecting a mock
`window.process`. It explicitly **normalizes UA / platform / screen to be mutually consistent** to
avoid "url doesn't match" rejections (deepwiki: "TikTok's sensitivity to environmental
consistency"). Additional community reports: "when a real browser is used with the same user-agent
and IP, login works without problems" (loadchange/amemv-crawler #125; the EterZ-byte and
davidteather/TikTok-Api #157 threads).

**Therefore the gate is not "Firefox JS-value distribution".** It is:
1. **automation-residue detection** — the very tells `--disable-blink-features=AutomationControlled`,
   removing `--enable-automation`, and the stealth plugin exist to suppress, i.e. `navigator.webdriver`,
   CDP `Runtime.enable` residue, `cdc_` Selenium vars, the puppeteer `window.process` leak; **and**
2. **environment self-consistency** — UA ⇄ platform ⇄ screen ⇄ actual engine behaviour must agree.

Patchright "fails" not because it is Chromium but because it is **Chromium-driven-over-CDP**: it
carries CDP/automation residue that the acrawler env probe (or the server-side consistency check
that folds the signature) keys on. Camoufox "passes" because it is a **genuine Firefox with C++-level
injection and no CDP automation surface** — its builtins are real and its automation footprint is
absent. The discriminator both share is **automation residue + injected-JS inconsistency**, NOT the
Gecko-vs-V8 value distribution.

### 2.2 BO's structural position (the central thesis applied)

This is the case the project's no-CDP thesis was made for:

- **BO has NO CDP.** It is in-process V8 via `deno_core`. There is no DevTools protocol, no
  `Runtime.enable`, no juggler, no `--enable-automation` flag, no `cdc_` vars, no
  `navigator.webdriver=true` default. BO sets `navigator.webdriver` → **`false`** on
  `Navigator.prototype` (`window_bootstrap.js:1003-1004`), and there is **no node `process` leak**
  (grep `window.process`/`globalThis.process` in the bootstraps → zero hits) — the exact tell
  puppeteer-stealth has to *paper over* with a mock.
- **BO's UA/platform/screen are already mutually consistent** for a real Chrome-on-macOS:
  `chrome_148_macos.yaml` → UA `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ... Chrome/148.0.0.0 Safari/537.36`,
  `platform: "MacIntel"`, `screen 1512×982`, `platform_version: 15.2.0`. No Linux-headless mismatch.

So **BO starts ahead of Patchright**: it lacks the CDP/automation residue that makes the
Chromium-over-CDP path fail. If douyin's gate is automation-residue + consistency (the new
evidence says it is), BO is *structurally positioned to pass a gate that no CDP-based Chromium engine
can*, **for the Chrome profile, without Firefox emulation**. That makes douyin **plausibly
engine-addressable** — exactly the reclassification this workflow was chartered to find.

**The honest caveat:** this is a *re-classification of probability*, not a confirmed pass. BO
synthesizes its builtins in JS bootstrap; a genuine browser does not. If the VM's residual
discriminator is a **builtin-integrity tell** (a `toString`/descriptor mismatch BO introduces by
synthesizing) rather than CDP residue, then BO has to *fix that specific tell* — still
engine-addressable (§3.2), but not free. The decisive experiment (§4) tells us which.

---

## 3. The concrete engine path (file:line) + how no-CDP helps

### 3.1 Encouraging: BO passes the VM entry probe and the gate plumbing already works

- **Entry probe passes.** The VM head `a()` does
  `if("function"==typeof Proxy)return!0` (grep-confirmed in the capture). `Proxy` and `Reflect`
  are V8 natives, present and un-deleted in BO (`cleanup_bootstrap.js` *uses* `Reflect.*`; neither
  is removed). So `a()` returns `true`, `Reflect.construct.sham` is falsy, and the VM proceeds —
  BO does **not** bail at the entry gate.
- **Reload + cookie plumbing already works** (per `SITE_douyin.md` §3.1, re-verified):
  - `location.reload()` → `window_bootstrap.js:1402-1410` sets
    `__pendingNavigation = { url, kind: "reload" }` (and is `_maskAsNative`'d alongside
    `assign`/`replace`/`toString`).
  - `crates/browser/src/page.rs` reload loop re-fetches and the **Cookie header is attached by the
    HttpClient layer** (`crates/net/src/headers.rs` — "Cookie is added by the HttpClient layer").
  - `document.cookie = "__ac_signature=..."` → `dom_bootstrap.js:1562-1563` → `op_cookie_set`
    (`fetch_ext.rs:401`) writes the **shared cookie jar** the reload GET reads via `op_cookie_get`
    (`fetch_ext.rs:389`).
  - `document.referrer` returns `""` (`dom_bootstrap.js:1536`) → the gate's
    `document.referrer || "__ac_blank"` cleanly falls back to `"__ac_blank"` — not a blocker.

  **So if `sign()` produced an accepted value, BO's existing path would carry it and the gate would
  clear.** The blocker is the signature *value/branch*, not the navigation. (Contrast AWS-WAF, which
  is a Web-Worker async-drain blocker per `HANDOFF_2026_05_28b` §4 — douyin's `sign()` is
  **synchronous**, no worker/drain involved.)

### 3.2 The most likely real lever — native-builtin integrity (`Function.prototype.toString` + descriptor shape)

This is where a Chrome-*mimicking* engine that synthesizes builtins (BO) can fail where a *genuine*
browser (Camoufox/real Chrome) passes — and it is **fixable**.

BO's masking machinery is solid and battle-tested against Kasada
(`stealth_bootstrap.js:7-104`):
- `Function.prototype.toString` is patched (`:25-48`) to return
  `function <tag>() { [native code] }` via a `Symbol.for('__browser_oxide_native__')` tag, with a
  re-entrancy guard and **method-shorthand semantics** so it is non-constructable exactly like real
  Chrome's native `toString` (`:19-24` documents that a plain `function toString(){}` was
  *constructable* and `class X extends Function.prototype.toString {}` did NOT throw, which real
  Chrome 147 throws — verified CDP-free; this was a Kasada `fsc` probe tell, now fixed).
- `_maskFunction` (`:51-80`) sets an own configurable `name` + the native tag, and **deliberately
  does NOT add an own `toString`** (own `toString` was a prior FP — Chrome native fns have
  `getOwnPropertyNames` of only `['length','name'(,'prototype')]`).
- `_maskAsNative` (`:82-104`) walks own-or-prototype to mask getters/setters/value methods.

The acrawler VM head **already** calls `Date.prototype.toString.call(Reflect.construct(Date,...))`
and the webmssdk RE (nullpt.rs, xugj520) documents heavy reliance on
`Function.prototype.toString` + native-code detection. **The path:**

1. Run the §4 R1 oracle to enumerate **every builtin the VM `toString`s or descriptor-inspects**
   during `init()`/`sign()`.
2. For each, diff BO's output against a real-Chrome-148 capture:
   - `Function.prototype.toString.call(fn)` must read `... { [native code] }`;
   - `Object.getOwnPropertyNames(fn)` must match Chrome (no stray own `toString`/`prototype`);
   - getters must live on the **prototype** as accessor descriptors, not as own data props;
   - `fn.name`, `fn.length`, constructability (`new fn` / `class X extends fn`) must match.
3. Fix any miss by extending `_maskAsNative` coverage / correcting descriptor placement. This is the
   same lever that closed Kasada `fsc` — **public engine, no per-vendor code.**

**How no-CDP helps here:** because BO has no CDP/automation residue, the integrity probe has **no
"cheap" tell to find** (no `webdriver`, no `cdc_`, no `process`, no `--enable-automation`). That
forces any residual discriminator to be a *builtin-shape* difference — which is precisely the kind
of bug R2 enumerates and fixes deterministically. Patchright cannot win this race (its CDP residue
is detected before the integrity probe even matters); BO can.

### 3.3 `crypto.subtle` is digest-only — almost certainly irrelevant here

`shared_apis_bootstrap.js:112-125` (dup in `window_bootstrap.js:3160-3181`):
`digest` is real (`op_crypto_digest`, SHA-1/256/384/512 — `crypto_ext.rs:11`); `sign`/`verify`/
`importKey`/`deriveBits`/… **reject** via `_subtleNotImplemented` (`:120-125`).
`crypto.getRandomValues` is real non-deterministic bytes (`op_crypto_random_fill`, `crypto_ext.rs:39`).
The acrawler VM does its crypto **inside the bytecode** (charCodeAt math, §1.2), so it is unlikely
to touch WebCrypto. The prior "missing `crypto.subtle` (~35%)" hypothesis is **falsified**. Fleshing
out `subtle.sign`/`importKey`/`deriveBits` (R4) has general value for PoW SPAs but is **low
probability for douyin**.

### 3.4 AudioContext fingerprint — profile-varied already (prior doc overstated determinism)

`SITE_douyin.md` §3.4 / §4-R3 claimed the offline-audio render is "constant … identical across all
4 profiles." **That is now inaccurate.** `audio_ext.rs:32 op_offline_audio_render` applies
**per-`audio_seed` compressor jitter** (threshold ±5 mdB, release ±0.1 ms, `sin()`-based so seed=0 →
zero jitter), wired through `canvas_bootstrap.js:986 op_offline_audio_render(seed, ...)` with the
seed pulled from the profile (`chrome_148_macos.yaml:69 audio_seed: 686413622842519738`;
`config.rs:200`). So BO's audio fp **does** vary per profile.

The corrected implication: since BO's audio fp already varies per profile **but the douyin body is
still identical across profiles**, the gate is almost certainly **NOT keying on the audio sample**
(an audio-keyed gate would yield *different* bodies per profile). This *down-ranks* the audio
hypothesis and *up-ranks* an early integrity bail (B1) or a non-audio constant tell — sharpening
the §4 priority toward R1→R2.

---

## 4. No-CDP-oracle capture + diff validation plan

The valid oracle is **real Chrome/Firefox launched WITHOUT CDP** + a passive capture — never
Playwright/Patchright (which douyin's automation probe detects, giving a false "even real Chrome
fails"). The repo already has the right tooling shape.

### R1 — Offline acrawler trace: does `sign()` throw (B1) or return-and-reject (B2)? (DECISIVE, do first, ~1 day)
- **What:** Point the existing `awswaf_probe`-style oracle (`crates/browser/examples/awswaf_probe.rs`)
  at the captured douyin gate (`/tmp/awswaf/douyin.html`) with the production
  `chrome_148_macos` profile. The probe installs an instrumentation Proxy *before* the page script
  runs and logs every `navigator.*` / `crypto.*` / `AudioContext` / `Function.prototype.toString`
  access (it already traps these for AWS). Add a wrapper around `byted_acrawler.sign` that records:
  (a) its **return value** (empty/undefined ⇒ B1; non-empty ⇒ B2), (b) `window.__scriptErrors`
  after Block 2 (any throw inside the VM), (c) whether `__pendingNavigation.kind=="reload"` fired.
- **Output:** the **call graph + access trace** the VM makes during `init()`/`sign()`, plus the B1/B2
  verdict. No live IP burned (offline isolate).
- **Confidence:** high (tooling exists). **Impact:** 0 sites directly; gates R2–R4. **Public engine.**

### R2 — Builtin-integrity diff vs no-CDP real Chrome (most likely real lever, 2–4 days)
- **What:** From R1's access list, capture the **same** `toString` / `getOwnPropertyNames` /
  descriptor outputs from a **real Chrome 148 launched without CDP** (a normal user-launched Chrome
  + a `// in-page dump` via the URL bar / a bookmarklet, results copied out — NOT via CDP), for every
  builtin the VM touched. Field-diff BO vs real-Chrome. Fix each mismatch by extending
  `_maskAsNative` / correcting descriptor placement in the bootstraps. Re-run R1 until BO's dump is
  byte-identical to real Chrome's for the touched surface.
- **Validate the flip:** non-live first via R1 (does `sign()` now return a populated value and does
  the reload fire?), then a **single** gated live nav (only when the competitor benchmark is NOT
  holding the IP) with `sweep_metrics chrome_148_macos` on the douyin URL; success = `L3-RENDERED`,
  `len > 100000`.
- **Confidence:** medium. **Impact:** douyin (1) + slow-burn for other ByteDance/`Function.toString`-
  probing vendors. **Public engine.**

### R3 — Automation-residue audit (cheap, mostly already clean, ~0.5 day)
- **What:** Confirm BO presents **zero** of the residues puppeteer-stealth exists to hide:
  `navigator.webdriver===false` (✓ `window_bootstrap.js:1003`), no `window.process` (✓ grep-clean),
  no `cdc_`/`$cdc_` globals, no `--enable-automation` analogue, `window.chrome` present and
  Chrome-shaped (`window_bootstrap.js:1594+`). This is BO's structural edge over Patchright — verify
  it holds end-to-end through the reload.
- **Confidence:** high it's already clean. **Impact:** confirms the no-CDP advantage is intact.
  **Public engine.**

### R4 (optional) — Real `crypto.subtle.sign/importKey/deriveBits` (low douyin probability, 3–5 days)
- General Web-API parity (PoW SPAs), not expected to move douyin. Defer behind R1/R2.

### R5 — `byted_acrawler` signature emulation (LAST RESORT — `vendor_solvers` ONLY)
- If R2/R3 do not flip douyin, the residual is reproducing/driving the acrawler VM's exact signing
  semantics. That is **per-vendor bypass** → forbidden in public crates by CLAUDE.md; it belongs in
  the private `vendor_solvers` crate via `browser::ChallengeSolver` / `Page::navigate_with_solvers`.
  Rotates daily (decision-log §R-SPA-DOUYIN-SIG); 1–2 weeks, open-ended. **NOT public engine.**

---

## 5. Honest verdict

**ENGINE-ADDRESSABLE (public) — re-classified UP from the prior "out-of-scope / Firefox-only",
with medium confidence**, pending the R1 decisive experiment.

Reasoning, evidence-based:

1. **The gate is not Firefox-keyed.** A maintained open signer produces valid acrawler signatures in
   **headless Chromium** (deepwiki: `carcabot/tiktok-signature`) by suppressing automation flags and
   normalizing UA/platform/screen consistency. The "Camoufox passes / Patchright fails" asymmetry is
   an **automation-residue + injected-JS-consistency** discriminator, NOT a Gecko-vs-V8 value
   distribution. This directly refutes the `vNext/05` premise that a Chrome-mimicking engine "probably
   won't ever flip it without Firefox-signature emulation."
2. **BO's no-CDP architecture is the structural advantage for this exact gate.** BO has no CDP, no
   `webdriver`, no `--enable-automation`, no `cdc_`, no node `process` leak, and a self-consistent
   Chrome-on-macOS UA/platform/screen. It **starts ahead of every CDP-based Chromium competitor**
   (incl. Patchright) and is positioned to pass a gate they cannot — for the Chrome profile, no
   Firefox emulation required. This is the central thesis instantiated.
3. **The plumbing already works** (entry probe passes in V8 §3.1; reload+cookie path proven §3.1).
   The blocker is the signature *value/branch*, narrowing the fix to a small, deterministic surface.
4. **The most probable residual lever is builtin-integrity** (`Function.prototype.toString` /
   descriptor shape — §3.2), the canonical Chromium-mimic-vs-genuine-browser tell, and the **same
   lever that already closed Kasada `fsc`** — i.e. a known, repeatable, public-engine fix pattern.
   The audio hypothesis is **down-ranked** (BO's audio fp already varies per profile, yet the douyin
   body is profile-invariant — an audio-keyed gate would differ per profile §3.4); the
   `crypto.subtle` hypothesis is **falsified** (digest is real, VM crypto is in-bytecode §3.3).

**Caveats kept honest:**
- This is a probability re-classification, not a confirmed pass. If R1 shows `sign()` returns a
  populated-but-rejected value AND R2's builtin diff comes back clean, the residual could be a
  **server-side behavioural/consistency signal** (mouse/key sequence, `performance.now` drift, or an
  IP/geo reputation factor) that pushes part of the gap toward `humanize.js` tuning or, in the limit,
  `vendor_solvers` (R5). The new evidence ("real browser, same UA + IP, works") suggests IP/geo is
  *not* the gate, but that should be confirmed, not assumed.
- **ROI remains low in absolute terms:** 1 site, v150 head-to-head currently **inconclusive** (v150's
  Firefox/Playwright driver crashes on douyin per `HANDOFF_2026_05_28b` §3, so we cannot same-IP
  confirm v150's 1 MB render). Keep douyin **behind** the AWS live-nav drain (7–8 sites) and homedepot
  hardening. But the *classification* is now "engine-addressable, worth the cheap R1 probe", not
  "out of scope."

**Bottom line:** douyin is a Layer-1 acrawler JS-challenge gate whose discriminator is automation
residue + environment consistency + builtin integrity — **not** Firefox-exclusive JS-value
distribution. BO's no-CDP cleanliness is a genuine structural edge over the Chromium-over-CDP
competitors that fail this gate. The path is public-engine: R1 (decide B1/B2) → R2 (builtin-integrity
diff vs no-CDP real Chrome) → R3 (confirm zero automation residue). Only an irreducible
signature-emulation residual (R5) belongs in `vendor_solvers`.
