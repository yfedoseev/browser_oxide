# Benchmark — anti-bot corpus rendering

BrowserOxide is measured against a **126-site corpus** of commercially-protected
pages (Cloudflare, Akamai, DataDome, PerimeterX, Kasada, Shape/F5, AWS WAF, …),
release build, same machine, same IP, same hour, classified by the engine's own
`browser_oxide::engine_classify`. **These numbers are from the vendor-stripped
open-source engine — no per-vendor bypass code in the tree** (see
[SCOPE.md](../SCOPE.md) and the "Challenge solving" section of the README).

## How a "pass" is scored (read this first)

There are two gates, and they disagree by ~4 sites. We report the **strict** one.

- **Strict — `≥15 KB` of real rendered content.** This is the honest metric and
  the headline below. It matches `ChallengeVerdict::Pass` in the engine's own
  audit harness.
- **Loose — the `L3-RENDERED` tag alone (any size).** This over-counts: ~10–15
  corpus sites are SPA bootstraps that ship a 2–13 KB shell to *any* HTTP client.
  They get tagged `L3-RENDERED` by the *absence* of a challenge marker even though
  no real render happened (e.g. `duolingo` returns a 13.5 KB shell — it is **not**
  a pass). Earlier versions of this file quoted the loose tag and were inflated by
  ~4 sites; those numbers are retracted.

Neither gate is perfect: the strict gate also has a few **false negatives** — a
site whose genuine full page is small (e.g. `areyouheadless.com` renders a real
3.6 KB results page) fails the `≥15 KB` cut despite rendering correctly.

## Result (latest full 4-profile cleanroom run, 2026-06-05, post deno_core 0.403 / V8-149)

| Profile | Rendered / 126 (`≥15 KB`) | loose `L3` tag |
|---|--:|--:|
| `chrome_148_macos` | **115** | 119 |
| `firefox_135_macos` | **113** | 117 |
| `pixel_9_pro_chrome_148` | **114** | 117 |
| `iphone_15_pro_safari_18` | **118** | 121 |
| **best-of-4 routed** | **119** | 122 |

"Routed" = the caller picks the best profile per domain, which most real scraping
pipelines do naturally (the `≥15 KB` routed union covers 119/126).

This run is on the **deno_core 0.403 / V8 149** engine. The migration initially
regressed two Akamai sites; tracing from the last-passing pre-deno commit found
and fixed the cause (see *Engine note* below), recovering both:
**`adidas` (2.5 KB stub → 1.52 MB)** and **`homedepot` (TIMEOUT → ~1.1 MB)** now
pass via routing — net **+2 routed** over the deno-0.403 baseline (117) and **+1**
over the pre-deno baseline (118), with **zero regressions**.

## The hard residual

**Six** sites returned no real content on **any** profile in this run:

| Site | Protection | Why |
|---|---|---|
| `canadagoose.com` | Kasada | no OSS tool publicly passes Kasada from scratch (challenge times out) |
| `hyatt.com` | Kasada | same |
| `realtor.com` | Kasada | `ScriptChallenge-CHL` interstitial, all profiles |
| `etsy.com` | DataDome | interactive Device-Check / human-gate (out of scope) |
| `duolingo.com` | (CSR SPA) | renders a ~13.5 KB shell; full client render not reached |
| `wildberries.ru` | WBAAS | reaches storefront intermittently; ~1.8 KB interstitial this run |

Three sites that look like fails but aren't hard blocks:
- **`adidas.com` passes** via routing — `chrome` rendered the full 1.52 MB
  storefront this run; other profiles caught the 2.5 KB Akamai interstitial on a
  bad risk-roll. **Profile-split + flaky**, not a hard fail.
- **`homedepot.com` passes** via routing — it's an Akamai **sec-cpt** site whose
  in-page proof-of-work lands ~131 s on our V8; with the env-configurable sec-cpt
  budget (`BROWSER_OXIDE_SECCPT_BUDGET_MS`, default 140 s) it renders ~1.1 MB on a
  workable roll (it did here on multiple profiles). Like `adidas` it's
  **±risk-roll**: on a `9l 773` stub roll Akamai rejects the sensor regardless of
  budget. Give the sweep enough per-site wall-clock (≥200 s) so the full ~185 s flow is not
  truncated (see [CONFIGURATION.md](CONFIGURATION.md)).
- **`areyouheadless.com`** renders correctly (`L3-RENDERED`, 3.6 KB) but its real
  page is below the 15 KB strict gate — a gate false-negative, not a block.

`adidas` and `homedepot` are both **flaky Akamai** (±risk-roll): on any given
cleanroom run, one or both may flip pass↔fail (a parallel re-run the same night
landed `homedepot` on a stub roll on 3/4 profiles). Routed `118–119` is the
central tendency, not a guaranteed per-run figure.

### Engine note — deno_core 0.403 / V8-149 migration

The migration regressed `adidas`/`homedepot` (both passed pre-deno). Tracing
from the last-passing commit found two causes, both fixed: (1) the op2 macro
forced `op_fetch`/`op_timer_sleep` to `async(lazy)`, which stopped eager-polling
and shifted the async-settle cadence the Akamai sensor measures — restored with
`async(deferred)`; (2) `performance.memory` returned an unbucketed, per-call-
varying heap value (a bot-tell no real Chrome emits) — now quantized to 100 KB
and stable per page. A `document.currentScript`/`.src`-absolute DOM-parity fix
(needed for sec-cpt bundles to locate their own script) and the generic
sec-cpt budget round it out. IP is confirmed clear for both sites (real Chrome
and camoufox both render them from the test IP) — these are engine fixes, not
proxy changes.

## Caveats

- **Anti-bot responses are noisy.** Single sweep runs vary by ±5 sites from WAF
  lottery alone — re-testing per-site 3× shows some endpoints (e.g. amazon variants)
  have ~1-in-3 pass rate per fetch. The numbers above are the central tendency of a
  full cleanroom run, not a guaranteed per-run result. Space same-IP, same-vendor
  calls when reproducing, or token-clustering produces false failures.
- **Re-measure per release.** These are point-in-time numbers; live sites and
  their defenses change. Treat the table as "what a fresh cleanroom sweep produced
  on 2026-06-03," not a standing guarantee.
- Profile labels reflect the actual emitted User-Agent; all presets ship a current
  Chrome 148 / Firefox 135 / Safari 18 identity.

## Reproduce

```bash
# one profile over a corpus JSON (records classifier tag + body len per site):
cargo run --release -p browser_oxide --example sweep_metrics -- chrome_148_macos corpus.json out.json
# score honestly — count results with len >= 15000, not the L3-RENDERED tag alone.
```

`corpus.json` is a JSON array of `{ "cat", "name", "url" }` entries. The
example renders the list in one process; for a long cold sweep, isolate each
site in a fresh process if you hit memory pressure.

Per-page wall-clock and memory characteristics are summarised in the README
("Per-page performance"): a single Rust process keeps resident memory in the tens
of MB versus a Chrome-over-CDP driver.
