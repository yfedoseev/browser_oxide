# 17 — Delta head-to-head: BO vs Camoufox v150, SAME IP (2026-05-28)

**Method:** `benchmarks/run_delta_headtohead.py 3` — the 12 contested sites
(11 Stratum-A + homedepot) only, BO's 4 gate profiles vs Camoufox **v150.0.2**,
same datacenter IP, same session, 3 trials, per-site browser isolation on the
Camoufox arm. Shared `classify_stdin` classifier for both (PASS = `L3-RENDERED`
∧ `len≥15000`). Replaces the 10h full gate as the *development* feedback loop;
the full gate is retained only for release certification.

**Why this beats the isolated 10h gate:** it controls for IP reputation + AWS
probabilistic token-rolling by running both engines against each site within
minutes of each other from the same IP. A per-site difference now isolates
*engine* quality. Runs in ~75 min, not 10h, so we take 3 trials to expose
variance.

## Headline: on the contested 12, **BO = 5, v150 = 8.** We are behind by 3.

The gap is concentrated entirely in the **AWS-WAF cluster + booking + imdb**.

| site | BO routed (best body / 3 trials) | v150 (3 trials) | verdict |
|---|---|---|---|
| imdb | **0/3** — stuck at 1995 B AWS stub | **3/3** — 16 k | **HARD ENGINE GAP** (deterministic) |
| booking | **0/3** — stuck at 8 k SPA shell | **3/3** — 21 k | **HARD ENGINE GAP** (deterministic) |
| amazon-in | **0/3** — stuck at 2 k stub | **3/3** — 32–696 k | **HARD ENGINE GAP** (deterministic) |
| amazon-fr | 1/3 — 819 k when it lands | 3/3 — 32–183 k | RELIABILITY gap (BO *can*, flaky) |
| amazon-jp | 2/3 — 828 k | 3/3 — 692–792 k | RELIABILITY gap (BO mostly works) |
| amazon-com-au | 1/3 — 928 k | 2/3 — 920–950 k | RELIABILITY gap (both flaky) |
| amazon-com | 1/3 — 1015 k | 1/3 — 1004 k | **IP/PROBABILISTIC** — equal, don't chase |
| amazon-ca | 0/3 — 2–5 k | 0/3 — 5 k (+1 ERR) | **IP/PROBABILISTIC** — v150 also fails, don't chase |
| x-com | **3/3** — 294 k | 3/3 — 378 k | **SOLVED / parity** (Sprint 2.3 holds) |
| homedepot | 0/3 — Akamai-CHL 2 k | 0/3 — Akamai-CHL 2 k | both fail — see correction §2 |
| douyin | 0/3 — 6 k | ERR×3 (v150 driver crash) | **INCONCLUSIVE** — v150 unmeasurable here |
| duolingo | 0/3 — 13 k | ERR×3 (v150 driver crash) | **INCONCLUSIVE** — v150 unmeasurable here |

## §1 — The efficient fix list (what the data says to do)

**Tier 1 — hard engine gaps, v150 PROVES they're solvable, BO deterministically 0/3:**
- **imdb** (AWS stub never escaped; v150 reliably squeaks to 16 k)
- **booking** (SPA shell never hydrates past 8 k; v150 reliably 21 k)
- **amazon-in** (AWS stub never escaped)

These three are the highest-confidence targets: BO has a *hard floor* (stuck at
the stub/shell), v150 clears it every time. No IP-noise confound — the
difference is the engine.

**Tier 2 — reliability gaps (BO already cracks them 1–2/3; fix is variance, not a missing primitive):**
- amazon-fr, amazon-jp, amazon-com-au — behavioural enrichment / cookie hygiene /
  retry, NOT a new capability. BO's huge bodies (819 k, 828 k, 928 k) prove the
  engine path works when the probabilistic roll lands.

