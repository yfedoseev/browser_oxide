# 02 — DataDome Deep: etsy + tripadvisor + yelp (frontier challenge)

**Mission frame:** challenge the "out of scope / true frontier" verdict on the
DataDome cluster. Find the concrete ENGINE-ADDRESSABLE path where one exists,
exploit BO's structural **no-CDP** advantage, and be honest where the residual is
genuinely `vendor_solvers` (daily-key encoder) or an unsolvable human gate
(yelp's interactive captcha).

**Cluster split (the decisive fork):**

| Site | `rt` variant | Challenge | Self-solvable headlessly? | BO verdict |
|---|---|---|---|---|
| **etsy** | `rt:'i'` interstitial | silent device-check (JS + canvas/audio + WASM PoW, **no human**) | **YES** (Camoufox model) | **ENGINE-ADDRESSABLE plumbing + `vendor_solvers` daily-key** |
| **tripadvisor** | `rt:'i'` interstitial | same as etsy | **YES** | same as etsy |
| **yelp** | `rt:'c'` captcha | interactive slider/puzzle requiring **human** mouse path | **NO** (even Camoufox v150 fails) | **human gate — out of reach for ALL headless engines** |

This is the single most important distinction in the cluster: **etsy/tripadvisor
are theoretically winnable by being a faithful enough runtime; yelp is not, by
anyone.** Don't conflate them.

**Prior measured baseline** (`07_DATADOME_PRIMITIVES.md`, `FAILED_SITES_ANALYSIS.md` §C.3):

| Site | BO (all profiles) | Camoufox v150 | Camoufox v135 | Patchright |
|---|---|---|---|---|
| etsy.com | `DataDome-CHL` ~1424 b | `DataDome-CHL` | `7913b` (loose, partial) | `DataDome-CHL` |
| tripadvisor.com | `DataDome-CHL` ~1430 b | `DataDome-CHL` (then L3 on some runs) | — | — |
| yelp.com | `DataDome-CHL` | `DataDome-CHL` (Camoufox also fails) | — | — |

v150 regressed from v135 on etsy → the DataDome target moved (daily-key /
ML tightening), not a fixed BO bug.

---

## 1. The exact detection mechanism blocking BO

### 1.1 The interstitial flow (etsy / tripadvisor, `rt:'i'`)

Corroborated across Hyper Solutions, glizzykingdreko, ZenRows, Scrapfly, Kameleo
(sources §6). A blocked request returns a small (~1.4 KB) HTML 403 carrying:

```javascript
var dd = {
  'rt':'i',                       // 'i'=interstitial(silent)  'c'=captcha(human)
  'cid':'AHrlq…',  'hsh':'14D0…', // seed the payload PRNG
  's': 44330, 'b': …,             // numeric params folded into the device link
  'host':'geo.captcha-delivery.com'
};
```
plus a reference to `https://ct.captcha-delivery.com/i.js` (the interstitial
bootstrap, distinct from the `c.js` captcha bootstrap). `i.js`:

1. Parses `dd`, reads the `datadome=` cookie already on the 403 (FP-D3: present
   on the *failing* response too — see §3.4), and builds a **deviceLink**:
   `https://geo.captcha-delivery.com/interstitial/?initialCid={cid}&hash={hsh}&cid={datadomeCookie}&referer={referer}&s={s}&b={b}&dm=cd`
2. **GET**s the deviceLink → device-check HTML/JS document.
3. Runs the device-check JS: collects ~31 signals — Picasso-canvas + audio
   fingerprint inputs, `hardwareConcurrency`, `navigator.plugins`, WebGL, plus
   behavioral signals (`_initialCoordsList` / `_coordsList` page-load→interaction
   mouse paths), and runs a **WASM proof-of-work** (`boring_challenge`: base64
   Wasm, 32-bit seed, nested XOR/shift/rotate state machine, 64-bit terminal).
4. Encodes everything through the **`ddCaptchaEncodedPayload`** pipeline:
   daily-rotated 6-char signal keys → per-key XOR (PRNG seeded from `hsh`+`cid`+salt)
   → whole-buffer XOR → custom URL-safe Base64 with decrementing salt.
5. **POST**s the encrypted form-data to `https://geo.captcha-delivery.com/interstitial/`.
6. Server validates and returns the clearance cookie. **Crucially** (Hyper
   Solutions): the cookie comes back **in the JSON response body** as
   `{"cookie":"datadome=…; Max-Age=31536000; …"}`, and `i.js` then writes it via
   **`document.cookie`** — it is *not always* a `Set-Cookie` response header.
   (Some deployments use both.) This detail decides which BO code path the
   clearance cookie travels (§3.3).
7. `i.js` reloads the origin; the origin now sees the valid `datadome=` and
   serves the real page.

**What blocks BO here is NOT one thing — it is a chain, and BO breaks at two
links, only one of which is engine-addressable:**

- **(A) Engine-addressable runtime/plumbing links** — does the interstitial even
  get detected, does its child context get fetched + executed + drained, and does
  the clearance cookie it lands actually reach the parent jar that gates the
  reload? (§3.2, §3.3 — the **isolated-cookie-jar bug** is the load-bearing
  public-engine break.)
- **(B) `vendor_solvers` link** — the daily-rotated `ddCaptchaEncodedPayload`
  encoder (step 4). BO must NOT reimplement this; it must let DataDome's own
  (daily-rotated) `i.js` compute it. The public engine's only job is to make that
  self-solve *possible*.

### 1.2 The interactive captcha (yelp, `rt:'c'`)

`c.js` injects a slider/GeeTest-style puzzle that requires a **validated human
mouse path** to clear. There is no silent self-solve — the validation explicitly
requires interaction biometrics no headless engine produces. **Camoufox v150 also
fails yelp.** This is a human gate, not an engine bug.

### 1.3 The upstream scoring gate (why a request gets `rt:'i'` at all)

Before any challenge, DataDome's ML model scores every request from TLS
fingerprint + HTTP/2 fingerprint + browser fingerprint + behavioral history + **IP
reputation + the CDP/automation signal**. A clean score → no challenge; a
borderline score → silent `rt:'i'`; a bad score → `rt:'c'` or hard 403. **The CDP
signal is a first-class input to this gate** (§2) — and it is exactly where BO has
its structural edge.

---

## 2. Does a no-CDP real browser pass it? → engine-addressable

### 2.1 The evidence

1. **DataDome published its own CDP-detection threat research**
   ([DataDome: How New Headless Chrome & the CDP Signal Are Impacting Bot
   Detection](https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/)).
   Chromium dispatches `Runtime` domain events *only after* a client sends
   `Runtime.enable` — which every CDP automation framework does at init. DataDome
   detects this generically: it targets *the automation transport itself*, not
   per-framework leaks, so it catches even "undetectable" stealth configs. The
   ZenRows 2026 guide states it plainly: *"DataDome can still detect through CDP
   detection. When automation tools send commands to the browser, anti-bots can
   detect the `Runtime.enable` command that Playwright uses."*
2. **The same sources confirm the runtime itself is enough when the transport is
   clean:** *"headless browsers execute JavaScript challenges and DataDome's
   fingerprinting code natively, which can bypass the anti-bot system"*
   (ZenRows). The challenge is self-solving JS — a faithful runtime runs it.
3. **Camoufox has zero DataDome code** (deepwiki on `daijro/camoufox`,
   re-verified this session) and historically passed the silent device-check
   purely by being a real Firefox: real WASM runs the PoW; `cross-process-storage.patch`
   syncs the per-context fingerprint into worker processes so the worker-side
   device check is consistent; `disable_coop` lets cross-origin captcha elements
   be interacted with. **The DataDome JS itself computes the daily-key payload —
   Camoufox never reimplements it.** This is the model BO must match.
4. **Repo prior evidence (the no-CDP oracle principle):** the standing correction
   in memory (`state_2026_05_15_playwright_ab_decisive`, `proxy_not_the_problem`)
   is that **Playwright/MCP is an INVALID real-browser oracle for these vendors —
   they are CDP and get detected**, giving a false "even real Chrome fails"
   reading. The valid oracle is real Chrome/Firefox launched WITHOUT CDP +
   passive capture. For the Kasada cluster, no-CDP real Chrome from THIS IP
   passes zero-interaction → the block is engine-fidelity, not IP. **DataDome
   weights the CDP signal even more explicitly than Kasada**, so the same logic
   applies with higher confidence: a no-CDP real browser is the right oracle, and
   it should clear the etsy/tripadvisor silent interstitial.

### 2.2 BO's structural advantage — quantified against the field

The central thesis holds strongest for DataDome of all the frontier vendors:

| Engine | Automation transport | DataDome CDP-signal exposure |
|---|---|---|
| Playwright / Patchright (Chromium) | **CDP** (`Runtime.enable`) | **HIGH** — directly fingerprinted; published detection |
| Camoufox (Firefox) | **Juggler** (custom, non-CDP, but *still a protocol*) | **LOW–MEDIUM** — no `Runtime.enable`, but Juggler is patched-but-present; CDP-specific checks miss it, yet it is an automation surface |
| **browser_oxide** | **NONE** — in-process V8, no DevTools endpoint, no juggler, no webdriver, no `cdc_` vars | **ZERO** — there is literally no automation transport to fingerprint |

This is not a marginal edge. DataDome's *generic* automation detection keys on
the existence of the control transport. Patchright is detectable by construction;
Camoufox sidesteps the *CDP-specific* checks but still ships Juggler (deepwiki
confirms Playwright drives Firefox via Juggler) and its known input-event prefs
(`dom.input_events.security.minNumTicks=0`) are themselves a tell. **BO has no
transport at all** — it is the only engine in the comparison that cannot fail the
CDP / automation-transport class of check. That is precisely the part of
DataDome's scoring gate (§1.3) that gates a *clean* `rt:'i'` (self-solvable) vs a
hostile `rt:'c'`/hard-403.

**Verdict on §2 for etsy/tripadvisor: ENGINE-ADDRESSABLE.** A no-CDP faithful
runtime is the documented winning path; BO's no-CDP cleanliness is a real,
load-bearing lever for getting the *self-solvable* `rt:'i'` variant rather than
the human-gated `rt:'c'`. The residual is runtime fidelity + the daily-key
encoder, not the transport.

**Verdict on §2 for yelp: NOT engine-addressable by any headless engine.** No-CDP
cleanliness raises the trust score but `rt:'c'` still demands a human mouse path.

---

## 3. The concrete engine path (file:line) + how no-CDP helps

The 3 vendor-agnostic primitives are **shipped** (commit `78a1241`, FIX-DD) and
verified against source. The gap is now sharply localized.

### 3.1 The 3 shipped primitives — verified present

- **Detection** — `crates/browser/src/page.rs:208`
  `fn is_datadome_challenge(html)` → `html.len() < 50_000 && html.contains("captcha-delivery.com")`.
  Wired at `page.rs:1794` (CSP relax) and `page.rs:1845` (`started_as_dd_challenge`).
- **Iframe materialization (FP-E1)** — `Page::rematerialize_iframes`
  (`page.rs:778`), called every 200 ms tick inside the challenge poll
  (`page.rs:2252`). DOM-walks the post-JS DOM, and for any iframe not already a
  real child runs the same cross-origin fetch + child-context build as build time
  via `iframe::ChildIframe::from_url` (`iframe.rs:73`). CSP-`frame-src`-gated
  (`iframe.rs:84`), idempotent.
- **Solved-cookie break + retry** — poll break at `page.rs:2287-2291`
  (`is_datadome_solved` OR a registered solver's `solved_signal`); cookie-diff
  retry at `page.rs:2474` (`cookies_after != cookies_before`).

### 3.2 The child iframe DOES get a real WASM drain (good)

The materialized child is NOT starved of async time:
- `from_url` → `run_until_idle(Duration::from_secs(10))` (`iframe.rs:232`).
- `from_srcdoc` → `run_until_idle(Duration::from_secs(5))` (`iframe.rs:64`).
- WebAssembly is the native V8 object (`js_runtime/src/js/window_bootstrap.js:17`);
  BO only polyfills `instantiate/compileStreaming`. So `boring_challenge` PoW
  **can compile and run** in BO. WASM execution is **not** the gap.

So when `geo.captcha-delivery.com` is materialized as an iframe, it gets up to 10 s
to fetch its bundle, run the WASM PoW, and POST the payload.

### 3.3 THE load-bearing public-engine bug — the child iframe runs on an ISOLATED cookie jar

This is the highest-leverage engine-addressable finding (first surfaced in
`SITE_etsy.md` §3c; here verified end-to-end against the cookie-sharing model).

**The parent navigation client is shared-jar; the child iframe's JS-fetch client is isolated-jar.**

- **Parent:** `navigate_with_init_solvers` builds its client via
  `net::HttpClient::shared(&profile)` (`page.rs:1158`). `shared()`
  (`net/src/lib.rs:353`) wires the client's `cookies` to the **process-wide
  `SHARED_SESSION.cookies`** `Arc<Mutex<CookieJar>>` (`net/src/lib.rs:363-368`).
  The cookie-diff retry and `is_datadome_solved` both read this shared jar via
  `client.cookies_for_url` (`page.rs:2269,2441`).
- **Child iframe HTTP resource fetches** (`client.get` inside `from_url`,
  `iframe.rs:101,152,201`) use the **passed-in parent client** → those land in
  the **shared jar**. Good — but these are GETs of the device-check doc + scripts,
  not the clearance-bearing POST.
- **Child iframe JS `fetch()` / XHR** (the actual encrypted POST that returns the
  `datadome=` clearance) route through the child V8's `op_fetch`
  (`fetch_ext.rs:233`), which uses the **thread-local `FETCH_CLIENT`**
  (`fetch_ext.rs:284`). That thread-local was set when the child runtime was built:
  `BrowserJsRuntime::with_options` → `init_fetch_client(profile)` +
  `FetchState::with_profile(profile)` (`runtime.rs:84-90`), and
  `with_profile` mints `net::HttpClient::new(profile)` (`fetch_ext.rs:42-44`) —
  which is `HttpClient::new` (`net/src/lib.rs:308`) = a **fresh, isolated
  `Arc<Mutex<CookieJar>>`, NOT the shared session.**

**Consequence:** when `i.js` POSTs to `geo.captcha-delivery.com/interstitial/`
and the response carries the clearance via a **`Set-Cookie` header**,
`op_fetch` → `client.fetch_post_bytes` → `store_set_cookies` (`net/src/lib.rs:874,892`)
writes `datadome=` into the **child's isolated jar**. The parent's
`cookies_for_url` (shared jar) never sees it → `cookies_after == cookies_before`
→ `should_retry = false` (`page.rs:2474`) → no reload → BO returns the 1.4 KB
interstitial → **`DataDome-CHL`**. *Even a hypothetically perfect solver running
in the child would still produce CHL*, because the cookie is structurally trapped.

**The one partial escape:** if `i.js` instead reads the JSON-body `cookie` field
(Hyper Solutions §1.1 step 6) and writes it via **`document.cookie`**, that goes
through `op_cookie_set_sync` (`fetch_ext.rs:418`), which writes to BOTH the
thread-local AND `net::set_shared_cookie_sync` (`fetch_ext.rs:432` →
`net/src/lib.rs:191`) → the **shared jar** → the parent retry sees it. So the
flow may *partially* work via the JSON+document.cookie path but **silently fail
via the Set-Cookie-header path.** This dual-path inconsistency is exactly the kind
of intermittent "works sometimes" behavior that matches v135's `7913b` partial
progress. **This must be fixed deterministically.**

> Secondary hazard (`SITE_etsy.md` §3c note): `init_fetch_client` *clobbers* the
> per-thread `FETCH_CLIENT` during the synchronous child build. If it is not
> restored to the parent's client after `from_url` returns, the parent's own
> subsequent same-thread `op_fetch`/`document.cookie` during the challenge poll
> route through the child's isolated client. Audit + restore. (The shared-jar
> design means `document.cookie` still reaches the shared session via
> `set_shared_cookie_sync`, but header-set cookies on the parent's poll fetches
> would be misfiled.)

### 3.4 Guards that prevent false "solved" (correct, keep)

- **FP-D3** — `is_datadome_solved` (`page.rs:221`) requires `datadome=` present
  **AND** `!is_datadome_challenge(body)`. `datadome=` lands on the *failing* 403
  too, so the body-shape transition is the real solve signal. Correct.
- **Marker guard** (`page.rs:2510-2529`, comment `page.rs:2504` names "DataDome
  (yelp/etsy/leboncoin/wsj)") rejects a 200-but-still-interstitial as a pass.

### 3.5 Why no-CDP helps the *engine path* specifically

The engine plumbing (§3.3) only matters if the upstream scoring gate (§1.3)
served the *self-solvable* `rt:'i'` rather than `rt:'c'`/hard-403. BO's zero
automation transport (§2.2) is what biases the gate toward `rt:'i'`. So the
no-CDP advantage and the cookie-jar fix are **complementary, not redundant**:
no-CDP gets BO the silent challenge; the jar fix lets BO actually *bank* the
clearance the silent challenge produces. Without the jar fix, even a perfectly
clean no-CDP request that earns `rt:'i'` still ends in CHL.

### 3.6 Ranked engine path (ROI order)

| # | Fix | Scope | Effort | Confidence (bug real / flips etsy) |
|---|---|---|---|---|
| **1** | **Live-nav trace experiment** — run live etsy with `[datadome-trace]` (`page.rs:3149`) + log the materialized-iframe POST and any `datadome=` delta in BOTH child and shared jars. Disambiguates "iframe never ran" (drain) vs "ran, cookie trapped in child jar" (§3.3) vs "ran, got fresh challenge" (fingerprint). The single highest-value unknown. | public (diagnostic) | 1 day | high / n.a. |
| **2** | **Child-iframe cookie-jar sharing** — make `ChildIframe::from_url`/`from_srcdoc` use the **shared session** for the child's V8 `FETCH_CLIENT` (thread `HttpClient::shared` or the parent's `Arc<Mutex<CookieJar>>` into `with_options` → `FetchState`), OR after each child build merge child-jar `Set-Cookie` deltas for the registrable domain back into the shared jar; AND audit/restore the `FETCH_CLIENT` thread-local after the child build (§3.3 note). Real browsers share one cookie store per eTLD+1. Unit test: a child-set `datadome=` is visible via `parent.cookies_for_url`. | **public** | 2-3 days | **high / low-alone** (necessary precondition; also unblocks CF Turnstile / any iframe-clearance vendor) |
| **3** | **Worker fingerprint inheritance audit** — DataDome can run device-check + PoW in a Web Worker; BO workers must expose the SAME canvas/WebGL/navigator/`hardwareConcurrency` as the main realm (BO's analogue of Camoufox `cross-process-storage.patch`). Worker secure-context already fixed (`5216336`, restored `crypto.subtle`). | **public** | 3-5 days | medium / medium |
| **4** | **`_initialCoordsList` movement realism** — `humanize.js` (`page.rs:3304`) must seed a plausible page-load→first-interaction mouse path so the ~31 movement signals aren't empty/anomalous. | **public** | 2-3 days | medium / medium |
| **5** | **Harden detection + multi-cookie solve** — broaden `is_datadome_challenge` (`page.rs:208`) beyond bare `captcha-delivery.com` to `ct.captcha-delivery.com`, the `i.js` ref, and `var dd=`/`"rt":"i"`; handle 5-10 KB bodies (`12_R-DATADOME-WASM` §75); extend `is_datadome_solved` to `datadome`+`_pxhd`/`_px3`. Insurance against the next body-shape rotation (the failure mode that broke v135). | **public** | 1 day | high / 0-direct |
| **6** | **DataDome interstitial daily-key signals encoder** — the `ddCaptchaEncodedPayload` 4-stage encoder + daily 6-char key derivation + loop-switch VM decode + canvas/audio/behavioral payload. Only needed if BO's runtime is *not faithful enough* to let `i.js` self-encode (and the only thing that can ever touch yelp's `rt:'c'`, which is still a human gate even then). | **`vendor_solvers`** | 1-2 wks + daily-rotation maintenance | low / low |

**Expected site impact:** fixes #1-#5 are pure runtime fidelity (drain budgets,
cookie-jar sharing, worker fingerprint, movement realism, detection robustness) —
all public, none names DataDome in flow code. Together they target **etsy +
tripadvisor** (`rt:'i'`): plausibly **+1 to +2 strict passes**, putting BO at or
above v150 on the cluster (v150 fails etsy too). **#2 is the gate everything else
depends on** — without it no clearance is ever banked. **yelp stays CHL** (human
`rt:'c'`); #6 is `vendor_solvers` and even it cannot pass yelp.

