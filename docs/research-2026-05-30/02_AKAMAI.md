# 02 — Akamai cluster: why BO fails homedepot (sec-cpt) and adidas/bestbuy where v150 passes

**Date:** 2026-05-30 · **Branch:** `fix/v0.1.0-fix4-canvas-parity` ·
**Cluster:** Akamai Bot Manager Premier (BMP v3) — `homedepot` (sec-cpt
crypto-PoW interstitial), `adidas` (plain `sensor_data` BMP), `bestbuy`
(i18n splash mis-attributed as Akamai).
**Ground truth:** v150 (real Firefox 146) renders homedepot 986 KB,
adidas 1.52 MB; BO returns `Akamai-CHL 2.7 KB` (homedepot sec-cpt) /
2.5 KB (adidas shell).

Tags: **[CODE]** = file:line I read this session · **[MECH]** =
externally-cited / internally-documented mechanism · **[HYP]** =
labelled hypothesis.

---

## 0. TL;DR — the three sites are three different problems

| Site | Vendor sub-kind | Real root cause | Engine-flippable? |
|---|---|---|---|
| **homedepot** | Akamai **sec-cpt `crypto`** PoW interstitial | Infra is correct; the flip is **non-deterministic budget-luck**. The post-solve reload is gated `iter+1 < iterations` AND the documented flip needed `nav_ms≈119 s` of *stacked* budget bumps the base 45 s budget does not deliver. The server-enforced `chlg_duration` (5–30 s) wait is not explicit in the timing math. | **YES** — make the budget explicit + arm the pending-nav bump on the sec-cpt early-break, not only on a JS reload signal. |
| **adidas** | Akamai **plain `sensor_data` BMP** | **Holistic-ML `_abck~-1~` verdict.** The sensor POST is *processed* (HTTP 201, `_abck` encrypted-state grows) but the trust slot never advances to `~0~`. No single missing field — audio/worker/canvas/humanize all tried, all moved bytes, none flipped the slot. | **NO single lever** — this is the holistic ML tail. Out of cheap scope. |
| **bestbuy** | NOT Akamai | The 7.3 KB body is the **"Choose a country" i18n splash** (`<title>Best Buy International…</title>`, zero `_abck`/`sec_cpt`/`sensor_data`). **Real Chrome from this IP gets the identical splash.** | **YES, trivially** — classifier mis-attribution + an `?intl=nosplash` / cookie follow. Not a PoW or fingerprint problem at all. |

**Strategic read:** the *named* cluster sites collapse to **one
addressable engine problem (homedepot timing determinism)**, one
**out-of-cheap-scope ML problem (adidas)**, and one **classifier-FP /
geo-cookie problem (bestbuy)**. The historical "Akamai edge H2
fingerprint" root cause (GAP_DEEP_ANALYSIS 04-28) **is already fixed in
code** (§2) — homedepot now gets *past* the edge to a sec-cpt
interstitial, which is itself proof the wire is no longer the gate.

---

## 1. The historical edge-fingerprint root cause is RESOLVED in code

