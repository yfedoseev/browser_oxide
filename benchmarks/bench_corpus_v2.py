#!/usr/bin/env python3
"""
Competitor 126-corpus benchmark — apples-to-apples with browser_oxide.

Same shared classifier as the BO sweep (target/release/examples/classify_stdin).
Adds the customer-facing metrics: per-page wall-clock, peak RSS, network bytes,
cold-start, throughput, failure-mode breakdown, per-vendor pass-rate.

Usage:
    bench_corpus_v2.py <engine> [out.json]
        <engine>: playwright | playwright_stealth | patchright | camoufox |
                  puppeteer | puppeteer_stealth | chromium

Output: a single JSON document with `summary` and `results` fields.
"""
import asyncio
import json
import os
import resource
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CORPUS_FILE = os.environ.get("CORPUS_FILE", "/tmp/corpus.json")
CORPUS = json.load(open(CORPUS_FILE))
CLASSIFY_BIN = REPO / "target" / "release" / "examples" / "classify_stdin"

NAV_TIMEOUT_MS = int(os.environ.get("NAV_TIMEOUT_MS", "45000"))
SETTLE_MS = int(os.environ.get("SETTLE_MS", "2500"))


def classify(html: str):
    """Pipe HTML through the shared Rust classifier -> (tag, len)."""
    p = subprocess.run(
        [str(CLASSIFY_BIN)],
        input=html.encode("utf-8", "replace"),
        capture_output=True,
    )
    out = p.stdout.decode().strip()
    if "\t" in out:
        tag, length = out.split("\t", 1)
        return tag, int(length)
    return "CLASSIFY-ERR", len(html)


def get_rss_mb(pid):
    """Read RSS for a PID via /proc."""
    try:
        with open(f"/proc/{pid}/statm") as f:
            return int(f.read().split()[1]) * 4 / 1024
    except Exception:
        return 0.0


def all_descendant_pids(root_pid):
    """Walk /proc/*/stat finding all descendants of root_pid (the browser
    spawns workers; we want total RSS across the process tree)."""
    out = {root_pid}
    try:
        for entry in Path("/proc").iterdir():
            if not entry.name.isdigit():
                continue
            pid = int(entry.name)
            try:
                with open(entry / "stat") as f:
                    parts = f.read().split()
                    ppid = int(parts[3])
                if ppid in out:
                    out.add(pid)
            except Exception:
                continue
    except Exception:
        pass
    # Take a second pass for grandchildren
    for _ in range(3):
        before = len(out)
        try:
            for entry in Path("/proc").iterdir():
                if not entry.name.isdigit():
                    continue
                pid = int(entry.name)
                if pid in out:
                    continue
                try:
                    with open(entry / "stat") as f:
                        parts = f.read().split()
                        ppid = int(parts[3])
                    if ppid in out:
                        out.add(pid)
                except Exception:
                    continue
        except Exception:
            pass
        if len(out) == before:
            break
    return out


def tree_rss_mb(root_pid):
    """Total RSS across a process tree, MB."""
    total = 0
    for pid in all_descendant_pids(root_pid):
        total += get_rss_mb(pid)
    return total


async def visit(ctx, site, root_pid, rss_tracker, use_cdp=True):
    url, name, cat = site["url"], site["name"], site["cat"]
    page = await ctx.new_page()
    t0 = time.perf_counter()
    err = None
    html = ""
    net_bytes = {"sent": 0, "recv": 0, "n_requests": 0}

    # Network accounting via CDP — Chromium-based engines expose it.
    # Camoufox is Firefox-based, no CDP — use Playwright route events instead.
    client = None
    if use_cdp:
        try:
            client = await page.context.new_cdp_session(page)
            await client.send("Network.enable")

            def on_req(p):
                net_bytes["n_requests"] += 1
                net_bytes["sent"] += int(p.get("request", {}).get("postData", "").__len__() or 0)

            def on_data(p):
                net_bytes["recv"] += int(p.get("dataLength", 0) or 0)

            client.on("Network.requestWillBeSent", on_req)
            client.on("Network.dataReceived", on_data)
        except Exception:
            client = None
    if client is None:
        # Firefox / no-CDP path: count requests via the response listener.
        def on_resp(resp):
            net_bytes["n_requests"] += 1

        page.on("response", on_resp)

    try:
        await page.goto(url, wait_until="load", timeout=NAV_TIMEOUT_MS)
        await page.wait_for_timeout(SETTLE_MS)
        html = await page.content()
    except Exception as e:
        err = f"{type(e).__name__}: {str(e)[:200]}"
        try:
            html = await page.content()
        except Exception:
            html = ""
    ms = int((time.perf_counter() - t0) * 1000)
    rss_now = tree_rss_mb(root_pid)
    rss_tracker["peak"] = max(rss_tracker["peak"], rss_now)

    tag, length = classify(html) if html else ("ERROR" if err else "THIN-BODY", 0)
    try:
        await page.close()
    except Exception:
        pass
    line = (
        f"corpus-v2: {cat} {name} {tag} len={length} ms={ms} "
        f"rss={rss_now:.0f} net_recv={net_bytes['recv']}"
        + (f" err={err}" if err else "")
    )
    print(line, flush=True)
    return {
        "cat": cat,
        "name": name,
        "url": url,
        "tag": tag,
        "len": length,
        "ms": ms,
        "rss_mb": round(rss_now, 1),
        "net_bytes_recv": net_bytes["recv"],
        "n_requests": net_bytes["n_requests"],
        "err": err,
    }


