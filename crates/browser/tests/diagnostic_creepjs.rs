#[cfg(test)]
mod tests {
    use browser::Page;

    #[tokio::test]
    #[ignore = "network: live HTTP against abrahamjuliot.github.io/creepjs"]
    async fn test_creepjs_oxide() {
        let profile = stealth::presets::chrome_148_macos();
        let mut page = Page::navigate("https://abrahamjuliot.github.io/creepjs/", profile, 5)
            .await
            .unwrap();

        // Wait for CreepJS to compute
        for _ in 0..10 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let ready = page
                .evaluate("!!document.querySelector('.fingerprint-header-container')")
                .unwrap_or_default();
            if ready == "true" {
                break;
            }
        }

        let js = r#"
            (() => {
                const lies = [...document.querySelectorAll('.lies')].map(el => el.innerText);
                const status = document.querySelector('.fingerprint-header-container')?.innerText;
                const prediction = document.querySelector('.fuzzy-signature')?.innerText;
                return JSON.stringify({ status, prediction, lies }, null, 2);
            })()
        "#;

        let r = page
            .evaluate(js)
            .unwrap_or_else(|e| format!("ERROR: {}", e));
        println!("CREEPJS OXIDE:\n{}", r);

        let inner_text = page
            .evaluate("document.body.innerText.substring(0, 1000)")
            .unwrap_or_default();
        println!("CREEPJS TEXT: {}", inner_text);
    }
}
