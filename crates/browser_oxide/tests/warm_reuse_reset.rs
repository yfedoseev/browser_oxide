//! Regression tests for issue #33 — `PagePool` / warm reuse leaked V8 heap.
//!
//! The engine's cross-navigation reapers were all wired to `Page::drop`, which
//! a pool by definition never reaches, and several bootstrap-JS registries are
//! scoped to the `JsRuntime` rather than to the document. Reusing a `Page`
//! therefore retained the previous page's listeners, DOM wrappers, custom
//! elements and window properties — ~10 MB of *live* heap per navigation,
//! without ceiling.
//!
//! These tests assert the observable consequences of that retention are gone
//! after `Page::reset_for_reuse()`. They deliberately do NOT measure heap size:
//! `used_heap_size` is GC-timing dependent and would be flaky. Retention is
//! instead proven by reachability — a listener that still fires, or a global
//! that still resolves, is a listener/global that is still retained.

use browser_oxide::stealth::StealthProfile;
use browser_oxide::Page;

const BLANK: &str = "<html><head></head><body></body></html>";

/// Listeners bound to `window` were keyed in a `WeakMap` against the one
/// object that is never collected for the life of the isolate, so neither the
/// callback nor anything its closure captured could ever be released.
#[tokio::test]
async fn reset_unbinds_window_listeners() {
    let mut page = Page::from_html(
        r#"<html><body><script>
            window.addEventListener('resize', function () {
                globalThis.__hits = (globalThis.__hits || 0) + 1;
            });
        </script></body></html>"#,
        None::<StealthProfile>,
    )
    .await
    .unwrap();

    // Sanity: the listener is live before the reset, otherwise the assertion
    // below would pass for the wrong reason.
    page.evaluate("window.dispatchEvent(new Event('resize'))")
        .unwrap();
    assert_eq!(
        page.evaluate("String(globalThis.__hits)").unwrap(),
        "1",
        "listener should fire before reset — test is not exercising anything otherwise"
    );

    page.reset_for_reuse();
    page.reload_html(BLANK, "about:blank");

    page.evaluate("window.dispatchEvent(new Event('resize'))")
        .unwrap();
    assert_eq!(
        page.evaluate("typeof globalThis.__hits").unwrap(),
        "undefined",
        "previous page's window listener still fired after reset"
    );
}

/// `_nodeListeners` is a strong `Map` keyed by `nodeId`, and node IDs restart
/// at zero when `replace_dom` swaps the document. So this was both a leak and
/// a correctness bug: the old page's handler for node 42 fired on the *new*
/// page's node 42.
#[tokio::test]
async fn reset_unbinds_node_listeners_across_documents() {
    let doc = r#"<html><body><div id="a"></div></body></html>"#;
    let mut page = Page::from_html(
        r#"<html><body><div id="a"></div><script>
            document.querySelector('#a').addEventListener('click', function () {
                globalThis.__hits = (globalThis.__hits || 0) + 1;
            });
        </script></body></html>"#,
        None::<StealthProfile>,
    )
    .await
    .unwrap();

    page.evaluate("document.querySelector('#a').dispatchEvent(new Event('click'))")
        .unwrap();
    assert_eq!(
        page.evaluate("String(globalThis.__hits)").unwrap(),
        "1",
        "listener should fire before reset"
    );

    page.reset_for_reuse();
    page.reload_html(doc, "about:blank");

    // Same markup ⇒ the new `#a` gets the same nodeId the old one had.
    page.evaluate("document.querySelector('#a').dispatchEvent(new Event('click'))")
        .unwrap();
    assert_eq!(
        page.evaluate("typeof globalThis.__hits").unwrap(),
        "undefined",
        "previous document's node listener fired on the new document's node"
    );
}

