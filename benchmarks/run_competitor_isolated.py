#!/usr/bin/env python3
"""Unified per-site ISOLATED competitor runner — fresh browser PER SITE for
ALL engines (playwright, playwright_stealth, patchright, camoufox v135/v150),
so every engine gets the SAME harness as BO's per-site isolation: fresh
browser/jar/profile, generous 12s settle (BO-comparable budget), retry on
driver crash, and BO's exact classify_stdin (zero classifier drift).

Usage: run_competitor_isolated.py <engine> <corpus.json> <out.json> [tag]
  engine: playwright | playwright_stealth | patchright | camoufox
"""
import asyncio
import json
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CLASSIFY = REPO / "target" / "release" / "examples" / "classify_stdin"
DIAG = {"areyouheadless"}
SETTLE = 12


def classify(html: str):
    try:
        out = subprocess.run([str(CLASSIFY)], input=html, capture_output=True,
                             text=True, timeout=30).stdout.strip()
        tag, length = out.split("\t")
        return tag, int(length)
    except Exception:
        return "ERROR", len(html)


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
            await page.goto(site["url"], timeout=45000, wait_until="domcontentloaded")
            await asyncio.sleep(SETTLE)
            return classify(await page.content()) + (None,)
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
        await page.goto(site["url"], timeout=45000, wait_until="domcontentloaded")
        await asyncio.sleep(SETTLE)
        return classify(await page.content()) + (None,)
    finally:
        try:
            await mgr.__aexit__(None, None, None)
        except Exception:
            pass


async def visit_one(site, engine):
    once = _camoufox_once if engine == "camoufox" else (lambda s: _chromium_once(s, engine))
    tag, length, err = "ERROR", 0, None
    for _ in range(5):
        try:
            return await once(site)
        except Exception as e:
            err = str(e)[:160]
            await asyncio.sleep(2)
    return tag, length, err


async def main():
    engine = sys.argv[1]
    corpus = json.loads(Path(sys.argv[2]).read_text())
    out_path = sys.argv[3]
    tag_name = sys.argv[4] if len(sys.argv) > 4 else engine
    results = []
    t0 = time.time()
    for i, site in enumerate(corpus):
        st = time.time()
        try:
            tag, length, err = await asyncio.wait_for(visit_one(site, engine), timeout=180)
        except asyncio.TimeoutError:
            tag, length, err = "TIMEOUT", 0, ">180s"
        ms = int((time.time() - st) * 1000)
        row = {"cat": site.get("cat", ""), "name": site["name"], "url": site["url"],
               "tag": tag, "len": length, "ms": ms, "rss_mb": 0.0, "err": err}
        if site.get("diagnostic"):
            row["diagnostic"] = True
        results.append(row)
        ok = tag == "L3-RENDERED" and length >= 15000
        print(f"[{i+1}/{len(corpus)}] {site['name']:18} {tag:16} len={length:>8} "
              f"ms={ms:>6} {'PASS' if ok else ''}", flush=True)
    prod = [r for r in results if r["name"] not in DIAG]

    def pz(r):
        return r["tag"] == "L3-RENDERED" and r["len"] >= 15000
    summary = {"engine": tag_name, "n": len(results),
               "pass": sum(1 for r in results if pz(r)),
               "production_n": len(prod),
               "production_pass": sum(1 for r in prod if pz(r)),
               "wall_total_ms": int((time.time() - t0) * 1000)}
    Path(out_path).write_text(json.dumps({"summary": summary, "results": results}, indent=2))
    print(f"\n{tag_name}: production {summary['production_pass']}/{summary['production_n']}")


if __name__ == "__main__":
    asyncio.run(main())