async def run_playwright(stealth=False):
    from playwright.async_api import async_playwright
    rss = {"peak": 0.0}
    results = []
    t0 = time.perf_counter()
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        t_launch = int((time.perf_counter() - t0) * 1000)
        ctx = await browser.new_context()
        # Apply stealth to context if requested
        if stealth:
            try:
                from playwright_stealth import Stealth
                await Stealth().apply_stealth_async(ctx)
            except Exception as e:
                print(f"stealth apply failed: {e}", flush=True)
        # Try to grab the browser process PID
        try:
            root_pid = browser.process.pid if hasattr(browser, 'process') and browser.process else os.getpid()
        except Exception:
            root_pid = os.getpid()
        # First-page-ready
        warm_page = await ctx.new_page()
        await warm_page.goto("about:blank")
        await warm_page.close()
        t_first_page = int((time.perf_counter() - t0) * 1000)
        for site in CORPUS:
            results.append(await visit(ctx, site, root_pid, rss))
        await browser.close()
    return results, t_launch, t_first_page, rss["peak"]


async def run_patchright():
    from patchright.async_api import async_playwright
    rss = {"peak": 0.0}
    results = []
    t0 = time.perf_counter()
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        t_launch = int((time.perf_counter() - t0) * 1000)
        ctx = await browser.new_context()
        try:
            root_pid = browser.process.pid if hasattr(browser, 'process') and browser.process else os.getpid()
        except Exception:
            root_pid = os.getpid()
        warm_page = await ctx.new_page()
        await warm_page.goto("about:blank")
        await warm_page.close()
        t_first_page = int((time.perf_counter() - t0) * 1000)
        for site in CORPUS:
            results.append(await visit(ctx, site, root_pid, rss))
        await browser.close()
    return results, t_launch, t_first_page, rss["peak"]


async def run_camoufox():
    from camoufox.async_api import AsyncCamoufox
    rss = {"peak": 0.0}
    results = []
    t0 = time.perf_counter()
    # Manage AsyncCamoufox lifecycle manually so we can swallow exceptions
    # from `__aexit__`'s `browser.close()` — Firefox/Camoufox's `Browser.close`
    # occasionally returns "Connection closed while reading from driver"
    # at process teardown after the corpus has already been swept (harmless,
    # but it raises in the playwright Python driver and would lose all
    # results without this guard).
    mgr = AsyncCamoufox(headless=True)
    browser = await mgr.__aenter__()
    t_launch = int((time.perf_counter() - t0) * 1000)
    try:
        # Camoufox/Firefox: use `browser.new_page` directly. Calling
        # `browser.new_context()` then `ctx.new_page()` crashes the driver
        # mid-sweep ("Connection closed while reading from the driver").
        # Locate the firefox process for RSS tracking. Use the same pattern
        # as run_playwright/run_patchright: prefer browser.process.pid (the
        # actual firefox-bin launcher; tree_rss_mb walks its descendants
        # including every Web Content / Privileged Content / RDD / GPU /
        # Socket / Utility process). Fall back to os.getpid() which walks
        # the entire Python process tree (still captures firefox-bin since
        # it's a child of Python). The prior "first /proc child whose comm
        # contains fox" heuristic could lock onto a sibling launcher and
        # miss the e10s content processes entirely — reported 48 MB peak
        # for the whole Firefox tree, which is the launcher process alone.
        try:
            root_pid = browser.process.pid if hasattr(browser, 'process') and browser.process else os.getpid()
        except Exception:
            root_pid = os.getpid()

        warm_page = await browser.new_page()
        await warm_page.goto("about:blank")
        await warm_page.close()
        t_first_page = int((time.perf_counter() - t0) * 1000)
        # Use the browser itself as the "context" for visit() — `visit`
        # only calls `.new_page()` on it, which exists on both Context
        # and Browser.
        for site in CORPUS:
            results.append(await visit(browser, site, root_pid, rss, use_cdp=False))
    finally:
        try:
            await mgr.__aexit__(None, None, None)
        except Exception as e:
            print(f"camoufox teardown ignored: {e}", flush=True)
    return results, t_launch, t_first_page, rss["peak"]


