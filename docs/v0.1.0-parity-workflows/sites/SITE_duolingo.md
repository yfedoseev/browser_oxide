# SITE: duolingo — reCAPTCHA-Enterprise-invisible Worker/iframe hydration gate

**URL:** `https://www.duolingo.com/`
**Status (2026-05-28):** BO `L3-RENDERED` **13 327 bytes**, 0/5 pass. Camoufox v150 = driver-crash NODATA on this box (inconclusive head-to-head), but v135/Patchright historically PASS ~697 KB. Bar = body > 50 000 bytes (loose-gate 15 KB; BO is ~1.7 KB shy of the *loose* gate, ~37 KB shy of the real-content gate).
**Cluster:** SPA-hydration gated on a reCAPTCHA Enterprise **invisible/score (v3-style)** token.
**Scope verdict:** the engine plumbing is **public-engine-addressable**; the actual "compute a valid Google reCAPTCHA token" is NOT something we forge — we make BO's engine *capable of running Google's own challenge JS to completion* the way a real browser does. No per-vendor bypass code.

---

## 1. What the repo already concluded (cite)

Prior research, in reading order:

- **`02_GAP_ANALYSIS.md` §2 (lines 81–110)** — duolingo loads three scripts in order: `recaptcha/enterprise.js?render=6LcLOdsj…`, `gstatic…/recaptcha__en.js`, `recaptcha/enterprise/webworker.js`. Hypothesis: the SPA hydrates only after `grecaptcha.execute()` resolves with a token; the worker "almost certainly fails." Tagged EASY, "1.7 KB to gate."
- **`05_SPA_HYDRATION_CLUSTER.md` §2 (lines 315–460)** — full hypothesis tree:
  - **H1 (~50%)** MessageChannel/MessagePort was a no-op stub → recaptcha's in-process port handoff dies. **This was fixed** (Fix 8, see §3.2 below) and verified by 3 unit tests, but the handoff notes "the duolingo-recaptcha flow specifically doesn't use the new MessageChannel path the way we hoped" (`HANDOFF_v0.2.0_CLOSE_V150_GAP.md:181`).
  - **H2 (~20%)** `new Worker(webworker.js)` fails to load/run.
  - **H3 (~15%)** missing Worker-context global (`OffscreenCanvas`, `crypto.subtle`, `self.location`, `importScripts` return).
  - **H4 (~15%)** non-Worker headless sentinel leak.
- **`FAILED_SITES_ANALYSIS.md` §A.3** — Stratum-A; v150 passes at 697 KB; "Fix 8 (MessageChannel) targeted this but didn't crack it"; entry point `worker_ext.rs`; filed **R-DUO-WORKER**.
- **`HANDOFF_v0.2.0_CLOSE_V150_GAP.md` §1.3 (R-DUO-WORKER)** — build an in-VM oracle for `webworker.js`, diff `postMessage`/`Worker` surfaces vs v150. 1-week investigation.
- **`HANDOFF_2026_05_28b.md` §4** — this session's worker `crypto.subtle` secure-context fix (commit `5216336`) was shipped as a *prerequisite* for AWS-WAF + reCAPTCHA PoW workers, but **duolingo did not flip** (`/tmp/awswaf/duo_fixed.json` still 13 327 B). The handoff hypothesised duolingo "shares the AWS live-nav-drain class (async progress never advancing)." **This doc falsifies that hypothesis for duolingo** — see §3.4.
- **`WORKERS.md`** — design doc for the workers surface (own V8 isolate per worker; structured clone; transferables: ArrayBuffer / **MessagePort** / OffscreenCanvas; `navigator.serviceWorker` presence). Note: the doc *promises* MessagePort transfer; the implementation does **not** deliver it (§3.3).

**Net of prior work:** four fixes shipped that are individually correct but none flips the site (MessageChannel paired routing, `self.location` in the worker realm, worker secure-context/`crypto.subtle`, the W5b async worker pump). The repo had **not** yet established *what actually executes vs what doesn't* on a live nav. This doc does that with a fresh live trace.

---

## 2. New external findings (cited)

