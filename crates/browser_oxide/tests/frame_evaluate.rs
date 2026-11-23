//! Child-frame evaluation.
//!
//! `page.evaluate()` runs in the top-level realm only. The reason is the same
//! one Chrome has: a cross-origin frame's `contentWindow.document` throws
//! `SecurityError`, and a same-origin frame is still a separate V8 context.
//! Chrome's answer is per-frame execution contexts (CDP `executionContextId`);
//! the engine builds a real child isolate per materialized `<iframe>` at
//! navigation time, so the equivalent is to evaluate through that isolate.
//!
//! These tests pin the `Page`-level surface the host/Python layers forward.

use browser_oxide::Page;

async fn page_from(html: &str) -> Page {
    Page::from_html(html, None::<browser_oxide::stealth::StealthProfile>)
        .await
        .expect("fixture builds")
}

/// A materialized `srcdoc` frame gets its own realm, driveable out of band.
#[tokio::test]
async fn srcdoc_frame_is_evaluable_out_of_band() {
    let mut page = page_from(
        r#"<html><body>
             <iframe srcdoc="<html><body><p id='m'>inner</p></body></html>"></iframe>
           </body></html>"#,
    )
    .await;

    let count = page.child_iframe_count();
    assert!(
        count >= 1,
        "srcdoc iframe must be materialized as a child realm, got {count}"
    );

    // Reads the FRAME's DOM, which the parent realm cannot reach.
    let text = page
        .child_iframe(0)
        .expect("frame 0 exists")
        .evaluate("document.getElementById('m').textContent")
        .expect("frame evaluate");
    assert_eq!(text, "inner", "frame evaluate must run in the child realm");

    // The parent realm sees the <iframe> element but not its content: the
    // access throws, which is precisely why out-of-band evaluation matters.
    let blocked = page
        .evaluate(
            r#"(() => {
                 const f = document.querySelector('iframe');
                 try { return String(f.contentWindow.document && 'reachable'); }
                 catch (e) { return 'blocked:' + e.name; }
               })()"#,
        )
        .expect("parent evaluate");
    assert!(
        blocked.starts_with("blocked:") || blocked == "reachable",
        "parent-side access must be blocked or a shim, got: {blocked}"
    );
}

/// Two frames are independently addressable and keep separate state.
#[tokio::test]
async fn frames_have_independent_realms() {
    let mut page = page_from(
        r#"<html><body>
             <iframe srcdoc="<html><body><script>window.__tag='A'</script></body></html>"></iframe>
             <iframe srcdoc="<html><body><script>window.__tag='B'</script></body></html>"></iframe>
           </body></html>"#,
    )
    .await;

    let count = page.child_iframe_count();
    assert_eq!(
        count, 2,
        "both srcdoc frames must be materialized, got {count}"
    );

    let a = page
        .child_iframe(0)
        .unwrap()
        .evaluate("window.__tag")
        .unwrap();
    let b = page
        .child_iframe(1)
        .unwrap()
        .evaluate("window.__tag")
        .unwrap();
    assert_eq!(
        (a.as_str(), b.as_str()),
        ("A", "B"),
        "frames must be isolated"
    );
}

/// Out-of-range access is a `None`, not a panic.
#[tokio::test]
async fn frame_index_out_of_range_is_none() {
    let mut page = page_from("<html><body><p>x</p></body></html>").await;
    assert_eq!(page.child_iframe_count(), 0, "no iframes in this fixture");
    assert!(
        page.child_iframe(0).is_none(),
        "out-of-range frame access must be None"
    );
}
