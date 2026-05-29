# SITE — AWS WAF cluster (amazon-{ca,com,com-au,fr,in,jp} + imdb, + duolingo worker-path)

**Status:** root-cause CLOSED to a single public-engine lever, with new empirical
evidence that **corrects** the prior "blob-URL PoW Web Worker / live-nav drain
duration" framing. Ranked fix list at the end.
**Owner of next step:** §5.1 of `docs/HANDOFF_2026_05_28b.md` — but the fix is
NOT "longer drain"; it is "classify the AWS stub as a challenge so the existing
poll-and-retry primitive fires" (see §3.4, §5.FIX-1).
**Reading order:** this doc → `docs/HANDOFF_2026_05_28b.md §4` →
`docs/v0.1.0-parity-workflows/external/VENDOR_awswaf.md` (corrected here) →
`docs/releases/v0.1.0-parity/06_AWS_WAF_SOLVER.md` (the solver plan) →
`28_AWS_WAF_EXTENDED.md` (product family) → `41_POW_WASM_WORKER_PATTERNS.md`.

---

## 0. TL;DR — the corrected root cause

The AWS-WAF block is **NOT** a fingerprint gap, **NOT** a WASM gap, **NOT** a
blob-URL Web-Worker gap, and **NOT** primarily a drain-duration gap. The decisive,
newly-measured root cause is:

> **The public classifier (`crates/browser/src/classify.rs`) has ZERO AWS-WAF
> markers, so the 1991-byte `challenge.js` stub is classified `L3-RENDERED`
> ("successfully rendered page").** As a result the navigate loop's
> `is_anti_bot_challenge()` returns **false** for the stub, and the engine
> **never enters the 90-second poll-and-retry drain** (`page.rs:2175-2277`) nor
> the **cookie-delta re-fetch** (`page.rs:2356-2360`) that every other vendor
> (DataDome, Akamai sec-cpt, Cloudflare) gets. The stub's self-solve runs as a
> **main-thread `crypto.subtle.digest('SHA-256')` proof-of-work chain** that
> *does* complete + POST the token + call `location.reload()` **when given one
> continuous ≥5 s drain** (proven in the offline oracle), but in the live path
> it is repeatedly interrupted by the outer loop re-fetching the same stub
> before the PoW finishes.

This is **100 % public-engine**, mirrors three existing public primitives
(`is_datadome_challenge` / `is_datadome_solved` / `is_seccpt_solved`), needs no
token forging, and per `CLAUDE.md` is in scope (structural protocol markers, not
bypass code — the AWS PoW/`/verify` body shape stays in `vendor_solvers`).

Distinguish the **6 winnable** sites from the **2 you should not chase**:
`amazon-com` / `amazon-ca` fail in Camoufox v150 too (IP/probabilistic, per
`HANDOFF_2026_05_28b §3`) — out of scope. The winnable set is **imdb, amazon-in,
amazon-fr, amazon-jp, amazon-com-au, duolingo** (duolingo shares the same
main-thread-PoW execution-model gap, not a worker gap).

---

## 1. What the existing repo docs already concluded (cited)

### 1.1 `docs/releases/v0.1.0-parity/06_AWS_WAF_SOLVER.md` (the solver plan)
- Stub is **2011 B (Amazon) / 1995 B (IMDb)**, loads a per-tenant `challenge.js`,
  calls `saveReferrer()` + `getToken()`, reloads once `aws-waf-token` is set
  (`06 §0.1`). **Verified here** — the live IMDb stub is 1991 B (§3.1).
- `06 §0.3` concluded the historical block was a **fingerprint bail** (`getToken`
  never reaches its `.then`). **This was overturned** by `HANDOFF_2026_05_28b §4`
  and is overturned *again, more precisely* here (§3.2/§3.3): the chain not only
  reaches `forceRefreshToken` but **runs the PoW to completion and reloads** in
  the oracle.
- `06 §5` — the **only** AWS-aware code allowed public is the detection logger on
  `x-amzn-waf-action`. **Confirmed present** at `page.rs:1172-1173`. The fix in
  §5 of this doc adds public *classification* markers (same scope class as the
  `captcha-delivery.com` / `sec-cpt` / `_cf_chl_opt` markers already public in
  `classify.rs:82-115`), NOT bypass code.
