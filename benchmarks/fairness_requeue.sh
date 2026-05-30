#!/usr/bin/env bash
# Queued fairness diagnostics — runs AFTER the camoufox driver-fix re-run frees
# the IP. Probes the suspected HARNESS-side failures on each engine (parallel to
# the camoufox playwright-driver bug) so no engine loses points to our tooling.
# Read-only on results; gathers data for analysis (does NOT auto-edit numbers).
set -uo pipefail
cd /home/yfedoseev/projects/browser_oxide
OUT=docs/benchmarks/runs/2026-05-30_fairness_diag
mkdir -p "$OUT"
STABLE=/tmp/warm_verify/sweep_stable
note(){ echo "[$(date +%H:%M:%S)] $*"; }

note "WAIT: camoufox driver-fix re-run to finish (free the IP)"
while pgrep -f 'run_cleanroom.py camoufox' >/dev/null 2>&1; do sleep 120; done
note "camoufox re-run done — starting fairness diagnostics"

# 1) CHROMIUM: the 3 ERR_HTTP2_PROTOCOL_ERROR sites + cnn THIN. Test whether a
#    harness change (retry / wait_until=load / longer settle / fresh context per
#    try) recovers them, vs a genuine block.
note "CHROMIUM diagnostic probe (hotels/costco/washingtonpost/cnn)"
/tmp/bo-venv/bin/python - > "$OUT/chromium_probe.log" 2>&1 <<'PY'
import asyncio, time
from playwright.async_api import async_playwright
SITES=[("hotels","https://www.hotels.com/"),("costco","https://www.costco.com/"),
       ("washingtonpost","https://www.washingtonpost.com/"),("cnn","https://www.cnn.com/")]
async def once(p, url, wait_until, settle, retries):
    last=""
    for _ in range(retries):
        b=await p.chromium.launch(headless=True, args=["--no-sandbox"])
        try:
            pg=await b.new_page()
            await pg.goto(url, timeout=60000, wait_until=wait_until)
            await asyncio.sleep(settle)
            html=await pg.content()
            await b.close()
            return f"OK len={len(html)}"
        except Exception as e:
            last=str(e)[:55]
            try: await b.close()
            except Exception: pass
            await asyncio.sleep(3)
    return f"FAIL {last}"
async def main():
    async with async_playwright() as p:
        for name,url in SITES:
            print(f"\n### {name}")
            for label,wu,settle,rt in [("A baseline dcl/12s","domcontentloaded",12,1),
                                       ("B retry3 dcl/12s","domcontentloaded",12,3),
                                       ("C load/15s","load",15,2),
                                       ("D dcl/25s longsettle","domcontentloaded",25,2)]:
                st=time.time(); r=await once(p,url,wu,settle,rt)
                print(f"  {label:24} {r:32} ms={int((time.time()-st)*1000)}")
asyncio.run(main())
PY
note "chromium probe done -> $OUT/chromium_probe.log"

# 2) BO: the 2 AWS nav-loop errors, spaced + generous budget (one at a time).
note "BO AWS spaced re-test (amazon-in@iphone, amazon-com-au@firefox)"
python3 -c "import json;json.dump([s for s in json.load(open('/tmp/corpus.json')) if s['name']=='amazon-in'],open('$OUT/amzin.json','w'))"
python3 -c "import json;json.dump([s for s in json.load(open('/tmp/corpus.json')) if s['name']=='amazon-com-au'],open('$OUT/amzau.json','w'))"
BO_SWEEP_BIN=$STABLE BO_SITE_TIMEOUT=200 $STABLE iphone_15_pro_safari_18 "$OUT/amzin.json" "$OUT/amzin_out.json" >"$OUT/bo_amzin.log" 2>&1 || true
sleep 180   # space same-vendor (AWS) per token-clustering policy
BO_SWEEP_BIN=$STABLE BO_SITE_TIMEOUT=200 $STABLE firefox_135_macos "$OUT/amzau.json" "$OUT/amzau_out.json" >"$OUT/bo_amzau.log" 2>&1 || true
note "BO AWS re-test done"
echo "amazon-in(iphone):  $(python3 -c "import json;r=json.load(open('$OUT/amzin_out.json'))['results'][0];print(r['tag'],r['len'])" 2>/dev/null)"
echo "amazon-com-au(ffx): $(python3 -c "import json;r=json.load(open('$OUT/amzau_out.json'))['results'][0];print(r['tag'],r['len'])" 2>/dev/null)"
note "FAIRNESS DIAGNOSTICS COMPLETE"
