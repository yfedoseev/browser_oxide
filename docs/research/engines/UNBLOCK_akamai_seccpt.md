# UNBLOCK — Akamai sec-cpt sites (homedepot.com, bestbuy.com)

**Author:** bypass-research agent · **Date:** 2026-05-16 ·
**Baseline git HEAD:** `fd98bfa` · **Scope:** the 2 Akamai sites the
directive named as "sec-cpt blocked" — `homedepot.com`, `bestbuy.com`.
**Method:** read state docs + live code, 2026 web research (cited),
synthesize a concrete ordered plan. NO heavy cargo run (build-lock
discipline). All claims tagged **[MECH]** (externally-cited mechanism),
**[CODE]** (file:line I read this session), **[HYP]** (labelled
hypothesis).

---

## 0. TL;DR — the two sites are NOT the same problem

| Site | What it actually is | Engine-flippable? | At what budget |
|---|---|---|---|
| **homedepot.com** | **Genuine Akamai sec-cpt** crypto-PoW interstitial (`<div id="sec-if-cpt-container">` + rotating obfuscated `/Wjv3…` bundle). Self-solves in JS with **no user interaction**. | **YES** — already flipped at `b623d5d` under the 3-iteration `holistic_sweep` metric (`L3-RENDERED len=2507`). The 1-iteration audit is structurally too few round-trips. | **≥3 nav iterations** + ~10–20 s wall budget headroom. NOT 1-iter. |
| **bestbuy.com** | **NOT a sec-cpt block at all.** The captured 7.3 KB body is the Best Buy **"Choose a country" international splash** (`<title>Best Buy International: Select your Country</title>`) — **zero** Akamai markers (`_abck`/`bm_sz`/`sec_cpt`/`sensor_data` all absent). Real Chrome 147 from this same datacenter IP gets the *same splash* [CODE]. | **PARTIALLY** — the splash is a **geo/cookie gate, not Akamai**. Engine-addressable with an `intl=nosplash` cookie/redirect follow; the *underlying* US homepage is plain-BMP and renders for us. | 1-iter is fine once the splash is handled; it was a **classifier mis-attribution**, not a PoW block. |

**The single most important correction in this document:** the directive's
premise "2 AKAMAI SEC-CPT-blocked sites — homedepot.com, bestbuy.com" is
**half wrong**. Only homedepot is sec-cpt. bestbuy's 7.9 KB edge-block is
the **international country-selection splash**, an IP-geo interstitial
that **real Chrome also receives from our IP** — it is not a PoW
challenge, not a fingerprint failure, and `sec_cpt::solve_crypto` is
irrelevant to it. Treating bestbuy as "the same sec-cpt PoW variant" would
send the next session chasing a non-existent Akamai blocker.

---

## 1. homedepot — mechanism (what sec-cpt actually is in 2026)

### 1.1 The crypto provider auto-solves in JS, no interaction [MECH]

2026 sources are unanimous that the Akamai sec-cpt **crypto** provider is
a *headless background JavaScript proof-of-work*, not a click puzzle:

- "When a client visits a website protected by Akamai, the system may
  prompt it to run a client-side JavaScript **in the background** within
  a specified time frame. This script is usually a crypto challenge"
  ([zenrows bypass-akamai, accessed 2026-05-16]).
- The "tile-based puzzle that takes seconds" is the **Content
  Protector / behavioral** variant, a *different* provider — it appears
  for the `behavioral` provider, not `crypto`
  ([dataresearchtools homedepot 2026; scrapeinsight 2026, accessed
  2026-05-16]). homedepot serves the **crypto** variant (rotating
  obfuscated bundle, `<div id="sec-if-cpt-container">`, no visible
  puzzle in the 2.6 KB body) [CODE: `akamai.md` §3; the 1 MB real-Chrome
  capture has zero puzzle markup].

⇒ **[MECH] homedepot's sec-cpt is exactly the "self-solve in V8" class.**
A real Chrome with JS enabled clears it with no human action. This is
why the `b623d5d` doc-20 fix worked: once our BMP POST stopped fighting
the bundle, the bundle's own JS PoW ran to completion in our V8.

### 1.2 The three providers (cite) [MECH]