---

## 4. The no-CDP-oracle capture + diff validation plan

**Do not run live navs on the contended IP during a competitor benchmark.** This
plan is offline-capture-first; the one live step is gated on a free IP window.

### 4.1 Capture a valid no-CDP oracle (the ground truth)

1. On a **non-CDP** real browser (normal Chrome or Firefox launched by a human,
   NO `--remote-debugging-port`, NO Playwright/Selenium/Puppeteer — Playwright is
   an INVALID oracle here per memory) behind a passive proxy (mitmproxy / HAR),
   load etsy + tripadvisor + yelp from the SAME datacenter IP BO uses.
2. Record per site: the initial 403 body (`var dd={…}` — **confirm the live `rt`
   value**: is etsy still `'i'`? has it moved to `'c'`?), the `i.js`/`c.js`
   fetch, the deviceLink GET, the encrypted POST body to
   `geo.captcha-delivery.com/interstitial/`, and the response (Set-Cookie header
   vs JSON `cookie` field) that lands `datadome=`.
3. **Decision gate:** if a no-CDP real browser silently clears etsy/tripadvisor
   from this IP zero-interaction → the block is engine-fidelity (proceed). If it
   *also* fails → the gate has tightened beyond runtime fidelity (IP/ML), and the
   etsy flip is genuinely out of reach this quarter regardless of engine work.

