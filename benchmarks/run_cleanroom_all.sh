#!/usr/bin/env bash
# Clean-room full comparison: EVERY engine, all 126 sites, ONE AT A TIME, on a
# quiet box (load-gated). Removes the CPU-contention confound that crashed
# camoufox's driver in the 2026-05-29 sweep. Resumable: an engine whose output
# already has 126 results is skipped. Long run (many hours) by design.
set -uo pipefail
cd /home/yfedoseev/projects/browser_oxide
OUT=/tmp/cleanroom_2026_05_29
mkdir -p "$OUT"
CORPUS=/tmp/corpus.json
STABLE=/tmp/warm_verify/sweep_stable
BOVENV=/tmp/bo-venv/bin/python
CF150=/tmp/cfv150/bin/python
export PLAYWRIGHT_BROWSERS_PATH=/home/yfedoseev/.cache/ms-playwright
export CR_MAX_LOAD=6.0 CR_RETRIES=4 CR_SETTLE=12 CR_COOLDOWN=4
export BO_MAX_LOAD=6.0 BO_SITE_TIMEOUT=180 BO_SWEEP_BIN="$STABLE"
note(){ echo "[$(date +%H:%M:%S)] $*"; }

# Ensure a stable binary (sweep_metrics + classify_stdin) immune to target churn.
note "ensuring binaries..."
nice -n 5 cargo build --release -p browser --example sweep_metrics --example classify_stdin 2>&1 | tail -2
cp -f target/release/examples/sweep_metrics "$STABLE" 2>/dev/null
[ -x "$STABLE" ] || { note "FATAL: no stable sweep_metrics"; exit 1; }
[ -x target/release/examples/classify_stdin ] || { note "FATAL: no classify_stdin"; exit 1; }
note "binaries ready"

have126(){ python3 -c "import json,sys;d=json.load(open('$1'));sys.exit(0 if len(d.get('results',[]))>=126 else 1)" 2>/dev/null; }

run_bo(){ # $1=profile
  local f="$OUT/bo_$1.json"
  if have126 "$f"; then note "SKIP bo $1 (done $(python3 -c "import json;d=json.load(open('$f'))['summary'];print(d['production_pass'],'/',d['production_n'])"))"; return; fi
  note "START bo $1 (sequential, load-gated)"
  nice -n 5 python3 benchmarks/run_bo_isolated.py "$1" "$CORPUS" "$f" > "$OUT/bo_$1.log" 2>&1
  note "DONE bo $1: $(python3 -c "import json;d=json.load(open('$f'))['summary'];print(d['production_pass'],'/',d['production_n'])" 2>/dev/null)"
}
run_comp(){ # $1=engine_kind  $2=tag  $3=python  $4=extra_env
  local f="$OUT/$2.json"
  if have126 "$f"; then note "SKIP $2 (done $(python3 -c "import json;d=json.load(open('$f'))['summary'];print(d['production_pass'],'/',d['production_n'])"))"; return; fi
  note "START $2 (cleanroom, load-gated)"
  env $4 nice -n 5 "$3" benchmarks/run_cleanroom.py "$1" "$CORPUS" "$f" "$2" > "$OUT/$2.log" 2>&1
  note "DONE $2: $(python3 -c "import json;d=json.load(open('$f'))['summary'];print(d['production_pass'],'/',d['production_n'])" 2>/dev/null)"
}

note "===== CLEAN-ROOM FULL COMPARISON START ====="
# Priority order: the contaminated camoufox first, then BO, then chromium.
run_comp camoufox camoufox_v150 "$CF150" "XDG_CACHE_HOME=/tmp/cf150_cache"
run_comp camoufox camoufox_v135 "$BOVENV" ""
run_bo chrome_148_macos
run_bo pixel_9_pro_chrome_148
run_bo iphone_15_pro_safari_18
run_bo firefox_135_macos
run_comp playwright_stealth playwright_stealth "$BOVENV" ""
run_comp patchright patchright "$BOVENV" ""
run_comp playwright playwright "$BOVENV" ""
note "===== CLEAN-ROOM FULL COMPARISON COMPLETE ====="
