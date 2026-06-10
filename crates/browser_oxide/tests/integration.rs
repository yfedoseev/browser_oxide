//! End-to-end integration tests crossing crate boundaries.

use browser_oxide::Page;

#[tokio::test]
async fn html_parse_js_execute_extract() {
    let mut page = Page::from_html(
        r#"
        <html><head><title>Integration</title></head><body>
            <div id="app"></div>
            <script>
                document.getElementById('app').innerHTML = '<h1>Dynamic Content</h1>';
            </script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(page.title(), "Integration");
    assert_eq!(page.text_of("h1"), Some("Dynamic Content".to_string()));
}

#[tokio::test]
async fn multiple_scripts_execute_in_order() {
    let mut page = Page::from_html(
        r#"
        <html><head></head><body>
            <div id="log"></div>
            <script>document.getElementById('log').textContent = 'A';</script>
            <script>document.getElementById('log').textContent += 'B';</script>
            <script>document.getElementById('log').textContent += 'C';</script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(page.text_of("#log"), Some("ABC".to_string()));
}

#[tokio::test]
async fn create_element_and_query() {
    let mut page = Page::from_html(
        r#"
        <html><head></head><body>
            <script>
                for (let i = 0; i < 5; i++) {
                    const p = document.createElement('p');
                    p.className = 'item';
                    p.textContent = 'Item ' + i;
                    document.body.appendChild(p);
                }
            </script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let count = page
        .evaluate("document.querySelectorAll('.item').length")
        .unwrap();
    assert_eq!(count, "5");
}

#[tokio::test]
async fn inner_html_set_and_query() {
    let mut page = Page::from_html(
        r#"
        <html><head></head><body>
            <div id="container"></div>
            <script>
                document.getElementById('container').innerHTML =
                    '<ul><li>One</li><li>Two</li><li>Three</li></ul>';
            </script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let count = page
        .evaluate("document.querySelectorAll('li').length")
        .unwrap();
    assert_eq!(count, "3");
}

#[tokio::test]
async fn set_timeout_fires_and_mutates_dom() {
    let mut page = Page::from_html(
        r#"
        <html><head></head><body>
            <div id="result">waiting</div>
            <script>
                setTimeout(() => {
                    document.getElementById('result').textContent = 'done';
                }, 50);
            </script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(page.text_of("#result"), Some("done".to_string()));
}

#[tokio::test]
async fn promise_chain() {
    let mut page = Page::from_html(
        r#"
        <html><head></head><body>
            <div id="out"></div>
            <script>
                Promise.resolve('step1')
                    .then(v => v + '-step2')
                    .then(v => { document.getElementById('out').textContent = v; });
            </script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(page.text_of("#out"), Some("step1-step2".to_string()));
}

#[tokio::test]
async fn class_list_manipulation() {
    let mut page = Page::from_html(
        r#"
        <html><head></head><body>
            <div id="el" class="initial"></div>
            <script>
                const el = document.getElementById('el');
                el.classList.add('added');
                el.classList.remove('initial');
            </script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let classes = page
        .evaluate("document.getElementById('el').className")
        .unwrap();
    assert!(classes.contains("added"), "classes: {}", classes);
    assert!(!classes.contains("initial"), "classes: {}", classes);
}

#[tokio::test]
async fn attribute_set_and_get() {
    let mut page = Page::from_html(
        r#"
        <html><head></head><body>
            <div id="el"></div>
            <script>
                document.getElementById('el').setAttribute('data-value', '42');
            </script>
        </body></html>
    "#,
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let val = page
        .evaluate("document.getElementById('el').getAttribute('data-value')")
        .unwrap();
    assert_eq!(val, "42");
}

#[tokio::test]
async fn cdp_session_runtime_evaluate() {
    use browser_oxide::protocol::{CdpRequest, CdpSession};

    let mut session = CdpSession::new();
    let mut page = Page::from_html(
        "<html><head></head><body></body></html>",
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let req = CdpRequest {
        id: 1,
        method: "Runtime.evaluate".into(),
        params: serde_json::json!({"expression": "Math.pow(2, 10)"}),
    };
    let (resp, _events) = session.handle_request(&mut page, &req, None).await;
    assert!(resp.contains("1024"), "response: {}", resp);
}

#[tokio::test]
async fn cdp_session_page_navigate_events() {
    use browser_oxide::protocol::{CdpRequest, CdpSession};

    let mut session = CdpSession::new();
    session.enable_domain("Page");
    let mut page = Page::from_html(
        "<html><head></head><body></body></html>",
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let req = CdpRequest {
        id: 2,
        method: "Page.navigate".into(),
        params: serde_json::json!({"url": "about:blank"}),
    };
    let (_resp, events) = session.handle_request(&mut page, &req, None).await;

    let event_methods: Vec<&str> = events.iter().map(|e| e.method.as_str()).collect();
    assert!(event_methods.contains(&"Page.frameNavigated"));
    assert!(event_methods.contains(&"Page.domContentEventFired"));
    assert!(event_methods.contains(&"Page.loadEventFired"));
}

#[tokio::test]
async fn cdp_session_dom_get_document() {
    use browser_oxide::protocol::{CdpRequest, CdpSession};

    let mut session = CdpSession::new();
    let mut page = Page::from_html(
        "<html><head></head><body><p>Test</p></body></html>",
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let req = CdpRequest {
        id: 3,
        method: "DOM.getDocument".into(),
        params: serde_json::json!({}),
    };
    let (resp, _) = session.handle_request(&mut page, &req, None).await;
    assert!(resp.contains("#document"), "response: {}", resp);
}

#[tokio::test]
async fn page_evaluate_returns_result() {
    let mut page = Page::from_html(
        "<html><head></head><body></body></html>",
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(page.evaluate("2 + 2").unwrap(), "4");
    assert_eq!(page.evaluate("'hello'.toUpperCase()").unwrap(), "HELLO");
    assert_eq!(
        page.evaluate("JSON.stringify({a: 1})").unwrap(),
        "{\"a\":1}"
    );
}

#[tokio::test]
async fn take_dom_preserves_content() {
    let page = Page::from_html(
        "<html><head></head><body><p id=\"keep\">Preserved</p></body></html>",
        None::<browser_oxide::stealth::StealthProfile>,
    )
    .await
    .unwrap();

    let dom = page.take_dom();
    let ps = dom.get_elements_by_tag_name(browser_oxide::dom::NodeId::DOCUMENT, "p");
    assert!(!ps.is_empty());
    assert_eq!(dom.text_content(ps[0]), "Preserved");
}
