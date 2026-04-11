//! Integration tests for Sprint 3 supporting Web APIs.
//!
//! Each test drives a real Page::from_html flow and verifies that the
//! API behaves as Chrome does from a black-box perspective. These
//! tests are the regression gate for Sprint 3 work.

use browser::Page;

// ============================================================================
// A1 — blob: URL fetch
// ============================================================================

#[tokio::test]
async fn fetch_blob_url_returns_text() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const blob = new Blob(['hello blob']);
                const url = URL.createObjectURL(blob);
                const resp = await fetch(url);
                const text = await resp.text();
                document.getElementById('out').textContent =
                    resp.status + '|' + resp.ok + '|' + text;
            })();
        </script></body></html>"#,
    )
    .await
    .expect("page builds");

    let out = page
        .text_of("#out")
        .expect("script should populate #out");
    assert_eq!(out, "200|true|hello blob");
}

#[tokio::test]
async fn fetch_blob_url_preserves_content_type() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const blob = new Blob(['x,y\n1,2'], { type: 'text/csv' });
                const url = URL.createObjectURL(blob);
                const resp = await fetch(url);
                document.getElementById('out').textContent =
                    resp.headers.get('content-type') || 'none';
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("text/csv".to_string()));
}

#[tokio::test]
async fn fetch_blob_url_arraybuffer_preserves_bytes() {
    // Binary fidelity: a blob of raw non-UTF8 bytes should survive
    // round-trip through arrayBuffer() without TextEncoder corruption.
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                // 0x00 0xFF 0x7F 0x80 — includes null, high-bit bytes.
                const bytes = new Uint8Array([0x00, 0xFF, 0x7F, 0x80]);
                const blob = new Blob([bytes], { type: 'application/octet-stream' });
                const url = URL.createObjectURL(blob);
                const resp = await fetch(url);
                const ab = await resp.arrayBuffer();
                const view = new Uint8Array(ab);
                const parts = [];
                for (let i = 0; i < view.length; i++) parts.push(view[i]);
                document.getElementById('out').textContent =
                    view.byteLength + ':' + parts.join(',');
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("4:0,255,127,128".to_string()));
}

#[tokio::test]
async fn fetch_unknown_blob_url_throws() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                try {
                    await fetch('blob:null/fake-nonexistent-uuid-1234');
                    document.getElementById('out').textContent = 'unexpected-ok';
                } catch (e) {
                    document.getElementById('out').textContent = 'threw:' + e.name;
                }
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    // Network error → TypeError per Fetch spec.
    let out = page.text_of("#out").unwrap_or_default();
    assert!(
        out.starts_with("threw:TypeError"),
        "expected TypeError, got {out}"
    );
}

// ============================================================================
// A2 — structuredClone
// ============================================================================

#[tokio::test]
async fn structured_clone_is_global_function() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            document.getElementById('out').textContent = typeof structuredClone;
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("function".to_string()));
}

#[tokio::test]
async fn structured_clone_primitives_and_date() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const d = new Date(1234567890000);
            const clone = structuredClone({
                n: 42,
                s: 'hello',
                b: true,
                nil: null,
                d: d,
                big: 9007199254740993n,
            });
            const parts = [];
            parts.push(clone.n === 42);
            parts.push(clone.s === 'hello');
            parts.push(clone.b === true);
            parts.push(clone.nil === null);
            parts.push(clone.d instanceof Date);
            parts.push(clone.d.getTime() === d.getTime());
            parts.push(clone.d !== d);
            parts.push(clone.big === 9007199254740993n);
            document.getElementById('out').textContent = parts.join(',');
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true,true,true,true,true,true,true,true".to_string())
    );
}

