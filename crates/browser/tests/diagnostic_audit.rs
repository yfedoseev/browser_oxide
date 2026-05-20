#[cfg(test)]
mod tests {
    use browser::Page;
    use stealth;

    #[tokio::test]
    #[ignore = "network: live HTTP against sinceyouarrived.world"]
    async fn audit_sinceyouarrived_oxide() {
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::navigate("https://sinceyouarrived.world/taken", profile, 5)
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;

        let r = page.evaluate("document.body.innerText").unwrap();
        println!("SINCEYOUARRIVED OXIDE: {}", r);
    }
}
