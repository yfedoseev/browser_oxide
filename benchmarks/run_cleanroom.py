#!/usr/bin/env python3
"""Clean-room single-engine runner: all 126 sites ONE AT A TIME on a quiet box.

Motivation: the 2026-05-29 competitor sweep ran while the shared box was at
load ~55 (concurrent rust builds + other sessions). camoufox's playwright-
firefox driver crashed under that CPU starvation — every camoufox "ERROR" was
`Connection closed while reading from the driver`, NOT a real block. This
runner removes that confound:

  * STRICTLY sequential — one site, one browser, at a time (no parallelism).
  * LOAD-GATE — before each site, wait until the 1-minute load average drops
    below --max-load (default 6.0 on this 8-core box) so the driver never
    launches into contention. This makes the run robust to OTHER processes on
    the shared box (it simply waits them out).
  * Generous budgets + driver-crash retries: on "Connection closed" / timeout
    it retries up to --retries times with a cooldown, so a transient crash
    doesn't get scored as a failure.
  * Inter-site cooldown lets the previous browser fully reap before the next.

Classification uses BO's exact classify_stdin (zero classifier drift) so the
numbers compare field-for-field with the BO gate.

Usage:
  run_cleanroom.py <engine> <corpus.json> <out.json> [tag]
    engine: playwright | playwright_stealth | patchright | camoufox
Env / flags via env:
  CR_MAX_LOAD     (default 6.0)   1-min loadavg ceiling before launching a site
  CR_SETTLE       (default 12)    seconds to let the page settle after goto
  CR_GOTO_MS      (default 60000) per-goto timeout
  CR_RETRIES      (default 4)     attempts per site (driver-crash recovery)
  CR_COOLDOWN     (default 4)     seconds between sites / retries
  CR_LOAD_WAIT    (default 600)   max seconds to wait for load to drop per site
"""
import asyncio
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CLASSIFY = REPO / "target" / "release" / "examples" / "classify_stdin"
DIAG = {"areyouheadless"}

MAX_LOAD = float(os.environ.get("CR_MAX_LOAD", "25.0"))
# Free RAM (GiB) required before launching a site. Each camoufox Firefox needs
# ~5 GB; on the shared 14 GB box it gets OOM-killed ("Connection closed") if it
# launches low on memory. THE important gate for camoufox.
MIN_FREE_GB = float(os.environ.get("CR_MIN_FREE_GB", "6.0"))
SETTLE = int(os.environ.get("CR_SETTLE", "12"))
GOTO_MS = int(os.environ.get("CR_GOTO_MS", "60000"))
RETRIES = int(os.environ.get("CR_RETRIES", "4"))
COOLDOWN = int(os.environ.get("CR_COOLDOWN", "4"))
LOAD_WAIT = int(os.environ.get("CR_LOAD_WAIT", "600"))

DRIVER_CRASH = ("Connection closed", "Target closed", "Browser closed",
                "crashed", "Timeout", "TimeoutError")


def classify(html: str):
    try:
        out = subprocess.run([str(CLASSIFY)], input=html, capture_output=True,
                             text=True, timeout=30).stdout.strip()
        tag, length = out.split("\t")
        return tag, int(length)
    except Exception:
        return "ERROR", len(html)


def _avail_gb():
    """MemAvailable in GiB (what the kernel thinks is allocatable without swap)."""
    try:
        for line in open("/proc/meminfo"):
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) / (1024 * 1024)
    except Exception:
        pass
    return 999.0


def wait_for_quiet(name):
    """Block until BOTH 1-min loadavg < MAX_LOAD AND free RAM > MIN_FREE_GB
    (or LOAD_WAIT elapses). The memory gate is the important one for camoufox:
    each Firefox needs ~5 GB and gets OOM-killed (`Connection closed`) if it
    launches when the shared 14 GB box is low on memory."""
    t0 = time.time()
    waited = False
    while time.time() - t0 < LOAD_WAIT:
        load1 = os.getloadavg()[0]
        avail = _avail_gb()
        if load1 < MAX_LOAD and avail > MIN_FREE_GB:
            if waited:
                print(f"    [gate] cleared (load={load1:.1f} mem={avail:.1f}GB) "
                      f"after {int(time.time()-t0)}s", flush=True)
            return load1
        if not waited:
            print(f"    [gate] {name}: load={load1:.1f} mem={avail:.1f}GB "
                  f"(need load<{MAX_LOAD} mem>{MIN_FREE_GB}GB), waiting...", flush=True)
            waited = True
        time.sleep(10)
    return os.getloadavg()[0]  # gave up; proceed anyway