- `06 §0.5 / §7.4` — AWS risk-rolls per request; even a perfect solver tops out
  ~85 %. This is why `amazon-com`/`amazon-ca` are not chaseable.

### 1.2 `28_AWS_WAF_EXTENDED.md` (product family + signals)
- AWS WAF Challenge action is **browser-interrogation only** (no mouse/keystroke
  behavioral signals — those are SDK-integration tokens). So no humanization is
  needed; this is purely an execution-completion problem. Aligns with the
  measured trace: the PoW reads `navigator.plugins`, `webdriver`,
  `performance.now`, WebGL once, then proceeds (§3.2).

### 1.3 `docs/HANDOFF_2026_05_28b.md §4` (the prior root-cause, now refined)
- §4 layer 1: NOT a fingerprint bail — `forceRefreshToken` is reached. **Confirmed.**
- §4 layer 3: "PoW runs in a **blob-URL Web Worker**" + "`crypto.subtle` was
  undefined in workers (fixed in `5216336`)." **This doc CORRECTS that:** for the
  current tenant `challenge.js` the PoW runs **on the main thread** via
  `crypto.subtle.digest('SHA-256')` — **no `new Worker` is constructed at all**
  in either the oracle or the live path (§3.2 measurement). The worker
  secure-context fix is still a correct prerequisite (other tenants / `getToken`
  paths can use the worker flavor; AWS ships HashcashScrypt/SHA256/NetworkBandwidth
  — see §2.1) but it is **not** the lever for the current tenant.
- §4 layer 4: "live nav produces zero async progress … 50 ms inter-script drain
  vs oracle 5 s." **Refined here:** the inter-script 50 ms drain is irrelevant
  (the inline `checkForceRefresh().then(...)` only *schedules* the PoW; the PoW
  runs in the *post-script* drain). The true difference is that the oracle gives
  **one continuous `run_until_idle(5 s)`** with no re-fetch, while the live path
  splits the budget across the build-phase drain + the outer loop and **re-fetches
  the stub** before the PoW completes — *because the stub was misclassified as
  rendered and the long poll never armed* (§3.3, §3.4).

### 1.4 `docs/v0.1.0-parity-workflows/external/VENDOR_awswaf.md`
- Carries the "blob-URL PoW Web Worker async self-solve chain" thesis verbatim
  from the handoff. **Superseded by §3 here** for the current tenant. Its §1
  doc-summary and its ranked-fix framing remain useful; treat its "worker pump
  ordering" fix as a *secondary* lever behind FIX-1 (classification).

### 1.5 `41_POW_WASM_WORKER_PATTERNS.md`
- Catalogs the PoW/Worker techniques. The measured AWS tenant uses the
  **main-thread `SubtleCrypto.digest` SHA-256** flavor — the simplest of the three
  AWS PoW types, and the one BO already fully supports (`op_crypto_digest`,
  `crypto_ext.rs:9-11`).

---

## 2. New external findings (cited)

### 2.1 AWS WAF Challenge PoW types are HashcashScrypt / SHA256 / NetworkBandwidth
AWS' own challenge runs a "computationally expensive task (proof of work)" with no
user interaction; open-source solvers confirm the three PoW flavors. The SHA-256
flavor matches the measured `crypto.subtle.digest('SHA-256')` chain exactly.
- AWS — *Protect against bots with AWS WAF Challenge and CAPTCHA actions*
  (https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/)
