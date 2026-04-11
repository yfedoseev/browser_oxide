// worker_bootstrap.js — runs inside a dedicated Worker V8 isolate.
//
// Sets up the worker-side surface: `self`, postMessage, onmessage dispatch,
// close, and a navigator stub. A setInterval-driven poll loop drains
// parent→worker messages via op_worker_self_recv and fires onmessage events.

((globalThis) => {
    const ops = Deno.core.ops;

    // The global object doubles as WorkerGlobalScope / DedicatedWorkerGlobalScope / self.
    const self = globalThis;
    self.self = self;

    // --- Brand classes (for instanceof and Symbol.toStringTag) ---
    class WorkerGlobalScope {}
    class DedicatedWorkerGlobalScope extends WorkerGlobalScope {}
    Object.defineProperty(WorkerGlobalScope.prototype, Symbol.toStringTag, {
        value: "WorkerGlobalScope",
        configurable: true,
    });
    Object.defineProperty(DedicatedWorkerGlobalScope.prototype, Symbol.toStringTag, {
        value: "DedicatedWorkerGlobalScope",
        configurable: true,
    });
    globalThis.WorkerGlobalScope = WorkerGlobalScope;
    globalThis.DedicatedWorkerGlobalScope = DedicatedWorkerGlobalScope;
    Object.setPrototypeOf(self, DedicatedWorkerGlobalScope.prototype);

    // --- Minimal navigator for workers (WorkerNavigator) ---
    if (!self.navigator) {
        const workerNavigator = {
            userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
            language: "en-US",
            languages: ["en-US", "en"],
            platform: "MacIntel",
            onLine: true,
            hardwareConcurrency: 8,
            deviceMemory: 8,
            appName: "Netscape",
            appVersion: "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
            product: "Gecko",
            productSub: "20030107",
            vendor: "Google Inc.",
            vendorSub: "",
        };
        Object.defineProperty(workerNavigator, Symbol.toStringTag, {
            value: "WorkerNavigator",
            configurable: true,
        });
        self.navigator = workerNavigator;
    }

    // --- self.location (stubbed; worker typically gets parent's origin) ---
    if (!self.location) {
        self.location = {
            href: "about:blank",
            origin: "null",
            protocol: "about:",
            host: "",
            hostname: "",
            port: "",
            pathname: "blank",
            search: "",
            hash: "",
        };
    }

    // --- EventTarget-like listener registry for message/error events ---
    const _listeners = {
        message: [],
        messageerror: [],
        error: [],
    };
    self.addEventListener = function (type, listener, _options) {
        if (!_listeners[type]) _listeners[type] = [];
        _listeners[type].push(listener);
    };
    self.removeEventListener = function (type, listener) {
        const arr = _listeners[type];
        if (!arr) return;
        const i = arr.indexOf(listener);
        if (i >= 0) arr.splice(i, 1);
    };
    self.dispatchEvent = function (event) {
        const arr = _listeners[event && event.type];
        if (arr) {
            for (const fn of arr.slice()) {
                try { fn.call(self, event); } catch (e) { /* swallow */ }
            }
        }
        // Also call the `on<type>` property handler.
        const on = self["on" + (event && event.type)];
        if (typeof on === "function") {
            try { on.call(self, event); } catch (e) { /* swallow */ }
        }
        return true;
    };
    self.onmessage = null;
    self.onmessageerror = null;
    self.onerror = null;

    // --- atob / btoa (shared with window_bootstrap's implementation) ---
    // Workers have these globals per WHATWG spec; some classic-worker
    // scripts (including our own `importScripts` data-URL loader) rely
    // on them, so we install the same minimal base64 helpers the main
    // thread uses rather than deferring to a dedicated bootstrap.
    if (!self.atob) {
        self.atob = function (s) {
            const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let out = "";
            s = String(s).replace(/[^A-Za-z0-9+/]/g, "");
            for (let i = 0; i < s.length; i += 4) {
                const a = chars.indexOf(s[i]), b = chars.indexOf(s[i + 1]);
                const c = chars.indexOf(s[i + 2]), d = chars.indexOf(s[i + 3]);
                out += String.fromCharCode((a << 2) | (b >> 4));
                if (c !== -1) out += String.fromCharCode(((b & 15) << 4) | (c >> 2));
                if (d !== -1) out += String.fromCharCode(((c & 3) << 6) | d);
            }
            return out;
        };
    }
    if (!self.btoa) {
        self.btoa = function (s) {
            const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let out = "";
            const str = String(s);
            for (let i = 0; i < str.length; i += 3) {
                const a = str.charCodeAt(i),
                    b = str.charCodeAt(i + 1),
                    c = str.charCodeAt(i + 2);
                out += chars[a >> 2] + chars[((a & 3) << 4) | (b >> 4)];
                out += isNaN(b) ? "=" : chars[((b & 15) << 2) | (c >> 6)];
                out += isNaN(c) ? "=" : chars[c & 63];
            }
            return out;
        };
    }

    // --- postMessage: send a message to the parent thread ---
    self.postMessage = function (message, transfer) {
        // Validate transferables (same shape as main thread).
        const transferList = Array.isArray(transfer) ? transfer : [];
        for (const t of transferList) {
            if (
                t !== null &&
                !(t instanceof ArrayBuffer) &&
                !(ArrayBuffer.isView && ArrayBuffer.isView(t))
            ) {
                throw new TypeError(
                    "postMessage: transferable must be an ArrayBuffer or view"
                );
            }
        }
        let wire;
        try {
            wire =
                (globalThis.__boxide &&
                    globalThis.__boxide.serializeForWire &&
                    globalThis.__boxide.serializeForWire(message)) ||
                message;
        } catch (e) {
            // DataCloneError — propagate.
            throw e;
        }
        let payload;
        try {
            payload = JSON.stringify({ data: wire });
        } catch (_e) {
            payload = JSON.stringify({ data: null });
        }
        ops.op_worker_self_post(payload);
    };

    // --- close: terminate this worker ---
    // Terminating a worker from inside is rare; the parent handles cleanup.
    self.close = function () {
        // No-op here; parent.terminate() drives real shutdown via AtomicBool.
    };

    // --- Poll loop: drain parent→worker messages and fire message events ---
    function drainOnce() {
        while (true) {
            const s = ops.op_worker_self_recv();
            if (!s) break;
            let payload;
            try {
                payload = JSON.parse(s);
            } catch (e) {
                continue;
            }
            const deserializer =
                globalThis.__boxide && globalThis.__boxide.deserializeFromWire;
            const data = deserializer
                ? deserializer(payload && payload.data)
                : payload && payload.data;
            const event = {
                type: "message",
                data,
                origin: "",
                lastEventId: "",
                source: null,
                ports: [],
                timeStamp: Date.now(),
            };
            self.dispatchEvent(event);
        }
    }
    // Prime the pump every 5ms. In a later pass this can be driven by the
    // event loop directly instead of setInterval.
    setInterval(drainOnce, 5);

    // --- importScripts: classic-worker synchronous script loader ---
    //
    // Per spec, `importScripts(...urls)` blocks the worker thread, fetches
    // each URL in order, and eval'd each result in the worker global scope.
    // Supports `http(s):`, `blob:`, and `data:` URLs. Errors propagate to
    // the caller as thrown `NetworkError`-shaped exceptions.
    self.importScripts = function importScripts(...urls) {
        for (const raw of urls) {
            const url = String(raw);
            let source;
            if (url.startsWith("blob:")) {
                // Fast path: blob registry is shared across threads via the
                // process-global BlobRegistry.
                source = ops.op_blob_fetch_text(url);
                if (!source) {
                    throw new Error(
                        "importScripts failed to load blob URL " + url
                    );
                }
            } else if (url.startsWith("data:")) {
                // RFC 2397 data URL. Parse `data:[<mediatype>][;base64],<data>`.
                const comma = url.indexOf(",");
                if (comma < 0) {
                    throw new Error("importScripts: malformed data URL");
                }
                const meta = url.slice(5, comma);
                const body = url.slice(comma + 1);
                if (meta.endsWith(";base64")) {
                    source = atob(decodeURIComponent(body));
                } else {
                    source = decodeURIComponent(body);
                }
            } else if (url.startsWith("http://") || url.startsWith("https://")) {
                // Synchronous HTTP fetch from the worker thread. Implemented
                // as an op that uses `block_on` on the worker's own tokio
                // runtime so this call is sync from the worker's
                // single-threaded V8 perspective.
                source = ops.op_worker_sync_fetch(url);
                if (!source) {
                    throw new Error(
                        "importScripts failed to load " + url
                    );
                }
            } else {
                throw new Error(
                    "importScripts: unsupported URL scheme: " + url
                );
            }
            // Evaluate in the worker global scope. `(0, eval)` forces
            // indirect eval so the source runs at global scope rather
            // than in the caller's scope chain.
            (0, eval)(source);
        }
    };
})(globalThis);