**Do NOT spend engine effort on:**
- amazon-com (1/3 == 1/3, genuinely probabilistic for both engines)
- amazon-ca (both fail from this IP — IP/geo, not engine)

**No work needed:** x-com (3/3 parity).

## §2 — Two honesty corrections to HANDOFF_2026_05_28

1. **homedepot "flip 994 KB" does NOT reproduce.** The handoff's headline
   Stratum-B win (Sprint 2.4 `is_seccpt_solved`) passed 0/3 here — both BO and
   v150 sit at the Akamai-CHL 2 k interstitial. This is exactly the
   `R-AKAMAI-SECCPT-FLAKE` failure the docs predicted: the sec-cpt bundle is
   daily-rotated and the single-trial smoke caught a lucky solve. **Downgrade
   homedepot from "shipped win" to "flaky — needs rotation hardening."** It is
   NOT a site where we beat v150.

2. **The first delta run "WE WIN 7/12, v150 0/12" was a measurement artifact.**
   Camoufox crashes on douyin and `bench_corpus_v2` only flushes JSON at the
   end → the crash destroyed v150's whole results file → every v150 site scored
   0. Caught and fixed (per-site isolation + explicit NODATA). The corrected
   result is the opposite: v150 is ahead.

3. **douyin + duolingo remain unmeasurable against v150** with this Camoufox
   build — its Firefox driver crashes on both (uncaught page error). Docs claim
   v150 passes them (1 MB / 697 k); we could not reproduce. BO independently
   fails (6 k / 13 k), so they stay BO engine gaps, but the v150 *reference* is
   unavailable here — debug BO-side, don't treat as a clean comparison.

## §3 — What this says about the AWS-WAF root cause (Hypothesis A vs B)

`02_GAP_ANALYSIS.md §7.1` framed the AWS residual as either IP-region-clustering
(A) or a specific JS-surface engine gap (B). This run is **evidence for B on the
deterministic sites**: if it were pure IP, all amazon TLDs would behave alike
from one IP. Instead BO is *hard-stuck at the 2 k stub on amazon-in/imdb* while
clearing amazon-jp 2/3 and amazon-fr 1/3 with full renders. v150 clearing
amazon-in 3/3 from the *same IP* proves the IP is not the blocker there — the
challenge.js fingerprint fork is. amazon-com (1/3==1/3) is the one genuinely
A-flavoured site.

## §3a — FIX-D2 outcome (WebGL1/WebGL2 split) — measured, did NOT flip AWS

Investigated the AWS deterministic bail (imdb/amazon-in stuck at stub on the
macOS arm). A cheap in-VM probe found a **real, severe conflation leak**:
`getContext("webgl")` returned the **WebGL 2** version string + WebGL-2-only
extensions (`EXT_color_buffer_float`) + `WebGLRenderingContext ===
WebGL2RenderingContext`. Implemented FIX-D2 (distinct WebGL1 surface +
`WebGL2RenderingContext` class; `crates/stealth/src/gpu.rs`,
`crates/js_runtime/src/js/canvas_bootstrap.js`, `stealth_ext.rs`). All leaks
verified gone; tests green (webgl_parity 12/12 incl. new guard, gpu.rs guard,
byte/chrome147 parity, stealth lib).

**Measured effect (chrome_148_macos, 5 trials, same-IP vs v150):**

| site | BO pre-FIX-D2 | BO post-FIX-D2 | v150 | verdict |
|---|---|---|---|---|
| imdb | 0/3 (1995 stub) | **0/5 (1995 stub)** | 5/5 | NO FLIP |
| amazon-in | 0/3 (2k stub) | **0/5 (2k stub)** | 4/5 | NO FLIP |
| amazon-fr | 1/3 | 1/5 | 5/5 | unchanged (noise) |
| amazon-jp | 2/3 | 3/5 | 4/5 | unchanged (noise) |
| homedepot | 0/3 | **3/5 (1.16 MB)** | 0/5 | flaky BO win (R-AKAMAI-SECCPT-FLAKE) |
| x-com | 3/3 | 5/5 | 5/5 | parity (no regression) |