Per [docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt,
accessed 2026-05-16] + [pkg.go.dev hyper-sdk-go/akamai, accessed
2026-05-16]:

| Provider | Wait | POSTs to `/_sec/verify` | Notes |
|---|---|---|---|
| `crypto` | `chlg_duration` **server-enforced, cannot be shortened** | **1** POST `?provider=crypto` | pure PoW; homedepot's variant |
| `behavioral` | none | 0 (sensor posts to script endpoint instead) | the "tile puzzle" / Content Protector |
| `adaptive` | `chlg_duration` | 1 POST `?provider=adaptive` **then up to 3 sensor posts** | combined |

### 1.3 The round-trip / timing budget — THE load-bearing number [MECH]

From the Hyper Solutions crypto-challenge reference flow
([gist justhyped 38e3cc4b…, accessed 2026-05-16] +
docs.hypersolutions.co), the *minimum* successful crypto sec-cpt
sequence is:

```
1. GET target                       → 428 + sec-cpt challenge (sets sec_cpt cookie, slot ≠ ~3~)
2. WAIT chlg_duration seconds        → SERVER-ENFORCED (example values: data-duration=5, JSON chlg_duration=30)
3. POST /_sec/verify?provider=crypto → submit PoW answers
4. GET  /_sec/cp_challenge/verify    → {"success":true}; sec_cpt cookie now ends ~3~
5. GET target again                  → real content (now that sec_cpt ~3~ is carried)
```

**Minimum 3 HTTP round-trips after the initial 428, plus a mandatory
multi-second `chlg_duration` sleep that is server-enforced and
un-skippable.** In the *in-page bundle self-solve* model (what we use),
the bundle does steps 2–4 itself inside the page, then triggers step 5
(a reload). So the engine must:

- **(a)** keep the page alive long enough for the bundle to run its PoW
  *and* sit out `chlg_duration` (seconds), then
- **(b)** perform the **post-solve reload** (step 5) carrying the now-
  `~3~` `sec_cpt` cookie, and
- **(c)** do this across **at least one full extra navigation
  iteration** (the reload is a fresh top-level GET).

This is exactly why **1 iteration cannot work and 3 can** — see §3.

---

## 2. Why our code flips at 3 iterations but not at 1 (the audit/holistic split)

### 2.1 The two measurement lenses are genuinely different [CODE]

- **`holistic_sweep.rs:52`** → `Page::navigate(url, profile, 3)` — **3
  iterations**. This is the directive-sanctioned metric (the 126-corpus
  ledger). homedepot flips here: `holistic-end: stores homedepot
  L3-RENDERED len=2507` (master plan §8.5 `b623d5d`).
- **`audit_failing_sites.rs:94,240`** → `Page::navigate(url,
  chrome_130_macos(), 1)` — **1 iteration**. homedepot stays
  edge-blocked here (2.6 KB sec-cpt interstitial).

Both call the *same* unified classifier `browser::engine_classify`
(`classify.rs:151`) [CODE]. The classifier is not the discrepancy —
**the iteration count is**. `engine_classify` tags `Akamai-sec-cpt-CHL`
on the literal `/_sec/cp_challenge` (any size) and `Akamai-CHL` on
`_abck`/`akam/13` when body < 30 KB; a post-solve 2507-byte page that no
longer carries any of those markers classifies `L3-RENDERED`
(`classify.rs:84,103-104,179`) [CODE]. So the *only* thing that changes
the verdict is **whether the bundle got enough iterations/time to clear
the markers**.

### 2.2 The exact 1-iter failure path in `navigate_loop_internal` [CODE]

Walking `crates/browser/src/page.rs`:

1. `started_as_seccpt_challenge = html.contains("sec-if-cpt-container")
   || "sec-cpt-if"` is captured from the **original** 428 body
   (`:1516`). Persistent — survives DOM mutation (the doc-20 fix).
2. `handle_akamai_flow` is **skipped** (`:1931-1937`): when
   `started_as_seccpt_challenge`, `akamai_state = NeedsSecCpt`, the wrong
   BMP `sensor_data` POST is suppressed → the bundle is sole actor
   (this is the `b623d5d` win) [CODE].
