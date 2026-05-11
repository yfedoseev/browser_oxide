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
        // navigator.userAgentData — must be present in Worker realm AND
        // return values consistent with the main thread. Per W6a research
        // (docs/W6a_DATADOME_PROBE_GAP_MATRIX_2026_05_10.md): DataDome's
        // tags.js v5.6.3 spawns a Worker that reads
        // `navigator.userAgentData ? .mobile : "NA"`. Main returns false,
        // worker previously returned "NA" — a cross-realm contradiction
        // DataDome scores against. Now both return false.
        const _osName = _p("os_name", "Windows");
        const _browserMajor = _p("browser_version", "147.0.7727.117").split(".")[0];
        const _browserFull = _p("browser_version", "147.0.7727.117");
        const _brands = [
            { brand: "Google Chrome", version: _browserMajor },
            { brand: "Not.A/Brand", version: "8" },
            { brand: "Chromium", version: _browserMajor },
        ];
        const _fullVersionList = [
            { brand: "Google Chrome", version: _browserFull },
            { brand: "Not.A/Brand", version: "8.0.0.0" },
            { brand: "Chromium", version: _browserFull },
        ];
        class WorkerNavigatorUAData {
            get brands() { return _brands.slice(); }
            get mobile() { return false; }
            get platform() { return _osName; }
            getHighEntropyValues(hints) {
                if (!Array.isArray(hints)) {
                    return Promise.reject(new TypeError(
                        "Failed to execute 'getHighEntropyValues' on 'NavigatorUAData': The provided value cannot be converted to a sequence."
                    ));
                }
                const out = { brands: _brands.slice(), mobile: false, platform: _osName };
                for (const h of hints) {
                    switch (h) {
                        case "architecture": out.architecture = _p("cpu_architecture", "x86"); break;
                        case "bitness": out.bitness = _p("cpu_bitness", "64"); break;
                        case "model": out.model = _p("ua_model", ""); break;
                        case "platformVersion": out.platformVersion = _p("platform_version", ""); break;
                        case "uaFullVersion": out.uaFullVersion = _browserFull; break;
                        case "fullVersionList": out.fullVersionList = _fullVersionList.slice(); break;
                        case "wow64": out.wow64 = _p("ua_wow64", "false") === "true"; break;
                        case "formFactors": out.formFactors = ["Desktop"]; break;
                        default: /* ignore unknown hints — Chrome silently drops */ break;
                    }
                }
                return Promise.resolve(out);
            }
            toJSON() { return { brands: _brands.slice(), mobile: false, platform: _osName }; }
        }
        Object.defineProperty(WorkerNavigatorUAData.prototype, Symbol.toStringTag, {
            value: "NavigatorUAData", configurable: true,
        });

        const workerNavigator = {
            userAgent: _p("user_agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36"),
            appVersion: _p("app_version", "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36"),
            language: _p("language", "en-US"),
            languages: _pJson("languages", ["en-US", "en"]),
            platform: _p("platform", "Win32"),
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
            userAgentData: new WorkerNavigatorUAData(),
        };
        Object.defineProperty(workerNavigator, Symbol.toStringTag, { value: "WorkerNavigator", configurable: true });
        self.navigator = workerNavigator;
    }

    // --- performance.now() humanization (matches window_bootstrap) ---
    if (!globalThis.performance) {
        globalThis.performance = {
            now() { return ops.op_perf_now_humanized(); },
        };
    } else {
        globalThis.performance.now = () => ops.op_perf_now_humanized();
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
            // Use Event constructor from interfaces_bootstrap
            const ev = new MessageEvent("message", { data });
            self.dispatchEvent(ev);
        }
    }
    // Prime the pump every 5ms. In a later pass this can be driven by the
    // event loop directly instead of setInterval.
    setInterval(drainOnce, 5);

    // --- importScripts: classic-worker synchronous script loader ---
    self.importScripts = function importScripts(...urls) {
        for (const raw of urls) {
            const url = String(raw);
            let source;
            if (url.startsWith("blob:")) {
                source = ops.op_blob_fetch_text(url);
                if (!source) throw new Error("importScripts failed to load blob URL " + url);
            } else if (url.startsWith("data:")) {
                const comma = url.indexOf(",");
                if (comma < 0) throw new Error("importScripts: malformed data URL");
                const meta = url.slice(5, comma);
                const body = url.slice(comma + 1);
                if (meta.endsWith(";base64")) {
                    source = atob(decodeURIComponent(body));
                } else {
                    source = decodeURIComponent(body);
                }
            } else if (url.startsWith("http://") || url.startsWith("https://")) {
                source = ops.op_worker_sync_fetch(url);
                if (!source) throw new Error("importScripts failed to load " + url);
            } else {
                throw new Error("importScripts: unsupported URL scheme: " + url);
            }
            (0, eval)(source);
        }
    };

    // MediaSource + MediaRecorder.isTypeSupported in Worker realm.
    // Kasada's `mrs` probe (W4a 2026-05-11) reads .isTypeSupported on
    // an undefined receiver in our Worker context — real Chrome has
    // MediaSource available in DedicatedWorker since Chrome 108.
    const _mediaTypes = new Set([
        "video/mp4", 'video/mp4;codecs="avc1.42E01E,mp4a.40.2"',
        'video/mp4;codecs="avc1.640028"', "video/webm",
        'video/webm;codecs="vp8,vorbis"', 'video/webm;codecs="vp9"',
        'video/webm;codecs="vp9,opus"', "audio/mp4",
        'audio/mp4;codecs="mp4a.40.2"', "audio/webm",
        'audio/webm;codecs=opus', 'audio/webm;codecs=vorbis',
    ]);
    if (!globalThis.MediaSource) {
        globalThis.MediaSource = class MediaSource {
            static isTypeSupported(type) {
                if (typeof type !== 'string') return false;
                if (_mediaTypes.has(type)) return true;
                const base = type.split(';')[0].trim();
                return _mediaTypes.has(base);
            }
        };
    }
    if (!globalThis.MediaRecorder) {
        globalThis.MediaRecorder = class MediaRecorder {
            static isTypeSupported(type) {
                if (typeof type !== 'string') return false;
                if (_mediaTypes.has(type)) return true;
                const base = type.split(';')[0].trim();
                return _mediaTypes.has(base);
            }
        };
    }
})(globalThis);
