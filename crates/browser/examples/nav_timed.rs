//! Phase-broken-down navigation timing for browser_oxide.
//!
//! Usage:
//!   cargo run --release -p browser --example nav_timed -- <url> [profile]
//!
//! Emits one JSON line on stdout. Phases:
//!   t_fetch_ms       — HTTP GET (no V8 at all)
//!   t_from_html_ms   — `Page::from_html(empty)` (cold V8 + bootstrap, no real HTML)
//!   t_reload_ms      — `reload_html(real_html)` on the warm isolate (parse + scripts)
//!   t_drain_ms       — `run_until_idle` after reload
//!   t_navigate_ms    — full `Page::navigate` (the production path) for comparison
//!   t_navigate_warm_ms — second `Page::navigate` immediately after (separate Page,
//!                       but shared HTTP connection is warm).
//! Two timings come out: one breaking the navigation into measurable subphases,
//! and one running the real `Page::navigate` end-to-end for an apples-to-apples
//! comparison with competitor engines.

use std::time::Instant;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("usage: nav_timed <url> [profile]");
    let profile_name = args.next().unwrap_or_else(|| "chrome_148_macos".to_string());

    let profile = match profile_name.as_str() {
        "chrome_148_macos" => stealth::presets::chrome_148_macos(),
        "chrome_148_windows" => stealth::presets::chrome_148_windows(),
        "firefox_135_macos" => stealth::presets::firefox_135_macos(),
        "iphone_15_pro_safari_18" => stealth::presets::iphone_15_pro_safari_18(),
        "pixel_9_pro_chrome_148" => stealth::presets::pixel_9_pro_chrome_148(),
        other => panic!("unknown profile {other}"),
    };

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            // Phase 1: HTTP fetch only (no V8 created yet).
            let client = net::HttpClient::shared(&profile).expect("client");
            let t_fetch_a = Instant::now();
            let resp = client.get_follow(&url, 10).await.expect("fetch");
            let t_fetch_ms = t_fetch_a.elapsed().as_millis() as u64;
            let cold_status = resp.status;
            let html = resp.text();
            let cold_body_len = html.len();
            let resp_url = resp.url.clone();
            drop(resp);

            // Phase 2: cold V8 isolate + bootstrap (no real HTML yet).
            let t_v8_a = Instant::now();
            let mut warm_page =
                browser::Page::from_html_fast("<html><head></head><body></body></html>", &resp_url, profile.clone())
                    .await
                    .expect("from_html_fast");
            let t_from_html_ms = t_v8_a.elapsed().as_millis() as u64;

            // Phase 3: reload_html with the real fetched body (warm isolate path).
            let t_reload_a = Instant::now();
            warm_page.reload_html(&html, &resp_url);
            let t_reload_ms = t_reload_a.elapsed().as_millis() as u64;

            // Phase 4: drain the event loop on the warm page (run any inline scripts to idle).
            let t_drain_a = Instant::now();
            let _ = warm_page
                .event_loop()
                .run_until_idle(std::time::Duration::from_secs(8))
                .await;
            let t_drain_ms = t_drain_a.elapsed().as_millis() as u64;

            let warm_path_body_len = warm_page.content().len();
            drop(warm_page);

            // Phase 5: full production Page::navigate, cold (separate fresh isolate).
            let t_nav_a = Instant::now();
            let nav_page_res = browser::Page::navigate(&url, profile.clone(), 3).await;
            let t_navigate_ms = t_nav_a.elapsed().as_millis() as u64;

            let (nav_body_len, nav_tag): (usize, String) = match nav_page_res {
                Ok(mut p) => {
                    let c = p.content();
                    let ec = browser::engine_classify(&c);
                    (ec.len, ec.tag.to_string())
                }
                Err(e) => (0, format!("ERR:{e}")),
            };

            // Phase 6: a second back-to-back navigate (shared HTTP client → warm
            // socket; fresh V8 isolate → cold bootstrap). Distinguishes network
            // warm-up from engine warm-up.
            let t_nav2_a = Instant::now();
            let _ = browser::Page::navigate(&url, profile.clone(), 3).await;
            let t_navigate_warm_ms = t_nav2_a.elapsed().as_millis() as u64;

            // Phase 7: pool-based navigation. Seed the pool with one warm
            // page, then navigate the same URL through the warm isolate.
            // This is the apples-to-apples cost of running browser_oxide
            // the way a high-throughput scraper would.
            let pool = browser::PagePool::new(4);
            let _seed = pool
                .acquire(Some(profile.clone()))
                .await
                .expect("seed pool");
            pool.release(_seed);

            let t_pool_a = Instant::now();
            let mut pool_page = match pool.navigate(&url, profile.clone()).await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("pool.navigate failed: {e}");
                    println!(
                        "{{\"engine\":\"browser_oxide\",\"err\":\"pool.navigate: {}\"}}",
                        e.to_string().replace('"', "'")
                    );
                    return;
                }
            };
            let t_pool_first_ms = t_pool_a.elapsed().as_millis() as u64;
            let pool_first_body = pool_page.content();
            let pool_first_ec = browser::engine_classify(&pool_first_body);
            pool.release(pool_page);

            // Phase 8: second pool navigation — measures TRUE steady-state
            // warm-reuse cost (no isolate creation at all).
            let t_pool2_a = Instant::now();
            let mut pool_page2 = pool
                .navigate(&url, profile.clone())
                .await
                .expect("pool second navigate");
            let t_pool_second_ms = t_pool2_a.elapsed().as_millis() as u64;
            let pool_second_body_len = pool_page2.content().len();
            pool.release(pool_page2);

            println!(
                "{{\"engine\":\"browser_oxide\",\"profile\":\"{profile_name}\",\"url\":\"{url}\",\"t_fetch_ms\":{t_fetch_ms},\"t_from_html_ms\":{t_from_html_ms},\"t_reload_ms\":{t_reload_ms},\"t_drain_ms\":{t_drain_ms},\"t_navigate_ms\":{t_navigate_ms},\"t_navigate_warm_ms\":{t_navigate_warm_ms},\"t_pool_first_ms\":{t_pool_first_ms},\"t_pool_second_ms\":{t_pool_second_ms},\"cold_status\":{cold_status},\"cold_body_len\":{cold_body_len},\"warm_path_body_len\":{warm_path_body_len},\"body_len\":{nav_body_len},\"tag\":\"{nav_tag}\",\"pool_first_body_len\":{},\"pool_first_tag\":\"{}\",\"pool_second_body_len\":{pool_second_body_len}}}",
                pool_first_ec.len,
                pool_first_ec.tag,
            );
        })
        .await;
}
