#[cfg(test)]
mod tests {
    use browser::Page;

    #[tokio::test]
    async fn test_screen_properties_oxide() {
        let profile = stealth::presets::chrome_148_macos();
        let mut page = Page::from_html_with_url(
            "<!DOCTYPE html><html><body></body></html>",
            "https://example.com",
            Some(profile),
        )
        .await
        .unwrap();

        let r = page
            .evaluate(
                r#"
            JSON.stringify({
                width: screen.width,
                height: screen.height,
                availWidth: screen.availWidth,
                colorDepth: screen.colorDepth,
                pixelDepth: screen.pixelDepth,
                type: typeof screen,
                proto: Object.getPrototypeOf(screen).constructor.name,
                ownKeys: Object.getOwnPropertyNames(screen)
            }, null, 2)
        "#,
            )
            .unwrap();
        println!("SCREEN OXIDE:\n{}", r);
    }
}
