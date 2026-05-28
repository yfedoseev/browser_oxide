#!/usr/bin/env python3
"""Sprint 1.2 + 1.3 wrapper: per-vendor sub-process isolation + cool-down.

Drives `target/release/examples/sweep_metrics` in N+M fresh subprocesses
so each vendor-cluster site (AWS WAF / DataDome / Akamai / Kasada /
Twitter) gets a fresh HTTP/2 connection pool, fresh cookie jar, and
fresh V8 isolate state — mimicking a "fresh visitor" as closely as
possible on a single datacenter IP (multi-IP is not available; see
HANDOFF_2026_05_27).

Untagged sites (the majority — ~108 of 126) batch in one sub-process
to amortize startup. The corpus is traversed in input order so the
upstream vendor-aware spacing pass (`corpus_vendor_map.space_by_vendor`)
still controls the request sequence.

Sprint 1.3: between two consecutive chunks whose vendor tag matches,
sleep `BO_GATE_VENDOR_COOLDOWN_S` seconds (default 45). Untagged batch
chunks have vendor=None and never trigger cool-down.

Output: a JSON file matching `sweep_metrics`'s schema field-for-field
so `run_fix12_gate.sh` aggregation (jq queries on `summary.pass`, etc.)
keeps working unchanged.

Usage:
    run_sweep_isolated.py <profile> <spaced_corpus.json> <out.json>

Env passthrough (forwarded to every spawned sweep_metrics child):
    BROWSER_OXIDE_SAMPLE_PROFILE   — per-site sampling (FIX-E2)
    BROWSER_OXIDE_SWEEP_POOL       — pool vs cold-page mode
    BO_GATE_VENDOR_COOLDOWN_S      — seconds between same-vendor chunks (default 45)
    BO_GATE_ISOLATE_DISABLE        — if set, skip isolation and behave
                                     as a thin shim over sweep_metrics
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO_ROOT))
from benchmarks.corpus_vendor_map import SITE_VENDOR  # noqa: E402

SWEEP_BIN = REPO_ROOT / "target" / "release" / "examples" / "sweep_metrics"


def chunk_by_vendor(corpus):
    """Yield (vendor_tag_or_None, [site,...]) chunks preserving order.

    Each vendor-tagged site is its own 1-element chunk. Consecutive
    untagged sites group into one batch chunk.
    """
    batch = []
    for site in corpus:
        v = SITE_VENDOR.get(site["name"])
        if v is None:
            batch.append(site)
        else:
            if batch:
                yield (None, batch)
                batch = []
            yield (v, [site])
    if batch:
        yield (None, batch)


def run_chunk(profile: str, sites: list, out_dir: Path, idx: int) -> Path:
    """Spawn sweep_metrics for `sites`. Returns the path to the JSON
    summary it wrote. On per-chunk failure (non-zero exit), we still
    pick up whatever the .partial captured."""
    corpus_file = out_dir / f"chunk_{idx:03d}_corpus.json"
    out_file = out_dir / f"chunk_{idx:03d}_out.json"
    corpus_file.write_text(json.dumps(sites))
    env = os.environ.copy()
    # Forward sampling + pool toggles; sweep_metrics already reads them.
    cmd = [str(SWEEP_BIN), profile, str(corpus_file), str(out_file)]
    print(f"[isolated] chunk {idx} n={len(sites)} -> {cmd}", flush=True)
    rc = subprocess.call(cmd, env=env)
    if rc != 0:
        print(f"[isolated] chunk {idx} exit={rc} (partial may exist)", flush=True)
    return out_file


def read_partial(out_file: Path) -> list:
    """Read per-site SiteResult JSON lines from `<out_file>.partial`,
    falling back to the aggregate JSON if .partial is missing."""
    partial = out_file.with_suffix(out_file.suffix + ".partial")
    if partial.exists():
        return [json.loads(l) for l in partial.read_text().splitlines() if l.strip()]
    if out_file.exists():
        return json.loads(out_file.read_text()).get("results", [])
    return []


def aggregate(profile: str, mode: str, results: list, wall_total_ms: int, diagnostic_names: set):
    """Re-aggregate to match sweep_metrics.rs Summary schema."""
    n = len(results)
    pass_count = sum(1 for r in results if r["tag"] == "L3-RENDERED" and r["len"] >= 15000)
    thin_shell = sum(1 for r in results if r["tag"] == "L3-RENDERED" and 1000 <= r["len"] < 15000)
    chl = sum(
        1 for r in results if "CHL" in r["tag"] or r["tag"] == "BLOCKED" or "PaH" in r["tag"]
    )
    thin_body = sum(
        1
        for r in results
        if r["tag"] != "L3-RENDERED" and r["len"] < 1000 and not r.get("err")
    )
    error = sum(1 for r in results if r.get("err"))
    timings = sorted(r["ms"] for r in results)
    median = timings[len(timings) // 2] if timings else 0
    p95 = timings[int(len(timings) * 0.95)] if timings else 0
    p99 = timings[int(len(timings) * 0.99)] if timings else 0
    by_cat = {}
    for r in results:
        b = by_cat.setdefault(r["cat"], {"n": 0, "pass": 0})
        b["n"] += 1
        if r["tag"] == "L3-RENDERED" and r["len"] >= 15000:
            b["pass"] += 1
    diagnostic_n = sum(1 for r in results if r["name"] in diagnostic_names)
    production_n = n - diagnostic_n
    production_pass = sum(
        1
        for r in results
        if r["name"] not in diagnostic_names
        and r["tag"] == "L3-RENDERED"
        and r["len"] >= 15000
    )
    rss_peak = max((r.get("rss_mb", 0) or 0) for r in results) if results else 0.0
    return {
        "engine": "browser_oxide",
        "profile": profile,
        "mode": mode,
        "n": n,
        "pass": pass_count,
        "thin_shell": thin_shell,
        "chl": chl,
        "thin_body": thin_body,
        "error": error,
        "pass_pct": round(100.0 * pass_count / max(1, n), 1),
        "diagnostic_n": diagnostic_n,
        "production_n": production_n,
        "production_pass": production_pass,
        "production_pass_pct": round(100.0 * production_pass / max(1, production_n), 1),
        "t_launch_ms": 0,
        "t_first_page_ready_ms": 0,
        "rss_peak_mb": round(rss_peak, 1),
        "ms_median": median,
        "ms_p95": p95,
        "ms_p99": p99,
        "wall_total_ms": wall_total_ms,
        "throughput_pages_per_min": round(60_000.0 * n / max(1, wall_total_ms), 2),
        "by_category": by_cat,
        "isolated_by_vendor": True,
    }


def main():
    if len(sys.argv) != 4:
        print("usage: run_sweep_isolated.py <profile> <corpus.json> <out.json>", file=sys.stderr)
        sys.exit(2)
    profile, corpus_path, out_path = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])

    if os.environ.get("BO_GATE_ISOLATE_DISABLE"):
        # Pass-through to sweep_metrics. Useful for A/B vs the isolated path.
        env = os.environ.copy()
        rc = subprocess.call([str(SWEEP_BIN), profile, str(corpus_path), str(out_path)], env=env)
        sys.exit(rc)

    cooldown_s = float(os.environ.get("BO_GATE_VENDOR_COOLDOWN_S", "45"))
    mode = "pool" if os.environ.get("BROWSER_OXIDE_SWEEP_POOL") else "cold"
    corpus = json.loads(corpus_path.read_text())
    diagnostic_names = {s["name"] for s in corpus if s.get("diagnostic")}

    chunks = list(chunk_by_vendor(corpus))
    print(
        f"[isolated] {len(corpus)} sites -> {len(chunks)} chunks "
        f"({sum(1 for v,_ in chunks if v)} vendor-isolated, "
        f"{sum(1 for v,_ in chunks if v is None)} batches), "
        f"cool-down={cooldown_s}s",
        flush=True,
    )

    # Merged-partial file: append per-site results AFTER each chunk
    # completes so a `timeout` kill of THIS wrapper still leaves data
    # the gate's reconstruct-from-partial path can pick up.
    merged_partial = out_path.parent / (out_path.name + ".partial")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    if merged_partial.exists():
        merged_partial.unlink()

    with tempfile.TemporaryDirectory(prefix="bo_isolated_") as tmp:
        tmp = Path(tmp)
        t0 = time.time()
        all_results = []
        prev_vendor = None
        for idx, (vendor, sites) in enumerate(chunks):
            # Sprint 1.3: same-vendor cool-down between chunks. With the
            # vendor-aware spacing pass (Sprint 1.1) ensuring no two
            # vendor-tagged sites are *adjacent*, this fires only when
            # spacing couldn't fully separate the cluster.
            if vendor is not None and vendor == prev_vendor and cooldown_s > 0:
                print(f"[isolated] vendor={vendor} cool-down {cooldown_s}s", flush=True)
                time.sleep(cooldown_s)
            out_file = run_chunk(profile, sites, tmp, idx)
            chunk_results = read_partial(out_file)
            all_results.extend(chunk_results)
            with merged_partial.open("a") as f:
                for r in chunk_results:
                    f.write(json.dumps(r) + "\n")
            if vendor is not None:
                prev_vendor = vendor
            # Untagged batch resets prev_vendor (so cool-down only fires
            # when truly back-to-back tagged chunks).
            elif vendor is None:
                prev_vendor = None

        wall_total_ms = int((time.time() - t0) * 1000)
        summary = aggregate(profile, mode, all_results, wall_total_ms, diagnostic_names)

        out_path.write_text(json.dumps({"summary": summary, "results": all_results}, indent=2))
        print(
            f"[isolated] done: pass={summary['pass']}/{summary['n']} "
            f"production={summary['production_pass']}/{summary['production_n']} "
            f"wall={wall_total_ms // 1000}s -> {out_path}",
            flush=True,
        )


if __name__ == "__main__":
    main()
