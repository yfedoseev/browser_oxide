/**
 * Interface bootstrap — defines standard Web IDL classes.
 * Runs FIRST to ensure these globals are available to all other scripts.
 */
((globalThis) => {
    function _define(name, cls) {
        if (globalThis[name]) {
            return;
        }
        Object.defineProperty(cls.prototype, Symbol.toStringTag, {
            value: name, configurable: true
        });
        Object.defineProperty(globalThis, name, {
            value: cls, configurable: true, writable: true, enumerable: false
        });
    }

    _define("Navigator", class Navigator {});
    _define("Location", class Location {});
    _define("History", class History {});
    _define("Screen", class Screen {});
    _define("EventTarget", class EventTarget {});
    _define("Event", class Event { constructor(type, init) { this.type = type; } });
    _define("MessageEvent", class MessageEvent extends (globalThis.Event || class {}) {});
    _define("CustomEvent", class CustomEvent extends (globalThis.Event || class {}) {});
    _define("Performance", class Performance {});
    _define("PluginArray", class PluginArray {});
    _define("MimeTypeArray", class MimeTypeArray {});
    _define("Plugin", class Plugin {});
    _define("MimeType", class MimeType {});
    _define("NetworkInformation", class NetworkInformation {});
    _define("MediaDevices", class MediaDevices {});
    _define("StorageManager", class StorageManager {});
    _define("Bluetooth", class Bluetooth {});
    _define("PermissionStatus", class PermissionStatus {});
    _define("Permissions", class Permissions {});
    _define("ScreenOrientation", class ScreenOrientation {});

    // Chrome 147 constructor surface — anti-bot enumeration probes
    // (CreepJS features, fp-collect navigatorPrototype walk, BotD
    // distinctive_props) hash the names AND existence of the full
    // constructor list. Each missing entry is a "this UA claims Chrome
    // 147 but doesn't ship X" tell. Stubs with Illegal-constructor
    // semantics match real Chrome behaviour for most of these.
    function _illegalCtor(name) {
        // Class with a thrown constructor mirrors Chrome's "Illegal
        // constructor" pattern for interfaces only created internally.
        const C = class {
            constructor() {
                throw new TypeError("Failed to construct '" + name + "': Illegal constructor");
            }
        };
        Object.defineProperty(C, "name", { value: name, configurable: true });
        return C;
    }

    // CSS-related
    _define("CSSStyleSheet", _illegalCtor("CSSStyleSheet"));
    _define("CSSRule", _illegalCtor("CSSRule"));
    _define("CSSStyleRule", _illegalCtor("CSSStyleRule"));
    _define("Highlight", class Highlight { constructor(...ranges) { this._ranges = ranges; this.priority = 0; } });
    _define("HighlightRegistry", _illegalCtor("HighlightRegistry"));
    _define("CSSPseudoElement", _illegalCtor("CSSPseudoElement"));

    // DOM ranges and other interfaces
    _define("StaticRange", class StaticRange { constructor(init) { Object.assign(this, init || {}); } });
    _define("XMLSerializer", class XMLSerializer { serializeToString(_node) { return ""; } });
    _define("XSLTProcessor", class XSLTProcessor { importStylesheet() {} transformToFragment() { return null; } transformToDocument() { return null; } reset() {} setParameter() {} getParameter() {} removeParameter() {} clearParameters() {} });

    // Newer Chrome API constructors
    _define("EditContext", class EditContext { constructor(init) { Object.assign(this, init || {}); } });
    _define("CookieStore", _illegalCtor("CookieStore"));
    // WebTransport — real Chrome's constructor returns successfully; the
    // `ready` and `closed` Promises reject asynchronously when the URL is
    // unreachable. Throwing synchronously is detectable by sites that
    // probe `typeof new WebTransport(...)`. We provide an instance whose
    // Promises reject after a microtask so listeners fire normally.
    //
    // [SecureContext] — undefined on insecure contexts (Phase 7). The
    // constructor is only registered on https/wss/file/localhost.
    //
    // Spec: https://www.w3.org/TR/webtransport/#webtransport
    if (Deno.core.ops.op_is_secure_context())
    _define("WebTransport", class WebTransport {
        constructor(url, options) {
            this._url = String(url || "");
            this._options = options || {};
            // WebTransportError equivalent — Chrome's WebTransport rejects
            // with a DOMException-like { name, message }.
            const _err = (msg) => {
                try { return new DOMException(msg, "WebTransportError"); }
                catch (_e) { const e = new Error(msg); e.name = "WebTransportError"; return e; }
            };
            this.ready = Promise.reject(_err("WebTransport: connection failed"));
            // Suppress the unhandled-rejection warning from V8 — sites
            // that don't `.catch()` on `.ready` shouldn't blow up the
            // surrounding navigation.
            this.ready.catch(() => {});
            this.closed = Promise.reject(_err("WebTransport: connection closed"));
            this.closed.catch(() => {});
            this.draining = Promise.resolve();
            // Default Chrome shape per spec.
            this.reliability = "pending";
            this.congestionControl = "default";
            this.protocol = "";
            this.anticipatedConcurrentIncomingBidirectionalStreams = null;
            this.anticipatedConcurrentIncomingUnidirectionalStreams = null;
            // Stream readers — empty ReadableStreams mirror Chrome's
            // pre-connection shape.
            const _emptyStream = () => {
                try { return new ReadableStream({ start(_c) {} }); }
                catch (_e) { return null; }
            };
            this.incomingBidirectionalStreams = _emptyStream();
            this.incomingUnidirectionalStreams = _emptyStream();
            this.datagrams = {
                readable: _emptyStream(),
                writable: typeof WritableStream !== "undefined" ? new WritableStream() : null,
                maxDatagramSize: 1024,
                incomingHighWaterMark: 1,
                incomingMaxAge: NaN,
                outgoingHighWaterMark: 1,
                outgoingMaxAge: NaN,
            };
        }
        getStats() { return Promise.resolve({ smoothedRtt: 0, rttVariation: 0, minRtt: 0, packetsLost: 0, packetsReceived: 0, packetsSent: 0, numOutgoingStreamsCreated: 0, numIncomingStreamsCreated: 0, bytesSent: 0, bytesReceived: 0, estimatedSendRate: 0, datagrams: { droppedIncoming: 0, expiredIncoming: 0, expiredOutgoing: 0, lostOutgoing: 0 }, timestamp: 0 }); }
        close(_info) {}
        createBidirectionalStream(_options) { return Promise.reject(new Error("WebTransport: connection failed")); }
        createUnidirectionalStream(_options) { return Promise.reject(new Error("WebTransport: connection failed")); }
    });
    _define("LaunchQueue", _illegalCtor("LaunchQueue"));

    // File system access
    _define("FileSystemHandle", _illegalCtor("FileSystemHandle"));
    _define("FileSystemFileHandle", _illegalCtor("FileSystemFileHandle"));
    _define("FileSystemDirectoryHandle", _illegalCtor("FileSystemDirectoryHandle"));
    _define("FileSystemWritableFileStream", _illegalCtor("FileSystemWritableFileStream"));

    // Push / background fetch
    _define("PushManager", _illegalCtor("PushManager"));
    _define("PushSubscription", _illegalCtor("PushSubscription"));
    _define("BackgroundFetchManager", _illegalCtor("BackgroundFetchManager"));

    // Payments / presentation
    _define("PaymentRequest", class PaymentRequest { constructor(_methods, _details) {} });
    _define("PresentationConnection", _illegalCtor("PresentationConnection"));
    _define("Presentation", _illegalCtor("Presentation"));

    // Sensors (DeviceMotion / DeviceOrientation API surface)
    const _sensor = (n) => {
        const C = class {
            constructor(_opts) { throw new TypeError("Failed to construct '" + n + "': permission denied"); }
        };
        Object.defineProperty(C, "name", { value: n, configurable: true });
        return C;
    };
    _define("Sensor", _illegalCtor("Sensor"));
    _define("Accelerometer", _sensor("Accelerometer"));
    _define("LinearAccelerationSensor", _sensor("LinearAccelerationSensor"));
    _define("GravitySensor", _sensor("GravitySensor"));
    _define("Gyroscope", _sensor("Gyroscope"));
    _define("Magnetometer", _sensor("Magnetometer"));
    _define("OrientationSensor", _illegalCtor("OrientationSensor"));
    _define("AbsoluteOrientationSensor", _sensor("AbsoluteOrientationSensor"));
    _define("RelativeOrientationSensor", _sensor("RelativeOrientationSensor"));

    // Battery / Geolocation / WebXR
    _define("BatteryManager", _illegalCtor("BatteryManager"));
    _define("Geolocation", _illegalCtor("Geolocation"));
    _define("XRSystem", _illegalCtor("XRSystem"));
    _define("XRSession", _illegalCtor("XRSession"));

    // Streams (newer)
    if (typeof globalThis.TextDecoderStream === "undefined") {
        _define("TextDecoderStream", class TextDecoderStream {});
    }
    if (typeof globalThis.TextEncoderStream === "undefined") {
        _define("TextEncoderStream", class TextEncoderStream {});
    }

    // Privacy Sandbox / FedCM-adjacent (shape-only — present in Chrome 147 even
    // though Topics/Protected Audience were retired in 2026).
    _define("CredentialsContainer", _illegalCtor("CredentialsContainer"));
    _define("Credential", _illegalCtor("Credential"));
    _define("PasswordCredential", class PasswordCredential { constructor(_init) {} });
    _define("FederatedCredential", class FederatedCredential { constructor(_init) {} });

    // WebGL Constants
    globalThis.WebGLRenderingContext = globalThis.WebGLRenderingContext || {
        UNMASKED_VENDOR_WEBGL: 0x9245,
        UNMASKED_RENDERER_WEBGL: 0x9246,
    };
    globalThis.WebGL2RenderingContext = globalThis.WebGL2RenderingContext || {
        UNMASKED_VENDOR_WEBGL: 0x9245,
        UNMASKED_RENDERER_WEBGL: 0x9246,
    };

    // Common non-standard Chrome global. Real Chrome on regular pages
    // exposes window.chrome with `app` / `csi` / `loadTimes`, but
    // **NOT** `runtime` — `chrome.runtime` only exists inside extension
    // contexts. Detection libraries probe `'runtime' in chrome` to
    // confirm a non-extension page; the early-built shim here used to
    // include `runtime`, leaking that we look extension-shaped.
    //
    // The fuller chrome surface (csi, loadTimes, etc.) is set later in
    // window_bootstrap.js; we just need an early-shape with `app` so
    // any inline scripts that touch `chrome.app` before that bootstrap
    // runs don't crash.
    if (!globalThis.chrome) {
        globalThis.chrome = {
            app: { isInstalled: false },
        };
    }

    // Common modern APIs
    if (!globalThis.requestIdleCallback) {
        globalThis.requestIdleCallback = function(cb) {
            return setTimeout(() => {
                cb({ didTimeout: false, timeRemaining: () => 10 });
            }, 1);
        };
        globalThis.cancelIdleCallback = function(id) {
            clearTimeout(id);
        };
    }

    // __errors is an internal buffer for challenge debugging. Must not
    // leak to page scripts — a site that does `Object.keys(window)`
    // would see it and flag us. Kept non-enumerable and deleted by
    // cleanup_bootstrap.js.
    Object.defineProperty(globalThis, '__errors', {
        value: [], enumerable: false, configurable: true, writable: true,
    });
    globalThis.onerror = function(msg, url, line, col, error) {
        globalThis.__errors.push({
            msg: String(msg),
            url: String(url),
            line: line,
            col: col,
            stack: error ? String(error.stack) : ""
        });
        return false;
    };

})(globalThis);