- **reCAPTCHA Enterprise invisible == score/v3-style flow.** The anchor URL BO fetched live is `…/enterprise/anchor?…&size=invisible&anchor-ms=20000&execute-ms=30000`. Per Google's docs, `grecaptcha.enterprise.execute(sitekey,{action})` returns a Promise that resolves to a token for **score-based** keys, with **no user interaction** required. (Google for Developers — reCAPTCHA v3 / Enterprise instrument-web-pages.) So duolingo is the *no-human-interaction* variant — exactly the case a faithful engine solves automatically; there is no visual puzzle to "solve."
  Sources: <https://developers.google.com/recaptcha/docs/v3>, <https://cloud.google.com/recaptcha/docs/instrument-web-pages>, <https://docs.cloud.google.com/recaptcha/docs/api-ref-checkbox-keys>
- **Camoufox passes this class "perfectly" because it is a real Firefox.** Camoufox docs: "For reCAPTCHA v3 and token-based CAPTCHAs, headless mode works perfectly… Camoufox has no built-in CAPTCHA solving" — i.e. it doesn't bypass anything; Firefox's real engine runs Google's JS, the worker computes the score signals, the token POSTs, the SPA hydrates.
  Source: <https://camoufox.com/stealth/>, <https://github.com/daijro/camoufox>
