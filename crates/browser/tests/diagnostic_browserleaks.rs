#[cfg(test)]
mod tests {
    use browser::Page;
    use stealth;

    #[tokio::test]
    async fn audit_browserleaks_oxide() {
        let profile = stealth::presets::chrome_130_linux();
        let mut page = Page::navigate("https://browserleaks.com/javascript", profile, 5).await.unwrap();
        
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        
        let js = r#"
            (() => {
                const results = {};
                document.querySelectorAll('table tr').forEach(tr => {
                    const cells = tr.querySelectorAll('th, td');
                    if (cells.length >= 2) {
                        const k = cells[0].innerText || cells[0].textContent;
                        const v = cells[1].innerText || cells[1].textContent;
                        if (k && v) {
                            results[k.trim()] = v.trim();
                        }
                    }
                });
                return JSON.stringify(results, null, 2);
            })()
        "#;
        
        let r = page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {}", e));
        println!("BROWSERLEAKS OXIDE:\n{}", r);
    }
}
