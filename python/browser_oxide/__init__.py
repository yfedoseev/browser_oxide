"""browser_oxide — a stealth headless browser engine (Rust), Python bindings.

A real HTML/CSS/DOM/JS browser built from scratch with its own BoringSSL TLS
stack and a native fingerprint — no Chromium, no CDP driver.

    from browser_oxide import Browser, Profile, Verdict

    with Browser(profile=Profile.chrome()) as b:
        page = b.navigate("https://example.com")
        print(page.title, len(page.html), page.verdict)
        if page.verdict == Verdict.PASS:
            print(page.evaluate("navigator.userAgent"))
"""

from enum import Enum

from ._native import Browser, Page, Profile

__all__ = ["Browser", "Page", "Profile", "Verdict"]
__version__ = "0.1.0"


class Verdict(str, Enum):
    """Honest render outcome. `Page.verdict` (a str) compares equal to these."""

    PASS = "pass"
    THIN_SHELL = "thin-shell"
    RENDER_INCOMPLETE = "render-incomplete"
    EDGE_BLOCK = "edge-block"
    SENSOR_FAIL = "sensor-fail"
    CHALLENGE_INCOMPLETE = "challenge-incomplete"

    @property
    def is_challenge(self) -> bool:
        return self in (
            Verdict.EDGE_BLOCK,
            Verdict.SENSOR_FAIL,
            Verdict.CHALLENGE_INCOMPLETE,
        )
