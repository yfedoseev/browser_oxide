# 07 — The No-CDP Structural Advantage (the competitive SOTA thesis)

**Date:** 2026-05-29
**Author:** frontier research agent (competitive-moat thesis)
**Status:** thesis + evidence + code-verified moat audit + the one caveat that can break it
**Scope:** Quantify browser_oxide's *no-CDP / no-automation-protocol* position as the
structural path to beat every CDP-based competitor (Camoufox v150, Playwright,
Patchright, nodriver, undetected-chromedriver) on the Kasada/DataDome/Akamai
frontier — and pin down exactly what BO must NOT break to keep the moat.

> **One-line thesis.** Every engine that fails canadagoose/hyatt/realtor/etsy/bestbuy
> drives a real browser over a *control channel* — CDP (Puppeteer/Playwright/Patchright/
> nodriver/UC) or the Firefox **Juggler** pipe (Camoufox). Modern Kasada/DataDome/Akamai
> fingerprint that channel's residue. **BO has no control channel at all** — it embeds V8
> in-process via `deno_core`, so it is on the *same side of the detection line as a real
> non-CDP human browser* and the *opposite side from every competitor*. The prior evidence
> (real no-CDP Chrome passes Kasada from this very IP, zero interaction) proves the residual
> gap is engine-fidelity, not a moat we lack. This doc converts that into a named,
> code-verified competitive advantage and a guard list.

**Reading order / cited prior art (do NOT duplicate):**
- `docs/v0.1.0-parity-workflows/external/DETECT_vectors.md` §5 — "CDP / automation-leak
  vectors — BO is structurally immune." This doc *quantifies and audits* that claim.
- `docs/v0.1.0-parity-workflows/external/ENGINE_chromium_stealth.md` §3.1 — the per-vector
  "free for BO" table (Patchright/rebrowser/nodriver/UC), grounded in source.
