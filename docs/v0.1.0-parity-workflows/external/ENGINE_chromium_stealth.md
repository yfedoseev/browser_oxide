# ENGINE — Chromium-stealth tier (Patchright / rebrowser-patches / nodriver / undetected-chromedriver)

**Audience:** browser_oxide engine engineers evaluating which lessons from the Chromium-CDP-stealth family transfer to BO's own-engine, in-process V8 model.
**Surveyed:** 2026-05-28. Branch `fix/v0.1.0-fix4-canvas-parity`.
**Thesis in one line:** The entire Chromium-stealth tier exists to hide a *CDP control channel* that BO structurally does not have. ~80% of their engineering effort is **free** for BO (we have no `Runtime.enable`, no `cdc_`, no remote debugging endpoint, no chromedriver binary). The residual transferable lessons are all in the *fingerprint / `Function.prototype.toString` / source-leak* class — and BO already implements every one of them. The net new finding for BO is structural, not a code gap: this tier does **not** explain the homedepot/imdb/AWS-WAF gap (those are real-Chrome-trust + JS self-solve-execution gaps), and **nothing in this tier is a vendor-bypass we should port into public crates**.

---

## 1. What the existing repo docs already concluded (cite)

The repo already studied this tier in three places; this doc extends, not duplicates, them.

- **`docs/releases/v0.1.0-parity/12_COMPETITIVE_LANDSCAPE.md` §1.3 + §2.4.** Patchright is "Playwright + CDP-leak patches", measured **88 Pass** on the 126-corpus — *identical to vanilla Playwright within the ±5-site noise floor* (`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`). The doc's verdict: "CDP hiding alone isn't enough … the heavy vendors check far more than navigator.webdriver." §2.4 states BO's structural advantage: "BO has none of these surfaces because it doesn't drive Chrome — it IS the JS environment. There is no DevTools protocol, no remote endpoint, no debugger to detect."
- **`docs/releases/v0.1.0-parity/27_VENDOR_COMPETITIVE_MATRIX.md` §6.** Enumerates exactly what Patchright patches (webdriver→undefined, `--enable-automation` removed / `--disable-blink-features=AutomationControlled` added, `Runtime.enable` not called via isolated ExecutionContexts, closed-shadow-root handling) and proves **Δ=0 vs Playwright on every vendor cluster** (AWS WAF 7/8, DataDome 0/4, Akamai 1/3, Kasada 1/3 loose, Cloudflare 0/7, PerimeterX 0/1). §6 line 220: "The underlying chromium fingerprint (font lists, WebGL bytes, WASM JIT timing) is what vendors fingerprint. Patchright cannot hide what real Chromium reports." §6.1: playwright-stealth is *one site behind* Playwright (amazon-ca) precisely because its `addInitScript` overrides leave a `Function.prototype.toString` / property-descriptor signature — the structural critique of JS-injected stealth.
- **`docs/releases/v0.1.0-parity/43_STRATEGIC_GAP_ASSESSMENT.md` §"To a customer worried about CDP-driver detection".** "We have NO CDP — invisible to CDP-detection heuristics. Patchright spends engineering effort hiding CDP; we don't have CDP to hide. We lead the CDP-driver tier by 15 Pass on the 126-corpus."

**Important correction this doc carries forward (`HANDOFF_2026_05_28b.md` §4 + §3):** the `27_VENDOR_COMPETITIVE_MATRIX.md` line claiming "imdb: Patchright `BLOCKED 117`, Camoufox `L3 1080960`" is the *trustworthy* reading. The task prompt's premise that "Patchright passes homedepot AND imdb" is only **half** true in BO's own measurements: Patchright passes **homedepot** (`L3 1245838`, real-Chrome Akamai-trust) but BO's 2026-05-24 sweep recorded Patchright **BLOCKED 117 on imdb** — i.e. imdb is an AWS-WAF site where Patchright does *not* reliably pass in our data; the engines that pass imdb are real-Chrome Playwright (sometimes) and **Camoufox v150** (`L3 1080960`, reliably). The general public claim that "Patchright passes Cloudflare/Kasada/Akamai/Shape/DataDome" ([Patchright README](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright), [ZenRows](https://www.zenrows.com/blog/patchright)) is **contradicted by our 126-site measurement** and should be treated as marketing, not as a BO target spec.

