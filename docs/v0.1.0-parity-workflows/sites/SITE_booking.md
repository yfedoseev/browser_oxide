# SITE: booking.com — root cause + fix plan

**URL:** `https://www.booking.com/`
**Bench symptom:** BO returns an ~8.4 KB shell (`L3-RENDERED`, < 15 KB thin-shell
gate → `Pass=false`); Camoufox v150 renders 465–513 KB. Same-IP delta baseline:
**BO 0/5, v150 5/5 (HARD ENGINE GAP)** — `HANDOFF_2026_05_28b.md` §3.
**Author:** research agent, 2026-05-28.
**One-line conclusion:** **booking.com is NOT an SPA-hydration gap. It is the AWS
WAF JS-challenge cluster.** Its homepage is served behind a self-hosted AWS WAF
`challenge.js` that is byte-identical in size and mechanism to imdb/amazon. The
fix is the SAME engine lever as the rest of the AWS cluster — get the AWS WAF
blob-URL PoW Web Worker self-solve to advance in the **live** navigate path — not
a React/MessageChannel/IntersectionObserver fix. Earlier docs that filed this as
"SPA bootstrap fetch chain fails mid-hydration" are superseded by direct capture.

---

## 1. What the existing repo docs concluded (and where they were wrong)

| Doc | Claim about booking | Verdict now |
|---|---|---|
| `releases/v0.1.0-parity/02_GAP_ANALYSIS.md:114-124` | "heavy server-side React app… first response is an SPA bootstrap that needs to fetch `/api/...` to hydrate. Something in that fetch chain fails." | **WRONG diagnosis.** No `/api` hydration is gated; the shell is an AWS WAF challenge page. |
| `releases/v0.1.0-parity/05_SPA_HYDRATION_CLUSTER.md:464-543` (§3) | Lists booking under the "SPA hydration cluster"; H1 IntersectionObserver, H2 missing fetch header, H3 drain > 15 s, H4 missing `window` global; proposes a multi-signal mount-heuristic (§5.1-5.2). | **Mostly off-target.** booking ships no recognized SPA mount (see §3), so the mount-heuristic is moot. H3 (drain too short) is the *closest* — but the real reason the drain doesn't help is the AWS async self-solve never advances, not React hydration. |
| `releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md:30,75-92` | booking in Cluster A "SPA hydration"; files **R-SPA-BOOKING-FETCH-CHAIN** (capture `fetches.json` diff). | The diff would have shown the answer immediately: there is no missing app fetch, there is an unsolved AWS WAF token. |
| `docs/vNext/11_R-AWSWAF-FIX-J-deep.md` | **Already reclassified booking correctly:** "The same fix likely improves booking.com (also AWS WAF self-hosted per audit/16 §R-SPA-BOOKING-FETCH-CHAIN reclassification)." | **CORRECT.** This doc confirms that reclassification with a fresh live capture and ties it to the live-nav drain. |
| `docs/HANDOFF_2026_05_28b.md` §4 / §5.4 | AWS cluster root cause = challenge.js's blob-URL PoW Web Worker self-solve runs in the offline oracle but produces **zero async progress** in the live navigate path (50 ms inter-script drain at `page.rs:~3566` vs the oracle's 5 s idle drain). §5.4: "booking — likely the same live-nav drain class — re-test after §5.1." | **CONFIRMED root cause for booking.** |
| `releases/v0.1.0-parity/06_AWS_WAF_SOLVER.md` | The canonical AWS WAF protocol writeup: stub → `challenge.js` → `AwsWafIntegration.checkForceRefresh()/getToken()/forceRefreshToken()` → `aws-waf-token` cookie → reload. `getToken()` "waits up to 2 s". Solver tracks belong in **private `vendor_solvers`**, public engine only gets generic primitives (drain caps, CSP relax). | Directly applies; booking is one more tenant of the exact same protocol. |

**Net:** the repo already contains the correct answer (vNext/11 + HANDOFF §4),
but the original site-cluster docs (02/05/FAILED_SITES) still file booking as SPA
hydration. **This document supersedes that classification.** Treat booking as an
AWS WAF cluster member, sharing the imdb/amazon fix path.

