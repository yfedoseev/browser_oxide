# 03 — Benchmark methodology

This chapter is the operating manual for every Pass number you see in
this release plan. It defines: the 126-site corpus, the classifier,
the four BO profiles, how the BO sweep runs, how competitor sweeps
run, the noise floor, and the **multi-run aggregation** that all
v0.1.0 numbers must be measured under.

> Read first: `01_CURRENT_STATE.md` (the baseline), `02_GAP_ANALYSIS.md`
> (the 10 sites we need to recover). This chapter assumes you understand
> what "Pass" means and why we measure it.

## 1. The 126-site holistic corpus

The corpus is defined once, in Rust source, and projected into JSON for
the Python competitor harness. Source of truth:
`crates/browser/tests/holistic_sweep.rs:714-862` (the `sites_list()`
function returns the 126-tuple list used by the parallel sweep — the
serial `site!(...)` macros above it (`crates/browser/tests/holistic_sweep.rs:198-707`)
define the same set as 126 individual `#[tokio::test]` cases).

### Category breakdown

| Category | n | What the sites stress | Examples |
|---|--:|---|---|
| search | 8 | TLS + UA basics | google, bing, brave, yandex |
| reference | 5 | Plain content, low protection | wikipedia, github, mdn, stackoverflow |
| news | 10 | Cloudflare, large HTML, big article bodies | bbc, cnn, ft, wsj, bloomberg |
| social | 10 | SPA hydration, login-walls, Akamai, Twitter WAF | reddit, x-com, linkedin, facebook |
| amazon | 8 | AWS WAF challenge.js + regional WAF risk-roll | amazon-{com,uk,de,fr,jp,in,ca,com-au} |
| stores | 17 | Akamai sec-cpt, DataDome, PerimeterX, sec-cpt-CHL | etsy, homedepot, walmart, wayfair, macys |
| streaming | 8 | Cloudflare, Akamai, fingerprint-keyed CDNs | netflix, hulu, disneyplus, spotify |
| travel | 8 | DataDome WASM-iframe, SPA hydration | booking, tripadvisor, kayak, skyscanner |
| realestate | 4 | Kasada (`realtor`), PerimeterX | realtor, zillow, redfin, trulia |
| tech | 9 | Cloud-marketing pages, low-protection | aws, azure, cloudflare, stripe, openai |
| ru | 6 | Russian WAFs (Yandex), wildberries SPA | yandex-ru, wildberries, ozon, vk |
| gov-bank | 6 | Akamai, F5 BIG-IP | irs, chase, bofa, wellsfargo, paypal |
| antibot | 10 | Diagnostic fingerprint probes | creepjs, sannysoft, areyouheadless, iphey |
| chl-known | 5 | Kasada + DataDome heavy hitters | canadagoose, hyatt, douyin, adidas, leboncoin |
| misc | 12 | Long tail (recaptcha, SPAs, varied WAFs) | imdb, yelp, duolingo, udemy, medium |

The categories are **not** vendor labels — `chl-known` overlaps Kasada
sites that also live in `realestate` etc. They group sites by what
*kind* of page they are, so per-category Pass numbers stay meaningful.

### Corpus JSON for the Python harness

The competitor harness reads JSON from `$CORPUS_FILE` (default
`/tmp/corpus.json`, see `benchmarks/bench_corpus_v2.py:26`). Shape:

```json
[
  {"cat": "search", "name": "google", "url": "https://www.google.com/"},
  {"cat": "amazon", "name": "amazon-de", "url": "https://www.amazon.de/"}
]
```

**RUN THIS** to regenerate `/tmp/corpus.json` from the Rust source of
truth (single command, mechanical):

```bash
# Extract sites_list() entries into JSON. The function is at
# crates/browser/tests/holistic_sweep.rs:714-862.
python3 - <<'PY' > /tmp/corpus.json
import re, json, pathlib
src = pathlib.Path("crates/browser/tests/holistic_sweep.rs").read_text()
m = re.search(r'fn sites_list\(\).*?vec!\[(.*?)\]\s*\}', src, re.S)
entries = re.findall(r'\("([^"]+)",\s*"([^"]+)",\s*"([^"]+)"\)', m.group(1))
print(f"// {len(entries)} sites")
json.dump([{"cat": c, "name": n, "url": u} for c, n, u in entries],
          open("/tmp/corpus.json", "w"), indent=1)
PY
jq 'length' /tmp/corpus.json  # must print 126
```