`GAP_DEEP_ANALYSIS_2026_04_28.md` named the Akamai gap as an HTTP/2
`WINDOW_UPDATE` off by 65,535 (`15_728_640` vs Chrome's `15_663_105`) +
priority weight 255-vs-256 — landing BO in the "untrusted browser"
`_abck~-1~` bucket *at the edge*, before any sensor JS runs.

**That is now byte-correct** [CODE]:
- `crates/net/src/h2_client.rs:11,28` — the documented Akamai H2
  fingerprint string is `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`.
- `:43-50` — `INITIAL_CONNECTION_WINDOW_SIZE = 15_728_640` is the
  *configured target*; the http2 lib emits `target − 65535 = 15_663_105`
  on the wire = Chrome match.
- `:167-174` — `headers_stream_dependency(StreamId::zero(), 255, true)` =
  Chrome 147's weight 256 (wire byte 255), exclusive, depends_on=0,
  verified vs the 2026-05-09 tls.peet.ws capture.
- SETTINGS order `:118-130` is Chrome's canonical 8-entry order; pseudo
  order `masp` `:97-104`.

**Decisive corroboration:** homedepot now returns a **sec-cpt
interstitial** (`<div id="sec-if-cpt-container">`), not a hard
`_abck~-1~` 403. Akamai only serves sec-cpt to sessions it has *already
edge-trusted enough to challenge in-band*. If the edge H2 fingerprint
were still wrong, BO would get the `~-1~` Access-Denied path (what adidas
gets), not a solvable PoW page. **The wire is no longer the homedepot
gate.** [HYP-strong, consistent with §0 + ANTIBOT_RESEARCH §"Akamai BMP"
tiered model: edge-fp → sensor-JS → sec-cpt].

---

## 2. homedepot — the real current root cause: non-deterministic PoW timing

### 2.1 What sec-cpt `crypto` actually requires [MECH]
homedepot serves the **`crypto`** sec-cpt provider — a headless
background JS proof-of-work, *no* user interaction (the tile-puzzle is
the *different* `behavioral`/Content-Protector provider). Per
hyper-sdk-go/akamai + docs.hypersolutions.co, the minimum successful
crypto sequence after the 428/interstitial is:

```
1. GET target            → interstitial, sets sec_cpt cookie (slot ≠ ~3~)
2. WAIT chlg_duration s   → SERVER-ENFORCED, un-skippable (5–30 s)
3. POST /_sec/verify?provider=crypto   → submit PoW answers
4. GET  /_sec/cp_challenge/verify       → {"success":true}; sec_cpt → ~3~
5. GET target again       → real content (carries sec_cpt~3~)
```

In BO's model the **obfuscated bundle does steps 2–4 itself in V8**
(`crypto.subtle.digest("SHA-256")` is a *real* sha2-backed op —
`crates/js_runtime/src/extensions/crypto_ext.rs:11-39`,
`shared_apis_bootstrap.js:112-118` [CODE], so the PoW math runs), then
triggers step 5 (a reload). The engine must (a) keep the page alive
through the PoW **and** the server-enforced `chlg_duration` sleep, then
(b) perform the post-solve reload carrying `sec_cpt~3~`, across **≥2 nav
iterations** (the reload is a fresh top-level GET).

### 2.2 Every prior fix IS landed — infra is correct [CODE]
This session confirms all four named fixes from the 05-16 / Sprint-2.4
plans are in `crates/browser/src/page.rs`:

1. **doc-20 BMP-suppression** — when `started_as_seccpt_challenge`, the
   wrong plain-BMP `sensor_data` POST is suppressed so the bundle is sole
   actor (the b623d5d win). `started_as_seccpt_challenge =
   html.contains("sec-if-cpt-container") || "sec-cpt-if"` `:1898`.
2. **`is_seccpt_solved` early-break** — `:242-247` + poll break
   `:2324-2346`: breaks the 90 s poll the instant
   `sec_cpt=` + `~3~` lands and the body no longer shows
   `sec-if-cpt-container`/`sec-cpt-if`.
3. **45 s host budget** — `:1993` `Some(h) if h.ends_with("homedepot.com")
   => 45_000` (Kasada heavy-PoW tier).
4. **M-3 keep-long-timers-refed** — `:3612-3618` sets
   `globalThis.__keepLongTimersRefed = true` before page scripts run when
   the doc is a sec-cpt challenge, so the `chlg_duration` `setTimeout`
   is NOT unref'd at `UNREF_THRESHOLD_MS=2000`
   (`timer_bootstrap.js:69`) — without this the drain hands off before
   the reload fires.

### 2.3 Why it still fails: budget arithmetic is implicit + brittle [CODE]
The b623d5d flip was observed at **`nav_ms≈119 s`** — far over the
nominal 45 s budget. It survived only by *stacking* budget bumps:

- The **`+25 s` cookie-delta extend** (`:2169-2171`) fires **only** for a
  `body_len > 50 KB` no-CHL iter-0 page. A sec-cpt 2.7 KB interstitial
  **never** triggers it [CODE].
- The **`+45 s` pending-nav bump** (`:2750-2752`) fires only when a JS
  **pending-nav** (`PENDING_NAV_JS`, i.e. the bundle calls
  `location.reload()`/assign) is observed AND `< 15 s` budget remains.
- The 90 s poll (`:2251`) pumps `run_until_idle(200 ms)`, but each new
  iteration is gated by the top-of-loop wall-clock check
  `if nav_t0.elapsed() >= nav_budget { break }` `:2039`.

So for homedepot the *effective* budget is **45 s + (one `+45 s` bump
IFF the bundle emits a JS reload signal the poll catches)**. The flip
needs ≈119 s. If the bundle clears `~3~` via the cookie alone (early-break
at `:2336`) **without** also setting a JS pending-nav, the poll `break`s
into the cookie-delta retry — but that retry's `MIN_RETRY_BUDGET` guard
(`:2515-2523`) **returns the current 2.7 KB page** if `< 15 s` remain,
and the `+45 s` pending-nav bump at `:2752` is in the *pending-nav*
branch, NOT reachable from the sec-cpt cookie-early-break path. **The
sec-cpt success path does not arm the long budget it actually needs.**
← **root-cause gap G-SC-1**.

Net: the flip depends on whichever non-deterministic branch the
obfuscated bundle happens to take this run (cookie-only vs JS-reload),
and on the server's `chlg_duration` value this rotation. ANTIBOT
homedepot is documented **stochastic near the borderline** (one 1-MB
pass in 5 runs) — consistent with a flip that survives only on budget
luck. The 05-16 UNBLOCK doc's own §2.3 flagged this as "implicit and
brittle"; the budget was raised to 45 s but the **success-path bump was
never wired**, so it remains brittle.

### 2.4 Secondary risk: the post-solve reload is hard-gated on iterations
The cookie-delta retry / pending-nav reload needs `iter + 1 < iterations`
(`:2736`). `holistic_sweep` calls `Page::navigate(url, profile, 3)`
(`holistic_sweep.rs:52` [CODE]) so 3 iters is fine — but any harness that
calls with `iterations==1` (the old audit lens) **cannot** flip sec-cpt
by construction. The ground-truth delta harness must use ≥3 iters for
homedepot to be measurable at all. [HYP: if the new ground-truth run used
a 1-iter or tight-budget path, homedepot's "fail" is partly a measurement
artifact, not purely an engine gap — verify the harness iter count.]

---

## 3. adidas — holistic-ML `_abck~-1~`, no single lever (out of cheap scope)

adidas is **plain `sensor_data` BMP**, not sec-cpt. The
`adidas_akamai_bmp_v3.md` probe (same engine) is unambiguous:

- The sensor POST is **processed**: every POST → HTTP 201, cookies grow
  to 4593+ chars (`_abck`, `bm_sz`, `bm_ss`, `ak_bmsc`, `AKA_A2`), and
  the `_abck` encrypted-state blob (`AAQAAAAF…`) **grows per POST** —
  the server is digesting our payload. But the trust slot stays `~-1~`
  across all three positions (`~0~` = trusted). The reference adidas
  capture (`akamai_sensor_analysis/scratch/adidas-network.json:1`) shows
  the real-Chrome sensor POST `{"sensor_data":"3;0;1;2048;…"}` → **201**,
  same status BO gets — so the *transport* is right; the *content
  verdict* differs.
- **Everything cheap was tried and moved bytes but not the slot:** T1.3
  Blink-audio port (sections 30→40, slot unchanged), T1.5 real Workers
  (sensor VM **never calls `new Worker()`** — probe-verified read=0),
  OffscreenCanvas + navigator class prototypes (±8 bytes noise),
  humanize (event counts plausible-ized, +19 % body, slot unchanged),
  cookie portability (NOT portable — verdict is bound to live in-session
  sensor execution).
- The probe confirms the VM reads **NO WebGL** and **NO Worker** on this
  path — so the canonical "you MUST have a real GPU string" Akamai signal
  is not even exercised here; the discriminator is in the
  *environmental/behavioral sensor content* (residual fields:
  `performance.memory` suspiciously round, `userAgentData.brands` fixed
  order, audio per-sample bit-accuracy, `navigator.connection.type`).

**Verdict:** adidas is the **holistic ML tail** — same class as the
Kasada "allow-but-blocked paradox" already closed as not-a-single-lever.
No cheap public-engine fix flips it. The only diagnostic that would name
the field is a **synchronized clean-IP real-Chrome sensor capture for the
same rotated VM URL** (blocked: Playwright is WAF-hard-blocked from this
IP; the offline probe can't sync to Akamai's rotating per-request VM).
Recommend **de-scoping adidas from the flip list** — it is not
engine-cheap-addressable and chasing it burns sessions (documented
dead-ends: T1.3/T1.5/canvas/humanize all closed).

---

## 4. bestbuy — NOT Akamai (classifier mis-attribution)

`ab_harness/shots/https_www_bestbuy_com_.html` (7.3 KB, captured by
real Chrome via `nocdp`) is the **"Choose a country" i18n splash**:
`<title>Best Buy International: Select your Country</title>`, escape link
`https://www.bestbuy.com/?intl=nosplash`. **Zero** `_abck` / `bm_sz` /
`sec_cpt` / `sec-if-cpt` / `sensor_data` markers. **Real Chrome from this
datacenter IP gets the identical splash** — it is a geo/cookie gate, not
a challenge, not a fingerprint failure, not an IP ban (a human clicking
"United States" proceeds). The classifier (`classify.rs`) is gated to NOT
over-match this: `akam/13` and `_abck`/`captcha` now require a co-signal
(`small_body_row_qualifies` `:175-186`, `AKAMAI_CHALLENGE_COSIGNAL`
`:138-146` [CODE]), so a bare splash should already classify `THIN-BODY`,
not `Akamai-CHL`. If the ground-truth still labels bestbuy "Akamai-CHL
thin", it is the *splash size gate*, addressable by following
`?intl=nosplash`. bestbuy is **not** a sec-cpt site — do not chase it as
one.

---

## 5. Is it fingerprint, the sec-cpt drain, or behavioral?

| Candidate | Verdict | Evidence |
|---|---|---|
| **Edge TLS/H2 fingerprint** | **NOT the homedepot gate** (resolved). Still the adidas gate's *floor* but not the discriminator (201s come back). | `h2_client.rs:11,167` byte-exact; homedepot reaches in-band sec-cpt = edge already trusted. |
| **sec-cpt drain / nav-budget** | **THE homedepot gate.** Infra correct; flip is budget-luck because the success path doesn't arm the `+45 s` bump and `chlg_duration` is implicit. | `page.rs:2515-2523, 2736, 2750-2752`; b623d5d `nav_ms≈119 s` vs 45 s budget. |
| **Behavioral (sensor content / mouse)** | **THE adidas gate** (holistic ML), but NOT a single cheap lever; humanize already tried, slot unchanged. | `adidas_akamai_bmp_v3.md` §"what we tried" — humanize moved bytes, `_abck~-1~` unchanged. |
| **BMP fingerprint (audio/canvas/webgl)** | Sound by construction; sensor VM doesn't even read WebGL/Worker on this path. NOT the bottleneck. | probe read counts: WebGL=0, Worker=0; T1.3 audio moved bytes only. |

---

## 6. Ranked fixes (public-engine only, per CLAUDE.md — NO vendor_solvers)

### FIX-A1 (homedepot) — arm the sec-cpt budget bump on the cookie early-break · **S · HIGH confidence**
**File:** `crates/browser/src/page.rs` (the sec-cpt early-break at
`:2324-2346`).
**Change:** when `is_seccpt_solved` / `solved_signal` fires in the poll,
bump `nav_budget += Duration::from_secs(45)` (and floor remaining at
`MIN_RETRY_BUDGET`) **before** breaking, so the subsequent cookie-delta
retry (the post-solve reload, step 5) is guaranteed to have build+drain
budget regardless of whether the bundle also emitted a JS pending-nav.
Today the `+45 s` bump (`:2752`) is reachable only from the *pending-nav*
branch, so a cookie-only `~3~` solve falls into the `MIN_RETRY_BUDGET`
early-return (`:2516-2523`) with the 2.7 KB page. This makes the flip
**deterministic on the documented `~3~` signal** instead of luck.
**Expected flips:** homedepot (chrome/pixel/iphone — firefox separately
blocked by §7). Gated entirely behind `started_as_seccpt_challenge` ⇒
zero §4-gate regression by construction.

### FIX-A2 (homedepot) — make `chlg_duration` explicit in the budget · **S · MEDIUM**
**File:** `page.rs:1984-1993`.
**Change:** raise the homedepot/sec-cpt tier to **60 000 ms** (currently
45 000) and add a comment that the budget must cover `build + ~1 MB
bundle + sha256 PoW + server-enforced chlg_duration(5–30 s) + post-solve
reload + that page's drain`. b623d5d needed ≈119 s; 45 s + one `+45 s`
bump = 90 s is marginal. With FIX-A1 the bump is guaranteed, but a 60 s
base removes the dependence on the bump firing in time when
`chlg_duration` is at the high (30 s) end of its rotation.
**Expected flips:** stabilizes homedepot across config rotations.

### FIX-A3 (measurement) — assert ≥3 iterations for sec-cpt in the delta harness · **S · HIGH**
**File:** `benchmarks/run_delta_headtohead.py` (+ any 1-iter caller).
**Change:** the post-solve reload is hard-gated `iter+1 < iterations`
(`page.rs:2736`). Verify the ground-truth harness calls
`Page::navigate(..., 3)` for homedepot — a 1-iter or tight-budget run
*cannot* flip sec-cpt and would report a false "engine gap". If the new
ground-truth used <3 iters, part of homedepot's "fail" is a measurement
artifact.
**Expected flips:** removes a false-fail; required for FIX-A1/A2 to be
measurable.

### FIX-A4 (bestbuy) — `?intl=nosplash` follow / classifier confirm · **S · HIGH**
**File:** `page.rs` (navigate loop, geo-splash follow) + confirm
`classify.rs:175-186` already de-attributes the bare splash.
**Change:** on a `bestbuy.com` body that is the i18n splash (title
contains "Select your Country", zero Akamai markers), re-issue
`https://www.bestbuy.com/?intl=nosplash` (or set the `intl` cookie) to
reach the real US homepage, which is plain-BMP and renders. This is a
geo-cookie follow, not a PoW solve.
**Expected flips:** bestbuy from THIN/Akamai-CHL → rendered. Zero risk
(narrow host + splash-shape gated).

### NOT-FIX (adidas) — de-scope · **N/A**
adidas is the holistic-ML `_abck~-1~` tail. No cheap public-engine lever
(T1.3/T1.5/canvas/humanize all closed, slot unchanged). Recommend
removing it from the flip list and tagging it "ML-tail, needs
synchronized clean-IP sensor diff" — not engine-cheap-addressable.

---

## 7. Cross-cutting: firefox-profile Akamai losses are a SEPARATE wire defect
homedepot/adidas under the **firefox** BO profile fail for an additional
reason orthogonal to sec-cpt: BO's firefox profile emits a **Chrome
ClientHello + Chrome H2** under a Firefox UA (`tls_impersonate:
"firefox_135"` is dead string; `net` branches on `device_class`, no
Firefox NSS arm — `crates/net/src/tls.rs`, `h2_client.rs`,
`presets.rs:~414`). A JA4↔UA cross-check (which Akamai does) buckets that
as high-risk regardless of sec-cpt. **That is the subject of the
firefox-wire doc, not this one** — but note: the FIX-A1/A2 sec-cpt fixes
will flip homedepot on **chrome/pixel/iphone**, not firefox, until the
NSS-TLS wire lands.

---

## Evidence index (file:line)
- `crates/net/src/h2_client.rs:11,28,43-50,118-130,167-174` — H2/window
  byte-exact (edge-fp resolved).
- `crates/browser/src/page.rs:242-247` `is_seccpt_solved`;
  `:1898` seccpt detect; `:1993` 45 s budget; `:2169-2171` +25 s extend
  (50 KB-gated, misses interstitials); `:2251` 90 s poll; `:2324-2346`
  seccpt early-break; `:2515-2523` MIN_RETRY early-return; `:2736`
  `iter+1<iterations` reload gate; `:2750-2752` +45 s pending-nav bump;
  `:3612-3618` M-3 keep-long-timers-refed.
- `crates/js_runtime/src/extensions/crypto_ext.rs:11-39` +
  `shared_apis_bootstrap.js:112-118` — real sha256 subtle.digest (PoW
  math runs).
- `crates/js_runtime/src/js/timer_bootstrap.js:69` — UNREF_THRESHOLD
  skip-on-`__keepLongTimersRefed`.
- `crates/browser/src/classify.rs:94,138-146,175-186` — sec-cpt /
  bestbuy classification + co-signal gating.
- `crates/browser/tests/holistic_sweep.rs:52` — `navigate(url, profile, 3)`.
- INTERNAL: `docs/research/engines/UNBLOCK_akamai_seccpt.md` (mechanism +
  the §2.3 "implicit/brittle budget" flag this doc operationalizes);
  `docs/universal_engine/site_debugging/{homedepot,adidas}_akamai_bmp_v3.md`
  (probe read-counts, `_abck~-1~` ML verdict, tried-and-failed list);
  `docs/akamai_sensor_analysis/scratch/adidas-network.json:1` (real-Chrome
  sensor POST → 201); `docs/GAP_DEEP_ANALYSIS_2026_04_28.md` (the
  now-resolved edge-fp root cause); `docs/ANTIBOT_RESEARCH_2026.md`
  (Akamai BMP tiered model + crypto/behavioral/adaptive provider split);
  `crates/akamai/src/sec_cpt.rs` (verified PoW solver — DEAD CODE, cannot
  be wired: homedepot serves the obfuscated bundle, not parseable 428
  JSON).
