use browser_oxide::stealth::presets::chrome_148_ru;
use browser_oxide::Page;

#[tokio::test]
async fn test_local_storage_persistence_across_navigation() {
    let profile = chrome_148_ru();

    // Iteration 0: Set a value in localStorage and trigger a reload
    let html_0 = r#"
        <html>
        <body>
            <script>
                localStorage.setItem('gate_passed', 'true');
                localStorage.persistent_token = 'xyz-123';
                console.log('Storage set, reloading...');
                location.reload();
            </script>
        </body>
        </html>
    "#;

    // Use Page::navigate-like logic manually to verify persistence
    // (Navigate with 2 iterations)
    let mut page = Page::navigate_with_html(html_0, "https://example.com/", profile, 2)
        .await
        .unwrap();

    // If persistence works, the second iteration (which we can simulate by
    // checking what's in the final page's storage) should have the values.

    let result = page
        .evaluate("localStorage.getItem('gate_passed') + ':' + localStorage.persistent_token")
        .unwrap();
    assert_eq!(result, "true:xyz-123");
}

#[tokio::test]
async fn test_session_storage_persistence_across_navigation() {
    let profile = chrome_148_ru();

    let html_0 = r#"
        <html>
        <body>
            <script>
                sessionStorage.setItem('session_id', 'sess-999');
                location.reload();
            </script>
        </body>
        </html>
    "#;

    let mut page = Page::navigate_with_html(html_0, "https://example.com/", profile, 2)
        .await
        .unwrap();

    let result = page
        .evaluate("sessionStorage.getItem('session_id')")
        .unwrap();
    assert_eq!(result, "sess-999");
}
