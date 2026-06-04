# browser-oxide (Python)

**A stealth headless browser engine, in Rust, with Python bindings.** A real
HTML/CSS/DOM/JavaScript browser built from scratch — own BoringSSL TLS stack,
native fingerprint, V8 JavaScript — with **no Chromium, no CDP driver, no
Selenium** underneath. Built for scraping, archival, and AI-agent web access
against modern anti-bot stacks (Cloudflare, Akamai, AWS WAF, DataDome).

```bash
pip install browser-oxide
```

```python
from browser_oxide import Browser, Profile, Verdict

with Browser(profile=Profile.chrome()) as b:
    page = b.navigate("https://example.com")
    print(page.title, len(page.html), page.verdict)
    if page.verdict == Verdict.PASS:           # challenge pages return HTTP 200 —
        print(page.evaluate("navigator.userAgent"))   # trust the verdict, not status
        print(page.query_text("h1"))
```

## API

| | |
|---|---|
| `Profile.chrome() / .firefox() / .iphone() / .pixel()` | built-in identities (TLS + headers + navigator + GPU, coherent) |
| `Profile.from_file(path)` | custom profile (YAML/JSON) |
| `Browser(profile=None)` | spawns the engine; context-manager friendly |
| `b.navigate(url, max_iterations=5) -> Page` | cold navigation (GIL released) |
| `page.url / .title / .html / .text / .verdict / .is_challenge` | properties |
| `page.evaluate(js) -> str` · `page.query_text(sel) -> str \| None` | scripting |
| `Verdict.PASS / THIN_SHELL / EDGE_BLOCK / SENSOR_FAIL / …` | honest render outcome |

## What's real vs. not

Real: from-scratch TLS (JA3/JA4-accurate), full V8, real DOM/CSS, Canvas 2D
raster, 100+ coherent fingerprint properties. **Not** implemented: real WebGL
raster, HTTP/3 by default, built-in per-vendor bypass (Kasada is the open gap).
Full detail + measured pass rates: see the
[repo docs](https://github.com/yfedoseev/browser_oxide/tree/main/docs)
(`getting-started-python.md`, `guides/STEALTH_FAQ.md`, `BENCHMARK.md`).

MIT OR Apache-2.0.
