// worker_bootstrap.js — runs inside a dedicated Worker V8 isolate.
//
// Sets up the worker-side surface: `self`, postMessage, onmessage dispatch,
// close, and a navigator stub. A setInterval-driven poll loop drains
// parent→worker messages via op_worker_self_recv and fires onmessage events.

((globalThis) => {
    const ops = Deno.core.ops;
    const _boxide = globalThis.__boxide;

    // Helper: read from stealth profile or use default
    const _p = (key, fallback) => {
        if (ops.op_has_stealth_profile && ops.op_has_stealth_profile()) {
            const v = ops.op_get_profile_value(key);
            return v !== "" ? v : fallback;
        }
        return fallback;
    };
    const _pInt = (key, fallback) => {
        const v = _p(key, "");
        return v !== "" ? parseInt(v, 10) : fallback;
    };
    const _pJson = (key, fallback) => {
        const v = _p(key, "");
        if (v !== "") try { return JSON.parse(v); } catch {}
        return fallback;
    };

    // The global object doubles as WorkerGlobalScope / DedicatedWorkerGlobalScope / self.
    const self = globalThis;
    self.self = self;

    // --- Intl Sync (matches window_bootstrap) ---
    if (ops.op_has_stealth_profile && ops.op_has_stealth_profile()) {
        const profileTz = ops.op_get_profile_value("timezone") || "Europe/Moscow";
        const profileLocale = ops.op_get_profile_value("language") || "ru-RU";
        if (globalThis.Intl) {
            const _intlClasses = ['DateTimeFormat', 'NumberFormat', 'Collator', 'PluralRules', 'RelativeTimeFormat'];
            for (const klass of _intlClasses) {
                if (globalThis.Intl[klass]) {
                    const proto = globalThis.Intl[klass].prototype;
                    const origResolved = proto.resolvedOptions;
                    proto.resolvedOptions = function() {
                        const res = origResolved.call(this);
                        res.timeZone = profileTz || res.timeZone;
                        res.locale = profileLocale || res.locale;
                        return res;
                    };
                }
            }
        }
    }

    // --- WorkerNavigator (matches StealthProfile) ---
    if (!self.navigator) {
        const workerNavigator = {
            userAgent: _p("user_agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36"),
            appVersion: _p("app_version", "5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36"),
            language: _p("language", "en-US"),
            languages: _pJson("languages", ["en-US", "en"]),
            platform: _p("platform", "Linux x86_64"),
            onLine: true,
            cookieEnabled: true,
            hardwareConcurrency: _pInt("hardware_concurrency", 8),
            deviceMemory: _pInt("device_memory", 8),
            appName: "Netscape",
            product: "Gecko",
            productSub: _p("product_sub", "20030107"),
            vendor: _p("vendor", "Google Inc."),
            vendorSub: _p("vendor_sub", ""),
            doNotTrack: null,
            pdfViewerEnabled: _p("pdf_viewer_enabled", "true") === "true",
            webdriver: false,
        };
        Object.defineProperty(workerNavigator, Symbol.toStringTag, { value: "WorkerNavigator", configurable: true });
        self.navigator = workerNavigator;
    }

    // --- performance.memory jitter (matches window_bootstrap) ---
    if (globalThis.performance) {
        Object.defineProperty(globalThis.performance, 'memory', {
            get() {
                const jsHeapSizeLimit = 4294705152;
                const base = 10485760; // 10 MB
                const jitter = ((Date.now() * 0x9e3779b9) >>> 0) % 5000000;
                const totalJSHeapSize = base + jitter;
                const usedJSHeapSize = Math.floor(totalJSHeapSize * 0.85);
                return { jsHeapSizeLimit, totalJSHeapSize, usedJSHeapSize };
            },
            configurable: true,
            enumerable: true
        });
    }

    // --- atob / btoa spec-compliant fixes ---
    if (!self.atob) {
        self.atob = function atob(s) {
            if (arguments.length === 0) throw new TypeError("1 argument required");
            const input = String(s).replace(/[\t\n\f\r ]/g, "");
            if (input.length === 0) return "";
            const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let out = "";
            for (let i = 0; i < input.length; i += 4) {
                const a = chars.indexOf(input[i]), b = chars.indexOf(input[i+1]);
                const c = chars.indexOf(input[i+2]), d = chars.indexOf(input[i+3]);
                out += String.fromCharCode((a << 2) | (b >> 4));
                if (c !== -1 && c !== 64) out += String.fromCharCode(((b & 15) << 4) | (c >> 2));
                if (d !== -1 && d !== 64) out += String.fromCharCode(((c & 3) << 6) | d);
            }
            return out;
        };
    }
    if (!self.btoa) {
        self.btoa = function btoa(s) {
            if (arguments.length === 0) throw new TypeError("1 argument required");
            const str = String(s);
            for (let i = 0; i < str.length; i++) {
                if (str.charCodeAt(i) > 255) throw new DOMException("InvalidCharacterError");
            }
            const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let out = "";
            for (let i = 0; i < str.length; i += 3) {
                const a = str.charCodeAt(i), b = str.charCodeAt(i+1), c = str.charCodeAt(i+2);
                out += chars[a >> 2] + chars[((a & 3) << 4) | (b >> 4)];
                out += (isNaN(b) ? "=" : chars[((b & 15) << 2) | (c >> 6)]);
                out += (isNaN(c) ? "=" : chars[c & 63]);
            }
            return out;
        };
    }

    // --- EventTarget for the worker global scope ---
    const _wListeners = {};
    self.addEventListener = function addEventListener(type, fn) {
        if (!_wListeners[type]) _wListeners[type] = [];
        if (!_wListeners[type].includes(fn)) _wListeners[type].push(fn);
    };
    self.removeEventListener = function removeEventListener(type, fn) {
        if (_wListeners[type]) {
            _wListeners[type] = _wListeners[type].filter(f => f !== fn);
        }
    };
    self.dispatchEvent = function dispatchEvent(event) {
        const type = event && event.type;
        const arr = _wListeners[type] || [];
        for (const fn of arr.slice()) {
            try { fn.call(self, event); } catch (_) {}
        }
        const on = self['on' + type];
        if (typeof on === 'function') {
            try { on.call(self, event); } catch (_) {}
        }
        return true;
    };

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
                (_boxide &&
                    _boxide.serializeForWire &&
                    _boxide.serializeForWire(message)) ||
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
                _boxide && _boxide.deserializeFromWire;
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
