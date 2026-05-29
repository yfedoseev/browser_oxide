# SITE: homedepot.com — Akamai sec-cpt reliability (BO ~3/5 → target 5/5)

**Vendor:** Akamai Bot Manager Premier (BMP), **sec-cpt crypto provider** (Strict-Response / high-tier rule).
**Current state (trustworthy, same-IP):** BO ~3/5 (1.16 MB L3-RENDERED on hits) · Camoufox v150 **0/5** · Patchright (real Chromium) passes.
**We already beat v150.** Goal: convert the flaky ~60% self-solve into a defensible reliable 5/5.
**Scope:** **public engine** — no `vendor_solvers` dependency required. The PoW math is cheap and the bundle self-solves in V8; the win is hardening the engine's *drain/timing* model so the bundle's mandatory-wait + verify + reload chain always completes inside the nav budget.
**Ticket:** `docs/vNext/01_R-AKAMAI-SECCPT-FLAKE.md`. **Latest state:** `docs/HANDOFF_2026_05_28b.md` §5.2.

---

## 1. What the repo already concluded

### 1.1 The mechanism is understood and the self-solve already works
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` §2.4 ("Flow B — sec-cpt PoW path") documents the full challenge:
  Akamai returns **HTTP 428** with JSON `{token, timestamp, nonce, difficulty, count, timeout, cpu, verify_url}`; a daily-rotating obfuscated bundle pair (~560 KB + ~425 KB, served as `/Wjv3…` or `/i4ENwVhj7…`) is loaded as `<script>`. The bundle:
  1. parses the JSON;
  2. brute-forces a base-16 float `r` such that the rolling-hash reduction `output = output*256+b mod (difficulty+i)` equals 0 (PoW cost ≈ **5 ms** per answer at difficulty 15000 — *trivial*);
  3. **waits the `chlg_duration` (5–30 s) server-enforced anti-replay window** — *this, not the PoW, is the real time floor*;
  4. POSTs the answer(s) to `verify_url`;
  5. server sets `sec_cpt=<sec>~3~…` and clears the failing `_abck`;
  6. bundle calls `window.location.reload(true)` → second GET succeeds.
- **Key engine conclusion (26 §2.4 line 200):** "public code does NOT need a sec-cpt PoW solver in Rust as long as the bundle is allowed to run to completion." The pre-strip `crates/akamai/src/sec_cpt.rs::solve_crypto` had **zero non-test callers**. The §6 plan deliberately does NOT re-add it.
- This matches the b623d5d history: `memory/state_2026_05_16_phase5_datadome.md` Inc-7 flipped homedepot `Akamai-sec-cpt-CHL → L3-RENDERED` once the engine stopped racing the bundle with the wrong BMP `sensor_data` POST.

### 1.2 Two engine guards already exist and are load-bearing
- `started_as_seccpt_challenge` (page.rs:1857-1858): `html.contains("sec-if-cpt-container") || html.contains("sec-cpt-if")`. Persistent **origin flag** captured from the *initial* response — survives the bundle mutating the DOM, so the poll-loop and cookie-delta retry stay alive (26 §3 lines 326-329; consumed at page.rs:1929-1938, 2176-2179, 2254, 2332-2333, 2358).
- The **BMP-suppression guard** (page.rs:2332-2333): `sub = "sec-cpt"` when `s.name()=="akamai-bmp" && started_as_seccpt_challenge` — preserves the doc-20 anti-pattern fix so the engine does not fire the wrong `sensor_data` POST and clobber the bundle's own self-solve. (Currently a near-no-op because `default_solvers()` is empty, but harmless.)

### 1.3 The Sprint-2.4 solve detector
- `is_seccpt_solved(cookies, body)` (page.rs:242-247): true iff `sec_cpt=` present **AND** value contains `~3~` **AND** body no longer contains `sec-if-cpt-container`/`sec-cpt-if`. Wired at the poll-break (page.rs:2254-2275) and the cookie-delta retry gate (page.rs:2358). Unit-tested (`seccpt_solved_requires_marker_and_clean_body`, page.rs:3851-3874). **This logic is correct** — the externally-confirmed success marker is exactly `~3~` (see §2). It is **not** the flakiness source.

### 1.4 The budget already gives homedepot the Kasada heavy-PoW tier
- page.rs:1938: `homedepot.com → 45_000` ms (comment lines 1929-1937 explicitly note "the b623d5d flip was observed at nav_ms ≈ 119 s, surviving only on budget-extend stacking" and bumps it to the heavy tier "so the flip is deterministic, not budget-luck").
- **The handoff (HANDOFF_2026_05_28b §1) measured this is still flaky 3/5.** So the 45 s budget is necessary but NOT sufficient. The remaining nondeterminism is elsewhere — see §3.

### 1.5 Documented unknowns the repo has NOT closed
From the ticket (`01_R-AKAMAI-SECCPT-FLAKE.md` lines 52-63): "Which specific primitive the bundle needs … probable shapes: cookie-observation hook / fetch-interception / V8-DOM surface change." The repo never ran the oracle (Steps 1-3 of the ticket are unexecuted). **This doc closes that gap below by code-level analysis of the drain/timer model.**

---

## 2. New external findings (2026-05)

The authoritative public reference for the protocol is Hyper Solutions (the BMP-bypass SDK vendor; cited as the canonical source in 26 §refs line 538).

- **`chlg_duration` is a hard, server-enforced wait** — "this wait time is enforced server-side and cannot be bypassed or shortened … **Submitting proof-of-work before this duration elapses will fail.**" ([Hyper Solutions — Handling 428 SEC-CPT](https://docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt)). This confirms the real time floor is a *wait*, not compute. A correct client MUST sleep the full `chlg_duration` before POSTing.
- **Three providers:** `crypto` (PoW + mandatory wait — homedepot's case), `behavioral` (sensor data), `adaptive` (both). Success across all = **`sec_cpt` cookie containing `~3~`** ([Hyper Solutions](https://docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt); [hyper-sdk-py sec_cpt.py](https://github.com/Hyper-Solutions/hyper-sdk-py/blob/master/hyper_sdk/akamai/sec_cpt.py)).
- **verify endpoint:** the **crypto** provider POSTs to the **static** `/_sec/cp_challenge/verify`; only behavioral uses a dynamic per-challenge `verify_url` (Hyper Solutions). Important for BO: the crypto provider's verify URL is *predictable*, so a future deterministic fallback is feasible without parsing `verify_url`.
- **Verify cadence:** a single verify request after the wait; behavioral does "a loop of up to 3 sensor posts with early exit once `sec_cpt` appears" (Hyper Solutions). The crypto path is a single shot — so a missed/early POST is unrecoverable within that challenge and forces a fresh 428 round-trip.
- **PoW algorithm** confirmed identical to 26 §2.4 (`sec + timestamp + nonce + difficulty`, sha256, rolling reduction) — see [hyper-sdk-go akamai package](https://pkg.go.dev/github.com/Hyper-Solutions/hyper-sdk-go/akamai) and [the crypto-challenge gist](https://gist.github.com/justhyped/38e3cc4b36456ddd9e4ecb2875043a08). Cost is negligible; nothing here is a BO compute gap.
- **Daily rotation** is real and affects **both** the bundle path *and the challenge parameters* (`difficulty`, `chlg_duration`, `timeout`). So the wait BO must survive is **different every day** — which is exactly the shape of a ~60% flake when BO's wait-survival is marginal.

**Synthesis:** the success criterion (`~3~`) is fixed and BO already detects it. The *variable* is the mandatory `chlg_duration` wait. Any path in BO that can let the event loop go idle (or cut the budget) before that wait + verify + reload completes will produce a daily-variable pass rate. That is precisely the failure profile we observe.

---

## 3. BO code-level root-cause analysis

The flakiness is a **timing/drain race against the bundle's `chlg_duration` setTimeout**, with three compounding contributors. The most important is new (not in the ticket's "probable shapes" list):

### 3.1 ROOT CAUSE — the `chlg_duration` wait timer is `unref`'d and stops pinning the event loop

`crates/js_runtime/src/js/timer_bootstrap.js:57`:
```js
const UNREF_THRESHOLD_MS = 2000;
const _maybeUnref = _unrefRaw ? (p, ms) => { if (ms >= UNREF_THRESHOLD_MS) _unrefRaw(p); } : () => {};
```
Every `setTimeout(fn, delay)` with `delay ≥ 2000 ms` is **unref'd** (timer_bootstrap.js:62-78) — meaning the underlying `op_timer_sleep` promise no longer keeps `run_until_idle` open. This was tuned for x.com hydration (comment lines 41-56): unref'ing long analytics timers prevents 90 s stalls.

**But the sec-cpt bundle's mandatory `chlg_duration` wait (5–30 s) is implemented as exactly such a `setTimeout`.** When the only outstanding work in the loop is that unref'd ≥2 s timer, every drain — the 8 s build-phase drain (page.rs:3643), the 200 ms poll iterations (page.rs:2185), and the outer drain (page.rs:2055) — can return `AllWorkDone` / reach idle **before the wait fires**, so the deferred verify POST and `location.reload(true)` never run. The bundle's continuation (`then(() => { fetch(verify); ... reload(); })`) is silently dropped because nothing refs it.

Why this is *flaky and not always-fail*:
- The bundle (560 KB + 425 KB) runs other sub-2 s timers (rendering ticks, RUM beacons, retry jitter) that ARE refed and keep the loop busy past `chlg_duration` on *some* days/parameter sets, letting the unref'd wait fire incidentally — exactly the "survived only on budget-extend stacking" observation in page.rs:1933-1934.
- When `chlg_duration` is short (≈5 s) and there is concurrent refed work, it lands → PASS. When `chlg_duration` is long (≈20–30 s, a high-difficulty day) and the bundle goes quiet during the wait, the loop idles out → FAIL. **This is the daily-variable ~60%.**

This is the same *class* of bug as the AWS-WAF live-nav drain (HANDOFF_2026_05_28b §4): the offline oracle (`run_until_idle(5s)` with the page kept alive) lets the async chain complete, but the live navigate path lets the loop go idle too early.

### 3.2 CONTRIBUTOR — the 50 ms inter-script drain starves the bundle's async kickoff

`crates/browser/src/page.rs:3566`:
```rust
// Run loop for a short burst between scripts to flush tasks
let _ = event_loop.run_until_idle(Duration::from_millis(50)).await;
```
Between executing each inline `<script>`, the engine drains only 50 ms. The sec-cpt inline bootstrap that parses the 428 JSON and *registers* the `chlg_duration` timer + PoW promise chain may not get its first microtask turn before the next script runs / the build phase moves on. The 8 s build-phase drain (page.rs:3643) partially compensates, but combined with §3.1 the wait timer is registered-then-abandoned. (This is the exact line HANDOFF_2026_05_28b §7 line 142 flags as the AWS-WAF §5.1 target — same fix benefits both clusters.)

### 3.3 CONTRIBUTOR — `MIN_RETRY_BUDGET` can abort the post-solve re-fetch on long-wait days

`crates/browser/src/page.rs:2420-2429`:
```rust
const MIN_RETRY_BUDGET: Duration = Duration::from_secs(15);
if nav_budget.saturating_sub(nav_t0.elapsed()) < MIN_RETRY_BUDGET {
    eprintln!("[navigate] iter={} skip cookie-delta retry: only {}ms left …");
    return Ok(page);   // returns the challenge stub
}
```
With a 45 s budget and a 30 s `chlg_duration`, the bundle solves at ~T+33 s and sets `sec_cpt=~3~`; the cookie-delta retry then needs ≥15 s of remaining budget to re-fetch real content. On the worst (highest-`chlg_duration`) days, `45 - 33 = 12 s < 15 s` → the engine **returns the challenge stub even though the cookie just flipped to `~3~`**. This is a second daily-variable cliff that aligns with the same long-difficulty days that trigger §3.1.

### 3.4 CONTRIBUTOR (minor) — `is_seccpt_solved` requires a body transition that may lag the cookie

`is_seccpt_solved` (page.rs:242-247) requires BOTH `~3~` in cookies AND the body to no longer contain the challenge markers. The cookie flips on the verify response, but the body only transitions after `location.reload(true)` completes. There is a window where `~3~` is set but `page.content()` is still the challenge page — during which `is_seccpt_solved` returns false at the poll-break (page.rs:2266). This is *correct* (we must not declare solved on the stub), but it means the **cookie-presence** signal (the earliest reliable success marker) is not used to *extend* the budget / *force* the re-fetch. The engine waits for the slower body signal instead of acting on the fast cookie signal.

### 3.5 What is NOT the cause (eliminations)
- **Not a fingerprint gap** (ticket lines 24-25; v150's hardware spoofing didn't move it).
- **Not a PoW/compute gap** (§2; ~5 ms; bundle self-solves in V8).
- **Not CSP blocking the bundle** today — 26 §refs confirms `started_as_seccpt_challenge` + the classifier `/_sec/cp_challenge` UNAMBIGUOUS row keep the bundle loadable; `[seccpt-trace]` (page.rs:3175-3182) already shows the bundle fetching 200 OK on hit days.
- **Not the success-marker detector** (`~3~` is externally confirmed correct, §2).

---

## 4. Ranked fix list (ROI order)

All fixes are **public-engine** (CLAUDE.md compliant — no vendor bypass code; the bundle keeps doing its own PoW). They target the *engine's drain/timing model* so the bundle's existing self-solve always completes.

### FIX-1 — Keep the sec-cpt wait timer refed on challenge navs (root cause)
**What:** when `started_as_seccpt_challenge` (or, generally, when the nav is on a known challenge origin), do NOT `unref` long timers — or raise `UNREF_THRESHOLD_MS` selectively. Concretely, thread a per-page `keep_long_timers_refed` flag into the timer bootstrap (set by the navigate loop when any `started_as_{seccpt,dd,cf}_challenge` is true) and gate timer_bootstrap.js:59 on it: `if (ms >= UNREF_THRESHOLD_MS && !globalThis.__keepLongTimersRefed) _unrefRaw(p);`. This makes the `chlg_duration` 5–30 s wait pin the loop so the deferred verify POST + `location.reload(true)` always fire. Pair with raising the homedepot poll/drain ceiling to comfortably exceed max `chlg_duration` + verify RTT + reload (e.g. the 90 s poll at page.rs:2181 already covers it; the issue is idle-exit, not the ceiling).
**Effort:** 0.5–1 day (flag plumbing through `build_page_with_scripts_init_and_storage` → JS global; the x.com regression risk is contained because the flag is challenge-nav-only).
**Expected impact:** homedepot 3/5 → **5/5**; likely also helps the **AWS-WAF cluster** (same root class per HANDOFF §4) and **booking** (§5.4 "likely the same live-nav drain class"). Primary single-site reward +1, but cross-cluster leverage.
**Confidence:** **high** (mechanism is code-confirmed; the unref threshold directly explains the daily-variable behavior).
**Engine:** public.

### FIX-2 — Replace the 50 ms inter-script drain with a challenge-aware burst
**What:** at page.rs:3566, when the page is a challenge document (cheap check: the `started_as_seccpt_challenge`-equivalent on `html`, or presence of the 428/sec-cpt markers), drain longer (e.g. 1–2 s) so the bundle's bootstrap registers its timer + promise chain before the build phase proceeds. Keep 50 ms for benign pages (it was tuned to avoid pinning on humanize timers).
**Effort:** 0.5 day.
**Expected impact:** removes the §3.2 starvation contributor; reinforces FIX-1. Same AWS-WAF co-benefit (this is the exact line HANDOFF §7 flags for §5.1). On its own, partial; combined with FIX-1, pushes reliability to 5/5.
**Confidence:** **high** (line-level, low blast radius if gated to challenge bodies).
**Engine:** public.

### FIX-3 — Use the `~3~` *cookie* signal to guarantee the post-solve re-fetch (fix the budget cliff)
**What:** (a) at the `MIN_RETRY_BUDGET` guard (page.rs:2420-2429), if `cookies` now contain `sec_cpt=…~3~` (success cookie present even if body hasn't transitioned), do NOT bail — extend the budget by one re-fetch window instead of returning the stub. (b) Add a cookie-only success path: when `sec_cpt=~3~` appears, immediately fire the cookie-delta re-fetch of the original URL rather than waiting for the body-transition condition in `is_seccpt_solved`. This decouples "challenge solved" (cookie) from "real content rendered" (body) and removes the §3.3 + §3.4 cliffs.
**Effort:** 1 day.
**Expected impact:** eliminates the worst-day (high-`chlg_duration`) cliff where BO solves but returns the stub anyway. Converts the residual flake after FIX-1/2 into 0; hardens 5/5.
**Confidence:** **medium-high** (logic is clear; need a test that the early re-fetch doesn't fire on a `~1~`/`~2~` in-progress cookie — the existing `is_seccpt_solved` test scaffold at page.rs:3851 extends naturally).
**Engine:** public.

### FIX-4 — Add the oracle + a deterministic regression test (durability, not a flip)
**What:** execute the ticket's Step 1-2 oracle (`seccpt_probe.rs` forked from `awswaf_probe.rs`): capture a same-day homedepot 428 stub + bundle, replay through `from_html_with_url` with a worker/timer trace, and assert the bundle reaches the verify POST. Land a `#[ignore]`-gated `homedepot_seccpt_self_solve` chrome_compat test that drives the *drain model* (not the network) — e.g. a synthetic stub whose inline script registers a `setTimeout(()=>{document.cookie='sec_cpt=x~3~'; ...}, 6000)` and asserts the navigate loop surfaces `~3~` (proves FIX-1's refed-timer behavior without network).
**Effort:** 1–2 days.
**Expected impact:** no new flip; **locks in** FIX-1/2/3 so the x.com-style unref retuning can't silently regress homedepot again (the regression that created this ticket in the first place).
**Confidence:** high (pure engineering).
**Engine:** public.

