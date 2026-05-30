#!/usr/bin/env python3
"""Controlled peak-RSS measurement, apples-to-apples, one site at a time.

For each site, launches an engine, navigates + settles, and samples the PEAK
resident memory of the WHOLE process tree (browser parent + all children) every
0.5 s. Run on a QUIET box for clean numbers.

  BO        : peak RSS of the sweep_metrics process (single process, no children).
  camoufox  : summed RSS of camoufox-bin parent + all descendants.
  chromium  : summed RSS of the chromium parent + all descendants.

Usage: measure_memory.py <engine> <sites.json> <out.json>
  engine: bo_chrome_148_macos | bo_firefox_135_macos | ... | camoufox | chromium
Env: MM_SETTLE (default 12s).
"""
import asyncio, json, os, sys, time, subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
STABLE = os.environ.get("BO_SWEEP_BIN", str(REPO / "target/release/examples/sweep_metrics"))
SETTLE = int(os.environ.get("MM_SETTLE", "12"))


def tree_rss_kb(root_pid):
    """Summed RSS (kB) of root_pid + all descendants, via /proc."""
    # build child map once per call (cheap enough at 0.5s cadence)
    kids = {}
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            with open(f"/proc/{p}/stat") as f:
                parts = f.read().split()
            ppid = int(parts[3])
            kids.setdefault(ppid, []).append(int(p))
        except Exception:
            pass
    seen, stack, total = set(), [root_pid], 0
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        try:
            with open(f"/proc/{pid}/statm") as f:
                rss_pages = int(f.read().split()[1])
            total += rss_pages * (os.sysconf("SC_PAGE_SIZE") // 1024)
        except Exception:
            pass
        stack.extend(kids.get(pid, []))
    return total


async def sample_during(coro, root_pid_getter):
    """Run coro while polling peak tree RSS every 0.5s."""
    peak = [0]
    stop = [False]
    async def poller():
        while not stop[0]:
            pid = root_pid_getter()
            if pid:
                peak[0] = max(peak[0], tree_rss_kb(pid))
            await asyncio.sleep(0.5)
    t = asyncio.create_task(poller())
    try:
        await coro
    finally:
        stop[0] = True
        await t
    return peak[0]


async def measure_playwright(site, engine):
    from playwright.async_api import async_playwright
    if engine == "camoufox":
        from camoufox.async_api import AsyncCamoufox
        mgr = AsyncCamoufox(headless=True)
        browser = await mgr.__aenter__()
        root = {"pid": None}
        # find camoufox-bin pid
        out = subprocess.run(["pgrep", "-f", "camoufox-bin"], capture_output=True, text=True)
        pids = [int(x) for x in out.stdout.split()]
        root["pid"] = min(pids) if pids else None
        async def nav():
            page = await browser.new_page()
            await page.goto(site["url"], timeout=60000, wait_until="domcontentloaded")
            await asyncio.sleep(SETTLE)
        peak = await sample_during(nav(), lambda: root["pid"])
        await mgr.__aexit__(None, None, None)
        return peak
    else:  # chromium
        async with async_playwright() as p:
            browser = await p.chromium.launch(headless=True, args=["--no-sandbox"])
            out = subprocess.run(["pgrep", "-f", "chrome.*--headless|chromium"], capture_output=True, text=True)
            pids = [int(x) for x in out.stdout.split()] or [os.getpid()]
            root = {"pid": min(pids)}
            async def nav():
                page = await browser.new_page()
                await page.goto(site["url"], timeout=60000, wait_until="domcontentloaded")
                await asyncio.sleep(SETTLE)
            peak = await sample_during(nav(), lambda: root["pid"])
            await browser.close()
            return peak


def measure_bo(site, profile):
    """Run sweep_metrics on one site, sampling its RSS in a poller thread."""
    import threading
    cf = Path(f"/tmp/mm_{site['name']}.json"); cf.write_text(json.dumps([site]))
    of = str(cf) + ".out"
    proc = subprocess.Popen([STABLE, profile, str(cf), of],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    peak = [0]
    def poll():
        while proc.poll() is None:
            peak[0] = max(peak[0], tree_rss_kb(proc.pid))
            time.sleep(0.5)
    t = threading.Thread(target=poll); t.start()
    proc.wait(timeout=200); t.join()
    for f in (cf, Path(of)):
        try: f.unlink()
        except OSError: pass
    return peak[0]


async def main():
    engine = sys.argv[1]; sites = json.loads(Path(sys.argv[2]).read_text()); out = sys.argv[3]
    rows = []
    for s in sites:
        st = time.time()
        if engine.startswith("bo_"):
            peak = measure_bo(s, engine[3:])
        else:
            peak = await measure_playwright(s, engine)
        rows.append({"name": s["name"], "peak_rss_mb": round(peak / 1024, 1)})
        print(f"  {s['name']:18} peak={peak/1024:7.1f} MB  ({int(time.time()-st)}s)", flush=True)
    peaks = sorted(r["peak_rss_mb"] for r in rows)
    summary = {"engine": engine, "n": len(rows),
               "median_mb": peaks[len(peaks)//2] if peaks else 0,
               "max_mb": max(peaks) if peaks else 0}
    Path(out).write_text(json.dumps({"summary": summary, "results": rows}, indent=2))
    print(f"\n{engine}: median={summary['median_mb']}MB max={summary['max_mb']}MB", flush=True)


if __name__ == "__main__":
    asyncio.run(main())