3. The page is built and the event loop drained once with
   `drain_timeout = remaining_budget.max(8 s)` (`:1703-1709`). Budget
   for `homedepot.com` = **25 000 ms** host-default (`:1591-1600`)
   [CODE]. The bundle (560 KB + 425 KB sub-bundles per the
   `seccpt-trace`) starts its PoW here.
4. The **90 s sec-cpt poll** (`:1827-1885`) is entered (gated on
   `started_as_seccpt_challenge`). It pumps `run_until_idle(200 ms)`
   waiting for a **pending nav** (`PENDING_NAV_JS`). **Critically:
   there is NO sec-cpt-specific cookie-break here** — only DataDome has
   `datadome_solved` early-break (`:1867-1883`). A sec-cpt site burns
   the poll until either a pending nav appears or the *outer nav budget*
   (25 s) is exhausted [CODE]. ← **gap G-SC-1**.
5. **Iter 0 → cookie-delta retry** (`:1945-2220`): re-issues an in-V8
   `fetch(location.href)`. The fix's `started_as_seccpt_challenge`
   keeps the retry active even after the marker is mutated away
   (`:1957`) [CODE]. This is **the post-solve reload (step 5 of §1.3)**.
   It needs `iter + 1 < iterations` (`:1960`) — **with `iterations==1`
   this branch is `0 + 1 < 1 == false` → the retry CANNOT fire** [CODE].
   ← **this is the structural reason 1-iter cannot flip homedepot.**
6. With `iterations==3` the retry fires on iter 0, the V8 refetch
   carries the bundle-set `sec_cpt~3~` cookie, the body comes back as
   the (intermediate) real content, classifies `L3-RENDERED`.

**[CODE] Conclusion: homedepot at 1 iteration cannot flip *by
construction* — the post-solve reload is hard-gated behind
`iter + 1 < iterations`. It is not a fingerprint/PoW failure; it is an
iteration-budget gate.** The `b623d5d` flip at 3 iterations is real and
correctly attributed; the 1-iter "still blocked" audit is *expected*,
not a regression, and the directive's honest distinction holds: the
sanctioned re-measure is the 3-iter `holistic_sweep` lens.

### 2.3 Residual fragility even at 3 iterations [HYP]

The flip at 3 iters is real but **not robust**:

- The 25 s `homedepot.com` budget must cover: build + bundle download
  (≈1 MB across two sub-bundles) + PoW + the **server-enforced
  `chlg_duration` sleep (seconds)** + the post-solve refetch + that
  page's drain. The `seccpt-trace` `nav_ms=119321` shows the *actual*
  flip run took ~119 s wall — far over the 25 s host budget, only
  surviving because the **pending-nav budget bump `nav_budget +=
  Duration::from_secs(45)`** (`:2263-2265`) and the per-iteration
  `MIN_RETRY_BUDGET` extensions stack up [CODE]. This is incidental,
  not designed — a timing change elsewhere could silently un-flip it.
- `chlg_duration` is **server-enforced and un-skippable** [MECH]. If
  Akamai raises homedepot's duration (it rotates configs ≈24–48 h, often
  faster — `lib.rs:243-249` notes a 40-min `fileHash` rotation [CODE]),
  the fixed budgets can fall short with no code change.

⇒ The flip is correct but **the budget math is implicit and brittle**.
§3 makes it explicit and adds a sec-cpt success early-break so it
doesn't depend on burning the full 90 s poll + lucky budget bumps.

---

## 3. bestbuy — it is NOT sec-cpt (decisive diagnosis)

### 3.1 The captured 7.9 KB body is the international splash [CODE]

`ab_harness/shots/https_www_bestbuy_com_.html` (7.3 KB, captured by
`hd_capture.js` / `nocdp` = **real Chrome 147, headless:false,
channel:chrome**, from this datacenter IP):

```html
<title>Best Buy International: Select your Country - Best Buy</title>
... <h1>Choose a country.</h1> <a href="...bestbuy.ca...">Canada</a>
<a href="https://www.bestbuy.com/?intl=nosplash">United States</a> ...
```

