//! Parse probe — builds a Page from a LOCAL HTML file via `from_html_fast`
//! (parse + DOM build + inline scripts, NO external fetch, NO drain) and reports
//! document structure. Isolates "did the parser/DOM-build produce a body?" from
//! "did external scripts wipe it?".
//!
//!   cargo run --release -p browser --example parse_probe -- <file.html>

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: parse_probe <file.html>");
    let html = std::fs::read_to_string(&path).expect("read file");
    let profile = stealth::presets::chrome_148_macos();

    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            println!("== parse_probe {path} (input {} bytes) ==", html.len());

            let mut page = browser::Page::from_html_fast(&html, "https://www.shopify.com/ca", profile)
                .await
                .expect("from_html_fast");

            let diag = r#"
                JSON.stringify({
                    docElLen: document.documentElement ? document.documentElement.outerHTML.length : -1,
                    bodyExists: !!document.body,
                    bodyLen: document.body ? document.body.outerHTML.length : -1,
                    bodyChildren: document.body ? document.body.children.length : -1,
                    headExists: !!document.head,
                    headChildren: document.head ? document.head.children.length : -1,
                    allNodes: document.querySelectorAll('*').length,
                    scriptTags: document.querySelectorAll('script').length,
                    divTags: document.querySelectorAll('div').length,
                    title: document.title,
                    readyState: document.readyState
                })
            "#;
            let out = page.event_loop().execute_script(diag).unwrap_or_else(|e| format!("ERR: {e}"));
            match deno_core::serde_json::from_str::<deno_core::serde_json::Value>(&out) {
                Ok(v) => println!("{}", deno_core::serde_json::to_string_pretty(&v).unwrap()),
                Err(_) => println!("{out}"),
            }
        })
        .await;
}
