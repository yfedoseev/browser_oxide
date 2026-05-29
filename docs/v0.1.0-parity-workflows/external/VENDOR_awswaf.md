# VENDOR — AWS WAF token challenge (`challenge.js` → `aws-waf-token`)

**Status:** technical deep-dive + ranked fix list.
**Audience:** the next engineer picking up the AWS-WAF cluster (`§5.1` of
`docs/HANDOFF_2026_05_28b.md`).
**Reading order:** this doc → `docs/HANDOFF_2026_05_28b.md §4` (the live-nav
root-cause) → `docs/releases/v0.1.0-parity/06_AWS_WAF_SOLVER.md` (the solver
design) → `28_AWS_WAF_EXTENDED.md` (product family + signal inventory) →
`41_POW_WASM_WORKER_PATTERNS.md` (the PoW/Worker technique inventory).

**One-line thesis (corrected against the 05-28 measurement):** AWS WAF is
**not** a fingerprint/stealth gap. `challenge.js` runs end-to-end under BO's
fingerprint in the offline oracle and reaches `forceRefreshToken`. The live
block is an **engine execution-model gap**: the blob-URL PoW Web Worker's
async self-solve chain (`checkForceRefresh().then(...)` → worker spawn → PoW →
`/verify` POST → `aws-waf-token` cookie → reload) never advances to completion
in the `navigate_with_init` live path. Closing it is **public-engine** work
(event-loop drain + worker pump ordering). No token forging is required, and
any token forging that *were* attempted is **`vendor_solvers`-only** per
`CLAUDE.md`.

---

## 1. What the repo docs already concluded (cite)

### 1.1 `06_AWS_WAF_SOLVER.md` (the solver plan)
- The challenge HTML stub is **2011 B (Amazon) / 1995 B (IMDb)**, verified
  correct. It loads a per-tenant `challenge.js` (~50–150 KB) and calls
  `AwsWafIntegration.saveReferrer()` + `getToken()`, then reloads once
  `aws-waf-token` is set (`06 §0.1`, TL;DR 1).
