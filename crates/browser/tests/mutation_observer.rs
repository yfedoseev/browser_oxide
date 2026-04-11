//! MutationObserver tests — real callbacks on DOM mutations.

use browser::Page;
use std::time::Duration;

fn html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head></head><body>{}</body></html>",
        body
    )
}

#[tokio::test]
async fn child_list_append() {
    let mut page = Page::from_html(&html(r#"<div id="target"></div>"#))
        .await
        .unwrap();
    page.evaluate(
        r#"
        globalThis.records = [];
        const observer = new MutationObserver((mutations) => {
            for (const m of mutations) {
                records.push({ type: m.type, addedCount: m.addedNodes.length });
            }
        });
        observer.observe(document.getElementById('target'), { childList: true });
    "#,
    )
    .unwrap();
    page.evaluate(
        r#"
        const el = document.createElement('span');
        document.getElementById('target').appendChild(el);
    "#,
    )
    .unwrap();
    // Give microtask queue time to fire
    page.evaluate_async("void 0", Duration::from_millis(50))
        .await
        .ok();
    let result = page.evaluate("JSON.stringify(records)").unwrap();
    assert!(
        result.contains("childList"),
        "should have childList record: {}",
        result
    );
    assert!(
        result.contains("addedCount"),
        "should have addedNodes: {}",
        result
    );
}

#[tokio::test]
async fn child_list_remove() {
    let mut page = Page::from_html(&html(r#"<div id="target"><span id="child"></span></div>"#))
        .await
        .unwrap();
    page.evaluate(
        r#"
        globalThis.records = [];
        const observer = new MutationObserver((mutations) => {
            for (const m of mutations) {
                records.push({ type: m.type, removedCount: m.removedNodes.length });
            }
        });
        observer.observe(document.getElementById('target'), { childList: true });
    "#,
    )
    .unwrap();
    page.evaluate("document.getElementById('child').remove()")
        .unwrap();
    page.evaluate_async("void 0", Duration::from_millis(50))
        .await
        .ok();
    let result = page.evaluate("JSON.stringify(records)").unwrap();
    assert!(
        result.contains("childList"),
        "should detect removal: {}",
        result
    );
}

#[tokio::test]
async fn attributes_mutation() {
    let mut page = Page::from_html(&html(r#"<div id="target"></div>"#))
        .await
        .unwrap();
    page.evaluate(
        r#"
        globalThis.records = [];
        const observer = new MutationObserver((mutations) => {
            for (const m of mutations) {
                records.push({ type: m.type, attr: m.attributeName });
            }
        });
        observer.observe(document.getElementById('target'), { attributes: true });
    "#,
    )
    .unwrap();
    page.evaluate("document.getElementById('target').setAttribute('data-x', '1')")
        .unwrap();
    page.evaluate_async("void 0", Duration::from_millis(50))
        .await
        .ok();
    let result = page.evaluate("JSON.stringify(records)").unwrap();
    assert!(
        result.contains("attributes"),
        "should detect attribute change: {}",
        result
    );
    assert!(
        result.contains("data-x"),
        "should have attributeName: {}",
        result
    );
}

#[tokio::test]
async fn disconnect_stops_callbacks() {
    let mut page = Page::from_html(&html(r#"<div id="target"></div>"#))
        .await
        .unwrap();
    page.evaluate(
        r#"
        globalThis.count = 0;
        const observer = new MutationObserver(() => { count++; });
        observer.observe(document.getElementById('target'), { childList: true });
        document.getElementById('target').appendChild(document.createElement('span'));
    "#,
    )
    .unwrap();
    page.evaluate_async("void 0", Duration::from_millis(50))
        .await
        .ok();
    page.evaluate(
        r#"
        observer.disconnect();
        document.getElementById('target').appendChild(document.createElement('span'));
    "#,
    )
    .unwrap();
    page.evaluate_async("void 0", Duration::from_millis(50))
        .await
        .ok();
    // Should have fired once before disconnect, not after
    let count = page.evaluate("count").unwrap();
    assert_eq!(count, "1", "should stop after disconnect, got {}", count);
}

#[tokio::test]
async fn take_records() {
    let mut page = Page::from_html(&html(r#"<div id="target"></div>"#))
        .await
        .unwrap();
    page.evaluate(
        r#"
        globalThis.observer = new MutationObserver(() => {});
        observer.observe(document.getElementById('target'), { childList: true });
        document.getElementById('target').appendChild(document.createElement('span'));
    "#,
    )
    .unwrap();
    page.evaluate_async("void 0", Duration::from_millis(50))
        .await
        .ok();
    let records = page
        .evaluate("JSON.stringify(observer.takeRecords())")
        .unwrap();
    // takeRecords returns pending records and clears queue
    assert_eq!(page.evaluate("observer.takeRecords().length").unwrap(), "0");
}