- **deepwiki `daijro/camoufox`:** Camoufox implements Web Workers and **cross-origin iframes natively via Firefox + Juggler + Fission site-isolation**, applying per-context fingerprint isolation (canvas/audio/timezone/navigator) *inside worker and iframe contexts*. `FrameTree._onWorkerCreated` manages real worker lifecycle; `RoverfoxStorageManager` propagates spoofed values cross-process so the **worker thread's `navigator` matches the window's** — the exact mismatch anti-bot worker-cross-checks hunt for. **This is the structural capability BO lacks** (BO has no cross-origin iframe realm, and its worker fetch path leaks a different fingerprint — §3.5).
- **Why this is not "solvable by stubbing":** anti-bot worker probes check `Object.getOwnPropertyDescriptor`, `fn.toString()` `[native code]`, and **window-vs-worker fingerprint divergence** (per Camoufox's own threat model). reCAPTCHA's `webworker.js` collects behavioural/environment signals; a stub that returns canned values would *lower* the score, not raise it. The only durable path is to **actually run Google's worker JS in a faithful worker realm**.

---

## 3. BO code-level analysis (file:line)

### 3.1 Live trace (fresh, 2026-05-28, `target/release/examples/sweep_metrics chrome_148_macos`)

All three recaptcha assets ARE fetched on a live nav:
- `recaptcha/enterprise.js?render=6LcLOdsj…` → 2045 B ✓
- `gstatic…/recaptcha__en.js` → 890 544 B ✓ (saved to `/tmp/fetched_script_890544.js`)
- `recaptcha/enterprise/anchor?…size=invisible&anchor-ms=20000&execute-ms=30000` ✓
- `recaptcha/enterprise/webworker.js?hl=en&v=…` ✓

**Decisive new evidence:**
1. `RUST_LOG=js_runtime::extensions::worker_ext=trace` over a full live nav logs **ZERO `worker_id=` / zero `op_worker_spawn`**. The recaptcha worker is **never spawned in BO's V8.** (Verified twice; `grep -c worker_id` = 0.)
2. `webworker.js` was fetched with **`sec-fetch-dest: document`, `sec-fetch-mode: navigate`** — i.e. it came from BO's *navigate-loop sub-resource prefetcher*, **not** from `new Worker()` (which would be `sec-fetch-dest: worker`). So the byte appears in the trace but the Worker was not constructed.
3. One `recaptcha__en.js` re-fetch went out with **`sec-ch-ua-platform: "Linux"` + `X11; Linux x86_64` UA** while the page profile is macOS — a fingerprint **leak** from the worker/helper-thread fetch fallback (§3.5).

Result: `tag=L3-RENDERED len=13327 ms=30267`. The page burned the full nav budget and still produced only the shell.

### 3.2 Worker creation site in recaptcha (deobfuscated)

`/tmp/fetched_script_890544.js` contains exactly one `new Worker`:
```
…&&(X=new Worker(C[30](66,R),void 0))&&…throw Error()
```
gated behind a runtime bitmask `(y^36)<w[0] && (y^5)>>4>=2`. The worker URL is built at runtime by `C[30](66,R)` (no literal `webworker.js` string in the source — fully constructed). This is the api2 client's PoW/telemetry worker. The challenge proper runs in the **anchor/bframe cross-origin iframe** that BO never instantiates as a realm (§3.4).

### 3.3 MessageChannel/MessagePort — Fix 8 is real but in-page only

`crates/js_runtime/src/js/window_bootstrap.js:2314–2427` (Fix 8) implements proper paired-port routing: `_PortPaired` WeakMap, `_deliver`, `_enable` (start-gate + queue drain), `close()` detach. This is correct **within a single isolate** and its 3 unit tests pass.

**Gap:** `MessagePort` is not a **transferable across the Worker boundary.** Two places drop it:
- `window_bootstrap.js:1975–2017` (`Worker.postMessage`) — the transfer list validator (1983–1994) only accepts `ArrayBuffer`/views and **throws `TypeError` for a `MessagePort`**. So `worker.postMessage(msg, [port])` (the standard reCAPTCHA handoff idiom) errors.
- `window_bootstrap.js:1945–1953` (the `_drainOnce` pump) hard-codes `ports: []` on every inbound `MessageEvent` — a worker can never hand a port back to the page.
- `worker_bootstrap.js:189–199` (`self.postMessage`) — same validator, MessagePort rejected worker→page.

So Fix 8 fixed the *page↔page* channel but the *page↔worker* port handoff (which is how recaptcha's worker and the api2 client actually rendezvous) is still broken. This matches the handoff note that "the duolingo-recaptcha flow doesn't use the new MessageChannel path the way we hoped."

### 3.4 The real root cause — no cross-origin iframe realm

BO only instantiates **`<iframe srcdoc=…>`** iframes as child realms: `crates/browser/src/page.rs:613–668` (`iframe::find_iframes` → `ChildIframe::from_srcdoc`). It has **no path that instantiates a script-created cross-origin iframe** (`document.createElement('iframe'); el.src = '…/anchor?…'`) as an executing realm. recaptcha__en.js creates the invisible **anchor** iframe dynamically and runs `grecaptcha.execute()`'s machinery (and, for the bframe, the worker) **inside that iframe's realm**. BO fetches the anchor HTML (we see it) but never builds a realm to execute it → the worker is never constructed → `grecaptcha.execute()`'s promise never resolves → no `X-Recaptcha-Token` → no `/api/…` hydration → 13 KB.

This is the same class as the long-standing **FP-E1** finding (memory: "script-created cross-origin iframes never fetched/executed; gates DataDome + Cloudflare, IP-independent"). duolingo is the cleanest single-site instance of FP-E1.

### 3.5 Worker thread fetch leaks a Linux fingerprint

`crates/js_runtime/src/extensions/worker_ext.rs:129–167` (`op_worker_sync_fetch`) and `:282–296` (the worker's own runtime build) call `fetch_ext::fetch_client()`. But `FETCH_CLIENT` is a **`thread_local!` `RefCell`** (`fetch_ext.rs:59`), set only on the main page thread (`page.rs:409` et al). A worker runs on a **freshly `std::thread::spawn`ed** thread (worker_ext.rs:143, 268) where that thread-local is `None`, so it falls back to `stealth::chrome_148_linux()` (worker_ext.rs:136, and the fetch_ext fallback at `:289`). **That is the `sec-ch-ua-platform: "Linux"` leak observed in §3.1.** Even once the worker spawns, its `importScripts`/sync fetches and its `navigator` would be a *Linux* identity while the page is macOS — precisely the window-vs-worker divergence Camoufox spends effort to prevent and that recaptcha's worker telemetry can read.

### 3.6 NOT the AWS live-nav-drain class (falsifies HANDOFF_2026_05_28b §4 guess)

The AWS drain bug is in `build_page_with_scripts_init_and_storage` (the 50 ms inter-script `run_until_idle` at `page.rs:3566`). But duolingo runs through the **outer navigate loop**, whose post-build drain is `remaining.max(Duration::from_secs(8))` (`page.rs:2051–2057`) under a V8DeadlineWatcher, and the host budget for a generic host is 15 000 ms (`page.rs:1949`). The live trace shows `ms=30267` — the page *did* stay alive ~30 s and still produced zero worker activity. The blocker is **structural (no iframe realm + worker never constructed)**, not drain-starvation. Lengthening the drain will not flip duolingo. (The drain fix may still help AWS; it is orthogonal to duolingo.)

### 3.7 Prerequisites already in place (so they're not the lever)
- `self.location` in worker realm: `worker_bootstrap.js:34–61` + `op_worker_self_url` (`worker_ext.rs:537–547`). ✓
- worker `crypto.subtle` via secure-context inheritance: `worker_ext.rs:240, 296` + `is_secure_context: is_secure_url(url)` (`page.rs:391`). duolingo is https → secure → `crypto.subtle` present. ✓
- async worker pump (no event-loop pinning): `op_worker_await_message` (`worker_ext.rs:457–499`) + `_drainOnce` (`window_bootstrap.js:1933–1959`). ✓

These are necessary-but-insufficient. The site needs §3.4 (iframe realm) and §3.3 (port transfer) and §3.5 (worker fetch identity).

---

## 4. Ranked fix list (ROI order)

> Honest expectation: duolingo is a **multi-fix** site, not a one-liner. v150 wins only because it's a real Firefox. The fixes below are the engine capabilities that, stacked, let Google's own challenge JS run to a token. Confidence that *any single one* flips the site is LOW; confidence that the stack (F1+F2+F3) flips it is MEDIUM. All are public-engine work (we run Google's JS, we don't forge tokens).

### F1 — Instantiate script-created cross-origin iframes as executing realms (FP-E1)
- **What:** When a script does `createElement('iframe')` + `.src = cross-origin-url` (or appends such an iframe), fetch the document and build a `ChildIframe`/child `Page` realm that executes its scripts, wired to the parent via the existing `postMessage`/`MessageEvent` surface (with correct `origin`). Extend `iframe::find_iframes` + `page.rs:613–668` from `srcdoc`-only to `src`-based, and hook the DOM `.src` setter / `appendChild` for iframes the way `page.rs` already arena-intercepts other mutations.
- **Effort:** 2–3 weeks (this is the big one; it is the FP-E1 backlog item and also gates DataDome/Cloudflare iframe challenges → leverage well beyond duolingo).
- **Expected impact:** duolingo (enables the anchor/bframe realm where the worker is constructed). Also unblocks the broader FP-E1 cluster (etsy/tripadvisor DataDome iframe, some Cloudflare). Highest strategic ROI.
- **Confidence:** MEDIUM that it's *necessary*; HIGH that it's the structural root.
- **Public engine:** yes.

### F2 — Make `MessagePort` a real transferable across the Worker boundary
- **What:** Accept `MessagePort` in the transfer lists at `window_bootstrap.js:1983–1994` (Worker.postMessage), `worker_bootstrap.js:191–199` (self.postMessage), and the page `postMessage` at `window_bootstrap.js:4218`. On transfer, allocate a routed channel id and proxy `postMessage`/`onmessage` across the mpsc bridge (`op_worker_post_to_worker` / `op_worker_self_post`), so a port handed to a worker delivers to its pair in the other isolate. Populate the inbound `ports: []` array (`window_bootstrap.js:1951`, `worker_bootstrap.js` recv) with reconstructed `MessagePort`s. Update `WORKERS.md` (it already claims this works).
- **Effort:** 1 week.
- **Expected impact:** duolingo (recaptcha's api2-client↔worker rendezvous uses port transfer); also any SPA using `MessageChannel` IPC into a worker. Only matters once F1 lands (the channel lives inside the iframe realm).
- **Confidence:** MEDIUM.
- **Public engine:** yes.

### F3 — Fix worker-thread fetch/navigator identity (kill the Linux leak)
- **What:** Pass the page profile into the worker thread (it already does for the runtime: `op_worker_spawn` captures `profile`, `worker_ext.rs:232–235, 296`) and **also seed the helper-thread `FETCH_CLIENT`** before `op_worker_sync_fetch`'s `std::thread::spawn` runs — e.g. build the `HttpClient` from the worker's `profile` instead of `fetch_ext::fetch_client()` (which is `None` off-thread → falls back to `chrome_148_linux`). Same fix for the worker's own `create_worker_runtime` fetch path. Then the worker's `navigator`/`importScripts`/sync-fetch identity matches the macOS window.
- **Effort:** 1–2 days.
- **Expected impact:** removes a window-vs-worker fingerprint divergence that recaptcha's `webworker.js` telemetry can read (would otherwise *lower* the score even after F1/F2). Necessary for a *passing* token, not just a *running* worker. Also helps any other in-worker fetch site (Akamai BMP blob workers).
- **Confidence:** HIGH (it's a confirmed live leak).
- **Public engine:** yes.

### F4 — Build the standalone reCAPTCHA-worker in-VM oracle (R-DUO-WORKER tooling)
- **What:** Mirror the AWS oracle (`aws_capture` + `awswaf_probe`): a `recaptcha_capture` example that fetches the live `enterprise.js`/`recaptcha__en.js`/`anchor`/`webworker.js`, an inject probe that wraps `Worker`, `MessagePort.postMessage`, `op_worker_sync_fetch`, and `grecaptcha.execute`, and runs the anchor HTML through a `run_until_idle(5s)` oracle to observe *which* of F1/F2/F3 the flow actually needs (the obfuscated bitmask gate at §3.2 means we should confirm empirically, not assume). This de-risks F1/F2 before the 2–3-week spend.
- **Effort:** 2–3 days.
- **Expected impact:** no direct flip; it's the measurement that orders F1/F2/F3 and confirms the iframe-realm hypothesis cheaply.
- **Confidence:** HIGH (the AWS oracle pattern already exists and is proven).
- **Public engine:** yes (tooling).

### F5 — Loose-gate quick win: force-render the lessons-list mount (LOW priority, possibly a measurement artifact)
- **What:** duolingo is ~1.7 KB shy of the *loose* 15 KB gate. A forced hydration of the static shell's lessons mount could nudge a *loose-gate* pass without a real token. **Do NOT pursue** as a real fix — per the holistic-FP-trap memory note, sub-30 KB "passes" are rendered FPs; the real bar is 50 KB of hydrated content. Listed only to flag it as a trap, not a win.
- **Effort:** <1 day.
- **Expected impact:** could flip a *loose-gate* scorer but not the real-content bar; not a defensible win over v150's 697 KB.
- **Confidence:** HIGH that it's a trap.
- **Public engine:** yes.

---

## 5. Definition of done
- A live duolingo nav shows: anchor iframe instantiated as a realm (F1) → `op_worker_spawn` fires for `webworker.js` (`worker_id=` in trace) → worker fetches use the macOS profile, no Linux leak (F3) → port/message rendezvous completes (F2) → `grecaptcha.execute()` promise resolves → `/api/…` hydration fetch with `X-Recaptcha-Token` → body > 50 KB.
- `worker.rs`, the 3 MessageChannel unit tests, and chrome_compat stay green.
- Re-measure with `benchmarks/run_delta_headtohead.py` on duolingo (and re-attempt a v150 same-IP trial, since the v150 driver currently crashes — get a real comparison number).

## 6. Reproduce
```bash
cat > /tmp/just_duolingo.json <<'JSON'
[{"cat":"misc","name":"duolingo","url":"https://www.duolingo.com/"}]
JSON
# Live: confirm zero worker spawns + the webworker.js sec-fetch-dest=document + Linux leak
BROWSER_OXIDE_DEBUG_NAV=1 \
RUST_LOG="js_runtime::extensions::worker_ext=trace,js_runtime::extensions::fetch_ext=debug,browser=info" \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/just_duolingo.json /tmp/duo_out.json 2>&1 \
  | grep -iE "worker_id|op_worker_spawn|webworker|sec-fetch-dest|Linux"
# Expect today: 0 worker_id lines; webworker.js fetched with sec-fetch-dest=document; one Linux-UA recaptcha refetch.
```