#[tokio::test]
async fn structured_clone_typed_array_survives() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const src = new Uint8Array([1, 2, 3, 255]);
            const clone = structuredClone(src);
            const isUint8 = clone instanceof Uint8Array;
            const sameValues = clone.length === 4 &&
                clone[0] === 1 && clone[3] === 255;
            // Mutating the clone must not affect the source.
            clone[0] = 99;
            const independent = src[0] === 1;
            document.getElementById('out').textContent =
                isUint8 + ',' + sameValues + ',' + independent;
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("true,true,true".to_string()));
}

#[tokio::test]
async fn structured_clone_map_set_cycle() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const m = new Map();
            m.set('a', 1);
            m.set('b', [1, 2, 3]);
            const s = new Set(['x', 'y', 'z']);
            // Cycle: object that references itself through a nested array.
            const cyclic = { name: 'root' };
            cyclic.self = cyclic;
            cyclic.ring = [cyclic];
            const bundle = { m, s, cyclic };
            const clone = structuredClone(bundle);
            const parts = [];
            parts.push(clone.m instanceof Map);
            parts.push(clone.m.get('a') === 1);
            parts.push(JSON.stringify(clone.m.get('b')) === '[1,2,3]');
            parts.push(clone.s instanceof Set);
            parts.push(clone.s.size === 3);
            parts.push(clone.s.has('y'));
            // Cycle preserved: clone.self points back at clone.
            parts.push(clone.cyclic.self === clone.cyclic);
            parts.push(clone.cyclic.ring[0] === clone.cyclic);
            // And NOT at the original.
            parts.push(clone.cyclic !== cyclic);
            document.getElementById('out').textContent = parts.join(',');
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true,true,true,true,true,true,true,true,true".to_string())
    );
}

#[tokio::test]
async fn structured_clone_regexp_preserves_source_and_flags() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const r = /foo.*bar/gim;
            const clone = structuredClone(r);
            document.getElementById('out').textContent =
                (clone instanceof RegExp) + ',' +
                clone.source + ',' + clone.flags;
        </script></body></html>"#,
    )
    .await
    .unwrap();
    let out = page.text_of("#out").unwrap_or_default();
    assert!(out.starts_with("true,foo.*bar,"));
    // Flags should contain g, i, m in some order.
    assert!(out.contains('g') && out.contains('i') && out.contains('m'));
}

#[tokio::test]
async fn structured_clone_function_throws() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            try {
                structuredClone({ fn: function() {} });
                document.getElementById('out').textContent = 'no-throw';
            } catch (e) {
                document.getElementById('out').textContent = 'threw:' + e.name;
            }
        </script></body></html>"#,
    )
    .await
    .unwrap();
    let out = page.text_of("#out").unwrap_or_default();
    assert!(
        out.starts_with("threw:DataCloneError"),
        "expected DataCloneError, got {out}"
    );
}

#[tokio::test]
async fn structured_clone_symbol_throws() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            try {
                structuredClone(Symbol('nope'));
                document.getElementById('out').textContent = 'no-throw';
            } catch (e) {
                document.getElementById('out').textContent = 'threw:' + e.name;
            }
        </script></body></html>"#,
    )
    .await
    .unwrap();
    let out = page.text_of("#out").unwrap_or_default();
    assert!(out.starts_with("threw:DataCloneError"), "got {out}");
}

#[tokio::test]
async fn structured_clone_array_buffer_copy() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const buf = new ArrayBuffer(8);
            new Uint8Array(buf).set([10, 20, 30, 40, 50, 60, 70, 80]);
            const clone = structuredClone(buf);
            const view = new Uint8Array(clone);
            const matches = view[0] === 10 && view[7] === 80;
            // Mutate original via a view — clone must be untouched.
            new Uint8Array(buf)[0] = 0;
            const independent = view[0] === 10;
            document.getElementById('out').textContent =
                (clone instanceof ArrayBuffer) + ',' +
                (clone !== buf) + ',' +
                matches + ',' + independent;
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true,true,true,true".to_string())
    );
}

// ============================================================================
// A4 — Streams (ReadableStream / WritableStream / TransformStream)
// ============================================================================

