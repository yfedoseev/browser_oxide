# UNBLOCK PLAN — the 10 "hard-block" sites, re-scoped (2026-05-16)

Synthesis of 4 parallel web+code research agents (full detail:
`UNBLOCK_kasada.md`, `UNBLOCK_datadome.md`, `UNBLOCK_akamai_seccpt.md`,
`UNBLOCK_spotify_duolingo.md`). **Headline: the 10 blocks are not 10
hard problems.** Re-bucketed by what it actually takes:

| Bucket | Sites | Reality | Effort |
|---|---|---|---|
| **Cheap classifier/UA fix** | spotify, duolingo, bestbuy | mislabeled — not real challenges | S |
| **Already flips; make deterministic** | homedepot | passes under 3-iter metric; 1-iter can't | S |
| **Probabilistic silent-pass OR 2 L builds** | etsy, tripadvisor | fingerprint/behavioral, then iframe subsystem | M → L+L |
| **Genuine human gate (out of scope)** | yelp | interactive solve-task captcha | none |
| **Engine-addressable; differential ready** | canadagoose, hyatt, realtor | Kasada — real Chrome passes this IP w/ zero behaviour ⇒ passive engine-parity gap; `/tl` oracle already captured | M (capture-ours + diff) |

So **4 of 10 are cheap/already-won**, 1 is out of scope (yelp),
2 are scoped big builds (etsy/tripadvisor), 3 are the genuine Kasada
tail. The honest tractable count is far better than "10".

---

## TIER 1 — Cheap, do first (S, mostly §4-gate-verifiable)

> **STATUS 2026-05-17: classifier half DONE** (branch
> `fix/engine-fp-backlog`). `classify.rs`: `akam/13` + bare `captcha`
> SMALL_BODY rows now co-signal-gated (`AKAMAI_CHALLENGE_COSIGNAL` /
> `INTERACTIVE_CAPTCHA_COSIGNAL` + `small_body_row_qualifies`). Effect:
> spotify/bestbuy/duolingo no longer mis-flagged challenges → fall to
> `ThinShell` (correctly "thin, not a block"). Regression:
> `classify::tests::fp_t1_invisible_recaptcha_and_akam13_cosignal`.
> Holistic classifier_tests preserved (`small_body_with_akam13_is_chl`
> still Akamai-CHL via the `bm-verify` co-signal). bestbuy audit bucket
> relabelled `Akamai-i18n-splash`. **Remaining:** duolingo's
> UA-redirect gate (Task #2 — separate, the classifier fix only stops
> the *mislabel*; reaching the real homepage needs the UA/feature
> alignment).

### spotify — classifier false positive (NOT a captcha)
- **Truth [CODE]:** the "captcha" hits are Spotify's invisible
  reCAPTCHA-v3 badge plumbing (`.grecaptcha-badge{display:none}` +
  hidden `g-recaptcha-response`) shipped on every page. Body = benign
  9.6 KB SPA shell, **zero anti-bot vendor cookies**. Root cause:
  `classify.rs` `("captcha","captcha-CHL")` bare substring, un-co-
  signal-gated (the FP-B2 class, this row left un-hardened).
- **Do:** require an interactive-challenge co-signal for the bare
  `captcha` row (or suppress when only invisible-v3 SDK/badge present).
  Spotify then → `ThinShell` (9.6 KB) = correctly not a challenge.
  Pure classifier fix, network-free regression test.

### duolingo — UA / browser-support redirect gate (NOT a captcha)
- **Truth [MECH]:** 302 → `/errors/not-supported.html` ("browser
  version not supported"); the captcha tag is the site-wide reCAPTCHA-
  v3 SDK matched by the same bare-substring bug. No vendor cookie.
- **Do:** (1) align `chrome_130_macos` UA / Client-Hints / probed JS
  feature so Duolingo's browser-support check passes (capture the
  pre-redirect response to see the exact gate key); (2) the same
  classifier fix as spotify. No captcha solving involved.

### bestbuy — NOT Akamai sec-cpt (mis-attribution)
- **Truth [MECH]/[CODE]:** the 7.9 KB body is Best Buy's "Choose a
  country" international splash, **zero Akamai markers**, reproduced by
  real Chrome 147 from our datacenter IP. The audit's hard-coded
  `("bestbuy","Akamai-CHL")` expectation (`audit_failing_sites.rs:27`)
  is wrong.
- **Do:** add a splash/geo-interstitial detector + correct the test
  expectation; optional `intl=nosplash`-style cookie follow to get the
  US homepage. No PoW work, no live oracle.

---

## TIER 2 — Already won; make deterministic (S)

