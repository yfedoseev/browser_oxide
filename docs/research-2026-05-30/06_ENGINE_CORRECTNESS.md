# 06 — Engine-Correctness Audit (2026-05-30)

**Cluster:** engine-correctness — "find what was implemented incorrectly."
**Method:** cross-cut the live engine for things implemented *wrong / incompletely* that suppress
pass-rate or break parity. Every claim below is read from current code (file:line), not the stale
2026-04-29 inventory docs. Ranked by likely pass-rate impact.

**Ground truth being explained:** v150 PASSES, BO FAILS on douyin / duolingo / homedepot / etsy /
ozon / adidas / wildberries. 5 of 7 (douyin, duolingo, ozon, adidas, wildberries) are
*thin-shell* renders (BO 1.8–13 KB vs v150 100 KB–1.5 MB) — a **JS-execution / SPA-hydration**
failure, NOT a stealth/IP failure (ozon+wildberries pass v150 on the same IP). homedepot+etsy are
challenge failures. The firefox-profile losses (14 sites) are a wire-fingerprint failure.

---

## TL;DR — what is implemented wrong, by impact

| # | Defect | File:line | Class | Blast radius | Confidence |
|---|--------|-----------|-------|-------------|------------|
| **1** | **No ES-module path for document scripts.** `type="module"` scripts are queued and run through classic `v8::Script::compile` → `SyntaxError: Cannot use import statement outside a module` → whole bundle dropped. The module entry of every modern Vite/React SPA never runs. | `lib.rs:122`, `script_runner.rs:44-53`, `dom_bootstrap.js:146` | JS exec | **5 of 7** (douyin, duolingo, adidas, ozon, wildberries) | **High** |
| **2** | **Module-loader capability EXISTS but is wired only to Workers.** `load_main_es_module_from_code` + `mod_evaluate` are already used for module *workers*; the document path never calls them. Defect #1 is a wiring gap, not a missing capability. | `worker_ext.rs:347-372` (has it) vs `page.rs`→`lib.rs:122` (doesn't) | JS exec | makes #1 cheap to fix | **High** |
| **3** | **Firefox profile emits a Chrome wire.** TLS branches only on `device_class`; there is no Firefox arm. `tls_impersonate:"firefox_135"` is a dead string (preset comment says "informational only"). Firefox UA + Firefox headers over Chrome-147 ClientHello + Chrome H2 = JA4↔UA contradiction. | `tls.rs:248-262`, `h2_client.rs:73-85`, `presets.rs:466-474` | Wire FP | **firefox class** (14 sites incl. etsy/DataDome) | **High** |
| **4** | **`run_until_idle` returns `AllWorkDone` while a far-future timer is pending.** A `setTimeout(fn, >budget)` makes the drain exit early instead of using the nav budget. Has an `#[ignore]`'d regression test admitting it. | `event_loop/src/lib.rs:316-376`, ignored test `:524` | Event loop | all deferred-hydration SPAs | **High** |
| **5** | **`IntersectionObserver` fires exactly once, never re-fires.** One synthetic `isIntersecting:true` per `observe()` via `Promise.resolve().then`; no scroll/mutation re-fire. Infinite-scroll / viewport-gated hydration loads one batch then stalls. | `window_bootstrap.js:3543-3570` | DOM/JS | feed/grid SPAs (douyin, wildberries) | **High** |
| **6** | **SPA nav-budget is a hardcoded host allowlist.** Only twitter/x/hulu/yandex/hm/khanacademy/spotify/uber get the 90 s tier; everything else gets 15 s. douyin/duolingo/ozon/wildberries are not listed → 15 s. (adidas gets 25 s as Akamai-BMP.) | `page.rs:1953-2005` | Budget | the 5 thin-shell sites | **High** |
| **7** | **`document.elementFromPoint(x,y)` is a hardcoded `return this.body`.** Ignores coords; `elementFromPoint(99999,99999)` returns `body` not `null` — a one-call CreepJS/PX layout lie-detector tell. | `dom_bootstrap.js:1524` | Fingerprint | DataDome/PX/CreepJS-gated | **High** |
| **8** | **Worker `setInterval(drainOnce, 5)` perpetual poll.** A 5 ms interval that never clears keeps `pending_intervals>0` forever — interacts with #4 and the React scheduler's worker-channel assumptions. | `worker_bootstrap.js:261` | Event loop | worker-using SPAs | **Medium** |
| **9** | **`crypto.subtle` is digest-only.** `generateKey/importKey/sign/verify/encrypt/...` reject `NotSupportedError`. A probe expecting a `CryptoKey` sees a rejection. | `window_bootstrap.js` SubtleCrypto class | Fingerprint | low (most antibot only call digest) | **Medium** |
| **10** | **Homedepot sec-cpt + etsy DataDome solvers are detected but never reach solve.** sec-cpt bundle runs but never produces the `~3~` marker in live nav; DataDome WASM solver is out-of-scope (`vendor_solvers`). | `page.rs:242-247,214-222,1898` | Challenge | homedepot, etsy | **High** (already known) |

