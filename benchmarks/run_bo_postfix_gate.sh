#!/usr/bin/env bash
# BO post-fix verification gate: 4 profiles x 126 sites, per-site isolated,
# parallel + vendor-spaced. Compares against the all-5 competitor run.
cd $(cd "$(dirname "$0")/.." && pwd)
GATE=/tmp/full_gate_2026_05_28
CORPUS=/tmp/corpus.json
note(){ echo "[$(date +%H:%M:%S)] $*"; }
note "BO POST-FIX GATE START — 4 profiles x 126, parallel=6, vendor-spaced"
for prof in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
  note "START $prof"
  BO_PARALLEL=6 nice -n 10 python3 benchmarks/run_bo_isolated.py "$prof" "$CORPUS" \
    "$GATE/bo_${prof}_postfix.json" --parallel 6 > "$GATE/bo_${prof}_postfix.log" 2>&1
  prod=$(python3 -c "import json;d=json.load(open('$GATE/bo_${prof}_postfix.json'))['summary'];print(d['production_pass'],'/',d['production_n'])" 2>/dev/null)
  note "DONE $prof: $prod"
done
note "BO POST-FIX GATE COMPLETE"
