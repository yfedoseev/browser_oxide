# Getting started (Python)

Python bindings for the BrowserOxide stealth headless browser engine — a real
HTML/CSS/DOM/JS browser with its own BoringSSL TLS stack and a native
fingerprint. No Chromium, no CDP driver, no Selenium.

## Install

```bash
pip install browser-oxide          # once published to PyPI
```

From source (this repo) with [maturin](https://www.maturin.rs/):

```bash
python -m venv .venv && source .venv/bin/activate
pip install maturin
maturin develop --release          # builds the Rust engine + installs the wheel
```

> First build downloads a ~130 MB prebuilt V8. It's slow once, then cached.

## Hello, stealth browser

```python
from browser_oxide import Browser, Profile, Verdict

with Browser(profile=Profile.chrome()) as b:
    page = b.navigate("https://example.com")
    print(page.title)          # "Example Domain"
    print(len(page.html))      # rendered outerHTML length
    print(page.verdict)        # "pass" / "thin-shell" / "edge-block" / ...

    if page.verdict == Verdict.PASS:
        print(page.evaluate("navigator.userAgent"))
        print(page.query_text("h1"))
```

## API

### `Profile` — the browser identity
```python
Profile.chrome()     # Chrome 148 / macOS (default)
Profile.firefox()    # Firefox 135 / macOS (real NSS TLS)
Profile.iphone()     # Safari 18 / iPhone 15 Pro
Profile.pixel()      # Chrome 148 / Pixel 9 Pro
Profile.from_file("my_profile.yaml")   # custom (YAML or JSON)
```
A profile sets TLS fingerprint, headers, `navigator`, GPU, canvas/audio seeds —
coherently. See [guides/PROFILES.md](guides/PROFILES.md).

### `Browser`
```python
Browser(profile=None)              # spawns the engine thread (defaults to chrome)
b.navigate(url, max_iterations=5)  # -> Page  (releases the GIL while it works)
b.evaluate(js)                     # -> str   (against the current page)
b.query_text(selector)            # -> str | None
b.close()                          # also happens on __exit__/GC
```
Use it as a context manager (`with Browser() as b:`) so the engine thread is
cleaned up deterministically.

### `Page` (properties + methods)
```python
page.url            # str        page.title    # str
page.html           # str        page.text     # str
page.verdict        # str (compares to Verdict)   page.is_challenge  # bool
page.evaluate(js)            # -> str
page.query_text(selector)   # -> str | None
```

### `Verdict`
```python
Verdict.PASS, Verdict.THIN_SHELL, Verdict.RENDER_INCOMPLETE,
Verdict.EDGE_BLOCK, Verdict.SENSOR_FAIL, Verdict.CHALLENGE_INCOMPLETE
Verdict.EDGE_BLOCK.is_challenge   # True
```
Challenge pages return HTTP 200, so check the **verdict**, not a status code.

## Notes & limits

- The engine is single-threaded (per-thread V8); one `Browser` serializes its
  calls. For parallelism, use multiple `Browser` instances (each owns a thread),
  or a process pool.
- `evaluate`/`query_text` run against the **most recent** `navigate`.
- Same honest boundary as the Rust API — see [guides/STEALTH_FAQ.md](guides/STEALTH_FAQ.md)
  (no real WebGL raster, no HTTP/3 by default, no built-in vendor bypass; Kasada
  is the open gap). Measured pass rates: [BENCHMARK.md](BENCHMARK.md).

## A small scraper

```python
from browser_oxide import Browser, Profile, Verdict

URLS = ["https://example.com", "https://news.ycombinator.com"]

with Browser(profile=Profile.chrome()) as b:
    for url in URLS:
        page = b.navigate(url)
        status = "ok" if page.verdict == Verdict.PASS else page.verdict
        print(f"{status:18} {len(page.html):>8}  {page.title}")
```