**The 2026-04-29 inventory is largely STALE — most of its named gaps are now fixed in code:**
BatteryManager is a real `class … extends EventTarget` (`window_bootstrap.js:1099`), `visualViewport`
is a real `VisualViewport extends EventTarget` (`:5820`), `AudioContext`/`OfflineAudioContext` exist
(`canvas_bootstrap.js:919-927`), `MediaSession` is a real class overriding the placeholder
(`:5886,5916`). Do **not** re-spend effort on those four. The *surviving* fingerprint defect from
that doc is **#7 elementFromPoint** (still `return this.body`).

---

## 1–2. The ES-module gap (dominant root cause for 5 of 7 fails)

### What's wrong
`JsRuntime::execute_script` compiles every document script with the **classic-script** entry:

```
crates/js_runtime/src/lib.rs:122
let script = deno_core::v8::Script::compile(tc_scope, source, script_origin.as_ref())
```

`v8::Script::compile` is the *classic* path. Any source containing a top-level `import`/`export`
throws `SyntaxError: Cannot use import statement outside a module` at **compile** time, and the
caller logs it and moves on (`page.rs:1714-1716` warm path, the cold loop equivalent).

The script discovery layer does **not** filter module scripts. `find_scripts`/`collect_scripts`
only skips JSON-LD / template `type`s; a `type="module"` script falls through the catch-all
`_ => {}` and is queued like any classic script:

```
crates/browser/src/script_runner.rs:44-53
match script_type {
    Some("application/ld+json") | Some("application/json")
    | Some("text/template") | Some("text/html")
    | Some("text/x-template") => { ... continue; }
    _ => {}   // <-- "module" lands here and is queued for classic compile
}
```

The dynamic-injection path is the same bug: `dom_bootstrap.js:146` classifies `type === 'module'`
as `isJs=true`, then runs the body through `(0, eval)(code)` (`:158/:236/:267`). `eval` is a
classic-script context, so module bodies throw there too. Dynamic `import('./chunk.js')` has no
host callback in this path and rejects.

### Why it produces a 1.8–13 KB shell
Modern Vite/React/Vue builds ship the app entry as
`<script type="module" src="/assets/index-[hash].js">`. That entry is the *only* thing that
bootstraps the app, fetches data chunks, and hydrates the DOM. It **never executes** → only the
server shell HTML remains. This matches douyin (6.3 KB), duolingo (13.3 KB), adidas (2.5 KB),
ozon (156 B), wildberries (1.8 KB) exactly.

### Why the fix is cheap (#2): the capability already exists
The engine **already** loads and evaluates ES modules — for module *Workers*:

```
crates/js_runtime/src/extensions/worker_ext.rs:347-357
if is_module {
    let specifier = ModuleSpecifier::parse(&format!("worker-oxide://{worker_id}/main.mjs"))...;
    match runtime.load_main_es_module_from_code(&specifier, script).await {
        Ok(mod_id) => { let eval_fut = runtime.mod_evaluate(mod_id); ... }
```

So `load_main_es_module_from_code` + `mod_evaluate` are present in the deno_core runtime BO links.
The document script path simply doesn't use them. **The fix is to route module scripts through the
same primitives**, plus a host `ModuleLoader` that resolves relative specifiers against the document
URL and recursively prefetches the import graph (BO already has a parallel-prefetch loop for classic
`<script src>` — `page.rs` build phase — to model it on). A dynamic-`import()` host callback closes
the last hole.

