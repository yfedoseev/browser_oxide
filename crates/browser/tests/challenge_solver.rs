//! Tests for JS challenge solving (AWS WAF, DataDome).
//! Run: cargo test -p browser --test challenge_solver -- --ignored --test-threads=1 --nocapture

use browser::Page;

#[tokio::test]
#[ignore]
async fn amazon_aws_waf_challenge() {
    println!("\n=== Amazon AWS WAF Challenge ===");
    let profile = stealth::chrome_130_windows();
    match Page::navigate("https://www.amazon.com", profile, 2).await {
        Ok(mut page) => {
            let title = page.title();
            let text = page.text_content();
            println!("[amazon] title: {title:?}");
            println!("[amazon] body length: {}", text.len());
            println!("[amazon] preview: {}", &text[..text.len().min(200)]);
            if title.contains("Amazon") || text.len() > 10000 {
                println!("[amazon] PASSED challenge!");
            } else {
                println!("[amazon] Still on challenge page");
            }
        }
        Err(e) => println!("[amazon] Error: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn tripadvisor_datadome_challenge() {
    println!("\n=== TripAdvisor DataDome Challenge ===");
    let profile = stealth::chrome_130_windows();
    match Page::navigate("https://www.tripadvisor.com", profile, 2).await {
        Ok(mut page) => {
            let title = page.title();
            let text = page.text_content();
            println!("[tripadvisor] title: {title:?}");
            println!("[tripadvisor] body length: {}", text.len());
            if title.contains("Tripadvisor") || text.len() > 10000 {
                println!("[tripadvisor] PASSED challenge!");
            } else {
                println!("[tripadvisor] Still on challenge page");
            }
        }
        Err(e) => println!("[tripadvisor] Error: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn nowsecure_with_challenges() {
    println!("\n=== nowsecure.nl (should pass without challenges) ===");
    let profile = stealth::chrome_130_linux();
    match Page::navigate("https://nowsecure.nl", profile, 1).await {
        Ok(mut page) => {
            let title = page.title();
            let content = page.content();
            println!("[nowsecure] title: {title:?}");
            println!("[nowsecure] content length: {}", content.len());
            assert!(content.len() > 1000, "should get real page content");
        }
        Err(e) => println!("[nowsecure] Error: {e}"),
    }
}

#[tokio::test]
#[ignore]
async fn hacker_news_with_external_scripts() {
    println!("\n=== Hacker News (has external scripts) ===");
    let profile = stealth::chrome_130_linux();
    match Page::navigate("https://news.ycombinator.com", profile, 0).await {
        Ok(mut page) => {
            let title = page.title();
            println!("[HN] title: {title:?}");
            assert_eq!(title, "Hacker News");
        }
        Err(e) => panic!("[HN] Error: {e}"),
    }
}