### FIX-5 (fallback, only if 1-3 underperform) — deterministic crypto sec-cpt solver in `vendor_solvers`
**What:** if (and only if) the bundle proves un-runnable on some rotation, port the documented crypto-provider solve to the **private** `vendor_solvers` crate: parse the 428 JSON, brute-force the rolling-hash `r`, sleep `chlg_duration`, POST to the static `/_sec/cp_challenge/verify`, then re-fetch. The math is fully public (26 §2.4; [hyper-sdk-go](https://pkg.go.dev/github.com/Hyper-Solutions/hyper-sdk-go/akamai)).
**Effort:** 2–3 days.
**Expected impact:** makes homedepot deterministic independent of the bundle running at all. **But:** 26 §2.4 line 200 found the solver had zero callers because the bundle self-solves — so this is strictly a belt-and-suspenders fallback.
**Confidence:** medium (the algorithm is known; risk is matching the exact verify payload shape per rotation).
**Engine:** **vendor_solvers ONLY** — CLAUDE.md forbids per-vendor PoW solvers in public crates. Flag accordingly; do NOT land in public.

---

## 5. Recommended sequence
1. **FIX-1** (refed challenge-nav timers) — highest ROI, fixes the root cause, cross-cluster.
2. **FIX-2** (challenge-aware inter-script drain) — cheap, reinforces FIX-1, shared AWS-WAF win.
3. Re-measure with `benchmarks/run_delta_headtohead.py 5` on homedepot; if any residual flake, add **FIX-3** (cookie-signal budget hardening).
4. **FIX-4** (oracle + regression test) to lock it in before the v0.2.0 gate.
5. **FIX-5** only if 1-4 cannot reach 5/5 on a rotation — and only in `vendor_solvers`.

**Definition of done:** `run_delta_headtohead.py` shows homedepot **5/5** across ≥2 different days (proving daily-rotation resilience), `webgl_parity`/`chrome147` parity stay green, and the new `#[ignore]` drain test passes.

---

## 6. Sources
- `docs/vNext/01_R-AKAMAI-SECCPT-FLAKE.md` — ticket, unknowns, oracle methodology
- `docs/HANDOFF_2026_05_28b.md` §1, §4, §5.2, §7 — flaky-3/5 measurement, live-nav drain class, target line
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` §2.4, §3, §refs — Flow B mechanism, primitives, Hyper Solutions reference
- `~/.claude/.../memory/state_2026_05_16_phase5_datadome.md` — b623d5d Inc-7 historical flip
- `crates/browser/src/page.rs` — 242-247 (`is_seccpt_solved`), 1857-1858 (`started_as_seccpt_challenge`), 1938 (45 s budget), 2055/2181-2186 (drains/poll), 2254-2275 (poll-break), 2332-2333 (BMP guard), 2420-2429 (`MIN_RETRY_BUDGET`), 3566 (50 ms inter-script drain), 3643 (8 s build drain), 3851-3874 (unit test)
- `crates/js_runtime/src/js/timer_bootstrap.js:41-79` — `UNREF_THRESHOLD_MS = 2000`, the unref logic (root cause)
- `crates/browser/src/classify.rs:84, 124-135` — `/_sec/cp_challenge` UNAMBIGUOUS row + `AKAMAI_CHALLENGE_COSIGNAL`
- [Hyper Solutions — Handling 428 SEC-CPT](https://docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt) — `chlg_duration` server-enforced wait, `~3~` success, crypto vs behavioral vs adaptive, verify endpoints
- [hyper-sdk-py sec_cpt.py](https://github.com/Hyper-Solutions/hyper-sdk-py/blob/master/hyper_sdk/akamai/sec_cpt.py) · [hyper-sdk-go akamai](https://pkg.go.dev/github.com/Hyper-Solutions/hyper-sdk-go/akamai) · [crypto-challenge gist](https://gist.github.com/justhyped/38e3cc4b36456ddd9e4ecb2875043a08) — PoW algorithm confirmation