#[tokio::test]
async fn streams_classes_exist() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            document.getElementById('out').textContent =
                (typeof ReadableStream) + '/' +
                (typeof WritableStream) + '/' +
                (typeof TransformStream);
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("function/function/function".to_string())
    );
}

#[tokio::test]
async fn readable_stream_single_chunk_read() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const s = new ReadableStream({
                    start(c) {
                        c.enqueue('hello');
                        c.close();
                    },
                });
                const r = s.getReader();
                const first = await r.read();
                const second = await r.read();
                document.getElementById('out').textContent =
                    first.done + ':' + first.value + '|' +
                    second.done + ':' + (second.value === undefined);
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("false:hello|true:true".to_string())
    );
}

#[tokio::test]
async fn readable_stream_multi_chunk_pull() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                let n = 0;
                const s = new ReadableStream({
                    pull(c) {
                        if (n >= 3) { c.close(); return; }
                        c.enqueue('chunk-' + n);
                        n++;
                    },
                });
                const r = s.getReader();
                const chunks = [];
                while (true) {
                    const { done, value } = await r.read();
                    if (done) break;
                    chunks.push(value);
                }
                document.getElementById('out').textContent = chunks.join(',');
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("chunk-0,chunk-1,chunk-2".to_string())
    );
}

#[tokio::test]
async fn response_body_is_readable_stream() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const blob = new Blob(['body-via-stream']);
                const url = URL.createObjectURL(blob);
                const resp = await fetch(url);
                const isStream = resp.body instanceof ReadableStream;
                const r = resp.body.getReader();
                const { value, done } = await r.read();
                const decoder = new TextDecoder();
                document.getElementById('out').textContent =
                    isStream + '/' + decoder.decode(value) + '/' + done;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true/body-via-stream/false".to_string())
    );
}

#[tokio::test]
async fn readable_stream_tee_branches_independently() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const s = new ReadableStream({
                    start(c) {
                        c.enqueue('A');
                        c.enqueue('B');
                        c.close();
                    },
                });
                const [a, b] = s.tee();
                const ra = a.getReader();
                const rb = b.getReader();
                const collectA = async () => {
                    const out = [];
                    while (true) {
                        const { done, value } = await ra.read();
                        if (done) break;
                        out.push(value);
                    }
                    return out.join('+');
                };
                const collectB = async () => {
                    const out = [];
                    while (true) {
                        const { done, value } = await rb.read();
                        if (done) break;
                        out.push(value);
                    }
                    return out.join('+');
                };
                const [resA, resB] = await Promise.all([collectA(), collectB()]);
                document.getElementById('out').textContent = resA + '|' + resB;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("A+B|A+B".to_string()));
}

#[tokio::test]
async fn writable_stream_accepts_writes() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const collected = [];
                const ws = new WritableStream({
                    write(chunk) { collected.push(chunk); },
                    close() { collected.push('CLOSED'); },
                });
                const w = ws.getWriter();
                await w.write('x');
                await w.write('y');
                await w.close();
                document.getElementById('out').textContent = collected.join(',');
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("x,y,CLOSED".to_string()));
}

#[tokio::test]
async fn transform_stream_pipes_through() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                // Source: emits 1, 2, 3.
                const src = new ReadableStream({
                    start(c) { c.enqueue(1); c.enqueue(2); c.enqueue(3); c.close(); },
                });
                // Transform: doubles each chunk.
                const doubler = new TransformStream({
                    transform(chunk, c) { c.enqueue(chunk * 2); },
                });
                const out = src.pipeThrough(doubler);
                const r = out.getReader();
                const vals = [];
                while (true) {
                    const { done, value } = await r.read();
                    if (done) break;
                    vals.push(value);
                }
                document.getElementById('out').textContent = vals.join(',');
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("2,4,6".to_string()));
}

// ============================================================================
// B4 — Proxy-backed DOM prototypes (#55)
// ============================================================================