Marker grep over the full body: **35× `bestbuy`, 1× the title,
ZERO** occurrences of `_abck`, `bm_sz`, `sec_cpt`, `sec-if-cpt`,
`/_sec/cp_challenge`, `sensor_data`, `bazadebezolkohpepadr`,
`akam/13` [CODE]. There is **no Akamai challenge in this body at all.**

### 3.2 What this means [MECH/CODE]

- bestbuy.com serves a **geo/cookie interstitial** ("Choose a country")
  when the request has no `intl=nosplash` (or `intl=` country) cookie
  and the IP geolocates as international / datacenter. The escape link
  is literally `https://www.bestbuy.com/?intl=nosplash`.
- **Real Chrome from our IP gets the identical splash** — so this is
  **not** a stealth gap, not a fingerprint failure, and not an IP ban
  (a human clicking "United States" proceeds normally).
- Our classifier currently tags this body `Akamai-CHL`/`EdgeBlock` (via
  the `audit_failing_sites` table literal `("bestbuy", "Akamai-CHL", …)`
  `:27` [CODE]) — a **mis-attribution**: the body is < 30 KB and may
  trip a weak marker, but there is no Akamai marker in it; it is the
  bare-`captcha`/size heuristics or the hard-coded expected-tag that
  paint it Akamai. This is a **classifier/labeling FP**, parallel to
  FP-DET-2 (thin-shell) and FP-B2 in `99_CODE_FALSE_POSITIVES.md`.

### 3.3 Is there a *real* Akamai layer behind the splash? [HYP]

bestbuy IS an Akamai BMP tenant (`get_tenant_settings` has a hardcoded
bestbuy seed `3_224_113` + obfuscated post_path; `known_file_hash`
`6_249_250` `lib.rs:254` [CODE]). The akamai.md §0 table already calls
bestbuy `pass (7.8 KB)` with a **thin-shell caveat**. The honest read:
once the `intl=nosplash` gate is passed, the US homepage is a normal
v3-protected page in the "renders for us" class (10/11 Akamai sites
render — Phase-0.2). **No evidence bestbuy ever serves sec-cpt.** The
7.9 KB "block" the directive cites is the splash, full stop.

⇒ **bestbuy is engine-addressable as a geo-cookie/redirect-follow
problem, not a PoW problem.** It does not need the Phase-5 bundle
self-solve. It needs (a) a classifier fix so the splash is not labeled
`Akamai-CHL`, and (b) optionally an `intl=nosplash` cookie/redirect
follow so the audit reaches the real US homepage.

---

## 4. Concrete ordered plan

Tags: **S/M/L** difficulty · risk · verification. The directive forbids
wiring `sec_cpt::solve_crypto` (eliminated dead-end, master plan §6;
also un-feedable — homedepot serves the *bundle* variant, not the
parseable `<iframe id="sec-cpt-if" challenge="…">` JSON the Go/Py SDK
parses [MECH: pkg.go.dev hyper-sdk-go/akamai]). None of the steps below
wire it.

### STEP 1 — [CODE] sec-cpt success early-break in the 90 s poll (homedepot) — **S, low risk**

**Problem:** the sec-cpt poll (`page.rs:1827-1885`) has a DataDome
`datadome_solved` early-break but **no sec-cpt analog**. A sec-cpt nav
either gets a pending nav or burns time until a budget bump; the flip
currently depends on incidental budget stacking (§2.3).

**Change:** inside the poll loop, when `started_as_seccpt_challenge`,
add a cheap cookie check mirroring the existing DataDome break:

```rust
if started_as_seccpt_challenge {
    if let Some(p) = parsed_current.as_ref() {
        let now = client.cookies_for_url(p).await.unwrap_or_default();
        // Akamai marks a SOLVED sec-cpt challenge with the `~3~`
        // suffix in the sec_cpt cookie (docs.hypersolutions.co,
        // accessed 2026-05-16). Presence of `sec_cpt` alone is the
        // *unsolved* challenge cookie (set by the 428) — require the
        // ~3~ marker, the documented success signal, exactly as
        // DataDome's datadome_solved requires a non-challenge body.
        if now.contains("sec_cpt=") && now.contains("~3~") {
            break; // bundle solved → fall through to the cookie-delta reload
        }
    }
}
```

