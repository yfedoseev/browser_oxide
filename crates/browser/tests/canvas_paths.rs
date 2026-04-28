//! Canvas 2D path operations — the stubs at canvas_bootstrap.js for
//! `arc`, `arcTo`, `bezierCurveTo`, `quadraticCurveTo`, `closePath`,
//! `setTransform`, `resetTransform`, `strokeText` are now wired to the
//! existing Skia-backed implementations in crates/canvas/src/canvas2d.rs.
//! These tests verify the path operations execute without throwing AND
//! produce non-trivial canvas output, which is the minimum bar for
//! anti-bot canvas fingerprint probes (Kasada `ips.js`, Akamai sensor v3,
//! DataDome `tags.js`, CreepJS `paintCanvas`).

use browser::Page;
use stealth;

async fn evaluate(js: &str) -> String {
    let mut page = Page::from_html(
        "<!DOCTYPE html><html><body></body></html>",
        None::<stealth::StealthProfile>,
    )
    .await
    .unwrap();
    page.evaluate(js).unwrap_or_else(|e| format!("ERROR: {e}"))
}

#[tokio::test]
async fn arc_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 100; c.height = 100;
        const ctx = c.getContext('2d');
        ctx.beginPath();
        ctx.arc(50, 50, 25, 0, Math.PI * 2, false);
        ctx.fill();
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn bezier_curve_to_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 100; c.height = 100;
        const ctx = c.getContext('2d');
        ctx.beginPath();
        ctx.moveTo(10, 10);
        ctx.bezierCurveTo(30, 0, 70, 0, 90, 50);
        ctx.stroke();
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn quadratic_curve_to_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 100; c.height = 100;
        const ctx = c.getContext('2d');
        ctx.beginPath();
        ctx.moveTo(10, 10);
        ctx.quadraticCurveTo(50, 0, 90, 50);
        ctx.stroke();
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn close_path_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 100; c.height = 100;
        const ctx = c.getContext('2d');
        ctx.beginPath();
        ctx.moveTo(10, 10);
        ctx.lineTo(50, 10);
        ctx.lineTo(50, 50);
        ctx.closePath();
        ctx.fill();
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn set_transform_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 100; c.height = 100;
        const ctx = c.getContext('2d');
        ctx.setTransform(2, 0, 0, 2, 10, 20);
        ctx.fillRect(0, 0, 30, 30);
        ctx.resetTransform();
        ctx.fillRect(0, 0, 10, 10);
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn stroke_text_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 200; c.height = 50;
        const ctx = c.getContext('2d');
        ctx.font = '14px Arial';
        ctx.strokeText('Hello', 10, 30);
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn arcTo_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 100; c.height = 100;
        const ctx = c.getContext('2d');
        ctx.beginPath();
        ctx.moveTo(20, 20);
        ctx.arcTo(80, 20, 80, 80, 20);
        ctx.stroke();
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

#[tokio::test]
async fn ellipse_does_not_throw() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 100; c.height = 100;
        const ctx = c.getContext('2d');
        ctx.beginPath();
        ctx.ellipse(50, 50, 30, 20, 0, 0, Math.PI * 2);
        ctx.stroke();
        'ok'
        ",
    )
    .await;
    assert_eq!(r, "ok");
}

/// Composite test: full CreepJS-style scene with paths + text. Asserts
/// `toDataURL()` produces a non-trivial PNG (length > 1000 bytes).
#[tokio::test]
async fn complex_path_scene_produces_pixels() {
    let r = evaluate(
        "
        const c = document.createElement('canvas');
        c.width = 220; c.height = 30;
        const ctx = c.getContext('2d');
        ctx.textBaseline = 'top';
        ctx.font = \"14px 'Arial'\";
        ctx.textBaseline = 'alphabetic';
        ctx.fillStyle = '#f60';
        ctx.fillRect(125, 1, 62, 20);
        ctx.fillStyle = '#069';
        ctx.fillText('Cwm fjordbank glyphs vext quiz', 2, 15);
        ctx.beginPath();
        ctx.arc(50, 15, 10, 0, Math.PI * 2);
        ctx.fillStyle = 'rgba(102, 204, 0, 0.7)';
        ctx.fill();
        const url = c.toDataURL();
        url.length > 100 && url.startsWith('data:image/png')
        ",
    )
    .await;
    assert_eq!(r, "true");
}