## 2. The classifier — one source of truth

Both BO (`sweep_metrics`) and the competitor harness pipe the final
rendered body through the **same** Rust function:
`browser::engine_classify` in `crates/browser/src/classify.rs:199-235`.

The competitor harness invokes it via the `classify_stdin` binary
(`crates/browser/examples/classify_stdin.rs`, called from
`benchmarks/bench_corpus_v2.py:34-45`). This eliminates the
double-classifier divergence that produced the FP-B1 measurement bug
(see the module doc at `crates/browser/src/classify.rs:1-32`).

### Verdicts

`engine_classify` returns a `(tag, len, verdict)` triple. The sweep
harnesses care about `tag` and `len`. The five aggregate buckets
defined by `sweep_metrics.rs:202-218` and
`bench_corpus_v2.py:290-294`:

| Bucket | Rule | Meaning |
|---|---|---|
| **Pass** (strict) | `tag == "L3-RENDERED" && len >= 15000` | A page that *both* rendered to L3 *and* has a non-shell body. The customer-relevant number. |
| **L3-RENDERED** (loose) | `tag == "L3-RENDERED"` | Rendered without a challenge marker, ANY size. Inflated by SPA shells. |
| **ThinShell** | `tag == "L3-RENDERED" && 1000 <= len < 15000` | Rendered but body too small to be the real page (SPA pre-hydration). |
| **CHL** | `tag.contains("CHL") OR tag == "BLOCKED" OR tag.contains("PaH")` | Vendor-specific challenge document. |
| **ThinBody** | `tag != "L3-RENDERED" && len < 1000 && err.is_none()` | Empty redirect dead-end. |
| **Error** | `err.is_some()` | Engine raised. |

### The marker tables (gotchas)

The classifier reads three needle lists, in order:

1. **UNAMBIGUOUS** (`classify.rs:81-86`) — any-size structural URL
   tokens. `cf-browser-verification`, `_cf_chl_opt`,
   `/_sec/cp_challenge`, `ddcaptchaencoded`. These NEVER appear in a
   rendered page.
2. **PHRASE** (`classify.rs:91-97`) — English-interstitial phrases.
   Only consulted when `len < INTERSTITIAL_MAX_BYTES = 30 KB`
   (`classify.rs:37`). `just a moment`, `checking your browser`,
   `captcha-delivery.com`, `press &amp; hold`, `pardon our interruption`.
3. **SMALL_BODY** (`classify.rs:102-118`) — vendor SDK tokens that
   ALSO appear on rendered pages. Only consulted when `len < 30 KB`.
   `akam/13`, `_abck`, `_kpsdk`, `ips.js`, `_pxhd`, `px-captcha`,
   `captcha`, `403 forbidden`, `access denied`.

Two SMALL_BODY rows are further co-signal-gated (`classify.rs:160-168`):

- `akam/13` requires one of `sensor_data`, `bm-verify`,
  `sec-if-cpt-container`, `sec-cpt-if`, `/_sec/cp_challenge`,
  `pardon our interruption`. Without a co-signal, the bare Akamai
  bootstrap is NOT a challenge (the historical bestbuy i18n-splash FP,
  `classify.rs:120-134`).
- `captcha` requires an interactive captcha co-signal:
  `api2/bframe`, `api2/anchor`, `hcaptcha.com`, `cf-turnstile`,
  `i'm not a robot`, `verify you are human`, etc. Without one, the bare
  word is invisible reCAPTCHA-v3 plumbing (the spotify/duolingo FP,
  `classify.rs:136-156`).

### Gotchas to remember

- **`L3-RENDERED loose` is not Pass.** Camoufox and BO both score
  **120 loose** on the 2026-05-24 sweep; the headline gap is 5 strict.
  Loose includes the 12 BO ThinShells that strict rejects.
- **A 2011-byte AWS WAF stub** classifies as `L3-RENDERED` (no challenge
  marker fires) but `len < 15000` → ThinShell, not Pass. The strict
  gate is what catches it. Do not regress to "loose L3 count" as the
  metric.
