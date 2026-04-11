use js_runtime::BrowserJsRuntime;

fn create_test_runtime() -> BrowserJsRuntime {
    let dom = html_parser::parse_html(
        "<html><head><title>Test</title></head><body><div id=\"main\" class=\"container\"><p>Hello world</p></div></body></html>"
    );
    BrowserJsRuntime::new(dom)
}

#[test]
fn basic_js_execution() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("1 + 2").unwrap();
    assert_eq!(result, "3");
}

#[test]
fn console_log_capture() {
    let mut rt = create_test_runtime();
    rt.execute_script("console.log('hello from JS')").unwrap();
    let output = rt.console_output();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].args[0], "hello from JS");
}

#[test]
fn document_exists() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("typeof document").unwrap();
    assert_eq!(result, "object");
}

#[test]
fn document_query_selector() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.querySelector('#main').tagName")
        .unwrap();
    assert_eq!(result, "DIV");
}

#[test]
fn document_get_element_by_id() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.getElementById('main').className")
        .unwrap();
    assert_eq!(result, "container");
}

#[test]
fn element_text_content() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.querySelector('p').textContent")
        .unwrap();
    assert_eq!(result, "Hello world");
}

#[test]
fn element_inner_html() {
    let mut rt = create_test_runtime();
    let result = rt
        .execute_script("document.querySelector('#main').innerHTML")
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
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('#new-span').tagName")
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
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('p').textContent")
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
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('#main span').textContent")
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
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('#main').getAttribute('data-test')")
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
    )
    .unwrap();

    let result = rt
        .execute_script("document.querySelector('#main').className")
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
    let result = rt.execute_script("document.title").unwrap();
    assert_eq!(result, "Test");
}

#[test]
fn document_has_focus() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("document.hasFocus()").unwrap();
    assert_eq!(result, "true");
}

#[test]
fn document_visibility_state() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("document.visibilityState").unwrap();
    assert_eq!(result, "visible");
}

#[test]
fn window_self_reference() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("window === globalThis").unwrap();
    assert_eq!(result, "true");
}

#[test]
fn node_types_exist() {
    let mut rt = create_test_runtime();
    let result = rt.execute_script("typeof Element").unwrap();
    assert_eq!(result, "function");
    let result = rt.execute_script("typeof Document").unwrap();
    assert_eq!(result, "function");
    let result = rt.execute_script("typeof Node").unwrap();
    assert_eq!(result, "function");
}

#[test]
fn take_dom_back() {
    let rt = create_test_runtime();
    let dom = rt.take_dom();
    // Verify the DOM is intact
    let html = dom.child_elements(dom::NodeId::DOCUMENT);
    assert!(!html.is_empty());
}

#[test]
fn query_selector_all() {
    let dom = html_parser::parse_html(
        "<html><body><ul><li>a</li><li>b</li><li>c</li></ul></body></html>",
    );
    let mut rt = BrowserJsRuntime::new(dom);
    let result = rt
        .execute_script("document.querySelectorAll('li').length")
        .unwrap();
    assert_eq!(result, "3");
}

#[test]
fn get_elements_by_tag_name() {
    let mut rt = create_test_runtime();
    // The test HTML has: html, head, title, body, div, p
    let result = rt
        .execute_script("document.getElementsByTagName('div').length")
        .unwrap();
    assert_eq!(result, "1");
}

// === Stealth profile tests ===

#[test]
fn stealth_profile_overrides_navigator() {
    let profile = stealth::chrome_130_windows();
    let dom = html_parser::parse_html("<html><head></head><body></body></html>");
    let mut rt = BrowserJsRuntime::with_profile(dom, profile);

    let ua = rt.execute_script("navigator.userAgent").unwrap();
    assert!(
        ua.contains("Windows NT 10.0"),
        "UA should be Windows: {}",
        ua
    );

    let platform = rt.execute_script("navigator.platform").unwrap();
    assert_eq!(platform, "Win32");

    let cores = rt.execute_script("navigator.hardwareConcurrency").unwrap();
    assert_eq!(cores, "8");
}

#[test]
fn stealth_profile_overrides_screen() {
    let profile = stealth::chrome_130_macos();
    let dom = html_parser::parse_html("<html><head></head><body></body></html>");
    let mut rt = BrowserJsRuntime::with_profile(dom, profile);

    let w = rt.execute_script("screen.width").unwrap();
    assert_eq!(w, "1440"); // macOS profile = 1440

    let dpr = rt.execute_script("devicePixelRatio").unwrap();
    assert_eq!(dpr, "2"); // macOS Retina = 2x
}

#[test]
fn stealth_profile_overrides_window_dims() {
    let profile = stealth::chrome_130_linux();
    let dom = html_parser::parse_html("<html><head></head><body></body></html>");
    let mut rt = BrowserJsRuntime::with_profile(dom, profile);

    let iw = rt.execute_script("window.innerWidth").unwrap();
    assert_eq!(iw, "1920");

    let ih = rt.execute_script("window.innerHeight").unwrap();
    assert_eq!(ih, "969");
}

#[test]
fn no_profile_uses_defaults() {
    let mut rt = create_test_runtime(); // no profile
    let ua = rt.execute_script("navigator.userAgent").unwrap();
    assert!(
        ua.contains("Chrome"),
        "Default UA should contain Chrome: {}",
        ua
    );
}