This makes the success path **deterministic on the documented success
signal** instead of "ran out the clock and got lucky with budget
bumps". Add a `datadome_solved`-style helper
`akamai::sec_cpt_solved(cookies) = cookies.contains("sec_cpt=") &&
cookies.contains("~3~")` so it is unit-testable network-free.

- **Difficulty:** S (≈15 lines, mirrors an existing, gate-green
  pattern). **Risk:** low — gated entirely behind
  `started_as_seccpt_challenge` (false for every non-sec-cpt site incl.
  the whole §4 gate; the 10 plain-BMP Akamai sites never serve
  `sec-if-cpt`). Zero §4 regression by construction (same argument that
  made `b623d5d` safe).
- **Verification:** network-free unit test for `sec_cpt_solved`
  (`"sec_cpt=…~3~"` ⇒ true; `"sec_cpt=…~1~"`/absent ⇒ false). Live
  flip verification = the directive-sanctioned `holistic_sweep`
  (`h_store_homedepot`, 3-iter) re-measure — `L3-RENDERED` must hold and
  ideally `nav_ms` should *drop* (no longer burning the 90 s poll).
- Tag: **[CODE]** behavior, **[MECH]** success-signal (`~3~`) cited.

### STEP 2 — [CODE] Make the homedepot iteration/time budget explicit, not incidental — **S/M, low-med risk**

**Problem (§2.3):** the flip survives only because pending-nav budget
bumps + `MIN_RETRY_BUDGET` extensions stack to ~119 s, while the
nominal `homedepot.com` host budget is 25 s. Implicit and brittle
against `chlg_duration` config rotation.

**Change (two parts, both narrowly gated):**

1. **Raise the sec-cpt host budget deliberately.** In the
   `host_budget_default_ms` match (`page.rs:1591-1600`), keep
   `homedepot.com` but make sec-cpt sites a *named* class with an
   explicit comment that the budget must cover `build + ~1 MB bundle +
   PoW + server-enforced chlg_duration (5–30 s) + post-solve reload +
   that page drain`. Set it to e.g. **45 000 ms** (same tier as Kasada,
   which also runs a heavy in-page VM PoW) instead of 25 000. This
   removes reliance on the accidental `+= 45 s` pending-nav bump.
2. **Ensure ≥3 iterations reach the homedepot audit path.** The
   1-iter `audit_failing_sites.rs:94,240` lens *cannot* flip sec-cpt by
   construction (§2.2 step 5: `iter+1 < iterations` is false at
   `iterations==1`). This is **honest expected behavior, not a bug** —
   document it: add a comment + a `#[test]`-asserted invariant that
   sec-cpt flips require ≥2 iterations, and (optionally) let the audit
   harness pass `2` for known sec-cpt hosts so the audit lens stops
   reporting a structurally-unreachable "block". Do **not** change the
   `holistic_sweep` count (it is the ledger metric and already passes
   at 3).

- **Difficulty:** S for (1), M for (2) (touches a shared harness — must
  not perturb the 126-ledger; keep `holistic_sweep` at 3, only the
  audit lens changes, and only for sec-cpt hosts). **Risk:** low for
  (1) (budget only grows, gated by host); med for (2) (harness change —
  verify the ledger count is byte-unchanged).
- **Verification:** `holistic_sweep` `h_store_homedepot` still
  `L3-RENDERED` (live, directive-sanctioned). The audit invariant test
  is network-free. Confirm `cargo test --test holistic_sweep
  classifier_tests` 10/10 unchanged (ledger equivalence).
- Tag: **[CODE]** + **[MECH]** (`chlg_duration` server-enforced).

### STEP 3 — [CODE] bestbuy: stop mis-labeling the international splash as Akamai-CHL — **S, low risk**

**Problem (§3):** bestbuy's 7.9 KB body is the "Choose a country"
splash with **zero Akamai markers**, yet it is reported as an Akamai
sec-cpt block. This is a classifier/expected-tag FP that corrupts the
"2 sec-cpt sites" framing.

**Change:**

