//! W3C Web API conformance tests.
//!
//! Tests real DOM behavior — not just API surface existence, but correct
//! mutation semantics (remove actually detaches, cloneNode deep copies, etc.).

use browser::Page;
use std::time::Duration;
use stealth;

fn html(body: &str) -> String {
    format!("<html><head></head><body>{}</body></html>", body)
}

async fn eval(js: &str) -> String {
    let mut page = Page::from_html(&html(""), None::<stealth::StealthProfile>)
        .await
        .unwrap();
    page.evaluate(js).unwrap()
}

async fn page_with(body: &str) -> Page {
    Page::from_html(&html(body), None::<stealth::StealthProfile>)
        .await
        .unwrap()
}

// ================================================================
// Node properties
// ================================================================

#[tokio::test]
async fn node_constants() {
    assert_eq!(eval("Node.ELEMENT_NODE").await, "1");
    assert_eq!(eval("Node.TEXT_NODE").await, "3");
    assert_eq!(eval("Node.COMMENT_NODE").await, "8");
    assert_eq!(eval("Node.DOCUMENT_NODE").await, "9");
    assert_eq!(eval("Node.DOCUMENT_FRAGMENT_NODE").await, "11");
}

#[tokio::test]
async fn node_owner_document() {
    assert_eq!(
        eval("document.body.ownerDocument === document").await,
        "true"
    );
    assert_eq!(eval("document.ownerDocument").await, "null");
}

#[tokio::test]
async fn node_is_connected() {
    let mut page = page_with("<div id='a'></div>").await;
    assert_eq!(
        page.evaluate("document.getElementById('a').isConnected")
            .unwrap(),
        "true"
    );
    page.evaluate("document.getElementById('a').remove()")
        .unwrap();
    // After removal, create a new disconnected element
    assert_eq!(
        page.evaluate("document.createElement('div').isConnected")
            .unwrap(),
        "false"
    );
}

#[tokio::test]
async fn node_value() {
    let mut page = page_with("<p>hello</p>").await;
    // Element nodeValue is null
    assert_eq!(
        page.evaluate("document.querySelector('p').nodeValue")
            .unwrap(),
        "null"
    );
    // Text node nodeValue is its data
    assert_eq!(
        page.evaluate("document.querySelector('p').firstChild.nodeValue")
            .unwrap(),
        "hello"
    );
}

#[tokio::test]
async fn node_base_uri() {
    assert_eq!(eval("typeof document.body.baseURI").await, "string");
}

// ================================================================
// Element.remove() — real DOM detachment
// ================================================================

#[tokio::test]
async fn element_remove_detaches() {
    let mut page = page_with("<div id='parent'><span id='child'>x</span></div>").await;
    // Verify child exists
    assert_eq!(
        page.evaluate("document.getElementById('child') !== null")
            .unwrap(),
        "true"
    );
    // Remove it
    page.evaluate("document.getElementById('child').remove()")
        .unwrap();
    // Should no longer be findable
    assert_eq!(
        page.evaluate("document.getElementById('child')").unwrap(),
        "null"
    );
    // Parent should have no element children
    assert_eq!(
        page.evaluate("document.getElementById('parent').children.length")
            .unwrap(),
        "0"
    );
}

#[tokio::test]
async fn element_remove_parent_is_null() {
    let mut page = page_with("<div id='el'></div>").await;
    page.evaluate(
        r#"
        const el = document.getElementById('el');
        el.remove();
        globalThis._parentAfterRemove = el.parentNode;
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("globalThis._parentAfterRemove").unwrap(),
        "null"
    );
}

// ================================================================
// Element.append/prepend/after/before/replaceWith
// ================================================================

#[tokio::test]
async fn element_append() {
    let mut page = page_with("<div id='c'></div>").await;
    page.evaluate(
        r#"
        const c = document.getElementById('c');
        const a = document.createElement('span');
        a.textContent = 'A';
        const b = document.createElement('span');
        b.textContent = 'B';
        c.append(a, b);
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('c').textContent")
            .unwrap(),
        "AB"
    );
}

#[tokio::test]
async fn element_append_strings() {
    let mut page = page_with("<div id='c'></div>").await;
    page.evaluate("document.getElementById('c').append('hello ', 'world')")
        .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('c').textContent")
            .unwrap(),
        "hello world"
    );
}