/// Properties page scripts hang off `window` outlived the navigation, because
/// `globalThis` is the same object for the life of the `JsRuntime`. A real
/// browser gives each navigation a fresh global.
#[tokio::test]
async fn reset_clears_page_authored_globals() {
    let mut page = Page::from_html(
        r#"<html><body><script>
            window.__appState = { rows: new Array(1000).fill('x') };
            window.onscroll = function () {};
            globalThis.__singleton = { self: null };
            globalThis.__singleton.self = globalThis.__singleton;
        </script></body></html>"#,
        None::<StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(page.evaluate("typeof window.__appState").unwrap(), "object");

    page.reset_for_reuse();
    page.reload_html(BLANK, "about:blank");

    assert_eq!(
        page.evaluate("typeof window.__appState").unwrap(),
        "undefined",
        "page-authored global survived reset"
    );
    assert_eq!(
        page.evaluate("typeof globalThis.__singleton").unwrap(),
        "undefined",
        "page-authored global survived reset"
    );
    assert_eq!(
        page.evaluate("String(window.onscroll)").unwrap(),
        "null",
        "page-authored on* handler survived reset"
    );
}

/// `on*` handlers are a distinct case from ordinary page globals: `onscroll`,
/// `onclick` and friends already exist as own properties at bootstrap, so a
/// page assigning to one mutates a *baseline* key rather than adding one. The
/// key-set diff cannot see that, so the value is swept separately. `document`
/// needs the same treatment — it is a singleton that survives `replace_dom`.
#[tokio::test]
async fn reset_clears_document_on_handlers() {
    let mut page = Page::from_html(
        r#"<html><body><script>
            document.onclick = function () { globalThis.__hits = 1; };
        </script></body></html>"#,
        None::<StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(
        page.evaluate("typeof document.onclick").unwrap(),
        "function",
        "handler should be set before reset"
    );

    page.reset_for_reuse();
    page.reload_html(BLANK, "about:blank");

    assert_ne!(
        page.evaluate("typeof document.onclick").unwrap(),
        "function",
        "page-authored document.on* handler survived reset"
    );
}

/// The `on*` sweep must not be a blanket wipe: the engine installs
/// `window.onerror` as its script-error instrumentation exactly once, on the
/// cold build, and never re-installs it on the warm path. Nulling it would
/// silently disable error capture for every pooled page after the first.
#[tokio::test]
async fn reset_preserves_engine_installed_on_handlers() {
    let mut page = Page::from_html(BLANK, None::<StealthProfile>)
        .await
        .unwrap();

    // Stand in for the engine's own install, then re-mark the baseline the
    // same way the cold build does after installing its instrumentation.
    page.evaluate(
        "window.onerror = function engineHandler() { return true; }; \
         globalThis.__markGlobalsBaseline(); 'ok'",
    )
    .unwrap();

    // A page then clobbers it with its own handler.
    page.evaluate("window.onerror = function pageHandler() {}; 'ok'")
        .unwrap();

    page.reset_for_reuse();
    page.reload_html(BLANK, "about:blank");

    assert_eq!(
        page.evaluate("window.onerror && window.onerror.name")
            .unwrap(),
        "engineHandler",
        "reset did not restore the engine's own on* handler — either it was \
         nulled (breaking instrumentation) or the page's was left in place"
    );
}

/// Guard against the global sweep being too aggressive: it must diff against
/// the engine's own baseline, not wipe the namespace. If this fails, the
/// bootstrap is being damaged and every later navigation is broken.
#[tokio::test]
async fn reset_preserves_engine_globals() {
    let mut page = Page::from_html(BLANK, None::<StealthProfile>)
        .await
        .unwrap();

    page.reset_for_reuse();
    page.reload_html(BLANK, "about:blank");

    for expr in [
        "typeof document",
        "typeof window",
        "typeof navigator",
        "typeof location",
        "typeof fetch",
        "typeof setTimeout",
        "typeof XMLHttpRequest",
        "typeof MutationObserver",
        "typeof customElements",
        "typeof Event",
    ] {
        let got = page.evaluate(expr).unwrap();
        assert_ne!(
            got, "undefined",
            "reset stripped an engine global: {expr} became undefined"
        );
    }

    // The DOM must still be usable, not merely present.
    page.reload_html(
        r#"<html><body><p id="p">hi</p></body></html>"#,
        "about:blank",
    );
    assert_eq!(
        page.evaluate("document.querySelector('#p').textContent")
            .unwrap(),
        "hi",
        "DOM unusable after reset"
    );
}

/// `_customElementsRegistry` retained every class every previously-loaded
/// document defined. That is a leak, and it silently broke the next page:
/// re-`define()`ing a name the previous page had registered was a no-op, so
/// the new page's element never upgraded.
#[tokio::test]
async fn reset_allows_custom_element_redefinition() {
    let mut page = Page::from_html(
        r#"<html><body><script>
            class First extends HTMLElement {}
            customElements.define('x-widget', First);
            globalThis.__which = 'first';
        </script></body></html>"#,
        None::<StealthProfile>,
    )
    .await
    .unwrap();

    assert_eq!(
        page.evaluate("typeof customElements.get('x-widget')")
            .unwrap(),
        "function"
    );

    page.reset_for_reuse();
    page.reload_html(BLANK, "about:blank");

    assert_eq!(
        page.evaluate("typeof customElements.get('x-widget')")
            .unwrap(),
        "undefined",
        "previous page's custom element definition survived reset"
    );

    // And the name is free again for the next document.
    page.evaluate(
        "class Second extends HTMLElement {}; customElements.define('x-widget', Second); 'ok'",
    )
    .unwrap();
    assert_eq!(
        page.evaluate("customElements.get('x-widget').name")
            .unwrap(),
        "Second",
        "re-defining a name the previous page used did not take effect"
    );
}

/// Timers must still work after a reset — `__cancelAllTimers()` bumps a
/// generation counter, and a bug there would silently kill every timer on
/// every pooled page after the first.
#[tokio::test]
async fn reset_leaves_timers_functional() {
    let mut page = Page::from_html(BLANK, None::<StealthProfile>)
        .await
        .unwrap();

    page.reset_for_reuse();
    page.reload_html(BLANK, "about:blank");

    page.evaluate_async(
        "globalThis.__fired = false; setTimeout(() => { globalThis.__fired = true; }, 0); 'ok'",
        std::time::Duration::from_millis(500),
    )
    .await
    .ok();

    assert_eq!(
        page.evaluate("String(globalThis.__fired)").unwrap(),
        "true",
        "setTimeout stopped firing after a reset"
    );
}

/// Repeated reuse must stay clean — a reset that only works once would leave
/// the pool leaking from the second navigation onward.
#[tokio::test]
async fn reset_is_idempotent_across_many_reuses() {
    let mut page = Page::from_html(BLANK, None::<StealthProfile>)
        .await
        .unwrap();

    for i in 0..10 {
        page.evaluate(&format!(
            "window.__leak{i} = new Array(100).fill('x'); \
             window.addEventListener('resize', function () {{ globalThis.__hits = 1; }}); 'ok'"
        ))
        .unwrap();

        page.reset_for_reuse();
        page.reload_html(BLANK, "about:blank");

        assert_eq!(
            page.evaluate(&format!("typeof window.__leak{i}")).unwrap(),
            "undefined",
            "global from reuse #{i} survived"
        );
        page.evaluate("window.dispatchEvent(new Event('resize'))")
            .unwrap();
        assert_eq!(
            page.evaluate("typeof globalThis.__hits").unwrap(),
            "undefined",
            "listener from reuse #{i} survived"
        );
    }
}
