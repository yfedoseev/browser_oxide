//! Iframe V8 isolation tests — verify separate JS context per iframe.

use browser::Page;
use stealth;

fn html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head></head><body>{}</body></html>",
        body
    )
}

#[tokio::test]
async fn iframe_srcdoc_creates_child() {
    let page = Page::from_html(
        r#"<!DOCTYPE html><html><body>
        <iframe srcdoc="<html><body><p>hello from iframe</p></body></html>"></iframe>
    </body></html>"#,
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(page.child_iframe_count(), 1, "should have 1 child iframe");
}

#[tokio::test]
async fn iframe_srcdoc_has_isolated_globals() {
    let mut page = Page::from_html(r#"<!DOCTYPE html><html><body>
        <script>globalThis.parentVar = 42;</script>
        <iframe srcdoc="<html><body><script>globalThis.childVar = 99;</script></body></html>"></iframe>
    </body></html>"#, None::<stealth::StealthProfile>).await.unwrap();
    // Parent sees its own var
    assert_eq!(page.evaluate("parentVar").unwrap(), "42");
    // Parent does NOT see child's var (isolated context)
    assert_eq!(page.evaluate("typeof childVar").unwrap(), "undefined");
    // Child sees its own var
    assert_eq!(
        page.child_iframe(0).unwrap().evaluate("childVar").unwrap(),
        "99"
    );
    // Child does NOT see parent's var
    assert_eq!(
        page.child_iframe(0)
            .unwrap()
            .evaluate("typeof parentVar")
            .unwrap(),
        "undefined"
    );
}

#[tokio::test]
async fn iframe_child_has_own_document() {
    let mut page = Page::from_html(
        r#"<!DOCTYPE html><html><body>
        <p id="parent-p">parent content</p>
        <iframe srcdoc="<html><body><p id='child-p'>child content</p></body></html>"></iframe>
    </body></html>"#,
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    // Parent sees its own DOM
    assert_eq!(
        page.evaluate("document.getElementById('parent-p').textContent")
            .unwrap(),
        "parent content"
    );
    // Parent doesn't see child's DOM
    assert_eq!(
        page.evaluate("document.getElementById('child-p')").unwrap(),
        "null"
    );
    // Child sees its own DOM via query_text
    assert_eq!(
        page.child_iframe(0)
            .unwrap()
            .query_text("#child-p")
            .unwrap(),
        "child content"
    );
}

#[tokio::test]
async fn iframe_srcdoc_executes_scripts() {
    let mut page = Page::from_html(r#"<!DOCTYPE html><html><body>
        <iframe srcdoc="<html><body><div id='target'>before</div><script>document.getElementById('target').textContent = 'after';</script></body></html>"></iframe>
    </body></html>"#, None::<stealth::StealthProfile>).await.unwrap();
    assert_eq!(
        page.child_iframe(0).unwrap().query_text("#target").unwrap(),
        "after"
    );
}

#[tokio::test]
async fn multiple_iframes_isolated() {
    let mut page = Page::from_html(
        r#"<!DOCTYPE html><html><body>
        <iframe srcdoc="<script>globalThis.x = 'iframe1';</script>"></iframe>
        <iframe srcdoc="<script>globalThis.x = 'iframe2';</script>"></iframe>
    </body></html>"#,
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(page.child_iframe_count(), 2);
    assert_eq!(
        page.child_iframe(0).unwrap().evaluate("x").unwrap(),
        "iframe1"
    );
    assert_eq!(
        page.child_iframe(1).unwrap().evaluate("x").unwrap(),
        "iframe2"
    );
}
