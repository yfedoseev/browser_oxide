#[cfg(test)]
mod tests {
    use browser::Page;
    use stealth;

    #[tokio::test]
    async fn audit_screen_keys() {
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::navigate("https://example.com/", profile, 1).await.unwrap();
        
        let js = r#"
            JSON.stringify({
                screenKeys: Object.keys(screen),
                screenProtoKeys: Object.getOwnPropertyNames(Screen.prototype)
            }, null, 2);
        "#;
        
        let r = page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {}", e));
        println!("SCREEN OXIDE:\n{}", r);
    }
}
