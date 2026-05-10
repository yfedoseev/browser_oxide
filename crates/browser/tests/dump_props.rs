#[cfg(test)]
mod tests {
    use browser::Page;
    use stealth;

    #[tokio::test]
    async fn dump_window_props() {
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::from_html_with_url(
            "<!DOCTYPE html><html><body></body></html>",
            "https://example.com/",
            Some(profile),
        )
        .await
        .unwrap();
        
        let r = page.evaluate("Object.getOwnPropertyNames(globalThis).sort().join(',')").unwrap();
        println!("OXIDE_PROPS:{}", r);
    }
}