- `docs/v0.1.0-parity-workflows/external/ENGINE_camoufox_v150.md` §0-2 — v150 wins on
  *runtime completeness*, not fingerprint; carries a Juggler surface (this doc's §3.2).
- `docs/v0.1.0-frontier-workflows/06_NOCDP_ORACLE_METHOD.md` §1.1, §2 — the oracle that
  *exploits* this thesis; the captured no-CDP PASS evidence.
- `docs/releases/v0.1.0-parity/12_COMPETITIVE_LANDSCAPE.md` §1.3/§2.4,
  `27_VENDOR_COMPETITIVE_MATRIX.md` §6, `43_STRATEGIC_GAP_ASSESSMENT.md` — the measured
  +15-Pass lead over the CDP-driver tier and the Δ=0 Patchright-vs-Playwright result.
- MEMORY: `proxy_not_the_problem`, `state_2026_05_15_playwright_ab_decisive` — the
  CDP-confound that nearly sent the project down a false "IP-ban" path.

---

## 1. The exact detection mechanism — what each competitor leaks, and which vendor catches it

This is the catalog the thesis rests on. Every competitor that fails the frontier
leaks at least one *control-channel* artifact. BO leaks none of them (§4). The table
maps **leak → competitor that emits it → mechanism the vendor uses → JS-observable?**

### 1.1 CDP-driver family (Playwright, Patchright, Puppeteer, nodriver, undetected-chromedriver)

| # | Control-channel artifact | Competitors that leak it | Vendor detection mechanism | JS-observable from page? |
|---|---|---|---|---|
| C1 | **`Runtime.enable` execution-context side-channel** — calling `Runtime.enable` makes the page's V8 isolate emit `Runtime.executionContextCreated`; the historical exploit serialized an `Error` via `Runtime.consoleAPICalled` and watched a `stack` getter fire. | Playwright, Puppeteer, vanilla CDP. (Patchright/rebrowser/Kameleo *patch* it; nodriver/UC do not fully.) | Anti-bot registers the getter / probes the extra context id. **rebrowser-bot-detector** names this `runtimeEnableLeak`; it is the #1 industry CDP tell. | **Yes** (was the canonical test). |
| C2 | **`cdc_` / `$cdc_` / `$wdc_` ChromeDriver DOM globals** | undetected-chromedriver (pre-patch), Selenium/ChromeDriver | direct property probe on `window`/`document`. UC's whole `patcher.py` exists to regex these out of the chromedriver binary. | **Yes.** |
| C3 | **`//# sourceURL=pptr:` / `__puppeteer_evaluation_script__` / utility-world name** | Puppeteer, Playwright | regex on script names surfaced in `error.stack`; world-name enumeration. rebrowser tests = `sourceUrlLeak`. | **Yes** (via stack). |
| C4 | **`page.exposeFunction` binding leak** | Puppeteer, Playwright | the exposed binding has a non-native `toString`/descriptor signature. rebrowser test = `exposeFunctionLeak`. | **Yes.** |
| C5 | **isolated-world / main-world execution mismatch** — patched stealth runs init in an isolated world, so main-world objects are missing | Patchright (`alwaysIsolated`-style), rebrowser `alwaysIsolated` mode | probe that a known main-world global set by page script is visible to the controller's code (rebrowser `mainWorldExecution`). The *fix for one leak creates another.* | **Yes.** |
| C6 | **`navigator.webdriver === true`** | any WebDriver-based default | direct read. (Most now mask, but in some stacks the mask is itself a CDP-set flag.) | **Yes.** |
| C7 | **automation launch tells** (`--enable-automation`, `HeadlessChrome/` UA tail, default-flag fingerprint) | Playwright/Puppeteer defaults | flag-induced UA/feature fingerprint; `--enable-automation` sets `navigator.webdriver`. | **Indirectly** (via UA + feature deltas). |
| C8 | **`Page.setBypassCSP` / `Emulation.*` overrides** | any CDP stealth that uses CDP to spoof UA/timezone | UA-version vs feature-support mismatch (claim v115, expose v128 features); CSP-bypass = "big red flag" (rebrowser sensitive-CDP-methods doc). | **Yes** (feature-probe). |
| C9 | **`Console.enable` / `Runtime.consoleAPICalled`** | CDP that enables the Console domain | observable via the same serialization path as C1. Patchright had to **break `console.*`** entirely to hide it. | **Yes** (historically). |

**2026 nuance (Castle, May 2025 V8 change).** Two V8 commits "Prevent side effects during
error preview" — DevTools no longer fires user-defined getters when previewing an `Error`,
so the *classic* C1/C9 console-getter signal **silently stopped working** for everyone.
This does **not** retire the CDP axis (vendors moved to context-id and timing probes and
to multi-signal behavioral models) — but it is a clean illustration of why a *signal-by-signal*
moat is fragile and a **structural** moat (no channel at all) is not. BO's immunity to C1/C9
is unconditional and survives any such V8 flip-flop. Source: Castle, "Why a classic CDP bot
detection signal suddenly stopped working."

### 1.2 Firefox-Juggler family (Camoufox v150 — the only competitor that beats BO on the frontier today)

Camoufox does **not** use CDP. It drives Gecko over the **Juggler protocol** — Playwright's
custom Firefox transport, a remote-debugging *pipe* + XPCOM `Juggler`/`Dispatcher`/
`BrowserHandler` components and a JS "Page Agent" injected into pages (deepwiki `daijro/camoufox`,
Architecture / Juggler Protocol §6.1). This is a genuinely different (and harder-to-detect)
surface than CDP, which is exactly why Camoufox wins where Patchright/Playwright fail. But it
is **still an automation control surface**, and Camoufox spends ongoing engineering to hide it:

| # | Juggler/automation artifact | Camoufox's mitigation | Residual risk |
|---|---|---|---|
| J1 | `Navigator::Webdriver()` defaulting to `true` under Juggler (the `0-playwright.patch` shipped it `true`) | `1-leak-fixes.patch` re-patches it to return **`false`** in C++ | none if the patch holds — but it is a *patch they must maintain*, not an absence. |
| J2 | Playwright Page-Agent JS injected into the page | sandboxed / "handled in an isolated scope outside of the page" to be "undetectable through JavaScript inspection" | the agent + its isolation are a surface; any sandbox-escape probe or future Gecko change re-exposes it. This is the structural cost BO does not pay. |
| J3 | Juggler-required prefs in `camoufox.cfg` (input events, data reporting, process isolation, `dom.ipc.processPrelaunch.enabled=false`) | chosen to "not directly expose automation indicators to in-page JS" | a configuration fingerprint; prefs are not zero-information. |
| J4 | remote-debugging pipe / marionette-class indicators | isolated via XPCOM module boundary | the pipe exists; isolation is the defense, not removal. |

**The competitive point:** Camoufox's frontier wins come from **(a) Firefox's real event loop
running challenge.js's async self-solve to completion** (`ENGINE_camoufox_v150.md` §0-2, the
runtime-completeness lever) and **(b) C++-level fingerprint spoofing** (`DETECT_vectors.md` §6,
axis-2). Its *automation surface* (J1-J4) is a **liability it actively manages**, not an asset.
BO has the analogous fingerprint axis to close (`DETECT_vectors.md`) and the runtime-completeness
lever to land (`ENGINE_camoufox_v150.md` FIX-1, the live-nav drain) — but BO carries **zero** of
J1-J4 and **zero** of C1-C9. That is the moat.