### homedepot — sec-cpt, ALREADY FLIPPED at `b623d5d`
- **Truth:** flips to `L3-RENDERED` under the directive-sanctioned
  **3-iteration `holistic_sweep`** metric. The 1-iteration audit
  "block" is **structurally expected, not a regression** — the
  post-sec-cpt-solve reload cannot fire at `iterations==1`
  (`iter+1 < iterations` is false, `page.rs:1960`). Hard floor ≥2
  iterations; reliable at 3 + ~45 s budget.
- **Do (S, low risk, gated by `started_as_seccpt_challenge` ⇒ zero §4
  regression):** (1) add a `sec_cpt~3~` success early-break in the 90 s
  poll (`page.rs:1827-1885`) — there is a DataDome `datadome_solved`
  early-break but no sec-cpt analog, so the flip currently survives on
  incidental budget stacking; (2) raise the sec-cpt host budget to the
  Kasada tier (~45 s) + a network-free invariant test documenting the
  ≥2-iteration requirement. Flip verification = the live-oracle 3-iter
  `holistic_sweep` re-measure (the §4 gate structurally cannot prove a
  sec-cpt flip — by design). `sec_cpt::solve_crypto` stays DEAD (the
  eliminated dead-end; the bundle self-solves in V8, which it does).

---

## TIER 3 — Scoped big builds (etsy, tripadvisor)

Two independent paths; do A first (highest ROI), B only if A insufficient.

