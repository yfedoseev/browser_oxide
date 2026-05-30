#!/usr/bin/env python3
"""Per-site isolated BO sweep — fresh sweep_metrics process PER SITE.

A single sweep_metrics process running all 126 sites accumulates memory
(V8 isolates / workers / timers from heavy now-passing pages like Amazon)
and eventually runs away (observed: 1.7 GB RSS, 100% CPU, 7 h stuck).
Running each site in its own short-lived process makes that impossible —
every site is a true fresh visitor (fresh jar, fresh isolate). With the
disk-cached V8 snapshot (snapshot.rs) the per-process init is ~0.1-0.3 s
(restore) instead of ~1.5-1.8 s (rebuild), so the per-site model is cheap.

Outputs a JSON matching sweep_metrics' {summary, results} schema so the
report generator + competitor harness compare field-for-field.

Usage: run_bo_isolated.py <profile> <spaced_corpus.json> <out.json>
       [--parallel N]
Env:
  BO_SITE_TIMEOUT       per-site timeout, default 150s
  BO_VENDOR_COOLDOWN_S  sequential-mode cooldown before a same-vendor site,
                        default 30s
  BO_PARALLEL           parallelism (overridden by --parallel); default 1
  BO_VENDOR_SPACING_S   parallel-mode min gap between two SAME-vendor starts,
                        default 150s (same-IP token-clustering guard)

Parallel scheduler (BO is nav-bound + every site is its own process, so it
parallelizes across idle cores) with two vendor constraints:
  * never two same-vendor sites in flight at once, and
  * >= BO_VENDOR_SPACING_S between two same-vendor STARTS.
AWS/DataDome/Akamai/Kasada thus serialize within their vendor; everything
else runs up to N at a time.
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
# Allow pointing at a stable copy of the binary (BO_SWEEP_BIN) so a concurrent
# `cargo` in the shared target/ dir can't delete it mid-run.
BIN = Path(os.environ.get("BO_SWEEP_BIN",
                          str(REPO / "target" / "release" / "examples" / "sweep_metrics")))
DIAGNOSTIC = {"areyouheadless"}


def _blank_row(site):
    row = {"cat": site.get("cat", ""), "name": site["name"],
           "url": site["url"], "tag": "ERROR", "len": 0, "ms": 0,
           "rss_mb": 0.0, "err": None}
    if site.get("diagnostic"):
        row["diagnostic"] = True
    return row


def _collect(of_path, row):
    """Merge a finished sweep_metrics out-file into `row` (mutates + returns)."""
    if os.path.exists(of_path):
        try:
            d = json.loads(Path(of_path).read_text())
            if d.get("results"):
                r = d["results"][0]
                row.update({k: r.get(k, row[k])
                            for k in ("tag", "len", "ms", "rss_mb", "err")})
        except Exception as e:  # noqa: BLE001
            row["err"] = f"parse: {str(e)[:160]}"
    return row


def _print_row(i, total, site, row):
    ok = row["tag"] == "L3-RENDERED" and row["len"] >= 15000
    print(f"[{i + 1}/{total}] {site['name']:18} {row['tag']:16} "
          f"len={row['len']:>8} ms={row['ms']:>6} {'PASS' if ok else ''}",
          flush=True)


def _wait_for_quiet():
    """Block until 1-min loadavg < BO_MAX_LOAD (clean-room mode). Off by default."""
    maxload = float(os.environ.get("BO_MAX_LOAD", "0") or "0")
    if maxload <= 0:
        return
    t0 = time.time()
    while time.time() - t0 < 600:
        if os.getloadavg()[0] < maxload:
            return
        time.sleep(10)


def run_sequential(corpus, profile, timeout, cooldown):
    results = [None] * len(corpus)
    prev_vendor = None
    for i, site in enumerate(corpus):
        _wait_for_quiet()
        vendor = SITE_VENDOR.get(site["name"])
        if vendor and vendor == prev_vendor and cooldown:
            time.sleep(cooldown)
        prev_vendor = vendor
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as cf:
            json.dump([site], cf)
            cf_path = cf.name
        of_path = cf_path + ".out"
        row = _blank_row(site)
        try:
            subprocess.run([str(BIN), profile, cf_path, of_path],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           timeout=timeout, check=False)
            _collect(of_path, row)
        except subprocess.TimeoutExpired:
            row["tag"], row["err"] = "TIMEOUT", f">{timeout}s"
        except Exception as e:  # noqa: BLE001
            row["err"] = str(e)[:200]
        finally:
            for p in (cf_path, of_path):
                try:
                    os.unlink(p)
                except OSError:
                    pass
        results[i] = row
        _print_row(i, len(corpus), site, row)
    return results


def run_parallel(corpus, profile, timeout, par, spacing):
    """Vendor-aware parallel scheduler. Returns results in corpus order."""
    total = len(corpus)
    results = [None] * total
    pending = list(range(total))            # corpus indices not yet started
    in_flight = {}                          # idx -> dict(popen, paths, t0, vendor)
    vendor_inflight = set()                 # vendors with a live process
    vendor_last_start = {}                  # vendor -> monotonic start time
    done = 0

    # Pre-warm the disk snapshot cache with a single process so the first
    # parallel wave restores (~0.1 s) instead of N cold rebuilds at once.
    if pending:
        seed_idx = pending.pop(0)
        site = corpus[seed_idx]
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as cf:
            json.dump([site], cf)
            cf_path = cf.name
        of_path = cf_path + ".out"
        row = _blank_row(site)
        try:
            subprocess.run([str(BIN), profile, cf_path, of_path],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                           timeout=timeout, check=False)
            _collect(of_path, row)
        except subprocess.TimeoutExpired:
            row["tag"], row["err"] = "TIMEOUT", f">{timeout}s"
        except Exception as e:  # noqa: BLE001
            row["err"] = str(e)[:200]
        finally:
            for p in (cf_path, of_path):
                try:
                    os.unlink(p)
                except OSError:
                    pass
        results[seed_idx] = row
        done += 1
        _print_row(seed_idx, total, site, row)

    def launch(idx):
        site = corpus[idx]
        vendor = SITE_VENDOR.get(site["name"])
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as cf:
            json.dump([site], cf)
            cf_path = cf.name
        of_path = cf_path + ".out"
        p = subprocess.Popen([str(BIN), profile, cf_path, of_path],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        in_flight[idx] = {"p": p, "cf": cf_path, "of": of_path,
                          "t0": time.monotonic(), "vendor": vendor, "site": site}
        if vendor:
            vendor_inflight.add(vendor)
            vendor_last_start[vendor] = time.monotonic()

    def finish(idx, timed_out=False):
        st = in_flight.pop(idx)
        row = _blank_row(st["site"])
        if timed_out:
            try:
                st["p"].kill()
            except OSError:
                pass
            row["tag"], row["err"] = "TIMEOUT", f">{timeout}s"
        else:
            _collect(st["of"], row)
        for p in (st["cf"], st["of"]):
            try:
                os.unlink(p)
            except OSError:
                pass
        if st["vendor"]:
            vendor_inflight.discard(st["vendor"])
        results[idx] = row
        return row

    while pending or in_flight:
        # 1) reap finished / timed-out processes
        for idx in list(in_flight):
            st = in_flight[idx]
            if st["p"].poll() is not None:
                row = finish(idx)
                done += 1
                _print_row(idx, total, st["site"], row)
            elif time.monotonic() - st["t0"] > timeout:
                row = finish(idx, timed_out=True)
                done += 1
                _print_row(idx, total, st["site"], row)

        # 2) launch as many eligible pending sites as slots allow
        now = time.monotonic()
        for idx in list(pending):
            if len(in_flight) >= par:
                break
            vendor = SITE_VENDOR.get(corpus[idx]["name"])
            if vendor:
                if vendor in vendor_inflight:
                    continue
                if now - vendor_last_start.get(vendor, -1e18) < spacing:
                    continue
            pending.remove(idx)
            launch(idx)

        if pending or in_flight:
            time.sleep(0.5)

    return results


def main():
    argv = sys.argv[1:]
    par = int(os.environ.get("BO_PARALLEL", "1"))
    if "--parallel" in argv:
        k = argv.index("--parallel")
        par = int(argv[k + 1])
        del argv[k:k + 2]
    profile, corpus_path, out_path = argv[0], argv[1], argv[2]
    corpus = json.loads(Path(corpus_path).read_text())
    timeout = int(os.environ.get("BO_SITE_TIMEOUT", "150"))
    cooldown = int(os.environ.get("BO_VENDOR_COOLDOWN_S", "30"))
    spacing = int(os.environ.get("BO_VENDOR_SPACING_S", "150"))

    t_start = time.time()
    if par > 1:
        print(f"[run_bo_isolated] parallel={par} spacing={spacing}s "
              f"timeout={timeout}s sites={len(corpus)}", flush=True)
        results = run_parallel(corpus, profile, timeout, par, spacing)
    else:
        results = run_sequential(corpus, profile, timeout, cooldown)

    n = len(results)
    diag = [r for r in results if r["name"] in DIAGNOSTIC]
    prod = [r for r in results if r["name"] not in DIAGNOSTIC]

    def passes(r):
        return r["tag"] == "L3-RENDERED" and r["len"] >= 15000
    summary = {
        "engine": "browser_oxide", "profile": profile,
        "mode": f"isolated-par{par}" if par > 1 else "isolated",
        "n": n, "pass": sum(1 for r in results if passes(r)),
        "production_n": len(prod),
        "production_pass": sum(1 for r in prod if passes(r)),
        "diagnostic_n": len(diag),
        "wall_total_ms": int((time.time() - t_start) * 1000),
    }
    Path(out_path).write_text(json.dumps({"summary": summary, "results": results}, indent=2))
    print(f"\n{profile}: production {summary['production_pass']}/{summary['production_n']}  "
          f"(raw {summary['pass']}/{n})  wall={summary['wall_total_ms']//60000}min", flush=True)


if __name__ == "__main__":
    main()