#[tokio::test]
async fn element_prepend() {
    let mut page = page_with("<div id='c'><span>B</span></div>").await;
    page.evaluate(
        r#"
        const a = document.createElement('span');
        a.textContent = 'A';
        document.getElementById('c').prepend(a);
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('c').textContent")
            .unwrap(),
        "AB"
    );
}

#[tokio::test]
async fn element_after() {
    let mut page = page_with("<div id='parent'><span id='first'>A</span></div>").await;
    page.evaluate(
        r#"
        const b = document.createElement('span');
        b.textContent = 'B';
        document.getElementById('first').after(b);
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('parent').textContent")
            .unwrap(),
        "AB"
    );
    assert_eq!(
        page.evaluate("document.getElementById('parent').children.length")
            .unwrap(),
        "2"
    );
}

#[tokio::test]
async fn element_before() {
    let mut page = page_with("<div id='parent'><span id='last'>B</span></div>").await;
    page.evaluate(
        r#"
        const a = document.createElement('span');
        a.textContent = 'A';
        document.getElementById('last').before(a);
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('parent').textContent")
            .unwrap(),
        "AB"
    );
}

#[tokio::test]
async fn element_replace_with() {
    let mut page = page_with("<div id='parent'><span id='old'>old</span></div>").await;
    page.evaluate(
        r#"
        const n = document.createElement('span');
        n.id = 'new';
        n.textContent = 'new';
        document.getElementById('old').replaceWith(n);
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('old')").unwrap(),
        "null"
    );
    assert_eq!(
        page.evaluate("document.getElementById('new').textContent")
            .unwrap(),
        "new"
    );
}

#[tokio::test]
async fn element_replace_children() {
    let mut page = page_with("<div id='c'><span>A</span><span>B</span></div>").await;
    page.evaluate(
        r#"
        const c = document.getElementById('c');
        const x = document.createElement('span');
        x.textContent = 'X';
        c.replaceChildren(x, 'Y');
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('c').textContent")
            .unwrap(),
        "XY"
    );
}

// ================================================================
// cloneNode — deep and shallow
// ================================================================

#[tokio::test]
async fn clone_node_shallow() {
    let mut page = page_with("<div id='src' class='a'><span>child</span></div>").await;
    page.evaluate(
        r#"
        const clone = document.getElementById('src').cloneNode(false);
        clone.id = 'clone';
        document.body.appendChild(clone);
    "#,
    )
    .unwrap();
    // Shallow clone should have no children
    assert_eq!(
        page.evaluate("document.getElementById('clone').children.length")
            .unwrap(),
        "0"
    );
    assert_eq!(
        page.evaluate("document.getElementById('clone').className")
            .unwrap(),
        "a"
    );
}

#[tokio::test]
async fn clone_node_deep() {
    let mut page = page_with("<div id='src'><span>child</span></div>").await;
    page.evaluate(
        r#"
        const clone = document.getElementById('src').cloneNode(true);
        clone.id = 'clone';
        document.body.appendChild(clone);
    "#,
    )
    .unwrap();
    // Deep clone should have the span child
    assert_eq!(
        page.evaluate("document.getElementById('clone').children.length")
            .unwrap(),
        "1"
    );
    assert_eq!(
        page.evaluate("document.getElementById('clone').textContent")
            .unwrap(),
        "child"
    );
}

#[tokio::test]
async fn clone_node_independent() {
    let mut page = page_with("<div id='src'><span>original</span></div>").await;
    page.evaluate(
        r#"
        const src = document.getElementById('src');
        const clone = src.cloneNode(true);
        clone.id = 'clone';
        document.body.appendChild(clone);
        // Modify the clone — original should not change
        clone.querySelector('span').textContent = 'modified';
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('src').textContent")
            .unwrap(),
        "original"
    );
    assert_eq!(
        page.evaluate("document.getElementById('clone').textContent")
            .unwrap(),
        "modified"
    );
}

// ================================================================
// insertAdjacentHTML
// ================================================================