- **Track A — probabilistic silent-pass (M, cross-vendor, NOT offline-
  verifiable):** etsy's `x-datadome-riskscore=0.367` (<0.5) proves the
  block is fingerprint/behaviour-driven, **not IP** — the deep WASM
  probes only fire *after* failing silent scoring, so closing the
  silent-pass gap removes the trigger entirely. Land: (1) wire
  `crates/stealth/src/behavior.rs` into the nav→onload window so
  DataDome's `mousemove` listener fills a non-empty `_initialCoordsList`
  (datadome.md's "decisive silent-pass gap"); (2) the 4 fingerprint-
  coherence shims (Worker `userAgentData`, Worker-vs-main WebGL
  identity, `getTimezoneOffset`↔`Intl`, `mob`↔UA) + full Sec-CH-UA-*
  retry after the 403 `accept-ch`. Pure hygiene, also lifts
  Akamai/Kasada. Exit criterion = authorized live-oracle re-measure on
  etsy+tripadvisor (probabilistic per-tenant ML threshold — every
  credible 2026 source: fingerprint alone is insufficient, behaviour
  weighted equally).
- **Track B — the deterministic `rt:'i'` chain (L + L, fallback):**
  TWO structural gaps, both required: (B1) FP-E1's
  `createElement('iframe')`/`.src` arena-DOM interception so the
  script-injected `geo.captcha-delivery.com` iframe becomes a real
  child context (rescan infra `rematerialize_iframes` is landed; the
  trigger is missing); **(B2) — newly found, independently fatal:**
  `ChildIframe` (`iframe.rs:22-145`) has **zero parent↔child
  postMessage bridge** (the doc comment claims one; no code does), so
  the `rt:'i'` success step (iframe → postMessage → parent writes
  `datadome=` → reload) cannot complete even with B1. Plus the 6-char
  daily key + server JA4 ⇒ live-oracle verification only.

---

## TIER 4 — Out of pure-stealth scope / multi-session

### yelp — genuine DataDome interactive captcha (human gate)
nocdp real Chrome from our IP gets the solvable "solve-this-task"
challenge (not a hard 403 ⇒ not an IP ban). Same bucket as the
human-gate set. **Zero engine work** — label-truth only. The only
"option" is a 3rd-party/paid captcha-solving service (explicitly out
of stealth scope).

### canadagoose, hyatt, realtor — Kasada — ENGINE-ADDRESSABLE (thesis corrected 2026-05-16)

**Decisive empirical anchor (re-confirmed):** `ab_harness/nocdp.sh`
launches **real Chrome 147**, opens the URL, waits, does **ZERO**
mouse/scroll/keyboard interaction, from **this exact datacenter IP** —
and **passes all three** (captured window titles = real homepages:
"Luxury Performance Outerwear … Canada Goose", "Hotel Reservations …
Hyatt", "Realtor.com® | Homes for Sale …"). Our engine, **also zero
interaction, same IP**, gets the Kasada 429 / 732 B `bot1225.b:1`
interstitial.

**What this falsifies (the corrected thesis):**
- ❌ **NOT IP / reputation** — real Chrome passes from the same IP.
- ❌ **NOT behavioural absence** — nocdp Chrome emits *zero* behaviour
  and still passes, so "zero behavioural variance" cannot be what
  separates us from a pass on these sites. (The earlier
  "behaviour is the #1 lever" framing was wrong *for the nocdp delta*;
  behaviour may still matter vs stricter ML but it is **not** the
  load-bearing gap here.)
- ❌ **NOT "no engine-only path / needs a paid real-browser farm"** —
  a real browser engine *does* pass from here; the public-OSS-solver
  survey was answering a different question.
- ✅ **The gap is a passive, static engine-vs-real-Chrome-147
  divergence** [HYP, sharply localisable]: something in our
  observable surface as Kasada's `ips.js` VM + server see it — the JS
  runtime environment ips.js measures, how ips.js *executes in our V8*
  vs Chrome's, TLS/JA4, HTTP/2, or GPU/canvas — makes the server
  compute `b:1` for us but not for real Chrome. This is bounded and
  engine-addressable, **not** a holistic-ML mystery.

**The live-oracle reference is ALREADY captured** (the hard part is
done): `tl_capture.sh` recorded real Chrome 147's decrypted Kasada
`/tl` sensor POST with TLS keylog + tcpdump + tshark —
`ab_harness/tl/hyatt.tl_body.bin` (36 KB) and `canadagoose.pcap`
(15 MB) + `.keys`. Concrete ordered path:

- **K1 (S, §4-verifiable):** gate the parallel Rust `compute_cd_header`
  PoW OFF when ips.js self-solves — a competing single-use
  `x-kpsdk-cd` is a plausible self-inflicted `b:1` confound; remove it
  before differencing.
- **K2-DIFF (M, the decisive engine experiment — now the priority):**
  capture **our engine's** `/tl` sensor POST for hyatt/canadagoose
  (we already run ips.js in V8; surface what it POSTs), then
  **field-by-field diff against the captured real-Chrome `/tl`**
  (`hyatt.tl_body.bin`). The divergent field(s) localise *exactly*
  which passive surface element our engine emits wrong → a concrete,
  named, fixable engine bug, not a mystery. This is no longer "build a
  live-oracle regime" — the oracle exists; this is "capture our side +
  diff."
- **K3 (M, downgraded):** wire `stealth::behavior` into `Page::navigate`
  — still worth doing for cross-vendor (DataDome Track A1) and stricter
  ML, but explicitly **not** the nocdp-delta fix and not a prerequisite
  for K2-DIFF.

Verification: the §4 network-free gate still cannot prove a live
Kasada flip, but K2-DIFF makes the gap a **named field divergence**
provable against the captured oracle offline — a far stronger position
than "holistic tail". This is the single most promising
currently-blocked cluster, not the most hopeless.

---

## Recommended order (ROI-first)

1. **Tier 1** (spotify + duolingo + bestbuy classifier/UA fixes) — S,
   mostly §4-verifiable, removes 3 false "blocks" immediately.
2. **Tier 2** (homedepot deterministic sec-cpt early-break + budget) —
   S, makes an existing flip reliable.
3. **K1** (Kasada parallel-PoW gate) — S, §4-verifiable, removes a
   self-inflicted confound.
4. **Shared M: wire `behavior.rs` into `navigate`** — single change
   that feeds DataDome Track A1 *and* Kasada K3 (the #1 2026 trigger).
5. **DataDome Track A** coherence shims — M, cross-vendor hygiene.
6. **Live-oracle regime (K2)** — the authorized gate for everything
   that the network-free §4 gate structurally cannot verify
   (etsy/tripadvisor deterministic, canadagoose/hyatt/realtor).
7. **DataDome Track B (B1+B2 subsystems)** — L+L, only if Track A
   proves insufficient under the live oracle.

**Honest bottom line (corrected 2026-05-16):** ~4 sites are
cheap/already-won (spotify, duolingo, bestbuy, homedepot), 1 out of
scope (yelp), 2 scoped builds (etsy/tripadvisor). The 3 Kasada sites
are **NOT a hopeless holistic tail** — real Chrome passes them from
this IP with zero behaviour, so it is a **passive engine-parity gap**
with the live-oracle `/tl` reference **already captured**; the
decisive next experiment (K2-DIFF: capture our `/tl` + diff vs the
real-Chrome capture) localises it to a named field. The earlier
"behaviour is the lever / needs a paid farm" framing was wrong for the
nocdp delta. Highest-leverage moves: (1) Tier-1 classifier/UA fixes,
(2) **K2-DIFF Kasada `/tl` differential** (oracle in hand — turns the
hardest cluster into a bounded RE task), (3) the shared `behavior.rs`
wiring for the *probabilistic* DataDome/stricter-ML gains. No
fabricated path; no claim is made beyond what the captured evidence
supports.