- **`reuters` shape**: 1.1 MB rendered page that contains the phrase
  `just a moment` in body copy. The 30 KB phrase gate prevents it from
  being miscounted as Cloudflare-CHL
  (`crates/browser/tests/holistic_sweep.rs:948-962`).
- **`wayfair` shape**: 1 MB rendered page that contains the literal
  `px-captcha` inside a cookie-consent JSON manifest. The 30 KB gate on
  the relocated SMALL_BODY row prevents the PerimeterX-CHL FP
  (`classify.rs:108-114`, regression test
  `classify.rs:356-390`).
- **`udemy` shape**: large CF orchestrator shell (`_cf_chl_opt`
  present, ≥ 50 KB body) classifies as `Cloudflare-CHL` with verdict
  `ChallengeIncomplete`, NOT `SensorFail`
  (`classify.rs:183-192`, regression test
  `classify.rs:400-438`).

## 3. The four BO profiles

Each profile is constructed by a `stealth::presets::*` function in
`crates/stealth/src/presets.rs`. The presets map to YAML fixtures in
`crates/stealth/profiles/*.yaml` for the load-from-file path
(`StealthProfile::load_from_file`).

| Profile | preset fn | UA | sec-ch-ua-platform | tls_impersonate codename |
|---|---|---|---|---|
| **chrome_148_macos** | `presets.rs:120-196` | `Chrome/148.0.0.0` on `Macintosh; Intel Mac OS X 10_15_7` | `"macOS"` | `chrome_147` |
| **firefox_135_macos** | `presets.rs:413-495` | `Firefox/135.0` on `Macintosh; Intel Mac OS X 14.5` | (no UA-CH) | `firefox_135` |
| **pixel_9_pro_chrome_148** | `presets.rs:690-793` | `Chrome/148.0.0.0 Mobile` on `Linux; Android 15; Pixel 9 Pro` | `"Android"` + Mobile=?1 | `chrome_147_android` |
| **iphone_15_pro_safari_18** | `presets.rs:795-875` | `Safari/604.1 Version/18.0.1` on `iPhone; CPU iPhone OS 18_0_1` | (no UA-CH) | `safari_18_ios` |

Per-profile details that matter for diagnosis:

### chrome_148_macos (`presets.rs:120-196`)
- UA reports the **reduced** version `148.0.0.0` (Chrome UA-reduction
  since v110, see comment at `presets.rs:115-118`). The full version
  `148.0.7778.168` is only exposed via
  `sec-ch-ua-full-version-list`.
- Viewport 1512×982, DPR 2.0, 8 cores, 8 GB device memory.
- WebGL: `Google Inc. (Apple)` / `ANGLE (Apple, ANGLE Metal Renderer:
  Apple M3, Unspecified Version)`.
- `enforce_csp: true` (`presets.rs:122`). CSP `connect-src` enforcement
  in fetch_ext (`fetch_ext.rs:240-260`); override with
  `BROWSER_OXIDE_CSP_BYPASS=1`.

### firefox_135_macos (`presets.rs:413-495`)
- WebGL masked to literal `"Mozilla"` / `"Mozilla"` (Firefox default
  since v113, `presets.rs:441-445`).
- `tls_impersonate: "firefox_135"` is informational; the actual TLS
  bytes are still Chrome-shape from `crates/net` via boring2. A real
  Firefox JA4 swap is a known gap (`presets.rs:457-463`).
- Viewport 1440×900, DPR 2.0, 10 cores, 16 GB.

### pixel_9_pro_chrome_148 (`presets.rs:690-793`)
- 412×870 viewport, **fractional DPR 2.625** (often a tell).
- `maxTouchPoints: 5`, `platform: "Linux armv81"`.
- `plugins_count: 0`, `mime_types_count: 0` — Android Chrome ships an
  empty plugins array (`presets.rs:738-743`). This is the single
  biggest desktop-vs-mobile tell on Chromium that anti-bot stacks key
  off.
- `cpu_architecture: ""` (empty per UA-CH reduction on Android).
- `has_platform_authenticator: false`.