- BO **executes** `challenge.js` (a `/report` telemetry POST was observed),
  but historically `getToken()` "never reaches its `then(token => …)`
  continuation" → no `/verify`, no cookie, no reload (`06 §0.3`, TL;DR 2).
  The doc's *then-current* conclusion was a **fingerprint bail** (`06 §0.3
  point 3`) — this is the claim the 05-28 measurement overturned (see §3).
- Three solver tracks: **A** engine-side stealth (PUBLIC), **B** Rust-side
  token POST / WASM-under-wasmtime (PRIVATE `vendor_solvers`), **C** in-flight
  `challenge.js` rewrite (PRIVATE). Recommended order A→B→C (`06 §3`).
- Detection-only logger is the **only** AWS-aware code allowed public; it
  lives at `crates/browser/src/page.rs` on `x-amzn-waf-action` (`06 §0.3`,
  `§5`). Confirmed still present — see §4.1.
- Hard ceiling: AWS risk-rolls per-request, so even a perfect solver tops out
  ~85 % (`06 §0.5`, `§7.4`).

### 1.2 `28_AWS_WAF_EXTENDED.md` (product family + signals)
- AWS WAF is **four** products sharing the `aws-waf-token`: Challenge action,
  CAPTCHA action, Bot Control, Fraud Control (ATP/ACFP). Our corpus only ever
  hits the **silent Challenge action** (`28 §0`).
- Challenge action tokens are **browser-interrogation only** — *no* mouse /
  keystroke behavioral signals (those are SDK-integration tokens). So
  behavioral simulation is **not** the gate (`28 §1.7`). This is load-bearing:
  it narrows the candidate failure set to passive fingerprint + execution.
- `getToken()` documented 2 s wait then **throws** on timeout; BO observed the
  promise stays **pending forever** — an anomaly flagged but unexplained
  (`28 §1.3`). §3/§5 here resolve it: the promise is pending because the
  worker round-trip never resolves in the live path.
- Token carried by **two** mechanisms: `Set-Cookie: aws-waf-token` AND request
  header `x-aws-waf-token` (`28 §1.5`). BO's cookie side is wired
  (SharedSession); the header side is **unwired** (`28 §1.5`).
- Endpoints `/inputs`, `/verify`, `/report` on `*.token.awswaf.com`; the
  `gokuProps.context` binds the token to a domain so a minted token can't be
  replayed across regional Amazon tenants (`28 §1.6`, `§5.1`).
- 18+ signal inventory with BO file:line coverage (`28 §3.2`); top-5
  "investigate" gaps = WebGL `getSupportedExtensions()` order, worker
  `userAgentData`, UA/UA-CH triple-coherence, WASM feature coverage,
  `performance.now()` quantization (`28 §3.3`).

### 1.3 `41_POW_WASM_WORKER_PATTERNS.md` (technique inventory)
- "PoW gate is upstream" pattern (`41 §2.5`): vendors filter on a cheap
  fingerprint check **before** issuing the PoW; adding faster PoW/WASM does
  not lift any failing corpus site — the gate is the leverage.
- BO runs JS+WASM PoW at Chrome speed via deno_core V8 (`41 §2.4`). The one
  structural WASM risk is **threads & atomics** (needs `SharedArrayBuffer` +
  cross-origin-isolated COOP/COEP, which BO doesn't set) (`41 §3.4`).
- **Stale claim to correct:** `41 §4.3`/`§6` and `05` say `MessageChannel` /
  `MessagePort` is a **NO-OP STUB** and rank fixing it as the top lever. This
  is **no longer true** — paired-port routing is now implemented
  (`window_bootstrap.js:2318-2440`, see §4.4). The docs predate that landing.

---

## 2. The protocol, grounded (external research)

### 2.1 Authoritative AWS docs
- Challenge action serves a small HTML page running a silent PoW; success sets
  `aws-waf-token`; min challenge immunity **300 s**
  ([waf-tokens-immunity-times](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-immunity-times.html)).
- Integration installed as `<script src="…/challenge.js" defer>`; auto-retrieves
  a token in the background on load
  ([waf-js-challenge-api](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html)).
- `getToken()` is async, returns a Promise, waits ≤2 s, throws on timeout;
  stores the cookie named `aws-waf-token`
  ([waf-js-challenge-api-get-token](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html)).
- Challenges run **only on HTTPS / secure contexts**
  ([waf-captcha-and-challenge-how-it-works](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-how-it-works.html))
  — this is why the 05-28 worker `crypto.subtle` secure-context fix
  (commit `5216336`) was a necessary prerequisite.
- "Computationally expensive task (proof of work)"
  ([Challenge & CAPTCHA blog](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/)).
- AWS docs explicitly note the challenge types are "computational tasks that
  can be executed in **web workers** for processing-intensive operations" —
  confirming the blob-Worker PoW design BO observed in `challenge.js`.

### 2.2 Reverse-engineering: the 5-phase flow (deepwiki query of
`xKiian/awswaf`, confirmed by `neiii/aws-waf-solver`, `Switch3301/Aws-Waf-Solver`)

1. **Extract** `gokuProps {key, iv, context}` + WAF `host` from the initial
   HTML stub (`window.gokuProps`; `host` is the `src` of the `challenge.js`
   script tag, e.g. `…d474e66d.us-west-2.token.awswaf.com`).
2. **GET `{host}/inputs?client=browser`** → returns `{challenge_type, input,
   difficulty}`.
3. **Generate fingerprint** → a **CRC32 checksum** + an **AES-GCM-encrypted**
   signals payload. (Correction vs `06 §0.2`/`28 §5`, which said AES-**CBC**:
   the current solvers use **AES-GCM** + CRC32, not CBC.)
4. **Solve PoW** = iterate `nonce` until `H(challengeInput ‖ checksum ‖ nonce)`
   meets `difficulty`. Algorithm chosen per-request by `challenge_type`:
   **SHA-256** (`HashPoW`) or **Scrypt** (`ComputeScryptNonce`); a third
   **NetworkBandwidth** flavor exists (`neiii`).
5. **POST `{host}/verify`** with JSON body:
   `{challenge, solution(nonce), signals:[{encrypted fingerprint}], checksum,
   existing_token, client:"Browser", domain, metrics:[…timings], goku_props}`.
   Response `{token}` → set as cookie `aws-waf-token` (also accepted as
   `x-aws-waf-token` header).

**Note:** the public solvers (`xKiian`) compute the PoW **directly in
Go/Python** and do **not** spawn a Worker — that is the *bypass* path. The real
browser `challenge.js` *does* spawn a blob-URL Worker (HANDOFF_2026_05_28b §4
grep: `new Worker ×2 + new Blob ×2, zero WebAssembly.*`). So **today's Amazon
tenant ships a JS-Worker PoW, not a WASM PoW** — superseding the "WASM PoW"
framing in `06 §0.1`/`28 §4`/`41 §3.2`. The `AGFzbQ` WASM-magic tell from
`06 §1.4` is not present in the captured bundle. This matters for fix routing:
there is no WASM module to run under wasmtime; the PoW is plain JS in a worker
that V8 executes natively.

### 2.3 Where the gate sits (resolved)
`06`/`28`/`41` all assumed a **fingerprint gate** that bails before the PoW.
The 05-28 oracle measurement (HANDOFF §4 point 1) disproves this for the
current Amazon tenant: fed the live stub through `aws_capture` → `awswaf_probe`,
`challenge.js` reads `navigator.plugins ×24`, `webdriver`, WebGL unmasked
vendor/renderer + `getSupportedExtensions`, `chrome.csi/loadTimes`,
`performance.now` — **and proceeds to call `forceRefreshToken`**. So the
fingerprint passes; the block is purely that the async self-solve does not run
to completion in the live path.

---

## 3. The BO-specific root cause (HANDOFF_2026_05_28b §4, verified in source)

Four layers were peeled (HANDOFF §4):
1. **NOT a fingerprint bail** — challenge.js proceeds (oracle reaches
   `forceRefreshToken`). ⇒ FIX-D2 (WebGL split) and any stealth fix **cannot**
   flip AWS; confirmed by the post-FIX-D2 delta (imdb/amazon-in unchanged).
2. **`challenge.js` IS fetched live** — `[seccpt-trace] challenge.js status=200
   bytes=1370004` on every nav iteration.
3. **NOT WebAssembly** — the PoW is a **blob-URL Web Worker** (`new Worker` +
   `new Blob`). The worker needed `crypto.subtle` for SHA-256; it was
   `undefined` in BO workers (SecureContext-gated). **Fixed** commit `5216336`
   (`op_worker_spawn` now inherits `is_secure_context`).
4. **The remaining lever** — even with the worker fix, the **live navigate
   path produces ZERO async progress**: challenge.js is fetched ×3 but there is
   **no worker spawn, no `/verify` POST, no token cookie**. The **offline
   oracle** (`from_html_with_url` + `run_until_idle(5s)`) runs it fully to
   `forceRefreshToken`.

### 3.1 Why the oracle works but the live path does not (code diff)

The two build paths execute external scripts differently:

| | Offline oracle `from_html_with_url` (`page.rs:475`) | Live `build_page_with_scripts_init_and_storage` (`page.rs:3053`) |
|---|---|---|
| External-script fetch | serial `get_follow` inside the doc-order loop (`:557`) | parallel prefetch into a map (`:3110-3245`), executed later (`:3511`) |
| Inter-script drain | **none** — tight synchronous loop (`:538-574`) | `run_until_idle(50ms)` **between every script** (`:3566`) |
| DOMContentLoaded / load | fired **synchronously** right after the loop (`:586-599`) | fired via `setTimeout(…,0)` (`:3582`) so handlers run inside the drain |
| Final settle drain | one `run_until_idle(8s)` (`:611`) | `run_until_idle(8s)` build drain (`:3643`) **then** the nav-loop drain (`:2055`, floored 8s, up to the 15 s host budget) |

The live path actually gives **more** total drain time than the oracle. So the
gap is not "not enough seconds" in aggregate — it is an **ordering / liveness**
problem in how the worker round-trip is pumped. The most probable mechanisms,
in priority order:

**(a) `defer` execution ordering.** `challenge.js` is `<script defer>`.
`find_scripts` (`script_runner.rs:19-92`) does **not** read the `defer` or
`async` attribute — every script is emitted in pure document order
(`script_runner.rs:69-86`). In the live path that means challenge.js executes
**before** the synthetic DOMContentLoaded (`page.rs:3582`). If the real
`challenge.js` defers its `getToken()` kickoff to a DOMContentLoaded listener
(the documented install pattern), the kickoff fires later, inside the 8 s
build drain — which is fine **only if** the worker pump survives that long. In
the oracle, DOMContentLoaded fires synchronously (`:588`) immediately, so the
chain starts at t≈0 with a full 8 s ahead of it. **In the live path the
chain may start much later** (after every inter-script 50 ms drain for a
1.37 MB bundle's worth of sibling scripts) and then races the budget.

**(b) Worker pump vs. the 50 ms inter-script drain.** The Worker pump is an
async chain: `op_worker_await_message` (`worker_ext.rs:457`) suspends on a
`tokio::sync::Notify`; the worker thread posts back and calls
`notify_parent.notify_one()` (`worker_ext.rs:513`). The mechanism is correct
(verified in source — see §4.4) and `run_event_loop()` will treat the
outstanding await as pending so `run_until_idle` won't return `AllWorkDone`
early. **But** the worker runs on its **own OS thread with its own
current-thread tokio runtime** (`worker_ext.rs:268-294`) and only drives its
event loop in **25 ms ticks** with a **5 ms sleep between ticks**
(`worker_ext.rs:340-346`). A SHA-256/Scrypt PoW that takes 50–500 ms therefore
completes across many ticks; meanwhile the parent must be **inside a
`run_event_loop()` await** to receive the `notify_one()`. The 50 ms
inter-script `run_until_idle` (`page.rs:3566`) is fine, but if the spawning
script is the **last** in the doc-order loop, the very next thing the parent
does is the 8 s drain — good — **unless** the worker spawn happens during the
nav-loop transition where the page is rebuilt.

**(c) The cookie-delta "F5" retry can't fire.** The live path's re-fetch with
the token is gated on a **cookie delta** during script execution
(`page.rs:2008-2016` snapshot before, `:2380+` compare after; the in-V8
re-fetch at `:2449-2472`). If the worker never posts → `/verify` never fires →
`aws-waf-token` never lands in the jar → no cookie delta → **no retry** → the
2011 B stub is returned as final. So even a *partially* working chain that
stalls anywhere before the cookie write produces exactly the observed "stub
sticks" symptom.

**(d) AWS hosts get no extended budget.** The host-budget table
(`page.rs:1902-1949`) gives Kasada 45 s, SPA shells 90 s, Akamai 25 s, but
`amazon.*` / `imdb.com` fall through to the **15 s default**. With a 1.37 MB
challenge bundle to parse+execute plus a worker PoW plus a `/verify` round-trip
plus a reload, 15 s is tight. There is **no** `token.awswaf.com` / `amazon`
arm in the budget match.

---

## 4. BO code-level analysis (file:line)

### 4.1 Detection (public, keep as-is)
`crates/browser/src/page.rs:1172-1173` — logs `[vendor-detect] aws-waf <action>`
on the `x-amzn-waf-action` header. Pure observation, no flow change. This is
the only AWS-aware line allowed in the public engine (`06 §5`). Confirmed the
old `page.rs:1061` reference in `06`/`28` has drifted to `:1172`.

### 4.2 The live script-execution loop + drains
- `build_page_with_scripts_init_and_storage` — `page.rs:3053`.
- Parallel external-script prefetch — `page.rs:3110-3245` (CSP-gated at
  `:3122`; `BROWSER_OXIDE_SC_TRACE` per-fetch trace at `:3163-3182`).
- Doc-order execution loop — `page.rs:3511-3567`; the **50 ms inter-script
  drain** is `page.rs:3566` (`run_until_idle(Duration::from_millis(50))`).
- DOMContentLoaded/load via `setTimeout(…,0)` — `page.rs:3582-3586`.
- **8 s build-phase settle drain** — `page.rs:3643`
  (`run_until_idle(Duration::from_secs(8))`). The comment block (`:3628-3642`)
  already names AWS WAF as a beneficiary of this drain — but it is the *build*
  drain, run **once per build**, before the nav-loop re-fetch logic.

### 4.3 The navigate loop (the F5 / cookie-delta retry)
- Loop + per-host budget — `page.rs:1902-1978`. **No AWS arm** (gap (d)).
- iter-0 cookies snapshot — `page.rs:2008-2016`.
- Build + V8DeadlineWatcher + nav-budget drain — `page.rs:2018-2057`
  (`drain_timeout = remaining.max(8s)`, `:2051-2055`).
- Cookie-delta → in-V8 re-fetch (the "F5 primitive") — `page.rs:2408-2472`;
  the `v8_html_is_real` guard rejects a re-fetch that still contains
  `AwsWafIntegration`/`gokuProps` (`page.rs:2521-2522`) so a stub-for-stub
  swap can't be mistaken for a solve.

### 4.4 Worker IPC — verified correct (corrects the stale "no-op stub" docs)
- `Worker` class + `_drainOnce()` await-pump — `window_bootstrap.js:1879-1960`.
  Uses `op_worker_await_message(...).then(...)` chained recursively (`:1935`).
- `postMessage` wire-serialization + transfer validation —
  `window_bootstrap.js:1975-2017`.
- Blob-URL worker resolution — `window_bootstrap.js:1860-1894`
  (`_resolveWorkerScript` handles `blob:`), backed by `op_blob_register` from
  `URL.createObjectURL` (`window_bootstrap.js:4396-4414`).
- **`MessageChannel`/`MessagePort` paired-port routing is implemented** —
  `window_bootstrap.js:2318-2440` (peer wiring, buffered until `start()` /
  `onmessage`, microtask dispatch). This **contradicts** `41 §4.3`/`§6` and
  `05 §2.3` which call it a NO-OP stub and rank it the #1 lever. Those docs are
  stale; update them.
- Worker side: spawn on dedicated 64 MB-stack OS thread with its own
  current-thread tokio runtime — `worker_ext.rs:221-374` (secure-context
  inheritance at `:236-240`, `:296`). Event loop driven in 25 ms ticks + 5 ms
  sleeps — `worker_ext.rs:336-357`.
- Parent await op — `worker_ext.rs:457-499`; worker→parent notify —
  `worker_ext.rs:505-514` (`op_worker_self_post` → `notify_one`).
- Idle detection — `crates/event_loop/src/lib.rs:288-391`: `run_until_idle`
  returns `AllWorkDone` only when `run_event_loop()` returns `Ok(())`; a
  pending `op_worker_await_message` keeps it from returning early. Sound.

### 4.5 The offline oracle (why it completes)
`from_html_with_url` — `page.rs:475-611`: serial script fetch (`:557`),
**no** inter-script drain, **synchronous** DOMContentLoaded/load (`:586-599`),
single `run_until_idle(8s)` (`:611`). The oracle harness
(`crates/browser/examples/awswaf_probe.rs`, `aws_capture.rs`) wraps this and
HANDOFF §4 reports it reaches `forceRefreshToken`. The behavioral delta vs the
live path is the synchronous, immediate, uninterrupted kickoff of the
DOMContentLoaded-gated `getToken()` chain.

---

## 5. What BO must do for worker-spawn + PoW + token-POST in the live path

The goal (HANDOFF §5.1, §8): make the live nav show the worker spawn, the
`/verify` POST, the `aws-waf-token` cookie write, and the cookie-gained
re-fetch returning real content. Concrete prerequisites:

1. **Worker must actually spawn.** Instrument: run live imdb with
   `RUST_LOG="js_runtime::extensions::worker_ext=debug"` (HANDOFF §5.1 repro).
   If `op_worker_spawn` (`worker_ext.rs:221`) is never called, the chain dies
   *before* the worker — point at the DOMContentLoaded ordering / defer
   handling (gap (a)), not the worker. **Most likely first finding.**
2. **DOMContentLoaded must fire before the build drain ends, and the
   getToken() chain must be allowed to start.** Because `find_scripts` ignores
   `defer`, challenge.js executes in document order; verify its `getToken()`
   kickoff (whether on DOMContentLoaded or immediate) actually runs and is not
   swallowed (capture `window.__scriptErrors` after the loop — already dumped
   at `page.rs:3649`).
3. **The page must stay alive (inside `run_event_loop`) while the worker
   computes + posts back + the `/verify` fetch resolves + the reload fires.**
   The 8 s build drain (`:3643`) + nav drain (`:2055`) cover this *if* the
   chain has started. If it starts late (gap (a)) it can run out of the 15 s
   default host budget (gap (d)).
4. **The cookie write must produce a jar delta** so the F5 retry
   (`page.rs:2408`) re-fetches. Confirm `/verify`'s `Set-Cookie: aws-waf-token`
   reaches `crates/net/src/cookie_jar.rs` (shared jar, commit `f62584d`), or
   that `document.cookie = aws-waf-token=…` from JS routes there.

### 5.1 Concrete public-engine fixes (ranked in §7)
- Honor `defer`/`async` in `find_scripts` and order execution so deferred
  scripts run after the synthetic DOMContentLoaded (matches Chrome; lets the
  getToken chain start at a deterministic point with full drain ahead of it).
- Add a `token.awswaf.com`-aware (or "stub < 4 KB contains `AwsWafIntegration`")
  **drain-budget bump** to ~30 s, the same generic mechanism Kasada/Akamai
  already use (`page.rs:1902`). This is vendor-*detection*-driven budget, not
  vendor bypass — same class as the existing host arms.
- Ensure the worker pump is **actively driven** during the build drain: today
  the build drain (`:3643`) is a `run_until_idle` that should pump
  `op_worker_await_message`, but verify the worker thread's 25 ms/5 ms tick
  cadence (`worker_ext.rs:340-346`) isn't starving a sub-500 ms PoW of timely
  `notify_one()` delivery under a parent that's busy parsing a 1.37 MB bundle.
- Consider firing DOMContentLoaded/load **synchronously** (oracle style) for
  challenge-stub pages instead of via `setTimeout(…,0)`, so the chain starts
  immediately.

### 5.2 What stays in `vendor_solvers` (NOT public)
Any actual **token forging** — parsing `gokuProps`, generating the CRC32 +
AES-GCM signals envelope, computing the SHA-256/Scrypt PoW in Rust, POSTing
`/verify`, injecting the `aws-waf-token` cookie — is the `AwsWafSolver` impl of
`ChallengeSolver`, private per `CLAUDE.md` (`06 §3.B`, `§5`). The public engine
must contain **no** string match on `gokuProps`/`AwsWafIntegration` beyond the
existing detection logger and the `v8_html_is_real` guard, and **no** call to
`*.awswaf.com` endpoints. The `x-aws-waf-token` request-header carrier
(`28 §1.5`) is generic enough to be a public primitive but is only useful once
a solver mints a token, so treat it as solver-adjacent.

---

## 6. Open questions to resolve with the instrumentation
1. Does `op_worker_spawn` fire at all in the live imdb nav? (settles gap (a)
   vs (b)).
2. Does challenge.js gate `getToken()` on DOMContentLoaded, or call it
   immediately at module-eval? (determines whether the `defer`-ordering fix is
   load-bearing).
3. Is the worker's SHA-256 PoW actually completing (worker posts a result) and
   the parent just not re-fetching, or does the worker never finish? (capture
   `op_worker_self_post` debug).
4. Does `/verify` 200 set `aws-waf-token` via response `Set-Cookie` (jar) or
   only via JS `document.cookie`? (determines whether the cookie-delta snapshot
   at `page.rs:2008` sees it).
5. Is the current Amazon tenant's PoW pure-JS-in-worker (as the 05-28 grep
   suggests) on **all** AWS arms, or does imdb/amazon-in differ from
   amazon-fr/jp? (affects whether one fix covers the whole cluster).

---

## 7. Ranked fix list (ROI order)

> All "expected impact" figures are against the 05-28 trustworthy baseline
> (HANDOFF §3): imdb 0/5, amazon-in 0/5 (hard); amazon-fr 1/5, amazon-jp 3/5,
> amazon-com-au 2/5 (reliability); amazon-com/ca = IP/probabilistic (don't
> chase). duolingo (0/5) is the same Worker-PoW class and may ride along.

### FIX-AWS-1 — Live-nav async self-solve drain (the §5.1 lever)
Instrument the live build (worker_ext + fetch_ext debug) to find exactly where
the chain stalls; then (a) honor `defer` in `find_scripts` so the getToken
chain starts after a deterministic DOMContentLoaded, (b) fire DOMContentLoaded
synchronously for challenge stubs, (c) ensure the worker pump is driven through
the build+nav drains. **Public engine.**

### FIX-AWS-2 — AWS/challenge-stub drain-budget bump
Add a generic detection-driven budget arm (stub `<4 KB` containing
`AwsWafIntegration`, or `Host: *.token.awswaf.com`) raising the 15 s default to
~30 s — same mechanism as the existing Kasada/Akamai host arms
(`page.rs:1902-1949`). Cheap; de-risks FIX-AWS-1 by removing the budget race.
**Public engine.**

### FIX-AWS-3 — Worker-thread tick cadence / pump latency audit
Verify the worker's 25 ms-tick + 5 ms-sleep loop (`worker_ext.rs:336-357`)
isn't adding tens of ms of latency to `notify_one()` delivery for a sub-500 ms
PoW while the parent parses a 1.37 MB bundle. Tighten if measured to matter.
**Public engine.**

### FIX-AWS-4 — Cookie-write routing for `aws-waf-token`
Confirm `/verify`'s `Set-Cookie` (or JS `document.cookie`) reaches the shared
jar so the cookie-delta F5 retry (`page.rs:2408`) fires; wire the
`x-aws-waf-token` request-header carrier if the tenant uses the header path.
**Public engine** (generic cookie/header plumbing).

### FIX-AWS-5 — `AwsWafSolver` in `vendor_solvers` (fallback only)
Rust-side `gokuProps` parse + CRC32/AES-GCM signals + SHA-256/Scrypt PoW +
`/verify` POST + `aws-waf-token` injection. Only if FIX-AWS-1..4 cannot drive
the in-browser self-solve. **PRIVATE `vendor_solvers`** per `CLAUDE.md`;
~30-day rotation maintenance.

### FIX-AWS-6 — Update stale `MessageChannel` "no-op stub" claims
Doc-only: `41 §4.3`/`§6`, `05 §2.3`, and the leverage matrices assert
`MessageChannel`/`MessagePort` is a no-op and rank it #1; it is implemented
(`window_bootstrap.js:2318-2440`). Correct the docs so the next engineer
doesn't re-do landed work. **N/A engine.**

---

## 8. Sources
- AWS — [Using the intelligent threat JS API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html)
- AWS — [getToken](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html)
- AWS — [How CAPTCHA & Challenge work (HTTPS / secure context)](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-how-it-works.html)
- AWS — [Token immunity times (300 s floor)](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-immunity-times.html)
- AWS — [Token use in intelligent threat mitigation](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens.html)
- AWS — [Challenge & CAPTCHA actions blog (PoW)](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/)
- [xKiian/awswaf](https://github.com/xKiian/awswaf) (5-phase flow, gokuProps, AES-GCM+CRC32, SHA-256/Scrypt, `/inputs`+`/verify`; via deepwiki)
- [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) (HashcashScrypt/SHA256/NetworkBandwidth flavors)
- [Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver)
- [roundproxies — bypass AWS WAF 2026](https://roundproxies.com/blog/bypass-aws-waf/)
- Repo: `docs/HANDOFF_2026_05_28b.md`, `docs/releases/v0.1.0-parity/{06,28,41}*.md`
