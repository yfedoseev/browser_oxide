# VENDOR — DataDome (etsy, yelp, tripadvisor, leboncoin)

**Scope:** Deep, code-level analysis of how DataDome protects sites in the BO
corpus, what BO already ships, what flips etsy specifically, what the
daily-rotating-key blocker is, and the public-engine vs `vendor_solvers` split.

**Corpus cells (DataDome cluster):** etsy, tripadvisor, yelp (US corpus);
leboncoin referenced historically (`page.rs:2504`). BO routed best-of-4 passes
~3/4 of the loose-DataDome targets; etsy is the hard strict-pass holdout.

**Status of BO work:** the 3 public-engine primitives are SHIPPED (commit
`78a1241`, FIX-DD). The remaining gap (the daily-key WASM signal solver) is
correctly deferred to `vendor_solvers`. This doc verifies that boundary against
source and adds external mechanism detail.

---

## 1. What the existing repo docs already concluded

### `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md`
Defined three vendor-agnostic engine primitives needed to *render* a DataDome
interstitial in V8, none of which name DataDome in flow code:

1. **Challenge-doc CSP relaxation** — without it, the strict origin CSP
   (`script-src 'self'`, `frame-src 'self'`) blocks the `captcha-delivery.com`
   script and the challenge iframe; `dd-script.js` never loads.
2. **Cross-origin challenge-iframe materialization** — `find_iframes` runs at
   build time only, so a script-injected `<iframe src="geo.captcha-delivery.com/…">`
   gets only a synthetic `contentWindow` shim and is never fetched/executed.
3. **Solved-cookie retry** — after the bundle lands `datadome=`, the outer doc
   must re-fetch the origin URL; with no solver registered the retry never fired.

It also established the measured baseline (doc §"Concrete targets"):

| Site | BO (all profiles) | Camoufox v150 |
|---|---|---|
| etsy.com | `DataDome-CHL len=1424 ms=90874` | `L3-RENDERED len=253384 ms=4856` |
| tripadvisor.com | `DataDome-CHL len=1430 ms=90866` | `L3-RENDERED len=433359` |
| yelp.com | `DataDome-CHL len=1424` | `DataDome-CHL len=1487` (Camoufox also fails) |

Key conclusion: **yelp serves the interactive (`rt:'c'`) captcha** — even
Camoufox fails it — so yelp is a stretch goal, not an engine bug. etsy +
tripadvisor are the realistic flips.

### `docs/vNext/12_R-DATADOME-WASM.md`
Confirms FIX-DD (commit `78a1241`) shipped the 3 primitives, with the
step-by-step boundary (steps 1–6 + 9 public engine; steps 7–8 = WASM solver in
`vendor_solvers`). The remaining work — the daily-key WASM token computation —
is explicitly out of scope per `CLAUDE.md`. Notes two public follow-ups: tune
the 50 KB interstitial size gate (newer interstitials are 5–10 KB; fine) and
handle the multi-cookie `datadome + _pxhd + _px3` pattern.

### `docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md` (§C.3)
etsy is **Stratum C** ("no open-source engine passes — true frontier"). BO +
v150 + Patchright all get `DataDome-CHL`. Camoufox **v135** got a partial
`7913b` (loose L3, not strict) — it progressed past the initial CHL — but
**v150 regressed** to CHL too. So a DataDome change since v135 broke the prior
behaviour for both Firefox builds. Filed as **R-DATADOME-DAILY-KEY**
(`vendor_solvers`, effort "unknown").

### Memory `state_2026_05_16_phase5_datadome.md`
The pre-strip Phase 5 work flipped homedepot and was advancing on etsy/tripadvisor
when the vendor-strip commit `aecdf19` removed `datadome_handler.rs` (423 LOC),
the akamai crate, and all per-vendor flows. FP-D3 is the load-bearing gotcha:
**`datadome=` is set on EVERY response including the failing 403**, so the cookie
alone is never a solve signal — the body-shape transition is.

---

## 2. New external findings (DataDome mechanism, 2025–2026)

### 2.1 The `var dd = {…}` literal and challenge URL construction
([glizzykingdreko, "Breaking Down DataDome Captcha WAF", Medium](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21))

A blocked request returns a small HTML doc containing:

```javascript
var dd = {
  'rt':   'c',       // response/challenge type: 'c'=captcha(interactive), 'i'=interstitial(silent)
  'cid':  'AHrlq…',  // client ID (seeds the payload PRNG)
  'hsh':  '14D062…', // website hash (seeds the payload PRNG)
  't':    'fe',      // challenge variant
  's':    44330,     // size param
  'e':    'a1dea4…', // entropy/nonce
  'host': 'geo.captcha-delivery.com',
  'cookie': 'Y1XRf…'
};
```

These fields are parsed by the page's bootstrap JS to build the challenge URL
and inject the captcha iframe. Endpoints under `captcha-delivery.com`:
`/captcha/` (slider/image), `/interstitial/` (full-page device check),
`/check` (device-fingerprint validation), with bootstrap JS from
`ct.captcha-delivery.com/c.js` and geo-localized hosts (`geo.captcha-delivery.com`).

### 2.2 Three challenge tiers
DataDome scores every request with an ML model fed by TLS fingerprint, browser
fingerprint, behavioral signals, and IP reputation
([ZenRows](https://www.zenrows.com/blog/datadome-bypass),
[Scrapfly](https://scrapfly.io/bypass/datadome)). Outcome is one of:
- **Allow** — clean trust score, no challenge.
- **Invisible device check** (`rt:'i'` interstitial) — JS + WASM runs silently,
  posts a payload, gets `datadome=` cookie, page reloads. **No human action.**
- **Captcha** (`rt:'c'`) — slider/GeeTest-style puzzle requiring mouse-path
  movement validation. (yelp gets this; even Camoufox fails it.)

### 2.3 The WASM proof-of-work (`boring_challenge`)
Critical detection logic is compiled to WebAssembly (per
[DataDome's own VM-obfuscation changelog](https://datadome.co/changelog/vm-based-obfuscation/),
behind a 403 to WebFetch but widely summarized). The PoW module:
1. Base64-decodes an embedded Wasm binary.
2. Seeds with a random 32-bit value (10–20M range).
3. Takes CPU-core count as a concurrency hint.
4. Runs a nested-loop state machine of XORs/shifts/rotates/magic-constants.
5. Returns a 64-bit result at a terminal state.

This is **a CPU tax** (forces real computation; server validates cheaply), NOT
behavioral fingerprinting. **A correct WASM engine runs it as-is** — the result
is deterministic given the seed.

### 2.4 The daily-rotating key — the actual blocker
([glizzykingdreko](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21);
[DataDome VM-obfuscation](https://datadome.co/changelog/vm-based-obfuscation/))

The blocker is **not** the WASM PoW (deterministic) and **not** fingerprint
(see §3 — Camoufox passes purely on fingerprint). It is the
**`ddCaptchaEncodedPayload` signal-encryption pipeline**, whose parameters
rotate daily:

- **Signal dictionary keys randomize daily** to random 6-char strings:
  `e('THcQWT', i.left)`, `e('ds6frg', i.right)`, … ~31 signals computed from
  two movement lists (`_initialCoordsList`, `_coordsList`) plus device traits.
  This breaks any static-key solver every 24h.
- **Loop-switch obfuscation** with a 128×512 lookup matrix generated by a PRNG
  seeded at init (`case s[263][471]` instead of `case 23`).
- **Encoding** = 4 stages: (1) PRNG seeded from website hash + `cid` + salt,
  (2) per-key-value XOR with PRNG bytes (delimiters also XOR'd), (3) whole-buffer
  secondary XOR from `cid`+salt PRNG, (4) custom URL-safe Base64 with decrementing
  salt XOR. "Each byte's final value depends on multiple dynamic parameters."
- The encoded payload is GET-posted to the validation endpoint; on success the
  server returns the `datadome=` clearance cookie.
- DataDome regenerates the entire stack (variable names, structure, encryption
  keys, VM opcodes, interpreter architecture) on schedule — "the cost to reverse
  engineer exceeds its useful lifetime"
  ([DataDome](https://datadome.co/bot-management-protection/how-to-bypass-datadome/)).

### 2.5 How competitors pass (the decisive comparison)
- **Camoufox** has **NO DataDome-specific code** (verified via deepwiki on
  `daijro/camoufox`). It passes the silent device-check purely by being a real
  Firefox: real WASM runtime runs the PoW, real Web Workers + Fission
  cross-process storage (`cross-process-storage.patch`) sync the per-context
  fingerprint into workers, and `disable_coop` lets cross-origin captcha
  elements be clicked. The DataDome JS *itself* computes the daily-key payload —
  Camoufox never reimplements it. **This is the model BO must match.**
- **API solvers** (CapSolver, CapMonster, TakionAPI, Scrapfly) reimplement the
  `ddCaptchaEncodedPayload` pipeline server-side and re-derive keys daily — this
  is exactly the per-vendor bypass code `CLAUDE.md` forbids in public crates.

**Takeaway:** the winning path for an open engine is Camoufox's — *don't solve
the challenge, just be a faithful enough runtime that the vendor's own JS solves
itself*. BO's primitives already aim at this. The residual is whether BO's V8 +
materialized-iframe runtime is faithful enough to let DataDome's own bundle
complete the daily-key payload.

---

## 3. BO code-level analysis

### 3.1 The 3 shipped primitives (verified against source)

**Detection** — `crates/browser/src/page.rs:208`:
```rust
fn is_datadome_challenge(html: &str) -> bool {
    html.len() < 50_000 && html.contains("captcha-delivery.com")
}
```
Generic body-shape gate (substring already lives in `classify.rs:94` as the
canonical marker). Solve detector at `page.rs:221`:
```rust
fn is_datadome_solved(cookies: &str, body: &str) -> bool {
    cookies.contains("datadome=") && !is_datadome_challenge(body)  // FP-D3 guard
}
```
The `&& !is_datadome_challenge(body)` is the FP-D3 fix — `datadome=` lands on
the 403 too, so the body-transition is the real solve signal.

**Primitive 1 — CSP relax** wired at `page.rs:1795` and `page.rs:1846`:
```rust
solvers.iter().any(|s| s.relax_response_csp(&html)) || is_datadome_challenge(&html);
```
Engine-side default now fires even with the empty solver set (`default_solvers`
returns `Arc<[]>`).

**Primitive 2 — iframe materialization** — `Page::rematerialize_iframes`
(`page.rs:705`) called in the poll at `page.rs:2201`. It DOM-walks current
iframes, and for any not already a real child, does the same cross-origin fetch +
child-context build as build time via `iframe::ChildIframe::from_url`
(`iframe.rs:73`). Idempotent, CSP-`frame-src`-gated, cheap.

**Primitive 3 — solved-cookie break/retry** — `page.rs:2236` breaks on
`is_datadome_solved(&now, &body)`; the cookie-delta retry block re-fetches the
origin (`page.rs:2718`+ extended-reload path).

### 3.2 The materialized iframe DOES get a real WASM drain (good)
The child iframe is NOT starved of async time the way the AWS main-doc path is.
`crates/browser/src/iframe.rs`:
- `from_url` → `run_until_idle(Duration::from_secs(10))` (`iframe.rs:232`)
- `from_srcdoc` → `run_until_idle(Duration::from_secs(5))` (`iframe.rs:64`)

So when the `geo.captcha-delivery.com` iframe is materialized, it gets up to 10s
to fetch its bundle, run the WASM PoW, and POST the payload. The outer-doc drain
(`page.rs:2055`) is floored at 8s and extends to the full nav budget.

### 3.3 WebAssembly is available in BO's V8 (good)
`crates/js_runtime/src/js/window_bootstrap.js:17-29`: `WebAssembly` is the native
V8 object (compile/instantiate work natively via deno_core/V8); BO only adds the
`instantiateStreaming` / `compileStreaming` polyfills (fetch→arrayBuffer→
instantiate). So DataDome's `boring_challenge` PoW module CAN compile and run in
BO. **WASM execution is not the gap.**

### 3.4 The marker guard that prevents false "solved" acceptance
`page.rs:2510-2529` — the V8-fetched body is accepted as "real" only if it does
NOT contain `captcha-delivery.com` / `dd-script` / `dd_engagement` (plus the
other vendors). Comment at `page.rs:2504` explicitly lists
**"DataDome (yelp/etsy/leboncoin/wsj)"** as the reason. Correct: prevents
returning a 200-but-still-interstitial as a pass.

### 3.5 Where the gap actually is
Steps 1–6 + 9 of the flow (12_R-DATADOME-WASM §"Architecture") are in place and
verified. The break is **step 7–8**: the DataDome bundle, running inside the
materialized iframe in BO's V8, must (a) compute the ~31 movement/device signals,
(b) feed them through the daily-rotated key derivation, (c) run the WASM PoW,
(d) emit a valid `ddCaptchaEncodedPayload`, and (e) POST it so the server issues
`datadome=`. For the **silent `rt:'i'`** path (etsy/tripadvisor), the bundle does
this with **no human input** — so in principle BO's faithful-runtime approach
should let it self-solve, exactly as Camoufox does. Two reasons it currently does
not flip etsy:

1. **Signal authenticity.** The ~31 signals include movement curvature/velocity
   from `_initialCoordsList` (page-load→click) and device traits (canvas/WebGL/
   audio/`hardwareConcurrency`/`navigator.plugins`). If BO produces an empty or
   anomalous movement list, or a fingerprint the ML model already flags, the
   payload posts but the server returns a *fresh challenge* rather than a
   clearance cookie — same as v150's regression from v135. **This is a
   fingerprint/behavior parity problem, not a missing-solver problem.** It is
   the same class of gap tracked by R-FP-AUDIT-2026Q3 and the FIX-D2 WebGL split.
2. **Worker context for the WASM PoW.** DataDome can run the PoW in a Web Worker.
   The session's worker secure-context fix (commit `5216336`) restored
   `crypto.subtle` in workers, but per-context fingerprint propagation into BO
   workers is the analogue of Camoufox's `cross-process-storage.patch`. If BO
   workers expose a *different* canvas/WebGL/navigator surface than the main
   realm, DataDome's worker-side device check sees an inconsistency. Verify BO's
   worker fingerprint inheritance.

### 3.6 What is genuinely in `vendor_solvers` scope (not public)
Reimplementing the daily-key `ddCaptchaEncodedPayload` pipeline (the 4-stage
XOR/PRNG/custom-Base64 encoder + daily key derivation + the loop-switch VM
decode) is per-vendor bypass code — forbidden in public crates by `CLAUDE.md` and
correctly parked in `12_R-DATADOME-WASM.md`. The public engine's job is to make
the bundle's OWN self-solve POSSIBLE; it must not encode the token itself.

---

## 4. What flips etsy specifically

etsy serves the **silent `rt:'i'` interstitial** (not yelp's `rt:'c'` captcha).
That means etsy is theoretically winnable by the Camoufox model — no human
solve, the bundle self-solves if the runtime is faithful. The flip requires, in
order of likelihood:

1. **Confirm the materialized `geo.captcha-delivery.com` iframe actually fetches
   and runs in BO live** (the offline-vs-live drain bug that bit AWS may bite
   here too). Instrument the existing `[datadome-trace]` hooks (`page.rs:3149-3215`
   trace the i.js fetch) on a live etsy nav and check: does the iframe POST a
   payload? Does a fresh `datadome=` come back? This single experiment
   disambiguates "runtime didn't run it" from "ran it, got a fresh challenge".
2. **If it runs but gets a fresh challenge → fingerprint/behavior parity.** Feed
   plausible `_initialCoordsList` movement (BO already ships `humanize.js` timers,
   `page.rs:3304`) and close the WebGL/canvas/audio gaps (FIX-D2 shipped the
   WebGL1/2 split; audit R-FP-AUDIT-2026Q3 is the umbrella). Match the worker
   fingerprint surface to the main realm.
3. **If it never runs → the live-nav drain bug.** Same root cause as AWS
   (HANDOFF_2026_05_28b §4): the materialized iframe must get its full
   `run_until_idle` budget on the LIVE path, not just the offline oracle.

The **daily-key blocker** is the step-2/step-3 frontier: even if BO runs the
bundle perfectly, the daily-rotated signal keys + custom encoding mean BO cannot
*shortcut* the solve — it must let DataDome's own (daily-rotated) JS compute the
payload. So BO must NOT cache or hardcode anything; it must re-run the freshly
served bundle every nav. The good news: that is exactly what the
materialize-iframe + 10s-drain primitives already do.

---

## 5. Ranked fix list (ROI order)

| # | Fix | Effort | Confidence | Public engine? |
|---|---|---|---|---|
| 1 | **etsy live-nav trace experiment** — run live etsy with the existing `[datadome-trace]` hooks (`page.rs:3149`) + log materialized-iframe POSTs and the `Set-Cookie: datadome=` response; disambiguate "iframe never ran" (drain bug, same as AWS) vs "ran, got fresh challenge" (fingerprint/behavior). | 1 day | high | yes (diagnostic) |
| 2 | **Live-nav iframe drain parity** — if #1 shows the materialized `captcha-delivery.com` iframe doesn't complete its async PoW/POST on the live path, give `rematerialize_iframes` children the same generous `run_until_idle` budget live as the offline oracle (mirror the AWS §5.1 inter-script drain fix). Shared root cause with AWS cluster. | 2-4 days | medium | yes |
| 3 | **Worker fingerprint inheritance audit** — verify BO Web Workers expose the SAME canvas/WebGL/navigator/`hardwareConcurrency` surface as the main realm (Camoufox's `cross-process-storage.patch` equivalent). DataDome can run device-check + PoW in a worker; an inconsistent worker surface = silent block. | 3-5 days | medium | yes |
| 4 | **`_initialCoordsList` movement realism** — ensure `humanize.js` (`page.rs:3304`) seeds a plausible page-load→first-interaction mouse path so the ~31 movement signals aren't empty/anomalous. ~31 signals are derived from it. | 2-3 days | medium | yes |
| 5 | **Multi-cookie solve detection** — extend `is_datadome_solved` to the `datadome + _pxhd + _px3` pattern (DataDome bundles PerimeterX-style cookies); per `12_R-DATADOME-WASM` follow-up. Low effort, prevents a future false-negative. | 0.5 day | high | yes |
| 6 | **DataDome daily-key WASM solver** — reimplement the `ddCaptchaEncodedPayload` 4-stage encoder + daily key derivation + loop-switch VM decode. **Only needed for the interactive `rt:'c'` path (yelp) where self-solve is impossible.** Per-vendor bypass code. | 1-2 weeks | low | **NO — `vendor_solvers`** |

**Expected site impact:**
- Fixes #1–#4 target **etsy + tripadvisor** (both silent `rt:'i'`): plausibly
  **+1 to +2 strict passes**, putting BO at/above v150 on the DataDome cluster
  (v150 currently fails etsy too). Confidence medium because v150 also regressed
  here — the bar moved when DataDome rotated, so a faithful runtime may still not
  suffice if the ML model has tightened.
- Fix #6 targets **yelp** (interactive captcha) — even Camoufox fails it, so this
  is a competitive *lift*, not parity. Lives in `vendor_solvers`, low confidence,
  high maintenance (daily rotation).

**Public-engine vs `vendor_solvers` split:** fixes #1–#5 are pure runtime
fidelity (drain budgets, worker fingerprint inheritance, movement realism,
cookie detection) — all public engine, none names DataDome in flow code. Fix #6
(the actual daily-key token encoder) is the only `vendor_solvers` item, and it is
only required for the interactive variant that no open engine currently passes.

---

## 6. Open questions
- Does the materialized `geo.captcha-delivery.com` iframe complete its
  PoW + payload POST on the LIVE nav path, or only in an offline oracle? (Fix #1
  answers this; it's the single highest-value unknown.)
- Why did Camoufox **v150 regress from v135** on etsy? If it's a DataDome ML
  tightening, BO faithful-runtime parity may be necessary-but-insufficient.
- Does DataDome run the device-check/PoW in a Web Worker for etsy, and does BO's
  worker realm expose a consistent fingerprint? (Fix #3.)
- Is the `rt` value for etsy currently `'i'` (silent) as historically assumed,
  or has it moved to `'c'`? A live capture of `var dd={…}` confirms whether etsy
  is even self-solvable.

## Sources
- [glizzykingdreko — Breaking Down DataDome Captcha WAF (Medium)](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21)
- [DataDome — VM-Based Obfuscation changelog](https://datadome.co/changelog/vm-based-obfuscation/)
- [DataDome — How to Bypass DataDome (And Why It's Not That Simple)](https://datadome.co/bot-management-protection/how-to-bypass-datadome/)
- [ZenRows — How to Bypass DataDome 2026](https://www.zenrows.com/blog/datadome-bypass)
- [Scrapfly — Bypass DataDome](https://scrapfly.io/bypass/datadome)
- [Kameleo — Guide to Bypassing DataDome 2025](https://kameleo.io/blog/guide-to-bypassing-datadome)
- deepwiki `daijro/camoufox` — confirms no DataDome-specific code; passes via real Firefox runtime + cross-process worker fingerprint sync
- BO source: `crates/browser/src/page.rs:208,221,705,1795,1846,2055,2201,2236,2504,2510,3149,3304`; `crates/browser/src/iframe.rs:64,73,232`; `crates/js_runtime/src/js/window_bootstrap.js:17`; `crates/browser/src/classify.rs:94`
- Repo docs: `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md`, `docs/vNext/12_R-DATADOME-WASM.md`, `docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md` §C.3