### iphone_15_pro_safari_18 (`presets.rs:795-875`)
- 393×852 viewport, integer DPR 3.0.
- **`cpu_cores: 2`** — Safari intentionally caps `hardwareConcurrency`
  (`presets.rs:817-818`).
- **`device_memory: 0`** — iOS Safari does not expose `deviceMemory`;
  the JS bootstrap returns `undefined` regardless of the YAML value
  (`presets.rs:819-822`).
- Safari does NOT send `Sec-CH-UA-*` headers at all.
- GPU profile is still `apple_m3_macos` — TODO marker at
  `presets.rs:873` for a real iOS A17 Pro GPU profile.

## 4. How `sweep_metrics` runs

Binary: `crates/browser/examples/sweep_metrics.rs` (286 lines, single
file, no library deps beyond `browser` + `stealth`).

### Invocation

```bash
# RUN THIS — single profile, cold path, 126 sites.
cargo build --release -p browser --example sweep_metrics --example classify_stdin
target/release/examples/sweep_metrics \
    chrome_148_macos \
    /tmp/corpus.json \
    /tmp/bo_chrome_cold.json
```

### Modes

- **Cold path** (default): one fresh `Page::navigate` per site
  (`sweep_metrics.rs:156-168`). Each call builds a new V8 isolate
  (~50-70 MB, 200-300 ms cold spin-up).
- **Pool path** (`BROWSER_OXIDE_SWEEP_POOL=1`): `PagePool::new(4)` and
  `pool.navigate(&site.url, profile.clone())` per site
  (`sweep_metrics.rs:101-102, 141-155`). Isolates are reused across
  pages; ~14 pages/min on the 97-site partial 2026-05-24 measurement
  (`docs/PERFORMANCE_2026_05_24.md`).

### Relevant env vars (verified in source)

| Var | Default | What it does | Source |
|---|---|---|---|
| `BROWSER_OXIDE_SWEEP_POOL` | unset | Set to anything ⇒ use PagePool instead of cold `Page::navigate`. | `sweep_metrics.rs:101` |
| `BROWSER_OXIDE_NAV_BUDGET_MS` | 15000 | Per-iteration nav budget. Override for slow links / debug. | `page.rs:1744` |
| `BROWSER_OXIDE_NAV_BUDGET_EXTEND_MS` | 25000 | Additional headroom when heavy challenge flow detected. | `page.rs:1750` |
| `BROWSER_OXIDE_BUILD_BUDGET_MS` | (computed) | `build_page` total budget. | `page.rs:1435`, `3035` |
| `BROWSER_OXIDE_DEBUG_NAV` | unset | Set ⇒ verbose per-iteration `[debug-nav]` logs to stderr. | `page.rs:1037`, `1124` |
| `BROWSER_OXIDE_BUILD_PROFILE` | unset | Set ⇒ emit `[bp]` build-page phase timings. | `page.rs:2824` |
| `BROWSER_OXIDE_WARM_PROFILE` | unset | Trace warm-navigate path. | `page.rs:1227` |
| `BROWSER_OXIDE_CSP_BYPASS` | unset | Set ⇒ disable engine CSP enforcement. | `page.rs:433`, `1296`, `1581`; `fetch_ext.rs:77` |
| `BROWSER_OXIDE_EVENT_LOOP_PROFILE` | unset | Set ⇒ per-poll event-loop timing dump. | `event_loop/src/lib.rs:39` |
| `BROWSER_OXIDE_EVENT_LOOP_PROFILE_LABEL` | unset | Label prefix for the profile dump. | `event_loop/src/lib.rs:378` |
| `BROWSER_OXIDE_DEBUG_CHILD_REALM` | unset | Verbose child-realm install diagnostics. | `dom_ext.rs:1372-1379` |
| `BROWSER_OXIDE_DD_TRACE` | unset | DataDome handler tracing. | `page.rs:2908` |
| `BROWSER_OXIDE_SC_TRACE` | unset | SharedCookie sync tracing. | `page.rs:2916` |
| `BROWSER_OXIDE_PROFILE` | `chrome_148_macos` | Profile selector for the holistic_sweep test harness (NOT used by `sweep_metrics`, which takes the profile as argv[1]). | `holistic_sweep.rs:31-44` |
| `BROWSER_OXIDE_PARALLEL_WORKERS` | 2 | Worker count for the parallel test path. | `holistic_sweep.rs:1024` |
| `RUST_LOG` | (unset) | Standard `env_logger` filter. Useful settings: `js_runtime=trace,browser=debug`, `js_runtime::extensions::worker_ext=trace,info`. | tokio_subscriber/env_logger |

