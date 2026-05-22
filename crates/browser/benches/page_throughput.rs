//! Throughput benchmarks for the Page API.
//!
//! These numbers are what downstream consumers (e.g. scraper_oxide) see when
//! they pipe pre-fetched HTML through a pooled V8 isolate. The goal is to
//! isolate each step of the fast path so regressions are easy to attribute:
//!
//! - `page_from_html_cold`   — first Page creation (worst-case, includes V8 isolate bootstrap)
//! - `reload_html_static`    — reloading a script-free page into a warm isolate
//! - `reload_html_js`        — reloading a page with inline JS that mutates the DOM
//! - `content_roundtrip`     — serializing the current DOM back to HTML
//! - `evaluate_async_idle`   — draining the event loop when there is nothing to do
//!
//! Run with `cargo bench -p browser --bench page_throughput`.

use browser::Page;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use std::time::Duration;
use stealth::presets::chrome_148_windows;
use tokio::runtime::Builder;

fn static_html() -> &'static str {
    r#"<!DOCTYPE html>
<html><head><title>Static</title></head>
<body>
  <h1>Hello</h1>
  <article>
    <p>One. Lorem ipsum dolor sit amet.</p>
    <p>Two. Consectetur adipiscing elit.</p>
    <p>Three. Sed do eiusmod tempor.</p>
  </article>
  <nav>
    <a href="/a">A</a><a href="/b">B</a><a href="/c">C</a>
  </nav>
</body></html>"#
}

fn js_html() -> &'static str {
    r#"<!DOCTYPE html>
<html><head><title>JS</title></head>
<body>
  <h1 id="t">placeholder</h1>
  <article id="c">loading...</article>
  <script>
    document.getElementById('t').textContent = 'Hello';
    document.getElementById('c').textContent = 'Rendered via JS';
  </script>
</body></html>"#
}

/// Build a `current_thread` runtime with a LocalSet. V8 isolates are `!Send`,
/// so everything that touches a Page must live on a single thread.
fn rt() -> tokio::runtime::Runtime {
    Builder::new_current_thread().enable_all().build().unwrap()
}

fn bench_from_html_cold(c: &mut Criterion) {
    c.bench_function("page_from_html_cold", |b| {
        b.iter_batched(
            rt,
            |rt| {
                rt.block_on(async {
                    Page::from_html(static_html(), Some(chrome_148_windows()))
                        .await
                        .unwrap()
                });
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_reload_static(c: &mut Criterion) {
    let rt = rt();
    let mut page = rt.block_on(async {
        Page::from_html_fast("<html></html>", "about:blank", chrome_148_windows())
            .await
            .unwrap()
    });

    c.bench_function("reload_html_static", |b| {
        b.iter(|| {
            page.reload_html(static_html(), "http://bench/page");
        });
    });
}

fn bench_reload_js(c: &mut Criterion) {
    let rt = rt();
    let mut page = rt.block_on(async {
        Page::from_html_fast("<html></html>", "about:blank", chrome_148_windows())
            .await
            .unwrap()
    });

    c.bench_function("reload_html_js", |b| {
        b.iter(|| {
            page.reload_html(js_html(), "http://bench/page");
        });
    });
}

fn bench_content_roundtrip(c: &mut Criterion) {
    let rt = rt();
    let mut page = rt.block_on(async {
        Page::from_html(static_html(), Some(chrome_148_windows()))
            .await
            .unwrap()
    });

    c.bench_function("content_roundtrip", |b| {
        b.iter(|| {
            let s = page.content();
            criterion::black_box(s);
        });
    });
}

fn bench_evaluate_async_idle(c: &mut Criterion) {
    let rt = rt();
    let mut page = rt.block_on(async {
        Page::from_html(static_html(), Some(chrome_148_windows()))
            .await
            .unwrap()
    });

    c.bench_function("evaluate_async_idle", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = page.evaluate_async("", Duration::from_secs(1)).await;
            });
        });
    });
}

/// End-to-end "pooled" path: reuse one Page, reload new HTML, drain loop,
/// serialize. Mirrors what scraper_oxide's native worker does per fetch.
fn bench_pooled_render_js(c: &mut Criterion) {
    let rt = rt();
    let mut page = rt.block_on(async {
        Page::from_html_fast("<html></html>", "about:blank", chrome_148_windows())
            .await
            .unwrap()
    });

    c.bench_function("pooled_render_js", |b| {
        b.iter(|| {
            rt.block_on(async {
                page.reload_html(js_html(), "http://bench/page");
                let _ = page.evaluate_async("", Duration::from_secs(2)).await;
                let s = page.content();
                criterion::black_box(s);
            });
        });
    });
}

criterion_group!(
    benches,
    bench_from_html_cold,
    bench_reload_static,
    bench_reload_js,
    bench_content_roundtrip,
    bench_evaluate_async_idle,
    bench_pooled_render_js
);
criterion_main!(benches);