**ROI:** highest in the codebase. Expected to flip duolingo, adidas, ozon, wildberries, douyin
(5 of 7 ground-truth fails) from thin-shell to rendered — contingent on the budget/observer fixes
below so the now-executing app has time and the right events to hydrate.

---

## 3. Firefox profile emits a Chrome wire (the firefox-class loss)

`chrome_connector` branches purely on `device_class` (Desktop / MobileAndroid / MobileIOS):

```
crates/net/src/tls.rs:248-251
let curves = match profile.device_class {
    DeviceClass::MobileAndroid => CURVES_ANDROID,
    DeviceClass::MobileIOS     => CURVES_SAFARI_IOS,
    DeviceClass::Desktop       => CURVES_DESKTOP,   // Firefox-desktop lands here = Chrome
};
```

There is **no Firefox arm** in TLS or H2 (`h2_client.rs:73-85` has only Desktop/Android=Chrome and
MobileIOS=Safari). The preset admits it:

```
crates/stealth/src/presets.rs:466-474
// String token — currently informational only. The actual TLS bytes are emitted
// ... with a Chrome-tuned ClientHello. A real Firefox JA4 swap requires ... NSS ...
tls_impersonate: "firefox_135".into(),
```

`headers.rs` *does* emit a correct Firefox header set, which makes it worse: a Firefox UA + Firefox
headers riding a **Chrome-147 ClientHello + Chrome H2 SETTINGS** is an internally contradictory
identity. Any vendor doing a JA4↔UA cross-check (AWS WAF, DataDome, Cloudflare) buckets it
high-risk in one comparison. This is the documented cause of every firefox-profile loss
(reuters/zillow/wsj/airbnb/spotify/tripadvisor all fail firefox specifically) and is also the
*only* documented DataDome bypass for etsy (DataDome weights Firefox-TLS=human, Chromium-TLS=bot).

**Fix (large):** an NSS-class boring2 reconfig behind a Firefox arm — `record_size_limit` (ext 28),
`delegated_credentials` (ext 34), FFDHE groups in supported_groups, real ECH (not GREASE), fixed
(non-shuffled) extension order, and Firefox H2 SETTINGS (keep SETTINGS id 3, Firefox priority tree).
Tracked internally as "Phase B.3"; never landed.

---

## 4. `run_until_idle` exits early on far-future timers

```
crates/event_loop/src/lib.rs:365-368
match result {
    Ok(Ok(())) => break Ok(IdleReason::AllWorkDone),  // <-- returns even if a far-future timer pends
```

`run_event_loop(PollEventLoopOptions::default())` resolves `Ok(())` when no *immediately runnable*
work remains; a pending `setTimeout(fn, 10000)` does not keep it pending. The crate's own test
admits the bug and is disabled:

```
crates/event_loop/src/lib.rs:524
#[ignore = "regression: run_until_idle returns AllWorkDone instead of Timeout when a far-future
            setTimeout is pending — see fix.md"]
async fn timeout_respected() { ... }
```

(Near-future chained timers DO advance — `chained_set_timeout` at `:540` passes — so this is
specifically about *far-future* deferred work.) Effect on SPAs: an app that schedules deferred
hydration / a polling fetch on `setTimeout(_, >0)` causes the drain to return `AllWorkDone` early,
ending the nav before the deferred work runs, even though budget remained. Fix: in `run_until_idle`,
treat `AllWorkDone` as terminal **only if** there are no pending timers/intervals scheduled before
`deadline`; otherwise sleep until `min(next_timer_deadline, deadline)` and continue.

---

## 5. `IntersectionObserver` is one-shot

```
crates/js_runtime/src/js/window_bootstrap.js:3550-3565
observe(target) {
    this._elements.add(target);
    Promise.resolve().then(() => {       // fires ONCE, synthetically
        ... isIntersecting: true ...
        this._callback([entry], this);
    });
}
```

It never re-fires on scroll or DOM mutation. Infinite-scroll feeds (douyin) and viewport-gated
product grids (wildberries) load exactly one batch's worth of content and then stall, because the
"load more when X enters viewport" callback only ever fires for the initially-observed targets.
There are also two empty `observe() {}` stubs at `:2266` and `:2293` (other observer classes) worth
auditing for the same "never fires" shape. Fix: re-fire on DOM mutation / on a synthetic scroll pass
so subsequent batches' sentinels trigger.

