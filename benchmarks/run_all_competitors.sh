#!/usr/bin/env bash
# Run ALL 5 competitors sequentially (single IP => no parallel) on the
# vendor-spaced 126-corpus, same as BO's gate. Chromium-tier uses the canonical
# bench_corpus_v2 (shared browser, stable+fast); camoufox uses the per-site
# isolated runner (its driver crashes in a sustained shared loop). All bodies
# classified via BO's classify_stdin -> comparable to BO's 115.
# Outputs -> /tmp/full_gate_2026_05_28/comp_*.json
set -uo pipefail
cd $(cd "$(dirname "$0")/.." && pwd)
GATE=/tmp/full_gate_2026_05_28
export CORPUS_FILE=/tmp/corpus.json
export PLAYWRIGHT_BROWSERS_PATH=$HOME/.cache/ms-playwright
PY=/tmp/bo-venv/bin/python
note(){ echo "[$(date +%H:%M:%S)] $*"; }

# 1-3: Chromium tier (shared browser via bench_corpus_v2).
for eng in playwright playwright_stealth patchright; do
  note "START $eng"
  $PY benchmarks/bench_corpus_v2.py "$eng" "$GATE/comp_${eng}.json" > "$GATE/comp_${eng}.log" 2>&1 \
    && note "DONE $eng: $(python3 -c "import json;d=json.load(open('$GATE/comp_${eng}.json'));s=d['summary'];print(s.get('production_pass',s.get('pass')),'/',s.get('production_n',s.get('n')))" 2>/dev/null)" \
    || note "WARN $eng nonzero"
done

# 4: camoufox v135 (bo-venv + default ~/.cache/camoufox = v135 binary).
note "START camoufox_v135 (isolated)"
$PY benchmarks/run_competitor_isolated.py camoufox "$CORPUS_FILE" "$GATE/comp_camoufox_v135.json" camoufox_v135 \
  > "$GATE/comp_camoufox_v135.log" 2>&1 || note "WARN v135 nonzero"
note "v135: $(grep '^camoufox_v135: production' $GATE/comp_camoufox_v135.log 2>/dev/null)"

# 5: camoufox v150 (cfv150 venv + cf150_cache, patched MIN_VERSION).
note "START camoufox_v150 (isolated, cfv150)"
XDG_CACHE_HOME=/tmp/cf150_cache /tmp/cfv150/bin/python benchmarks/run_competitor_isolated.py \
  camoufox "$CORPUS_FILE" "$GATE/comp_camoufox_v150.json" camoufox_v150 \
  > "$GATE/comp_camoufox_v150.log" 2>&1 || note "WARN v150 nonzero"
note "v150: $(grep '^camoufox_v150: production' $GATE/comp_camoufox_v150.log 2>/dev/null)"

note "ALL 5 COMPETITORS DONE"