#[tokio::test]
async fn canvas_methods_are_own_properties_of_prototype() {
    // Real Chrome puts getContext/toDataURL/toBlob on
    // HTMLCanvasElement.prototype directly, not on Element.prototype.
    // A probe that reads the descriptors must see them as own props.
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const proto = HTMLCanvasElement.prototype;
            const parts = [];
            for (const name of ['getContext', 'toDataURL', 'toBlob']) {
                const d = Object.getOwnPropertyDescriptor(proto, name);
                parts.push(name + ':' + (d ? 'own' : 'inherited'));
            }
            document.getElementById('out').textContent = parts.join(',');
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("getContext:own,toDataURL:own,toBlob:own".to_string())
    );
}

#[tokio::test]
async fn canvas_get_context_illegal_invocation() {
    // Calling HTMLCanvasElement.prototype.getContext with a non-canvas
    // receiver must throw `TypeError: Illegal invocation` — this is
    // the fingerprint-leak the old Element-prototype patch had: a
    // probe would see getContext accept any element.
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const getContext = HTMLCanvasElement.prototype.getContext;
            const fake = {};
            try {
                getContext.call(fake, '2d');
                document.getElementById('out').textContent = 'no-throw';
            } catch (e) {
                document.getElementById('out').textContent =
                    e.name + '|' + (e.message.includes('Illegal invocation') ? 'ok' : e.message);
            }
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("TypeError|ok".to_string()));
}

#[tokio::test]
async fn canvas_to_data_url_illegal_invocation() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const toDataURL = HTMLCanvasElement.prototype.toDataURL;
            try {
                toDataURL.call({ tagName: 'DIV' });
                document.getElementById('out').textContent = 'no-throw';
            } catch (e) {
                document.getElementById('out').textContent = e.name;
            }
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("TypeError".to_string()));
}

#[tokio::test]
async fn canvas_methods_still_work_on_real_canvas() {
    // Regression: after the proxy move, the normal canvas-on-element
    // path must still work for HTML-parsed canvases.
    let mut page = Page::from_html(
        r#"<html><body>
            <canvas id="c" width="50" height="50"></canvas>
            <div id="out"></div>
            <script>
                const c = document.getElementById('c');
                const ctx = c.getContext('2d');
                ctx.fillStyle = 'magenta';
                ctx.fillRect(0, 0, 50, 50);
                const url = c.toDataURL();
                document.getElementById('out').textContent =
                    (ctx !== null) + '/' + url.startsWith('data:image/png;base64,');
            </script>
        </body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("true/true".to_string()));
}

// ============================================================================
// B3 — OffscreenCanvas (main thread + workers)
// ============================================================================

#[tokio::test]
async fn offscreen_canvas_main_thread_is_functional() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            const oc = new OffscreenCanvas(100, 50);
            const ctx = oc.getContext('2d');
            const ctxIsCtx = ctx !== null && typeof ctx.fillRect === 'function';
            ctx.fillStyle = 'red';
            ctx.fillRect(0, 0, 10, 10);
            // measureText should return real values from T1.2 font stack.
            ctx.font = '16px Arial';
            const m = ctx.measureText('Hello');
            const widthOk = m.width > 0;
            document.getElementById('out').textContent =
                ctxIsCtx + '/' + widthOk;
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("true/true".to_string()));
}

#[tokio::test]
async fn offscreen_canvas_convert_to_blob_png() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const oc = new OffscreenCanvas(20, 20);
                const ctx = oc.getContext('2d');
                ctx.fillStyle = '#00ff00';
                ctx.fillRect(0, 0, 20, 20);
                const blob = await oc.convertToBlob({ type: 'image/png' });
                const ab = await blob.arrayBuffer();
                const bytes = new Uint8Array(ab);
                // PNG magic: 89 50 4E 47
                const magic = bytes[0] === 0x89 && bytes[1] === 0x50 &&
                    bytes[2] === 0x4e && bytes[3] === 0x47;
                document.getElementById('out').textContent =
                    magic + '/' + blob.type + '/' + (bytes.byteLength > 30);
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true/image/png/true".to_string())
    );
}

