use browser_oxide::js_runtime::BrowserJsRuntime;

fn create_test_runtime() -> BrowserJsRuntime {
    let dom = browser_oxide::html_parser::parse_html(
        "<html><head><title>Test</title></head><body><div id=\"main\" class=\"container\"><p>Hello world</p></div></body></html>"
    );
    BrowserJsRuntime::new(dom)
}

#[test]
fn basic_js_execution() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("1 + 2", None).unwrap();
    assert_eq!(result, "3");
}

#[test]
fn console_log_capture() {
    let mut rt = create_test_runtime();
    rt.execute_script("console.log('hello from JS')", None)
        .unwrap();
    let output = rt.console_output();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].args[0], "[string] hello from JS");
}

#[test]
fn document_exists() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("typeof document", None).unwrap();
    assert_eq!(result, "object");
}

#[test]
fn document_query_selector() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.querySelector('#main').tagName", None)
        .unwrap();
    assert_eq!(result, "DIV");
}

#[test]
fn document_get_element_by_id() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.getElementById('main').className", None)
        .unwrap();
    assert_eq!(result, "container");
}

#[test]
fn element_text_content() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.querySelector('p').textContent", None)
        .unwrap();
    assert_eq!(result, "Hello world");
}

#[test]
fn element_inner_html() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.querySelector('#main').innerHTML", None)
        .unwrap();
    assert!(
        result.contains("<p>"),
        "innerHTML should contain <p>, got: {}",
        result
    );
    assert!(
        result.contains("Hello world"),
        "innerHTML should contain text"
    );
}

#[test]
fn create_element_and_append() {
    let mut rt = create_test_runtime();
    rt.execute_script(
        r#"
        const div = document.createElement('span');
        div.setAttribute('id', 'new-span');
        document.querySelector('#main').appendChild(div);
    "#,
        None,
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('#new-span').tagName", None)
        .unwrap();
    assert_eq!(result, "SPAN");
}

#[test]
fn set_text_content() {
    let mut rt = create_test_runtime();
    rt.execute_script(
        r#"
        document.querySelector('p').textContent = 'Modified!';
    "#,
        None,
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('p').textContent", None)
        .unwrap();
    assert_eq!(result, "Modified!");
}

#[test]
fn set_inner_html() {
    let mut rt = create_test_runtime();
    rt.execute_script(
        r#"
        document.querySelector('#main').innerHTML = '<span>New content</span>';
    "#,
        None,
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('#main span').textContent", None)
        .unwrap();
    assert_eq!(result, "New content");
}

#[test]
fn set_attribute() {
    let mut rt = create_test_runtime();
    rt.execute_script(
        r#"
        document.querySelector('#main').setAttribute('data-test', 'hello');
    "#,
        None,
    )
    .unwrap();

    let result = rt
        .execute_script(
            "document.querySelector('#main').getAttribute('data-test')",
            None,
        )
        .unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn class_list() {
    let mut rt = create_test_runtime();
    rt.execute_script(
        r#"
        const el = document.querySelector('#main');
        el.classList.add('new-class');
    "#,
        None,
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('#main').className", None)
        .unwrap();
    assert!(
        result.contains("new-class"),
        "className should contain new-class, got: {}",
        result
    );
    assert!(
        result.contains("container"),
        "className should still contain container"
    );
}

#[test]
fn document_title() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("document.title", None).unwrap();
    assert_eq!(result, "Test");
}

#[test]
fn document_has_focus() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("document.hasFocus()", None).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn document_visibility_state() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("document.visibilityState", None).unwrap();
    assert_eq!(result, "visible");
}

#[test]
fn window_self_reference() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("window === globalThis", None).unwrap();
    assert_eq!(result, "true");
}

#[test]
fn node_types_exist() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("typeof Element", None).unwrap();
    assert_eq!(result, "function");
    let result = rt.execute_script("typeof Document", None).unwrap();
    assert_eq!(result, "function");
    let result = rt.execute_script("typeof Node", None).unwrap();
    assert_eq!(result, "function");
}

#[test]
fn take_dom_back() {
    let rt = create_test_runtime();
    let dom = rt.take_dom();
    // Verify the DOM is intact
    let html = dom.child_elements(browser_oxide::dom::NodeId::DOCUMENT);
    assert!(!html.is_empty());
}

#[test]
fn query_selector_all() {
    let dom = browser_oxide::html_parser::parse_html(
        "<html><body><ul><li>a</li><li>b</li><li>c</li></ul></body></html>",
    );
    let mut rt = BrowserJsRuntime::new(dom);
    let result = rt
        .execute_script("document.querySelectorAll('li').length", None)
        .unwrap();
    assert_eq!(result, "3");
}

#[test]
fn get_elements_by_tag_name() {
    let mut rt = create_test_runtime();
    // The test HTML has: html, head, title, body, div, p
    let result = rt
        .execute_script("document.getElementsByTagName('div').length", None)
        .unwrap();
    assert_eq!(result, "1");
}

// === Stealth profile tests ===

#[test]
fn stealth_profile_overrides_navigator() {
    let profile = browser_oxide::stealth::chrome_148_windows();
    let dom = browser_oxide::html_parser::parse_html("<html><head></head><body></body></html>");
    let mut rt = BrowserJsRuntime::with_profile(dom, profile);

    let ua = rt.execute_script("navigator.userAgent", None).unwrap();
    assert!(
        ua.contains("Windows NT 10.0"),
        "UA should be Windows: {}",
        ua
    );

    let platform = rt.execute_script("navigator.platform", None).unwrap();
    assert_eq!(platform, "Win32");

    let cores = rt
        .execute_script("navigator.hardwareConcurrency", None)
        .unwrap();
    assert_eq!(cores, "8");
}

#[test]
fn stealth_profile_overrides_screen() {
    let profile = browser_oxide::stealth::chrome_148_macos();
    let dom = browser_oxide::html_parser::parse_html("<html><head></head><body></body></html>");
    let mut rt = BrowserJsRuntime::with_profile(dom, profile);

    let w = rt.execute_script("screen.width", None).unwrap();
    assert_eq!(w, "1512"); // macOS M3 MacBook Pro profile = 1512

    let dpr = rt.execute_script("devicePixelRatio", None).unwrap();
    assert_eq!(dpr, "2"); // macOS Retina = 2x
}

#[test]
fn stealth_profile_overrides_window_dims() {
    let profile = browser_oxide::stealth::chrome_148_linux();
    let dom = browser_oxide::html_parser::parse_html("<html><head></head><body></body></html>");
    let mut rt = BrowserJsRuntime::with_profile(dom, profile);

    let iw = rt.execute_script("window.innerWidth", None).unwrap();
    assert_eq!(iw, "1920");

    let ih = rt.execute_script("window.innerHeight", None).unwrap();
    assert_eq!(ih, "969");
}

#[test]
fn get_computed_style_basic() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("getComputedStyle(document.body).display", None)
        .unwrap();
    assert_eq!(result, "block");
}