### 4.2 Offline child-realm oracle (cheap, runs anytime)

Reuse the `aws_capture`/`awswaf_probe` harness pattern
(`HANDOFF_2026_05_28b §6`), adapted to DataDome:
1. Feed the captured etsy device-check document into a `ChildIframe::from_url`
   built in isolation; assert `i.js` runs to completion (canvas ops, audio ops,
   `crypto.subtle`, WASM `instantiate`) and does NOT bail on a missing API / CSP
   block on a sub-resource. If it bails early → a cheap missing-primitive fix,
   not the daily-key solver.
2. **Cookie-jar assertion (validates Fix #2):** after the child build, check both
   the child's isolated jar AND the shared session jar for any `datadome=` the
   child's `fetch()` set. Pre-fix: present only in child jar (the bug). Post-fix:
   present in the shared jar where the parent retry reads it.

### 4.3 Live-nav trace (Fix #1, gated on a free IP window)

Run live etsy with `[datadome-trace]` hooks (`page.rs:3149`) + temporary logging
of: (a) materialized-iframe count, (b) the iframe's POST to
`geo.captcha-delivery.com`, (c) `client.cookies_for_url` on BOTH jars before/after.
Three outcomes, three different fixes:
- iframe never materialized / POST never sent → **drain or detection** (Fix #1/#5).
- POST sent, `datadome=` in child jar only → **cookie-jar trap** (Fix #2). *Most
  likely per §3.3.*
- POST sent, server returns a *fresh* `rt:'i'` instead of clearance → **fingerprint
  / movement parity** (Fix #3/#4) or the daily-key encoder is needed (#6 — but
  then so would Camoufox).

### 4.4 Regression gate

`target/release/examples/sweep_metrics chrome_148_macos <(echo '[{"cat":"misc","name":"etsy","url":"https://www.etsy.com/"}]') /tmp/etsy.json`
— expect `L3-RENDERED > 15 KB` on a flip. Parallel-sweep etsy + tripadvisor + a
**known-clean DataDome site** + yelp to confirm no regression and that yelp
correctly stays CHL (don't false-flip the human gate). Run on **release**, size-
gate ≥30 KB (memory: holistic FP trap).

---

## 5. Honest verdict per site

| Site | Verdict | Confidence | Rationale |
|---|---|---|---|
| **etsy** | **ENGINE-ADDRESSABLE (public) for the plumbing; `vendor_solvers` for the daily-key encoder IFF runtime fidelity proves insufficient** | medium | `rt:'i'` is silent/self-solvable (Camoufox model). No-CDP real browser is the documented winning path and BO's zero-transport is the strongest lever in the corpus. **The load-bearing public bug = child-iframe isolated cookie jar (§3.3)** — fixable. Residual daily-key encoder is `vendor_solvers` only if BO can't make `i.js` self-encode; v150 also fails, so this is a *lead over v150*, not parity. |
| **tripadvisor** | **same as etsy** | medium | Same `rt:'i'` interstitial; same primitives, same jar bug, same lever. |
| **yelp** | **HUMAN GATE — not engine-addressable by ANY headless engine; not IP-bound, not stealth-fixable** | high | `rt:'c'` interactive captcha requires a validated human mouse path. Camoufox v150 also fails. `vendor_solvers` cannot pass it either (no silent solve exists). Out of scope honestly, not as a cop-out — as a documented property of the challenge type. |

**Net:** the cluster is NOT a monolith. etsy + tripadvisor are a **real
engine-addressable opportunity** where BO's no-CDP cleanliness is a structural
advantage over every CDP competitor and even over Juggler-based Camoufox — gated
on one concrete public bug (the cookie-jar trap, Fix #2) plus runtime-fidelity
fixes (#3/#4), with the daily-key encoder correctly parked in `vendor_solvers`.
yelp is an honest human gate that no headless engine passes.

**Sequencing note (`SITE_etsy.md` §5):** for the *v150-parity* goal etsy is NOT
on the critical path (v150 also fails it → contributes 0 to closing the BO−v150
gap). Land Fix #2 (cheap, broad — also unblocks Cloudflare Turnstile and any
iframe-clearance vendor) + Fix #5 (insurance) now; treat the full etsy flip as a
**frontier lift** (outperform v150) after the AWS-WAF cluster + booking + imdb.

---

## 6. Sources

**Repo docs (cited, not duplicated):**
- `docs/v0.1.0-parity-workflows/external/VENDOR_datadome.md` — prior deep analysis
- `docs/v0.1.0-parity-workflows/sites/SITE_etsy.md` — the isolated-jar bug (§3c), ranked fixes
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` — the 3 primitives spec
- `docs/vNext/12_R-DATADOME-WASM.md` — public/`vendor_solvers` boundary, 10-step flow
- `docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md` §C.3 — v135→v150 regression, Stratum C
- memory: `state_2026_05_16_phase5_datadome`, `state_2026_05_15_playwright_ab_decisive`, `proxy_not_the_problem`, `measurement_holistic_chl_fp_trap`

**BO source (verified this session):**
- `crates/browser/src/page.rs:208,221,778,1158,1794,1845,2252,2269,2287,2441,2474,2504,3149,3304`
- `crates/browser/src/iframe.rs:64,73,84,101,152,201,232`
- `crates/js_runtime/src/runtime.rs:84-90`
- `crates/js_runtime/src/extensions/fetch_ext.rs:42-44,233,284,418,432`
- `crates/net/src/lib.rs:146,191,308,353,363-368,874,892`
- `crates/js_runtime/src/js/window_bootstrap.js:17`

**External (2025-2026):**
- [DataDome — How New Headless Chrome & the CDP Signal Are Impacting Bot Detection](https://datadome.co/threat-research/how-new-headless-chrome-the-cdp-signal-are-impacting-bot-detection/) — `Runtime.enable` / CDP signal detection
- [DataDome — Playwright headless browser detection](https://datadome.co/headless-browsers/playwright/)
- [Hyper Solutions — DataDome Interstitial](https://docs.hypersolutions.co/datadome/interstitial) — deviceLink construction, JSON-body `cookie` return
- [glizzykingdreko — Breaking Down DataDome Captcha WAF](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21) — daily 6-char key rotation, `ddCaptchaEncodedPayload`
- [ZenRows — Bypass DataDome 2026](https://www.zenrows.com/blog/datadome-bypass) — "headless runs the JS natively"; CDP `Runtime.enable` detection
- [Scrapfly — Bypass DataDome 2026](https://scrapfly.io/blog/posts/how-to-bypass-datadome-anti-scraping)
- [Kameleo — Guide to Bypassing DataDome 2025](https://kameleo.io/blog/guide-to-bypassing-datadome)
- [CapSolver](https://docs.capsolver.com/en/guide/captcha/datadome/), [CapMonster](https://docs.capmonster.cloud/docs/captchas/datadome/), [TakionAPI](https://docs.takionapi.tech/datadome) — `vendor_solvers`-class daily-key encoders (the forbidden path)
- deepwiki `daijro/camoufox` — no DataDome code; `cross-process-storage.patch` worker fingerprint sync; **uses Juggler (non-CDP but a protocol), Playwright drives Firefox via Juggler**
