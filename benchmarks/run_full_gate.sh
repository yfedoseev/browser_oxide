#!/usr/bin/env bash
# Full verification gate (parity-workflows, 2026-05-28):
#   browser_oxide 4 profiles (cold) + 1 pool, vs ALL competitors:
#   playwright, playwright_stealth, patchright, camoufox v150, camoufox v135.
# Corpus is VENDOR-SPACED (corpus_vendor_map) so back-to-back same-vendor
# (esp. AWS-WAF) token-clustering doesn't produce false failures.
# Serial single-IP. Total ~3-5 hours. Outputs -> /tmp/full_gate_2026_05_28/
set -uo pipefail

OUT=/tmp/full_gate_2026_05_28
mkdir -p "$OUT"
LOG="$OUT/run.log"
: > "$LOG"

REPO=/home/yfedoseev/projects/browser_oxide
BO_BIN="$REPO/target/release/examples/sweep_metrics"
PY=/tmp/bo-venv/bin/python
COMP="$REPO/benchmarks/bench_corpus_v2.py"
export CORPUS_FILE=/tmp/corpus.json
export PLAYWRIGHT_BROWSERS_PATH=/home/yfedoseev/.cache/ms-playwright
CACHE=/home/yfedoseev/.cache

note() { echo "[$(date +%H:%M:%S)] $*" | tee -a "$LOG"; }

run_bo() {
  local profile="$1" mode="$2"
  local out="$OUT/bo_${profile}_${mode}.json"
  note "START bo $profile $mode"
  local start=$(date +%s)
  if [ "$mode" = "pool" ]; then
    BROWSER_OXIDE_SWEEP_POOL=1 "$BO_BIN" "$profile" "$CORPUS_FILE" "$out" \
      > "$OUT/bo_${profile}_${mode}.log" 2>&1 || note "WARN bo $profile $mode nonzero"
  else
    "$BO_BIN" "$profile" "$CORPUS_FILE" "$out" \
      > "$OUT/bo_${profile}_${mode}.log" 2>&1 || note "WARN bo $profile $mode nonzero"
  fi
  note "DONE  bo $profile $mode in $(($(date +%s)-start))s"
}

run_comp() {
  local engine="$1" tag="${2:-$1}"
  local out="$OUT/comp_${tag}.json"
  note "START comp $tag"
  local start=$(date +%s)
  "$PY" "$COMP" "$engine" "$out" > "$OUT/comp_${tag}.log" 2>&1 \
    || note "WARN comp $tag nonzero"
  note "DONE  comp $tag in $(($(date +%s)-start))s"
}

note "==== FULL GATE START ($(date)) corpus=$(python3 -c 'import json;print(len(json.load(open("/tmp/corpus.json"))))') sites ===="

# 1) browser_oxide — 4 profiles cold + pool for the headline profile.
for prof in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
  run_bo "$prof" cold
done
run_bo chrome_148_macos pool

# 2) Chromium-tier competitors.
for eng in playwright playwright_stealth patchright; do
  run_comp "$eng"
done

# 3) Camoufox v150 (active cache).
ACTIVE_VER=$(python3 -c "import json;print(json.load(open('$CACHE/camoufox/version.json')).get('version','?'))" 2>/dev/null || echo '?')
note "camoufox active cache version=$ACTIVE_VER"
run_comp camoufox "camoufox_v150"

# 4) Camoufox v135 — swap the cache dir, run, swap back.
if [ -d "$CACHE/camoufox.v135.bak" ]; then
  note "swapping camoufox cache -> v135"
  mv "$CACHE/camoufox" "$CACHE/camoufox.v150.tmp" \
    && mv "$CACHE/camoufox.v135.bak" "$CACHE/camoufox"
  run_comp camoufox "camoufox_v135"
  note "restoring camoufox cache -> v150"
  mv "$CACHE/camoufox" "$CACHE/camoufox.v135.bak" \
    && mv "$CACHE/camoufox.v150.tmp" "$CACHE/camoufox"
else
  note "WARN no camoufox.v135.bak — skipping v135"
fi

note "==== FULL GATE DONE ($(date)) ===="
ls -la "$OUT"