1. Add a structural splash detector to `classify.rs` (network-free):
   a body containing `"Select your Country"` /
   `"?intl=nosplash"` / `<title>Best Buy International:` AND no Akamai
   marker ⇒ a distinct verdict (`GeoSplash` / reuse `ThinShell`-class),
   **not** `Akamai-CHL`/`EdgeBlock`. Mirrors the FP-B2/FP-B3 size-gate
   discipline already in the codebase.
2. Fix the hard-coded expectation in `audit_failing_sites.rs:27`
   (`("bestbuy", "Akamai-CHL", …)`) — bestbuy's edge response is the
   geo splash, not an Akamai challenge; the expected tag is wrong.
3. **Optional (M):** seed an `intl=nosplash` cookie (or follow the
   `?intl=nosplash` link) for `*.bestbuy.com` before/at first nav so
   the audit reaches the real US homepage (which is plain-BMP and in
   the "renders for us" class). This is a cookie/redirect-follow, **not**
   a PoW solve.

- **Difficulty:** S for (1)+(2), M for (3). **Risk:** low — a new
  splash classification cannot regress the §4 gate (no gate fixture is
  the bestbuy splash); the cookie seed is host-gated to bestbuy.
- **Verification:** network-free classifier fixture (the captured
  `https_www_bestbuy_com_.html` ⇒ verdict != Akamai-CHL). Live: a
  bestbuy nav with the cookie reaches a multi-KB US homepage (live-
  oracle, optional).
- Tag: **[CODE]** (captured body), **[MECH]** (geo splash mechanic).

### STEP 4 — [CODE] Wire `BotScoreVector::parse` as a passive sec-cpt regression oracle — **S, zero nav risk**

`akamai::BotScoreVector::parse` (the `Server-Timing: ak_p` 6-sub-score)
is **dead code** (`99_CODE_FALSE_POSITIVES.md` FP-CODE-4). For sec-cpt
work it is the single best *free, passive* signal of whether our
Phase-1 score is improving or regressing — it costs zero extra requests
(Akamai already returns it on every response). Log it (debug-gated) in
the nav loop for Akamai hosts so the next session can see, per run,
whether homedepot's score moved without needing a live flip. Pure
diagnostic ROI, no nav-path change.

- **Difficulty:** S. **Risk:** none (read-only header parse, debug-
  gated). **Verification:** an integration assertion that a sweep run
  records a non-empty `BotScoreVector` for ≥1 Akamai response.
- Tag: **[CODE]**.

### NON-STEPS (explicitly do not do)

- **Do NOT wire `sec_cpt::solve_crypto`.** Eliminated dead-end (master
  plan §6). Also **un-feedable**: it parses the JSON/`<iframe
  id="sec-cpt-if" challenge="…">` shape [MECH: pkg.go.dev
  hyper-sdk-go/akamai `ParseSecCptChallenge`]; homedepot serves the
  **rotating obfuscated `/Wjv3…` bundle** with `<div
  id="sec-if-cpt-container">` and **no parseable challenge attribute**
  [CODE: `akamai.md` §3]. The bundle MUST self-solve in V8 — which it
  already does post-`b623d5d`. The work is *budget/poll plumbing*
  (Steps 1–2), not crypto.
- **Do NOT chase a bestbuy "sec-cpt PoW variant".** It does not exist —
  bestbuy's edge block is the geo splash (§3), reproduced by real
  Chrome from our IP.
- **Do NOT "give homedepot more iterations" in `holistic_sweep`.** It
  already passes at 3 (the ledger metric). Touching it risks the
  126-corpus count. Only the *audit* lens (1-iter) needs the documented
  expected-behavior note.

---

## 5. Answers to the directive's three concrete questions

**(a) How many nav iterations / event-loop time does the bundle PoW
need; should the orchestrator give homedepot more iterations?**
[CODE/MECH] **≥3 nav iterations is sufficient and already wired**
(`holistic_sweep` `Page::navigate(.., 3)` → `b623d5d` flip). The hard
floor is **≥2** (the post-solve reload is gated `iter + 1 <
iterations`, `page.rs:1960` — at `iterations==1` it can *never* fire,
so 1-iter is structurally impossible regardless of fingerprint). Time:
the bundle needs build + ≈1 MB bundle download + JS PoW + a
**server-enforced `chlg_duration` (typ. 5–30 s, un-skippable [MECH])**
+ post-solve reload; the observed flip run wall-clock was `nav_ms≈119
s`, surviving only via incidental budget-bump stacking. **What makes it
reliable at low iteration count:** the `started_as_seccpt_challenge`
persistent flag (already shipped, `b623d5d`) keeps the reload path
active across DOM mutation; what is *missing* is a deterministic
`sec_cpt~3~` success early-break (Step 1) and an explicit, non-
incidental budget (Step 2). The honest distinction: the directive's
sanctioned re-measure is the **3-iter `holistic_sweep`** lens (flipped);
the **1-iter audit** lens is structurally unable to flip sec-cpt and
that is expected, not a regression.

