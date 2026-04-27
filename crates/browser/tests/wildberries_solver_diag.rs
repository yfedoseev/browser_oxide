//! Focused diagnostic for the wildberries WBAAS solver flow.
//!
//! Runs the full `navigate_with_challenges` path against wildberries.ru and
//! captures: (1) the cookies in the jar before the initial GET, (2) after the
//! challenge GET, (3) after the solver runs, (4) at the Rust-level retry, and
//! (5) the exact headers the retry sent.
//!
//! Run: cargo test -p browser --test wildberries_solver_diag -- \
//!          --ignored --test-threads=1 --nocapture

#[tokio::test]
#[ignore]
async fn wildberries_full_solver_trace() {
    use net::HttpClient;
    use url::Url;

    let profile = stealth::presets::chrome_130_ru();
    let client = HttpClient::new(&profile).unwrap();
    let url = "https://www.wildberries.ru/";
    let parsed = Url::parse(url).unwrap();

    println!("\n=== STAGE 1: initial GET ===");
    let resp1 = client.get(url).await.unwrap();
    println!("status={} body={}b", resp1.status, resp1.body.len());
    println!(
        "x-wbaas-token header: {:?}",
        resp1.headers.get("x-wbaas-token")
    );
    println!(
        "status-no-id header: {:?}",
        resp1.headers.get("status-no-id")
    );
    println!("set-cookies: {}", resp1.set_cookies.len());
    for c in &resp1.set_cookies {
        let trim: String = c.chars().take(120).collect();
        println!("  set-cookie: {trim}");
    }
    let cookies1 = client.cookies_for_url(&parsed).await.unwrap_or_default();
    println!("jar cookies ({}):", cookies1.len());
    for kv in cookies1.split("; ") {
        println!("  {kv}");
    }

    println!("\n=== STAGE 2: run the solver via Page::navigate_with_challenges ===");
    let page_result = browser::Page::navigate(url, stealth::presets::chrome_130_ru(), 1).await;
    match page_result {
        Ok(mut page) => {
            let body = page.content();
            println!("page OK — body {}b", body.len());
            let preview: String = body.chars().take(300).collect();
            println!("preview: {preview}");
        }
        Err(e) => println!("page ERR: {e}"),
    }

    println!("\n=== STAGE 3: post-solver jar state ===");
    // We need a fresh client since Page consumed it; build one with the same
    // cookie jar would be ideal but our public API doesn't expose that. For
    // now, read the "set_fetch_client" static that Page installs.
    let client2 = HttpClient::new(&profile).unwrap();
    let cookies3 = client2.cookies_for_url(&parsed).await.unwrap_or_default();
    println!("fresh-client jar (should be empty): {}", cookies3.len());
    // The real check: can we hit the page directly with the cookies still
    // in the static fetch client that Page left behind? We can't access that
    // from here without a new API — but we can note that the jar is per-
    // HttpClient-instance and doesn't persist across navigations.

    println!("\n=== STAGE 4: reach into the static fetch client ===");
    // Page::navigate_with_challenges calls set_fetch_client on the static
    // OnceLock. If we create a new HttpClient and call get, it uses its own
    // jar, not the one Page's client used. Verify this is indeed the case.
    let resp2 = client2.get(url).await.unwrap();
    println!(
        "2nd GET from fresh client: status={} body={}b",
        resp2.status,
        resp2.body.len()
    );
    println!("jar after 2nd GET:");
    let cookies4 = client2.cookies_for_url(&parsed).await.unwrap_or_default();
    for kv in cookies4.split("; ").take(12) {
        println!("  {kv}");
    }
}
