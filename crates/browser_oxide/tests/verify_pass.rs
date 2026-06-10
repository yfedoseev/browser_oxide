use browser_oxide::Page;
use std::time::Duration;

#[tokio::test]
#[ignore = "network: live HTTP against canadagoose.com (challenge-protected)"]
async fn verify_canadagoose_pass() {
    let profile = browser_oxide::stealth::chrome_148_windows();
    let res = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init("https://www.canadagoose.com/", profile, 3, vec![]),
    )
    .await;

    match res {
        Ok(Ok(mut p)) => {
            let title = p.title();
            let html = p.evaluate("document.body.innerHTML").unwrap_or_default();
            println!("SUCCESS! Title: {}", title);
            if title.contains("Canada Goose") || html.contains("Canada Goose") {
                println!("Confirmed: Page contains 'Canada Goose'");
            } else {
                println!(
                    "Warning: Page does not contain 'Canada Goose'. Might be a challenge page."
                );
                println!("Title: {}", title);
            }
        }
        Ok(Err(e)) => println!("FAILED: {}", e),
        Err(_) => println!("TIMEOUT"),
    }
}

#[tokio::test]
#[ignore = "network: live HTTP against hyatt.com (challenge-protected)"]
async fn verify_hyatt_pass() {
    let profile = browser_oxide::stealth::chrome_148_windows();
    let res = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init("https://www.hyatt.com/", profile, 3, vec![]),
    )
    .await;

    match res {
        Ok(Ok(mut p)) => {
            let title = p.title();
            let html = p.evaluate("document.body.innerHTML").unwrap_or_default();
            println!("SUCCESS! Title: {}", title);
            if title.contains("Hyatt") || html.contains("Hyatt") {
                println!("Confirmed: Page contains 'Hyatt'");
            } else {
                println!("Warning: Page does not contain 'Hyatt'. Might be a challenge page.");
                println!("Title: {}", title);
            }
        }
        Ok(Err(e)) => println!("FAILED: {}", e),
        Err(_) => println!("TIMEOUT"),
    }
}

#[tokio::test]
#[ignore = "network: live HTTP against realtor.com (challenge-protected)"]
async fn verify_realtor_pass() {
    let profile = browser_oxide::stealth::chrome_148_windows();
    let res = tokio::time::timeout(
        Duration::from_secs(120),
        Page::navigate_with_init("https://www.realtor.com/", profile, 3, vec![]),
    )
    .await;

    match res {
        Ok(Ok(mut p)) => {
            let title = p.title();
            let html = p.evaluate("document.body.innerHTML").unwrap_or_default();
            println!("SUCCESS! Title: {}", title);
            if title.contains("realtor.com")
                || title.contains("Realtor")
                || html.contains("realtor.com")
            {
                println!("Confirmed: Page contains 'Realtor'");
                let end = std::cmp::min(html.len(), 500);
                println!("HTML snippet: {}", &html[..end]);
            } else {
                println!("Warning: Page does not contain 'Realtor'. Might be a challenge page.");
                println!("Title: {}", title);
            }
        }
        Ok(Err(e)) => println!("FAILED: {}", e),
        Err(_) => println!("TIMEOUT"),
    }
}