> **There is no `BROWSER_OXIDE_NO_SOLVERS` env var.** The public engine
> already calls `Page::default_solvers()` which returns an empty slice
> (`page.rs:848-852`). To wire vendor solvers, an embedder calls
> `Page::navigate_with_solvers(...)` explicitly
> (`page.rs:961-970`).

### Output JSON schema

Per `sweep_metrics.rs:264-269`, the JSON has two top-level keys:

```json
{
  "summary": {
    "engine": "browser_oxide",
    "profile": "chrome_148_macos",
    "mode": "cold",
    "n": 126,
    "pass": 99,
    "thin_shell": 17,
    "chl": 7,
    "thin_body": 5,
    "error": 0,
    "pass_pct": 78.6,
    "t_launch_ms": 0,
    "t_first_page_ready_ms": 0,
    "rss_peak_mb": 419.0,
    "ms_median": 15100,
    "ms_p95": 92900,
    "ms_p99": 115300,
    "wall_total_ms": 2760000,
    "throughput_pages_per_min": 2.7,
    "by_category": { "amazon": {"n": 8, "pass": 1}, "...": "..." }
  },
  "results": [
    {
      "cat": "search", "name": "google",
      "url": "https://www.google.com/",
      "tag": "L3-RENDERED", "len": 217341,
      "ms": 1832, "rss_mb": 312.0,
      "err": null
    }
  ]
}
```

## 5. How competitor sweeps run

Harness: `benchmarks/bench_corpus_v2.py` (368 lines). Engines
supported: `playwright`, `playwright_stealth`, `patchright`,
`camoufox`, `puppeteer`, `puppeteer_stealth`, `chromium`.

Critical implementation details:
- Per-page wall-clock measured around
  `page.goto(url, wait_until="load", timeout=NAV_TIMEOUT_MS)` followed
  by `page.wait_for_timeout(SETTLE_MS)`
  (`bench_corpus_v2.py:144-145`). Default `NAV_TIMEOUT_MS = 45000`,
  `SETTLE_MS = 2500` (`bench_corpus_v2.py:30-31`).
- Body retrieved via `page.content()` even on goto failure
  (`bench_corpus_v2.py:148-152`); ensures we see partial renders.
- Classified by piping HTML to the BO `classify_stdin` binary
  (`bench_corpus_v2.py:34-45`) — same `engine_classify` Rust function
  the BO sweep uses.
- RSS peak measured by walking the entire process tree from
  `browser.process.pid` via `/proc/*/stat` PPID chains
  (`bench_corpus_v2.py:48-106`). This is the post-fix tree-walker;
  the pre-fix version (first /proc child whose `comm` contained
  "fox") missed Firefox e10s content processes, producing the bogus
  48 MB Camoufox number (`bench_corpus_v2.py:252-268`).

Sweep runner: `benchmarks/run_full_sweep.sh` (70 lines). Runs 4 BO
profiles cold + 1 BO pool + 4 competitors serially on one IP, writing
each to `$OUT/{bo,comp}_*.json` and `.log`.

**RUN THIS** — full 4-profile + competitor sweep (~3-4 hours on one IP):

```bash
# Prerequisites:
#  - Rust release build of sweep_metrics + classify_stdin
#  - Python venv at /tmp/bo-venv with playwright, patchright, camoufox,
#    playwright_stealth installed
#  - /tmp/corpus.json regenerated (see §1 RUN THIS)
#  - PLAYWRIGHT_BROWSERS_PATH cache populated
cargo build --release -p browser --example sweep_metrics --example classify_stdin
bash benchmarks/run_full_sweep.sh
ls /tmp/full_sweep_$(date +%Y_%m_%d)/
```

## 6. The ±5 noise floor

