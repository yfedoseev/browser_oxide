#[cfg(test)]
mod tests {
    use browser_oxide::Page;

    #[tokio::test]
    #[ignore = "network: live HTTP against example.com"]
    async fn audit_screen_keys() {
        let profile = browser_oxide::stealth::presets::chrome_148_macos();
        let mut page = Page::navigate("https://example.com/", profile, 1)
            .await
            .unwrap();

        let js = r#"
            JSON.stringify({
                screenKeys: Object.keys(screen),
                screenProtoKeys: Object.getOwnPropertyNames(Screen.prototype)
            }, null, 2);
        "#;

        let r = page
            .evaluate(js)
            .unwrap_or_else(|e| format!("ERROR: {}", e));
        println!("SCREEN OXIDE:\n{}", r);
    }
}
