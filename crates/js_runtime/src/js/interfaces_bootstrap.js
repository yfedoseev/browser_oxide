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


    // ============================================================
    // Phase 7 — bulk-register Chrome 147 constructors + event handlers
    // + window data props to close the ownPropertyNames structural
    // gap (372 → 980 names). Most constructors are not directly
    // constructible in Chrome (e.g., `new HTMLAnchorElement()` throws),
    // so we register them as _illegalCtor stubs to match. Anti-bot
    // scripts compare the global namespace size.
    // ============================================================

    // ---- Constructors (498) ----
    _define("AbstractRange", _illegalCtor("AbstractRange"));
    _define("AnalyserNode", _illegalCtor("AnalyserNode"));
    _define("Animation", _illegalCtor("Animation"));
    _define("AnimationEffect", _illegalCtor("AnimationEffect"));
    _define("AnimationPlaybackEvent", _illegalCtor("AnimationPlaybackEvent"));
    _define("AnimationTimeline", _illegalCtor("AnimationTimeline"));
    _define("AnimationTrigger", _illegalCtor("AnimationTrigger"));
    _define("AsyncDisposableStack", _illegalCtor("AsyncDisposableStack"));
    _define("Attr", _illegalCtor("Attr"));
    _define("Audio", _illegalCtor("Audio"));
    _define("AudioBuffer", _illegalCtor("AudioBuffer"));
    _define("AudioBufferSourceNode", _illegalCtor("AudioBufferSourceNode"));
    _define("AudioData", _illegalCtor("AudioData"));
    _define("AudioDestinationNode", _illegalCtor("AudioDestinationNode"));
    _define("AudioListener", _illegalCtor("AudioListener"));
    _define("AudioNode", _illegalCtor("AudioNode"));
    _define("AudioParam", _illegalCtor("AudioParam"));
    _define("AudioParamMap", _illegalCtor("AudioParamMap"));
    _define("AudioProcessingEvent", _illegalCtor("AudioProcessingEvent"));
    _define("AudioScheduledSourceNode", _illegalCtor("AudioScheduledSourceNode"));
    _define("AudioSinkInfo", _illegalCtor("AudioSinkInfo"));
    _define("AudioWorkletNode", _illegalCtor("AudioWorkletNode"));
    _define("BackgroundFetchRecord", _illegalCtor("BackgroundFetchRecord"));
    _define("BackgroundFetchRegistration", _illegalCtor("BackgroundFetchRegistration"));
    _define("BarProp", _illegalCtor("BarProp"));
    _define("BaseAudioContext", _illegalCtor("BaseAudioContext"));
    _define("BeforeInstallPromptEvent", _illegalCtor("BeforeInstallPromptEvent"));
    _define("BiquadFilterNode", _illegalCtor("BiquadFilterNode"));
    _define("BlobEvent", _illegalCtor("BlobEvent"));
    _define("BluetoothUUID", _illegalCtor("BluetoothUUID"));
    _define("BrowserCaptureMediaStreamTrack", _illegalCtor("BrowserCaptureMediaStreamTrack"));
    _define("ByteLengthQueuingStrategy", _illegalCtor("ByteLengthQueuingStrategy"));
    _define("CDATASection", _illegalCtor("CDATASection"));
    _define("CSPViolationReportBody", _illegalCtor("CSPViolationReportBody"));
    _define("CSSAnimation", _illegalCtor("CSSAnimation"));
    _define("CSSConditionRule", _illegalCtor("CSSConditionRule"));
    _define("CSSContainerRule", _illegalCtor("CSSContainerRule"));
    _define("CSSCounterStyleRule", _illegalCtor("CSSCounterStyleRule"));
    _define("CSSFontFaceRule", _illegalCtor("CSSFontFaceRule"));
    _define("CSSFontFeatureValuesRule", _illegalCtor("CSSFontFeatureValuesRule"));
    _define("CSSFontPaletteValuesRule", _illegalCtor("CSSFontPaletteValuesRule"));
    _define("CSSFunctionDeclarations", _illegalCtor("CSSFunctionDeclarations"));
    _define("CSSFunctionDescriptors", _illegalCtor("CSSFunctionDescriptors"));
    _define("CSSFunctionRule", _illegalCtor("CSSFunctionRule"));
    _define("CSSGroupingRule", _illegalCtor("CSSGroupingRule"));
    _define("CSSImageValue", _illegalCtor("CSSImageValue"));
    _define("CSSImportRule", _illegalCtor("CSSImportRule"));
    _define("CSSKeyframeRule", _illegalCtor("CSSKeyframeRule"));
    _define("CSSKeyframesRule", _illegalCtor("CSSKeyframesRule"));
    _define("CSSKeywordValue", _illegalCtor("CSSKeywordValue"));
    _define("CSSLayerBlockRule", _illegalCtor("CSSLayerBlockRule"));
    _define("CSSLayerStatementRule", _illegalCtor("CSSLayerStatementRule"));
    _define("CSSMarginRule", _illegalCtor("CSSMarginRule"));
    _define("CSSMathClamp", _illegalCtor("CSSMathClamp"));
    _define("CSSMathInvert", _illegalCtor("CSSMathInvert"));
    _define("CSSMathMax", _illegalCtor("CSSMathMax"));
    _define("CSSMathMin", _illegalCtor("CSSMathMin"));
    _define("CSSMathNegate", _illegalCtor("CSSMathNegate"));
    _define("CSSMathProduct", _illegalCtor("CSSMathProduct"));
    _define("CSSMathSum", _illegalCtor("CSSMathSum"));
    _define("CSSMathValue", _illegalCtor("CSSMathValue"));
    _define("CSSMatrixComponent", _illegalCtor("CSSMatrixComponent"));
    _define("CSSMediaRule", _illegalCtor("CSSMediaRule"));
    _define("CSSNamespaceRule", _illegalCtor("CSSNamespaceRule"));
    _define("CSSNestedDeclarations", _illegalCtor("CSSNestedDeclarations"));
    _define("CSSNumericArray", _illegalCtor("CSSNumericArray"));
    _define("CSSNumericValue", _illegalCtor("CSSNumericValue"));
    _define("CSSPageRule", _illegalCtor("CSSPageRule"));
    _define("CSSPerspective", _illegalCtor("CSSPerspective"));
    _define("CSSPositionTryDescriptors", _illegalCtor("CSSPositionTryDescriptors"));
    _define("CSSPositionTryRule", _illegalCtor("CSSPositionTryRule"));
    _define("CSSPositionValue", _illegalCtor("CSSPositionValue"));
    _define("CSSPropertyRule", _illegalCtor("CSSPropertyRule"));
    _define("CSSRotate", _illegalCtor("CSSRotate"));
    _define("CSSRuleList", _illegalCtor("CSSRuleList"));
    _define("CSSScale", _illegalCtor("CSSScale"));
    _define("CSSScopeRule", _illegalCtor("CSSScopeRule"));
    _define("CSSSkew", _illegalCtor("CSSSkew"));
    _define("CSSSkewX", _illegalCtor("CSSSkewX"));
    _define("CSSSkewY", _illegalCtor("CSSSkewY"));
    _define("CSSStartingStyleRule", _illegalCtor("CSSStartingStyleRule"));
    _define("CSSStyleDeclaration", _illegalCtor("CSSStyleDeclaration"));
    _define("CSSStyleValue", _illegalCtor("CSSStyleValue"));
    _define("CSSSupportsRule", _illegalCtor("CSSSupportsRule"));
    _define("CSSTransformComponent", _illegalCtor("CSSTransformComponent"));
    _define("CSSTransformValue", _illegalCtor("CSSTransformValue"));
    _define("CSSTransition", _illegalCtor("CSSTransition"));
    _define("CSSTranslate", _illegalCtor("CSSTranslate"));
    _define("CSSUnitValue", _illegalCtor("CSSUnitValue"));
    _define("CSSUnparsedValue", _illegalCtor("CSSUnparsedValue"));
    _define("CSSVariableReferenceValue", _illegalCtor("CSSVariableReferenceValue"));
    _define("CSSViewTransitionRule", _illegalCtor("CSSViewTransitionRule"));
    _define("CanvasCaptureMediaStreamTrack", _illegalCtor("CanvasCaptureMediaStreamTrack"));
    _define("CanvasGradient", _illegalCtor("CanvasGradient"));
    _define("CanvasPattern", _illegalCtor("CanvasPattern"));
    _define("CaretPosition", _illegalCtor("CaretPosition"));
    _define("ChannelMergerNode", _illegalCtor("ChannelMergerNode"));
    _define("ChannelSplitterNode", _illegalCtor("ChannelSplitterNode"));
    _define("ChapterInformation", _illegalCtor("ChapterInformation"));
    _define("CharacterBoundsUpdateEvent", _illegalCtor("CharacterBoundsUpdateEvent"));
    _define("CharacterData", _illegalCtor("CharacterData"));
    _define("CloseWatcher", _illegalCtor("CloseWatcher"));
    _define("CommandEvent", _illegalCtor("CommandEvent"));
    _define("CompositionEvent", _illegalCtor("CompositionEvent"));
    _define("ConstantSourceNode", _illegalCtor("ConstantSourceNode"));
    _define("ContentVisibilityAutoStateChangeEvent", _illegalCtor("ContentVisibilityAutoStateChangeEvent"));
    _define("ConvolverNode", _illegalCtor("ConvolverNode"));
    _define("CountQueuingStrategy", _illegalCtor("CountQueuingStrategy"));
    _define("CrashReportContext", _illegalCtor("CrashReportContext"));
    _define("CropTarget", _illegalCtor("CropTarget"));
    _define("CustomStateSet", _illegalCtor("CustomStateSet"));
    _define("DOMError", _illegalCtor("DOMError"));
    _define("DOMImplementation", _illegalCtor("DOMImplementation"));
    _define("DOMQuad", _illegalCtor("DOMQuad"));
    _define("DOMRectList", _illegalCtor("DOMRectList"));
    _define("DOMStringList", _illegalCtor("DOMStringList"));
    _define("DOMStringMap", _illegalCtor("DOMStringMap"));
    _define("DataTransfer", _illegalCtor("DataTransfer"));
    _define("DataTransferItem", _illegalCtor("DataTransferItem"));
    _define("DataTransferItemList", _illegalCtor("DataTransferItemList"));
    _define("DelayNode", _illegalCtor("DelayNode"));
    _define("DelegatedInkTrailPresenter", _illegalCtor("DelegatedInkTrailPresenter"));
    _define("DisposableStack", _illegalCtor("DisposableStack"));
    _define("DocumentPictureInPictureEvent", _illegalCtor("DocumentPictureInPictureEvent"));
    _define("DocumentTimeline", _illegalCtor("DocumentTimeline"));
    _define("DocumentType", _illegalCtor("DocumentType"));
    _define("DynamicsCompressorNode", _illegalCtor("DynamicsCompressorNode"));
    _define("ElementInternals", _illegalCtor("ElementInternals"));
    _define("EncodedAudioChunk", _illegalCtor("EncodedAudioChunk"));
    _define("EncodedVideoChunk", _illegalCtor("EncodedVideoChunk"));
    _define("External", _illegalCtor("External"));
    _define("FeaturePolicy", _illegalCtor("FeaturePolicy"));
    _define("Fence", _illegalCtor("Fence"));
    _define("FencedFrameConfig", _illegalCtor("FencedFrameConfig"));
    _define("FileList", _illegalCtor("FileList"));
    _define("FontFaceSetLoadEvent", _illegalCtor("FontFaceSetLoadEvent"));
    _define("FormDataEvent", _illegalCtor("FormDataEvent"));
    _define("FragmentDirective", _illegalCtor("FragmentDirective"));
    _define("GainNode", _illegalCtor("GainNode"));
    _define("Gamepad", _illegalCtor("Gamepad"));
    _define("GamepadButton", _illegalCtor("GamepadButton"));
    _define("GamepadEvent", _illegalCtor("GamepadEvent"));
    _define("GamepadHapticActuator", _illegalCtor("GamepadHapticActuator"));
    _define("GeolocationCoordinates", _illegalCtor("GeolocationCoordinates"));
    _define("GeolocationPosition", _illegalCtor("GeolocationPosition"));
    _define("GeolocationPositionError", _illegalCtor("GeolocationPositionError"));
    _define("HTMLAllCollection", _illegalCtor("HTMLAllCollection"));
    _define("HTMLAreaElement", _illegalCtor("HTMLAreaElement"));
    _define("HTMLBRElement", _illegalCtor("HTMLBRElement"));
    _define("HTMLBaseElement", _illegalCtor("HTMLBaseElement"));
    _define("HTMLCollection", _illegalCtor("HTMLCollection"));
    _define("HTMLDListElement", _illegalCtor("HTMLDListElement"));
    _define("HTMLDataElement", _illegalCtor("HTMLDataElement"));
    _define("HTMLDataListElement", _illegalCtor("HTMLDataListElement"));
    _define("HTMLDetailsElement", _illegalCtor("HTMLDetailsElement"));
    _define("HTMLDialogElement", _illegalCtor("HTMLDialogElement"));
    _define("HTMLDirectoryElement", _illegalCtor("HTMLDirectoryElement"));
    _define("HTMLEmbedElement", _illegalCtor("HTMLEmbedElement"));
    _define("HTMLFencedFrameElement", _illegalCtor("HTMLFencedFrameElement"));
    _define("HTMLFieldSetElement", _illegalCtor("HTMLFieldSetElement"));
    _define("HTMLFontElement", _illegalCtor("HTMLFontElement"));
    _define("HTMLFormControlsCollection", _illegalCtor("HTMLFormControlsCollection"));
    _define("HTMLFrameElement", _illegalCtor("HTMLFrameElement"));
    _define("HTMLFrameSetElement", _illegalCtor("HTMLFrameSetElement"));
    _define("HTMLGeolocationElement", _illegalCtor("HTMLGeolocationElement"));
    _define("HTMLHRElement", _illegalCtor("HTMLHRElement"));
    _define("HTMLLegendElement", _illegalCtor("HTMLLegendElement"));
    _define("HTMLMapElement", _illegalCtor("HTMLMapElement"));
    _define("HTMLMarqueeElement", _illegalCtor("HTMLMarqueeElement"));
    _define("HTMLMediaElement", _illegalCtor("HTMLMediaElement"));
    _define("HTMLMenuElement", _illegalCtor("HTMLMenuElement"));
    _define("HTMLMeterElement", _illegalCtor("HTMLMeterElement"));
    _define("HTMLModElement", _illegalCtor("HTMLModElement"));
    _define("HTMLObjectElement", _illegalCtor("HTMLObjectElement"));
    _define("HTMLOptGroupElement", _illegalCtor("HTMLOptGroupElement"));
    _define("HTMLOptionsCollection", _illegalCtor("HTMLOptionsCollection"));
    _define("HTMLOutputElement", _illegalCtor("HTMLOutputElement"));
    _define("HTMLParamElement", _illegalCtor("HTMLParamElement"));
    _define("HTMLPictureElement", _illegalCtor("HTMLPictureElement"));
    _define("HTMLProgressElement", _illegalCtor("HTMLProgressElement"));
    _define("HTMLSelectedContentElement", _illegalCtor("HTMLSelectedContentElement"));
    _define("HTMLSlotElement", _illegalCtor("HTMLSlotElement"));
    _define("HTMLSourceElement", _illegalCtor("HTMLSourceElement"));
    _define("HTMLTableCaptionElement", _illegalCtor("HTMLTableCaptionElement"));
    _define("HTMLTableColElement", _illegalCtor("HTMLTableColElement"));
    _define("HTMLTimeElement", _illegalCtor("HTMLTimeElement"));
    _define("HTMLTitleElement", _illegalCtor("HTMLTitleElement"));
    _define("HTMLTrackElement", _illegalCtor("HTMLTrackElement"));
    _define("HTMLUnknownElement", _illegalCtor("HTMLUnknownElement"));
    _define("IDBCursorWithValue", _illegalCtor("IDBCursorWithValue"));
    _define("IDBIndex", _illegalCtor("IDBIndex"));
    _define("IDBRecord", _illegalCtor("IDBRecord"));
    _define("IDBVersionChangeEvent", _illegalCtor("IDBVersionChangeEvent"));
    _define("IIRFilterNode", _illegalCtor("IIRFilterNode"));
    _define("IdleDeadline", _illegalCtor("IdleDeadline"));
    _define("ImageBitmapRenderingContext", _illegalCtor("ImageBitmapRenderingContext"));
    _define("ImageData", _illegalCtor("ImageData"));
    _define("Ink", _illegalCtor("Ink"));
    _define("InputDeviceInfo", _illegalCtor("InputDeviceInfo"));
    _define("IntegrityViolationReportBody", _illegalCtor("IntegrityViolationReportBody"));
    _define("InterestEvent", _illegalCtor("InterestEvent"));
    _define("IntersectionObserverEntry", _illegalCtor("IntersectionObserverEntry"));
    _define("KeyframeEffect", _illegalCtor("KeyframeEffect"));
    _define("LargestContentfulPaint", _illegalCtor("LargestContentfulPaint"));
    _define("LaunchParams", _illegalCtor("LaunchParams"));
    _define("LayoutShift", _illegalCtor("LayoutShift"));
    _define("LayoutShiftAttribution", _illegalCtor("LayoutShiftAttribution"));
    _define("MathMLElement", _illegalCtor("MathMLElement"));
    _define("MediaCapabilities", _illegalCtor("MediaCapabilities"));
    _define("MediaElementAudioSourceNode", _illegalCtor("MediaElementAudioSourceNode"));
    _define("MediaEncryptedEvent", _illegalCtor("MediaEncryptedEvent"));
    _define("MediaError", _illegalCtor("MediaError"));
    _define("MediaList", _illegalCtor("MediaList"));
    _define("MediaQueryListEvent", _illegalCtor("MediaQueryListEvent"));
    _define("MediaRecorder", _illegalCtor("MediaRecorder"));
    _define("MediaSourceHandle", _illegalCtor("MediaSourceHandle"));
    _define("MediaStream", _illegalCtor("MediaStream"));
    _define("MediaStreamAudioDestinationNode", _illegalCtor("MediaStreamAudioDestinationNode"));
    _define("MediaStreamAudioSourceNode", _illegalCtor("MediaStreamAudioSourceNode"));
    _define("MediaStreamEvent", _illegalCtor("MediaStreamEvent"));
    _define("MediaStreamTrack", _illegalCtor("MediaStreamTrack"));
    _define("MediaStreamTrackAudioStats", _illegalCtor("MediaStreamTrackAudioStats"));
    _define("MediaStreamTrackEvent", _illegalCtor("MediaStreamTrackEvent"));
    _define("MediaStreamTrackGenerator", _illegalCtor("MediaStreamTrackGenerator"));
    _define("MediaStreamTrackProcessor", _illegalCtor("MediaStreamTrackProcessor"));
    _define("MediaStreamTrackVideoStats", _illegalCtor("MediaStreamTrackVideoStats"));
    _define("NamedNodeMap", _illegalCtor("NamedNodeMap"));
    _define("NavigateEvent", _illegalCtor("NavigateEvent"));
    _define("Navigation", _illegalCtor("Navigation"));
    _define("NavigationActivation", _illegalCtor("NavigationActivation"));
    _define("NavigationCurrentEntryChangeEvent", _illegalCtor("NavigationCurrentEntryChangeEvent"));
    _define("NavigationDestination", _illegalCtor("NavigationDestination"));
    _define("NavigationHistoryEntry", _illegalCtor("NavigationHistoryEntry"));
    _define("NavigationPrecommitController", _illegalCtor("NavigationPrecommitController"));
    _define("NavigationTransition", _illegalCtor("NavigationTransition"));
    _define("NavigatorUAData", _illegalCtor("NavigatorUAData"));
    _define("NodeFilter", _illegalCtor("NodeFilter"));
    _define("NodeIterator", _illegalCtor("NodeIterator"));
    _define("NotRestoredReasonDetails", _illegalCtor("NotRestoredReasonDetails"));
    _define("NotRestoredReasons", _illegalCtor("NotRestoredReasons"));
    _define("Observable", _illegalCtor("Observable"));
    _define("OfflineAudioCompletionEvent", _illegalCtor("OfflineAudioCompletionEvent"));
    _define("OffscreenCanvasRenderingContext2D", _illegalCtor("OffscreenCanvasRenderingContext2D"));
    _define("Option", _illegalCtor("Option"));
    _define("Origin", _illegalCtor("Origin"));
    _define("OscillatorNode", _illegalCtor("OscillatorNode"));
    _define("OverconstrainedError", _illegalCtor("OverconstrainedError"));
    _define("PageRevealEvent", _illegalCtor("PageRevealEvent"));
    _define("PageSwapEvent", _illegalCtor("PageSwapEvent"));
    _define("PannerNode", _illegalCtor("PannerNode"));
    _define("Path2D", _illegalCtor("Path2D"));
    _define("PerformanceElementTiming", _illegalCtor("PerformanceElementTiming"));
    _define("PerformanceEventTiming", _illegalCtor("PerformanceEventTiming"));
    _define("PerformanceLongAnimationFrameTiming", _illegalCtor("PerformanceLongAnimationFrameTiming"));
    _define("PerformanceLongTaskTiming", _illegalCtor("PerformanceLongTaskTiming"));
    _define("PerformanceMark", _illegalCtor("PerformanceMark"));
    _define("PerformanceMeasure", _illegalCtor("PerformanceMeasure"));
    _define("PerformanceNavigation", _illegalCtor("PerformanceNavigation"));
    _define("PerformanceNavigationTiming", _illegalCtor("PerformanceNavigationTiming"));
    _define("PerformanceObserverEntryList", _illegalCtor("PerformanceObserverEntryList"));
    _define("PerformancePaintTiming", _illegalCtor("PerformancePaintTiming"));
    _define("PerformanceResourceTiming", _illegalCtor("PerformanceResourceTiming"));
    _define("PerformanceScriptTiming", _illegalCtor("PerformanceScriptTiming"));
    _define("PerformanceServerTiming", _illegalCtor("PerformanceServerTiming"));
    _define("PerformanceTiming", _illegalCtor("PerformanceTiming"));
    _define("PerformanceTimingConfidence", _illegalCtor("PerformanceTimingConfidence"));
    _define("PeriodicSyncManager", _illegalCtor("PeriodicSyncManager"));
    _define("PeriodicWave", _illegalCtor("PeriodicWave"));
    _define("PictureInPictureEvent", _illegalCtor("PictureInPictureEvent"));
    _define("PictureInPictureWindow", _illegalCtor("PictureInPictureWindow"));
    _define("ProcessingInstruction", _illegalCtor("ProcessingInstruction"));
    _define("Profiler", _illegalCtor("Profiler"));
    _define("PromiseRejectionEvent", _illegalCtor("PromiseRejectionEvent"));
    _define("PushSubscriptionOptions", _illegalCtor("PushSubscriptionOptions"));
    _define("QuotaExceededError", _illegalCtor("QuotaExceededError"));
    _define("RTCCertificate", _illegalCtor("RTCCertificate"));
    _define("RTCDTMFSender", _illegalCtor("RTCDTMFSender"));
    _define("RTCDTMFToneChangeEvent", _illegalCtor("RTCDTMFToneChangeEvent"));
    _define("RTCDataChannel", _illegalCtor("RTCDataChannel"));
    _define("RTCDataChannelEvent", _illegalCtor("RTCDataChannelEvent"));
    _define("RTCDtlsTransport", _illegalCtor("RTCDtlsTransport"));
    _define("RTCEncodedAudioFrame", _illegalCtor("RTCEncodedAudioFrame"));
    _define("RTCEncodedVideoFrame", _illegalCtor("RTCEncodedVideoFrame"));
    _define("RTCError", _illegalCtor("RTCError"));
    _define("RTCErrorEvent", _illegalCtor("RTCErrorEvent"));
    _define("RTCIceTransport", _illegalCtor("RTCIceTransport"));
    _define("RTCPeerConnectionIceErrorEvent", _illegalCtor("RTCPeerConnectionIceErrorEvent"));
    _define("RTCPeerConnectionIceEvent", _illegalCtor("RTCPeerConnectionIceEvent"));
    _define("RTCRtpReceiver", _illegalCtor("RTCRtpReceiver"));
    _define("RTCRtpScriptTransform", _illegalCtor("RTCRtpScriptTransform"));
    _define("RTCRtpSender", _illegalCtor("RTCRtpSender"));
    _define("RTCRtpTransceiver", _illegalCtor("RTCRtpTransceiver"));
    _define("RTCSctpTransport", _illegalCtor("RTCSctpTransport"));
    _define("RTCStatsReport", _illegalCtor("RTCStatsReport"));
    _define("RTCTrackEvent", _illegalCtor("RTCTrackEvent"));
    _define("RadioNodeList", _illegalCtor("RadioNodeList"));
    _define("ReadableByteStreamController", _illegalCtor("ReadableByteStreamController"));
    _define("ReadableStreamBYOBReader", _illegalCtor("ReadableStreamBYOBReader"));
    _define("ReadableStreamBYOBRequest", _illegalCtor("ReadableStreamBYOBRequest"));
    _define("RemotePlayback", _illegalCtor("RemotePlayback"));
    _define("ReportBody", _illegalCtor("ReportBody"));
    _define("ResizeObserverEntry", _illegalCtor("ResizeObserverEntry"));
    _define("ResizeObserverSize", _illegalCtor("ResizeObserverSize"));
    _define("RestrictionTarget", _illegalCtor("RestrictionTarget"));
    _define("SVGAElement", _illegalCtor("SVGAElement"));
    _define("SVGAngle", _illegalCtor("SVGAngle"));
    _define("SVGAnimateElement", _illegalCtor("SVGAnimateElement"));
    _define("SVGAnimateMotionElement", _illegalCtor("SVGAnimateMotionElement"));
    _define("SVGAnimateTransformElement", _illegalCtor("SVGAnimateTransformElement"));
    _define("SVGAnimatedAngle", _illegalCtor("SVGAnimatedAngle"));
    _define("SVGAnimatedBoolean", _illegalCtor("SVGAnimatedBoolean"));
    _define("SVGAnimatedEnumeration", _illegalCtor("SVGAnimatedEnumeration"));
    _define("SVGAnimatedInteger", _illegalCtor("SVGAnimatedInteger"));
    _define("SVGAnimatedLength", _illegalCtor("SVGAnimatedLength"));
    _define("SVGAnimatedLengthList", _illegalCtor("SVGAnimatedLengthList"));
    _define("SVGAnimatedNumber", _illegalCtor("SVGAnimatedNumber"));
    _define("SVGAnimatedNumberList", _illegalCtor("SVGAnimatedNumberList"));
    _define("SVGAnimatedPreserveAspectRatio", _illegalCtor("SVGAnimatedPreserveAspectRatio"));
    _define("SVGAnimatedRect", _illegalCtor("SVGAnimatedRect"));
    _define("SVGAnimatedString", _illegalCtor("SVGAnimatedString"));
    _define("SVGAnimatedTransformList", _illegalCtor("SVGAnimatedTransformList"));
    _define("SVGAnimationElement", _illegalCtor("SVGAnimationElement"));
    _define("SVGCircleElement", _illegalCtor("SVGCircleElement"));
    _define("SVGClipPathElement", _illegalCtor("SVGClipPathElement"));
    _define("SVGComponentTransferFunctionElement", _illegalCtor("SVGComponentTransferFunctionElement"));
    _define("SVGDefsElement", _illegalCtor("SVGDefsElement"));
    _define("SVGDescElement", _illegalCtor("SVGDescElement"));
    _define("SVGEllipseElement", _illegalCtor("SVGEllipseElement"));
    _define("SVGFEBlendElement", _illegalCtor("SVGFEBlendElement"));
    _define("SVGFEColorMatrixElement", _illegalCtor("SVGFEColorMatrixElement"));
    _define("SVGFEComponentTransferElement", _illegalCtor("SVGFEComponentTransferElement"));
    _define("SVGFECompositeElement", _illegalCtor("SVGFECompositeElement"));
    _define("SVGFEConvolveMatrixElement", _illegalCtor("SVGFEConvolveMatrixElement"));
    _define("SVGFEDiffuseLightingElement", _illegalCtor("SVGFEDiffuseLightingElement"));
    _define("SVGFEDisplacementMapElement", _illegalCtor("SVGFEDisplacementMapElement"));
    _define("SVGFEDistantLightElement", _illegalCtor("SVGFEDistantLightElement"));
    _define("SVGFEDropShadowElement", _illegalCtor("SVGFEDropShadowElement"));
    _define("SVGFEFloodElement", _illegalCtor("SVGFEFloodElement"));
    _define("SVGFEFuncAElement", _illegalCtor("SVGFEFuncAElement"));
    _define("SVGFEFuncBElement", _illegalCtor("SVGFEFuncBElement"));
    _define("SVGFEFuncGElement", _illegalCtor("SVGFEFuncGElement"));
    _define("SVGFEFuncRElement", _illegalCtor("SVGFEFuncRElement"));
    _define("SVGFEGaussianBlurElement", _illegalCtor("SVGFEGaussianBlurElement"));
    _define("SVGFEImageElement", _illegalCtor("SVGFEImageElement"));
    _define("SVGFEMergeElement", _illegalCtor("SVGFEMergeElement"));
    _define("SVGFEMergeNodeElement", _illegalCtor("SVGFEMergeNodeElement"));
    _define("SVGFEMorphologyElement", _illegalCtor("SVGFEMorphologyElement"));
    _define("SVGFEOffsetElement", _illegalCtor("SVGFEOffsetElement"));
    _define("SVGFEPointLightElement", _illegalCtor("SVGFEPointLightElement"));
    _define("SVGFESpecularLightingElement", _illegalCtor("SVGFESpecularLightingElement"));
    _define("SVGFESpotLightElement", _illegalCtor("SVGFESpotLightElement"));
    _define("SVGFETileElement", _illegalCtor("SVGFETileElement"));
    _define("SVGFETurbulenceElement", _illegalCtor("SVGFETurbulenceElement"));
    _define("SVGFilterElement", _illegalCtor("SVGFilterElement"));
    _define("SVGForeignObjectElement", _illegalCtor("SVGForeignObjectElement"));
    _define("SVGGElement", _illegalCtor("SVGGElement"));
    _define("SVGGeometryElement", _illegalCtor("SVGGeometryElement"));
    _define("SVGGradientElement", _illegalCtor("SVGGradientElement"));
    _define("SVGGraphicsElement", _illegalCtor("SVGGraphicsElement"));
    _define("SVGImageElement", _illegalCtor("SVGImageElement"));
    _define("SVGLength", _illegalCtor("SVGLength"));
    _define("SVGLengthList", _illegalCtor("SVGLengthList"));
    _define("SVGLineElement", _illegalCtor("SVGLineElement"));
    _define("SVGLinearGradientElement", _illegalCtor("SVGLinearGradientElement"));
    _define("SVGMPathElement", _illegalCtor("SVGMPathElement"));
    _define("SVGMarkerElement", _illegalCtor("SVGMarkerElement"));
    _define("SVGMaskElement", _illegalCtor("SVGMaskElement"));
    _define("SVGMatrix", _illegalCtor("SVGMatrix"));
    _define("SVGMetadataElement", _illegalCtor("SVGMetadataElement"));
    _define("SVGNumber", _illegalCtor("SVGNumber"));
    _define("SVGNumberList", _illegalCtor("SVGNumberList"));
    _define("SVGPathElement", _illegalCtor("SVGPathElement"));
    _define("SVGPatternElement", _illegalCtor("SVGPatternElement"));
    _define("SVGPoint", _illegalCtor("SVGPoint"));
    _define("SVGPointList", _illegalCtor("SVGPointList"));
    _define("SVGPolygonElement", _illegalCtor("SVGPolygonElement"));
    _define("SVGPolylineElement", _illegalCtor("SVGPolylineElement"));
    _define("SVGPreserveAspectRatio", _illegalCtor("SVGPreserveAspectRatio"));
    _define("SVGRadialGradientElement", _illegalCtor("SVGRadialGradientElement"));
    _define("SVGRect", _illegalCtor("SVGRect"));
    _define("SVGRectElement", _illegalCtor("SVGRectElement"));
    _define("SVGSVGElement", _illegalCtor("SVGSVGElement"));
    _define("SVGScriptElement", _illegalCtor("SVGScriptElement"));
    _define("SVGSetElement", _illegalCtor("SVGSetElement"));
    _define("SVGStopElement", _illegalCtor("SVGStopElement"));
    _define("SVGStringList", _illegalCtor("SVGStringList"));
    _define("SVGStyleElement", _illegalCtor("SVGStyleElement"));
    _define("SVGSwitchElement", _illegalCtor("SVGSwitchElement"));
    _define("SVGSymbolElement", _illegalCtor("SVGSymbolElement"));
    _define("SVGTSpanElement", _illegalCtor("SVGTSpanElement"));
    _define("SVGTextContentElement", _illegalCtor("SVGTextContentElement"));
    _define("SVGTextElement", _illegalCtor("SVGTextElement"));
    _define("SVGTextPathElement", _illegalCtor("SVGTextPathElement"));
    _define("SVGTextPositioningElement", _illegalCtor("SVGTextPositioningElement"));
    _define("SVGTitleElement", _illegalCtor("SVGTitleElement"));
    _define("SVGTransform", _illegalCtor("SVGTransform"));
    _define("SVGTransformList", _illegalCtor("SVGTransformList"));
    _define("SVGUnitTypes", _illegalCtor("SVGUnitTypes"));
    _define("SVGUseElement", _illegalCtor("SVGUseElement"));
    _define("SVGViewElement", _illegalCtor("SVGViewElement"));
    _define("Sanitizer", _illegalCtor("Sanitizer"));
    _define("Scheduler", _illegalCtor("Scheduler"));
    _define("Scheduling", _illegalCtor("Scheduling"));
    _define("ScriptProcessorNode", _illegalCtor("ScriptProcessorNode"));
    _define("ScrollTimeline", _illegalCtor("ScrollTimeline"));
    _define("ShadowRoot", _illegalCtor("ShadowRoot"));
    _define("SharedStorage", _illegalCtor("SharedStorage"));
    _define("SharedStorageAppendMethod", _illegalCtor("SharedStorageAppendMethod"));
    _define("SharedStorageClearMethod", _illegalCtor("SharedStorageClearMethod"));
    _define("SharedStorageDeleteMethod", _illegalCtor("SharedStorageDeleteMethod"));
    _define("SharedStorageModifierMethod", _illegalCtor("SharedStorageModifierMethod"));
    _define("SharedStorageSetMethod", _illegalCtor("SharedStorageSetMethod"));
    _define("SharedStorageWorklet", _illegalCtor("SharedStorageWorklet"));
    _define("SnapEvent", _illegalCtor("SnapEvent"));
    _define("SourceBuffer", _illegalCtor("SourceBuffer"));
    _define("SourceBufferList", _illegalCtor("SourceBufferList"));
    _define("SpeechGrammar", _illegalCtor("SpeechGrammar"));
    _define("SpeechGrammarList", _illegalCtor("SpeechGrammarList"));
    _define("SpeechRecognition", _illegalCtor("SpeechRecognition"));
    _define("SpeechRecognitionErrorEvent", _illegalCtor("SpeechRecognitionErrorEvent"));
    _define("SpeechRecognitionEvent", _illegalCtor("SpeechRecognitionEvent"));
    _define("SpeechSynthesisErrorEvent", _illegalCtor("SpeechSynthesisErrorEvent"));
    _define("SpeechSynthesisEvent", _illegalCtor("SpeechSynthesisEvent"));
    _define("SpeechSynthesisUtterance", _illegalCtor("SpeechSynthesisUtterance"));
    _define("SpeechSynthesisVoice", _illegalCtor("SpeechSynthesisVoice"));
    _define("StereoPannerNode", _illegalCtor("StereoPannerNode"));
    _define("Storage", _illegalCtor("Storage"));
    _define("StylePropertyMap", _illegalCtor("StylePropertyMap"));
    _define("StylePropertyMapReadOnly", _illegalCtor("StylePropertyMapReadOnly"));
    _define("StyleSheet", _illegalCtor("StyleSheet"));
    _define("StyleSheetList", _illegalCtor("StyleSheetList"));
    _define("SubmitEvent", _illegalCtor("SubmitEvent"));
    _define("Subscriber", _illegalCtor("Subscriber"));
    _define("SuppressedError", _illegalCtor("SuppressedError"));
    _define("SyncManager", _illegalCtor("SyncManager"));
    _define("TaskAttributionTiming", _illegalCtor("TaskAttributionTiming"));
    _define("TaskController", _illegalCtor("TaskController"));
    _define("TaskPriorityChangeEvent", _illegalCtor("TaskPriorityChangeEvent"));
    _define("TaskSignal", _illegalCtor("TaskSignal"));
    _define("TextEvent", _illegalCtor("TextEvent"));
    _define("TextFormat", _illegalCtor("TextFormat"));
    _define("TextFormatUpdateEvent", _illegalCtor("TextFormatUpdateEvent"));
    _define("TextMetrics", _illegalCtor("TextMetrics"));
    _define("TextTrack", _illegalCtor("TextTrack"));
    _define("TextTrackCue", _illegalCtor("TextTrackCue"));
    _define("TextTrackCueList", _illegalCtor("TextTrackCueList"));
    _define("TextTrackList", _illegalCtor("TextTrackList"));
    _define("TextUpdateEvent", _illegalCtor("TextUpdateEvent"));
    _define("TimeRanges", _illegalCtor("TimeRanges"));
    _define("TimelineTrigger", _illegalCtor("TimelineTrigger"));
    _define("TimelineTriggerRange", _illegalCtor("TimelineTriggerRange"));
    _define("TimelineTriggerRangeList", _illegalCtor("TimelineTriggerRangeList"));
    _define("ToggleEvent", _illegalCtor("ToggleEvent"));
    _define("TrackEvent", _illegalCtor("TrackEvent"));
    _define("TransformStreamDefaultController", _illegalCtor("TransformStreamDefaultController"));
    _define("TreeWalker", _illegalCtor("TreeWalker"));
    _define("TrustedTypePolicy", _illegalCtor("TrustedTypePolicy"));
    _define("TrustedTypePolicyFactory", _illegalCtor("TrustedTypePolicyFactory"));
    _define("URLPattern", _illegalCtor("URLPattern"));
    _define("UserActivation", _illegalCtor("UserActivation"));
    _define("VTTCue", _illegalCtor("VTTCue"));
    _define("ValidityState", _illegalCtor("ValidityState"));
    _define("VideoColorSpace", _illegalCtor("VideoColorSpace"));
    _define("VideoFrame", _illegalCtor("VideoFrame"));
    _define("VideoPlaybackQuality", _illegalCtor("VideoPlaybackQuality"));
    _define("ViewTimeline", _illegalCtor("ViewTimeline"));
    _define("ViewTransitionTypeSet", _illegalCtor("ViewTransitionTypeSet"));
    _define("Viewport", _illegalCtor("Viewport"));
    _define("VirtualKeyboardGeometryChangeEvent", _illegalCtor("VirtualKeyboardGeometryChangeEvent"));
    _define("VisibilityStateEntry", _illegalCtor("VisibilityStateEntry"));
    _define("WaveShaperNode", _illegalCtor("WaveShaperNode"));
    _define("WebGLActiveInfo", _illegalCtor("WebGLActiveInfo"));
    _define("WebGLBuffer", _illegalCtor("WebGLBuffer"));
    _define("WebGLContextEvent", _illegalCtor("WebGLContextEvent"));
    _define("WebGLFramebuffer", _illegalCtor("WebGLFramebuffer"));
    _define("WebGLObject", _illegalCtor("WebGLObject"));
    _define("WebGLProgram", _illegalCtor("WebGLProgram"));
    _define("WebGLQuery", _illegalCtor("WebGLQuery"));
    _define("WebGLRenderbuffer", _illegalCtor("WebGLRenderbuffer"));
    _define("WebGLSampler", _illegalCtor("WebGLSampler"));
    _define("WebGLShader", _illegalCtor("WebGLShader"));
    _define("WebGLShaderPrecisionFormat", _illegalCtor("WebGLShaderPrecisionFormat"));
    _define("WebGLSync", _illegalCtor("WebGLSync"));
    _define("WebGLTexture", _illegalCtor("WebGLTexture"));
    _define("WebGLTransformFeedback", _illegalCtor("WebGLTransformFeedback"));
    _define("WebGLUniformLocation", _illegalCtor("WebGLUniformLocation"));
    _define("WebGLVertexArrayObject", _illegalCtor("WebGLVertexArrayObject"));
    _define("WebKitMutationObserver", _illegalCtor("WebKitMutationObserver"));
    _define("WebSocketError", _illegalCtor("WebSocketError"));
    _define("WebSocketStream", _illegalCtor("WebSocketStream"));
    _define("Window", _illegalCtor("Window"));
    _define("WindowControlsOverlayGeometryChangeEvent", _illegalCtor("WindowControlsOverlayGeometryChangeEvent"));
    _define("XMLDocument", _illegalCtor("XMLDocument"));
    _define("XMLHttpRequestEventTarget", _illegalCtor("XMLHttpRequestEventTarget"));
    _define("XMLHttpRequestUpload", _illegalCtor("XMLHttpRequestUpload"));
    _define("XPathEvaluator", _illegalCtor("XPathEvaluator"));
    _define("XPathExpression", _illegalCtor("XPathExpression"));
    _define("XPathResult", _illegalCtor("XPathResult"));

    // ---- Event handlers (120) ----
    // Event handlers — Chrome 147 exposes ~120 on* accessors on Window.
    // All return null, no observable behaviour.
    for (const _h of [
        "onabort", "onafterprint", "onanimationcancel", "onanimationend",
        "onanimationiteration", "onanimationstart", "onappinstalled", "onauxclick",
        "onbeforeinput", "onbeforeinstallprompt", "onbeforematch", "onbeforeprint",
        "onbeforetoggle", "onbeforeunload", "onbeforexrselect", "onblur",
        "oncancel", "oncanplay", "oncanplaythrough", "onchange",
        "onclick", "onclose", "oncommand", "oncontentvisibilityautostatechange",
        "oncontextlost", "oncontextmenu", "oncontextrestored", "oncuechange",
        "ondblclick", "ondrag", "ondragend", "ondragenter",
        "ondragleave", "ondragover", "ondragstart", "ondrop",
        "ondurationchange", "onemptied", "onended", "onfocus",
        "onformdata", "ongamepadconnected", "ongamepaddisconnected", "ongotpointercapture",
        "onhashchange", "oninput", "oninvalid", "onkeydown",
        "onkeypress", "onkeyup", "onlanguagechange", "onload",
        "onloadeddata", "onloadedmetadata", "onloadstart", "onlostpointercapture",
        "onmessage", "onmessageerror", "onmousedown", "onmouseenter",
        "onmouseleave", "onmousemove", "onmouseout", "onmouseover",
        "onmouseup", "onmousewheel", "onoffline", "ononline",
        "onpagehide", "onpagereveal", "onpageshow", "onpageswap",
        "onpause", "onplay", "onplaying", "onpointercancel",
        "onpointerdown", "onpointerenter", "onpointerleave", "onpointermove",
        "onpointerout", "onpointerover", "onpointerup", "onpopstate",
        "onprogress", "onratechange", "onrejectionhandled", "onreset",
        "onresize", "onscroll", "onscrollend", "onscrollsnapchange",
        "onscrollsnapchanging", "onsearch", "onsecuritypolicyviolation", "onseeked",
        "onseeking", "onselect", "onselectionchange", "onselectstart",
        "onslotchange", "onstalled", "onstorage", "onsubmit",
        "onsuspend", "ontimeupdate", "ontoggle", "ontransitioncancel",
        "ontransitionend", "ontransitionrun", "ontransitionstart", "onunhandledrejection",
        "onunload", "onvolumechange", "onwaiting", "onwebkitanimationend",
        "onwebkitanimationiteration", "onwebkitanimationstart", "onwebkittransitionend", "onwheel",
    ]) {
        if (!(_h in globalThis)) {
            let _v = null;
            Object.defineProperty(globalThis, _h, {
                get: () => _v,
                set: function(v) { _v = (typeof v === 'function' || v === null) ? v : null; },
                configurable: true, enumerable: true,
            });
        }
    }

    // ---- Other window props (43) ----
    // Misc Window data properties + bars + webkit aliases.
    const _otherProps = {
        "blur": function() {},
        "captureEvents": function() {},
        "clientInformation": globalThis.navigator,
        "closed": false,
        "crashReport": {},
        "credentialless": false,
        "event": undefined,
        "external": { AddSearchProvider: function() {}, IsSearchProviderInstalled: function() { return 0; } },
        "fence": null,
        "find": function() {},
        "focus": function() {},
        "frameElement": null,
        "launchQueue": { setConsumer: function() {} },
        "length": 0,
        "locationbar": { visible: true },
        "menubar": { visible: true },
        "moveBy": function() {},
        "moveTo": function() {},
        "name": "",
        "navigation": {},
        "offscreenBuffering": false,
        "originAgentCluster": false,
        "personalbar": { visible: true },
        "releaseEvents": function() {},
        "resizeBy": function() {},
        "resizeTo": function() {},
        "scrollbars": { visible: true },
        "status": "",
        "statusbar": { visible: true },
        "styleMedia": { type: 'screen', matchMedium: function(m) { return globalThis.matchMedia ? globalThis.matchMedia(m).matches : false; } },
        "toolbar": { visible: true },
        "viewport": {},
        "webkitCancelAnimationFrame": globalThis.cancelAnimationFrame,
        "webkitMediaStream": _illegalCtor('MediaStream'),
        "webkitRequestAnimationFrame": globalThis.requestAnimationFrame,
        "webkitRequestFileSystem": function webkitRequestFileSystem(){ return undefined; },
        "webkitResolveLocalFileSystemURL": function webkitResolveLocalFileSystemURL(){ return undefined; },
        "webkitSpeechGrammar": _illegalCtor('SpeechGrammar'),
        "webkitSpeechGrammarList": _illegalCtor('SpeechGrammarList'),
        "webkitSpeechRecognition": _illegalCtor('SpeechRecognition'),
        "webkitSpeechRecognitionError": _illegalCtor('SpeechRecognitionError'),
        "webkitSpeechRecognitionEvent": _illegalCtor('SpeechRecognitionEvent'),
        "webkitURL": globalThis.URL,
    };
    for (const _k of Object.keys(_otherProps)) {
        if (!(_k in globalThis)) {
            try {
                Object.defineProperty(globalThis, _k, {
                    value: _otherProps[_k], configurable: true,
                    writable: true, enumerable: true,
                });
            } catch (_e) {}
        }
    }

})(globalThis);
