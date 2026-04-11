//! Loads the captured adidas Akamai sensor VM and runs it under instrumented
//! accessors on the APIs most likely to gate a Worker-spawn code path. Dumps
//! every read count so we can see which APIs the sensor VM touches and which
//! (if any) it trips over.
//!
//! Requires that `/tmp/adidas_sensor_vm.js` exists — run the
//! `adidas_fetch_sensor_vm` test first if not.
//!
//! Run:
//!   cargo test -p browser --test adidas_sensor_api_probes -- \
//!     --ignored --test-threads=1 --nocapture

use event_loop::BrowserEventLoop;
use js_runtime::BrowserJsRuntime;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn adidas_sensor_api_probes() {
    let vm_path = "/tmp/adidas_sensor_vm.js";
    let vm = match std::fs::read_to_string(vm_path) {
        Ok(s) => s,
        Err(e) => {
            println!("[skip] {vm_path}: {e} — run adidas_fetch_sensor_vm first");
            return;
        }
    };
    println!("[vm] loaded {vm_path} ({} bytes)", vm.len());

    // Build a page with the normal bootstrap (Worker, Blob, URL, navigator, …).
    let dom = html_parser::parse_html(
        "<html><head></head><body><div id=\"out\"></div></body></html>",
    );
    let mut evloop = BrowserEventLoop::new(BrowserJsRuntime::with_profile(
        dom,
        stealth::chrome_130_macos(),
    ));

    // Install access counters on the globals most likely to gate a Worker
    // code path. Each counter captures:
    //   - how many times the property was read
    //   - the first few stack traces, so we can see who read it
    //
    // We replace each global with a getter that increments the counter and
    // returns the original value. This catches `typeof Worker`, `new Worker`,
    // `'Worker' in globalThis`, etc.
    let instrumentation = r#"
        globalThis.__apiProbes = {};
        function _probe(obj, key) {
            if (!obj) return;
            const descriptor = Object.getOwnPropertyDescriptor(obj, key);
            let original;
            try {
                original = obj[key];
            } catch (e) {
                original = undefined;
            }
            globalThis.__apiProbes[key] = {
                count: 0,
                type: typeof original,
                present: original !== undefined,
                stacks: [],
            };
            try {
                Object.defineProperty(obj, key, {
                    configurable: true,
                    get: function() {
                        const probe = globalThis.__apiProbes[key];
                        probe.count++;
                        if (probe.stacks.length < 3) {
                            try {
                                throw new Error('trace');
                            } catch (e) {
                                probe.stacks.push(
                                    String((e && e.stack) || '').substring(0, 300)
                                );
                            }
                        }
                        return original;
                    },
                    set: function(v) {
                        original = v;
                    }
                });
            } catch (e) {
                globalThis.__apiProbes[key].defineError = String(e && e.message || e);
            }
        }
        const gates = [
            'Worker', 'SharedWorker', 'ServiceWorker', 'WorkerGlobalScope',
            'OffscreenCanvas', 'SharedArrayBuffer', 'Atomics', 'BroadcastChannel',
            'MessageChannel', 'crossOriginIsolated', 'structuredClone',
            'WebAssembly', 'FinalizationRegistry', 'WeakRef',
            'createImageBitmap', 'ImageBitmap', 'ImageData',
            'Permissions', 'PermissionStatus',
            'Notification', 'PushManager', 'PushSubscription',
            'USB', 'Bluetooth', 'Serial', 'HID',
            'Clipboard', 'ClipboardItem',
            'CSSStyleSheet', 'CSSRule',
            'IntersectionObserver', 'ResizeObserver', 'MutationObserver',
            'PerformanceObserver',
            'TrustedTypes', 'trustedTypes',
            'XRSystem', 'xr',
            'DOMMatrix', 'DOMMatrixReadOnly', 'DOMPoint',
        ];
        for (const g of gates) _probe(globalThis, g);
        // Navigator sub-objects
        if (globalThis.navigator) {
            for (const g of [
                'serviceWorker', 'locks', 'storage', 'permissions',
                'mediaDevices', 'credentials', 'keyboard', 'hid', 'usb',
                'bluetooth', 'serial', 'xr',
                'hardwareConcurrency', 'deviceMemory', 'maxTouchPoints',
                'connection', 'webdriver', 'userAgentData',
            ]) _probe(globalThis.navigator, g);
        }
        // performance.memory
        if (globalThis.performance) {
            _probe(globalThis.performance, 'memory');
        }
    "#;
    evloop
        .execute_script(instrumentation)
        .expect("instrumentation install");

    // Secondary instrumentation: wrap EVERY own method and EVERY getter on
    // CanvasRenderingContext2D.prototype (plus Canvas / Element extraction
    // paths), log argument previews for fillText/fillRect/arc, and catch
    // any thrown errors so we can see if the sensor VM is hashing via
    // exception behavior. Also wrap AudioContext + WebGL for completeness.
    let method_instrumentation = r#"
        globalThis.__callLog = [];
        globalThis.__errorLog = [];
        globalThis.__methodProbes = {};
        globalThis._previewArg = function _previewArg(a) {
            try {
                if (a === null) return 'null';
                if (a === undefined) return 'undefined';
                if (typeof a === 'string') return JSON.stringify(a.substring(0, 40));
                if (typeof a === 'number' || typeof a === 'boolean') return String(a);
                if (typeof a === 'function') return 'fn';
                if (typeof a === 'object') {
                    const ctor = a.constructor && a.constructor.name;
                    return '<' + (ctor || 'obj') + '>';
                }
                return typeof a;
            } catch (e) { return '?'; }
        };
        globalThis.__wrapDiag = { enter: 0, pushed: 0, caught: 0 };
        globalThis._mprobe = function _mprobe(proto, key, label) {
            if (!proto || typeof proto[key] !== 'function') return;
            const original = proto[key];
            globalThis.__methodProbes[label] = { count: 0 };
            proto[key] = function(...args) {
                globalThis.__wrapDiag.enter++;
                const probe = globalThis.__methodProbes[label];
                probe.count++;
                try {
                    if (probe.count <= 3) {
                        let preview = '';
                        const limit = args.length > 8 ? 8 : args.length;
                        for (let i = 0; i < limit; i++) {
                            if (i > 0) preview += ', ';
                            try { preview += globalThis._previewArg(args[i]); }
                            catch (e) { preview += '?'; }
                        }
                        if (args.length > 8) preview += ', … (' + args.length + ' total)';
                        else if (args.length === 0) preview = '';
                        preview = '[' + args.length + '] ' + preview;
                        if (!globalThis.__callLog) globalThis.__callLog = [];
                        if (globalThis.__callLog.length < 500) {
                            globalThis.__callLog.push(label + '(' + preview + ')');
                            globalThis.__wrapDiag.pushed++;
                        }
                    }
                } catch (e) {
                    globalThis.__wrapDiag.caught++;
                    globalThis.__wrapDiag.lastErr = String(e && e.message || e);
                }
                try {
                    return original.apply(this, args);
                } catch (e) {
                    if (globalThis.__errorLog && globalThis.__errorLog.length < 40) {
                        globalThis.__errorLog.push(label + ' THREW: ' + String(e && e.message || e).substring(0, 120));
                    }
                    throw e;
                }
            };
        };
        const _mprobe = globalThis._mprobe;
        // Wrap a getter — observes every access to an accessor property.
        function _gprobe(proto, key, label) {
            if (!proto) return;
            const desc = Object.getOwnPropertyDescriptor(proto, key);
            if (!desc || !desc.get) return;
            const orig = desc.get;
            globalThis.__methodProbes[label] = { count: 0, sample: null };
            Object.defineProperty(proto, key, {
                configurable: true,
                enumerable: desc.enumerable,
                get: function() {
                    const probe = globalThis.__methodProbes[label];
                    probe.count++;
                    try {
                        const v = orig.call(this);
                        if (probe.sample === null && probe.count <= 1) {
                            try { probe.sample = JSON.stringify(v); } catch { probe.sample = String(v); }
                        }
                        return v;
                    } catch (e) {
                        if (globalThis.__errorLog.length < 40) {
                            globalThis.__errorLog.push(label + ' THREW: ' + String(e && e.message || e).substring(0, 120));
                        }
                        throw e;
                    }
                },
                set: desc.set,
            });
        }
        // Wrap EVERY own property of a prototype — both methods and getters.
        function _proto_all(proto, clsLabel, skip) {
            if (!proto) return;
            skip = skip || new Set();
            for (const key of Object.getOwnPropertyNames(proto)) {
                if (skip.has(key) || key === 'constructor') continue;
                const desc = Object.getOwnPropertyDescriptor(proto, key);
                if (!desc) continue;
                if (typeof desc.value === 'function') {
                    _mprobe(proto, key, clsLabel + '.' + key);
                } else if (desc.get) {
                    _gprobe(proto, key, clsLabel + '#' + key);
                }
            }
        }
        // Full sweep: wrap every method and every getter on the prototypes
        // we care about. This ensures we catch side-channel extractors like
        // measureText / isPointInPath / getLineDash, and getters like
        // `canvas`, `fillStyle`, `font`, etc. that return observable state.
        _proto_all(CanvasRenderingContext2D.prototype, 'Ctx2D');
        if (globalThis.CanvasRenderingContext2D) {
            // defineProperty getters are sometimes on instances rather than
            // the prototype in our runtime; sweep HTMLCanvasElement.prototype
            // and Element.prototype too.
        }
        _proto_all(HTMLCanvasElement.prototype, 'Canvas');
        _proto_all(Element.prototype, 'Element');
        _proto_all(AudioContext.prototype, 'AudioContext');
        _proto_all(OfflineAudioContext.prototype, 'OfflineAudioContext');
        if (globalThis.AudioBuffer) _proto_all(AudioBuffer.prototype, 'AudioBuffer');
        if (globalThis.OscillatorNode) _proto_all(OscillatorNode.prototype, 'OscillatorNode');
        if (globalThis.DynamicsCompressorNode) _proto_all(DynamicsCompressorNode.prototype, 'DynamicsCompressorNode');
        if (globalThis.AnalyserNode) _proto_all(AnalyserNode.prototype, 'AnalyserNode');
        if (globalThis.WebGLRenderingContext) _proto_all(WebGLRenderingContext.prototype, 'WebGL');
        if (globalThis.WebGL2RenderingContext) _proto_all(WebGL2RenderingContext.prototype, 'WebGL2');
        _mprobe(Document.prototype, 'createElement', 'Document.createElement');
        _mprobe(Document.prototype, 'querySelector', 'Document.querySelector');
        _mprobe(Document.prototype, 'querySelectorAll', 'Document.querySelectorAll');
        _mprobe(Document.prototype, 'getElementById', 'Document.getElementById');

        // performance.now() — timing-based fingerprint is a strong candidate
        // explanation for canvas paint calls without extraction methods.
        if (globalThis.performance && typeof performance.now === 'function') {
            const _origNow = performance.now.bind(performance);
            globalThis.__perfNowCalls = 0;
            performance.now = function() {
                globalThis.__perfNowCalls++;
                return _origNow();
            };
        }

        // Wrap the AudioBuffer shape our OfflineAudioContext.startRendering
        // returns. Since our buf is a plain object, AudioBuffer.prototype
        // doesn't apply — intercept via monkey-patching the ops dispatch
        // path. Instead we wrap the Promise resolution: patch the prototype
        // of Promise to track `.then` calls to see how many deferred
        // callbacks are chained.
        globalThis.__getChannelDataCalls = 0;

        // Function.prototype.toString — fingerprinting trick: sensor VMs
        // hash `HTMLCanvasElement.prototype.toDataURL.toString()` to check if
        // it contains "[native code]". If our stub returns JS source instead,
        // the hash differs from Chrome's. Capture every .toString() call and
        // record the receiver if it's a known canvas/audio method.
        const _origFnToString = Function.prototype.toString;
        globalThis.__fnToStringLog = [];
        Function.prototype.toString = function() {
            const result = _origFnToString.call(this);
            try {
                if (globalThis.__fnToStringLog.length < 200) {
                    // Find the function's name to identify canvas-related methods.
                    const name = this && this.name ? String(this.name) : '(anon)';
                    globalThis.__fnToStringLog.push({
                        name: name,
                        containsNative: result.indexOf('[native code]') !== -1,
                        len: result.length,
                        preview: result.substring(0, 80),
                    });
                }
            } catch (e) {}
            return result;
        };
    "#;
    evloop
        .execute_script(method_instrumentation)
        .expect("method instrumentation install");

    // Diagnostic: confirm __callLog is reachable and writable from script.
    let diag = evloop
        .execute_script(
            r#"(function(){
                try { globalThis.__callLog.push('DIAG-SENTINEL'); }
                catch(e) { return 'ERR:' + (e && e.message || e); }
                return String(globalThis.__callLog.length) + ':' + typeof globalThis._previewArg;
            })()"#,
        )
        .unwrap_or_default();
    println!("[diag] callLog state after setup: {diag}");

    // Capture async errors — if the sensor VM's .then() chain throws after
    // canvas drawing, window.onerror / unhandledrejection is our only way
    // to see it.
    evloop
        .execute_script(
            r#"
            globalThis.__asyncErrors = [];
            globalThis.addEventListener && globalThis.addEventListener('unhandledrejection', function(e) {
                const reason = (e && e.reason) || 'unknown';
                const msg = (reason && reason.message) || String(reason);
                const stack = (reason && reason.stack) || '';
                globalThis.__asyncErrors.push('REJECT: ' + String(msg).substring(0, 300) + '\n' + String(stack).substring(0, 400));
            });
            globalThis.onerror = function(msg, src, line, col, err) {
                globalThis.__asyncErrors.push('ERROR: ' + String(msg) + ' @ ' + (src || '?') + ':' + line + (err && err.stack ? '\n' + String(err.stack).substring(0, 400) : ''));
            };
        "#,
        )
        .ok();

    // Now run the sensor VM. Wrap in try/catch so we see any top-level throw.
    let wrapped = format!(
        r#"
        try {{
            (function () {{
                {vm}
            }})();
            globalThis.__vmRan = 'ok';
        }} catch (e) {{
            globalThis.__vmRan = 'err:' + (e && e.message || e);
        }}
        "#
    );
    evloop.execute_script(&wrapped).expect("vm execute");
    // Snapshot the call count BEFORE draining the event loop, then drain
    // for 10 seconds, then compare. If Ctx2D call counts grow during the
    // drain, the sensor VM is doing deferred extraction in a Promise or
    // setTimeout callback.
    let sync_fill = evloop
        .execute_script(
            "String((globalThis.__methodProbes['Ctx2D.fillText'] || {count:0}).count)",
        )
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let sync_getImageData = evloop
        .execute_script(
            "String((globalThis.__methodProbes['Ctx2D.getImageData'] || {count:0}).count)",
        )
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    println!("[sync-counts] Ctx2D.fillText={sync_fill}, Ctx2D.getImageData={sync_getImageData}");
    let _ = evloop.run_until_idle(Duration::from_secs(10)).await;
    let post_fill = evloop
        .execute_script(
            "String((globalThis.__methodProbes['Ctx2D.fillText'] || {count:0}).count)",
        )
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let post_getImageData = evloop
        .execute_script(
            "String((globalThis.__methodProbes['Ctx2D.getImageData'] || {count:0}).count)",
        )
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    println!(
        "[post-drain-counts] Ctx2D.fillText={post_fill} (delta {}), Ctx2D.getImageData={post_getImageData} (delta {})",
        post_fill - sync_fill,
        post_getImageData - sync_getImageData
    );
    let perf_now = evloop
        .execute_script("String(globalThis.__perfNowCalls || 0)")
        .unwrap_or_default();
    println!("[perf-now-calls] {perf_now}");

    // Async errors captured during the sensor VM's .then() chains.
    let async_errors = evloop
        .execute_script(
            "(Array.isArray(globalThis.__asyncErrors) ? globalThis.__asyncErrors.join('\\n===\\n') : '')",
        )
        .unwrap_or_default();
    println!("[async-errors]");
    if async_errors.is_empty() {
        println!("  (none)");
    } else {
        for line in async_errors.lines().take(40) {
            println!("  {line}");
        }
    }

    let ran = evloop
        .execute_script("String(globalThis.__vmRan)")
        .unwrap_or_default();
    println!("[vm] ran: {ran}");

    let probes = evloop
        .execute_script(
            r#"JSON.stringify(
                Object.entries(globalThis.__apiProbes || {})
                    .sort((a, b) => (b[1].count || 0) - (a[1].count || 0))
                    .map(([k, v]) => ({
                        k,
                        count: v.count,
                        type: v.type,
                        present: v.present,
                        firstStack: (v.stacks && v.stacks[0]) || '',
                    }))
            )"#,
        )
        .unwrap_or_default();

    // Also dump the VALUES our runtime returns for the APIs the sensor VM
    // actually touched. Presence is only half the story; the sensor VM will
    // hash whatever value it gets.
    let values = evloop
        .execute_script(
            r#"(function() {
                function safe(fn) {
                    try { return fn(); } catch (e) { return 'ERR:' + String(e && e.message || e); }
                }
                function short(v) {
                    const s = typeof v === 'string' ? v : JSON.stringify(v);
                    return s === undefined ? 'undefined' : String(s).substring(0, 200);
                }
                const nav = globalThis.navigator || {};
                const perf = globalThis.performance || {};
                const out = {};
                out.crossOriginIsolated = safe(() => globalThis.crossOriginIsolated);
                out.OffscreenCanvas = safe(() => typeof globalThis.OffscreenCanvas);
                out.SharedArrayBuffer = safe(() => typeof globalThis.SharedArrayBuffer);
                out.Notification = safe(() => typeof globalThis.Notification);
                out.ServiceWorker = safe(() => typeof globalThis.ServiceWorker);
                out.SharedWorker = safe(() => typeof globalThis.SharedWorker);
                out.Worker = safe(() => typeof globalThis.Worker);
                out['nav.hardwareConcurrency'] = safe(() => nav.hardwareConcurrency);
                out['nav.deviceMemory'] = safe(() => nav.deviceMemory);
                out['nav.maxTouchPoints'] = safe(() => nav.maxTouchPoints);
                out['nav.webdriver'] = safe(() => nav.webdriver);
                out['nav.userAgentData'] = safe(() => nav.userAgentData && {
                    brands: nav.userAgentData.brands,
                    mobile: nav.userAgentData.mobile,
                    platform: nav.userAgentData.platform,
                });
                out['nav.connection'] = safe(() => nav.connection && {
                    effectiveType: nav.connection.effectiveType,
                    rtt: nav.connection.rtt,
                    downlink: nav.connection.downlink,
                    saveData: nav.connection.saveData,
                    type: nav.connection.type,
                });
                out['nav.permissions'] = safe(() => nav.permissions && typeof nav.permissions.query);
                out['nav.storage'] = safe(() => nav.storage && Object.getOwnPropertyNames(Object.getPrototypeOf(nav.storage) || {}));
                out['nav.mediaDevices'] = safe(() => nav.mediaDevices && Object.getOwnPropertyNames(Object.getPrototypeOf(nav.mediaDevices) || {}));
                out['nav.bluetooth'] = safe(() => nav.bluetooth && typeof nav.bluetooth);
                out['nav.serviceWorker'] = safe(() => nav.serviceWorker && Object.getOwnPropertyNames(Object.getPrototypeOf(nav.serviceWorker) || {}));
                out['performance.memory'] = safe(() => perf.memory && {
                    jsHeapSizeLimit: perf.memory.jsHeapSizeLimit,
                    totalJSHeapSize: perf.memory.totalJSHeapSize,
                    usedJSHeapSize: perf.memory.usedJSHeapSize,
                });
                const result = {};
                for (const k in out) result[k] = short(out[k]);
                return JSON.stringify(result);
            })()"#,
        )
        .unwrap_or_default();
    println!("[values]");
    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&values) {
        if let Some(o) = obj.as_object() {
            for (k, v) in o {
                let vstr = v.as_str().unwrap_or("");
                println!("  {k:30}  {vstr}");
            }
        }
    }
    println!("[probes]");
    // Pretty print
    if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&probes) {
        if let Some(arr) = arr.as_array() {
            for entry in arr {
                let k = entry.get("k").and_then(|v| v.as_str()).unwrap_or("");
                let count = entry.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                let type_ = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let present = entry
                    .get("present")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mark = if count > 0 { "✓" } else { "·" };
                println!("  {mark} {k:25} reads={count:4} type={type_} present={present}");
                if count > 0 {
                    if let Some(s) = entry.get("firstStack").and_then(|v| v.as_str()) {
                        if !s.is_empty() {
                            let short: String = s.chars().take(200).collect();
                            println!("      first-stack: {short}");
                        }
                    }
                }
            }
        }
    } else {
        println!("  (failed to parse probes JSON)");
        println!("  {probes}");
    }

    let diag2 = evloop
        .execute_script(
            r#"(function(){
                const d = globalThis.__wrapDiag || {};
                const clog = globalThis.__callLog;
                return JSON.stringify({
                    wrap: d,
                    callLogType: typeof clog,
                    callLogIsArray: Array.isArray(clog),
                    callLogLen: clog && clog.length,
                    callLogSample: (clog && clog.length ? clog[0] : null),
                    arrayPush: Array.prototype.push === [].push,
                });
            })()"#,
        )
        .unwrap_or_default();
    println!("[wrapDiag] {diag2}");

    // Ordered call log with arg previews — shows the shape of the sensor
    // VM's canvas/audio drawing, in the order calls happened. Iterate
    // individual indices because join('\n') returned empty (the array has
    // string entries but join misbehaves — possibly a realm/prototype issue).
    println!("[call-log]");
    let len: usize = evloop
        .execute_script(
            "(Array.isArray(globalThis.__callLog) ? globalThis.__callLog.length : 0)",
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    println!("  (len={len})");
    for i in 0..len.min(500) {
        let v = evloop
            .execute_script(&format!(
                "(globalThis.__callLog && globalThis.__callLog[{i}]) ? String(globalThis.__callLog[{i}]) : ''"
            ))
            .unwrap_or_default();
        if !v.is_empty() {
            println!("  {i:3}  {v}");
        }
    }

    // Function.prototype.toString calls — sensor VM may be hashing method
    // bodies to verify "[native code]". Print a summary.
    let fn_log_len: usize = evloop
        .execute_script(
            "(Array.isArray(globalThis.__fnToStringLog) ? globalThis.__fnToStringLog.length : 0)",
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    println!("[fn-toString-log] ({fn_log_len} total calls)");
    for i in 0..fn_log_len.min(30) {
        let entry = evloop
            .execute_script(&format!(
                r#"(function(){{
                    const e = globalThis.__fnToStringLog[{i}];
                    if (!e) return '';
                    return JSON.stringify(e.containsNative) + '|' + (e.name || '(anon)') + '|len=' + e.len + '|' + (e.preview || '').replace(/\n/g,' ');
                }})()"#
            ))
            .unwrap_or_default();
        if !entry.is_empty() {
            println!("  {i:3}  {entry}");
        }
    }

    // Any exceptions thrown inside instrumented methods (sensor VM may be
    // probing exception behavior as a fingerprint signal).
    let errors = evloop
        .execute_script("JSON.stringify(globalThis.__errorLog || [])")
        .unwrap_or_default();
    println!("[method-errors]");
    if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&errors) {
        if let Some(arr) = arr.as_array() {
            if arr.is_empty() {
                println!("  (none)");
            } else {
                for e in arr {
                    if let Some(s) = e.as_str() {
                        println!("  {s}");
                    }
                }
            }
        }
    }

    // Dump method call counts, sorted descending.
    let method_counts = evloop
        .execute_script(
            r#"JSON.stringify(
                Object.entries(globalThis.__methodProbes || {})
                    .map(([k, v]) => [k, v.count])
                    .sort((a, b) => b[1] - a[1])
            )"#,
        )
        .unwrap_or_default();
    println!("[method-probes]");
    if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&method_counts) {
        if let Some(arr) = arr.as_array() {
            for entry in arr {
                if let Some(a) = entry.as_array() {
                    let k = a.first().and_then(|v| v.as_str()).unwrap_or("");
                    let c = a.get(1).and_then(|v| v.as_u64()).unwrap_or(0);
                    let mark = if c > 0 { "✓" } else { "·" };
                    println!("  {mark} {k:45} calls={c}");
                }
            }
        }
    }
}