- AWS — *Options for challenges and token acquisition*
  (https://docs.aws.amazon.com/waf/latest/developerguide/waf-managed-protections-comparison-table-token.html)
- AWS — *Using the intelligent threat JavaScript API* / *How to use getToken*
  (https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html,
   https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html)
- `neiii/aws-waf-solver` (https://github.com/neiii/aws-waf-solver) — "handles
  HashcashScrypt, SHA256, and NetworkBandwidth challenge types"; flow = discover
  challenge URL → solve PoW → POST → receive `aws-waf-token`. (Hypothesis-grade,
  per `06 §1` caveat; used here only to corroborate the PoW-type list.)

**Implication:** the AWS PoW is a pure CPU loop of `digest` calls — it needs
**uninterrupted CPU/event-loop time on the main isolate**, not a worker, not WASM.
The fix is "let it finish", which is exactly what the existing challenge
poll-and-retry primitive provides.

### 2.2 `challenge.js` may be `defer` or `async`
AWS docs state the script "can be configured with `defer` or `async`". BO's
`script_runner::find_scripts` (`crates/browser/src/script_runner.rs:5-15`) does
**not** capture `defer`/`async`, executing every script in document order. For the
captured IMDb stub `challenge.js` is a plain `<script src>` in `<head>` followed
by the inline `checkForceRefresh()` body script — document order already matches
the intended semantics here, so this is **not** load-bearing for the current
tenant (noted for completeness; a future tenant that relies on `async` ordering
could regress).

---

## 3. New code-level analysis — the measurement trail

All runs use the in-repo oracle (`crates/browser/examples/awswaf_probe.rs`) on the
cached `/tmp/awswaf/imdb_stub.html` (1991 B, captured by `aws_capture.rs`), and the
live path via `target/release/examples/sweep_metrics chrome_148_macos`.

### 3.1 The stub (verbatim, `/tmp/awswaf/imdb_stub.html`, 1991 B)
```html
<head>
  <script>window.gokuProps = { "key":"…", "iv":"CgAEbjNs4gAAAlgJ", "context":"…" };</script>
  <script src="https://fb423e1ef94f.6277d64d.us-east-1.token.awswaf.com/…/challenge.js"></script>
</head>
<body>
  <div id="challenge-container"></div>
  <script>
    AwsWafIntegration.saveReferrer();
    AwsWafIntegration.checkForceRefresh().then((forceRefresh) => {
      if (forceRefresh) AwsWafIntegration.forceRefreshToken().then(() => location.reload(true));
      else              AwsWafIntegration.getToken().then(() => location.reload(true));
    });
  </script>
</body>
```
`challenge.js` is **1,370,004 bytes** (not the 50–150 KB `06 §0.1` assumed — the
tenant bundle grew). It contains `new Worker`×2, `requestIdleCallback`×1,
`setInterval`×1, `setTimeout`×7, `postMessage`×7 — but the Worker path is **not
taken** (§3.2).

### 3.2 Oracle trace — the self-solve COMPLETES (corrects §1.3)
Instrumented `crypto.subtle.*`, `fetch`, `XMLHttpRequest`, `Worker`,
`URL.createObjectURL`, `location.reload`, and `document.cookie` set, then ran the
oracle (`run_until_idle(5 s)`). Result (`/tmp/awswaf/imdb_*.html` probes):

- `checkForceRefresh()` resolves with `forceRefresh=true` → `forceRefreshToken()` fires.
- **NO `new Worker` constructed. NO `URL.createObjectURL`.** (The HANDOFF/VENDOR
  "blob-URL PoW Web Worker" is wrong for this tenant.)
- A long run of **`SUBTLE.digest:SHA-256` → `SUBTLE.digest_OK`** pairs (dozens
  observed) — the **main-thread SHA-256 PoW**. `getRandomValues` seeds it.
- **Two `fetch` → `200 OK`** to the `…token.awswaf.com/…` host (the PoW
  inputs/verify round-trip).
- Final events: **`POST_RESP:200 …token.awswaf.com…`** then **`RELOAD_CALLED`**.
- `errors: []`, no `unhandledrejection`.

**Conclusion: given one continuous ≥5 s drain and no re-fetch, BO solves the AWS
challenge end-to-end with its real fingerprint.** The engine machinery
(`op_crypto_digest`, async `op_fetch`, cookie jar, `location.reload` → pending-nav)
is all sufficient. There is no fingerprint, WASM, or worker gap for this tenant.

### 3.3 Live trace — the loop interrupts the PoW (the real failure)
`BROWSER_OXIDE_DEBUG_NAV=1 BROWSER_OXIDE_SC_TRACE=1 BROWSER_OXIDE_BUILD_PROFILE=1 …
sweep_metrics chrome_148_macos … just_imdb.json`:

```
[navigate] iter=0 url=https://www.imdb.com/ html_len=1991
[seccpt-trace] script fetch OK …/challenge.js status=200 bytes=1370004
[bp]  3233ms meta-refresh scanner install
[bp]  6176ms build-phase run_until_idle          ← iter0 drain EXITED at ~2.9 s
[navigate] iter=0 installing V8DeadlineWatcher with 8793ms remaining
[navigate] iter=0 FETCH GET https://www.imdb.com/  ← RE-FETCHED the stub
[navigate] iter=1 …  html_len=1991
[bp] 11123ms build-phase run_until_idle          ← iter1 drain ran the full 8 s
[navigate] iter=1 FETCH GET https://www.imdb.com/  ← RE-FETCHED again
[navigate] iter=2 … html_len=1991                 ← still the stub → returned
```

Two failure facts:
1. **iter=0's build-phase `run_until_idle` exited at ~2.9 s, not its 8 s cap.** The
   PoW is a `digest().then(...)` micro/macrotask chain; at a yield boundary the
   main isolate momentarily has no pending V8 async op, so `run_event_loop()`
   returns `Ok(())` → `run_until_idle` breaks with `IdleReason::AllWorkDone`
   (`crates/event_loop/src/lib.rs:365-368`) **mid-PoW**. (iter=1 happened to keep
   a pending timer across the whole window and ran the full 8 s — the variance is
   the tell.)
2. **The loop then re-fetches the same 1991 B stub and starts a brand-new isolate**,
   discarding all PoW progress. The PoW never gets the *one continuous* window the
   oracle gives it.

`/tmp/awswaf/imdb_w.log` confirms: `challenge.js status=200 bytes=1370004` printed
**3 times** (once per iter) — fetched repeatedly, never solved.

### 3.4 WHY the loop doesn't give it a continuous window — the decisive cause
The navigate loop has a purpose-built **90 s poll-and-retry drain** for challenge
pages (`crates/browser/src/page.rs:2175-2277`) — it loops `run_until_idle(200 ms)`
and accumulates PoW progress across ticks (each `setTimeout`/`requestIdleCallback`
yield keeps a pending timer so the next tick resumes). It also has a **cookie-delta
re-fetch** (`page.rs:2356-2360`). **Both are gated on the same condition:**
```rust
if pending_info.is_empty()
   && (page.is_anti_bot_challenge()       // ← AWS stub returns FALSE
       || started_as_dd_challenge
       || started_as_seccpt_challenge
       || started_as_cf_challenge) { … }   // 90 s poll  (line 2175)
```
`is_anti_bot_challenge()` (`page.rs:337-340`) delegates to
`crate::classify::engine_classify(body).verdict.is_challenge()`. **The classifier
has ZERO AWS-WAF markers** — verified: `grep -niE 'amzn|awswaf|goku|AwsWaf'
crates/browser/src/classify.rs` → 0 hits. The marker tables
(`classify.rs:82-115`) cover Cloudflare (`_cf_chl_opt`), Akamai (`/_sec/cp_challenge`,
`_abck`), DataDome (`captcha-delivery.com`, `ddcaptchaencoded`), Kasada
(`_kpsdk`, `ips.js`), PerimeterX — but **not** `AwsWafIntegration` / `gokuProps` /
`token.awswaf.com`.

Empirical proof:
```
$ cat /tmp/awswaf/imdb_stub.html    | classify_stdin  → L3-RENDERED   1991
$ cat /tmp/awswaf/amazonin_stub.html | classify_stdin → L3-RENDERED   2007
```
The engine literally thinks the challenge stub is a finished page. So:
- the 90 s poll never arms,
- the cookie-delta re-fetch never arms,
- the AWS host-budget tier never arms (host-budget table `page.rs:1902-1949` has
  no `awswaf`/amazon entry → imdb gets the **15 s** default; the build-phase drain
  alone burns ~6–9 s of that),
- and the stub is also under the 50 KB fast-exit floor (`page.rs:2086`) so it
  isn't even extended — the loop just spins iters 0/1/2 and returns the stub.

**This is the single highest-ROI lever and it is pure public-engine code.**

### 3.5 The solved-side primitive that must also exist
When the PoW completes it sets `aws-waf-token=…` and calls `location.reload()`.
`location.reload()` already sets `__pendingNavigation` (handled by the loop's
pending-nav break). But if the reload fires inside a poll tick we want the
loop to re-fetch *with the cookie in the shared jar*. The existing
`is_datadome_solved` / `is_seccpt_solved` (`page.rs:221-244`) pattern is the model:
add `is_awswaf_solved(cookies, body) = cookies.contains("aws-waf-token=") &&
!is_awswaf_challenge(body)` and OR it into the poll's break conditions
(`page.rs:2215`, `2254`) so the cookie-gained re-fetch fires deterministically.

### 3.6 Code map (file:line)
| Concern | Location |
|---|---|
| AWS detect logger (only AWS code public today) | `crates/browser/src/page.rs:1172-1173` |
| `is_anti_bot_challenge()` → classifier | `crates/browser/src/page.rs:337-340` |
| Classifier marker tables (NO AWS marker) | `crates/browser/src/classify.rs:82-115` |
| `is_datadome_challenge` / `_solved` (the public template) | `crates/browser/src/page.rs:208-223` |
| `is_seccpt_solved` (public template) | `crates/browser/src/page.rs:242-244` |
| `started_as_dd/seccpt/cf_challenge` setup | `crates/browser/src/page.rs:1845-1871` |
| 90 s poll-and-retry drain (gated) | `crates/browser/src/page.rs:2175-2277` |
| DD/sec-cpt solved-break inside poll | `crates/browser/src/page.rs:2215-2276` |
| Cookie-delta re-fetch (gated) | `crates/browser/src/page.rs:2356-2360` |
| Host nav-budget tiers (no AWS entry) | `crates/browser/src/page.rs:1902-1949` |
| Build-phase 50 ms inter-script drain | `crates/browser/src/page.rs:3566` |
| Build-phase 8 s post-script drain | `crates/browser/src/page.rs:3643` |
| `run_until_idle` early-exit on `AllWorkDone` | `crates/event_loop/src/lib.rs:365-368` |
| `crypto.subtle.digest` (sync op, microtask resolve) | `window_bootstrap.js:3163-3173`, `crypto_ext.rs:9-11` |
| `requestIdleCallback` = `setTimeout(cb,1)` (keeps loop pending) | `window_bootstrap.js:3572-3577` |
| `new Worker` JS binding (not taken by this tenant) | `window_bootstrap.js:1879-1960` |
| `op_worker_spawn` + secure-ctx inherit (5216336) | `crates/js_runtime/src/extensions/worker_ext.rs:221-296` |
| `op_worker_await_message` (async, keeps loop pending) | `crates/js_runtime/src/extensions/worker_ext.rs:457-499` |
| `script_runner` drops `defer`/`async` | `crates/browser/src/script_runner.rs:5-15,56-87` |

---

## 4. duolingo (shares the path)
duolingo's 13 KB shell also stalls because its first-paint async chain isn't
recognized as a challenge and so gets only the split budget. It is the **same
execution-model class** (main-thread async self-solve interrupted by the loop),
not a worker gap. The worker secure-context fix (`5216336`) is a correct
prerequisite for duolingo's recaptcha-style worker if it takes that branch, but
FIX-1's classification/continuous-drain change is the primary lever. Re-measure
duolingo with FIX-1 + FIX-2 before doing any duolingo-specific work.

---

## 5. Ranked fix list (ROI order) — ALL public-engine

> Validate every fix with `benchmarks/run_delta_headtohead.py 5` on the AWS
> cluster + duolingo (`HANDOFF_2026_05_28b §6`), and the offline oracle for fast
> inner-loop iteration. Acceptance per `06 §4`: imdb/amazon-in ≥ 5–7/10; no
> regression > −2 elsewhere; full 126-gate once before v0.2.0 cert.

### FIX-1 — Classify the AWS-WAF stub as a challenge (THE lever)
**What:** Add a public-engine `is_awswaf_challenge(html)` mirroring
`is_datadome_challenge` (`page.rs:208`):
`html.len() < 4096 && html.contains("AwsWafIntegration") && html.contains("gokuProps")`
(structural AWS-protocol markers — same scope class as the public `captcha-delivery.com`
/ `sec-cpt` / `_cf_chl_opt` markers already in `classify.rs`; NOT bypass code).
Add `started_as_awswaf_challenge` at `page.rs:1871` and OR it into the **90 s poll
gate** (`page.rs:2175`) and the **cookie-delta re-fetch gate** (`page.rs:2356`). This
gives the main-thread SHA-256 PoW the continuous-tick window it provably needs
(§3.2/§3.4). Optionally also OR the marker into the classifier's challenge table so
`is_anti_bot_challenge()` returns true directly (cleaner, but watch the size gate so
a *rendered* amazon page that mentions the string isn't misflagged — keep the
`< 4096` / `< 50_000` body-size guard as DataDome does).
**Effort:** 0.5–1 day. **Expected impact:** the whole winnable cluster —
imdb + amazon-in (the deterministic stubs) flip to solvable; amazon-fr/jp/com-au
reliability rises; duolingo re-tests. Up to **6 sites**. **Confidence:** high — the
oracle proves the solve completes once uninterrupted; this is the only thing
preventing it live. **Public engine:** yes.

### FIX-2 — `is_awswaf_solved` break + cookie-gained re-fetch
**What:** Add `is_awswaf_solved(cookies, body) = cookies.contains("aws-waf-token=")
&& !is_awswaf_challenge(body)` (mirror `is_seccpt_solved`, `page.rs:242`) and OR it
into the poll's solved-break (`page.rs:2215/2254`) so the loop re-fetches the
original URL the instant the token lands, instead of waiting out the 90 s deadline
or racing the `location.reload` pending-nav. Ensures the cookie-gained re-fetch is
deterministic, not budget-luck.
**Effort:** 0.5 day (rides on FIX-1). **Expected impact:** converts FIX-1's
"sometimes solves" into reliable 5/5 on imdb/amazon-in; tightens the cluster.
**Confidence:** high. **Public engine:** yes.

### FIX-3 — Add an AWS host-budget tier
**What:** Add `amazon.* / imdb.com` (and a generic `token.awswaf.com`-seen flag) to
the host-budget table (`page.rs:1902-1949`) at the Akamai-BMP tier (25 s) so the
PoW + token round-trip + reload comfortably fit, rather than racing the 15 s default
that the build-phase drain already half-consumes.
**Effort:** 0.25 day. **Expected impact:** removes the budget-starvation tail on
slower PoW rolls; raises amazon-fr/jp/com-au reliability. **Confidence:** medium-high.
**Public engine:** yes.

### FIX-4 — Harden `run_until_idle` against premature `AllWorkDone` for challenge pages
**What:** The early-exit at `event_loop/src/lib.rs:365-368` breaks the moment the
main isolate momentarily idles, even mid-PoW (§3.3 fact 1). Add an opt-in
"settle-confirm" mode: when draining a known-challenge page, require N consecutive
idle ticks (or a small grace window) before declaring `AllWorkDone`, so a single
microtask-gap doesn't end the PoW drain. (FIX-1's poll loop already mitigates this
by re-calling `run_until_idle(200 ms)`, so FIX-4 is a robustness backstop, not
strictly required.)
**Effort:** 1 day + careful regression (touches every drain). **Expected impact:**
reduces PoW-completion variance across the cluster; helps any main-thread-PoW vendor.
**Confidence:** medium (engine-wide blast radius — gate carefully). **Public engine:** yes.

### FIX-5 — Capture `defer`/`async` in `script_runner` (forward-proofing)
**What:** Extend `ScriptInfo` (`script_runner.rs:5-15`) with `defer`/`async` and
honor execution timing (defer → after parse, before DOMContentLoaded). Not
load-bearing for the current tenant (§2.2) but removes a latent regression if AWS
ships an `async` challenge.js variant, and improves general Chrome fidelity.
**Effort:** 1–2 days. **Expected impact:** 0 sites today; insurance + correctness.
**Confidence:** medium. **Public engine:** yes.

### NON-FIX — do NOT chase amazon-com / amazon-ca
Both fail in Camoufox v150 too (`HANDOFF_2026_05_28b §3`: amazon-com 1/5 v150,
amazon-ca 0/5 v150) → IP/probabilistic per-request risk-rolling (`06 §0.5`). No
engine lever flips them; pursuing them burns budget against AWS' dice.

### OUT OF SCOPE (vendor_solvers only, per CLAUDE.md)
Any token *forging* (parsing `gokuProps.{key,iv,context}`, reimplementing
HashcashScrypt/SHA256/NetworkBandwidth, POSTing a synthetic `/verify` body,
in-flight `challenge.js` rewrite). **Not needed** — FIX-1+2 let AWS' own JS mint a
real token. Keep these in the private `vendor_solvers::AwsWafSolver` as
`06 §3` Alt-B/C if AWS ever blocks the self-solve path.

---

## 6. Definition of done (supersedes `HANDOFF_2026_05_28b §8`)
- FIX-1 + FIX-2 land; live imdb nav shows: challenge.js fetched **once**, a single
  continuous challenge-drain, `aws-waf-token` in the jar, `__pendingNavigation`
  set by `location.reload`, cookie-gained re-fetch returns real IMDb content
  (≥ 15 KB, `L3-RENDERED`).
- `run_delta_headtohead.py 5` shows imdb + amazon-in flip (≥ 1/5, targeting v150
  parity) and duolingo re-measured.
- No regression in `holistic_sweep` / chrome147 / worker.rs.
- Update `external/VENDOR_awswaf.md §1.3` and `HANDOFF_2026_05_28b §4` to the
  corrected main-thread-PoW + classifier-gap root cause.

— Research agent, 2026-05-28 (AWS-WAF cluster deep-dive, evidence in `/tmp/awswaf/`)

---

## ADDENDUM — Live validation 2026-05-28 (implementation session)

Implemented M-2 (`is_awswaf_challenge`/`is_awswaf_solved` + `started_as_awswaf_challenge`
+ poll/retry wiring), M-3 (refed timers on challenge navs), M-4 (25s budget tier),
F3 (worker-thread fetch identity), INVERSE-CHL classifier guard, and env-gated
worker/blob instrumentation. Commits `6e35f42`, `12c074a`, `f3276a8`, `42dea01`,
`7598fe3`.

**Live re-measure on imdb (chrome_148_macos), imdb actively serving the AWS
challenge (HTTP 202):**

- ✅ M-4 works — `V8DeadlineWatcher with ~23s remaining` (budget bumped to 25s tier).
- ✅ M-2 works — `started_as_awswaf_challenge` arms the cookie-diff retry (3 iters);
  INVERSE-CHL now tags imdb `AWS-WAF-CHL` (was silently `ThinShell` at 1995 B).
- ✅ challenge.js loads — `script fetch OK …token.awswaf.com/…/challenge.js status=200
  bytes=1370004` every iteration.
- ❌ **NOT flipped** — in-V8 refetch returns `status=202` (the stub again); token unsolved.

**DEFINITIVE root cause (corrects this doc's earlier drain hypothesis):** with the
new instrumentation, challenge.js loads + executes but emits **ZERO `op_blob_register`
and ZERO `op_worker_spawn`**. It bails inside the stub's trailing
`AwsWafIntegration.checkForceRefresh().then(fr => fr ? forceRefreshToken() : getToken())`
chain **before the blob PoW worker is ever created**. So this is **NOT** a live-nav
async-drain gap (M-1) — a longer drain has nothing to drain. The offline oracle DID
reach getToken (handoff §4), so the difference is environment/execution inside
challenge.js's own logic.

**Next lever (task #21):** inject an instrumentation init_script into a live AWS nav
wrapping `Worker` / `URL.createObjectURL` / `AwsWafIntegration.*` + capturing
`__scriptErrors` to learn whether `checkForceRefresh()` resolves/rejects, which branch
runs, and whether an internal fetch (token/report endpoint) fails. M-1's drain idea is
deprioritized for AWS (still possibly relevant to booking SPA hydration).
