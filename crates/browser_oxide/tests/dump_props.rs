#[cfg(test)]
mod tests {
    use browser_oxide::Page;

    #[tokio::test]
    async fn dump_window_props() {
        let profile = browser_oxide::stealth::presets::chrome_148_macos();
        let mut page = Page::from_html_with_url(
            "<!DOCTYPE html><html><body></body></html>",
            "https://example.com/",
            Some(profile),
        )
        .await
        .unwrap();

        let r = page
            .evaluate("Object.getOwnPropertyNames(globalThis).sort().join(',')")
            .unwrap();
        println!("OXIDE_PROPS:{}", r);
    }
}