Source: `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`. Methodology was:
take the baseline branch alone, run each "regression" site
single-thread × 3 consecutive runs, observe how often the result
changes by itself.

### Headline findings

- **AWS WAF amazon variants** are deterministic-stub or 1-of-3 pass
  per-IP. amazon-de chrome: stub / stub / **L3 871 KB**; amazon-fr
  chrome: **L3 752 KB** / stub / stub.
- **wayfair (firefox)** on the same FIX branch is 1.2 MB / 14 KB / 1.2 MB
  across 3 runs.
- **imdb (chrome)**: stub / stub / **L3 1.27 MB** in 3 runs.
- A 4-amazon-pass expected value across 5 variants has Bernoulli
  stddev ≈ 1.05 just from the per-fetch 33% pass rate, **before**
  adding wayfair/leboncoin/imdb.

**Total per-side single-run variance: ~5 sites.** A single-run -1
to -3 Pass delta is **statistically indistinguishable from zero**.

### Implication for v0.1.0

Every per-site claim ("fix X recovers reddit") must be verified across
**≥ 3 sweep runs**. A single-run H2H comparison is invalid as evidence
on its own (the 2026-05-23 H2H "regression" list was entirely
explained by this — see the table at
`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md:44-56`).

## 7. Multi-run aggregation requirement

**This is the gating change for v0.1.0 measurement.** Every published
number from this release must be a 3-run median, not a single-run
snapshot.

### Pipeline spec

For each `(profile, mode)` pair:

1. Run `sweep_metrics` **3 times back-to-back**, writing
   `/tmp/sweep_<ts>/run{1,2,3}_<profile>_<mode>.json`. Successive runs
   reuse the same Rust binary but get fresh isolates and a fresh
   SharedSession state (one-process-per-sweep is the default).
2. Aggregate the 3 runs into a single
   `/tmp/sweep_<ts>/median_<profile>_<mode>.json` with the schema
   below.
3. Aggregate per-profile medians into the routed `best-of-4` view by
   taking, per site, the *best* tag across the 4 profile medians (Pass
   beats CHL beats ThinShell beats ThinBody beats Error).

### Per-site median — definition of "median"

For each site, the 3 runs produce 3 `(tag, len, ms)` triples. The
"median" is defined as:

- **tag**: the **mode** (most common) of the 3 tags. Ties broken in
  favor of the "better" verdict in the order
  `Pass > L3-RENDERED-loose > ThinShell > CHL > ThinBody > Error`.
  Rationale: WAFs are stochastic; if 2-of-3 runs pass and 1 fails, the
  truth is "this site passes on this IP from this engine, modulo WAF
  variance."
- **len**: numeric median of the 3 byte counts. Used only for
  diagnostics; it does NOT change the verdict bucket.
- **ms**: numeric median of the 3 wall-clock times.

### Aggregator JSON schema

```json
{
  "summary": {
    "engine": "browser_oxide",
    "profile": "chrome_148_macos",
    "mode": "cold",
    "n_runs": 3,
    "run_timestamps": [1716580000, 1716583000, 1716586000],
    "n": 126,
    "pass_median": 99,
    "pass_min": 96,
    "pass_max": 101,
    "pass_per_run": [99, 96, 101],
    "..." : "(all other summary fields are 3-run medians)"
  },
  "results": [
    {
      "cat": "amazon", "name": "amazon-de",
      "url": "https://www.amazon.de/",
      "tag_runs": ["ThinShell", "ThinShell", "L3-RENDERED"],
      "len_runs": [2011, 2011, 871051],
      "ms_runs": [1832, 1755, 2204],
      "tag_median": "ThinShell",
      "len_median": 2011,
      "ms_median": 1832,
      "flaky": true,
      "err_runs": [null, null, null]
    }
  ]
}
```

- **`flaky` = true** when not all 3 tags match. Use this to flag sites
  in WAF-lottery space; do NOT count flaky sites toward "fixed by my
  patch" without a separate H2H paired test.

### Reference aggregator script (write under `benchmarks/median3.py`)

