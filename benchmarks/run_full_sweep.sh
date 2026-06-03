#!/usr/bin/env bash
# Full 126-corpus sweep — 4 BO profiles (cold) + 1 BO profile (pool) + 4 competitors.
# Serial single-IP to avoid cross-engine WAF rate-limit contamination.
# Each engine: 8-25 minutes; total ~3-4 hours.
#
# Outputs land in /tmp/full_sweep_2026_05_24/
set -euo pipefail

OUT=/tmp/full_sweep_2026_05_24
mkdir -p "$OUT"
LOG="$OUT/run.log"
: > "$LOG"

REPO=$(cd "$(dirname "$0")/.." && pwd)
BO_BIN="$REPO/target/release/examples/sweep_metrics"
PY=/tmp/bo-venv/bin/python
COMP_HARNESS="$REPO/benchmarks/bench_corpus_v2.py"
export CORPUS_FILE=/tmp/corpus.json
export PLAYWRIGHT_BROWSERS_PATH=$HOME/.cache/ms-playwright

note() {
    echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"
}

run_bo() {
    local profile="$1" mode="$2"
    local out="$OUT/bo_${profile}_${mode}.json"
    local log="$OUT/bo_${profile}_${mode}.log"
    note "START bo $profile $mode -> $out"
    local start=$(date +%s)
    if [ "$mode" = "pool" ]; then
        BROWSER_OXIDE_SWEEP_POOL=1 "$BO_BIN" "$profile" "$CORPUS_FILE" "$out" \
            > "$log" 2>&1 || note "WARN bo $profile $mode exited non-zero"
    else
        "$BO_BIN" "$profile" "$CORPUS_FILE" "$out" \
            > "$log" 2>&1 || note "WARN bo $profile $mode exited non-zero"
    fi
    local dur=$(($(date +%s) - start))
    note "DONE  bo $profile $mode in ${dur}s"
}

run_comp() {
    local engine="$1"
    local out="$OUT/comp_${engine}.json"
    local log="$OUT/comp_${engine}.log"
    note "START $engine -> $out"
    local start=$(date +%s)
    "$PY" "$COMP_HARNESS" "$engine" "$out" > "$log" 2>&1 \
        || note "WARN $engine exited non-zero"
    local dur=$(($(date +%s) - start))
    note "DONE  $engine in ${dur}s"
}

note "==== full sweep start ($(date)) ===="

# Browser_oxide first — local Rust, fastest to fail-fast on bugs.
for prof in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
    run_bo "$prof" cold
done

# Pool-mode for the headline profile.
run_bo chrome_148_macos pool

# Competitors — slower (real Chrome / Firefox), single-IP serial.
for eng in playwright playwright_stealth patchright camoufox; do
    run_comp "$eng"
done

note "==== full sweep done ($(date)) ===="
ls -la "$OUT"
