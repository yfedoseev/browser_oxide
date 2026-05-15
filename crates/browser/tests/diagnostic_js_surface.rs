#[cfg(test)]
mod tests {
    use browser::Page;
    use stealth;

    #[tokio::test]
    async fn audit_js_surface() {
        let profile = stealth::presets::chrome_130_macos();
        let mut page = Page::navigate("https://example.com/", profile, 1)
            .await
            .unwrap();

        let js = r#"
            (async () => {
                const res = {
                    screenWidth: typeof screen !== 'undefined' ? screen.width : 'no screen',
                    screenHeight: typeof screen !== 'undefined' ? screen.height : 'no screen',
                    availWidth: typeof screen !== 'undefined' ? screen.availWidth : 'no screen',
                    colorDepth: typeof screen !== 'undefined' ? screen.colorDepth : 'no screen',
                    plugins: navigator.plugins ? navigator.plugins.length : 'no plugins'
                };
                if (navigator.userAgentData && navigator.userAgentData.getHighEntropyValues) {
                    try {
                        const vals = await navigator.userAgentData.getHighEntropyValues(["formFactors"]);
                        res.formFactors = vals.formFactors;
                    } catch(e) {
                        res.formFactors = "err: " + e;
                    }
                } else {
                    res.formFactors = 'no uad';
                }
                globalThis.__surface_res = JSON.stringify(res, null, 2);
            })();
        "#;
        page.evaluate(js).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let r = page.evaluate("globalThis.__surface_res || 'wait'").unwrap();
        println!("JS SURFACE OXIDE:\n{}", r);
    }
}