```python
#!/usr/bin/env python3
"""Aggregate 3 sweep_metrics or bench_corpus_v2 JSONs into a median.

Usage: median3.py run1.json run2.json run3.json out_median.json
"""
import json, statistics, sys
from collections import Counter

RANK = {"L3-RENDERED": 5, "ThinShell": 4,
        "Cloudflare-CHL": 3, "DataDome-CHL": 3, "Akamai-CHL": 3,
        "Kasada-CHL": 3, "PerimeterX-CHL": 3, "captcha-CHL": 3,
        "Akamai-sec-cpt-CHL": 3, "PerimeterX-PaH": 3, "BLOCKED": 3,
        "THIN-BODY": 2, "ERROR": 1}

def rank(tag, length):
    if tag == "L3-RENDERED" and length >= 15000:
        return 6  # Pass — top-rank
    if tag == "L3-RENDERED" and length < 15000:
        return 4  # ThinShell
    return RANK.get(tag, 0)

def medianish(runs):
    tags = [r["tag"] for r in runs]
    lens = [r["len"] for r in runs]
    # Mode-with-best-rank tiebreak
    counts = Counter(tags)
    top_n = max(counts.values())
    best_tags = [t for t in counts if counts[t] == top_n]
    best_tags.sort(key=lambda t: -max(rank(t, l) for l in lens))
    tag_med = best_tags[0]
    len_med = sorted(lens)[len(lens) // 2]
    ms_med = sorted(r["ms"] for r in runs)[len(runs) // 2]
    flaky = len(set(tags)) > 1
    return tag_med, len_med, ms_med, flaky

def main():
    runs = [json.load(open(p)) for p in sys.argv[1:-1]]
    out_path = sys.argv[-1]
    by_name = {}
    for run_idx, run in enumerate(runs):
        for r in run["results"]:
            by_name.setdefault(r["name"], []).append(r)
    out_results = []
    for name, rs in by_name.items():
        tag, ln, ms, fl = medianish(rs)
        out_results.append({
            "cat": rs[0]["cat"], "name": name, "url": rs[0]["url"],
            "tag_runs": [r["tag"] for r in rs],
            "len_runs": [r["len"] for r in rs],
            "ms_runs": [r["ms"] for r in rs],
            "tag_median": tag, "len_median": ln, "ms_median": ms,
            "flaky": fl,
            "err_runs": [r.get("err") for r in rs],
        })
    pass_per_run = [
        sum(1 for r in run["results"]
            if r["tag"] == "L3-RENDERED" and r["len"] >= 15000)
        for run in runs
    ]
    summary = dict(runs[0]["summary"])
    summary.update({
        "n_runs": len(runs),
        "pass_per_run": pass_per_run,
        "pass_median": sorted(pass_per_run)[len(pass_per_run) // 2],
        "pass_min": min(pass_per_run),
        "pass_max": max(pass_per_run),
    })
    json.dump({"summary": summary, "results": out_results},
              open(out_path, "w"), indent=1)

if __name__ == "__main__":
    main()
```

### Routed best-of-4 aggregator

After 4 profile medians exist, the routed view picks per site the best
median tag across profiles:

```bash
# RUN THIS
python3 benchmarks/route_best.py \
    /tmp/sweep_2026_05_24/median_chrome_148_macos_cold.json \
    /tmp/sweep_2026_05_24/median_firefox_135_macos_cold.json \
    /tmp/sweep_2026_05_24/median_pixel_9_pro_chrome_148_cold.json \
    /tmp/sweep_2026_05_24/median_iphone_15_pro_safari_18_cold.json \
    /tmp/sweep_2026_05_24/routed_best.json
```

`route_best.py` is a sibling of `median3.py` that consumes 4 median
JSONs and emits one routed JSON. See `14_TESTING_VALIDATION.md` §4 for
the script.

## 8. Reproduce-from-zero — `git clone` to baseline number

