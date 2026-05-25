# 14 — Testing + validation

This chapter spec'd the validation harness for v0.1.0. Without these gates, every fix is a guess.

## Three layers of validation

| Layer | When to run | Wall clock | Catches |
|---|---|--:|---|
| **L1 — Unit + integration tests** | every change | < 30 s | broken builds, broken bootstrap JS, broken DOM ops |
| **L2 — Single-site capture diff** | per gap-site fix | ~ 30-60 s | regression on the specific site being fixed; flip confirmation |
| **L3 — 33-site spot-check** | every PR | ~ 12-18 min | regressions on broader corpus, WAF noise sanity check |
| **L4 — 126-site full sweep** | nightly + on RC tag | ~ 50-90 min | the source-of-truth pass rate |
| **L5 — 3-run aggregated full sweep** | weekly + before merge of any significant fix | ~ 2.5-4.5 h | filters WAF variance noise (±5 sites); produces honest baseline |

## L1 — Unit / integration tests

### Existing test infrastructure

| Path | Purpose |
|---|---|
| `cargo test --workspace -- --test-threads=1` | Full unit + integration suite. **MUST be single-threaded** — V8 isolates are per-thread, multi-threaded crashes the test process. CI enforces this. |
| `cargo clippy --workspace` | Advisory while backlog clears; CI gate is `-D warnings` via `0c2ad3e` |
| `cargo fmt --all -- --check` | Strict |
| `cargo doc --no-deps --workspace` | Doc build, no warnings |
| `crates/browser/tests/holistic_sweep.rs` | The 126-site corpus + `#[ignore]` sweep tests (run with `--ignored` locally only) |
| `crates/browser/tests/chrome_compat.rs` | Per-API parity tests against captured real-Chrome references |
| `crates/browser/tests/anti_bot.rs` | Anti-bot challenge document handling |
| `crates/browser/tests/navigation_primitives.rs` | `location.reload`/`assign`/`replace`, meta-refresh, form-submit |
| `crates/browser/tests/storage_persistence.rs` | localStorage/sessionStorage across navs |
| `crates/browser/tests/public_detection.rs` | Engine-side vendor-detection |

### Regression gate before any PR

```bash
# Must all pass before push:
cargo build --workspace 2>&1 | tail -5
cargo test --workspace -- --test-threads=1 2>&1 | tail -20
cargo clippy --workspace 2>&1 | tail -10
cargo fmt --all -- --check
cargo doc --no-deps --workspace 2>&1 | tail -10
```

### Network tests are `#[ignore]`

They require internet + live target sites. Run with `--ignored` locally only:
```bash
cargo test --workspace --ignored -- --test-threads=1 chrome_compat
```
CI does NOT run these (no network in CI, and live WAFs would create flaky tests).

## L2 — Single-site capture diff

**Use this whenever you're fixing a specific gap site** (reddit, duolingo, etsy, …).

Workflow:
1. Capture pre-fix state (so you have a baseline to diff against)
2. Apply your fix
3. Capture post-fix state
4. Diff
5. If positive change AND no regression in cold-vs-warm, move to L3

### Pre-flight capture (depends on `04_TOOLING_SPEC.md` tooling)

```bash
# Pre-fix baseline
git stash    # save your in-progress fix (or `git checkout HEAD~1` if already committed)
cargo build --release -p browser --example sweep_metrics --example classify_stdin
target/release/examples/sweep_metrics --capture reddit chrome_148_macos /tmp/pre_fix/
git stash pop  # restore your fix

# Apply your fix, rebuild
cargo build --release -p browser --example sweep_metrics

# Post-fix capture
target/release/examples/sweep_metrics --capture reddit chrome_148_macos /tmp/post_fix/

# Diff
benchmarks/capture_diff.py /tmp/pre_fix/reddit/ /tmp/post_fix/reddit/
```

