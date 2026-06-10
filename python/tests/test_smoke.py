"""Offline smoke tests for the browser_oxide Python bindings.

Network-free: verifies the binding loads and the API surface is wired correctly.
Run with: pytest python/tests/  (after `maturin develop`).
"""

import browser_oxide
from browser_oxide import Browser, Page, Profile, Verdict


def test_module_surface():
    assert browser_oxide.__version__
    assert {Browser, Page, Profile, Verdict} <= set(
        getattr(browser_oxide, n) for n in browser_oxide.__all__
    )


def test_profiles_build():
    for ctor in (Profile.chrome, Profile.firefox, Profile.iphone, Profile.pixel):
        p = ctor()
        assert "Profile(" in repr(p)


def test_verdict_enum():
    assert Verdict.PASS == "pass"
    assert Verdict.EDGE_BLOCK.is_challenge is True
    assert Verdict.PASS.is_challenge is False


def test_browser_constructs_and_closes():
    b = Browser(profile=Profile.chrome())
    b.close()  # idempotent; engine thread also stops on GC


def test_context_manager():
    with Browser() as b:  # default profile
        assert b is not None