**(b) Is bestbuy the same sec-cpt path or a different Akamai variant
(BMP/SBSD)?** [CODE] **Neither.** bestbuy's 7.9 KB edge body is the
**Best Buy international "Choose a country" splash** — a geo/cookie
interstitial with **zero Akamai markers**, reproduced byte-for-shape by
**real Chrome 147 from our datacenter IP** (captured
`ab_harness/shots/https_www_bestbuy_com_.html`). Not sec-cpt, not BMP
sensor-fail, not SBSD, not an IP ban. It is a **classifier mis-
attribution** (the hard-coded `("bestbuy","Akamai-CHL")` expectation +
the small-body heuristics). The underlying US homepage is plain-BMP and
in the "renders for us" class once `intl=nosplash` is set.

**(c) What concrete page.rs/akamai changes flip both under the strict
lens without wiring the Rust solver?** [CODE] **homedepot:** Step 1
(sec-cpt `~3~` early-break, ≈15 lines, S, low risk) + Step 2 (explicit
sec-cpt budget + audit-lens expected-behavior note, S/M). These make
the *already-working* 3-iter flip deterministic and document why 1-iter
cannot (honest, not a fix to force it). **bestbuy:** Step 3 (splash
classifier fix + correct the wrong expected tag, S; optional
`intl=nosplash` cookie follow, M) — a labeling/cookie fix, **no PoW
work**. Neither touches `solve_crypto`.

---

## 6. Sources (URL · claim · accessed 2026-05-16)

- **docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt**
  — sec-cpt providers (crypto/behavioral/adaptive); `chlg_duration`
  server-enforced & un-skippable; crypto = 1 POST `/_sec/verify?provider=crypto`;
  adaptive = 1 POST + up to 3 sensor posts; success ⇔ `sec_cpt` cookie
  contains `~3~`.
- **gist.github.com/justhyped/38e3cc4b36456ddd9e4ecb2875043a08** —
  crypto flow: GET → wait → POST `/_sec/verify?provider=crypto` → GET
  `/_sec/cp_challenge/verify` → check `.success`; **min 3 round-trips**
  after the 428 (loop may iterate more).
- **pkg.go.dev/github.com/Hyper-Solutions/hyper-sdk-go/akamai** —
  `ParseSecCptChallenge` expects `<iframe id="sec-cpt-if"
  provider="crypto" challenge="…" data-duration=5
  src="/_sec/cp_challenge/ak-challenge-4-3.htm">` (HTML) **or** JSON
  (`sec-cp-challenge`,`provider`,`chlg_duration`,`token`,`timestamp`,
  `nonce`,`difficulty`,`timeout`,`cpu`); example values
  `data-duration=5`, `chlg_duration=30`, `difficulty=15000`,
  `timeout=1000`; `Sleep()` waits `chlg_duration`. Package deprecated →
  use `…/hyper-sdk-go/v` (v2). **This is the parseable shape our dead
  `sec_cpt.rs` matches — NOT what homedepot serves.**
- **zenrows.com/blog/bypass-akamai** — Akamai crypto challenge is a
  client-side JS run **in the background** within a time frame; "burden
  of proof" PoW; humans not interrupted. ⇒ crypto sec-cpt auto-solves,
  no user interaction.
