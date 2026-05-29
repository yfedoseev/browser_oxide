# v0.1.0 — Per-profile consistency match workflows

**Headline:** browser_oxide hit **115/125 routed-best-of-4** on the full
gate (2026-05-29) — it now **leads Camoufox v150** (~112–113 single-engine).
But the routed union hides **per-profile variance**: each of the four BO
profiles passes a *different* subset, so no single profile reaches the 115.

| profile | pass/125 |
|---|--:|
| chrome_148_macos | 110 |
| pixel_9_pro_chrome_148 | 108 |
| iphone_15_pro_safari_18 | 108 |
| firefox_135_macos | 106 |
| **routed best-of-4** | **115** |

- **95** sites pass on all 4 profiles.
- **20** sites are **consistency gaps** (pass some profiles, fail others) —
  this is the work tracked in these docs.
- **10** sites fail all 4 (the *frontier*: bestbuy, canadagoose, douyin,
  duolingo, etsy, hyatt, ozon, realtor, redfin, wildberries) — **out of
  scope here**.

**The consistency target:** every individual profile should reach **~115**,
matching/beating v150's ~112–113 per-profile. Closing the 20 consistency
gaps lifts each profile to ~110–116 and turns 20 routed-fallback wins into
all-four-pass wins.

**The unifying root cause** (cluster 07): the gaps are not 20 independent
site bugs. They collapse onto a small number of **per-profile coherence
defects** — places where a profile's UA tells one story and its TLS / HTTP-2
/ WebGL fingerprint tells another (the #1 signal every 2026 anti-bot vendor
weights), plus a separate **nav-reliability** track. All fixes are
**public-engine** (in `crates/net` / `crates/stealth` per `CLAUDE.md`); no
`vendor_solvers` bypass code.

---

## Document index

| # | File | Scope | Profile(s) | Lever |
|---|---|---|---|---|
| 00 | `00_DATA_per_profile_matrix.md` | Ground-truth per-site × per-profile tag+len matrix (the data) | all | — |
| 00 | `00_README.md` | This index + the headline | all | — |
| 01 | `01_IPHONE_CLOUDFLARE.md` | iphone loses 6 sites to Cloudflare-CHL | iphone | iOS Safari TLS cipher-SET fix |
| 02 | `02_FIREFOX_DATADOME_PX.md` | firefox loses reuters/wsj/tripadvisor (DataDome) + zillow (PX) + macys | firefox | real Firefox TLS+H2 wire class |
| 03 | `03_PIXEL_NAV_ERRORS.md` | pixel THIN-BODY/ERROR on adidas/airbnb/yandex-ru/prime-video | pixel | mobile fetch UA-CH coherence |
| 04 | `04_DESKTOP_DATADOME_TIMEOUT.md` | desktop DataDome (spotify/tripadvisor/yelp) + uber TIMEOUT | chrome, firefox, pixel | uber nav-budget + Firefox wire |
| 05 | `05_HOMEDEPOT_SECCPT_CONSISTENCY.md` | homedepot passes chrome only; Akamai-CHL on the other 3 | pixel, iphone, firefox | challenge-aware drain + Firefox wire |
| 06 | `06_AMAZONCA_GATE_CLUSTERING.md` | amazon-ca chrome-only fail = AWS-WAF token-clustering measurement artifact | chrome | harness spacing policy (no engine code) |
| 07 | `07_PRESET_COHERENCE_AUDIT.md` | cross-cutting coherence audit: D1/D2/D3 unify clusters 01/02 | all | the unifying root cause |
| 08 | `08_MASTER_CONSISTENCY_ROADMAP.md` | the consolidated ROI-ranked roadmap + phased plan + projections | all | **start here for the plan** |

---

## Quick read order

1. `00_DATA_per_profile_matrix.md` — see exactly which site fails which profile.
2. `08_MASTER_CONSISTENCY_ROADMAP.md` — the ranked plan and per-phase projections.
3. `07_PRESET_COHERENCE_AUDIT.md` — why the gaps share a small set of root causes.
4. The individual cluster docs (01–06) for code-level detail per cluster.
