import asyncio, sys, time
import nodriver as uc

async def main():
    url = sys.argv[1] if len(sys.argv) > 1 else 'https://www.canadagoose.com/'
    print(f"[nodriver] target: {url}", flush=True)
    t0 = time.time()
    browser = await uc.start(headless=True, no_sandbox=True)
    page = await browser.get(url)
    await asyncio.sleep(20)
    title = await page.evaluate("document.title")
    content = await page.evaluate("document.documentElement.outerHTML")
    body = await page.evaluate("document.body ? document.body.innerText.substring(0, 200) : ''")
    current_url = await page.evaluate("location.href")
    has_ips = '/ips.js' in content or '/149e9513-' in content
    elapsed = time.time() - t0
    print(f"[nodriver] elapsed: {elapsed*1000:.0f}ms")
    print(f"[nodriver] final_url: {current_url}")
    print(f"[nodriver] title: {title}")
    print(f"[nodriver] content_len: {len(content)}")
    print(f"[nodriver] has_ips.js_marker: {has_ips}")
    print(f"[nodriver] looks_like_real_homepage: {not has_ips and len(content) > 50000}")
    print(f"[nodriver] body: {body!r}")
    browser.stop()

asyncio.run(main())