---

## 2. New findings — direct live capture (2026-05-28)

I fetched the live homepage and challenge bundle through the faithful TLS path
(curl with Chrome 148 UA returns the *same* shell BO/`sweep_metrics` sees —
proving this is a deterministic per-request gate, not an IP-roll).

### 2.1 The 8.4 KB body IS an AWS WAF challenge page

`curl -sL https://www.booking.com/ -A "<Chrome 148 UA>"` → **8410 bytes**, with
`<title></title>` (empty). This matches BO's 8473-byte shell exactly. The shell
contains, in document order:

1. A `getAjaxObject()` XHR helper (IE-era cruft, 4167 B inline).
2. `window.awsWafCookieDomainList = ['booking.com'];` (48 B inline) — **the AWS
   WAF integration marker.**
3. `<script src="https://www.booking.com/__challenge_<id>/<id>/<id>/challenge.js"
   nonce="…">` — the AWS WAF challenge bundle, served from booking's own origin
   under a `__challenge_*` path (self-hosted integration, so there is **no**
   `x-amzn-waf-action` response header and **no** `*.token.awswaf.com` host — the
   token endpoints live under booking.com itself).
4. The inline bootstrap that drives the solve (1775 B), reproduced verbatim:

```js
setTimeout(() => {                       // 20 s watchdog
    try { reportChallengeError("Reloaded without updating url", new Error("")); } catch (e) {}
    document.location.reload();
}, 20000);
// … builds newHref with chal_t + force_referer params …
AwsWafIntegration.saveReferrer();
AwsWafIntegration.checkForceRefresh().then((forceRefresh) => {
    if (forceRefresh) {
        AwsWafIntegration.forceRefreshToken().then(() => { window.location.href = newHref; });
    } else {
        AwsWafIntegration.getToken().then(() => { window.location.href = newHref; });
    }
});
```

This is **the exact `AwsWafIntegration.checkForceRefresh().then(...)` chain** that
HANDOFF_2026_05_28b §4 localized for imdb/amazon. The real homepage (465–513 KB)
is only served on the `window.location.href = newHref` reload **after** the
`aws-waf-token` cookie is set.

### 2.2 challenge.js is byte-identical in class to imdb's

`curl` of booking's `challenge.js` → **1,370,019 bytes**. imdb's challenge.js was
**1,370,004 bytes** (HANDOFF §4). Marker scan of booking's bundle:

```
2  new Blob
2  new Worker
7  postMessage
0  WebAssembly.*        (confirmed absent — same as imdb)
```

So booking runs the **identical mechanism**: the PoW executes inside a **blob-URL
Web Worker** (`new Blob([...]) → createObjectURL → new Worker(blobUrl)`), not in
WASM, communicating over `postMessage`. The corpus-wide AWS challenge.js is one
shared artifact; booking is just another tenant.

### 2.3 No SPA mount → the SPA-fast-exit is NOT the cause

The only structural container in the shell is `<div id="challenge-container">`.
There is **no** `#react-root` / `#__next` / `#app` / `#root` / `[data-reactroot]`.
Therefore the SPA-fast-exit at `crates/browser/src/page.rs:2134-2158` (which
returns early when a known mount has ≥1 child) **never trips** for booking — it
finds `mount_populated == 0`. booking instead burns the full nav budget
(observed 15 s in FAILED_SITES) and returns the challenge stub. This **rules out**
the 05_SPA_HYDRATION_CLUSTER §5 hypothesis that an over-eager mount-heuristic
returns the shell prematurely. The body is small because the AWS self-solve never
completes, full stop.

### 2.4 External validation of the mechanism

- AWS WAF Challenge action runs a **client-side proof-of-work**; success yields
  an encrypted, tamper-proof `aws-waf-token` cookie that gates subsequent
  requests. Challenge flavours: **HashcashScrypt, SHA256, NetworkBandwidth**.
  ([AWS: Protect against bots with Challenge and CAPTCHA actions](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/),
  [AWS: intelligent threat JS API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html))
