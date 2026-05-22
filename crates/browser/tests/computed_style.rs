//! Tests for getComputedStyle wired to actual DOM inline styles.

use browser::Page;

fn html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head></head><body>{}</body></html>",
        body
    )
}

#[tokio::test]
async fn inline_style_color() {
    let mut page = Page::from_html(
        &html(r#"<div id="el" style="color: red"></div>"#),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let val = page
        .evaluate("getComputedStyle(document.getElementById('el')).color")
        .unwrap();
    assert_eq!(val, "red", "should return inline style color");
}

#[tokio::test]
async fn inline_style_font_size() {
    let mut page = Page::from_html(
        &html(r#"<div id="el" style="font-size: 20px"></div>"#),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let val = page
        .evaluate("getComputedStyle(document.getElementById('el')).fontSize")
        .unwrap();
    assert_eq!(val, "20px");
}

#[tokio::test]
async fn inline_style_multiple_properties() {
    let mut page = Page::from_html(
        &html(r#"<div id="el" style="color: blue; opacity: 0.5; display: flex"></div>"#),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).color")
            .unwrap(),
        "blue"
    );
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).opacity")
            .unwrap(),
        "0.5"
    );
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).display")
            .unwrap(),
        "flex"
    );
}

#[tokio::test]
async fn no_inline_style_returns_default() {
    let mut page = Page::from_html(
        &html(r#"<div id="el"></div>"#),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    // No inline style — should return CSS defaults
    let display = page
        .evaluate("getComputedStyle(document.getElementById('el')).display")
        .unwrap();
    assert_eq!(display, "block");
    let vis = page
        .evaluate("getComputedStyle(document.getElementById('el')).visibility")
        .unwrap();
    assert_eq!(vis, "visible");
}

#[tokio::test]
async fn get_property_value_method() {
    let mut page = Page::from_html(
        &html(r#"<div id="el" style="margin-top: 10px"></div>"#),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    let val = page
        .evaluate("getComputedStyle(document.getElementById('el')).getPropertyValue('margin-top')")
        .unwrap();
    assert_eq!(val, "10px");
}

#[tokio::test]
async fn js_style_mutation_reflected() {
    let mut page = Page::from_html(
        &html(r#"<div id="el"></div>"#),
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate("document.getElementById('el').style.backgroundColor = 'green'")
        .unwrap();
    let val = page
        .evaluate("getComputedStyle(document.getElementById('el')).backgroundColor")
        .unwrap();
    assert_eq!(val, "green");
}

// --- Style block tests (these need CSS cascade wiring) ---

#[tokio::test]
async fn style_block_color() {
    let html = r#"<!DOCTYPE html><html><head><style>.red { color: red; }</style></head>
    <body><div id="el" class="red"></div></body></html>"#;
    let mut page = Page::from_html(html, None::<stealth::StealthProfile>)
        .await
        .unwrap();
    let val = page
        .evaluate("getComputedStyle(document.getElementById('el')).color")
        .unwrap();
    assert_eq!(val, "red");
}

#[tokio::test]
async fn style_block_specificity_id_beats_class() {
    let html = r#"<!DOCTYPE html><html><head><style>
        .blue { color: blue; }
        #el { color: green; }
    </style></head>
    <body><div id="el" class="blue"></div></body></html>"#;
    let mut page = Page::from_html(html, None::<stealth::StealthProfile>)
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).color")
            .unwrap(),
        "green"
    );
}

#[tokio::test]
async fn inline_style_beats_style_block() {
    let html = r#"<!DOCTYPE html><html><head><style>#el { color: blue; }</style></head>
    <body><div id="el" style="color: red"></div></body></html>"#;
    let mut page = Page::from_html(html, None::<stealth::StealthProfile>)
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).color")
            .unwrap(),
        "red"
    );
}

#[tokio::test]
async fn style_block_font_size() {
    let html = r#"<!DOCTYPE html><html><head><style>
        .big { font-size: 24px; }
    </style></head>
    <body><div id="el" class="big"></div></body></html>"#;
    let mut page = Page::from_html(html, None::<stealth::StealthProfile>)
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).fontSize")
            .unwrap(),
        "24px"
    );
}

#[tokio::test]
async fn multiple_rules_last_wins_same_specificity() {
    let html = r#"<!DOCTYPE html><html><head><style>
        div { color: red; }
        div { color: blue; }
    </style></head>
    <body><div id="el"></div></body></html>"#;
    let mut page = Page::from_html(html, None::<stealth::StealthProfile>)
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("getComputedStyle(document.getElementById('el')).color")
            .unwrap(),
        "blue"
    );
}
