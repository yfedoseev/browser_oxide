//! Quality-regression guard (deterministic, network-free).
//!
//! Runs in the gating CI test job. These assert the engine invariants that, if
//! they silently broke, would degrade real-world anti-bot quality — without
//! depending on a live site. The live pass-rate signal is the separate
//! `canary` workflow + `BENCHMARK.md`.

use browser_oxide::stealth::presets::chrome_148_macos;
use browser_oxide::{ChallengeVerdict, Page};

async fn page(html: &str) -> Page {
    Page::from_html(html, Some(chrome_148_macos()))
        .await
        .expect("from_html")
}

#[tokio::test]
async fn renders_local_html() {
    let mut p =
        page("<!doctype html><html><body><h1>Hello</h1><p id=\"x\">world</p></body></html>").await;
    assert_eq!(p.text_of("h1").as_deref(), Some("Hello"));
    assert!(p.has_element("#x"));
    assert!(!p.content().is_empty());
}

#[tokio::test]
async fn javascript_executes() {
    let mut p = page("<html><body></body></html>").await;
    assert_eq!(p.evaluate("1 + 1").unwrap().trim(), "2");
}

/// The load-bearing stealth invariants. A regression in any of these is exactly
/// what flips a previously-passing anti-bot site to a block.
#[tokio::test]
async fn stealth_invariants_hold() {
    let mut p = page("<html><body></body></html>").await;

    // navigator.webdriver must never be true — the #1 automation tell.
    assert_ne!(
        p.evaluate("String(navigator.webdriver)").unwrap().trim(),
        "true"
    );
    // User-Agent matches the selected profile (coherence).
    let ua = p.evaluate("navigator.userAgent").unwrap();
    assert!(ua.contains("Chrome/148"), "unexpected UA: {ua}");
    // Real navigators expose these; empty/missing is a tell.
    assert!(
        p.evaluate("navigator.languages.length")
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap_or(0)
            >= 1
    );
    assert_eq!(
        p.evaluate("typeof navigator.plugins").unwrap().trim(),
        "object"
    );
    assert!(
        p.evaluate("navigator.hardwareConcurrency")
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap_or(0)
            >= 1
    );
}

/// Real, substantial content classifies as a pass (the honest gate the
/// benchmark uses). Guards against the classifier regressing real pages to
/// thin/challenge.
#[tokio::test]
async fn substantial_content_is_a_pass() {
    let body = "<p>lorem ipsum dolor sit amet </p>".repeat(2000); // ~66 KB
    let mut p = page(&format!("<!doctype html><html><body>{body}</body></html>")).await;
    assert_eq!(p.challenge_verdict(), ChallengeVerdict::Pass);
}
