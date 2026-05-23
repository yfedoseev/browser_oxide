#!/usr/bin/env python3
"""
126-corpus competitor benchmark — apples-to-apples with browser_oxide.

For a single engine (selected by argv[1]), navigate every URL in the
shared corpus (/tmp/corpus.json, extracted from holistic_sweep.rs),
capture the *final rendered DOM* (page.content()), and classify it with
the SAME classifier browser_oxide uses for its own sweep — the
`classify_stdin` example binary calling `browser::engine_classify`. This
guarantees zero classifier drift between us and the competitors.

Usage:
    python3 bench_corpus.py <engine> [out.json]

    <engine> in: playwright_stealth | patchright | camoufox | chromium

Output: JSON list of {cat,name,url,tag,len,ms,err} + a summary line.
A site counts as a PASS iff tag == "L3-RENDERED" (identical rule to the
browser_oxide ledger).
"""
import asyncio
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CORPUS = json.load(open(os.environ.get("CORPUS_FILE", "/tmp/corpus.json")))
CLASSIFY_BIN = REPO / "target" / "release" / "examples" / "classify_stdin"

NAV_TIMEOUT_MS = 45000
SETTLE_MS = 2500  # let SPA/challenge JS run after load, like our engine's n=3 ticks


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


async def grab(page) -> str:
    try:
        return await page.content()
    except Exception:
        return ""


async def run_playwright(stealth: bool):
    from playwright.async_api import async_playwright
    results = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        ctx = await browser.new_context()
        if stealth:
            from playwright_stealth import Stealth
            await Stealth().apply_stealth_async(ctx)
        for site in CORPUS:
            results.append(await visit(ctx, site))
        await browser.close()
    return results


async def run_patchright():
    from patchright.async_api import async_playwright
    results = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        ctx = await browser.new_context()
        for site in CORPUS:
            results.append(await visit(ctx, site))
        await browser.close()
    return results


async def run_camoufox():
    from camoufox.async_api import AsyncCamoufox
    results = []
    async with AsyncCamoufox(headless=True) as browser:
        ctx = await browser.new_context()
        for site in CORPUS:
            results.append(await visit(ctx, site))
    return results


async def visit(ctx, site):
    url, name, cat = site["url"], site["name"], site["cat"]
    page = await ctx.new_page()
    t0 = time.time()
    err = None
    html = ""
    try:
        await page.goto(url, wait_until="load", timeout=NAV_TIMEOUT_MS)
        await page.wait_for_timeout(SETTLE_MS)
        html = await grab(page)
    except Exception as e:
        err = f"{type(e).__name__}: {str(e)[:120]}"
        html = await grab(page)  # capture whatever rendered (challenge page etc.)
    ms = int((time.time() - t0) * 1000)
    tag, length = classify(html) if html else ("ERROR" if err else "THIN-BODY", 0)
    try:
        await page.close()
    except Exception:
        pass
    line = f"corpus-end: {cat} {name} {tag} len={length} ms={ms}" + (f" err={err}" if err else "")
    print(line, flush=True)
    return {"cat": cat, "name": name, "url": url, "tag": tag, "len": length, "ms": ms, "err": err}


def main():
    engine = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else f"/tmp/stealth-bench/corpus_{engine}.json"
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    if not CLASSIFY_BIN.exists():
        sys.exit(f"classify binary missing: {CLASSIFY_BIN} (cargo build --release --example classify_stdin)")

    print(f"=== CORPUS SWEEP: {engine} ({len(CORPUS)} sites) ===", flush=True)
    if engine == "playwright_stealth":
        results = asyncio.run(run_playwright(stealth=True))
    elif engine == "chromium":
        results = asyncio.run(run_playwright(stealth=False))
    elif engine == "patchright":
        results = asyncio.run(run_patchright())
    elif engine == "camoufox":
        results = asyncio.run(run_camoufox())
    else:
        sys.exit(f"unknown engine: {engine}")

    passed = sum(1 for r in results if r["tag"] == "L3-RENDERED")
    json.dump({"engine": engine, "passed": passed, "total": len(results), "results": results},
              open(out, "w"), indent=2)
    print(f"\n=== {engine}: {passed}/{len(results)} L3-RENDERED ===  -> {out}", flush=True)


if __name__ == "__main__":
    main()
