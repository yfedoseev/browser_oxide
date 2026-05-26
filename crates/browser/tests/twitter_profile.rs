//! Dedicated profile test for twitter.com / x.com SPA hydration.
//!
//! Run:
//!   BROWSER_OXIDE_EVENT_LOOP_PROFILE=1 \
//!   BROWSER_OXIDE_EVENT_LOOP_PROFILE_LABEL=xcom \
//!   cargo test --release -p browser --test twitter_profile \
//!     -- --ignored --test-threads=1 --nocapture 2>&1 | tee /tmp/xcom.log

use browser::Page;
use std::time::{Duration, Instant};

#[tokio::test]
#[ignore]
async fn profile_twitter() {
    let url =
        std::env::var("BROWSER_OXIDE_TARGET").unwrap_or_else(|_| "https://twitter.com/".into());
    let profile = stealth::presets::chrome_148_macos();
    let t0 = Instant::now();
    let result =
        tokio::time::timeout(Duration::from_secs(180), Page::navigate(&url, profile, 1)).await;
    let nav_ms = t0.elapsed().as_millis();
    match result {
        Ok(Ok(mut page)) => {
            let html = page.content();
            let mounted = page
                .event_loop()
                .execute_script(
                    "(function(){\
                        var sels=['#react-root','#__next','#app','#root','[data-reactroot]','#main-app','#mount-point'];\
                        for (var i=0;i<sels.length;i++){\
                            var el=document.querySelector(sels[i]);\
                            if (el && el.children) return sels[i]+':'+el.children.length;\
                        }\
                        return 'none';\
                    })()",
                )
                .unwrap_or_default();
            eprintln!(
                "PROFILE_RESULT url={} body={} nav_ms={} mounted={}",
                url,
                html.len(),
                nav_ms,
                mounted,
            );
        }
        Ok(Err(e)) => eprintln!("PROFILE_RESULT url={} ERROR: {}", url, e),
        Err(_) => eprintln!("PROFILE_RESULT url={} TIMEOUT after 180s", url),
    }
}