- `getToken()` waits up to ~2 s for the token-acquisition workflow before timing
  out ([getToken doc](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html)).
  → the **2 s ceiling is fine if the workflow advances at all** — the BO problem
  is that in the live nav path it produces *zero* async progress, so the worker
  never even spawns within the 50 ms inter-script drain window.
- Community solvers (Node) confirm the worker-based PoW + token POST shape
  ([neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver),
  [Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver),
  [DEV: How to Solve AWS WAF Challenges with Node.js](https://dev.to/ren_joyce_cd41204d5cb261f/how-to-solve-aws-waf-challenges-with-nodejs-2obe)).
  These belong conceptually in `vendor_solvers`, not the public engine.

---

## 3. BO code-level analysis — why the live nav path stalls

### 3.1 The execution model mismatch (the root cause)

`build_page_with_scripts_init_and_storage` (`crates/browser/src/page.rs:3053`)
prefetches every external script found in the **initial** DOM
(`script_runner::find_scripts(&dom)` at `page.rs:3073`; parallel prefetch loop at
`page.rs:3109-3247`) and then executes them in document order:

```
crates/browser/src/page.rs:3511   for (i, script) in scripts.iter().enumerate() { … }
crates/browser/src/page.rs:3540       event_loop.execute_script_with_name(&code, &name)
crates/browser/src/page.rs:3566       let _ = event_loop.run_until_idle(Duration::from_millis(50)).await;  // ← 50 ms
```

booking's challenge.js (1.37 MB) is executed at `page.rs:3540`. challenge.js's
top-level code installs `AwsWafIntegration`, and the *inline* bootstrap (§2.1)
calls `checkForceRefresh().then(...)`. That promise chain's continuation does:

1. build the blob (`new Blob([...])`),
2. `URL.createObjectURL(blob)`,
3. `new Worker(blobUrl)` — spawns the PoW worker
   (`crates/js_runtime/src/extensions/worker_ext.rs:214-358`, a 64 MB-stack OS
   thread + child `JsRuntime`),
4. `postMessage` the puzzle in, await the solved nonce back,
5. POST the token, set `aws-waf-token`, then `window.location.href = newHref`.

Each of steps 1-5 is **async** (promise + worker-thread round trips + a network
fetch). The **50 ms** `run_until_idle` at `page.rs:3566` between scripts, plus the
**8 s** build-phase drain at `page.rs:3643`, do not reliably carry that full chain
to completion — the worker thread has to start, the child runtime has to boot, the
puzzle has to be solved, and a token POST has to round-trip. By contrast the
**offline oracle** (`aws_capture` → `awswaf_probe`, which does
`from_html_with_url` + a single `run_until_idle(5s)`) runs it all the way to
`forceRefreshToken` (HANDOFF §4, layer 1). **Same JS, same fingerprint, different
drain model → different outcome.** That is the whole bug.

### 3.2 What is NOT the problem (verified)

- **Fingerprint / stealth:** AWS challenge.js *proceeds* with BO's fingerprint —
  it reads `navigator.plugins`, `webdriver`, WebGL unmasked vendor/renderer,
  `chrome.csi/loadTimes`, `performance.now`, and still calls the token function
  (HANDOFF §4 layer 1). FIX-D2 (WebGL1/2 split) measurably did **not** move
  imdb/amazon-in. booking will behave the same.
- **WebAssembly:** absent in challenge.js (§2.2). No WASM gap.
- **`crypto.subtle` in workers:** was undefined in workers (SecureContext-gated);
  **already fixed** this session (commit `5216336`,
  `worker_ext.rs` inherits `is_secure_context`). Prerequisite in place.
- **FileReader.readAsDataURL:** the AWS payload base64 step. Was a no-op stub;
  **already fixed** (FIX-J) — real impl at
  `crates/js_runtime/src/js/shared_apis_bootstrap.js:610-619`. Prerequisite in
  place.
- **IntersectionObserver / MessageChannel / React mount heuristics:** irrelevant
  — booking has no SPA mount (§2.3); IntersectionObserver is already real-rect
  wired (`window_bootstrap.js:3521-3547` → `Element.getBoundingClientRect` at
  `dom_bootstrap.js:716-718` calls the taffy layout op `op_layout_get_bounding_rect`);
  MessageChannel is the duolingo lever, not booking's (booking uses a real
  `Worker`, whose IPC goes through `worker_ext.rs`, not the no-op `MessagePort`
  stub at `window_bootstrap.js:2256-2272`).
- **Dynamic `<script src>` injection:** handled (`_onNodeInserted` at
  `dom_bootstrap.js:142-296`, hooked into `appendChild`/`insertBefore`); not the
  bottleneck here because challenge.js is in the initial HTML and is prefetched.

### 3.3 The cookie-gained re-fetch primitive already exists

Once `aws-waf-token` lands in the jar, BO's existing **cookie-delta retry** in
`navigate_loop_internal` re-issues the URL: the loop snapshots
`cookies_before` (`page.rs:2009`), and the bounded poll / cookie-diff path
(`page.rs:2175-2230`) re-fetches when the jar gains cookies during script
execution. So **we do not need challenge.js to call `location.href` itself** — if
the worker sets the token cookie, the retry returns the 465–513 KB homepage.
**The only missing piece is letting the async self-solve actually run to the point
where the cookie is set.**

---

## 4. Reproduce

```bash
cd /home/yfedoseev/projects/browser_oxide
cargo build --release -p browser --example sweep_metrics

cat > /tmp/just_booking.json <<'JSON'
[{"cat":"travel","name":"booking","url":"https://www.booking.com/"}]
JSON

# Live nav: expect 8 KB shell, NO worker spawn / NO token POST
BROWSER_OXIDE_DEBUG_NAV=1 BROWSER_OXIDE_SC_TRACE=1 \
  RUST_LOG="js_runtime::extensions::worker_ext=debug,js_runtime::extensions::fetch_ext=debug,browser=info" \
  target/release/examples/sweep_metrics chrome_148_macos /tmp/just_booking.json /tmp/booking_out.json \
  2>&1 | tee /tmp/booking.log
jq '.results[] | select(.name=="booking") | {tag, len, ms}' /tmp/booking_out.json
grep -E "challenge\.js|new Worker|worker_id=|aws-waf-token|token|forceRefresh|JS ERROR" /tmp/booking.log

# Offline oracle: expect challenge.js to run to forceRefreshToken
target/release/examples/aws_capture "https://www.booking.com/" /tmp/awswaf/booking_stub.html
# prepend benchmarks/awswaf_probe_inject.js into the stub <head>, then:
target/release/examples/awswaf_probe /tmp/awswaf/booking_instrumented.html "https://www.booking.com/"
```

**Acceptance bar:** `tag == "L3-RENDERED"` AND `len > 30000` (Camoufox got 37 915
in the older sweep; same-IP v150 now renders 465–513 KB). Test all 4 profiles —
booking serves device-specific SSR (iphone was a *different* 3891 B shell), so a
chrome fix may not transfer to iphone (verify the iphone shell is also AWS WAF).

---

## 5. Ranked fix list (ROI order)

> All fixes are **public-engine** generic primitives EXCEPT FIX-B6, which is a
> per-vendor token solver and MUST live in the private `vendor_solvers` crate per
> `CLAUDE.md`. booking shares 100% of its fix path with the AWS WAF cluster, so
> every item here is a cluster-wide lever, not a one-site hack.

### FIX-B1 — AWS-WAF live-nav async drain (THE lever)
**What:** when the initial response is an AWS WAF challenge page (detect by
`window.awsWafCookieDomainList` / `AwsWafIntegration` / `gokuProps` /
`__challenge_` script src / `x-amzn-waf-action` header), give the build-phase
post-script drain enough wall-clock to let the blob-URL PoW worker spawn, solve,
POST the token, and set `aws-waf-token`. Concretely: replace/augment the fixed
50 ms inter-script drain (`page.rs:3566`) and the 8 s build drain (`page.rs:3643`)
with a **bounded busy-wait keyed on async progress** for AWS-WAF pages — poll
every 200 ms up to ~15-20 s for either (a) `aws-waf-token` appearing in the jar,
or (b) `__pendingNavigation` being set — exactly the pattern the challenge poll at
`page.rs:2175-2230` already uses for DataDome/sec-cpt. This is identical to
HANDOFF §5.1 (task #5); booking is a free rider on that fix.
**Effort:** 2-4 days (instrument first per HANDOFF §5.1 step 1, then widen drain).
**Expected impact:** booking + the whole AWS cluster (imdb, amazon-in, +
reliability on fr/jp/com-au) — up to ~7-8 sites. booking should flip if the worker
self-solve completes from this IP.
**Confidence:** medium-high (mechanism proven; residual risk = the self-solve may
still depend on a downstream IP-cluster check per vNext/11 — see FIX-B5).
**Public engine:** YES (generic drain primitive; no vendor bypass code).

### FIX-B2 — Confirm worker self-solve actually runs under the wider drain
**What:** before widening the global drain, prove the worker path completes in the
live nav by trapping `new Worker(blobUrl)` and the worker's first `postMessage` in
`worker_ext.rs` with `RUST_LOG` and checking the offline oracle reaches
`forceRefreshToken` for booking specifically (it does for imdb). If the live
worker spawns but the token POST 4xx's, the gap is downstream (FIX-B5), not drain.
**Effort:** 1 day (diagnostic; gates FIX-B1's design).
**Expected impact:** none directly — de-risks FIX-B1 so we don't widen the drain
for nothing.
**Confidence:** high (pure measurement).
**Public engine:** YES (diagnostics).

### FIX-B3 — Treat AWS-WAF token-cookie gain as a hard "solved" signal in the retry loop
**What:** ensure the cookie-delta retry (`page.rs:2009` snapshot +
`page.rs:2175-2230` poll) explicitly recognizes `aws-waf-token` (and booking's
self-hosted equivalent) as a solve signal and re-issues the original URL even if
challenge.js never calls `location.href`. Today the cookie-diff retry is gated on
`is_anti_bot_challenge() || started_as_dd_challenge || …`
(`page.rs:2175-2179`) — add an `started_as_awswaf_challenge` flag set from the
§5-detect markers so booking (which has no `x-amzn-waf-action` header) enters the
poll branch.
**Effort:** 1-2 days.
**Expected impact:** booking + AWS cluster reliability (closes the "worker solved
but loop returned the stub" race). Likely required alongside FIX-B1.
**Confidence:** medium-high.
**Public engine:** YES.

### FIX-B4 — Reclassify booking in the corpus/docs as AWS-WAF, retire R-SPA-BOOKING-FETCH-CHAIN
**What:** move booking out of the SPA-hydration cluster in `02_GAP_ANALYSIS.md`,
`05_SPA_HYDRATION_CLUSTER.md` §3, and `FAILED_SITES_ANALYSIS.md`; point its row at
the AWS WAF chapter (06) + vNext/11. Prevents future contributors burning time on
React/MessageChannel/IntersectionObserver hypotheses that §2-3 disprove.
**Effort:** 1-2 hours (docs only).
**Expected impact:** 0 sites directly; saves days of misdirected effort.
**Confidence:** high.
**Public engine:** YES (docs).

### FIX-B5 — Downstream IP-cluster / per-region rate handling (final mile)
**What:** per vNext/11, after the bailout chain is unblocked, AWS still flips
*one site at a time per trial* (IP-clustering / per-region WAF rate-limit). booking
may need the same per-region IP rotation / behavioural pacing as the amazon
cluster to be reliable rather than 1/N. This is the difference between "booking can
pass" and "booking passes 5/5".
**Effort:** 1-2 weeks (cluster-wide; see vNext/11).
**Expected impact:** turns booking + AWS cluster from flaky into consistent.
**Confidence:** medium.
**Public engine:** YES (behavioural/IP primitives) — but the *token math* belongs
in FIX-B6.

### FIX-B6 — Direct AWS-WAF token computation (Rust-side solver) — vendor_solvers ONLY
**What:** if the in-VM worker self-solve proves too slow/fragile under the drain,
compute the HashcashScrypt/SHA256 PoW and POST the token from Rust, registered via
the existing `ChallengeSolver` trait (`crates/browser/src/challenge.rs:103`,
`Page::navigate_with_solvers`). This is a per-vendor bypass and is **forbidden in
public crates** by `CLAUDE.md` — implement in the private `vendor_solvers` crate.
booking is just another tenant of the same protocol, so one solver covers the
whole cluster.
**Effort:** 1-2 weeks (reverse-engineering the current obfuscated bundle;
high-maintenance — AWS rotates obfuscation).
**Expected impact:** booking + entire AWS cluster, deterministically.
**Confidence:** medium (closed-source, rotates).
**Public engine:** NO — `vendor_solvers` only.

---

## 6. Definition of done

- Live booking nav (chrome_148_macos) shows: challenge.js fetched → blob-URL
  worker spawns (`worker_id=` in trace) → token POST → `aws-waf-token` set →
  cookie-gained re-fetch returns the real homepage.
- `jq '.results[] | select(.name=="booking") | .len > 30000'` prints `true` on
  chrome (and ideally pixel/firefox; verify iphone shell is AWS WAF too).
- `run_delta_headtohead.py` shows booking flipping (BO ≥1/5, targeting v150's 5/5).
- No regression on the 110+ passing sites (full 4-profile sweep vs the
  2026-05-24 baseline).

---

## 7. Files referenced

| File:line | What |
|---|---|
| `crates/browser/src/page.rs:3053` | `build_page_with_scripts_init_and_storage` |
| `crates/browser/src/page.rs:3073, 3109-3247` | initial-DOM script discovery + parallel prefetch |
| `crates/browser/src/page.rs:3511-3567` | document-order script execution + **50 ms inter-script drain (3566)** ← FIX-B1 target |
| `crates/browser/src/page.rs:3643` | 8 s build-phase `run_until_idle` ← FIX-B1 target |
| `crates/browser/src/page.rs:2009, 2175-2230` | cookie-before snapshot + bounded challenge poll / cookie-diff retry ← FIX-B3 |
| `crates/browser/src/page.rs:2134-2158` | SPA-fast-exit (does NOT trip for booking — no mount) |
| `crates/browser/src/page.rs:1172-1174` | `x-amzn-waf-action` vendor-detect (absent for self-hosted booking) |
| `crates/browser/src/page.rs:2521-2522` | existing `AwsWafIntegration`/`gokuProps` body markers |
| `crates/browser/src/classify.rs:47, 180-181` | 15 KB thin-shell gate |
| `crates/js_runtime/src/extensions/worker_ext.rs:214-358` | `op_worker_spawn` (blob-URL PoW worker) |
| `crates/js_runtime/src/js/shared_apis_bootstrap.js:593-646` | real `FileReader` (FIX-J) |
| `crates/js_runtime/src/js/window_bootstrap.js:3521-3547` | IntersectionObserver (real rects — not the issue) |
| `crates/js_runtime/src/js/dom_bootstrap.js:142-296` | `_onNodeInserted` dynamic script handling |
| `crates/browser/src/challenge.rs:103` | `ChallengeSolver` trait (FIX-B6 / vendor_solvers) |
| `crates/browser/examples/aws_capture.rs`, `awswaf_probe.rs` | offline oracle |
| `benchmarks/run_delta_headtohead.py` | dev-loop delta harness |

**Sources (external):**
[AWS WAF Challenge & CAPTCHA blog](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/) ·
[AWS intelligent threat JS API](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api.html) ·
[AWS getToken (2 s timeout)](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html) ·
[AWS token domains / config](https://docs.aws.amazon.com/waf/latest/developerguide/web-acl-captcha-challenge-token-domains.html) ·
[neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) ·
[Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver) ·
[DEV: Solve AWS WAF with Node.js](https://dev.to/ren_joyce_cd41204d5cb261f/how-to-solve-aws-waf-challenges-with-nodejs-2obe)
