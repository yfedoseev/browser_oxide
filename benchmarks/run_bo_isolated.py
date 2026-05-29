#!/usr/bin/env python3
"""Per-site isolated BO sweep — fresh sweep_metrics process PER SITE.

A single sweep_metrics process running all 126 sites accumulates memory
(V8 isolates / workers / timers from heavy now-passing pages like Amazon)
and eventually runs away (observed: 1.7 GB RSS, 100% CPU, 7 h stuck).
Running each site in its own short-lived process makes that impossible —
every site is a true fresh visitor (fresh jar, fresh isolate).

Outputs a JSON matching sweep_metrics' {summary, results} schema so the
report generator + competitor harness compare field-for-field.

Usage: run_bo_isolated.py <profile> <spaced_corpus.json> <out.json>
Env: BO_SITE_TIMEOUT (default 150s), BO_VENDOR_COOLDOWN_S (default 30,
     applied before AWS/DataDome/Akamai/Kasada-tagged sites to avoid
     same-IP token clustering).
"""
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
try:
    from benchmarks.corpus_vendor_map import SITE_VENDOR
except Exception:
    SITE_VENDOR = {}

REPO = Path(__file__).resolve().parent.parent
BIN = REPO / "target" / "release" / "examples" / "sweep_metrics"
DIAGNOSTIC = {"areyouheadless"}


def main():
    profile, corpus_path, out_path = sys.argv[1], sys.argv[2], sys.argv[3]
    corpus = json.loads(Path(corpus_path).read_text())
    timeout = int(os.environ.get("BO_SITE_TIMEOUT", "150"))
    cooldown = int(os.environ.get("BO_VENDOR_COOLDOWN_S", "30"))
    results = []
    t_start = time.time()
    prev_vendor = None
    for i, site in enumerate(corpus):
        vendor = SITE_VENDOR.get(site["name"])
        # Cool down before a vendor-clustered site if the previous site
        # hit the same vendor (same-IP token clustering guard).
        if vendor and vendor == prev_vendor and cooldown:
            time.sleep(cooldown)
        prev_vendor = vendor
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as cf:
            json.dump([site], cf)
            cf_path = cf.name
        of_path = cf_path + ".out"
        row = {"cat": site.get("cat", ""), "name": site["name"],
               "url": site["url"], "tag": "ERROR", "len": 0, "ms": 0,
               "rss_mb": 0.0, "err": None}
        if site.get("diagnostic"):
            row["diagnostic"] = True
        try:
            subprocess.run([str(BIN), profile, cf_path, of_path],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           timeout=timeout, check=False)
            if os.path.exists(of_path):
                d = json.loads(Path(of_path).read_text())
                if d.get("results"):
                    r = d["results"][0]
                    row.update({k: r.get(k, row[k]) for k in
                                ("tag", "len", "ms", "rss_mb", "err")})
        except subprocess.TimeoutExpired:
            row["tag"], row["err"] = "TIMEOUT", f">{timeout}s"
        except Exception as e:
            row["err"] = str(e)[:200]
        finally:
            for p in (cf_path, of_path):
                try:
                    os.unlink(p)
                except OSError:
                    pass
        results.append(row)
        ok = row["tag"] == "L3-RENDERED" and row["len"] >= 15000
        print(f"[{i+1}/{len(corpus)}] {site['name']:18} {row['tag']:16} "
              f"len={row['len']:>8} ms={row['ms']:>6} {'PASS' if ok else ''}",
              flush=True)

    n = len(results)
    diag = [r for r in results if r["name"] in DIAGNOSTIC]
    prod = [r for r in results if r["name"] not in DIAGNOSTIC]

    def passes(r):
        return r["tag"] == "L3-RENDERED" and r["len"] >= 15000
    summary = {
        "engine": "browser_oxide", "profile": profile, "mode": "isolated",
        "n": n, "pass": sum(1 for r in results if passes(r)),
        "production_n": len(prod),
        "production_pass": sum(1 for r in prod if passes(r)),
        "diagnostic_n": len(diag),
        "wall_total_ms": int((time.time() - t_start) * 1000),
    }
    Path(out_path).write_text(json.dumps({"summary": summary, "results": results}, indent=2))
    print(f"\n{profile}: production {summary['production_pass']}/{summary['production_n']}  "
          f"(raw {summary['pass']}/{n})  wall={summary['wall_total_ms']//60000}min")


if __name__ == "__main__":
    main()
