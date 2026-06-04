# Benchmark — anti-bot corpus rendering

browser_oxide is measured against a **126-site corpus** of commercially-protected
pages (Cloudflare, Akamai, DataDome, PerimeterX, Kasada, Shape/F5, AWS WAF, …),
release build, same machine, same IP, same hour, classified by the engine's own
`browser::engine_classify`. **These numbers are from the vendor-stripped
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

## Result (latest full 4-profile cleanroom run, 2026-06-03)

| Profile | Rendered / 126 (`≥15 KB`) | loose `L3` tag |
|---|--:|--:|
| `chrome_148_macos` | **114** | 118 |
| `firefox_135_macos` | **111** | 115 |
| `pixel_9_pro_chrome_148` | **114** | 118 |
| `iphone_15_pro_safari_18` | **118** | 121 |
| **best-of-4 routed** | **118** | 121 |

"Routed" = the caller picks the best profile per domain, which most real scraping
pipelines do naturally (the `≥15 KB` routed union covers 118/126).

## The hard residual

**Seven** sites returned no real content on **any** profile in this run:

| Site | Protection | Why |
|---|---|---|
| `canadagoose.com` | Kasada | no OSS tool publicly passes Kasada from scratch (challenge times out) |
| `hyatt.com` | Kasada | same |
| `realtor.com` | Kasada | `Kasada-CHL` interstitial, all profiles |
| `etsy.com` | DataDome | interactive Device-Check / human-gate (out of scope) |
| `duolingo.com` | (CSR SPA) | renders a ~13.5 KB shell; full client render not reached |
| `wildberries.ru` | WBAAS | reaches storefront intermittently; ~1.8 KB interstitial this run |
| `homedepot.com` | Akamai (sec-cpt) | **flaky** — times out on a bad risk-roll (has rendered in other runs) |

Two sites that look like fails but aren't hard blocks:
- **`adidas.com` passes** via routing — `chrome` (1.5 MB) and `iphone` (1.38 MB)
  render the full storefront; `firefox`/`pixel` stay at the 2.5 KB Akamai
  interstitial. It's **profile-split + flaky**, not a hard fail. (The firefox
  miss is a side effect of coherent Firefox-TLS impersonation — Akamai's firefox
  bot-model catches it where the old `UA=Firefox + TLS=Chrome` edge case slipped
  through. Correct global tradeoff; chrome/iphone routing still wins the site.)
- **`areyouheadless.com`** renders correctly (`L3-RENDERED`, 3.6 KB) but its real
  page is below the 15 KB strict gate — a gate false-negative, not a block.

`adidas` and `homedepot` are both **flaky Akamai** (±risk-roll): on any given
cleanroom run, one or both may flip pass↔fail. Routed `118` is the central
tendency, not a guaranteed per-run figure.

## Caveats

- **Anti-bot responses are noisy.** Single sweep runs vary by ±5 sites from WAF
  lottery alone — re-testing per-site 3× shows some endpoints (e.g. amazon variants)
  have ~1-in-3 pass rate per fetch. The numbers above are the central tendency of a
  full cleanroom run, not a guaranteed per-run result. Space same-IP, same-vendor
  calls when reproducing, or token-clustering produces false failures.
- **Re-measure per release.** These are point-in-time numbers; live sites and
  their defenses change. Treat the table as "what a fresh cleanroom sweep produced
  on 2026-05-31," not a standing guarantee.
- Preset constructors `chrome_130_*` / `pixel_9_pro_chrome_147` are deprecated
  aliases that emit a current Chrome 148 UA — the profile labels reflect the actual
  emitted User-Agent.

## Reproduce

```bash
cargo build --release -p browser --example sweep_metrics
# one profile over the corpus (per-site isolated processes; records body len):
python3 benchmarks/run_bo_isolated.py chrome_148_macos benchmarks/corpus.json out.json
# score honestly — count results with len >= 15000, not the L3-RENDERED tag alone.
```

Per-page wall-clock and memory characteristics are summarised in the README
("Per-page performance"): a single Rust process keeps resident memory in the tens
of MB versus a Chrome-over-CDP driver.
