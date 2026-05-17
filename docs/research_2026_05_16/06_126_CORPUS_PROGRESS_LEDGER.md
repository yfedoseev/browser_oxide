# 06 — 126-CORPUS PROGRESS LEDGER (single source of truth)

**Purpose.** One canonical, consistent record of how many of the 126 sites
we open, so a session does **not** need to re-run the full ~70-min sweep to
know where we stand. Re-measure only after a *site-affecting* commit lands;
otherwise trust this file. Supersedes ad-hoc number-quoting from older docs.

## What "126" is

The corpus is `crates/browser/tests/holistic_sweep.rs::sites_list()` — 126
sites, the full benchmark. Distinct from the **29-site canonical anti-bot
hard set** (`audit_failing_sites.rs`), which is a stricter, single-profile,
typed-classifier *subset lens* (see reconciliation below). Both are valid;
they answer different questions. This ledger is the authority for the 126.

## Coverage — last measured 2026-05-13 (`docs/CHROME_148_SWEEP_RESULTS_2026_05_13.md`, same source IP)

| Strategy | Open / 126 |
|---|---:|
| Single profile — desktop Chrome 148 (`chrome_130_macos`) | **113** |
| Single profile — Android (`pixel_9_pro_chrome_147`) | **115** |
| Single profile — iOS (`iphone_15_pro_safari_18`) | **115** |
| Single profile — Firefox 135 | 112 |
| **Per-domain routing (best profile per site)** | **120** |
| Practical 3-chromium-profile routing | **119** |

**Headline: we open ~113/126 on one desktop profile, ~120/126 with
per-site profile routing.** Ceiling = 126 − 6 universal blocks.

## The 6 universal blocks (no profile passes — these are the real residual)

| Site | Vendor | Note |
|---|---|---|
| canadagoose | Kasada | holistic ML tail; Phase 2 closed the realm/identity line — no single lever |
| hyatt | Kasada | same class |
| realtor | Kasada | same class |
| udemy | Cloudflare | JS-challenge / datacenter-ASN — IP-ban hypothesis unverified |
| douyin | captcha | Chinese vendor, region-locked — out of stealth scope |
| wildberries | ERROR/captcha | Cyrillic region scoring — out of stealth scope |

Engine-addressable subset = **Kasada×3 + udemy**; douyin/wildberries are
region/human gates, not stealth problems.

## Reconciliation with the 29-set re-baseline (so the two numbers stay consistent)

Phase 0.2 (master plan §8.5) typed-re-baselined the 29-set on **one fixed
desktop profile + the strict structural classifier** → 18/29 render, hard
residual = {Kasada×3, homedepot, etsy, tripadvisor}. This does **not**
contradict 120/126: homedepot/etsy/tripadvisor are *not* in the 126
universal-block set because some profile passes them under routing, and
because the 126 classifier is the older render-based one. The 29-set lens
is deliberately the harder, single-profile, anti-bot-only view; the 126
ledger is the full-corpus routed view. Quote the right number for the
question: full-corpus standing → this file; hard-set engine work → §8.5.

## Validity / when to re-run

These numbers are **still current as of 2026-05-16**. Commits
`3739da9..7d748b2` (Phase 0/1/2) were non-site-affecting. Phase 5
Increments 1–5 (`4cece4c`, `201cac9`, `87cf3fa`, `dfd0645`) **are**
site-affecting; per this clause both engine-addressable hard sites were
live-re-measured **targeted** with all 5 increments active:
- **homedepot** (`h_store_homedepot`): `in-V8 refetch 200/2615` loops on
  the 2.6 KB sec-cpt interstitial — `/Wjv3…` bundle does not self-solve
  → **NOT flipped**.
- **etsy** (`h_store_etsy`): i.js loads `200/15014`, refetch `403/805`,
  `holistic-end: DataDome-CHL` → **NOT flipped**.
⇒ ZERO engine-addressable flips with Inc 1–5.

**UPDATE — Increment 7 (`b623d5d`, the doc-20 anti-pattern fix) FLIPS
homedepot.** Targeted directive-sanctioned re-measure
(`h_store_homedepot`, `holistic_sweep::classify`):
`holistic-end: stores homedepot L3-RENDERED len=2507` — was
`Akamai-sec-cpt-CHL` (2.6 KB interstitial, looping). The sec-cpt
bundles (560 KB + 425 KB) now fetch 200 OK and self-solve as sole
actor once the wrong BMP POST is suppressed (doc 20). §4 gate GREEN
with Inc 6+7.

- **Single-profile desktop (`chrome_130_macos`): 113 → 114/126**
  (homedepot now classifies `L3-RENDERED`, the sanctioned pass class).
- **Routed ceiling 120/126:** homedepot was NOT in the 6 universal
  blocks (those are canadagoose/hyatt/realtor/udemy/douyin/
  wildberries), so the *routed* union is unchanged at 120; but
  homedepot is one of the directive's named engine-addressable hard
  sites and is now flipped — the single-profile count rises and the
  hard set shrinks. A full `BOXIDE_PROFILE=chrome_130_macos cargo test
  --release -p browser --test holistic_sweep holistic_sweep_parallel
  -- --ignored --nocapture` (≈70 min) is the recommended confirmation
  of the exact new single-profile total; the targeted sanctioned
  measurement already confirms the flip itself.
- **Rigor caveat:** `len=2507` = sec-cpt challenge cleared + RENDERED
  per the sanctioned metric (no challenge marker, ≥1000 B), but a
  post-sec-cpt intermediate page, not the full multi-MB homepage —
  flipped for ceiling purposes, content-depth refinement is follow-up.

etsy/tripadvisor remain the DataDome WASM-iframe-daily-key L endgame
(unchanged — see master plan §8.5). Gate green at HEAD (`chrome_compat` 437/0, `v8_natives`
11/11, `iframe_isolation` 5/5, `v8_inspector_parity` 3/3, +8/8
`datadome_handler`). **Do not re-run the full sweep to "check
progress."** Re-run the **full** sweep **only** when a site-flipping
change is confirmed by a targeted live check first, then update this
table + the date. Re-measure command:

```bash
BOXIDE_PROFILE=chrome_130_macos cargo test --release -p browser \
  --test holistic_sweep holistic_sweep_parallel -- --ignored --nocapture
```

## Bottom line

126/126 corpus is fully accounted for: **120 open under routing**, 6
universally blocked (4 engine/IP — Kasada×3 + udemy — 2 region-locked).
The path to lift the ceiling is master-plan Phase 5 (in-engine vendor
bundle self-solve); Kasada×3 is the holistic tail with no single lever
(do not re-chase — Phase 2 OUTCOME A). This file is the consistent
progress record — no full re-sweep needed until a site-affecting commit.
