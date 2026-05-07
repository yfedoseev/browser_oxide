/**
 * Interface bootstrap — defines standard Web IDL classes.
 * Runs FIRST to ensure these globals are available to all other scripts.
 */
((globalThis) => {
    function _define(name, cls) {
        if (globalThis[name]) {
            return;
        }
        if (cls && cls.prototype) {
            Object.defineProperty(cls.prototype, Symbol.toStringTag, {
                value: name, configurable: true
            });
        }
        if (typeof _maskFunction === 'function' && typeof cls === 'function') {
            _maskFunction(cls, name);
        }
        Object.defineProperty(globalThis, name, {
            value: cls, configurable: true, writable: true, enumerable: false
        });
    }

    _define("Navigator", class Navigator {});
    _define("Location", class Location {});
    _define("History", class History {});
    _define("Screen", class Screen {});

    // Canonical EventTarget — base for ~70% of the Web API surface.
    // Must be defined ONCE and shared across all bootstraps to ensure
    // `x instanceof EventTarget` holds for all inherited stubs.
    const _EventTarget = class EventTarget {
        constructor() {}
    };
    _define("EventTarget", _EventTarget);

    // Canonical Event — base for all event types.
    const _Event = class Event {
        constructor(type, init = {}) {
            this.type = String(type);
            this.bubbles = !!init.bubbles;
            this.cancelable = !!init.cancelable;
            this.composed = !!init.composed;
            this.defaultPrevented = false;
            this.eventPhase = 0; // NONE
            this.target = null;
            this.currentTarget = null;
            this.timeStamp = Date.now();
            this.isTrusted = false;
            this._stopped = false;
            this._stoppedImmediate = false;
        }
        stopPropagation() { this._stopped = true; }
        stopImmediatePropagation() { this._stopped = true; this._stoppedImmediate = true; }
        preventDefault() { if (this.cancelable) this.defaultPrevented = true; }
    };
    _define("Event", _Event);

    _define("MessageEvent", class MessageEvent extends _Event {});
    _define("CustomEvent", class CustomEvent extends _Event {
        constructor(type, init = {}) {
            super(type, init);
            this.detail = init.detail || null;
        }
    });

    _define("Performance", class Performance extends _EventTarget {});
    _define("PluginArray", class PluginArray {});
    _define("MimeTypeArray", class MimeTypeArray {});
    _define("Plugin", class Plugin {});
    _define("MimeType", class MimeType {});
    _define("NetworkInformation", class NetworkInformation extends _EventTarget {});
    _define("MediaDevices", class MediaDevices extends _EventTarget {});
    _define("StorageManager", class StorageManager extends _EventTarget {});
    _define("Bluetooth", class Bluetooth extends _EventTarget {});
    _define("Permissions", class Permissions {});
    _define("ScreenOrientation", class ScreenOrientation extends _EventTarget {});

    // Chrome 147 constructor surface — anti-bot enumeration probes
    // (CreepJS features, fp-collect navigatorPrototype walk, BotD
    // distinctive_props) hash the names AND existence of the full
    // constructor list. Each missing entry is a "this UA claims Chrome
    // 147 but doesn't ship X" tell. Stubs with Illegal-constructor
    // semantics match real Chrome behaviour for most of these.
    function _stub(name, base = Object) {
        // Many Chrome classes are stubs that throw "Illegal constructor"
        // when called via `new`, but MUST have the correct prototype chain.
        const C = class extends base {
            constructor() {
                super();
                throw new TypeError("Failed to construct '" + name + "': Illegal constructor");
            }
        };
        // Ensure C.name is exactly correct (minifiers might mangle otherwise)
        Object.defineProperty(C, "name", { value: name, configurable: true });
        return C;
    }

    // CSS-related
    _define("CSSStyleSheet", _stub("CSSStyleSheet"));
    _define("CSSRule", _stub("CSSRule"));
    _define("CSSStyleRule", _stub("CSSStyleRule"));
    _define("Highlight", class Highlight { constructor(...ranges) { this._ranges = ranges; this.priority = 0; } });
    _define("HighlightRegistry", _stub("HighlightRegistry"));
    _define("CSSPseudoElement", _stub("CSSPseudoElement"));

    // DOM ranges and other interfaces
    _define("StaticRange", class StaticRange { constructor(init) { Object.assign(this, init || {}); } });
    _define("XMLSerializer", class XMLSerializer { serializeToString(_node) { return ""; } });
    _define("XSLTProcessor", class XSLTProcessor { importStylesheet() {} transformToFragment() { return null; } transformToDocument() { return null; } reset() {} setParameter() {} getParameter() {} removeParameter() {} clearParameters() {} });

    // Newer Chrome API constructors
    _define("EditContext", class EditContext { constructor(init) { Object.assign(this, init || {}); } });
    _define("CookieStore", _stub("CookieStore"));
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
    _define("LaunchQueue", _stub("LaunchQueue"));

    // File system access
    _define("FileSystemHandle", _stub("FileSystemHandle"));
    _define("FileSystemFileHandle", _stub("FileSystemFileHandle"));
    _define("FileSystemDirectoryHandle", _stub("FileSystemDirectoryHandle"));
    _define("FileSystemWritableFileStream", _stub("FileSystemWritableFileStream"));

    // Push / background fetch
    _define("PushManager", _stub("PushManager"));
    _define("PushSubscription", _stub("PushSubscription"));
    _define("BackgroundFetchManager", _stub("BackgroundFetchManager"));

    // Payments / presentation
    _define("PaymentRequest", class PaymentRequest { constructor(_methods, _details) {} });
    _define("PresentationConnection", _stub("PresentationConnection"));
    _define("Presentation", _stub("Presentation"));

    // Sensors (DeviceMotion / DeviceOrientation API surface)
    const _sensor = (n) => {
        const C = class {
            constructor(_opts) { throw new TypeError("Failed to construct '" + n + "': permission denied"); }
        };
        Object.defineProperty(C, "name", { value: n, configurable: true });
        return C;
    };
    _define("Sensor", _stub("Sensor"));
    _define("Accelerometer", _sensor("Accelerometer"));
    _define("LinearAccelerationSensor", _sensor("LinearAccelerationSensor"));
    _define("GravitySensor", _sensor("GravitySensor"));
    _define("Gyroscope", _sensor("Gyroscope"));
    _define("Magnetometer", _sensor("Magnetometer"));
    _define("OrientationSensor", _stub("OrientationSensor"));
    _define("AbsoluteOrientationSensor", _sensor("AbsoluteOrientationSensor"));
    _define("RelativeOrientationSensor", _sensor("RelativeOrientationSensor"));

    // Battery / Geolocation / WebXR
    _define("BatteryManager", _stub("BatteryManager"));
    _define("Geolocation", _stub("Geolocation"));
    _define("XRSystem", _stub("XRSystem"));
    _define("XRSession", _stub("XRSession"));

    // Streams (newer)
    if (typeof globalThis.TextDecoderStream === "undefined") {
        _define("TextDecoderStream", class TextDecoderStream {});
    }
    if (typeof globalThis.TextEncoderStream === "undefined") {
        _define("TextEncoderStream", class TextEncoderStream {});
    }

    // Privacy Sandbox / FedCM-adjacent (shape-only — present in Chrome 147 even
    // though Topics/Protected Audience were retired in 2026).
    _define("CredentialsContainer", _stub("CredentialsContainer"));
    _define("Credential", _stub("Credential"));
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
    _define("AbstractRange", _stub("AbstractRange"));
    _define("AnalyserNode", _stub("AnalyserNode"));
    _define("Animation", _stub("Animation"));
    _define("AnimationEffect", _stub("AnimationEffect"));
    _define("AnimationPlaybackEvent", _stub("AnimationPlaybackEvent", _Event));
    _define("AnimationTimeline", _stub("AnimationTimeline"));
    _define("AnimationTrigger", _stub("AnimationTrigger"));
    _define("AsyncDisposableStack", _stub("AsyncDisposableStack"));
    _define("Attr", _stub("Attr"));
    _define("Audio", _stub("Audio"));
    _define("AudioBuffer", _stub("AudioBuffer"));
    _define("AudioBufferSourceNode", _stub("AudioBufferSourceNode"));
    _define("AudioData", _stub("AudioData"));
    _define("AudioDestinationNode", _stub("AudioDestinationNode"));
    _define("AudioListener", _stub("AudioListener"));
    _define("AudioNode", _stub("AudioNode", _EventTarget));
    _define("AudioParam", _stub("AudioParam"));
    _define("AudioParamMap", _stub("AudioParamMap"));
    _define("AudioProcessingEvent", _stub("AudioProcessingEvent"));
    _define("AudioScheduledSourceNode", _stub("AudioScheduledSourceNode"));
    _define("AudioSinkInfo", _stub("AudioSinkInfo"));
    _define("AudioWorkletNode", _stub("AudioWorkletNode"));
    _define("BackgroundFetchRecord", _stub("BackgroundFetchRecord"));
    _define("BackgroundFetchRegistration", _stub("BackgroundFetchRegistration"));
    _define("BarProp", _stub("BarProp"));
    _define("BaseAudioContext", _stub("BaseAudioContext"));
    _define("BeforeInstallPromptEvent", _stub("BeforeInstallPromptEvent"));
    _define("BiquadFilterNode", _stub("BiquadFilterNode"));
    _define("BlobEvent", _stub("BlobEvent", _Event));
    _define("BluetoothUUID", _stub("BluetoothUUID"));
    _define("BrowserCaptureMediaStreamTrack", _stub("BrowserCaptureMediaStreamTrack"));
    _define("ByteLengthQueuingStrategy", _stub("ByteLengthQueuingStrategy"));
    _define("CDATASection", _stub("CDATASection"));
    _define("CSPViolationReportBody", _stub("CSPViolationReportBody"));
    _define("CSSAnimation", _stub("CSSAnimation"));
    _define("CSSConditionRule", _stub("CSSConditionRule"));
    _define("CSSContainerRule", _stub("CSSContainerRule"));
    _define("CSSCounterStyleRule", _stub("CSSCounterStyleRule"));
    _define("CSSFontFaceRule", _stub("CSSFontFaceRule"));
    _define("CSSFontFeatureValuesRule", _stub("CSSFontFeatureValuesRule"));
    _define("CSSFontPaletteValuesRule", _stub("CSSFontPaletteValuesRule"));
    _define("CSSFunctionDeclarations", _stub("CSSFunctionDeclarations"));
    _define("CSSFunctionDescriptors", _stub("CSSFunctionDescriptors"));
    _define("CSSFunctionRule", _stub("CSSFunctionRule"));
    _define("CSSGroupingRule", _stub("CSSGroupingRule"));
    _define("CSSImageValue", _stub("CSSImageValue"));
    _define("CSSImportRule", _stub("CSSImportRule"));
    _define("CSSKeyframeRule", _stub("CSSKeyframeRule"));
    _define("CSSKeyframesRule", _stub("CSSKeyframesRule"));
    _define("CSSKeywordValue", _stub("CSSKeywordValue"));
    _define("CSSLayerBlockRule", _stub("CSSLayerBlockRule"));
    _define("CSSLayerStatementRule", _stub("CSSLayerStatementRule"));
    _define("CSSMarginRule", _stub("CSSMarginRule"));
    _define("CSSMathClamp", _stub("CSSMathClamp"));
    _define("CSSMathInvert", _stub("CSSMathInvert"));
    _define("CSSMathMax", _stub("CSSMathMax"));
    _define("CSSMathMin", _stub("CSSMathMin"));
    _define("CSSMathNegate", _stub("CSSMathNegate"));
    _define("CSSMathProduct", _stub("CSSMathProduct"));
    _define("CSSMathSum", _stub("CSSMathSum"));
    _define("CSSMathValue", _stub("CSSMathValue"));
    _define("CSSMatrixComponent", _stub("CSSMatrixComponent"));
    _define("CSSMediaRule", _stub("CSSMediaRule"));
    _define("CSSNamespaceRule", _stub("CSSNamespaceRule"));
    _define("CSSNestedDeclarations", _stub("CSSNestedDeclarations"));
    _define("CSSNumericArray", _stub("CSSNumericArray"));
    _define("CSSNumericValue", _stub("CSSNumericValue"));
    _define("CSSPageRule", _stub("CSSPageRule"));
    _define("CSSPerspective", _stub("CSSPerspective"));
    _define("CSSPositionTryDescriptors", _stub("CSSPositionTryDescriptors"));
    _define("CSSPositionTryRule", _stub("CSSPositionTryRule"));
    _define("CSSPositionValue", _stub("CSSPositionValue"));
    _define("CSSPropertyRule", _stub("CSSPropertyRule"));
    _define("CSSRotate", _stub("CSSRotate"));
    _define("CSSRuleList", _stub("CSSRuleList"));
    _define("CSSScale", _stub("CSSScale"));
    _define("CSSScopeRule", _stub("CSSScopeRule"));
    _define("CSSSkew", _stub("CSSSkew"));
    _define("CSSSkewX", _stub("CSSSkewX"));
    _define("CSSSkewY", _stub("CSSSkewY"));
    _define("CSSStartingStyleRule", _stub("CSSStartingStyleRule"));
    _define("CSSStyleDeclaration", _stub("CSSStyleDeclaration"));
    _define("CSSStyleValue", _stub("CSSStyleValue"));
    _define("CSSSupportsRule", _stub("CSSSupportsRule"));
    _define("CSSTransformComponent", _stub("CSSTransformComponent"));
    _define("CSSTransformValue", _stub("CSSTransformValue"));
    _define("CSSTransition", _stub("CSSTransition"));
    _define("CSSTranslate", _stub("CSSTranslate"));
    _define("CSSUnitValue", _stub("CSSUnitValue"));
    _define("CSSUnparsedValue", _stub("CSSUnparsedValue"));
    _define("CSSVariableReferenceValue", _stub("CSSVariableReferenceValue"));
    _define("CSSViewTransitionRule", _stub("CSSViewTransitionRule"));
    _define("CanvasCaptureMediaStreamTrack", _stub("CanvasCaptureMediaStreamTrack"));
    _define("CanvasGradient", _stub("CanvasGradient"));
    _define("CanvasPattern", _stub("CanvasPattern"));
    _define("CaretPosition", _stub("CaretPosition"));
    _define("ChannelMergerNode", _stub("ChannelMergerNode"));
    _define("ChannelSplitterNode", _stub("ChannelSplitterNode"));
    _define("ChapterInformation", _stub("ChapterInformation"));
    _define("CharacterBoundsUpdateEvent", _stub("CharacterBoundsUpdateEvent"));
    _define("CharacterData", _stub("CharacterData"));
    _define("CloseWatcher", _stub("CloseWatcher"));
    _define("CommandEvent", _stub("CommandEvent"));
    _define("CompositionEvent", _stub("CompositionEvent"));
    _define("ConstantSourceNode", _stub("ConstantSourceNode"));
    _define("ContentVisibilityAutoStateChangeEvent", _stub("ContentVisibilityAutoStateChangeEvent"));
    _define("ConvolverNode", _stub("ConvolverNode"));
    _define("CountQueuingStrategy", _stub("CountQueuingStrategy"));
    _define("CrashReportContext", _stub("CrashReportContext"));
    _define("CropTarget", _stub("CropTarget"));
    _define("CustomStateSet", _stub("CustomStateSet"));
    _define("DOMError", _stub("DOMError"));
    _define("DOMImplementation", _stub("DOMImplementation"));
    _define("DOMQuad", _stub("DOMQuad"));
    _define("DOMRectList", _stub("DOMRectList"));
    _define("DOMStringList", _stub("DOMStringList"));
    _define("DOMStringMap", _stub("DOMStringMap"));
    _define("DataTransfer", _stub("DataTransfer"));
    _define("DataTransferItem", _stub("DataTransferItem"));
    _define("DataTransferItemList", _stub("DataTransferItemList"));
    _define("DelayNode", _stub("DelayNode"));
    _define("DelegatedInkTrailPresenter", _stub("DelegatedInkTrailPresenter"));
    _define("DisposableStack", _stub("DisposableStack"));
    _define("DocumentPictureInPictureEvent", _stub("DocumentPictureInPictureEvent"));
    _define("DocumentTimeline", _stub("DocumentTimeline"));
    _define("DocumentType", _stub("DocumentType"));
    _define("DynamicsCompressorNode", _stub("DynamicsCompressorNode"));
    _define("ElementInternals", _stub("ElementInternals"));
    _define("EncodedAudioChunk", _stub("EncodedAudioChunk"));
    _define("EncodedVideoChunk", _stub("EncodedVideoChunk"));
    _define("External", _stub("External"));
    _define("FeaturePolicy", _stub("FeaturePolicy"));
    _define("Fence", _stub("Fence"));
    _define("FencedFrameConfig", _stub("FencedFrameConfig"));
    _define("FileList", _stub("FileList"));
    _define("FontFaceSetLoadEvent", _stub("FontFaceSetLoadEvent"));
    _define("FormDataEvent", _stub("FormDataEvent"));
    _define("FragmentDirective", _stub("FragmentDirective"));
    _define("GainNode", _stub("GainNode"));
    _define("Gamepad", _stub("Gamepad"));
    _define("GamepadButton", _stub("GamepadButton"));
    _define("GamepadEvent", _stub("GamepadEvent"));
    _define("GamepadHapticActuator", _stub("GamepadHapticActuator"));
    _define("GeolocationCoordinates", _stub("GeolocationCoordinates"));
    _define("GeolocationPosition", _stub("GeolocationPosition"));
    _define("GeolocationPositionError", _stub("GeolocationPositionError"));
    _define("HTMLAllCollection", _stub("HTMLAllCollection"));
    _define("HTMLAreaElement", _stub("HTMLAreaElement"));
    _define("HTMLBRElement", _stub("HTMLBRElement"));
    _define("HTMLBaseElement", _stub("HTMLBaseElement"));
    _define("HTMLCollection", _stub("HTMLCollection"));
    _define("HTMLDListElement", _stub("HTMLDListElement"));
    _define("HTMLDataElement", _stub("HTMLDataElement"));
    _define("HTMLDataListElement", _stub("HTMLDataListElement"));
    _define("HTMLDetailsElement", _stub("HTMLDetailsElement"));
    _define("HTMLDialogElement", _stub("HTMLDialogElement"));
    _define("HTMLDirectoryElement", _stub("HTMLDirectoryElement"));
    _define("HTMLEmbedElement", _stub("HTMLEmbedElement"));
    _define("HTMLFencedFrameElement", _stub("HTMLFencedFrameElement"));
    _define("HTMLFieldSetElement", _stub("HTMLFieldSetElement"));
    _define("HTMLFontElement", _stub("HTMLFontElement"));
    _define("HTMLFormControlsCollection", _stub("HTMLFormControlsCollection"));
    _define("HTMLFrameElement", _stub("HTMLFrameElement"));
    _define("HTMLFrameSetElement", _stub("HTMLFrameSetElement"));
    _define("HTMLGeolocationElement", _stub("HTMLGeolocationElement"));
    _define("HTMLHRElement", _stub("HTMLHRElement"));
    _define("HTMLLegendElement", _stub("HTMLLegendElement"));
    _define("HTMLMapElement", _stub("HTMLMapElement"));
    _define("HTMLMarqueeElement", _stub("HTMLMarqueeElement"));
    _define("HTMLMediaElement", _stub("HTMLMediaElement"));
    _define("HTMLMenuElement", _stub("HTMLMenuElement"));
    _define("HTMLMeterElement", _stub("HTMLMeterElement"));
    _define("HTMLModElement", _stub("HTMLModElement"));
    _define("HTMLObjectElement", _stub("HTMLObjectElement"));
    _define("HTMLOptGroupElement", _stub("HTMLOptGroupElement"));
    _define("HTMLOptionsCollection", _stub("HTMLOptionsCollection"));
    _define("HTMLOutputElement", _stub("HTMLOutputElement"));
    _define("HTMLParamElement", _stub("HTMLParamElement"));
    _define("HTMLPictureElement", _stub("HTMLPictureElement"));
    _define("HTMLProgressElement", _stub("HTMLProgressElement"));
    _define("HTMLSelectedContentElement", _stub("HTMLSelectedContentElement"));
    _define("HTMLSlotElement", _stub("HTMLSlotElement"));
    _define("HTMLSourceElement", _stub("HTMLSourceElement"));
    _define("HTMLTableCaptionElement", _stub("HTMLTableCaptionElement"));
    _define("HTMLTableColElement", _stub("HTMLTableColElement"));
    _define("HTMLTimeElement", _stub("HTMLTimeElement"));
    _define("HTMLTitleElement", _stub("HTMLTitleElement"));
    _define("HTMLTrackElement", _stub("HTMLTrackElement"));
    _define("HTMLUnknownElement", _stub("HTMLUnknownElement"));
    _define("IDBCursorWithValue", _stub("IDBCursorWithValue"));
    _define("IDBIndex", _stub("IDBIndex"));
    _define("IDBRecord", _stub("IDBRecord"));
    _define("IDBVersionChangeEvent", _stub("IDBVersionChangeEvent"));
    _define("IIRFilterNode", _stub("IIRFilterNode"));
    _define("IdleDeadline", _stub("IdleDeadline"));
    _define("ImageBitmapRenderingContext", _stub("ImageBitmapRenderingContext"));
    _define("ImageData", _stub("ImageData"));
    _define("Ink", _stub("Ink"));
    _define("InputDeviceInfo", _stub("InputDeviceInfo"));
    _define("IntegrityViolationReportBody", _stub("IntegrityViolationReportBody"));
    _define("InterestEvent", _stub("InterestEvent"));
    _define("IntersectionObserverEntry", _stub("IntersectionObserverEntry"));
    _define("KeyframeEffect", _stub("KeyframeEffect"));
    _define("LargestContentfulPaint", _stub("LargestContentfulPaint"));
    _define("LaunchParams", _stub("LaunchParams"));
    _define("LayoutShift", _stub("LayoutShift"));
    _define("LayoutShiftAttribution", _stub("LayoutShiftAttribution"));
    _define("MathMLElement", _stub("MathMLElement"));
    _define("MediaCapabilities", _stub("MediaCapabilities"));
    _define("MediaElementAudioSourceNode", _stub("MediaElementAudioSourceNode"));
    _define("MediaEncryptedEvent", _stub("MediaEncryptedEvent"));
    _define("MediaError", _stub("MediaError"));
    _define("MediaList", _stub("MediaList"));
    _define("MediaQueryListEvent", _stub("MediaQueryListEvent"));
    _define("MediaRecorder", _stub("MediaRecorder"));
    _define("MediaSourceHandle", _stub("MediaSourceHandle"));
    _define("MediaStream", _stub("MediaStream"));
    _define("MediaStreamAudioDestinationNode", _stub("MediaStreamAudioDestinationNode"));
    _define("MediaStreamAudioSourceNode", _stub("MediaStreamAudioSourceNode"));
    _define("MediaStreamEvent", _stub("MediaStreamEvent"));
    _define("MediaStreamTrack", _stub("MediaStreamTrack"));
    _define("MediaStreamTrackAudioStats", _stub("MediaStreamTrackAudioStats"));
    _define("MediaStreamTrackEvent", _stub("MediaStreamTrackEvent"));
    _define("MediaStreamTrackGenerator", _stub("MediaStreamTrackGenerator"));
    _define("MediaStreamTrackProcessor", _stub("MediaStreamTrackProcessor"));
    _define("MediaStreamTrackVideoStats", _stub("MediaStreamTrackVideoStats"));
    _define("NamedNodeMap", _stub("NamedNodeMap"));
    _define("NavigateEvent", _stub("NavigateEvent"));
    _define("Navigation", _stub("Navigation"));
    _define("NavigationActivation", _stub("NavigationActivation"));
    _define("NavigationCurrentEntryChangeEvent", _stub("NavigationCurrentEntryChangeEvent"));
    _define("NavigationDestination", _stub("NavigationDestination"));
    _define("NavigationHistoryEntry", _stub("NavigationHistoryEntry"));
    _define("NavigationPrecommitController", _stub("NavigationPrecommitController"));
    _define("NavigationTransition", _stub("NavigationTransition"));
    _define("NavigatorUAData", _stub("NavigatorUAData"));
    _define("NodeFilter", _stub("NodeFilter"));
    _define("NodeIterator", _stub("NodeIterator"));
    _define("NotRestoredReasonDetails", _stub("NotRestoredReasonDetails"));
    _define("NotRestoredReasons", _stub("NotRestoredReasons"));
    _define("Observable", _stub("Observable"));
    _define("OfflineAudioCompletionEvent", _stub("OfflineAudioCompletionEvent"));
    _define("OffscreenCanvasRenderingContext2D", _stub("OffscreenCanvasRenderingContext2D"));
    _define("Option", _stub("Option"));
    _define("Origin", _stub("Origin"));
    _define("OscillatorNode", _stub("OscillatorNode"));
    _define("OverconstrainedError", _stub("OverconstrainedError"));
    _define("PageRevealEvent", _stub("PageRevealEvent"));
    _define("PageSwapEvent", _stub("PageSwapEvent"));
    _define("PannerNode", _stub("PannerNode"));
    _define("Path2D", _stub("Path2D"));
    _define("PerformanceElementTiming", _stub("PerformanceElementTiming"));
    _define("PerformanceEventTiming", _stub("PerformanceEventTiming"));
    _define("PerformanceLongAnimationFrameTiming", _stub("PerformanceLongAnimationFrameTiming"));
    _define("PerformanceLongTaskTiming", _stub("PerformanceLongTaskTiming"));
    _define("PerformanceMark", _stub("PerformanceMark"));
    _define("PerformanceMeasure", _stub("PerformanceMeasure"));
    _define("PerformanceNavigation", _stub("PerformanceNavigation"));
    _define("PerformanceNavigationTiming", _stub("PerformanceNavigationTiming"));
    _define("PerformanceObserverEntryList", _stub("PerformanceObserverEntryList"));
    _define("PerformancePaintTiming", _stub("PerformancePaintTiming"));
    _define("PerformanceResourceTiming", _stub("PerformanceResourceTiming"));
    _define("PerformanceScriptTiming", _stub("PerformanceScriptTiming"));
    _define("PerformanceServerTiming", _stub("PerformanceServerTiming"));
    _define("PerformanceTiming", _stub("PerformanceTiming"));
    _define("PerformanceTimingConfidence", _stub("PerformanceTimingConfidence"));
    _define("PeriodicSyncManager", _stub("PeriodicSyncManager"));
    _define("PeriodicWave", _stub("PeriodicWave"));
    _define("PictureInPictureEvent", _stub("PictureInPictureEvent"));
    _define("PictureInPictureWindow", _stub("PictureInPictureWindow"));
    _define("ProcessingInstruction", _stub("ProcessingInstruction"));
    _define("Profiler", _stub("Profiler"));
    _define("PromiseRejectionEvent", _stub("PromiseRejectionEvent"));
    _define("PushSubscriptionOptions", _stub("PushSubscriptionOptions"));
    _define("QuotaExceededError", _stub("QuotaExceededError"));
    _define("RTCCertificate", _stub("RTCCertificate"));
    _define("RTCDTMFSender", _stub("RTCDTMFSender"));
    _define("RTCDTMFToneChangeEvent", _stub("RTCDTMFToneChangeEvent"));
    _define("RTCDataChannel", _stub("RTCDataChannel"));
    _define("RTCDataChannelEvent", _stub("RTCDataChannelEvent"));
    _define("RTCDtlsTransport", _stub("RTCDtlsTransport"));
    _define("RTCEncodedAudioFrame", _stub("RTCEncodedAudioFrame"));
    _define("RTCEncodedVideoFrame", _stub("RTCEncodedVideoFrame"));
    _define("RTCError", _stub("RTCError"));
    _define("RTCErrorEvent", _stub("RTCErrorEvent"));
    _define("RTCIceTransport", _stub("RTCIceTransport"));
    _define("RTCPeerConnectionIceErrorEvent", _stub("RTCPeerConnectionIceErrorEvent"));
    _define("RTCPeerConnectionIceEvent", _stub("RTCPeerConnectionIceEvent"));
    _define("RTCRtpReceiver", _stub("RTCRtpReceiver"));
    _define("RTCRtpScriptTransform", _stub("RTCRtpScriptTransform"));
    _define("RTCRtpSender", _stub("RTCRtpSender"));
    _define("RTCRtpTransceiver", _stub("RTCRtpTransceiver"));
    _define("RTCSctpTransport", _stub("RTCSctpTransport"));
    _define("RTCStatsReport", _stub("RTCStatsReport"));
    _define("RTCTrackEvent", _stub("RTCTrackEvent"));
    _define("RadioNodeList", _stub("RadioNodeList"));
    _define("ReadableByteStreamController", _stub("ReadableByteStreamController"));
    _define("ReadableStreamBYOBReader", _stub("ReadableStreamBYOBReader"));
    _define("ReadableStreamBYOBRequest", _stub("ReadableStreamBYOBRequest"));
    _define("RemotePlayback", _stub("RemotePlayback"));
    _define("ReportBody", _stub("ReportBody"));
    _define("ResizeObserverEntry", _stub("ResizeObserverEntry"));
    _define("ResizeObserverSize", _stub("ResizeObserverSize"));
    _define("RestrictionTarget", _stub("RestrictionTarget"));
    _define("SVGAElement", _stub("SVGAElement"));
    _define("SVGAngle", _stub("SVGAngle"));
    _define("SVGAnimateElement", _stub("SVGAnimateElement"));
    _define("SVGAnimateMotionElement", _stub("SVGAnimateMotionElement"));
    _define("SVGAnimateTransformElement", _stub("SVGAnimateTransformElement"));
    _define("SVGAnimatedAngle", _stub("SVGAnimatedAngle"));
    _define("SVGAnimatedBoolean", _stub("SVGAnimatedBoolean"));
    _define("SVGAnimatedEnumeration", _stub("SVGAnimatedEnumeration"));
    _define("SVGAnimatedInteger", _stub("SVGAnimatedInteger"));
    _define("SVGAnimatedLength", _stub("SVGAnimatedLength"));
    _define("SVGAnimatedLengthList", _stub("SVGAnimatedLengthList"));
    _define("SVGAnimatedNumber", _stub("SVGAnimatedNumber"));
    _define("SVGAnimatedNumberList", _stub("SVGAnimatedNumberList"));
    _define("SVGAnimatedPreserveAspectRatio", _stub("SVGAnimatedPreserveAspectRatio"));
    _define("SVGAnimatedRect", _stub("SVGAnimatedRect"));
    _define("SVGAnimatedString", _stub("SVGAnimatedString"));
    _define("SVGAnimatedTransformList", _stub("SVGAnimatedTransformList"));
    _define("SVGAnimationElement", _stub("SVGAnimationElement"));
    _define("SVGCircleElement", _stub("SVGCircleElement"));
    _define("SVGClipPathElement", _stub("SVGClipPathElement"));
    _define("SVGComponentTransferFunctionElement", _stub("SVGComponentTransferFunctionElement"));
    _define("SVGDefsElement", _stub("SVGDefsElement"));
    _define("SVGDescElement", _stub("SVGDescElement"));
    _define("SVGEllipseElement", _stub("SVGEllipseElement"));
    _define("SVGFEBlendElement", _stub("SVGFEBlendElement"));
    _define("SVGFEColorMatrixElement", _stub("SVGFEColorMatrixElement"));
    _define("SVGFEComponentTransferElement", _stub("SVGFEComponentTransferElement"));
    _define("SVGFECompositeElement", _stub("SVGFECompositeElement"));
    _define("SVGFEConvolveMatrixElement", _stub("SVGFEConvolveMatrixElement"));
    _define("SVGFEDiffuseLightingElement", _stub("SVGFEDiffuseLightingElement"));
    _define("SVGFEDisplacementMapElement", _stub("SVGFEDisplacementMapElement"));
    _define("SVGFEDistantLightElement", _stub("SVGFEDistantLightElement"));
    _define("SVGFEDropShadowElement", _stub("SVGFEDropShadowElement"));
    _define("SVGFEFloodElement", _stub("SVGFEFloodElement"));
    _define("SVGFEFuncAElement", _stub("SVGFEFuncAElement"));
    _define("SVGFEFuncBElement", _stub("SVGFEFuncBElement"));
    _define("SVGFEFuncGElement", _stub("SVGFEFuncGElement"));
    _define("SVGFEFuncRElement", _stub("SVGFEFuncRElement"));
    _define("SVGFEGaussianBlurElement", _stub("SVGFEGaussianBlurElement"));
    _define("SVGFEImageElement", _stub("SVGFEImageElement"));
    _define("SVGFEMergeElement", _stub("SVGFEMergeElement"));
    _define("SVGFEMergeNodeElement", _stub("SVGFEMergeNodeElement"));
    _define("SVGFEMorphologyElement", _stub("SVGFEMorphologyElement"));
    _define("SVGFEOffsetElement", _stub("SVGFEOffsetElement"));
    _define("SVGFEPointLightElement", _stub("SVGFEPointLightElement"));
    _define("SVGFESpecularLightingElement", _stub("SVGFESpecularLightingElement"));
    _define("SVGFESpotLightElement", _stub("SVGFESpotLightElement"));
    _define("SVGFETileElement", _stub("SVGFETileElement"));
    _define("SVGFETurbulenceElement", _stub("SVGFETurbulenceElement"));
    _define("SVGFilterElement", _stub("SVGFilterElement"));
    _define("SVGForeignObjectElement", _stub("SVGForeignObjectElement"));
    _define("SVGGElement", _stub("SVGGElement"));
    _define("SVGGeometryElement", _stub("SVGGeometryElement"));
    _define("SVGGradientElement", _stub("SVGGradientElement"));
    _define("SVGGraphicsElement", _stub("SVGGraphicsElement"));
    _define("SVGImageElement", _stub("SVGImageElement"));
    _define("SVGLength", _stub("SVGLength"));
    _define("SVGLengthList", _stub("SVGLengthList"));
    _define("SVGLineElement", _stub("SVGLineElement"));
    _define("SVGLinearGradientElement", _stub("SVGLinearGradientElement"));
    _define("SVGMPathElement", _stub("SVGMPathElement"));
    _define("SVGMarkerElement", _stub("SVGMarkerElement"));
    _define("SVGMaskElement", _stub("SVGMaskElement"));
    _define("SVGMatrix", _stub("SVGMatrix"));
    _define("SVGMetadataElement", _stub("SVGMetadataElement"));
    _define("SVGNumber", _stub("SVGNumber"));
    _define("SVGNumberList", _stub("SVGNumberList"));
    _define("SVGPathElement", _stub("SVGPathElement"));
    _define("SVGPatternElement", _stub("SVGPatternElement"));
    _define("SVGPoint", _stub("SVGPoint"));
    _define("SVGPointList", _stub("SVGPointList"));
    _define("SVGPolygonElement", _stub("SVGPolygonElement"));
    _define("SVGPolylineElement", _stub("SVGPolylineElement"));
    _define("SVGPreserveAspectRatio", _stub("SVGPreserveAspectRatio"));
    _define("SVGRadialGradientElement", _stub("SVGRadialGradientElement"));
    _define("SVGRect", _stub("SVGRect"));
    _define("SVGRectElement", _stub("SVGRectElement"));
    _define("SVGSVGElement", _stub("SVGSVGElement"));
    _define("SVGScriptElement", _stub("SVGScriptElement"));
    _define("SVGSetElement", _stub("SVGSetElement"));
    _define("SVGStopElement", _stub("SVGStopElement"));
    _define("SVGStringList", _stub("SVGStringList"));
    _define("SVGStyleElement", _stub("SVGStyleElement"));
    _define("SVGSwitchElement", _stub("SVGSwitchElement"));
    _define("SVGSymbolElement", _stub("SVGSymbolElement"));
    _define("SVGTSpanElement", _stub("SVGTSpanElement"));
    _define("SVGTextContentElement", _stub("SVGTextContentElement"));
    _define("SVGTextElement", _stub("SVGTextElement"));
    _define("SVGTextPathElement", _stub("SVGTextPathElement"));
    _define("SVGTextPositioningElement", _stub("SVGTextPositioningElement"));
    _define("SVGTitleElement", _stub("SVGTitleElement"));
    _define("SVGTransform", _stub("SVGTransform"));
    _define("SVGTransformList", _stub("SVGTransformList"));
    _define("SVGUnitTypes", _stub("SVGUnitTypes"));
    _define("SVGUseElement", _stub("SVGUseElement"));
    _define("SVGViewElement", _stub("SVGViewElement"));
    _define("Sanitizer", _stub("Sanitizer"));
    _define("Scheduler", _stub("Scheduler"));
    _define("Scheduling", _stub("Scheduling"));
    _define("ScriptProcessorNode", _stub("ScriptProcessorNode"));
    _define("ScrollTimeline", _stub("ScrollTimeline"));
    _define("ShadowRoot", _stub("ShadowRoot"));
    _define("SharedStorage", _stub("SharedStorage"));
    _define("SharedStorageAppendMethod", _stub("SharedStorageAppendMethod"));
    _define("SharedStorageClearMethod", _stub("SharedStorageClearMethod"));
    _define("SharedStorageDeleteMethod", _stub("SharedStorageDeleteMethod"));
    _define("SharedStorageModifierMethod", _stub("SharedStorageModifierMethod"));
    _define("SharedStorageSetMethod", _stub("SharedStorageSetMethod"));
    _define("SharedStorageWorklet", _stub("SharedStorageWorklet"));
    _define("SnapEvent", _stub("SnapEvent"));
    _define("SourceBuffer", _stub("SourceBuffer"));
    _define("SourceBufferList", _stub("SourceBufferList"));
    _define("SpeechGrammar", _stub("SpeechGrammar"));
    _define("SpeechGrammarList", _stub("SpeechGrammarList"));
    _define("SpeechRecognition", _stub("SpeechRecognition"));
    _define("SpeechRecognitionErrorEvent", _stub("SpeechRecognitionErrorEvent"));
    _define("SpeechRecognitionEvent", _stub("SpeechRecognitionEvent"));
    _define("SpeechSynthesisErrorEvent", _stub("SpeechSynthesisErrorEvent"));
    _define("SpeechSynthesisEvent", _stub("SpeechSynthesisEvent"));
    _define("SpeechSynthesisUtterance", _stub("SpeechSynthesisUtterance"));
    _define("SpeechSynthesisVoice", _stub("SpeechSynthesisVoice"));
    _define("StereoPannerNode", _stub("StereoPannerNode"));
    _define("Storage", _stub("Storage"));
    _define("StylePropertyMap", _stub("StylePropertyMap"));
    _define("StylePropertyMapReadOnly", _stub("StylePropertyMapReadOnly"));
    _define("StyleSheet", _stub("StyleSheet"));
    _define("StyleSheetList", _stub("StyleSheetList"));
    _define("SubmitEvent", _stub("SubmitEvent"));
    _define("Subscriber", _stub("Subscriber"));
    _define("SuppressedError", _stub("SuppressedError"));
    _define("SyncManager", _stub("SyncManager"));
    _define("TaskAttributionTiming", _stub("TaskAttributionTiming"));
    _define("TaskController", _stub("TaskController"));
    _define("TaskPriorityChangeEvent", _stub("TaskPriorityChangeEvent"));
    _define("TaskSignal", _stub("TaskSignal"));
    _define("TextEvent", _stub("TextEvent"));
    _define("TextFormat", _stub("TextFormat"));
    _define("TextFormatUpdateEvent", _stub("TextFormatUpdateEvent"));
    _define("TextMetrics", _stub("TextMetrics"));
    _define("TextTrack", _stub("TextTrack"));
    _define("TextTrackCue", _stub("TextTrackCue"));
    _define("TextTrackCueList", _stub("TextTrackCueList"));
    _define("TextTrackList", _stub("TextTrackList"));
    _define("TextUpdateEvent", _stub("TextUpdateEvent"));
    _define("TimeRanges", _stub("TimeRanges"));
    _define("TimelineTrigger", _stub("TimelineTrigger"));
    _define("TimelineTriggerRange", _stub("TimelineTriggerRange"));
    _define("TimelineTriggerRangeList", _stub("TimelineTriggerRangeList"));
    _define("ToggleEvent", _stub("ToggleEvent"));
    _define("TrackEvent", _stub("TrackEvent"));
    _define("TransformStreamDefaultController", _stub("TransformStreamDefaultController"));
    _define("TreeWalker", _stub("TreeWalker"));
    _define("TrustedTypePolicy", _stub("TrustedTypePolicy"));
    _define("TrustedTypePolicyFactory", _stub("TrustedTypePolicyFactory"));
    _define("URLPattern", _stub("URLPattern"));
    _define("UserActivation", _stub("UserActivation"));
    _define("VTTCue", _stub("VTTCue"));
    _define("ValidityState", _stub("ValidityState"));
    _define("VideoColorSpace", _stub("VideoColorSpace"));
    _define("VideoFrame", _stub("VideoFrame"));
    _define("VideoPlaybackQuality", _stub("VideoPlaybackQuality"));
    _define("ViewTimeline", _stub("ViewTimeline"));
    _define("ViewTransitionTypeSet", _stub("ViewTransitionTypeSet"));
    _define("Viewport", _stub("Viewport"));
    _define("VirtualKeyboardGeometryChangeEvent", _stub("VirtualKeyboardGeometryChangeEvent"));
    _define("VisibilityStateEntry", _stub("VisibilityStateEntry"));
    _define("WaveShaperNode", _stub("WaveShaperNode"));
    _define("WebGLActiveInfo", _stub("WebGLActiveInfo"));
    _define("WebGLBuffer", _stub("WebGLBuffer"));
    _define("WebGLContextEvent", _stub("WebGLContextEvent"));
    _define("WebGLFramebuffer", _stub("WebGLFramebuffer"));
    _define("WebGLObject", _stub("WebGLObject"));
    _define("WebGLProgram", _stub("WebGLProgram"));
    _define("WebGLQuery", _stub("WebGLQuery"));
    _define("WebGLRenderbuffer", _stub("WebGLRenderbuffer"));
    _define("WebGLSampler", _stub("WebGLSampler"));
    _define("WebGLShader", _stub("WebGLShader"));
    _define("WebGLShaderPrecisionFormat", _stub("WebGLShaderPrecisionFormat"));
    _define("WebGLSync", _stub("WebGLSync"));
    _define("WebGLTexture", _stub("WebGLTexture"));
    _define("WebGLTransformFeedback", _stub("WebGLTransformFeedback"));
    _define("WebGLUniformLocation", _stub("WebGLUniformLocation"));
    _define("WebGLVertexArrayObject", _stub("WebGLVertexArrayObject"));
    _define("WebKitMutationObserver", _stub("WebKitMutationObserver"));
    _define("WebSocketError", _stub("WebSocketError"));
    _define("WebSocketStream", _stub("WebSocketStream"));
    _define("Window", _stub("Window"));
    _define("WindowControlsOverlayGeometryChangeEvent", _stub("WindowControlsOverlayGeometryChangeEvent"));
    _define("XMLDocument", _stub("XMLDocument"));
    _define("XMLHttpRequestEventTarget", _stub("XMLHttpRequestEventTarget"));
    _define("XMLHttpRequestUpload", _stub("XMLHttpRequestUpload"));
    _define("XPathEvaluator", _stub("XPathEvaluator"));
    _define("XPathExpression", _stub("XPathExpression"));
    _define("XPathResult", _stub("XPathResult"));
    
    _define("BarcodeDetector", _stub("BarcodeDetector"));
    _define("FaceDetector", _stub("FaceDetector"));
    _define("TextDetector", _stub("TextDetector"));

    // ---- Event handlers (120) ----
    // Event handlers — Chrome 147 exposes ~120 on* accessors on Window.
    // All return null, no observable behaviour.
    (() => {
        const proto = globalThis.Window ? globalThis.Window.prototype : globalThis;
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
            if (!(_h in proto)) {
                let _v = null;
                Object.defineProperty(proto, _h, {
                    get: () => _v,
                    set: function(v) { _v = (typeof v === 'function' || v === null) ? v : null; },
                    configurable: true, enumerable: true,
                });

            }
        }
    })();

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
        "webkitMediaStream": _stub('MediaStream'),
        "webkitRequestAnimationFrame": globalThis.requestAnimationFrame,
        "webkitRequestFileSystem": function webkitRequestFileSystem(){ return undefined; },
        "webkitResolveLocalFileSystemURL": function webkitResolveLocalFileSystemURL(){ return undefined; },
        "webkitSpeechGrammar": _stub('SpeechGrammar'),
        "webkitSpeechGrammarList": _stub('SpeechGrammarList'),
        "webkitSpeechRecognition": _stub('SpeechRecognition'),
        "webkitSpeechRecognitionError": _stub('SpeechRecognitionError'),
        "webkitSpeechRecognitionEvent": _stub('SpeechRecognitionEvent'),
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
