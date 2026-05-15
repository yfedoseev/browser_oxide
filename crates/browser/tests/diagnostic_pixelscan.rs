#[cfg(test)]
mod tests {
    use browser::Page;
    use stealth;

    #[tokio::test]
    async fn test_pixelscan_oxide() {
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::navigate("https://pixelscan.net/", profile, 5)
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        // Wait for it to click the scan button if we have to, or maybe Pixelscan just loads it directly if we do an API request?
        // Wait, Pixelscan has a "Scan My Browser Now" button on the homepage, maybe we should navigate directly to /fingerprint-check
        // Let's try /fingerprint-check directly.
    }

    #[tokio::test]
    async fn test_pixelscan_check_oxide() {
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::navigate("https://pixelscan.net/fingerprint-check", profile, 5)
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        let html = page.content();
        println!("Pixelscan size: {}", html.len());

        let r = page
            .evaluate(
                r#"
            (() => {
                const main = document.querySelector('main');
                return main ? main.innerText.substring(0, 1000) : "No main";
            })()
        "#,
            )
            .unwrap_or_default();

        println!("PIXELSCAN OXIDE:\n{}", r);
    }
}