#[test]
fn get_computed_style_instanceof() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script(
            "getComputedStyle(document.body) instanceof CSSStyleDeclaration",
            None,
        )
        .unwrap();
    assert_eq!(result, "true");
}

#[test]
fn no_profile_uses_defaults() {
    let mut rt = create_test_runtime(); // no profile
    let ua = rt.execute_script("navigator.userAgent", None).unwrap();
    assert!(
        ua.contains("Chrome"),
        "Default UA should contain Chrome: {}",
        ua
    );
}

// ===== vNext/10 — URL polyfill opaque-scheme handling =====

/// Real Chrome on `new URL("blob:null/uuid").protocol` returns `"blob:"`.
/// Pre-fix, BO's URL polyfill emitted `""`. Caught during worker-URL testing
/// and fixed in commit (this commit).
#[test]
fn url_blob_scheme_protocol_and_origin() {
    let mut rt = create_test_runtime();
    let proto = rt
        .execute_script(r#"new URL("blob:null/7aeb61c9-deadbeef").protocol"#, None)
        .unwrap();
    assert_eq!(proto, "blob:");
    let origin = rt
        .execute_script(r#"new URL("blob:null/7aeb61c9-deadbeef").origin"#, None)
        .unwrap();
    assert_eq!(origin, "null");
    let href = rt
        .execute_script(r#"new URL("blob:null/7aeb61c9-deadbeef").href"#, None)
        .unwrap();
    assert_eq!(href, "blob:null/7aeb61c9-deadbeef");
}

/// `data:` URLs are opaque per WHATWG URL spec: protocol="data:",
/// origin="null". Real Chrome behavior.
#[test]
fn url_data_scheme_protocol_and_origin() {
    let mut rt = create_test_runtime();
    let proto = rt
        .execute_script(r#"new URL("data:text/html,<p>hi</p>").protocol"#, None)
        .unwrap();
    assert_eq!(proto, "data:");
    let origin = rt
        .execute_script(r#"new URL("data:text/html,<p>hi</p>").origin"#, None)
        .unwrap();
    assert_eq!(origin, "null");
}

/// `javascript:` URLs are also opaque.
#[test]
fn url_javascript_scheme_protocol() {
    let mut rt = create_test_runtime();
    let proto = rt
        .execute_script(r#"new URL("javascript:void(0)").protocol"#, None)
        .unwrap();
    assert_eq!(proto, "javascript:");
}

/// `about:` URLs (about:blank, about:srcdoc) are opaque.
#[test]
fn url_about_scheme_protocol() {
    let mut rt = create_test_runtime();
    let proto = rt
        .execute_script(r#"new URL("about:blank").protocol"#, None)
        .unwrap();
    assert_eq!(proto, "about:");
}

/// Regression: http(s) URLs must still parse correctly — the opaque-scheme
/// branch is added BEFORE the http regex, so it must not divert non-opaque
/// schemes.
#[test]
fn url_https_still_parses_after_opaque_branch() {
    let mut rt = create_test_runtime();
    let proto = rt
        .execute_script(r#"new URL("https://example.com/foo?a=1#b").protocol"#, None)
        .unwrap();
    assert_eq!(proto, "https:");
    let host = rt
        .execute_script(r#"new URL("https://example.com/foo?a=1#b").host"#, None)
        .unwrap();
    assert_eq!(host, "example.com");
    let origin = rt
        .execute_script(r#"new URL("https://example.com/foo?a=1#b").origin"#, None)
        .unwrap();
    assert_eq!(origin, "https://example.com");
}
