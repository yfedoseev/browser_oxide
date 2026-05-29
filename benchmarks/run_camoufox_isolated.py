#!/usr/bin/env python3
"""Per-site ISOLATED camoufox runner — fresh camoufox browser PER SITE.

The shared-browser path (bench_corpus_v2 / run_camoufox_min) crashes the
camoufox/playwright driver partway through a sustained 126-site loop
("Connection closed" / "pipe closed"). Relaunching the browser per site makes
that impossible — and mirrors BO's per-site isolation, so the comparison is
fair. Classifies each DOM through BO's classify_stdin (zero classifier drift).

Usage: run_camoufox_isolated.py <corpus.json> <out.json> [version_tag]
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


def classify(html: str):
    try:
        out = subprocess.run([str(CLASSIFY)], input=html, capture_output=True,
                             text=True, timeout=30).stdout.strip()
        tag, length = out.split("\t")
        return tag, int(length)
    except Exception:
        return "ERROR", len(html)


async def visit_one(site):
    """Fresh camoufox browser for a single site."""
    from camoufox.async_api import AsyncCamoufox
    tag, length, err = "ERROR", 0, None
    mgr = AsyncCamoufox(headless=True)
    try:
        browser = await mgr.__aenter__()
        page = await browser.new_page()
        await page.goto(site["url"], timeout=35000, wait_until="load")
        await asyncio.sleep(3)
        html = await page.content()
        tag, length = classify(html)
    except Exception as e:
        err = str(e)[:160]
    finally:
        try:
            await mgr.__aexit__(None, None, None)
        except Exception:
            pass
    return tag, length, err


async def main():
    corpus = json.loads(Path(sys.argv[1]).read_text())
    out_path = sys.argv[2]
    tag_ver = sys.argv[3] if len(sys.argv) > 3 else "camoufox"
    results = []
    t0 = time.time()
    for i, site in enumerate(corpus):
        st = time.time()
        try:
            tag, length, err = await asyncio.wait_for(visit_one(site), timeout=90)
        except asyncio.TimeoutError:
            tag, length, err = "TIMEOUT", 0, ">90s"
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

    def p(r):
        return r["tag"] == "L3-RENDERED" and r["len"] >= 15000
    summary = {"engine": tag_ver, "n": len(results),
               "pass": sum(1 for r in results if p(r)),
               "production_n": len(prod),
               "production_pass": sum(1 for r in prod if p(r)),
               "wall_total_ms": int((time.time() - t0) * 1000)}
    Path(out_path).write_text(json.dumps({"summary": summary, "results": results}, indent=2))
    print(f"\n{tag_ver}: production {summary['production_pass']}/{summary['production_n']}")


if __name__ == "__main__":
    asyncio.run(main())
