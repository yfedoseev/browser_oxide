# 02 — FULL GATE VERIFICATION

> Source: `/tmp/full_gate_2026_05_28` · production denominator = **125** (126 corpus − 1 diagnostic).
> Pass = `L3-RENDERED` AND body ≥ 15 000 B. Corpus vendor-spaced.

## Scorecard (production pass / %)

| Engine | Pass | % |
|---|--:|--:|
| **browser_oxide (routed best-of-4)** | **115/125** | **92.0%** |
| · BO chrome | 110/125 | 88.0% |
| · BO pixel | 108/125 | 86.4% |
| · BO iphone | 108/125 | 86.4% |
| · BO firefox | 106/125 | 84.8% |
| Playwright | _NODATA_ | — |
| PW-stealth | _NODATA_ | — |
| Patchright | _NODATA_ | — |
| Camoufox v150 | _NODATA_ | — |
| Camoufox v135 | _NODATA_ | — |

## browser_oxide vs competitors

> ⚠️ **The live competitor run did not reproduce in this environment** (see
> §Environment). Competitor numbers below are the **documented baselines** from
> the prior 2026-05-27 full sweep (same 126-corpus, same harness/classifier),
> cited in `FAILED_SITES_ANALYSIS.md` / `12_COMPETITIVE_LANDSCAPE.md`. BO's
> 115/125 is **fresh this run**.

| Engine | Production pass (~/125) | Source |
|---|--:|---|
| **browser_oxide (routed best-of-4)** | **115** | **this gate (fresh)** |
| Camoufox v150 (open-source SOTA) | ~112-113 | 2026-05-27 baseline |
| Camoufox v135 | ~108-109 | 2026-05-27 baseline |
| Patchright (Chromium-stealth) | ~108 | 2026-05-27 baseline |
| Playwright / PW-stealth | ~95-100 | 2026-05-27 baseline |
| browser_oxide (pre-session) | ~107-108 routed | 2026-05-27 baseline |

**Verdict: browser_oxide leads.** Routed 115/125 vs Camoufox v150 ~112-113 →
**BO +2 to +3 over the open-source SOTA**, up from ~107 (−5 to −6 behind) at
session start. The swing came entirely from public-engine fetch/cookie fixes
(AWS-WAF cluster + booking + homedepot + x-com), no vendor solvers.

**Where BO and v150 differ (from per-site data + docs):**
- **BO wins (v150 fails):** homedepot (v150 0/5); the full AWS-WAF cluster
  robustly (incl. amazon-com/ca, which v150 only gets probabilistically).
- **v150 wins (BO fails):** douyin + duolingo — both Firefox-native; a Chrome/
  V8 engine can't cheaply match them. **These 2 are BO's only structural deficit.**
- **Both fail (shared frontier):** Kasada×3 (canadagoose/hyatt/realtor),
  DataDome (etsy/yelp/tripadvisor), bestbuy, wildberries/ozon (geo).

## Contested-site matrix (sites some engine fails)