- **dataresearchtools.com/how-to-scrape-homedepot/ ; scrapeinsight.com
  /blog/scrape-the-home-depot-product/** — Home Depot uses Akamai Bot
  Manager + a JS behavioral challenge rejecting datacenter IPs; the
  interactive (tile-puzzle) interstitial is the *behavioral/Content
  Protector* variant (seconds to clear); HD also exposes a GraphQL
  product API (out of scope here, noted).
- **scrapfly.io/blog/posts/how-to-bypass-akamai-anti-scraping**
  (2026-04-18) — general Akamai detection (JA3/JA4, IP, H2,
  fingerprint); **no sec-cpt / homedepot / bestbuy specifics**
  (checked, negative result, recorded so it is not re-fetched).
- **roundproxies.com/blog/bypass-akamai/** — marketing-heavy; only
  generic "maintain `_abck`/`bm_sz`"; **no concrete sec-cpt timing**
  (negative result).
- **gist.github.com/0xdevalias/b34feb567bd50b37161293694066dd53** —
  Cloudflare-centric; no Akamai sec-cpt detail (negative result).
- **github.com/voidstar0/akamai-deobfuscator** —
  Akamai-script deobfuscator (tooling; supports analyzing the bundle,
  not a one-call sec-cpt solver). Maintained-ish; not load-bearing here
  (we self-solve in V8, we do not statically reverse the bundle).

### Local sources read (file:line)

- `crates/akamai/src/sec_cpt.rs` (full — DEAD, doc-labelled, JSON/iframe
  parser; un-feedable for the bundle variant).
- `crates/akamai/src/lib.rs` :236-256 (`known_file_hash`),
  :452-500 (`get_tenant_settings` bestbuy/homedepot).
- `crates/browser/src/page.rs` :448-599 (`handle_akamai_flow`),
  :1490-1617 (challenge-origin flags + budget), :1618-1714
  (iter loop + drain), :1730-1811 (fast-exit), :1827-1885 (90 s sec-cpt
  poll — **no sec-cpt success break, G-SC-1**), :1916-1937 (BMP-suppress
  fix `b623d5d`), :1945-2220 (cookie-delta retry, `iter+1<iterations`
  gate), :2330-2467 (Inc-8 self-solve window — DataDome only).
- `crates/browser/src/classify.rs` :37-185 (canonical
  `engine_classify`; sec-cpt marker `/_sec/cp_challenge`; size gates).
- `crates/browser/tests/holistic_sweep.rs` :52 (`navigate(..,3)`),
  :357-360 (`h_store_homedepot`), :842-872 (classify wrapper).
- `crates/browser/tests/audit_failing_sites.rs` :27,36
  (expected-tag table — **bestbuy `Akamai-CHL` is wrong**),
  :94,240 (`navigate(..,1)`).
- `ab_harness/shots/https_www_bestbuy_com_.html` (7.3 KB — **real
  Chrome international splash, zero Akamai markers**),
  `…/https_www_homedepot_com_.html` (1 MB — real Chrome post-clear
  render), `ab_harness/hd_capture.js`, `nocdp_multi.sh` (capture
  provenance = real Chrome 147, headless:false).
- `docs/research/engines/akamai.md` (§0,§3,§9,§10,§11),
  `docs/research/engines/99_CODE_FALSE_POSITIVES.md`,
  `docs/research_2026_05_16/00_MASTER_PLAN.md` §6/§8.5,
  `docs/research_2026_05_16/06_126_CORPUS_PROGRESS_LEDGER.md`,
  `docs/universal_engine/site_debugging/homedepot_akamai_bmp_v3.md`.

### Verification regime (where it structurally fails)

Steps 1, 3(1-2), 4 and the Step-2 audit invariant are
**network-free-verifiable** (unit tests, classifier fixtures, gate).
The homedepot live flip (Step 1/2 effect) is **only** verifiable
against a live, daily-rotating Akamai oracle via the directive-
sanctioned 3-iter `holistic_sweep` re-measure — the network-free §4
gate structurally cannot prove a sec-cpt flip (no daily key, no live
edge), exactly as `akamai.md` §11 and master plan §8.5 state. Any claim
of "homedepot fully passes" from a green offline gate is FP-DET-3; the
honest target is "challenge cleared, classifies `L3-RENDERED` under the
sanctioned metric, deterministically" — and `len=2507` remains a
post-sec-cpt intermediate page, not the full multi-MB homepage (a
follow-up nav-continuation refinement, out of scope here).

— End —