async def _chromium_once(site, engine):
    if engine == "patchright":
        from patchright.async_api import async_playwright
    else:
        from playwright.async_api import async_playwright
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        try:
            if engine == "playwright_stealth":
                from playwright_stealth import Stealth
                ctx = await browser.new_context()
                await Stealth().apply_stealth_async(ctx)
                page = await ctx.new_page()
            else:
                page = await browser.new_page()
            await page.goto(site["url"], timeout=GOTO_MS, wait_until="domcontentloaded")
            await asyncio.sleep(SETTLE)
            return classify(await page.content())
        finally:
            try:
                await browser.close()
            except Exception:
                pass


async def _camoufox_once(site):
    from camoufox.async_api import AsyncCamoufox
    mgr = AsyncCamoufox(headless=True)
    try:
        browser = await mgr.__aenter__()
        page = await browser.new_page()
        await page.goto(site["url"], timeout=GOTO_MS, wait_until="domcontentloaded")
        await asyncio.sleep(SETTLE)
        return classify(await page.content())
    finally:
        try:
            await mgr.__aexit__(None, None, None)
        except Exception:
            pass


async def visit_one(site, engine):
    once = _camoufox_once if engine == "camoufox" else (lambda s: _chromium_once(s, engine))
    last_err = None
    for attempt in range(RETRIES):
        # Load-gate BEFORE every attempt so the driver never launches hot.
        wait_for_quiet(site["name"])
        try:
            tag, length = await asyncio.wait_for(once(site), timeout=GOTO_MS / 1000 + SETTLE + 90)
            return tag, length, None, attempt + 1
        except Exception as e:
            last_err = str(e)[:120]
            crash = any(k in last_err for k in DRIVER_CRASH)
            print(f"    [retry {attempt+1}/{RETRIES}] {site['name']}: "
                  f"{'DRIVER-CRASH' if crash else 'err'} {last_err[:60]}", flush=True)
            await asyncio.sleep(COOLDOWN * (attempt + 1))  # back off
    return "ERROR", 0, last_err, RETRIES


async def main():
    engine = sys.argv[1]
    corpus = json.loads(Path(sys.argv[2]).read_text())
    out_path = sys.argv[3]
    tag_name = sys.argv[4] if len(sys.argv) > 4 else engine
    results = []
    t0 = time.time()
    print(f"[cleanroom] {tag_name}: {len(corpus)} sites, sequential, "
          f"max_load={MAX_LOAD}, retries={RETRIES}, settle={SETTLE}s", flush=True)
    for i, site in enumerate(corpus):
        st = time.time()
        tag, length, err, attempts = await visit_one(site, engine)
        ms = int((time.time() - st) * 1000)
        row = {"cat": site.get("cat", ""), "name": site["name"], "url": site["url"],
               "tag": tag, "len": length, "ms": ms, "rss_mb": 0.0,
               "err": err, "attempts": attempts}
        if site.get("diagnostic"):
            row["diagnostic"] = True
        results.append(row)
        ok = tag == "L3-RENDERED" and length >= 15000
        print(f"[{i+1}/{len(corpus)}] {site['name']:18} {tag:16} len={length:>8} "
              f"ms={ms:>6} att={attempts} {'PASS' if ok else ''}", flush=True)
        # checkpoint after every site (resumable / inspectable)
        _write(out_path, tag_name, results, t0)
        time.sleep(COOLDOWN)
    _write(out_path, tag_name, results, t0)
    s = json.load(open(out_path))["summary"]
    print(f"\n{tag_name}: production {s['production_pass']}/{s['production_n']}  "
          f"(raw {s['pass']}/{s['n']})", flush=True)


def _write(out_path, tag_name, results, t0):
    prod = [r for r in results if r["name"] not in DIAG]

    def pz(r):
        return r["tag"] == "L3-RENDERED" and r["len"] >= 15000
    summary = {"engine": tag_name, "n": len(results),
               "pass": sum(1 for r in results if pz(r)),
               "production_n": len(prod),
               "production_pass": sum(1 for r in prod if pz(r)),
               "wall_total_ms": int((time.time() - t0) * 1000)}
    Path(out_path).write_text(json.dumps({"summary": summary, "results": results}, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