```bash
# RUN THIS — assumes a Linux/macOS box with rustup + python3.10+
git clone https://github.com/anthropic/browser_oxide.git
cd browser_oxide
git checkout main

# 1. Build the release-mode sweep harness + the shared classifier.
cargo build --release -p browser \
    --example sweep_metrics \
    --example classify_stdin

# 2. Regenerate /tmp/corpus.json from holistic_sweep.rs (see §1).
python3 - <<'PY' > /tmp/corpus.json
import re, json, pathlib
src = pathlib.Path("crates/browser/tests/holistic_sweep.rs").read_text()
m = re.search(r'fn sites_list\(\).*?vec!\[(.*?)\]\s*\}', src, re.S)
entries = re.findall(r'\("([^"]+)",\s*"([^"]+)",\s*"([^"]+)"\)', m.group(1))
json.dump([{"cat": c, "name": n, "url": u} for c, n, u in entries],
          open("/tmp/corpus.json", "w"), indent=1)
PY

# 3. Set up the competitor venv (optional — only needed for H2H).
python3 -m venv /tmp/bo-venv
/tmp/bo-venv/bin/pip install playwright playwright-stealth patchright camoufox
/tmp/bo-venv/bin/python -m playwright install
/tmp/bo-venv/bin/python -m camoufox fetch

# 4. Three-run BO sweep for chrome_148_macos cold.
OUT=/tmp/sweep_$(date +%Y_%m_%d)
mkdir -p $OUT
for i in 1 2 3; do
    target/release/examples/sweep_metrics \
        chrome_148_macos /tmp/corpus.json \
        $OUT/run${i}_chrome_148_macos_cold.json \
        2>&1 | tee $OUT/run${i}_chrome_148_macos_cold.log
done

# 5. Aggregate.
python3 benchmarks/median3.py \
    $OUT/run1_chrome_148_macos_cold.json \
    $OUT/run2_chrome_148_macos_cold.json \
    $OUT/run3_chrome_148_macos_cold.json \
    $OUT/median_chrome_148_macos_cold.json

# 6. Read the headline number.
jq '.summary | {pass_median, pass_min, pass_max, pass_per_run}' \
    $OUT/median_chrome_148_macos_cold.json
```

Expected output (chrome_148_macos cold, single-IP, 2026-05-24 baseline):

```json
{
  "pass_median": 99,
  "pass_min": 96,
  "pass_max": 102,
  "pass_per_run": [99, 96, 102]
}
```

If you see `pass_median < 90` or `pass_median > 110`, something is
wrong with the corpus or the network — check `$OUT/*.log` for `THIN-BODY`
spikes in `gov-bank`/`search` (those should always pass).

## Files referenced

- `crates/browser/tests/holistic_sweep.rs:1-1078` — corpus + per-site
  test definitions + parallel sweep
- `crates/browser/src/classify.rs:1-591` — `engine_classify` + size
  gates + marker tables + regression tests
- `crates/browser/examples/sweep_metrics.rs:1-286` — BO sweep harness
- `crates/browser/examples/classify_stdin.rs` — shared classifier
  binary used by the Python competitor harness
- `crates/browser/examples/nav_timed.rs:1-141` — single-site nav
  phase timing
- `crates/browser/src/page.rs:848-1140` — `Page::navigate*` signatures,
  `default_solvers` (returns empty), env-var hooks
- `crates/js_runtime/src/extensions/fetch_ext.rs:200-381` — `op_fetch`,
  CSP enforcement, FETCH_CLIENT thread-local
- `crates/js_runtime/src/extensions/console_ext.rs:1-31` — console
  capture ops
- `crates/js_runtime/src/state.rs:7-12, 67` — `DomState.console_output`
- `crates/stealth/src/presets.rs:39-875` — all 4 BO profile preset
  functions
- `crates/stealth/profiles/chrome_148_macos.yaml:1-85` — YAML profile
  fixture (the editable companion to the Rust preset)
- `benchmarks/bench_corpus_v2.py:1-368` — competitor harness
- `benchmarks/run_full_sweep.sh:1-70` — full 4-profile + competitor
  driver
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md:1-105` — ±5 noise floor
  characterization
- `docs/BENCHMARK_2026_05_24.md` — narrative of the 2026-05-24 baseline
- `docs/PERFORMANCE_2026_05_24.md` — pool-path perf investigation
- `docs/releases/v0.1.0-parity/04_TOOLING_SPEC.md` — capture mode +
  per-site diff tool (built on top of this methodology)
- `docs/releases/v0.1.0-parity/14_TESTING_VALIDATION.md` — regression
  gates, CI, A/B harness (consumes the median schema spec'd here)
