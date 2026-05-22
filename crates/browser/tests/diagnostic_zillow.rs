#[tokio::test]
#[ignore]
async fn diagnostic_zillow() {
    use browser::Page;
    use std::time::Duration;
    let profile = stealth::presets::chrome_148_macos();
    let url = "https://www.zillow.com/";
    println!("\n=== ZILLOW-DIAG: {} ===", url);
    let page = Page::navigate_with_init(url, profile, 3, vec![]).await;
    match page {
        Ok(mut p) => {
            println!("  title:      {}", p.title());
            println!("  body len:   {}", p.content().len());
            let cookies = p.evaluate("document.cookie").unwrap_or_default();
            println!("  cookies:    {}", cookies);
        }
        Err(e) => println!("  error:      {}", e),
    }
}