#[tokio::test]
async fn offscreen_canvas_in_worker() {
    // Workers now run canvas_bootstrap too — `new OffscreenCanvas(w,h)
    // .getContext('2d')` should return a real CanvasRenderingContext2D
    // in the worker scope.
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = `
                    self.onmessage = () => {
                        const hasOC = typeof OffscreenCanvas === 'function';
                        if (!hasOC) { self.postMessage('no-offscreen'); return; }
                        const oc = new OffscreenCanvas(40, 40);
                        const ctx = oc.getContext('2d');
                        if (!ctx) { self.postMessage('no-context'); return; }
                        ctx.fillStyle = 'blue';
                        ctx.fillRect(0, 0, 40, 40);
                        const img = ctx.getImageData(5, 5, 1, 1);
                        const px = img.data;
                        self.postMessage(
                            px[0] + ',' + px[1] + ',' + px[2] + ',' + px[3]
                        );
                    };
                `;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url);
                const result = await new Promise((r) => {
                    w.onmessage = (e) => r(e.data);
                    setTimeout(() => w.postMessage('go'), 100);
                });
                w.terminate();
                document.getElementById('out').textContent = result;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    // Blue at (5,5) = (0, 0, 255, 255) from our skia-safe canvas.
    assert_eq!(
        page.text_of("#out"),
        Some("0,0,255,255".to_string())
    );
}

// ============================================================================
// B2 — Worker transferables + binary-safe postMessage
// ============================================================================

#[tokio::test]
async fn worker_post_message_typed_array_round_trip() {
    // Previously, JSON serialization would silently drop TypedArrays
    // in Worker.postMessage (receiver got `{}`). With the wire
    // serializer they survive round-trip with byte-exact content.
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = `
                    self.onmessage = (e) => {
                        const u = e.data.u8;
                        const isU8 = u instanceof Uint8Array;
                        const bytes = isU8
                            ? Array.from(u).join(',')
                            : 'NOT_U8:' + Object.prototype.toString.call(u);
                        self.postMessage({ isU8, bytes });
                    };
                `;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url);
                const sendBytes = new Uint8Array([1, 2, 3, 250, 255]);
                const result = await new Promise((r) => {
                    w.onmessage = (e) => r(e.data);
                    setTimeout(() => w.postMessage({ u8: sendBytes }), 100);
                });
                w.terminate();
                document.getElementById('out').textContent =
                    result.isU8 + '|' + result.bytes;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true|1,2,3,250,255".to_string())
    );
}

#[tokio::test]
async fn worker_post_message_arraybuffer_round_trip() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = `
                    self.onmessage = (e) => {
                        const buf = e.data;
                        const isAB = buf instanceof ArrayBuffer;
                        const byteLen = isAB ? buf.byteLength : -1;
                        const view = isAB ? new Uint8Array(buf) : null;
                        const first = view ? view[0] : -1;
                        const last = view ? view[view.length - 1] : -1;
                        self.postMessage({ isAB, byteLen, first, last });
                    };
                `;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url);
                const buf = new ArrayBuffer(4);
                new Uint8Array(buf).set([0x10, 0x20, 0x30, 0x40]);
                const result = await new Promise((r) => {
                    w.onmessage = (e) => r(e.data);
                    setTimeout(() => w.postMessage(buf, [buf]), 100);
                });
                w.terminate();
                document.getElementById('out').textContent =
                    result.isAB + '|' + result.byteLen + '|' +
                    result.first + '|' + result.last;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true|4|16|64".to_string())
    );
}