---

## 2. Does a no-CDP real browser pass? (evidence ⇒ the gap is engine-addressable, and the moat is real)

**Yes — captured, repeatable, from this datacenter IP, zero interaction.** This is the
load-bearing fact (`06_NOCDP_ORACLE_METHOD.md` §2; MEMORY `state_2026_05_15_playwright_ab_decisive`):

| Site (vendor) | No-CDP real Chrome 147, this IP, 0 interaction | Playwright/CDP "real Chrome", same IP | Reading |
|---|---|---|---|
| canadagoose (Kasada) | PASS — real product `<title>` | Kasada 429, empty `<title>` | the **CDP channel** is what Kasada blocks, not the IP/browser |
| hyatt (Kasada) | PASS | 429 | same |
| realtor (Kasada) | PASS | 429 | same |

Kasada's 429 challenge page has an **empty title**; the real product titles are impossible
if Kasada had blocked. This rules out, *with measurement*: IP reputation (real Chrome passes
from this IP), behavioral absence (zero mouse/scroll, still passes), paid-farm requirement
(vanilla first-visit). It leaves exactly two classes of explanation for why a given client
*fails*: (i) it leaks a control-channel artifact (C1-C9 / J1-J4 — the CDP competitors), or
(ii) it has a passive static engine-vs-real-Chrome fingerprint divergence (BO's residual).

**Why this proves the moat, not just the gap.** BO is in class (ii) only — it has *none* of
the class-(i) artifacts (§4, source-verified). So BO sits beside the no-CDP real Chrome that
**passes**. Every CDP competitor sits in class (i) and gets the empty-title 429. **There exists
a region of the detection space — "no control channel + correct-enough fingerprint" — that BO
can occupy and no CDP-based engine can.** Camoufox reaches the frontier from a *third* position
(Juggler, harder to detect than CDP, plus a real Firefox event loop) — but it pays J1-J4 to
stay there and still loses to BO across the broad 126-corpus on the CDP-driver tier
(`43_STRATEGIC_GAP_ASSESSMENT.md`: +15 Pass).

⇒ **The Kasada cluster is ENGINE-ADDRESSABLE by construction** and BO's no-CDP position is the
reason it is *worth* working: BO is one of only two client classes (it + no-CDP real Chrome)
that can plausibly pass, and the only *programmable* one.

---

## 3. The concrete engine path (file:line) + how the no-CDP advantage helps

### 3.1 BO's in-process V8 navigate path — there is no control channel (verified)

`Page::navigate` runs V8 **in-process** through `deno_core`, never over a protocol:
- `crates/browser/src/page.rs:459` — `BrowserJsRuntime::with_options(...)` builds the isolate;
  `page.rs:382,400,475,492,525` — page/external scripts run via
  `event_loop.execute_script` / `execute_script_with_name(&script.code, url)`. No
  `Runtime.evaluate`, no round-trip, no second "controller" world.
- `crates/js_runtime/src/runtime.rs:285-297` — caller `init_scripts` run **in-process** after
  built-in bootstraps, *named `<anonymous>`* (V8's eval-default tag) specifically so no
  `init_script_N` / browser_oxide identifier can surface in `Error.stack` (the comment records a
  prior VM trace that literally captured `at h (<init_script_0>:51:34)` — now scrubbed). This is
  the in-process analogue of C3 (`sourceURL`/world-name) closed *by construction*.
- `crates/js_runtime/src/runtime.rs:32-35` — `init_scripts` is documented as the
  "`Page.addScriptToEvaluateOnNewDocument` CDP command" capability — **but delivered with no CDP
  registration record and no separate world** (`ENGINE_chromium_stealth.md` §3.1).

**How no-CDP helps each frontier vendor:** the vendor's first-line `runtimeEnableLeak` /
utility-world / `cdc_` / `sourceURL` probes (Kasada `ips.js` VM, DataDome `tags.js` device-check,
Akamai `sensor_data`) **return clean for BO with no work** — BO is past the gate the CDP
competitors die at, landing it in the same env-probe path a real no-CDP Chrome reaches. The
residual work is then pure fingerprint/runtime parity (the other frontier docs), not channel-hiding.

### 3.2 THE MOAT CAVEAT — BO *does* ship a CDP server crate; it must stay OFF the stealth path

**This is the single most important guard in this doc and it is not in the prior docs.**

`crates/protocol/` is a **full Chrome DevTools Protocol WebSocket server** — by its own
doc-comment, it lets "Puppeteer and Playwright drive browser_oxide as a drop-in replacement for
headless Chrome" (`crates/protocol/src/lib.rs:1-5`). It implements `Runtime.enable`
(`crates/protocol/src/session.rs:226`), binds a `TcpListener` on `127.0.0.1:<port>`
(`crates/protocol/src/server.rs:60,155`), and exposes `webSocketDebuggerUrl`
(`server.rs:534,551`). `crates/browser/Cargo.toml:36` lists `protocol` as a (non-optional) dep.

**Why the moat still holds — verified:**
- `CdpServer::start*` is invoked **only** from `crates/browser/tests/browser_comparison.rs`
  (the A/B harness that drives BO over CDP for benchmarking) — grep across the whole tree finds
  **zero** non-test, non-protocol-crate callers of `CdpServer::start*`.
- `Page::navigate` / `page.rs` do **not** touch `protocol::` at all (the only `protocol`
  hits in `page.rs:1189,1206` are unrelated comments about HTTP-protocol CHL markers).
- So in production navigation the CDP server is **never bound**, no port is opened, and there is
  **nothing on the wire or in-process for a vendor to detect**. The moat is intact *for the
  navigate path*.

**The guard (must-not-break list):**
1. **Never start `CdpServer` in a stealth/production navigate.** It opens a real
   `--remote-debugging`-equivalent endpoint; running a frontier nav while it is bound reintroduces
   the *entire* C1-C9 surface (you would have literally built the thing BO's moat is the absence of).
2. **Keep `protocol` an isolated, opt-in tool** — ideally behind a Cargo feature (it is currently
   a plain dep; a `cdp-server` feature off-by-default would make the moat *mechanically* enforced,
   matching the `deny.toml` discipline in CLAUDE.md). **Recommend** gating it.
3. **Do not "add CDP for tooling" to the engine.** Any future need to drive BO externally must use
   the in-process API or the out-of-band oracle (`06_NOCDP_ORACLE_METHOD.md`), never a live CDP
   endpoint during a measured stealth run.
4. **Do not regress the `<anonymous>` script naming** (`runtime.rs:285`) or the `Error.stack`
   scrub posture — those close the in-process analogue of C3 and are the only way a non-CDP engine
   could still leak a "tooling" identifier.

### 3.3 Worker / async model — keep it native, do not bolt on a protocol

`crates/js_runtime/src/extensions/worker_ext.rs` runs each Web Worker on its own thread with a
real `run_event_loop` (`ENGINE_camoufox_v150.md` §3.2). The frontier runtime-completeness lever
(live-nav drain, `ENGINE_camoufox_v150.md` FIX-1 / `page.rs:1678,1730,1881-2129`) is the analogue
of "let Firefox's event loop finish challenge.js" — and it is achievable **purely in-process**, so
landing it does **not** cost the moat. Guard: implement the drain as a generic "keep pumping while
async work is outstanding" predicate; never introduce an external scheduler/protocol to do it.

---

## 4. The no-CDP-oracle capture + diff validation plan (prove the moat, then prove the residual)

The oracle (`06_NOCDP_ORACLE_METHOD.md`) is the instrument; this is the moat-specific use of it.

**Step A — Prove BO leaks no control-channel artifact (the moat assertion, as a standing test).**
Adopt the **rebrowser-bot-detector** test page served offline and assert BO passes all five:
`runtimeEnableLeak`, `dummyFn`, `exposeFunctionLeak`, `sourceUrlLeak`, `mainWorldExecution`
(`ENGINE_chromium_stealth.md` FIX-CS-1). BO should pass **by construction** — but there is no
test asserting it stays clean across `deno_core`/V8 bumps. Wire it through the existing in-VM probe
path (`awswaf_probe.rs` pattern). *Add an extra assertion this doc requires:* in the same harness,
spin up `CdpServer::start_ephemeral` and confirm the detector now **fails** `runtimeEnableLeak`
— this proves (i) the detector works and (ii) the moat is exactly "CDP server off." 0.5-1 day.

**Step B — Prove a no-CDP real browser passes (re-confirm the §2 verdict).**
`ab_harness/nocdp.sh <slug> <url>` (Tier-0, zero automation surface) → non-empty product title +
screenshot for canadagoose/hyatt/realtor. This is the only valid oracle baseline; **never** use
Playwright/Patchright/Camoufox as the reference (they are class-(i) and get the poisoned 429 —
the 2026-05-15 confound). Re-confirm periodically (verdicts drift).

**Step C — Bound BO's residual to a named field list (the K2-DIFF).**
With the channel proven clean (Step A) and the target proven passable (Step B), the *only*
remaining diff is fingerprint/runtime. Capture the real passing `/tl` (Kasada) via
`tl_capture.sh` (SSLKEYLOGFILE + tcpdump, TLS-intact) → `tl/hyatt.tl_body.bin`; replay the same
challenge offline through BO (`nocdp_oracle.rs`, `awswaf_probe.rs`-derived) capturing BO's `/tl`
**pre-XOR**; decode both (`base64→json→base64→xor(omgtopkek)`) and field-diff
(`06_NOCDP_ORACLE_METHOD.md` §3.4). The net-side `/tl` log already exists
(`crates/net/src/lib.rs:680`) to confirm whether BO even *reaches* the POST or bails pre-network
(the Kasada silent-bail / AWS live-nav-drain execution-divergence class).

**Step D — Attribute, fix, re-diff.** Each divergent field → public-engine-stub vs `vendor_solvers`.
Re-run Steps C-D until the list is empty or only `vendor_solvers` fields remain. The moat (Step A)
guarantees you are diffing the *fingerprint*, not chasing a phantom channel leak.

---

## 5. Honest verdict (engine-addressable / vendor_solvers / IP-geo) — per frontier cluster

The no-CDP advantage is **real, measured, and structural**, but it is a *gate-pass*, not a
*full solve*: it gets BO into the same env-probe path as a passing no-CDP Chrome. Whether BO then
passes depends on the fingerprint/runtime parity that the other frontier docs own.

| Cluster | No-CDP real browser passes? | Does the no-CDP moat help? | Residual after the moat | Verdict |
|---|---|---|---|---|
| **Kasada** (canadagoose/hyatt/realtor) | **YES**, this IP, 0-interaction (§2) | **Decisively** — it is the gate the CDP competitors die at; BO is past it for free | axis-2 *lies* + child-realm propagation (JS-spoof residual BO can minimize, not zero, vs Camoufox C++); bounded by K2-DIFF to a finite field list | **ENGINE-ADDRESSABLE**, moat-enabled. Pursue if K2-DIFF ≤3 public-stubable fields; the deep VM env-probe field, if any, is `vendor_solvers`. |
| **DataDome** (etsy) | partial (yelp = interactive captcha = human gate) | **Yes** for the `tags.js`/device-check channel probes (BO clean) | daily-rotating-key WASM signal solver | **MIXED** — channel-clean (moat), but the WASM key is `vendor_solvers` (`VENDOR_datadome.md`). |
| **Akamai** (bestbuy / homedepot) | homedepot engine-tractable; bestbuy = no from-scratch engine passes | **Yes** for `sensor_data` channel probes | sec-cpt self-solve reliability (homedepot) + per-day BMP obfuscation/PoW (bestbuy) | homedepot **ENGINE-ADDRESSABLE** (reliability, `R-AKAMAI-SECCPT-FLAKE`); bestbuy BMP PoW = `vendor_solvers`. The live-nav drain (an *engine* fix, in-process, moat-safe) is the lever Method-C ordering pinpoints. |
| **AWS WAF** (imdb / amazon-in) | n/a (offline oracle proves self-solve works) | indirect — BO's fingerprint is *accepted*; the gap is execution, not channel | live-nav async drain (`ENGINE_camoufox_v150.md` FIX-1, `page.rs:1881-2129`) | **ENGINE-ADDRESSABLE execution gap**, moat-safe (in-process drain, no protocol). Highest site ROI. |
| **In-house geo** (wildberries/ozon) + **Firefox-sig** (douyin) | unknown / likely region-gated; douyin = Firefox value distribution | no — these are not control-channel gated | RU/CN geo IP and/or a Firefox-only signature that contradicts BO's Chrome identity | **IP-GEO-BOUND** (wildberries/ozon — confirm only with a captured hard-403 from no-CDP in-region Chrome, per MEMORY rule) / **out-of-identity** (douyin = `vendor_solvers`/skip, `vNext/05`). The moat does not apply. |

**Bottom line — the SOTA thesis, stated precisely.**
1. BO occupies a detection region — **no control channel + programmable fingerprint** — that **no
   CDP-based competitor can reach** (they all leak C1-C9) and that **Camoufox reaches only from a
   different, self-maintained Juggler position** (J1-J4) and still trails BO on the broad CDP-driver
   tier by +15 Pass (`43_STRATEGIC_GAP_ASSESSMENT.md`).
2. The moat is **measured** (no-CDP Chrome passes Kasada from this IP; CDP "Chrome" gets 429) and
   **source-verified** (no `Runtime.enable`/`cdc_`/`sourceURL`/utility-world surface on the
   navigate path; `<anonymous>` script naming; in-process V8).
3. The moat is **conditional on one thing**: `crates/protocol`'s CDP server must stay **off** the
   stealth path (verified: only `tests/browser_comparison.rs` ever starts it; `Page::navigate`
   never does). **Recommend** gating `protocol` behind an off-by-default Cargo feature to enforce
   this mechanically.
4. With the moat held, the frontier reduces to the **fingerprint/runtime parity** the other
   frontier docs own (Kasada K2-DIFF, AWS/booking/duolingo live-nav drain, DataDome/Akamai WASM/PoW
   = `vendor_solvers`). The moat is what makes that residual *worth* closing: BO is one of only two
   client classes that can pass, and the only programmable one.

---

## 6. Sources

- BO source (verified this session): `crates/browser/src/page.rs:382,400,459,475,492,525,1678,1730,1881-2129`;
  `crates/js_runtime/src/runtime.rs:32-35,285-297`; `crates/protocol/src/lib.rs:1-5`,
  `src/server.rs:60,155,534,551`, `src/session.rs:226`; `crates/browser/Cargo.toml:36`;
  `crates/browser/tests/browser_comparison.rs:662,768,...` (sole `CdpServer::start*` callers);
  `crates/net/src/lib.rs:680`; `crates/js_runtime/src/extensions/worker_ext.rs`.
- BO docs: `DETECT_vectors.md` §5-6, `ENGINE_chromium_stealth.md` §1-4, `ENGINE_camoufox_v150.md` §0-3,
  `06_NOCDP_ORACLE_METHOD.md` §1-5, `12_COMPETITIVE_LANDSCAPE.md` §1.3/§2.4,
  `27_VENDOR_COMPETITIVE_MATRIX.md` §6, `43_STRATEGIC_GAP_ASSESSMENT.md`.
- External: [rebrowser-bot-detector](https://github.com/rebrowser/rebrowser-bot-detector) ·
  [bot-detector.rebrowser.net](https://bot-detector.rebrowser.net/) ·
  [Rebrowser — Sensitive CDP Methods](https://rebrowser.net/docs/sensitive-cdp-methods) ·
  [Rebrowser — Fix Runtime.enable detection](https://rebrowser.net/blog/how-to-fix-runtime-enable-cdp-detection-of-puppeteer-playwright-and-other-automation-libraries) ·
  [Castle — Why a classic CDP bot detection signal suddenly stopped working](https://blog.castle.io/why-a-classic-cdp-bot-detection-signal-suddenly-stopped-working-and-nobody-noticed/) ·
  [Patchright](https://github.com/Kaliiiiiiiiii-Vinyzu/patchright) ·
  [daijro/camoufox](https://github.com/daijro/camoufox) · deepwiki `daijro/camoufox` (Juggler Protocol Architecture §6.1, 1-leak-fixes.patch) ·
  [anti-detect-browser-tools-tech-comparison](https://github.com/pim97/anti-detect-browser-tools-tech-comparison).
</content>
</invoke>