---

## 2. New external findings — exactly what each repo patches (cited)

Researched via deepwiki MCP on each repo + the rebrowser bot-detector test harness.

### 2.1 Patchright (`Kaliiiiiiiiii-Vinyzu/patchright`, Apache-2.0)

Patched against the Playwright codebase via `patchright.patch`; test impact recorded in `modify_tests.js`. Mechanisms (deepwiki, grounded in the patch files):

| # | Leak / vector | Patchright fix | Patched site |
|---|---|---|---|
| 1 | **`Runtime.enable` CDP side-channel** — calling `Runtime.enable` makes the page's V8 isolate emit `Runtime.executionContextCreated`; a page script can observe the side effect (e.g. a debugger-statement timing or a context-id probe). | **Never calls `session.send('Runtime.enable')`.** Removes it from `CRSession` init. JS runs through an **isolated ("utility") world** via a new `isolatedContext` param on `Frame.evaluate`/`evaluateHandle`; `_onExecutionContextCreated` in `crPage.ts` creates and tracks these. | `crPage.ts`, `frames.ts` |
| 2 | **`Page.addScriptToEvaluateOnNewDocument` main-world leak** — the standard init-script API runs in the page's main world and is enumerable / observable. | **Does not send `Page.addScriptToEvaluateOnNewDocument`.** Instead `addInitScript` → `installInjectRoute()` intercepts the *document HTTP request* via a Playwright Route and rewrites it through an internal URL (`<proto>://patchright-init-script-inject.internal/`), injecting JS into the HTML before parse. Scripts are stored internally, never registered with CDP. | `page.ts` |
| 3 | **`navigator.webdriver === true`** | Driven entirely by the command-line flag change (#4) — no JS override needed. | launch args |
| 4 | **Automation command-line tells** | Adds `--disable-blink-features=AutomationControlled`; removes `--enable-automation`, `--disable-component-update`, `--disable-default-apps`, `--disable-extensions`, `--disable-popup-blocking` (these defaults are themselves a Playwright fingerprint). | launch args |
| 5 | **`Runtime.consoleAPICalled` / `Console.enable` leak** — enabling the Console domain is observable. | **Completely disables the Console API in the driver** — `console.*` events do not work in Patchright (acknowledged trade-off; many console tests are `FIXME_TARGETS` in `modify_tests.js`). | driver |
| 6 | **Closed shadow roots** — vanilla Playwright cannot pierce `mode:'closed'` shadow DOM; failing to interact is itself a behavioral tell. | Selector engines + XPath extended to traverse closed shadow roots. | selector engine |

### 2.2 rebrowser-patches (`rebrowser/rebrowser-patches`, for Puppeteer + Playwright)

The most *surgically documented* of the four. Fixes three named leaks, each tied to a test on the public **rebrowser bot-detector** ([bot-detector.rebrowser.net](https://bot-detector.rebrowser.net/), [repo](https://github.com/rebrowser/rebrowser-bot-detector)). The bot-detector tests are the de-facto industry checklist: `dummyFn` (main-context exec), `exposeFunctionLeak`, `sourceUrlLeak`, `mainWorldExecution`, `runtimeEnableLeak`.

- **`Runtime.enable` leak** — fixed by *manually* acquiring execution-context IDs instead of subscribing via `Runtime.enable`. Three modes via `REBROWSER_PATCHES_RUNTIME_FIX_MODE`:
  1. **`addBinding`** (default): register a random-named binding with `Runtime.addBinding`; for frames, add a script that calls the binding + create an isolated world to dispatch a trigger event; for web workers, call the binding via `Runtime.evaluate`. The `Runtime.bindingCalled` event then yields the context id. **Preserves main-world access, supports workers + iframes.**
  2. **`alwaysIsolated`**: every script runs in an isolated world (`__re__getIsolatedWorld`). Defeats `MutationObserver`-based detection but **loses main-context variable access and worker support**.
  3. **`enableDisable`**: enable `Runtime` just long enough to capture the context id, then immediately disable — minimizes the detection window.
  Implemented by patching Puppeteer's `ExecutionContext` and Playwright's `CRConnection.__re__emitExecutionContext`.
- **`sourceURL` leak** — Puppeteer appends `//# sourceURL=pptr://...` to every `page.evaluate()` script; a page can read this. Patch rewrites to `//# sourceURL=app.js` (or `REBROWSER_PATCHES_SOURCE_URL`). Implemented in `getSourceUrlComment`.
- **utility world name leak** — Puppeteer's default isolated-world name is `'__puppeteer_utility_world__'+version`, readable via CDP-adjacent probes; rebrowser changes it to `util` (or `REBROWSER_PATCHES_UTILITY_WORLD_NAME`).
- README scope: "Helps to avoid Cloudflare and DataDome CAPTCHA pages."

### 2.3 nodriver (`ultrafunkamsterdam/nodriver`, successor to undetected-chromedriver)

- **No WebDriver protocol at all** — talks raw CDP (40+ domains), so it never spawns the `chromedriver` binary and inherits **none of the `cdc_` DOM-variable leaks** that bind to chromedriver. This is its single biggest win and it is *architectural*, exactly like BO's "no driver" stance, but one layer up (nodriver still drives a real Chrome over CDP; BO has no Chrome at all).
- `navigator.webdriver` — not set true because there's no WebDriver layer.
- Command-line flags — configurable via `browser_args` / `Config`; `expert=True` disables web-security/origin-trials (and is *more* detectable — a documented anti-pattern).
- JS execution — `Runtime.evaluate` with optional `context_id`/`unique_context_id` to pick the execution context. nodriver does **not** ship the rebrowser-style `Runtime.enable` avoidance as a headline feature; its stealth bet is "no WebDriver" rather than "no `Runtime.enable`."

### 2.4 undetected-chromedriver (`ultrafunkamsterdam/undetected-chromedriver`, Selenium-based)

Oldest of the four; binary-patch + JS-injection era. Mechanisms (deepwiki, grounded in `patcher.py` / `__init__.py`):

- **`cdc_` leak** — `Patcher.patch_exe()` regex-replaces the `{window.cdc_...;}` block *inside the chromedriver executable* with a padded `{console.log("undetected chromedriver 1337!")}`. This is the canonical chromedriver tell.
- **`navigator.webdriver`** — JS injected via `Page.addScriptToEvaluateOnNewDocument` redefining `window.navigator` so `webdriver` returns `false`.
- **Flags** — `--no-default-browser-check`, `--no-first-run`, `--no-sandbox`+`--test-type`, `--headless=new`, `--window-size`, `--lang`. (Note: it does *not* itself strip `enable-automation` consistently — that's left to the caller's `excludeSwitches`.)
- **JS injection bundle** — mock `window.chrome` (`app`+`runtime`), `navigator.permissions.query`→"granted" for notifications, strip "Headless" from UA, and crucially **`Function.prototype.toString` patched to return `function query() { [native code] }`** for the overridden functions. This last one is the only fingerprint-class lesson in the whole tier — and it is exactly BO's `_maskAsNative` strategy.
- Does **not** address `Runtime.enable` (deepwiki: "the provided codebase does not explicitly detail mechanisms to avoid Runtime.enable detection").

---

## 3. BO code-level analysis — which advantages are free, which lessons transfer

### 3.1 Free for BO (structural immunity) — confirmed against source

BO is in-process V8 via `deno_core` (`crates/js_runtime/src/runtime.rs`). There is no CDP client, no WebSocket debugging endpoint, no chromedriver binary, no `--enable-automation` launch path. Therefore:

| Tier vector | BO status | Why free |
|---|---|---|
| `Runtime.enable` side-channel (Patchright #1, rebrowser, the dominant rebrowser-bot-detector test) | **N/A — immune** | BO never calls `Runtime.enable`. The page's JS is evaluated by `JsRuntime::execute_script` inside the same isolate; there is no separate CDP "controller" world to leak a second execution context, and no `Runtime.executionContextCreated` event for a page script to observe. |
| `cdc_` chromedriver DOM variables (UC §2.4) | **N/A — immune** | No chromedriver binary exists. |
| `Console.enable` / `Runtime.consoleAPICalled` (Patchright #5) | **N/A — immune, *and* better** | BO has a real, working `console` (`console_bootstrap.js`) with no observable CDP enable. Patchright had to *break* console to hide the leak; BO keeps console functional. |
| `sourceURL=pptr://` / utility-world-name leak (rebrowser §2.2) | **N/A — immune** | Confirmed: `grep sourceURL crates/js_runtime/src` → **zero hits**. BO does not wrap page scripts in a `page.evaluate` envelope, so there is no injected `//# sourceURL` comment and no named isolated "utility world" to enumerate. |
| `Page.addScriptToEvaluateOnNewDocument` main-world enumerability (Patchright #2) | **N/A — immune** | `runtime.rs:30-35`: BO's `init_scripts` run in-process "AFTER all built-in bootstraps but BEFORE any parsed-HTML `<script>` tags" — same *timing* as CDP addScript but with **no CDP registration record** and no separate world. The comment even notes it "Mirrors Chromium's `Page.addScriptToEvaluateOnNewDocument`" — BO gets the capability without the leak. |
| automation launch flags / `HeadlessChrome/` UA tail (all) | **N/A — immune** | UA comes from the `StealthProfile` struct (`crates/stealth/src/profile.rs`); there is no Chromium command line. |
| closed shadow root interaction tell (Patchright #6) | **N/A** | BO owns the DOM (`crates/dom`), so there is no "driver cannot pierce closed shadow root" behavioral gap — though see §3.3 caveat: BO must still *implement* shadow DOM correctly for sites that use it. |

**Quantified:** this is the lead `43_STRATEGIC_GAP_ASSESSMENT.md` already monetized — "+15 Pass over the CDP-driver tier." That lead is real and structural; nothing in this research erodes it.

### 3.2 Fingerprint-class lessons that DO transfer — and BO already ships them all

The only non-CDP lesson in the tier is the `Function.prototype.toString` / property-descriptor authenticity problem (the reason playwright-stealth is *behind* vanilla Playwright per `27 §6.1`, and the reason undetected-chromedriver hand-patches `toString`). BO already addresses each:

- **`navigator.webdriver`** — `crates/js_runtime/src/js/window_bootstrap.js:991-995` defines it on `Navigator.prototype` returning **`false`** (not `undefined`) via `_maskFunction(... 'get webdriver')`, and the comment (lines 983-990) records the K2-DIFF Kasada finding that `webdriver:"undefined"` is itself the headless tell — BO deliberately matches modern Chrome's `false`. Re-applied on the duplicated `_NavProto` path at `1657-1660`, in iframes at `dom_bootstrap.js:2784`, and in workers at `worker_bootstrap.js:156`. **This is more correct than Patchright's `undefined` and undetected-chromedriver's `false`-via-injection.**
- **`Function.prototype.toString` authenticity** — `_maskAsNative` / `_maskFunction` is BO's systematic equivalent of UC's hand-patched `toString`, applied across the whole Navigator surface (`window_bootstrap.js:1029-1035`) and **every console method** (`stealth_bootstrap.js:171-181`). The console mask comment (160-170) cites the exact Kasada `/ofc/r` sensor that caught the un-masked `log(...args){ core.ops.op_console_log(...) }` leak — i.e. BO already fixed the leak class that the Chromium tier is famous for, and validated it against a live vendor sensor.
- **`window.chrome` object** — `window_bootstrap.js:1571-1584`: full `chrome.app`/`runtime`/`csi`/`loadTimes` with native-`toString` `csi`/`loadTimes`, and `cleanup_bootstrap.js:255` correctly *removes* it on iOS Safari. This is UC's `window.chrome` mock done at engine fidelity rather than JS injection.

**Conclusion:** there is **no fingerprint code-gap** that this tier reveals. BO is at or above their bar on every transferable item.

### 3.3 The one genuinely transferable *idea* (not a code gap): WebIDL-layer vs JS-layer masking

`audit/02_CAMOUFOX_V150_OVERVIEW.md` §6 already flagged the analogous Camoufox point: Camoufox masks at the C++ WebIDL binding layer, so `Object.getOwnPropertyNames` / descriptor walks cannot enumerate the override, whereas BO's `_maskAsNative` operates in JS and *can* in principle be enumerated by a sandbox-escape probe. The Chromium-stealth tier's `alwaysIsolated` mode (rebrowser) is the same instinct — run the controller's code where the page cannot see it. **For BO this is low priority**: it only matters against an attacker that walks every property descriptor looking for a JS-defined getter, and our `_maskAsNative` already normalizes descriptors + `toString`. The deeper move (define these at the Rust/`op2`/binding layer so no JS getter object exists at all) is a large refactor with no measured site flip behind it. File as a watch-item, not a fix.

### 3.4 Why this tier does NOT explain the homedepot/imdb/AWS gap

This is the load-bearing finding for the prompt's premise:

- **homedepot** — Patchright passes it (`L3 1245838`) for the same reason vanilla Playwright does: **real-Chrome Akamai-trust + the sec-cpt bundle self-solving in a real Chrome runtime**, *not* because of any CDP-leak patch (Δ=0 vs Playwright, `27 §6`). BO *already* beats Patchright here on a good roll (`HANDOFF_2026_05_28b` §3: BO 3/5, v150 0/5) — the BO gap is **sec-cpt rotation flakiness** (`R-AKAMAI-SECCPT-FLAKE`), an Akamai self-solve-reliability problem, not a CDP-stealth problem.
- **imdb / AWS-WAF cluster** — `HANDOFF_2026_05_28b` §4 proved with the offline oracle that AWS challenge.js **proceeds with BO's fingerprint and calls `forceRefreshToken`** — it is *not* a fingerprint or CDP-detection bail. The live-path blocker is the **50 ms inter-script `run_until_idle` drain** in `build_page_with_scripts_init_and_storage` (`crates/browser/src/page.rs` ~3535) starving challenge.js's blob-URL PoW Web Worker of async progress. No Chromium-stealth patch touches this; it is a BO async-execution/drain gap.

**The single most useful thing this tier confirms:** the path to homedepot/imdb is *not* "hide more automation tells" (BO has none); it is **(a) make challenge.js's async self-solve run to completion in the live nav path** (the `HANDOFF_2026_05_28b` §5.1 lever) and **(b) reliability-harden the Akamai sec-cpt self-solve**. The Chromium tier's value to BO is purely as **confirmation that the CDP axis is closed** — stop looking there.

---

## 4. Ranked fix list (ROI order)

All effort estimates assume one engineer familiar with the relevant crate. "Public engine" = fixable in public crates per CLAUDE.md; per-vendor encoder/bypass code belongs in private `vendor_solvers`.

### FIX-CS-1 — Adopt the rebrowser bot-detector as a standing regression gate **(do first; near-zero risk, locks in the structural lead)**
- **What:** Add an offline-served copy of `rebrowser-bot-detector`'s test page to BO's harness and assert BO passes `runtimeEnableLeak`, `dummyFn`, `exposeFunctionLeak`, `sourceUrlLeak`, `mainWorldExecution` (BO should pass all by construction). Wire into the existing in-VM probe path (like the `awswaf_probe`/`worker_caps` oracles).
- **Why:** BO is *structurally* immune to all five, but there is no test asserting it stays immune across `deno_core`/V8 bumps (`24_RISK_REGISTER.md` already flags deno_core drift). This converts a free advantage into a *guarded* free advantage.
- **Effort:** 0.5–1 day. **Confidence:** high. **Expected site impact:** 0 flips (it's a guard), but protects the +15 CDP-tier lead. **Public engine.**

### FIX-CS-2 — AWS live-nav async drain (the actual lever behind imdb/AWS cluster) **(highest site ROI)**
- **What:** Per `HANDOFF_2026_05_28b` §5.1: give AWS-WAF-challenge pages a longer post-script `run_until_idle` drain (or fix the dropped async continuation) in `crates/browser/src/page.rs` ~3340–3620 (the 50 ms inter-script drain at ~3535), so challenge.js's blob-URL PoW worker spawns, POSTs the token, and BO's existing cookie-gained re-fetch returns content. This research *re-confirms* it is the lever (not a CDP/fingerprint fix).
- **Effort:** 3–6 days (instrument → tune drain → re-measure with `run_delta_headtohead.py`). **Confidence:** medium. **Expected site impact:** imdb + amazon-in (hard) + amazon-fr/jp/com-au (reliability) + duolingo ≈ **up to 6–7 sites**, the bulk of the v150 gap. **Public engine.**
- *Note:* listed here for ROI ordering completeness; it is owned by the AWS workflow, not this Chromium-stealth research. This doc's contribution is the negative result — the Chromium-stealth tier does **not** offer a shortcut to it.

### FIX-CS-3 — homedepot Akamai sec-cpt reliability hardening (`R-AKAMAI-SECCPT-FLAKE`)
- **What:** Harden `is_seccpt_solved` so BO's already-winning homedepot pass is reliable 5/5 instead of ~60%. Confirmed by this research that Patchright's win here is real-Chrome-trust, *not* a CDP patch — so the BO fix is purely in the self-solve reliability path.
- **Effort:** 2–4 days. **Confidence:** medium. **Expected site impact:** locks in 1 site BO already beats v150 on (defensive). **Public engine** for the reliability logic; the actual sec-cpt solving sits at the public/`vendor_solvers` boundary — keep generic timer/state logic public, any Akamai-specific encoder private.

### FIX-CS-4 — (watch-item, not a fix) WebIDL/binding-layer masking for the highest-value getters
- **What:** Optionally move the most-probed getters (`navigator.webdriver`, `navigator.plugins`, WebGL `getParameter`) from JS `_maskAsNative` getters to Rust/`op2`-backed bindings so no enumerable JS getter object exists — closing the descriptor-walk vector that Camoufox closes natively and rebrowser's `alwaysIsolated` mode approximates.
- **Effort:** 1–2 weeks (large refactor across `crates/js_runtime/src/js/*_bootstrap.js` + `extensions/`). **Confidence:** low (no measured site flip behind it; `_maskAsNative` already normalizes descriptor+toString). **Expected site impact:** 0 measured today; speculative against future descriptor-walk probes. **Public engine.** **Recommendation: defer** — document as a monitoring item, do not schedule.

### Explicitly NOT a fix list (do-not-port)
- Patchright's `--disable-blink-features=AutomationControlled` / `excludeSwitches` work, UC's `cdc_` binary patch, the `Runtime.enable` avoidance modes, nodriver's no-WebDriver bet, the `sourceURL`/utility-world renames — **all N/A for BO** (no CDP, no chromedriver, no Playwright envelope). Porting any of them would be dead code. None are vendor-bypass either, so the CLAUDE.md `vendor_solvers` rule isn't even engaged — they're simply inapplicable.

---

## 5. Sources

- deepwiki MCP: `Kaliiiiiiiiii-Vinyzu/patchright`, `rebrowser/rebrowser-patches`, `ultrafunkamsterdam/nodriver`, `ultrafunkamsterdam/undetected-chromedriver` (queried 2026-05-28).
- [Patchright (GitHub)](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright) · [patchright-python](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright-python) · [PyPI](https://pypi.org/project/patchright/)
- [rebrowser-patches (GitHub)](https://github.com/rebrowser/rebrowser-patches) · [rebrowser-bot-detector (GitHub)](https://github.com/rebrowser/rebrowser-bot-detector) · [bot-detector.rebrowser.net](https://bot-detector.rebrowser.net/) · [Rebrowser docs](https://rebrowser.net/docs/rebrowser-bot-detector)
- [nodriver (GitHub)](https://github.com/ultrafunkamsterdam/nodriver) · [undetected-chromedriver (GitHub)](https://github.com/ultrafunkamsterdam/undetected-chromedriver)
- [ZenRows — How to Scrape with Patchright](https://www.zenrows.com/blog/patchright) · [anti-detect-browser-tools-tech-comparison](https://github.com/pim97/anti-detect-browser-tools-tech-comparison)
- BO internal: `docs/releases/v0.1.0-parity/12_COMPETITIVE_LANDSCAPE.md`, `27_VENDOR_COMPETITIVE_MATRIX.md`, `43_STRATEGIC_GAP_ASSESSMENT.md`, `audit/02_CAMOUFOX_V150_OVERVIEW.md`, `docs/HANDOFF_2026_05_28b.md`.
- BO source: `crates/js_runtime/src/runtime.rs:30-35`, `crates/js_runtime/src/js/window_bootstrap.js:983-995,1029-1035,1571-1584,1657-1660`, `crates/js_runtime/src/js/stealth_bootstrap.js:160-181`, `crates/js_runtime/src/js/dom_bootstrap.js:2784`, `crates/js_runtime/src/js/worker_bootstrap.js:156`, `crates/js_runtime/src/js/cleanup_bootstrap.js:255`, `crates/browser/src/page.rs` ~3535.