#[tokio::test]
async fn worker_post_message_map_set_date_survive() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = `
                    self.onmessage = (e) => {
                        const {m, s, d, re} = e.data;
                        const parts = [];
                        parts.push(m instanceof Map);
                        parts.push(m.get('k') === 42);
                        parts.push(s instanceof Set);
                        parts.push(s.has('x'));
                        parts.push(d instanceof Date);
                        parts.push(d.getTime() === 1000);
                        parts.push(re instanceof RegExp);
                        parts.push(re.source === 'foo');
                        self.postMessage(parts.join(','));
                    };
                `;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url);
                const m = new Map([['k', 42]]);
                const s = new Set(['x', 'y']);
                const d = new Date(1000);
                const re = /foo/gi;
                const result = await new Promise((r) => {
                    w.onmessage = (e) => r(e.data);
                    setTimeout(() => w.postMessage({m, s, d, re}), 100);
                });
                w.terminate();
                document.getElementById('out').textContent = result;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("true,true,true,true,true,true,true,true".to_string())
    );
}

#[tokio::test]
async fn worker_post_message_transferable_list_accepted() {
    // The transferables list must be accepted as an array of
    // ArrayBuffers/views without throwing. (We don't actually detach
    // the source — that requires V8 internals — but the shape check
    // that fingerprint probes do for `postMessage(buf, [buf])` passes.)
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = `
                    self.onmessage = () => self.postMessage('received');
                `;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url);
                const buf = new ArrayBuffer(8);
                try {
                    w.postMessage({ buf }, [buf]);
                    const ok = await new Promise((r) => { w.onmessage = e => r(e.data); });
                    document.getElementById('out').textContent = 'ok:' + ok;
                } catch (e) {
                    document.getElementById('out').textContent = 'threw:' + e.message;
                }
                w.terminate();
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("ok:received".to_string())
    );
}

#[tokio::test]
async fn worker_post_message_rejects_non_transferable() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = `self.onmessage = () => self.postMessage('x');`;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url);
                try {
                    w.postMessage('hi', [{}]);
                    document.getElementById('out').textContent = 'no-throw';
                } catch (e) {
                    document.getElementById('out').textContent = 'threw:' + e.name;
                }
                w.terminate();
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("threw:TypeError".to_string())
    );
}

// ============================================================================
// B1 — Module workers
// ============================================================================

#[tokio::test]
async fn module_worker_option_accepted() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                // Sanity: a module-type worker body with no imports
                // should run the same way as a classic worker and be
                // able to post a message back.
                const src = `
                    self.onmessage = function(e) {
                        self.postMessage('module-says:' + e.data);
                    };
                `;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url, { type: 'module' });
                const result = await new Promise((resolve) => {
                    w.onmessage = (e) => resolve(e.data);
                    setTimeout(() => w.postMessage('hi'), 100);
                });
                w.terminate();
                document.getElementById('out').textContent = result;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(
        page.text_of("#out"),
        Some("module-says:hi".to_string())
    );
}

#[tokio::test]
async fn module_worker_import_meta_url_available() {
    // Module workers expose `import.meta` — a classic worker does not.
    // Even if the URL value is our synthetic worker-oxide scheme, the
    // presence of import.meta (without a ReferenceError) proves the
    // body was loaded in module mode.
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = `
                    const hasImportMeta = typeof import.meta === 'object';
                    self.onmessage = () => {
                        self.postMessage(hasImportMeta ? 'module' : 'not-module');
                    };
                `;
                const url = URL.createObjectURL(new Blob([src]));
                const w = new Worker(url, { type: 'module' });
                const result = await new Promise((resolve) => {
                    w.onmessage = (e) => resolve(e.data);
                    setTimeout(() => w.postMessage('q'), 100);
                });
                w.terminate();
                document.getElementById('out').textContent = result;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("module".to_string()));
}

// ============================================================================
// A5 — IndexedDB
// ============================================================================