def aggregate(engine, results, t_launch, t_first_page, rss_peak, wall_total_ms):
    """Build the per-engine summary across customer-facing metrics."""
    n = len(results)
    pass_count = sum(1 for r in results if r["tag"] == "L3-RENDERED" and r["len"] >= 15000)
    thin_shell = sum(1 for r in results if r["tag"] == "L3-RENDERED" and 1000 <= r["len"] < 15000)
    chl = sum(1 for r in results if "CHL" in r["tag"] or r["tag"] == "BLOCKED" or "PaH" in r["tag"])
    thin_body = sum(1 for r in results if r["tag"] not in {"L3-RENDERED"} and r["len"] < 1000 and not r["err"])
    error = sum(1 for r in results if r["err"])
    timing = sorted(r["ms"] for r in results)
    median = timing[len(timing) // 2] if timing else 0
    p95 = timing[int(len(timing) * 0.95)] if timing else 0
    p99 = timing[int(len(timing) * 0.99)] if timing else 0
    total_recv = sum(r["net_bytes_recv"] for r in results)
    avg_req = sum(r["n_requests"] for r in results) / max(1, n)
    throughput_per_min = 60_000.0 * n / max(1, wall_total_ms)
    # Per-vendor pass-rate. Inferred from corpus category name (we keep
    # this coarse; the corpus categories map approximately to vendor.).
    by_vendor = {}
    for r in results:
        v = r["cat"]
        by_vendor.setdefault(v, {"n": 0, "pass": 0})
        by_vendor[v]["n"] += 1
        if r["tag"] == "L3-RENDERED" and r["len"] >= 15000:
            by_vendor[v]["pass"] += 1

    return {
        "engine": engine,
        "n": n,
        "pass": pass_count,
        "thin_shell": thin_shell,
        "chl": chl,
        "thin_body": thin_body,
        "error": error,
        "pass_pct": round(100.0 * pass_count / n, 1) if n else 0,
        "t_launch_ms": t_launch,
        "t_first_page_ready_ms": t_first_page,
        "rss_peak_mb": round(rss_peak, 1),
        "ms_median": median,
        "ms_p95": p95,
        "ms_p99": p99,
        "wall_total_ms": wall_total_ms,
        "throughput_pages_per_min": round(throughput_per_min, 2),
        "net_total_mb": round(total_recv / (1024 * 1024), 1),
        "avg_requests_per_page": round(avg_req, 1),
        "by_category": by_vendor,
    }


def main():
    engine = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else f"/tmp/stealth-bench/corpus_{engine}.json"
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    if not CLASSIFY_BIN.exists():
        sys.exit(f"classify binary missing: {CLASSIFY_BIN} (cargo build --release --example classify_stdin)")

    print(f"=== CORPUS SWEEP v2: {engine} ({len(CORPUS)} sites) ===", flush=True)
    sweep_t0 = time.perf_counter()
    if engine == "playwright_stealth":
        results, t_launch, t_first, rss = asyncio.run(run_playwright(stealth=True))
    elif engine in ("playwright", "chromium"):
        results, t_launch, t_first, rss = asyncio.run(run_playwright(stealth=False))
    elif engine == "patchright":
        results, t_launch, t_first, rss = asyncio.run(run_patchright())
    elif engine == "camoufox":
        results, t_launch, t_first, rss = asyncio.run(run_camoufox())
    else:
        sys.exit(f"unknown engine: {engine}")
    wall_total_ms = int((time.perf_counter() - sweep_t0) * 1000)

    summary = aggregate(engine, results, t_launch, t_first, rss, wall_total_ms)
    json.dump({"summary": summary, "results": results},
              open(out, "w"), indent=1)
    print(f"\n=== {engine}: pass={summary['pass']}/{summary['n']} "
          f"({summary['pass_pct']}%) wall={wall_total_ms / 1000:.0f}s "
          f"rss_peak={summary['rss_peak_mb']:.0f}MB "
          f"median={summary['ms_median']}ms p95={summary['ms_p95']}ms ===",
          flush=True)
    print(f"  -> {out}", flush=True)


if __name__ == "__main__":
    main()
