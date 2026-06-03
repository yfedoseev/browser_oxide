#!/usr/bin/env bash
# Fix-12-style gate v3: 3 trials × 4 profiles × 126 sites, single-IP serial.
#
# Improvements over v2 (HANDOFF_2026_05_27 Sprint 1.1):
# - **Vendor-aware spacing pass** after random shuffle: ensures no two
#   consecutive sites share an antibot vendor tag (AWS WAF / DataDome /
#   Akamai / Kasada / Twitter), which removes the dominant per-IP
#   token-clustering bias from the same-IP serial sweep. See
#   `benchmarks/corpus_vendor_map.py` for the tag map + algorithm.
#
# Carried over from v2:
# - Per-run shuffled corpus (different seed per run).
# - Atomic-checkpoint `.partial` files written per-site by sweep_metrics.
#   If the 50-min cap fires before 126/126 completes, the .partial
#   captures the per-site verdicts the run DID get. Aggregator below
#   reconstructs a summary from .partial if the main .json is missing.
#
# Total wall ~10 h.
# Output: /tmp/fix12_gate/<profile>_run{1,2,3}.{json,log,partial}

set -uo pipefail
cd $(cd "$(dirname "$0")/.." && pwd)
mkdir -p /tmp/fix12_gate

PROFILES=(chrome_148_macos chrome_148_windows firefox_135_macos iphone_15_pro_safari_18)
RUNS=(1 2 3)

CORPUS=/tmp/corpus.json
BIN=target/release/examples/sweep_metrics
# Sprint 1.2 + 1.3 wrapper: per-vendor sub-process isolation + same-vendor
# cool-down. Set BO_GATE_ISOLATE_DISABLE=1 to fall back to the direct
# sweep_metrics path (useful for A/B verification of the isolation lift).
WRAPPER=benchmarks/run_sweep_isolated.py

if [[ ! -f $CORPUS ]]; then
  echo "missing $CORPUS — run benchmarks/build_corpus_json.py first" >&2
  exit 1
fi
if [[ ! -x $BIN ]]; then
  echo "missing $BIN — run cargo build --release -p browser --example sweep_metrics" >&2
  exit 1
fi
if [[ ! -f $WRAPPER ]]; then
  echo "missing $WRAPPER — benchmarks/run_sweep_isolated.py" >&2
  exit 1
fi

T0=$(date +%s)
echo "[gate] starting 3×${#PROFILES[@]}=$((${#RUNS[@]} * ${#PROFILES[@]})) sweeps at $(date)"

for profile in "${PROFILES[@]}"; do
  for run in "${RUNS[@]}"; do
    out=/tmp/fix12_gate/${profile}_run${run}.json
    log=/tmp/fix12_gate/${profile}_run${run}.log
    partial=/tmp/fix12_gate/${profile}_run${run}.json.partial
    shuffled=/tmp/fix12_gate/corpus_shuffled_${profile}_run${run}.json

    # Per-run shuffle + vendor-aware spacing pass. Seed = profile-run
    # hash so the order is reproducible. The spacing pass ensures no two
    # consecutive sites share an antibot vendor (AWS WAF / DataDome /
    # Akamai / Kasada / Twitter) — see benchmarks/corpus_vendor_map.py.
    PYTHONPATH=$(cd "$(dirname "$0")/.." && pwd) python3 -c "
import json, random
from benchmarks.corpus_vendor_map import space_by_vendor, vendor_run_summary
c = json.load(open('$CORPUS'))
random.seed('${profile}-run${run}')
random.shuffle(c)
before = vendor_run_summary(c)
c = space_by_vendor(c)
after = vendor_run_summary(c)
print(f'[shuffle] ${profile}-run${run} vendor-adjacent clashes before={before} after={after}')
json.dump(c, open('$shuffled', 'w'))
" || { echo "shuffle failed"; exit 1; }

    echo "[gate] === ${profile} run ${run} (shuffled, vendor-isolated) ==="
    # Sprint 1.2/1.3: drive sweep_metrics through the isolation wrapper.
    # Vendor-tagged sites run in fresh sweep_metrics children (one site
    # per process); untagged sites batch. Same-vendor consecutive chunks
    # get a BO_GATE_VENDOR_COOLDOWN_S (default 45 s) sleep between them.
    timeout 90m python3 "$WRAPPER" "$profile" "$shuffled" "$out" > "$log" 2>&1
    rc=$?
    elapsed=$(( $(date +%s) - T0 ))

    # Reconstruct summary from .partial if the main JSON wasn't written
    # (cap-truncated runs leave .partial but skip the aggregate step).
    if [[ ! -f $out && -f $partial ]]; then
      python3 -c "
import json
results = [json.loads(l) for l in open('$partial')]
passed = sum(1 for r in results if r['tag'] == 'L3-RENDERED' and r['len'] >= 15000)
n = len(results)
# Diagnostic sites are intended-to-fail; production excludes them.
diag_names = {'areyouheadless'}
prod_n = sum(1 for r in results if r['name'] not in diag_names)
prod_pass = sum(1 for r in results if r['name'] not in diag_names and r['tag'] == 'L3-RENDERED' and r['len'] >= 15000)
summary = {
    'pass': passed, 'n': n,
    'pass_pct': round(100 * passed / max(1, n), 1),
    'production_pass': prod_pass, 'production_n': prod_n,
    'production_pass_pct': round(100 * prod_pass / max(1, prod_n), 1),
    'reconstructed_from_partial': True,
    'expected_n': 126,
}
json.dump({'summary': summary, 'results': results}, open('$out', 'w'), indent=2)
print(f'reconstructed summary from .partial: {passed}/{n} (expected 126)')
" 2>&1
    fi

    if [[ -f $out ]]; then
      pass=$(jq -r '.summary.pass // 0' "$out" 2>/dev/null)
      n=$(jq -r '.summary.n // 0' "$out" 2>/dev/null)
      prod_pass=$(jq -r '.summary.production_pass // 0' "$out" 2>/dev/null)
      prod_n=$(jq -r '.summary.production_n // 0' "$out" 2>/dev/null)
      recon=$(jq -r '.summary.reconstructed_from_partial // false' "$out" 2>/dev/null)
      tag="[truncated]"; [[ "$recon" != "true" ]] && tag=""
      echo "[gate] ${profile} run ${run}: pass=${pass}/${n} | production=${prod_pass}/${prod_n} ${tag} | rc=${rc} | elapsed=${elapsed}s"
    else
      echo "[gate] ${profile} run ${run}: NO OUTPUT (rc=${rc}, elapsed=${elapsed}s)"
    fi
  done
done

echo "[gate] done at $(date), total elapsed $(( $(date +%s) - T0 ))s"
echo "[gate] results:"
for f in /tmp/fix12_gate/*.json; do
  [[ "$f" == */corpus_shuffled_* ]] && continue
  pass=$(jq -r '.summary.pass // "-"' "$f" 2>/dev/null)
  n=$(jq -r '.summary.n // "-"' "$f" 2>/dev/null)
  pp=$(jq -r '.summary.production_pass // "-"' "$f" 2>/dev/null)
  pn=$(jq -r '.summary.production_n // "-"' "$f" 2>/dev/null)
  recon=$(jq -r '.summary.reconstructed_from_partial // false' "$f" 2>/dev/null)
  tag=""; [[ "$recon" == "true" ]] && tag="[truncated]"
  printf "  %-55s  pass=%s/%s production=%s/%s %s\n" "$(basename "$f")" "$pass" "$n" "$pp" "$pn" "$tag"
done