#[tokio::test]
async fn indexeddb_open_put_get_round_trip() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const req = indexedDB.open('test-db', 1);
                req.onupgradeneeded = () => {
                    req.result.createObjectStore('items', { keyPath: 'id' });
                };
                await new Promise((resolve, reject) => {
                    req.onsuccess = resolve;
                    req.onerror = reject;
                });
                const db = req.result;
                const tx = db.transaction('items', 'readwrite');
                const store = tx.objectStore('items');
                store.put({ id: 1, name: 'alpha' });
                store.put({ id: 2, name: 'beta' });
                await new Promise((resolve) => { tx.oncomplete = resolve; });
                const tx2 = db.transaction('items', 'readonly');
                const getReq = tx2.objectStore('items').get(1);
                await new Promise((r) => { getReq.onsuccess = r; });
                document.getElementById('out').textContent =
                    getReq.result.id + '/' + getReq.result.name;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("1/alpha".to_string()));
}

#[tokio::test]
async fn indexeddb_deep_clone_isolates_stored_values() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const req = indexedDB.open('isolation-db', 1);
                req.onupgradeneeded = () => {
                    req.result.createObjectStore('s', { keyPath: 'id' });
                };
                await new Promise((r) => { req.onsuccess = r; });
                const db = req.result;
                const obj = { id: 1, nested: { value: 'original' } };
                const tx = db.transaction('s', 'readwrite');
                tx.objectStore('s').put(obj);
                await new Promise((r) => { tx.oncomplete = r; });
                // Mutate the source AFTER put — the stored copy must
                // NOT reflect the mutation.
                obj.nested.value = 'mutated';
                const getReq = db.transaction('s').objectStore('s').get(1);
                await new Promise((r) => { getReq.onsuccess = r; });
                const stored = getReq.result.nested.value;
                // And mutating the retrieved value must NOT affect
                // the store (second get returns original).
                getReq.result.nested.value = 'leaked';
                const getReq2 = db.transaction('s').objectStore('s').get(1);
                await new Promise((r) => { getReq2.onsuccess = r; });
                document.getElementById('out').textContent =
                    stored + '/' + getReq2.result.nested.value;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("original/original".to_string()));
}

#[tokio::test]
async fn indexeddb_key_range_query() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const req = indexedDB.open('range-db', 1);
                req.onupgradeneeded = () => {
                    req.result.createObjectStore('n', { keyPath: 'i' });
                };
                await new Promise((r) => { req.onsuccess = r; });
                const db = req.result;
                const tx = db.transaction('n', 'readwrite');
                const store = tx.objectStore('n');
                for (let i = 1; i <= 5; i++) store.put({ i });
                await new Promise((r) => { tx.oncomplete = r; });
                const range = IDBKeyRange.bound(2, 4);
                const getAllReq = db.transaction('n').objectStore('n').getAll(range);
                await new Promise((r) => { getAllReq.onsuccess = r; });
                const ids = getAllReq.result.map((o) => o.i).join(',');
                document.getElementById('out').textContent = ids;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("2,3,4".to_string()));
}

#[tokio::test]
async fn indexeddb_cursor_iterates_in_key_order() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const req = indexedDB.open('cursor-db', 1);
                req.onupgradeneeded = () => {
                    req.result.createObjectStore('c', { keyPath: 'k' });
                };
                await new Promise((r) => { req.onsuccess = r; });
                const db = req.result;
                const tx = db.transaction('c', 'readwrite');
                const store = tx.objectStore('c');
                // Insert out of order — cursor should still come back
                // in sorted key order.
                store.put({ k: 3 });
                store.put({ k: 1 });
                store.put({ k: 2 });
                await new Promise((r) => { tx.oncomplete = r; });
                const curReq = db.transaction('c').objectStore('c').openCursor();
                const keys = [];
                await new Promise((resolve) => {
                    curReq.onsuccess = () => {
                        const cur = curReq.result;
                        if (!cur) { resolve(); return; }
                        keys.push(cur.key);
                        cur.continue();
                    };
                });
                document.getElementById('out').textContent = keys.join(',');
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("1,2,3".to_string()));
}