#[tokio::test]
async fn insert_adjacent_html_beforebegin() {
    let mut page = page_with("<div id='parent'><span id='ref'>R</span></div>").await;
    page.evaluate("document.getElementById('ref').insertAdjacentHTML('beforebegin', '<em>A</em>')")
        .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('parent').textContent")
            .unwrap(),
        "AR"
    );
    assert_eq!(
        page.evaluate("document.getElementById('parent').children.length")
            .unwrap(),
        "2"
    );
}

#[tokio::test]
async fn insert_adjacent_html_afterbegin() {
    let mut page = page_with("<div id='c'><span>B</span></div>").await;
    page.evaluate(
        "document.getElementById('c').insertAdjacentHTML('afterbegin', '<span>A</span>')",
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('c').textContent")
            .unwrap(),
        "AB"
    );
}

#[tokio::test]
async fn insert_adjacent_html_beforeend() {
    let mut page = page_with("<div id='c'><span>A</span></div>").await;
    page.evaluate("document.getElementById('c').insertAdjacentHTML('beforeend', '<span>B</span>')")
        .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('c').textContent")
            .unwrap(),
        "AB"
    );
}

#[tokio::test]
async fn insert_adjacent_html_afterend() {
    let mut page = page_with("<div id='parent'><span id='ref'>A</span></div>").await;
    page.evaluate("document.getElementById('ref').insertAdjacentHTML('afterend', '<em>B</em>')")
        .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('parent').textContent")
            .unwrap(),
        "AB"
    );
}

// ================================================================
// toggleAttribute
// ================================================================

#[tokio::test]
async fn toggle_attribute() {
    let mut page = page_with("<div id='el'></div>").await;
    assert_eq!(
        page.evaluate("document.getElementById('el').toggleAttribute('hidden')")
            .unwrap(),
        "true"
    );
    assert_eq!(
        page.evaluate("document.getElementById('el').hasAttribute('hidden')")
            .unwrap(),
        "true"
    );
    assert_eq!(
        page.evaluate("document.getElementById('el').toggleAttribute('hidden')")
            .unwrap(),
        "false"
    );
    assert_eq!(
        page.evaluate("document.getElementById('el').hasAttribute('hidden')")
            .unwrap(),
        "false"
    );
}

// ================================================================
// replaceChild
// ================================================================

#[tokio::test]
async fn replace_child() {
    let mut page = page_with("<div id='p'><span id='old'>old</span></div>").await;
    page.evaluate(
        r#"
        const p = document.getElementById('p');
        const n = document.createElement('span');
        n.id = 'new';
        n.textContent = 'new';
        p.replaceChild(n, document.getElementById('old'));
    "#,
    )
    .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('old')").unwrap(),
        "null"
    );
    assert_eq!(
        page.evaluate("document.getElementById('new').textContent")
            .unwrap(),
        "new"
    );
}

// ================================================================
// Node comparison methods
// ================================================================

#[tokio::test]
async fn is_same_node() {
    let mut page = page_with("<div id='a'></div>").await;
    assert_eq!(
        page.evaluate(
            r#"
        const _a1 = document.getElementById('a');
        _a1.isSameNode(_a1)
    "#
        )
        .unwrap(),
        "true"
    );
    assert_eq!(
        page.evaluate(
            r#"
        const _a2 = document.getElementById('a');
        _a2.isSameNode(document.body)
    "#
        )
        .unwrap(),
        "false"
    );
}

#[tokio::test]
async fn is_equal_node() {
    let mut page = page_with("").await;
    page.evaluate(
        r#"
        const a = document.createElement('div');
        a.className = 'x';
        a.textContent = 'hi';
        const b = a.cloneNode(true);
        globalThis._eq = a.isEqualNode(b);
    "#,
    )
    .unwrap();
    assert_eq!(page.evaluate("globalThis._eq").unwrap(), "true");
}

// ================================================================
// Element navigation: nextElementSibling, previousElementSibling
// ================================================================

#[tokio::test]
async fn element_sibling_navigation() {
    let mut page = page_with("<div id='a'></div><div id='b'></div><div id='c'></div>").await;
    assert_eq!(
        page.evaluate("document.getElementById('a').nextElementSibling.id")
            .unwrap(),
        "b"
    );
    assert_eq!(
        page.evaluate("document.getElementById('c').previousElementSibling.id")
            .unwrap(),
        "b"
    );
    assert_eq!(
        page.evaluate("document.getElementById('c').nextElementSibling")
            .unwrap(),
        "null"
    );
}

