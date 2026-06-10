"""Minimal browser_oxide Python example.

    maturin develop --release        # build + install into your venv
    python python/examples/hello.py [url]
"""

import sys

from browser_oxide import Browser, Profile, Verdict

url = sys.argv[1] if len(sys.argv) > 1 else "https://example.com"

with Browser(profile=Profile.chrome()) as b:
    page = b.navigate(url)
    print(f"url     : {page.url}")
    print(f"title   : {page.title}")
    print(f"bytes   : {len(page.html)}")
    print(f"verdict : {page.verdict}")
    print(f"h1      : {page.query_text('h1')}")
    print(f"ua      : {page.evaluate('navigator.userAgent')}")

    if page.verdict == Verdict.PASS:
        print("=> real content rendered")
    elif page.is_challenge:
        print("=> blocked by an anti-bot challenge")
    else:
        print(f"=> {page.verdict} (rendered, but a thin/SPA shell)")
