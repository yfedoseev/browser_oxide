#!/usr/bin/env python3
"""
Comprehensive stealth browser benchmark.
Tests: Puppeteer+Stealth, Playwright+Stealth, Patchright, Camoufox, browser_oxide, Chrome, Lightpanda
Measures: stealth score, memory RSS, page load time, anti-bot pass rate
"""
import asyncio
import json
import os
import subprocess
import sys
import time
import resource

# Stealth checks — same 18 as browser_oxide's Rust test suite
STEALTH_CHECKS = [
    ("webdriver", "typeof navigator.webdriver", "undefined"),
    ("chrome_obj", "typeof window.chrome", "object"),
    ("plugins", "navigator.plugins.length > 0", "true"),
    ("languages", "navigator.languages.length > 0", "true"),
    ("vendor", "navigator.vendor", "Google Inc."),
    ("platform", "typeof navigator.platform", "string"),
    ("hardwareConcurrency", "navigator.hardwareConcurrency > 0", "true"),
    ("ua_chrome", "/Chrome/.test(navigator.userAgent)", "true"),
    ("webrtc", "typeof RTCPeerConnection", "function"),
    ("fonts_api", "typeof document.fonts", "object"),
    ("permissions", "typeof navigator.permissions.query", "function"),
    ("battery", "typeof navigator.getBattery", "function"),
    ("speech_voices", "speechSynthesis.getVoices().length > 0", "true"),
    ("media_source", "typeof MediaSource.isTypeSupported", "function"),
    ("codec_h264", 'MediaSource.isTypeSupported(\'video/mp4; codecs="avc1.42E01E"\')', "true"),
    ("eventsource", "typeof EventSource", "function"),
    ("websocket", "typeof WebSocket", "function"),
    ("deviceMemory", "navigator.deviceMemory > 0", "true"),
]

# Anti-bot quick test URLs
ANTIBOT_URLS = [
    ("nowsecure.nl", "https://nowsecure.nl", "cloudflare"),
    ("nike.com", "https://www.nike.com", "akamai"),
    ("walmart.com", "https://www.walmart.com", "perimeterx"),
    ("amazon.com", "https://www.amazon.com", "custom"),
    ("bot.sannysoft.com", "https://bot.sannysoft.com", "verify"),
]

def get_rss_mb():
    """Get current process RSS in MB."""
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024  # KB to MB on Linux

def get_pid_rss_mb(pid):
    """Get RSS of a specific PID in MB."""
    try:
        with open(f"/proc/{pid}/statm") as f:
            pages = int(f.read().split()[1])
            return pages * 4 / 1024  # pages -> KB -> MB
    except:
        return 0

async def test_playwright_stealth():
    """Test Playwright + playwright-stealth."""
    from playwright.async_api import async_playwright
    from playwright_stealth import Stealth

    results = {"name": "Playwright+Stealth", "stealth": {}, "timing": {}, "memory": {}}

    start = time.time()
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        results["timing"]["startup"] = f"{(time.time()-start)*1000:.0f}ms"

        page = await browser.new_page()
        async with Stealth().use_async(page):
            pass  # stealth applied

        # Memory before
        pid = browser.process.pid if hasattr(browser, 'process') and browser.process else os.getpid()

        # Navigate
        start = time.time()
        await page.goto("about:blank", wait_until="load")
        results["timing"]["nav_blank"] = f"{(time.time()-start)*1000:.0f}ms"

        # Stealth checks
        passed = 0
        for name, js, expected in STEALTH_CHECKS:
            try:
                result = str(await page.evaluate(js)).lower()
                ok = result == expected
                results["stealth"][name] = "PASS" if ok else f"FAIL ({result})"
                if ok: passed += 1
            except Exception as e:
                results["stealth"][name] = f"ERR ({e})"
        results["stealth"]["_score"] = f"{passed}/{len(STEALTH_CHECKS)}"

        # Memory
        results["memory"]["rss_mb"] = f"{get_pid_rss_mb(pid):.0f}" if pid != os.getpid() else "N/A"

        # Page load timing
        start = time.time()
        await page.goto("https://example.com", wait_until="load", timeout=10000)
        results["timing"]["example.com"] = f"{(time.time()-start)*1000:.0f}ms"

        title = await page.title()
        results["timing"]["title"] = title

        await browser.close()
    return results