// ================================================================
// dataset
// ================================================================

#[tokio::test]
async fn element_dataset() {
    let mut page = page_with("<div id='el' data-foo='bar'></div>").await;
    assert_eq!(
        page.evaluate("document.getElementById('el').dataset.foo")
            .unwrap(),
        "bar"
    );
    page.evaluate("document.getElementById('el').dataset.baz = 'qux'")
        .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('el').getAttribute('data-baz')")
            .unwrap(),
        "qux"
    );
}

// ================================================================
// document.write — appends to body
// ================================================================

#[tokio::test]
async fn document_write_creates_elements() {
    let mut page = page_with("").await;
    page.evaluate("document.write('<div id=\"written\">content</div>')")
        .unwrap();
    assert_eq!(
        page.evaluate("document.getElementById('written').textContent")
            .unwrap(),
        "content"
    );
}

// ================================================================
// Event system — real dispatch and propagation
// ================================================================

#[tokio::test]
async fn event_dispatch_fires_listener() {
    let mut page = page_with("<div id='el'></div>").await;
    page.evaluate(r#"
        globalThis._fired = false;
        document.getElementById('el').addEventListener('click', () => { globalThis._fired = true; });
        document.getElementById('el').dispatchEvent(new Event('click'));
    "#).unwrap();
    assert_eq!(page.evaluate("globalThis._fired").unwrap(), "true");
}

#[tokio::test]
async fn event_bubbling() {
    let mut page = page_with("<div id='parent'><span id='child'>x</span></div>").await;
    page.evaluate(r#"
        globalThis._parentGotEvent = false;
        document.getElementById('parent').addEventListener('click', () => { globalThis._parentGotEvent = true; });
        document.getElementById('child').dispatchEvent(new Event('click', { bubbles: true }));
    "#).unwrap();
    assert_eq!(page.evaluate("globalThis._parentGotEvent").unwrap(), "true");
}

#[tokio::test]
async fn event_no_bubble_when_not_set() {
    let mut page = page_with("<div id='parent'><span id='child'>x</span></div>").await;
    page.evaluate(r#"
        globalThis._parentGotEvent = false;
        document.getElementById('parent').addEventListener('test', () => { globalThis._parentGotEvent = true; });
        document.getElementById('child').dispatchEvent(new Event('test', { bubbles: false }));
    "#).unwrap();
    assert_eq!(
        page.evaluate("globalThis._parentGotEvent").unwrap(),
        "false"
    );
}

#[tokio::test]
async fn event_once_listener() {
    let mut page = page_with("<div id='el'></div>").await;
    page.evaluate(r#"
        globalThis._count = 0;
        document.getElementById('el').addEventListener('ping', () => { globalThis._count++; }, { once: true });
        document.getElementById('el').dispatchEvent(new Event('ping'));
        document.getElementById('el').dispatchEvent(new Event('ping'));
    "#).unwrap();
    assert_eq!(page.evaluate("globalThis._count").unwrap(), "1");
}

#[tokio::test]
async fn event_stop_propagation() {
    let mut page = page_with("<div id='parent'><span id='child'>x</span></div>").await;
    page.evaluate(r#"
        globalThis._parentGotEvent = false;
        document.getElementById('child').addEventListener('click', (e) => { e.stopPropagation(); });
        document.getElementById('parent').addEventListener('click', () => { globalThis._parentGotEvent = true; });
        document.getElementById('child').dispatchEvent(new Event('click', { bubbles: true }));
    "#).unwrap();
    assert_eq!(
        page.evaluate("globalThis._parentGotEvent").unwrap(),
        "false"
    );
}

#[tokio::test]
async fn event_prevent_default() {
    let mut page = page_with("<div id='el'></div>").await;
    page.evaluate(r#"
        document.getElementById('el').addEventListener('test', (e) => { e.preventDefault(); });
        globalThis._result = document.getElementById('el').dispatchEvent(new Event('test', { cancelable: true }));
    "#).unwrap();
    assert_eq!(page.evaluate("globalThis._result").unwrap(), "false");
}

// ================================================================
// Event classes exist
// ================================================================

#[tokio::test]
async fn event_classes_exist() {
    assert_eq!(eval("typeof MouseEvent").await, "function");
    assert_eq!(eval("typeof KeyboardEvent").await, "function");
    assert_eq!(eval("typeof InputEvent").await, "function");
    assert_eq!(eval("typeof FocusEvent").await, "function");
    assert_eq!(eval("typeof PointerEvent").await, "function");
    assert_eq!(eval("typeof WheelEvent").await, "function");
    assert_eq!(eval("typeof TouchEvent").await, "function");
    assert_eq!(eval("typeof MessageEvent").await, "function");
    assert_eq!(eval("typeof ErrorEvent").await, "function");
    assert_eq!(eval("typeof CustomEvent").await, "function");
    assert_eq!(eval("typeof AnimationEvent").await, "function");
    assert_eq!(eval("typeof TransitionEvent").await, "function");
    assert_eq!(eval("typeof ClipboardEvent").await, "function");
    assert_eq!(eval("typeof PopStateEvent").await, "function");
    assert_eq!(eval("typeof DragEvent").await, "function");
    assert_eq!(eval("typeof EventTarget").await, "function");
}

#[tokio::test]
async fn mouse_event_has_properties() {
    assert_eq!(
        eval(
            r#"
        const e = new MouseEvent('click', { clientX: 100, clientY: 200, button: 1 });
        e.clientX + ',' + e.clientY + ',' + e.button
    "#
        )
        .await,
        "100,200,1"
    );
}

#[tokio::test]
async fn keyboard_event_has_properties() {
    assert_eq!(
        eval(
            r#"
        const e = new KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 13 });
        e.key + ',' + e.code + ',' + e.keyCode
    "#
        )
        .await,
        "Enter,Enter,13"
    );
}

// ================================================================
// Window APIs
// ================================================================

#[tokio::test]
async fn history_push_state() {
    let mut page = page_with("").await;
    page.evaluate("history.pushState({ page: 1 }, '', '/page1')")
        .unwrap();
    assert_eq!(page.evaluate("history.state.page").unwrap(), "1");
    assert_eq!(page.evaluate("history.length").unwrap(), "2");
    page.evaluate("history.pushState({ page: 2 }, '', '/page2')")
        .unwrap();
    assert_eq!(page.evaluate("history.length").unwrap(), "3");
    page.evaluate("history.back()").unwrap();
    assert_eq!(page.evaluate("history.state.page").unwrap(), "1");
}

#[tokio::test]
async fn history_replace_state() {
    let mut page = page_with("").await;
    page.evaluate("history.replaceState({ replaced: true }, '', '/new')")
        .unwrap();
    assert_eq!(page.evaluate("history.state.replaced").unwrap(), "true");
    assert_eq!(page.evaluate("history.length").unwrap(), "1"); // doesn't add
}

#[tokio::test]
async fn match_media_exists() {
    assert_eq!(eval("typeof matchMedia").await, "function");
    assert_eq!(
        eval("typeof matchMedia('(min-width: 100px)').matches").await,
        "boolean"
    );
}

#[tokio::test]
async fn match_media_evaluates() {
    // innerWidth defaults to 1920
    assert_eq!(
        eval("matchMedia('(min-width: 100px)').matches").await,
        "true"
    );
    assert_eq!(
        eval("matchMedia('(max-width: 100px)').matches").await,
        "false"
    );
}

// ================================================================
// AbortController
// ================================================================

#[tokio::test]
async fn abort_controller_basic() {
    let mut page = page_with("").await;
    page.evaluate(
        r#"
        const ac = new AbortController();
        globalThis._aborted = ac.signal.aborted;
        ac.abort();
        globalThis._abortedAfter = ac.signal.aborted;
    "#,
    )
    .unwrap();
    assert_eq!(page.evaluate("globalThis._aborted").unwrap(), "false");
    assert_eq!(page.evaluate("globalThis._abortedAfter").unwrap(), "true");
}

#[tokio::test]
async fn abort_signal_fires_listener() {
    let mut page = page_with("").await;
    page.evaluate(
        r#"
        const ac = new AbortController();
        globalThis._notified = false;
        ac.signal.addEventListener('abort', () => { globalThis._notified = true; });
        ac.abort();
    "#,
    )
    .unwrap();
    assert_eq!(page.evaluate("globalThis._notified").unwrap(), "true");
}

// ================================================================
// DOMParser
// ================================================================

#[tokio::test]
async fn dom_parser() {
    let mut page = page_with("").await;
    page.evaluate(
        r#"
        const parser = new DOMParser();
        const doc = parser.parseFromString('<div id="parsed">content</div>', 'text/html');
        globalThis._text = doc.querySelector('#parsed').textContent;
    "#,
    )
    .unwrap();
    assert_eq!(page.evaluate("globalThis._text").unwrap(), "content");
}

// ================================================================
// URLSearchParams
// ================================================================

#[tokio::test]
async fn url_search_params() {
    assert_eq!(eval("new URLSearchParams('a=1&b=2').get('a')").await, "1");
    assert_eq!(eval("new URLSearchParams('a=1&b=2').get('b')").await, "2");
    assert_eq!(
        eval("new URLSearchParams('a=1&b=2').has('a')").await,
        "true"
    );
    assert_eq!(
        eval("new URLSearchParams('a=1&b=2').has('c')").await,
        "false"
    );
}

#[tokio::test]
async fn url_search_params_mutations() {
    let mut page = page_with("").await;
    page.evaluate(
        r#"
        const p = new URLSearchParams('a=1');
        p.set('a', '2');
        p.append('b', '3');
        globalThis._result = p.toString();
    "#,
    )
    .unwrap();
    assert_eq!(page.evaluate("globalThis._result").unwrap(), "a=2&b=3");
}

// ================================================================
// FormData
// ================================================================

#[tokio::test]
async fn form_data_basic() {
    let mut page = page_with("").await;
    page.evaluate(
        r#"
        const fd = new FormData();
        fd.append('name', 'value');
        globalThis._has = fd.has('name');
        globalThis._get = fd.get('name');
    "#,
    )
    .unwrap();
    assert_eq!(page.evaluate("globalThis._has").unwrap(), "true");
    assert_eq!(page.evaluate("globalThis._get").unwrap(), "value");
}

// ================================================================
// Image constructor
// ================================================================

#[tokio::test]
async fn image_constructor() {
    assert_eq!(eval("typeof Image").await, "function");
    assert_eq!(eval("new Image().tagName").await, "IMG");
}

// ================================================================
// DOMRect
// ================================================================

#[tokio::test]
async fn dom_rect_class() {
    assert_eq!(eval("typeof DOMRect").await, "function");
    assert_eq!(eval("new DOMRect(10, 20, 100, 50).right").await, "110");
    assert_eq!(eval("new DOMRect(10, 20, 100, 50).bottom").await, "70");
}

// ================================================================
// Selection / getSelection
// ================================================================

#[tokio::test]
async fn get_selection_exists() {
    assert_eq!(eval("typeof getSelection").await, "function");
    assert_eq!(eval("typeof getSelection()").await, "object");
    assert_eq!(eval("getSelection().rangeCount").await, "0");
}

// ================================================================
// Document properties
// ================================================================

#[tokio::test]
async fn document_properties() {
    // Phase 7 — HTML legacy default per spec is windows-1252.
    assert_eq!(eval("document.characterSet").await, "windows-1252");
    assert_eq!(eval("document.contentType").await, "text/html");
    assert_eq!(eval("document.compatMode").await, "CSS1Compat");
    assert_eq!(eval("document.defaultView === window").await, "true");
    assert_eq!(
        eval("document.activeElement === document.body").await,
        "true"
    );
}

#[tokio::test]
async fn document_create_event() {
    assert_eq!(eval("typeof document.createEvent").await, "function");
}

#[tokio::test]
async fn document_exec_command() {
    assert_eq!(eval("typeof document.execCommand").await, "function");
    assert_eq!(eval("document.execCommand('bold')").await, "false");
}

// ================================================================
// customElements
// ================================================================

#[tokio::test]
async fn custom_elements_exists() {
    assert_eq!(eval("typeof customElements").await, "object");
    assert_eq!(eval("typeof customElements.define").await, "function");
    assert_eq!(eval("typeof customElements.get").await, "function");
}

// ================================================================
// Window stubs
// ================================================================

#[tokio::test]
async fn window_dialog_stubs() {
    assert_eq!(eval("typeof alert").await, "function");
    assert_eq!(eval("typeof confirm").await, "function");
    assert_eq!(eval("typeof prompt").await, "function");
    assert_eq!(eval("typeof open").await, "function");
}

// ================================================================
// HTMLElement subtypes exist
// ================================================================

#[tokio::test]
async fn html_element_types_exist() {
    assert_eq!(eval("typeof HTMLElement").await, "function");
    assert_eq!(eval("typeof HTMLDivElement").await, "function");
    assert_eq!(eval("typeof HTMLInputElement").await, "function");
    assert_eq!(eval("typeof HTMLFormElement").await, "function");
    assert_eq!(eval("typeof HTMLAnchorElement").await, "function");
    assert_eq!(eval("typeof HTMLImageElement").await, "function");
    assert_eq!(eval("typeof HTMLCanvasElement").await, "function");
    assert_eq!(eval("typeof HTMLVideoElement").await, "function");
    assert_eq!(eval("typeof SVGElement").await, "function");
}

// ================================================================
// Blob / File
// ================================================================

#[tokio::test]
async fn blob_exists() {
    assert_eq!(eval("typeof Blob").await, "function");
    assert_eq!(eval("new Blob(['hello']).size").await, "5");
}

#[tokio::test]
async fn file_exists() {
    assert_eq!(eval("typeof File").await, "function");
    assert_eq!(eval("new File(['x'], 'test.txt').name").await, "test.txt");
}

// §6.6 item 9 — MediaDevices.enumerateDevices pre-permission behavior.
// Must: (a) expose devices, (b) use WebIDL camelCase keys, (c) empty all
// labels before camera/microphone permission is granted.
#[tokio::test]
async fn test_media_devices_pre_permission() {
    // Phase 7 — mediaDevices is [SecureContext]; load over https.
    let profile = stealth::presets::chrome_130_windows();
    let expected_count = profile.media_devices.len();
    let mut page = Page::with_profile("", "https://example.com/", profile)
        .await
        .unwrap();

    let probe = r#"
        (async () => {
            const list = await navigator.mediaDevices.enumerateDevices();
            globalThis.__mediaResult = JSON.stringify({
                count: list.length,
                // WebIDL shape: deviceId, groupId (camelCase).
                keys0: list[0] ? Object.keys(list[0]).sort() : [],
                labels: list.map(d => d.label),
                kinds: list.map(d => d.kind).sort(),
                hasDeviceIds: list.every(d => typeof d.deviceId === 'string' && d.deviceId.length > 0),
                hasGroupIds: list.every(d => typeof d.groupId === 'string' && d.groupId.length > 0),
                // Chrome exposes NO snake_case keys.
                anySnakeCase: list.some(d => 'device_id' in d || 'group_id' in d),
            });
        })()
    "#;
    page.evaluate(probe).unwrap();
    page.evaluate_async("void 0", Duration::from_millis(100))
        .await
        .ok();
    let raw = page.evaluate("globalThis.__mediaResult").unwrap();
    let obj: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|_| panic!("media probe result was not JSON: {}", raw));

    assert_eq!(obj["count"].as_u64().unwrap() as usize, expected_count);
    assert_eq!(
        obj["anySnakeCase"], false,
        "snake_case leaks WebIDL mismatch"
    );
    // Phase 6 D2 — pre-permission, deviceId and groupId are blanked
    // (empty strings) per spec. Labels also empty (asserted below).
    assert_eq!(obj["hasDeviceIds"], false);
    assert_eq!(obj["hasGroupIds"], false);
    assert_eq!(
        obj["keys0"].as_array().unwrap(),
        &serde_json::json!(["deviceId", "groupId", "kind", "label"])
            .as_array()
            .unwrap()
            .clone()
    );
    // All labels empty pre-permission — the core assertion.
    for l in obj["labels"].as_array().unwrap() {
        assert_eq!(
            l.as_str().unwrap(),
            "",
            "pre-permission label leak is a classic automation tell"
        );
    }
}