**Conclusion:** FIX-D2 is a legitimate correctness fix (a confirmed cross-API
bot tell, permanently eliminated, no regressions — likely matters for
DataDome/creepjs/combined fingerprinting) but is **NOT the AWS bailout signal**.
imdb/amazon-in remain deterministically stuck at the stub. The evidence-gate
result: **escalate to the awswaf_probe oracle** (vNext task, audit §2) to find
the actual fingerprint surface challenge.js keys on (fonts / AudioContext /
hardwareConcurrency / WASM / navigator — not WebGL). Keep FIX-D2.

**Bonus finding:** homedepot is a *genuine* BO-beats-v150 site (v150 0/5), but
flaky at ~60% — the handoff's "flip" was real, the earlier 0/3 was an unlucky
sample. Needs sec-cpt rotation hardening, not a new fix.

## §3b — AWS oracle: root cause narrowed (the bail is NOT fingerprint)

Built a reusable AWS oracle to find the real bailout signal:
- `crates/browser/examples/aws_capture.rs` — fetches the live stub/challenge.js
  via BO's net stack (faithful TLS + nav headers).
- `benchmarks/awswaf_probe_inject.js` — instrumentation that logs every
  fingerprint read (navigator/screen/chrome/webgl/audio/fonts/perf) + traps
  `AwsWafIntegration.getToken/forceRefreshToken` to record the proceed/bail
  decision. (`awswaf_probe.rs` dump now reports `get_token_called`.)

**Findings (imdb, chrome_148_macos):**

1. **The bail is NOT a fingerprint rejection.** Fed the live stub, challenge.js
   ran to completion and **called `forceRefreshToken()`** (the proceed branch;
   `getToken` is the alt branch — this stub was in force-refresh state). It read
   `navigator.plugins` ×24, `navigator.webdriver` ×4, `userAgent`, `doNotTrack`,
   `geolocation`, WebGL UNMASKED_VENDOR/RENDERER + `getSupportedExtensions`,
   `chrome.csi()/loadTimes()`, `performance.now()` ×19 — then proceeded. No
   fingerprint-based bail. (So FIX-D2 / WebGL was never going to flip it.)

2. **challenge.js IS fetched live** (`[seccpt-trace] script fetch OK …challenge.js
   status=200 bytes=1370004`) on every nav iteration.

3. **But in the live navigate path it produces ZERO downstream activity** — no
   token POST, no WASM, no JS errors (13-line debug log) — whereas the offline
   oracle (`from_html_with_url` + `run_until_idle(5s)`) runs it fully. BO then
   re-fetches imdb 3× and gets the same 1991-byte stub every time.

**Conclusion / root cause (narrowed to a worker-realm gap):** the AWS gap is
**not stealth/fingerprint** and **not WebAssembly**. challenge.js's token PoW
runs in a **blob-URL Web Worker** (`grep`: `new Worker` ×2 + `new Blob` ×2,
**zero `WebAssembly.*`**). It does *not* complete in the live navigate path
(no token POST, no `aws-waf-token` cookie, no reload → stub persists every iter).

