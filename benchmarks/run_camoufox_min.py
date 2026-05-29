#!/usr/bin/env python3
"""Minimal, robust camoufox corpus runner (bypasses the flaky bench_corpus_v2
warm-page/process-pid path that crashes the driver on new_page).

Mirrors the working smoke pattern: one AsyncCamoufox, a fresh new_page per
site, goto + content, classify the final DOM through BO's own classify_stdin
(EXACT engine_classify — zero classifier drift), record tag+len. Output JSON
matches the {summary, results} schema.

Usage: run_camoufox_min.py <spaced_corpus.json> <out.json>
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


async def main():
    corpus = json.loads(Path(sys.argv[1]).read_text())
    out_path = sys.argv[2]
    from camoufox.async_api import AsyncCamoufox
    results = []
    t0 = time.time()
    mgr = AsyncCamoufox(headless=True)
    browser = await mgr.__aenter__()
    try:
        for i, site in enumerate(corpus):
            tag, length, err = "ERROR", 0, None
            st = time.time()
            page = None
            try:
                page = await browser.new_page()
                await page.goto(site["url"], timeout=35000, wait_until="load")
                await asyncio.sleep(2)  # let SPA/challenge JS settle
                html = await page.content()
                tag, length = classify(html)
            except Exception as e:
                err = str(e)[:160]
            finally:
                if page:
                    try:
                        await page.close()
                    except Exception:
                        pass
            ms = int((time.time() - st) * 1000)
            row = {"cat": site.get("cat", ""), "name": site["name"],
                   "url": site["url"], "tag": tag, "len": length, "ms": ms,
                   "rss_mb": 0.0, "err": err}
            if site.get("diagnostic"):
                row["diagnostic"] = True
            results.append(row)
            ok = tag == "L3-RENDERED" and length >= 15000
            print(f"[{i+1}/{len(corpus)}] {site['name']:18} {tag:16} "
                  f"len={length:>8} ms={ms:>6} {'PASS' if ok else ''}", flush=True)
    finally:
        try:
            await mgr.__aexit__(None, None, None)
        except Exception:
            pass

    prod = [r for r in results if r["name"] not in DIAG]

    def p(r):
        return r["tag"] == "L3-RENDERED" and r["len"] >= 15000
    summary = {"engine": "camoufox", "n": len(results),
               "pass": sum(1 for r in results if p(r)),
               "production_n": len(prod),
               "production_pass": sum(1 for r in prod if p(r)),
               "wall_total_ms": int((time.time() - t0) * 1000)}
    Path(out_path).write_text(json.dumps({"summary": summary, "results": results}, indent=2))
    print(f"\ncamoufox: production {summary['production_pass']}/{summary['production_n']}")


if __name__ == "__main__":
    asyncio.run(main())
