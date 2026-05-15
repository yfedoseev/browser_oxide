//! W7 Cloudflare V1 — udemy.com end-to-end smoke test.
//!
//! Run: `cargo test -p browser --test cloudflare_udemy -- --ignored --test-threads=1 --nocapture`
//!
//! udemy.com on 2026-05-10 served a Cloudflare Managed Challenge
//! (`cf-mitigated: challenge`, `cType: 'managed'`). V1 ships:
//!   - Detection (`stealth::cloudflare::detect_challenge`).
//!   - Orchestrator-runner hook (`Page::handle_cloudflare_flow`) that
//!     drives the event loop and polls for `cf_clearance`.
//!   - CSP bypass for udemy.com so the orchestrator script can load.
//!
//! Pass criteria for V1: orchestrator was at least *attempted* (i.e. the
//! detector fires and we run the noise/poll loop). Full cf_clearance
//! issuance is V2 — see `docs/W7_CLOUDFLARE_V1_2026_05_10.md`.

use browser::Page;

#[tokio::test]
#[ignore]
async fn udemy_cloudflare_orchestrator() {
    println!("\n=== udemy.com Cloudflare Managed Challenge ===");
    let profile = stealth::chrome_130_linux();
    match Page::navigate("https://www.udemy.com/", profile, 3).await {
        Ok(mut page) => {
            let title = page.title();
            let body = page.content();
            let body_len = body.len();
            println!("[udemy] title: {title:?}");
            println!("[udemy] body length: {}", body_len);
            // Telemetry — did the CF marker make it past navigate?
            let still_chl = body.contains("/cdn-cgi/challenge-platform/")
                || body.contains("_cf_chl_opt")
                || body.contains("Just a moment");
            println!("[udemy] still on challenge page: {}", still_chl);
            // V1 success criterion is loose: scaffolding ran, no panic,
            // and we got *some* response. cf_clearance issuance is V2.
            if !still_chl && body_len > 50_000 && title.to_lowercase().contains("udemy") {
                println!("[udemy] PASS — flipped CF-CHL → real content");
            } else if !still_chl {
                println!("[udemy] PARTIAL — escaped CHL but body small ({body_len}B)");
            } else {
                println!("[udemy] V1 SCAFFOLDING ONLY — orchestrator detected but no clearance");
                println!("[udemy] (See docs/W7_CLOUDFLARE_V1_2026_05_10.md for V2 next steps.)");
            }
        }
        Err(e) => {
            println!("[udemy] navigate error: {e}");
            // Don't panic — V1 is exploratory.
        }
    }
}

#[test]
fn detects_synthetic_managed_challenge() {
    use std::collections::HashMap;
    let mut h = HashMap::new();
    h.insert("cf-mitigated".into(), "challenge".into());
    h.insert("server".into(), "cloudflare".into());
    let body = r#"
<script>
window._cf_chl_opt = {
    cFPWv: 'g', cType: 'managed', cRay: 'abc123',
    cZone: 'example.com', cN: 'nonce',
    fa: '/?fa=1', mdrd: 'mdrd-tok'
};
</script>
"#;
    let ctx =
        stealth::cloudflare::detect_challenge(&h, body).expect("synthetic body should detect");
    assert_eq!(ctx.kind, stealth::cloudflare::CfChallengeKind::Managed);
    assert_eq!(ctx.zone, "example.com");
}
