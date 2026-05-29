#!/usr/bin/env bash
# Spaced AWS-WAF cluster check (parity-workflows). Each amazon TLD + imdb is a
# FRESH sweep_metrics process with a 150s gap, because same-IP same-vendor
# calls back-to-back trigger AWS per-IP token-clustering and produce FALSE
# failures (amazon-fr: THIN 5KB back-to-back, 799KB PASS spaced). With spacing
# the whole cluster passes (9/9, 2026-05-28). Build first:
#   cargo build --release --example sweep_metrics
# Spaced all-amazon + imdb AWS-WAF check: each site a fresh sweep_metrics
# process, 150s gap between calls to avoid per-IP token clustering.
set -u
cd /home/yfedoseev/projects/browser_oxide
BIN=target/release/examples/sweep_metrics
OUT=/tmp/awswaf/spaced_aws
mkdir -p "$OUT"
SUMMARY="$OUT/SUMMARY.txt"
: > "$SUMMARY"

# name|url  (imdb first, then amazon TLDs)
SITES=(
  "imdb|https://www.imdb.com/"
  "amazon-com|https://www.amazon.com/"
  "amazon-ca|https://www.amazon.ca/"
  "amazon-co-uk|https://www.amazon.co.uk/"
  "amazon-com-au|https://www.amazon.com.au/"
  "amazon-de|https://www.amazon.de/"
  "amazon-fr|https://www.amazon.fr/"
  "amazon-in|https://www.amazon.in/"
  "amazon-jp|https://www.amazon.co.jp/"
)
DELAY=150
n=${#SITES[@]}
i=0
for entry in "${SITES[@]}"; do
  i=$((i+1))
  name="${entry%%|*}"
  url="${entry##*|}"
  cj="$OUT/$name.json"
  printf '[{"cat":"shopping","name":"%s","url":"%s"}]\n' "$name" "$url" > "$cj"
  ts=$(date '+%H:%M:%S')
  echo "[$ts] ($i/$n) $name ..." | tee -a "$SUMMARY"
  timeout 150 "$BIN" chrome_148_macos "$cj" "$OUT/${name}_res.json" >/dev/null 2>&1
  line=$(python3 -c "
import json,sys
try:
    r=json.load(open('$OUT/${name}_res.json'))['results'][0]
    flip='PASS' if (r['len']>=15000 and r['tag']=='L3-RENDERED') else 'fail'
    print(f\"    {r['tag']:16} len={r['len']:>8} ms={r['ms']:>6} => {flip}\")
except Exception as e:
    print('    NODATA',e)
")
  echo "$line" | tee -a "$SUMMARY"
  if [ "$i" -lt "$n" ]; then
    sleep "$DELAY"
  fi
done
echo "DONE $(date '+%H:%M:%S')" | tee -a "$SUMMARY"
