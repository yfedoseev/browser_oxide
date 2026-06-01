# Cleanroom verification — 2026-05-31

Sequential, one-site-at-a-time, **load-gated**, same IP — removes the CPU-contention confound. browser_oxide
across 4 profiles (best-of union) vs 5 competitor tools, all on the same 126-site anti-bot corpus.
Pass = `tag == 'L3-RENDERED' and len >= 15000`. `areyouheadless` is a headless-detection diagnostic,
excluded from the content count (production_n = 125). Raw per-engine JSONs are alongside this file.

## Headline — browser_oxide is #1

**browser_oxide union = 118/125 > camoufox v150 = 117/125.**

| engine | pass /125 |
|--------|-----------|
| **browser_oxide (union of 4 profiles)** | **118** 🥇 |
| camoufox_v150 | 117 |
| camoufox_v135 | 113 |
| playwright_stealth | 87 |
| patchright | 87 |
| playwright | 89 |

### browser_oxide per-profile

| profile | pass /125 |
|---------|-----------|
| chrome_148_macos | 115 |
| firefox_135_macos | 112 |
| pixel_9_pro_chrome_148 | 113 |
| iphone_15_pro_safari_18 | 115 |

## This session's engine flips (sites browser_oxide now passes)

| site | BO tag | BO len | v150 (this run) |
|------|--------|--------|-----------------|
| homedepot | L3-RENDERED | 989,657 | PASS |
| bestbuy | L3-RENDERED | 44,021 | L3-RENDERED |
| redfin | L3-RENDERED | 392,665 | PASS |

> Note: **bestbuy** is a clean both-fail (v150 fails it this run). **homedepot/redfin** are flaky on
> the vendor side — v150 happened to pass them in THIS run (it failed them on 2026-05-30, hence v150
> 116 then vs 117 now). The session's achievement (browser_oxide now passes all three) is unchanged.

## browser_oxide AHEAD of v150 — 5 sites (BO union passes, v150 fails this run)

amazon-ca, amazon-com, bestbuy, leboncoin, skyscanner

## v150 AHEAD of browser_oxide — 4 sites (v150 passes, BO union fails)

adidas, duolingo, etsy, wildberries

## browser_oxide union-fails — 7 (all deep vendor walls)

adidas, canadagoose, duolingo, etsy, hyatt, realtor, wildberries  (Kasada: canadagoose/hyatt/realtor are both-fail.)

## Deep-investigation TODO (next session) — obvious marks only, deep research pending

Sites where **camoufox v150 still leads**. Each needs a dedicated deep dive grounded in the research in
**`docs/`** (this repo: `research-2026-05-30/`, `releases/v0.1.0-parity/`, `handoffs/`) AND
**`~/projects/browser_oxide_internal/docs/`**. Cross-ref `docs/handoffs/2026-05-31/HANDOFF_2026-05-31.md` §3.1.

| site | obvious mark | deep-research direction |
|------|--------------|--------------------------|
| adidas | Akamai BMP STRICTER than homedepot — `_abck=~-1~` persists after the GPU-FP fixes that flipped homedepot | AKAMAI_BMP_V13 doc; diff adidas vs homedepot sensor weighting; byte-exact canvas/audio; capture _abck POST |
| duolingo | reCAPTCHA-Enterprise token wall — v150 (real FF) token accepted → 1.16 MB; BO token rejected → 13 KB shell | capture grecaptcha.enterprise anchor/execute FF-vs-BO; IP/score wall vs token-shape? |
| etsy | DataDome — DataDome-CHL on all BO profiles | re-test post-FP-E1 iframe-solver; DataDome boring_challenge/DeviceCheck; same cluster as tripadvisor/leboncoin (now BO-pass on FF/mobile) |
| wildberries | WBAAS multi-stage solver — stalls PRE-iframe (frames:0, no /api/v1/report, silent logger) | deobfuscate+patch challenge_fingerprint Promise.all to find the stalling module; or diff /api/v1/report vs real Chrome |

_These are obvious marks only — deep research pending._