Probed the worker layer directly:
- BO `WebAssembly.instantiate` works (irrelevant — challenge.js doesn't use it).
- **A basic blob-URL Worker round-trips fine** (`new Worker(createObjectURL(new
  Blob([code])))` + postMessage echo returned 42). So it is NOT a blanket
  blob-worker gap.

⇒ challenge.js's worker needs a **specific worker-realm capability** BO doesn't
provide to finish its PoW (candidates: the worker's own `fetch`, `crypto.subtle`,
`importScripts`, structured-clone of a specific payload, a `navigator`/timing
field inside the worker, or nested worker/messaging). This is the **same class
as the duolingo recaptcha `webworker.js` gap** (`R-DUO-WORKER`, vNext §06/§5.6)
— very likely a shared root cause across AWS WAF (7 sites) + duolingo.

**Next step (precise):** instrument the worker realm — extend
`benchmarks/awswaf_probe_inject.js` to wrap `Worker.prototype.postMessage` +
inject an in-worker access logger (or reuse `crates/js_runtime/tests/worker.rs`)
and run challenge.js's worker to capture which API it calls that BO's worker
realm (`worker_ext.rs`) returns wrong/undefined. Fix that capability →
challenge.js self-solves → re-measure with `run_delta_headtohead.py`. Engine-
addressable; unifies AWS WAF + duolingo.

**Tooling note:** env-gated debug `eprintln`s added to page.rs script-prefetch
(`BROWSER_OXIDE_DEBUG_NAV`), matching the existing `dd_trace`/`sc_trace` pattern.
`BROWSER_OXIDE_SC_TRACE=1` already logs every external-script fetch.

## §3c — Worker realm: secure-context gap found + fixed; deeper blocker pinpointed

Probed BO's worker realm directly (deterministic, `awswaf_probe` + a worker-caps
probe — avoids challenge.js's flaky offline async chain):

**Gap found + FIXED (commit `5216336`):** a Worker is a secure context iff its
owner is (HTML spec), but `create_worker_runtime` defaulted
`StealthState.is_secure_context=false`, so `cleanup_bootstrap.js` stripped
**`crypto.subtle` + `crypto.randomUUID` in EVERY worker**. A worker doing SHA-256
PoW (`crypto.subtle.digest`) threw `Cannot read properties of undefined (reading
digest)`. Fix: `op_worker_spawn` inherits the parent's `is_secure_context` →
`create_worker_runtime` → `new_with_flags`. Verified: worker on a blob:https page
now has `crypto.subtle` (digest 32B) + `randomUUID` + `isSecureContext=true`.
worker.rs tests 3/3, no regression. **Correct prerequisite for any worker-PoW
site (AWS WAF, reCAPTCHA/duolingo).**

**But it did NOT flip imdb or duolingo** (still 1995 / 13327). The worker fix
isn't even reached in the live path, because:

**Deeper blocker (next lever):** challenge.js executes its async self-solve in
the **offline oracle** (`from_html_with_url` + `run_until_idle(5s)` → defines
AwsWafIntegration, calls `forceRefreshToken`) but produces **ZERO async progress
in the live navigate path** — fetched ×3 (200, 1.37 MB) yet **no worker spawn, no
token POST, no fetch** (verbose `worker_ext`/`fetch_ext` trace). So challenge.js's
`checkForceRefresh().then(...)` chain never advances live. The difference is the
execution/drain model: `build_page_with_scripts_init_and_storage` runs external
scripts with a **50 ms inter-script `run_until_idle`** (page.rs:~3535) then hands
to the outer nav loop, vs the oracle's single 5 s idle drain. Hypothesis: the
prefetched-external-script async continuation (challenge.js's promise chain /
its deferred worker creation) is not drained before the nav loop re-fetches the
stub. **Next step:** instrument challenge.js execution in build_page (does it
throw? does checkForceRefresh's promise resolve? how long does the page actually
drain before the iter re-fetches?) — and/or give AWS-WAF-challenge pages a longer
post-script drain so the self-solve completes. This is the remaining
engine-addressable AWS/duolingo lever; the worker secure-context fix is a
prerequisite already in place.

## §4 — Artifacts

- Harness: `benchmarks/run_delta_headtohead.py`
- Raw: `/tmp/delta_headtohead.json` (per-site, per-trial, per-profile bodies)
- Per-trial logs: `/tmp/delta_bo_<profile>_t<N>.log`, `/tmp/delta_camoufox_<site>_t<N>.log`
- Venv (Camoufox v150 launcher, recreated after /tmp wipe): `/tmp/bo-venv`