Expected `capture_diff` output structure (spec'd in `04_TOOLING_SPEC.md`):
```
=== diff: pre_fix vs post_fix ===
body_size: 8326 -> 1145961 (+1137635 bytes) ✅
verdict:   ThinShell -> Pass ✅
iter_count: 1 -> 2 ✅
new fetches in post_fix not in pre_fix:
  POST https://www.reddit.com/ (solution=...) → 200
  GET  https://www.reddit.com/r/popular.json → 200
script_errors removed:
  (none)
cookie writes added:
  reddit_session=abc123 (HttpOnly, Secure)
```

### Quick gap-corpus run (curated set)

The 11-site gap corpus (`/tmp/gap_corpus.json` if it exists from prior sessions; otherwise re-create from `02_GAP_ANALYSIS.md`):

```bash
cat > /tmp/gap_corpus.json <<'JSON'
[
  {"cat":"social",     "name":"reddit",        "url":"https://www.reddit.com/"},
  {"cat":"amazon",     "name":"amazon-de",     "url":"https://www.amazon.de/"},
  {"cat":"amazon",     "name":"amazon-in",     "url":"https://www.amazon.in/"},
  {"cat":"amazon",     "name":"amazon-com-au", "url":"https://www.amazon.com.au/"},
  {"cat":"amazon",     "name":"amazon-jp",     "url":"https://www.amazon.co.jp/"},
  {"cat":"misc",       "name":"imdb",          "url":"https://www.imdb.com/"},
  {"cat":"misc",       "name":"duolingo",      "url":"https://www.duolingo.com/"},
  {"cat":"travel",     "name":"booking",       "url":"https://www.booking.com/"},
  {"cat":"chl-known",  "name":"douyin",        "url":"https://www.douyin.com/"},
  {"cat":"stores",     "name":"etsy",          "url":"https://www.etsy.com/"},
  {"cat":"social",     "name":"x-com",         "url":"https://x.com/"}
]
JSON
target/release/examples/sweep_metrics chrome_148_macos /tmp/gap_corpus.json /tmp/gap_out.json
```

Expected wall-clock: 3-5 minutes.

## L3 — 33-site stratified spot-check

The spot-check covers every corpus category. Run on every PR.

### Generating the stratified corpus

```python
# Run once to seed /tmp/spotcheck_corpus.json
python3 << 'EOF'
import json, random
sites = json.load(open('/tmp/corpus.json'))  # full 126 from holistic_sweep.rs
by_cat = {}
for s in sites:
    by_cat.setdefault(s['cat'], []).append(s)
random.seed(42)  # reproducible
selection = []
for cat, items in by_cat.items():
    n = min(len(items), max(2, len(items) // 4))
    selection.extend(random.sample(items, n))
json.dump(selection, open('/tmp/spotcheck_corpus.json','w'), indent=1)
print(f"Generated {len(selection)} sites")
EOF
```

If `/tmp/corpus.json` doesn't exist, regenerate from the source corpus:
```bash
python3 -c '
import re, json
src = open("crates/browser/tests/holistic_sweep.rs").read()
pat = re.compile(r"site!\s*\(\s*\w+\s*,\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*\)\s*;", re.DOTALL)
sites = [{"cat":m.group(1),"name":m.group(2),"url":m.group(3)} for m in pat.finditer(src)]
json.dump(sites, open("/tmp/corpus.json","w"), indent=1)
print(len(sites))
'
```

### Running spot-check

```bash
target/release/examples/sweep_metrics chrome_148_macos /tmp/spotcheck_corpus.json /tmp/spotcheck.json
```

Wall-clock: 12-18 minutes (some sites burn 90 s on DataDome/Kasada timeouts).

### Spot-check pass criteria

```python
# Read /tmp/spotcheck.json + previous baseline; assert no regressions
python3 << 'EOF'
import json, sys
new = json.load(open('/tmp/spotcheck.json'))
old = json.load(open('/tmp/full_sweep_2026_05_24/bo_chrome_148_macos_cold.json'))
old_by_name = {r['name']:r for r in old['results']}
regressions = 0
flips = 0
for r in new['results']:
    o = old_by_name.get(r['name'], {})
    o_pass = o.get('tag','').startswith('L3') and o.get('len',0) >= 15000
    n_pass = r['tag'].startswith('L3') and r['len'] >= 15000
    if o_pass and not n_pass:
        print(f'REGRESSION: {r["name"]} {o["tag"]}/{o["len"]} -> {r["tag"]}/{r["len"]}')
        regressions += 1
    elif n_pass and not o_pass:
        print(f'FLIP: {r["name"]} {o.get("tag","?")}/{o.get("len","?")} -> {r["tag"]}/{r["len"]}')
        flips += 1
print(f'\nNet: +{flips} flip, -{regressions} regression')
sys.exit(1 if regressions > 1 else 0)  # allow 1 flake; > 1 fails the gate
EOF
```

Acceptance: ≤ 1 regression (allows for WAF variance).

## L4 — Full 126-site sweep (single run)

Run nightly + on every RC tag.

```bash
# Single-profile
target/release/examples/sweep_metrics chrome_148_macos /tmp/corpus.json /tmp/full_chrome.json

# All 4 profiles (serial — ~ 200 min wall clock)
for profile in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
    target/release/examples/sweep_metrics $profile /tmp/corpus.json /tmp/full_$profile.json
done
```

Or use the existing orchestrator: `benchmarks/run_full_sweep.sh` (runs 4 BO cold + 1 BO pool + 4 competitors, ~4 h 17 m).

### Pass criteria (single run)

For each profile, compare against the 2026-05-24 baseline at `/tmp/full_sweep_2026_05_24/bo_<profile>_cold.json`:

| Metric | Pass criterion |
|---|---|
| Strict Pass | ≥ baseline - 3 (single run, accounts for ±5 noise) |
| Loose L3 | ≥ baseline - 3 |
| CHL count | ≤ baseline + 3 |
| Error count | == 0 |
| RSS peak | ≤ baseline * 1.1 |
| Throughput | ≥ baseline * 0.9 |

Run `benchmarks/build_report.py` to generate the comparison markdown.

## L5 — 3-run aggregated sweep (the source of truth)

Single-run pass numbers are noisy (±5 per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`). **Every "is this fix real?" decision uses the 3-run median.**

### Pipeline

```bash
# Run 3x sequentially for each profile (about 50 min × 4 profiles × 3 runs = 10 hours)
# In practice: nightly cron job, takes 8-12 h end-to-end
for run in 1 2 3; do
    for profile in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
        target/release/examples/sweep_metrics $profile /tmp/corpus.json /tmp/agg/${profile}_${run}.json
    done
done
```

### Aggregation logic

```python
# tools/aggregate_runs.py
import json, glob, statistics
from collections import defaultdict

# Per (profile, site) collect 3 results
per = defaultdict(list)
for f in glob.glob('/tmp/agg/*.json'):
    profile = f.split('/')[-1].rsplit('_', 1)[0]
    data = json.load(open(f))
    for r in data['results']:
        per[(profile, r['name'])].append(r)

# Median rule
# - For verdict: majority vote (2 of 3); tie → take the result with median body_len
# - For body_len: statistics.median of the 3 (numeric)
# - For ms: statistics.median
# - For rss_mb: statistics.median (over per-site reports)

medians = {}
for (profile, site), results in per.items():
    tags = [r['tag'] for r in results]
    majority_tag = max(set(tags), key=tags.count)
    if tags.count(majority_tag) == 1:  # 3-way tie
        results.sort(key=lambda r: r.get('len',0))
        majority_tag = results[1]['tag']  # median by body size
    medians[(profile, site)] = {
        'tag': majority_tag,
        'len': statistics.median(r.get('len',0) for r in results),
        'ms': statistics.median(r['ms'] for r in results),
        'rss_mb': statistics.median(r.get('rss_mb',0) for r in results),
    }

# Per-profile aggregate
for profile in {p for p, _ in medians}:
    pass_count = sum(
        1 for (p, s), v in medians.items()
        if p == profile and v['tag'].startswith('L3') and v['len'] >= 15000
    )
    print(f'{profile}: median Pass = {pass_count}')
```

### Routed best-of-4 (per-site)

```python
# Across all 4 profiles, pick the best result per site
sites = {s for _, s in medians}
routed = {}
for site in sites:
    candidates = [medians[(p, site)] for p in {p for p, s in medians if s == site}]
    # Best = highest body_len among L3-RENDERED ones; else first CHL/THIN/Error
    l3 = [c for c in candidates if c['tag'].startswith('L3')]
    if l3:
        routed[site] = max(l3, key=lambda c: c['len'])
    else:
        routed[site] = candidates[0]
routed_pass = sum(1 for v in routed.values() if v['tag'].startswith('L3') and v['len'] >= 15000)
print(f'routed best-of-4 median Pass: {routed_pass}')
```

### Acceptance for v0.1.0 (the BAR)

- `routed best-of-4 median Pass` ≥ **115** (Camoufox = 113)
- `chrome_148_macos median Pass` ≥ **103** (current 99)
- `pixel_9_pro_chrome_148 median Pass` ≥ **106** (current 102)
- All 4 profiles complete the 126-sweep with zero errors (no panics, no timeouts of >2× nav budget)
- 3-run RSS peak (median) ≤ 350 MB on all 4 profiles
- Pool path completes 126/126 with no panic
- 3-run pool median throughput ≥ 13.5 pages/min

## A/B harness — for evaluating a specific fix

When evaluating a fix that touches the engine, run side-by-side against the prior commit.

### Spec: `tools/ab_sweep.sh`

```bash
#!/usr/bin/env bash
# Usage: tools/ab_sweep.sh <commit_a> <commit_b> <profile> <corpus.json>
# Runs the same sweep on both commits and prints a per-site delta table.

set -euo pipefail
COMMIT_A="$1"; COMMIT_B="$2"; PROFILE="$3"; CORPUS="$4"
OUT=/tmp/ab_$(date +%s)
mkdir -p "$OUT"

# Build A
git checkout "$COMMIT_A" -- .
cargo build --release -p browser --example sweep_metrics --example classify_stdin
mv target/release/examples/sweep_metrics "$OUT/sweep_A"
mv target/release/examples/classify_stdin "$OUT/classify_A"

# Build B
git checkout "$COMMIT_B" -- .
cargo build --release -p browser --example sweep_metrics --example classify_stdin
mv target/release/examples/sweep_metrics "$OUT/sweep_B"
mv target/release/examples/classify_stdin "$OUT/classify_B"

# Run both
"$OUT/sweep_A" "$PROFILE" "$CORPUS" "$OUT/A.json"
"$OUT/sweep_B" "$PROFILE" "$CORPUS" "$OUT/B.json"

# Diff
python3 - <<EOF
import json
a = {r['name']:r for r in json.load(open('$OUT/A.json'))['results']}
b = {r['name']:r for r in json.load(open('$OUT/B.json'))['results']}
print(f'{"site":<24} {"A tag":<14}/{"A len":>8} -> {"B tag":<14}/{"B len":>8} ms_A ms_B  Δ')
flips = 0; regs = 0
for s in sorted(a):
    if s not in b: continue
    ra, rb = a[s], b[s]
    a_pass = ra['tag'].startswith('L3') and ra['len'] >= 15000
    b_pass = rb['tag'].startswith('L3') and rb['len'] >= 15000
    flag = ''
    if b_pass and not a_pass: flag = '✅'; flips += 1
    elif a_pass and not b_pass: flag = '❌'; regs += 1
    print(f'{s:<24} {ra["tag"]:<14}/{ra["len"]:>8} -> {rb["tag"]:<14}/{rb["len"]:>8} {ra["ms"]:>5} {rb["ms"]:>5}  {flag}')
print(f'Net: +{flips} -{regs}')
EOF
```

### A/B usage examples

```bash
# Evaluate fix B (uncommitted) vs HEAD
tools/ab_sweep.sh HEAD HEAD chrome_148_macos /tmp/gap_corpus.json   # baseline noise
# (then apply your uncommitted changes, commit to a branch)
tools/ab_sweep.sh HEAD my-fix-branch chrome_148_macos /tmp/gap_corpus.json

# Evaluate SharedSession revert (D from this plan)
tools/ab_sweep.sh HEAD revert-sharedsession-branch chrome_148_macos /tmp/corpus.json
```

## CI integration

### Workflow skeleton

Check existing CI: `ls .github/workflows/ 2>/dev/null && cat .github/workflows/*.yml`

Add a new file `.github/workflows/sweep-nightly.yml`:

```yaml
name: nightly-sweep
on:
  schedule:
    - cron: '0 6 * * *'  # 06:00 UTC daily
  workflow_dispatch: {}  # manual trigger
jobs:
  sweep:
    # Self-hosted runner with network egress + ~10 GB free disk
    runs-on: [self-hosted, sweep]
    strategy:
      matrix:
        profile: [chrome_148_macos, pixel_9_pro_chrome_148, iphone_15_pro_safari_18, firefox_135_macos]
      fail-fast: false
    steps:
      - uses: actions/checkout@v4
      - name: Build
        run: |
          cargo build --release -p browser --example sweep_metrics --example classify_stdin
      - name: Generate corpus
        run: |
          python3 -c '...'  # see L4 above
      - name: Sweep
        timeout-minutes: 90
        run: |
          target/release/examples/sweep_metrics ${{ matrix.profile }} /tmp/corpus.json /tmp/${{ matrix.profile }}.json
      - name: Upload result
        uses: actions/upload-artifact@v4
        with:
          name: sweep-${{ matrix.profile }}-${{ github.run_id }}
          path: /tmp/${{ matrix.profile }}.json
      - name: Regression check
        run: |
          # Fetch the 7-day rolling baseline, compare, fail on > -3 delta
          python3 tools/regression_check.py /tmp/${{ matrix.profile }}.json
```

Plus a separate weekly workflow for 3-run aggregation (`.github/workflows/sweep-weekly-agg.yml`) that runs 3× and aggregates.

### `tools/regression_check.py` spec

```python
# Reads the new sweep JSON, fetches the 7-day rolling baseline from
# (e.g.) S3 bucket / artifact store, and exits non-zero if:
# - strict Pass < baseline - 3
# - any error count > 0 (was 0 in baseline)
# - RSS peak > baseline * 1.2
# Logs per-site delta to stdout for the run log.
```

Acceptance: regression-check exits non-zero on > -3 site drop OR any new error.

## Per-phase acceptance tests

### Phase 0 (tooling)
- [ ] `sweep_metrics --capture <site>` writes the 7 spec'd files (body.html, fetches.json, script_errors.json, cookie_writes.json, pending_nav_timeline.json, console.txt, iter_summary.json) per `04_TOOLING_SPEC.md`
- [ ] `benchmarks/capture_camoufox.py <site>` produces the same shape
- [ ] `benchmarks/capture_diff.py BO_DIR CAMOUFOX_DIR` exits 0 on a known-equivalent pair, non-zero on a known-divergent pair
- [ ] `tools/ab_sweep.sh` produces a per-site delta table
- [ ] `tools/aggregate_runs.py` produces a 3-run median report

### Phase 1 (SPA cluster — `05`)
- [ ] reddit: body > 100 KB on at least 2 of 4 profiles
- [ ] duolingo: body > 50 KB on at least 2 of 4 profiles
- [ ] booking: body > 30 KB on at least 2 of 4 profiles
- [ ] douyin: body > 100 KB on at least 1 of 4 profiles
- [ ] 3-run median confirms (not single-run flakes)

### Phase 2 (AWS WAF — `06`)
- [ ] amazon-de body > 100 KB on > 70% of runs (accounts for AWS roll)
- [ ] amazon-in body > 100 KB on > 70%
- [ ] amazon-com-au body > 100 KB on > 70%
- [ ] imdb body > 100 KB on > 70%
- [ ] amazon-co-uk variance reduced (was 696K/2K/1M/694K across profiles in one run; should be more consistent)

### Phase 3 (DataDome primitives — `07`)
- [ ] etsy: tag changes from DataDome-CHL to L3-RENDERED with body > 30 KB
- [ ] tripadvisor: same
- [ ] yelp: same
- [ ] No regression on previously-CHL DataDome sites (verify each from the baseline)

### Phase 4 (memory + timing — `09`, `10`)
- [ ] Cold RSS peak < 350 MB on all 4 profiles (was 388-472)
- [ ] Pool sweep completes 126/126 with no panic
- [ ] Pool RSS peak < 800 MB on full 126 sweep
- [ ] Cold throughput ≥ 2.3 pages/min maintained
- [ ] Pool throughput ≥ 13.5 pages/min

## Final v0.1.0 gate

Before tagging v0.1.0, every Phase acceptance above must be green PLUS:
- [ ] 3-run aggregated full sweep (4 profiles): routed best-of-4 median Pass ≥ 115
- [ ] All `cargo test --workspace -- --test-threads=1` pass
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] All 16 docs in this directory final + reviewed
- [ ] `CHANGELOG.md` updated with the per-fix attribution
- [ ] Customer-facing `README.md` updated with the new v0.1.0 numbers (replacing the 2026-05-24 numbers)
- [ ] `benchmarks/run_full_sweep.sh` regenerates the canonical comparison table
- [ ] `git tag v0.1.0-parity` after a clean 3-run

## Files referenced

- `crates/browser/tests/holistic_sweep.rs` — corpus definition
- `crates/browser/examples/sweep_metrics.rs` — sweep harness
- `benchmarks/bench_corpus_v2.py` — competitor sweep
- `benchmarks/run_full_sweep.sh` — orchestrator
- `benchmarks/build_report.py` — aggregator
- `/tmp/full_sweep_2026_05_24/` — baseline artifacts
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5 noise documentation
- `04_TOOLING_SPEC.md` — capture mode spec (blocking dep)
- `03_BENCHMARK_METHODOLOGY.md` — methodology spec
- `01_CURRENT_STATE.md` — baseline numbers to beat