---

## 6. SPA budget is a hardcoded allowlist

```
crates/browser/src/page.rs:1969-1983
Some(h) if h.ends_with("twitter.com") || ... || h.ends_with("uber.com") => 90_000,
...
_ => 15_000,
```

douyin, duolingo, ozon, wildberries are not on any heavy tier → 15 s; adidas gets 25 s via the
Akamai-BMP arm (`:1995-2003`). Even once #1 makes the app execute, a 1.5 MB React hydrate in BO's
op-bridged DOM can exceed 15 s. Fix: make the tier content-adaptive (e.g. detect `type="module"`
entry or React/Vue globals and grant the 90 s tier) instead of a per-host literal.

---

## 7. `elementFromPoint` is a hardcoded stub (surviving FP defect)

```
crates/js_runtime/src/js/dom_bootstrap.js:1524
elementFromPoint(x, y) { return this.body; }
```

Ignores `(x,y)` entirely. `document.elementFromPoint(99999, 99999)` returns `body`; real Chrome
returns `null` for out-of-viewport coordinates. One call distinguishes BO. This is the *only*
top-5 fingerprint gap from the 04-29 inventory that is still open (the other four were fixed).
Cheap partial fix: return `null` for coords outside the viewport box; full fix needs layout hit-test.

---

## 8–9. Secondary correctness issues

- **Worker perpetual interval** (`worker_bootstrap.js:261`): `setInterval(drainOnce, 5)` never
  clears. Keeps `pending_intervals>0`, perturbing idle detection (#4) and modeling a 5 ms tick that
  real workers don't expose. Worth replacing with a self-rescheduling `setTimeout` that yields when
  the queue drains.
- **`crypto.subtle` digest-only**: real `digest` (Rust-backed) but `generateKey/importKey/sign/
  verify/encrypt/decrypt/derive*/wrap*` reject `NotSupportedError`. Low priority — most antibot
  paths only `digest` — but a generateKey probe sees a non-Chrome rejection.

## 10. Challenge solvers detected but unsolved (already known, restated for completeness)

- **homedepot (Akamai sec-cpt):** `started_as_seccpt_challenge` is set (`page.rs:1898`), the bundle
  runs, but `is_seccpt_solved` (`:242`, requires `sec_cpt=` + `~3~` + container gone) never trips in
  live nav — the bundle's worker self-solve produces no async progress in the navigate drain (ties
  back to #4/#8). 
- **etsy (DataDome):** `is_datadome_challenge` detects the interstitial and CSP is relaxed, but the
  DataDome WASM solve lives in `vendor_solvers` (out of scope here) so `datadome=` never lands.
  Structurally also gated by #3 (Firefox-NSS TLS is the documented bypass).

---

## Recommended fix order (ROI-ranked)

1. **Module path for document scripts (#1 via #2).** Reuse `load_main_es_module_from_code` +
   `mod_evaluate`; add a host `ModuleLoader` (resolve vs doc URL, recursive import-graph prefetch) +
   dynamic-`import()` callback; gate `find_scripts`/dom_bootstrap to route `type="module"` and any
   `import`/`export`-bearing source there. **Unblocks 5 of 7.** Effort: medium (capability already
   exists). Files: `js_runtime/src/lib.rs:122`, `browser/src/script_runner.rs:44`,
   `js/dom_bootstrap.js:146`, new loader.
2. **`run_until_idle` far-future-timer fix (#4)** + **content-adaptive SPA budget (#6).** Cheap, and
   they let the now-executing app actually finish hydrating. Files: `event_loop/src/lib.rs:316`,
   `browser/src/page.rs:1953`.
3. **IntersectionObserver re-fire (#5).** Unblocks infinite-scroll feeds. File:
   `js/window_bootstrap.js:3543`.
4. **Firefox-NSS TLS arm (#3).** Large but the *only* lever for the 14-site firefox class + etsy
   DataDome. Files: `net/src/tls.rs`, `net/src/h2_client.rs`, `stealth/src/presets.rs`.
5. **elementFromPoint null-for-OOB (#7), worker-interval, subtle-crypto.** Small parity polish.

Note: do **not** re-fix BatteryManager / visualViewport / AudioContext / MediaSession — those named
2026-04-29 gaps are already closed in current code.