#[tokio::test]
async fn indexeddb_version_upgrade_flow() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                // Open v1: create 'old' store.
                const r1 = indexedDB.open('upgrade-db', 1);
                r1.onupgradeneeded = () => {
                    r1.result.createObjectStore('old');
                };
                await new Promise((r) => { r1.onsuccess = r; });
                r1.result.close();
                // Reopen at v2: onupgradeneeded should fire with
                // oldVersion=1 newVersion=2.
                let upgradeEvent = null;
                const r2 = indexedDB.open('upgrade-db', 2);
                r2.onupgradeneeded = (e) => {
                    upgradeEvent = { old: e.oldVersion, new_: e.newVersion };
                    r2.result.createObjectStore('new');
                };
                await new Promise((r) => { r2.onsuccess = r; });
                const db = r2.result;
                const stores = [...db.objectStoreNames].sort().join(',');
                document.getElementById('out').textContent =
                    upgradeEvent.old + '->' + upgradeEvent.new_ + '/' + stores;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("1->2/new,old".to_string()));
}

// ============================================================================
// A3 — Worker importScripts()
// ============================================================================

#[tokio::test]
async fn worker_import_scripts_from_blob_url() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const importSrc = `
                    self._imported_value = 123;
                    self.onmessage = function(e) {
                        self.postMessage({ got: e.data, imported: self._imported_value });
                    };
                `;
                const importURL = URL.createObjectURL(new Blob([importSrc]));

                const workerSrc = `
                    importScripts(${JSON.stringify(importURL)});
                `;
                const workerURL = URL.createObjectURL(new Blob([workerSrc]));

                const w = new Worker(workerURL);
                const result = await new Promise((resolve) => {
                    w.onmessage = (e) => resolve(e.data);
                    setTimeout(() => w.postMessage('ping'), 100);
                });
                w.terminate();
                document.getElementById('out').textContent =
                    result.got + '/' + result.imported;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("ping/123".to_string()));
}

#[tokio::test]
async fn worker_import_scripts_from_data_url() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const src = btoa("self._data_imported = true;");
                const dataURL = "data:application/javascript;base64," + src;

                const workerSrc = `
                    importScripts(${JSON.stringify(dataURL)});
                    self.postMessage(self._data_imported === true ? 'yes' : 'no');
                `;
                const workerURL = URL.createObjectURL(new Blob([workerSrc]));

                const w = new Worker(workerURL);
                const result = await new Promise((resolve) => {
                    w.onmessage = (e) => resolve(e.data);
                });
                w.terminate();
                document.getElementById('out').textContent = result;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("yes".to_string()));
}

#[tokio::test]
async fn worker_import_scripts_unknown_blob_throws() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const workerSrc = `
                    try {
                        importScripts('blob:null/does-not-exist-uuid');
                        self.postMessage('no-throw');
                    } catch (e) {
                        self.postMessage('threw:' + e.message);
                    }
                `;
                const url = URL.createObjectURL(new Blob([workerSrc]));
                const w = new Worker(url);
                const msg = await new Promise((res) => { w.onmessage = e => res(e.data); });
                w.terminate();
                document.getElementById('out').textContent = msg;
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    let out = page.text_of("#out").unwrap_or_default();
    assert!(out.starts_with("threw:"), "got {out}");
}

#[tokio::test]
async fn blob_revoke_invalidates_url() {
    let mut page = Page::from_html(
        r#"<html><body><div id="out"></div><script>
            (async () => {
                const url = URL.createObjectURL(new Blob(['revoked']));
                URL.revokeObjectURL(url);
                try {
                    await fetch(url);
                    document.getElementById('out').textContent = 'fetched';
                } catch (e) {
                    document.getElementById('out').textContent = 'error';
                }
            })();
        </script></body></html>"#,
    )
    .await
    .unwrap();
    assert_eq!(page.text_of("#out"), Some("error".to_string()));
}