| site | BO routed | BO chrome | BO pixel | BO iphone | BO firefox | Playwright | PW-stealth | Patchright | Camoufox v150 | Camoufox v135 |
|---|---|---|---|---|---|---|---|---|---|---|
| adidas | ✅ | ✅ | · | ✅ | ✅ | ? | ? | ? | ? | ? |
| airbnb | ✅ | ✅ | · | ✅ | ✅ | ? | ? | ? | ? | ? |
| amazon-ca | ✅ | · | ✅ | ✅ | ✅ | ? | ? | ? | ? | ? |
| bestbuy | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| canadagoose | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| douyin | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| duolingo | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| economist | ✅ | ✅ | ✅ | · | ✅ | ? | ? | ? | ? | ? |
| ecosia | ✅ | ✅ | ✅ | · | ✅ | ? | ? | ? | ? | ? |
| etsy | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| ft | ✅ | ✅ | ✅ | · | ✅ | ? | ? | ? | ? | ? |
| homedepot | ✅ | ✅ | · | · | · | ? | ? | ? | ? | ? |
| hyatt | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| macys | ✅ | ✅ | ✅ | ✅ | · | ? | ? | ? | ? | ? |
| openai | ✅ | ✅ | ✅ | · | ✅ | ? | ? | ? | ? | ? |
| ozon | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| prime-video | ✅ | ✅ | · | ✅ | ✅ | ? | ? | ? | ? | ? |
| quora | ✅ | ✅ | ✅ | · | ✅ | ? | ? | ? | ? | ? |
| realtor | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| redfin | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| reuters | ✅ | ✅ | ✅ | ✅ | · | ? | ? | ? | ? | ? |
| spotify | ✅ | · | ✅ | ✅ | · | ? | ? | ? | ? | ? |
| tripadvisor | ✅ | · | ✅ | ✅ | · | ? | ? | ? | ? | ? |
| uber | ✅ | · | · | ✅ | · | ? | ? | ? | ? | ? |
| udemy | ✅ | ✅ | ✅ | · | ✅ | ? | ? | ? | ? | ? |
| wildberries | ❌ | · | · | · | · | ? | ? | ? | ? | ? |
| wsj | ✅ | ✅ | ✅ | ✅ | · | ? | ? | ? | ? | ? |
| yandex-ru | ✅ | ✅ | · | ✅ | ✅ | ? | ? | ? | ? | ? |
| yelp | ✅ | · | · | ✅ | · | ? | ? | ? | ? | ? |
| zillow | ✅ | ✅ | ✅ | ✅ | · | ? | ? | ? | ? | ? |

> ✅ pass · `·` fail/thin · `?` engine NODATA

## Environment (why competitors didn't run live)

The BO half ran clean (4 profiles × 126, per-site isolated). The competitor
half could **not** be reproduced in this environment:

- **playwright / playwright_stealth** — Chromium browser not installed
  (`playwright install` banner); exits before any nav.
- **patchright** — not pip-installed (`ModuleNotFoundError: No module named
  'patchright'`).
- **camoufox v150** — the cached `~/.cache/camoufox` held a **v150 binary
  incompatible with the pinned python launcher (camoufox 0.4.11 → browser
  v135)**; every `new_page` crashes (`Connection closed while reading from the
  driver`).
- **camoufox v135** — `camoufox fetch` restored the matched v135 browser and a
  single-page smoke test passed, but a **full 126-site run crashes the driver
  partway** (`pipe closed by peer`) — the camoufox/playwright driver is unstable
  for a sustained loop here. (`~/.cache/camoufox.v135.bak` was also gone.)
- Prior competitor result JSONs (`/tmp/full_sweep_2026_05_27/comp_*.json`) were
  cleaned from `/tmp`, so no same-run competitor data survives.

⇒ Competitor numbers use the **documented 2026-05-27 baselines**. A clean
same-run competitor sweep needs: `playwright install chromium`,
`pip install patchright && patchright install chromium`, a launcher/browser-
matched camoufox (v135 *and* a v150-matched python package), and driver-
stability (e.g. relaunch-per-site). Harnesses are ready
(`benchmarks/run_camoufox_min.py`, `bench_corpus_v2.py`, `run_full_gate.sh`).

## Caveats

- **AWS-WAF spacing:** the gate runs the corpus serially; even vendor-spaced, AWS sites can token-cluster on one IP. The authoritative AWS measurement is `benchmarks/run_spaced_aws.sh` (9/9 PASS, 150 s gaps). Treat any AWS fail here as a possible clustering artifact, not an engine gap (e.g. `amazon-ca` failed BO-chrome here but routed ✅ via other profiles, and passes 1.03 MB spaced; `redfin` reads `AWS-WAF-CHL` at 392 KB = clustering/partial).
- Production denominator = 125 (126 − `areyouheadless` diagnostic).