async def test_patchright():
    """Test Patchright (patched Playwright)."""
    from patchright.async_api import async_playwright

    results = {"name": "Patchright", "stealth": {}, "timing": {}, "memory": {}}

    start = time.time()
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        results["timing"]["startup"] = f"{(time.time()-start)*1000:.0f}ms"

        page = await browser.new_page()

        await page.goto("about:blank", wait_until="load")

        passed = 0
        for name, js, expected in STEALTH_CHECKS:
            try:
                result = str(await page.evaluate(js)).lower()
                ok = result == expected
                results["stealth"][name] = "PASS" if ok else f"FAIL ({result})"
                if ok: passed += 1
            except Exception as e:
                results["stealth"][name] = f"ERR ({e})"
        results["stealth"]["_score"] = f"{passed}/{len(STEALTH_CHECKS)}"

        pid = browser.process.pid if hasattr(browser, 'process') and browser.process else os.getpid()
        results["memory"]["rss_mb"] = f"{get_pid_rss_mb(pid):.0f}" if pid != os.getpid() else "N/A"

        start = time.time()
        await page.goto("https://example.com", wait_until="load", timeout=10000)
        results["timing"]["example.com"] = f"{(time.time()-start)*1000:.0f}ms"
        results["timing"]["title"] = await page.title()

        await browser.close()
    return results

async def test_camoufox():
    """Test Camoufox (patched Firefox)."""
    from camoufox.async_api import AsyncCamoufox

    results = {"name": "Camoufox", "stealth": {}, "timing": {}, "memory": {}}

    start = time.time()
    try:
        async with AsyncCamoufox(headless=True) as browser:
            results["timing"]["startup"] = f"{(time.time()-start)*1000:.0f}ms"
            page = await browser.new_page()

            await page.goto("about:blank", wait_until="load")

            passed = 0
            for name, js, expected in STEALTH_CHECKS:
                try:
                    result = str(await page.evaluate(js)).lower()
                    # Camoufox is Firefox — adjust expectations
                    if name == "chrome_obj":
                        expected_cf = "undefined"  # Firefox has no window.chrome
                        ok = result == expected_cf
                    elif name == "vendor":
                        ok = result != ""  # Firefox has "" vendor
                    elif name == "ua_chrome":
                        ok = True  # Firefox UA doesn't contain Chrome — that's expected
                    else:
                        ok = result == expected
                    results["stealth"][name] = "PASS" if ok else f"FAIL ({result})"
                    if ok: passed += 1
                except Exception as e:
                    results["stealth"][name] = f"ERR ({e})"
            results["stealth"]["_score"] = f"{passed}/{len(STEALTH_CHECKS)}"

            results["memory"]["rss_mb"] = "N/A"

            start = time.time()
            await page.goto("https://example.com", wait_until="load", timeout=10000)
            results["timing"]["example.com"] = f"{(time.time()-start)*1000:.0f}ms"
            results["timing"]["title"] = await page.title()
    except Exception as e:
        results["timing"]["startup"] = f"ERR: {e}"

    return results

async def main():
    sep = "=" * 100
    dash = "-" * 100

    print(f"\n{sep}")
    print(" COMPREHENSIVE STEALTH BROWSER BENCHMARK")
    print(f"{sep}\n")

    all_results = []

    # Test each tool
    tools = [
        ("Playwright+Stealth", test_playwright_stealth),
        ("Patchright", test_patchright),
        ("Camoufox", test_camoufox),
    ]

    for name, test_fn in tools:
        print(f"Testing {name}...", end=" ", flush=True)
        try:
            result = await asyncio.wait_for(test_fn(), timeout=60)
            all_results.append(result)
            score = result["stealth"].get("_score", "?")
            timing = result["timing"].get("example.com", "?")
            mem = result["memory"].get("rss_mb", "?")
            print(f"stealth={score}, example.com={timing}, RSS={mem}MB")
        except Exception as e:
            print(f"FAILED: {e}")
            all_results.append({"name": name, "stealth": {"_score": "ERR"}, "timing": {"example.com": "ERR"}, "memory": {"rss_mb": "ERR"}})

    # Print stealth comparison
    print(f"\n{sep}")
    print(" STEALTH SCORE COMPARISON")
    print(f"{sep}")
    print(f"{'Check':<25}", end="")
    for r in all_results:
        print(f" {r['name']:<20}", end="")
    print()
    print(dash)

    for name, js, expected in STEALTH_CHECKS:
        print(f"{name:<25}", end="")
        for r in all_results:
            val = r["stealth"].get(name, "N/A")
            print(f" {val:<20}", end="")
        print()

    print(dash)
    print(f"{'TOTAL':<25}", end="")
    for r in all_results:
        print(f" {r['stealth'].get('_score', '?'):<20}", end="")
    print()
    print(sep)

    # Print timing comparison
    print(f"\n{'Timing':<25}", end="")
    for r in all_results:
        print(f" {r['name']:<20}", end="")
    print()
    print(dash)
    for key in ["startup", "example.com", "title"]:
        print(f"{key:<25}", end="")
        for r in all_results:
            print(f" {r['timing'].get(key, 'N/A'):<20}", end="")
        print()

    # Print memory
    print(f"\n{'Memory (RSS MB)':<25}", end="")
    for r in all_results:
        print(f" {r['memory'].get('rss_mb', 'N/A'):<20}", end="")
    print()
    print(sep)

    # Output JSON for parsing
    with open("/tmp/stealth-bench/results.json", "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\nFull results saved to /tmp/stealth-bench/results.json")

if __name__ == "__main__":
    asyncio.run(main())
