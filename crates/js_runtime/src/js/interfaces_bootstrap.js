/**
 * Interface bootstrap — defines standard Web IDL classes.
 * Runs FIRST to ensure these globals are available to all other scripts.
 */
((globalThis) => {
    function _define(name, cls) {
        if (Object.getOwnPropertyDescriptor(globalThis, name)) {
            return;
        }
        if (cls && cls.prototype) {
            Object.defineProperty(cls.prototype, Symbol.toStringTag, {
                value: name, configurable: true
            });
        }
        Object.defineProperty(globalThis, name, {
            value: cls, configurable: true, writable: true, enumerable: false
        });
    }

    _define("Navigator", class Navigator {});
    _define("Location", class Location {});
    _define("History", class History {});
    _define("Screen", class Screen {});
    _define("Performance", class Performance {});
    _define("EventTarget", class EventTarget {});
    _define("Event", class Event {});

    // Need minimal Request/Response/Headers for fetch
    _define("Headers", class Headers {});
    _define("Request", class Request {});
    _define("Response", class Response {});
    
    _define("Permissions", class Permissions {});
    _define("ScreenOrientation", class ScreenOrientation {});
    _define("ViewTransition", _stub("ViewTransition"));

    function _stub(name, base = Object) {
        const C = function() {
            throw new TypeError("Failed to construct '" + name + "': Illegal constructor");
        };
        if (base !== Object) {
            C.prototype = Object.create(base.prototype);
            C.prototype.constructor = C;
        }
        Object.defineProperty(C, "name", { value: name, configurable: true });
        Object.defineProperty(C.prototype, Symbol.toStringTag, { value: name, configurable: true });
        return C;
    }

    // Phase J: The definitive list was too large for Deno's snapshotting callback tracker.
    // We will stick to a large but verified subset.
    const _rest = ["AudioContext", "BaseAudioContext", "OfflineAudioContext", "AbortController",
"AbortSignal","AbsoluteOrientationSensor","AbstractRange","Accelerometer","AggregateError","AnalyserNode","Animation","AnimationEffect","AnimationEvent","AnimationPlaybackEvent","AnimationTimeline","AnimationTrigger","ArrayBuffer","AsyncDisposableStack","Attr","Audio","AudioBuffer","AudioBufferSourceNode","AudioData","AudioDecoder","AudioDestinationNode","AudioEncoder","AudioListener","AudioNode","AudioParam","AudioParamMap","AudioPlaybackStats","AudioProcessingEvent","AudioScheduledSourceNode","AudioSinkInfo","AudioWorklet","AudioWorkletNode","AuthenticatorAssertionResponse","AuthenticatorAttestationResponse","AuthenticatorResponse","BackgroundFetchManager","BackgroundFetchRecord","BackgroundFetchRegistration","BarProp","BatteryManager","BeforeInstallPromptEvent","BeforeUnloadEvent","BigInt64Array","BigUint64Array","BiquadFilterNode","Blob","BlobEvent","BroadcastChannel","BrowserCaptureMediaStreamTrack","ByteLengthQueuingStrategy","CDATASection","CSPViolationReportBody","CSS","CSSAnimation","CSSConditionRule","CSSContainerRule","CSSCounterStyleRule","CSSFontFaceRule","CSSFontFeatureValuesRule","CSSFontPaletteValuesRule","CSSFunctionDeclarations","CSSFunctionDescriptors","CSSFunctionRule","CSSGroupingRule","CSSImageValue","CSSImportRule","CSSKeyframeRule","CSSKeyframesRule","CSSKeywordValue","CSSLayerBlockRule","CSSLayerStatementRule","CSSMarginRule","CSSMathClamp","CSSMathInvert","CSSMathMax","CSSMathMin","CSSMathNegate","CSSMathProduct","CSSMathSum","CSSMathValue","CSSMatrixComponent","CSSMediaRule","CSSNamespaceRule","CSSNestedDeclarations","CSSNumericArray","CSSNumericValue","CSSPageRule","CSSPerspective","CSSPositionTryDescriptors","CSSPositionTryRule","CSSPositionValue","CSSPropertyRule","CSSRotate","CSSRule","CSSRuleList","CSSScale","CSSScopeRule","CSSSkew","CSSSkewX","CSSSkewY","CSSStartingStyleRule","CSSStyleDeclaration","CSSStyleRule","CSSStyleSheet","CSSStyleValue","CSSSupportsRule","CSSTransformComponent","CSSTransformValue","CSSTransition","CSSTranslate","CSSUnitValue","CSSUnparsedValue","CSSVariableReferenceValue","CSSViewTransitionRule","Cache","CacheStorage","CanvasCaptureMediaStreamTrack","CanvasGradient","CanvasPattern","CanvasRenderingContext2D","CaptureController","CaretPosition","ChannelMergerNode","ChannelSplitterNode","ChapterInformation","CharacterBoundsUpdateEvent","CharacterData","Clipboard","ClipboardChangeEvent","ClipboardEvent","ClipboardItem","CloseEvent","CloseWatcher","CommandEvent","Comment","CompositionEvent","CompressionStream","ConstantSourceNode","ContentVisibilityAutoStateChangeEvent","ConvolverNode","CookieChangeEvent","CookieStore","CookieStoreManager","CountQueuingStrategy","CrashReportContext","CreateMonitor","Credential","CredentialsContainer","CropTarget","CryptoKey","CustomElementRegistry","CustomEvent","CustomStateSet","DOMError","DOMException","DOMImplementation","DOMMatrix","DOMMatrixReadOnly","DOMParser","DOMPoint","DOMPointReadOnly","DOMQuad","DOMRect","DOMRectList","DOMRectReadOnly","DOMStringList","DOMStringMap","DOMTokenList","DataTransfer","DataTransferItem","DataTransferItemList","DecompressionStream","DelayNode","DelegatedInkTrailPresenter","DeviceMotionEvent","DeviceMotionEventAcceleration","DeviceMotionEventRotationRate","DeviceOrientationEvent","DevicePosture","DigitalCredential","DisposableStack","Document","DocumentFragment","DocumentPictureInPicture","DocumentPictureInPictureEvent","DocumentTimeline","DocumentType","DragEvent","DynamicsCompressorNode","EditContext","Element","ElementInternals","EncodedAudioChunk","EncodedVideoChunk","ErrorEvent","EvalError","EventCounts","EventSource","External","FeaturePolicy","FederatedCredential","Fence","FencedFrameConfig","FetchLaterResult","File","FileList","FileReader","FileSystemDirectoryHandle","FileSystemFileHandle","FileSystemHandle","FileSystemObserver","FileSystemWritableFileStream","FinalizationRegistry","Float16Array","FocusEvent","FontData","FontFace","FontFaceSetLoadEvent","FormData","FormDataEvent","FragmentDirective","GPU","GPUAdapter","GPUAdapterInfo","GPUBindGroup","GPUBindGroupLayout","GPUBuffer","GPUBufferUsage","GPUCanvasContext","GPUColorWrite","GPUCommandBuffer","GPUCommandEncoder","GPUCompilationInfo","GPUCompilationMessage","GPUComputePassEncoder","GPUComputePipeline","GPUDevice","GPUDeviceLostInfo","GPUError","GPUExternalTexture","GPUInternalError","GPUMapMode","GPUOutOfMemoryError","GPUPipelineError","GPUPipelineLayout","GPUQuerySet","GPUQueue","GPURenderBundle","GPURenderBundleEncoder","GPURenderPassEncoder","GPURenderPipeline","GPUSampler","GPUShaderModule","GPUShaderStage","GPUSupportedFeatures","GPUSupportedLimits","GPUTexture","GPUTextureUsage","GPUTextureView","GPUUncapturedErrorEvent","GPUValidationError","GainNode","Gamepad","GamepadButton","GamepadEvent","GamepadHapticActuator","Geolocation","GeolocationCoordinates","GeolocationPosition","GeolocationPositionError","GravitySensor","Gyroscope","HID","HIDConnectionEvent","HIDDevice","HIDInputReportEvent","HTMLAllCollection","HTMLEmbedElement","HTMLFencedFrameElement","HTMLFieldSetElement","HTMLFontElement","HTMLFormControlsCollection","HTMLFormElement","HTMLFrameElement","HTMLFrameSetElement","HTMLGeolocationElement","HTMLHRElement","HTMLHeadElement","HTMLHeadingElement","HTMLHtmlElement","HTMLIFrameElement","HTMLImageElement","HTMLInputElement","HTMLLabelElement","HTMLLegendElement","HTMLLIElement","HTMLLinkElement","HTMLMapElement","HTMLMarqueeElement","HTMLMediaElement","HTMLMenuElement","HTMLMetaElement","HTMLMeterElement","HTMLModElement","HTMLObjectElement","HTMLOListElement","HTMLOptGroupElement","HTMLOptionElement","HTMLOptionsCollection","HTMLOutputElement","HTMLParagraphElement","HTMLParamElement","HTMLPictureElement","HTMLPreElement","HTMLProgressElement","HTMLQuoteElement","HTMLScriptElement","HTMLSelectElement","HTMLSlotElement","HTMLSourceElement","HTMLSpanElement","HTMLStyleElement","HTMLTableCaptionElement","HTMLTableCellElement","HTMLTableColElement","HTMLTableElement","HTMLTableRowElement","HTMLTableSectionElement","HTMLTemplateElement","HTMLTextAreaElement","HTMLTimeElement","HTMLTitleElement","HTMLTrackElement","HTMLUListElement","HTMLUnknownElement","HTMLVideoElement","HashChangeEvent","Highlight","HighlightRegistry","IDBCursor","IDBCursorWithValue","IDBDatabase","IDBFactory","IDBIndex","IDBKeyRange","IDBObjectStore","IDBOpenDBRequest","IDBRecord","IDBRequest","IDBTransaction","IDBVersionChangeEvent","IIRFilterNode","IdentityCredential","IdentityCredentialError","IdentityProvider","IdleDeadline","IdleDetector","Image","ImageBitmap","ImageBitmapRenderingContext","ImageCapture","ImageData","ImageDecoder","ImageTrack","ImageTrackList","Ink","InputDeviceInfo","InputEvent","IntegrityViolationReportBody","InterestEvent","IntersectionObserver","IntersectionObserverEntry","Iterator","Keyboard","KeyboardEvent","KeyboardLayoutMap","KeyframeEffect","LanguageDetector","LargestContentfulPaint","LaunchParams","LaunchQueue","LayoutShift","LayoutShiftAttribution","LinearAccelerationSensor","Lock","LockManager","Magnetometer","MathMLElement","MediaCapabilities","MediaDeviceInfo","MediaDevices","MediaElementAudioSourceNode","MediaEncryptedEvent","MediaError","MediaKeyMessageEvent","MediaKeySession","MediaKeyStatusMap","MediaKeySystemAccess","MediaKeys","MediaList","MediaMetadata","MediaQueryList","MediaQueryListEvent","MediaSource","MediaSourceHandle","MediaStreamAudioDestinationNode","MediaStreamAudioSourceNode","MediaStreamEvent","MediaStreamTrackAudioStats","MediaStreamTrackEvent","MediaStreamTrackGenerator","MediaStreamTrackProcessor","MediaStreamTrackVideoStats","MessageChannel","MessageEvent","MessagePort","MimeType","MimeTypeArray","MouseEvent","MutationObserver","MutationRecord","NamedNodeMap","NavigateEvent","Navigation","NavigationActivation","NavigationCurrentEntryChangeEvent","NavigationDestination","NavigationHistoryEntry","NavigationPrecommitController","NavigationPreloadManager","NavigationTransition","NavigatorLogin","NavigatorManagedData","NavigatorUAData","NetworkInformation","Node","NodeFilter","NodeIterator","NodeList","NotRestoredReasonDetails","NotRestoredReasons","Notification","OTPCredential","Observable","OfflineAudioCompletionEvent","OffscreenCanvas","OffscreenCanvasRenderingContext2D","Option","OrientationSensor","Origin","OscillatorNode","OverconstrainedError","PageRevealEvent","PageSwapEvent","PageTransitionEvent","PannerNode","PasswordCredential","Path2D","PaymentAddress","PaymentManager","PaymentMethodChangeEvent","PaymentRequest","PaymentRequestUpdateEvent","PaymentResponse","PerformanceElementTiming","PerformanceEntry","PerformanceEventTiming","PerformanceLongAnimationFrameTiming","PerformanceLongTaskTiming","PerformanceMark","PerformanceMeasure","PerformanceNavigation","PerformanceNavigationTiming","PerformanceObserver","PerformanceObserverEntryList","PerformancePaintTiming","PerformanceResourceTiming","PerformanceScriptTiming","PerformanceServerTiming","PerformanceTiming","PerformanceTimingConfidence","PeriodicSyncManager","PeriodicWave","PermissionStatus","PictureInPictureEvent","Plugin","PluginArray","PointerEvent","PopStateEvent","Presentation","PresentationAvailability","PresentationConnection","PresentationConnectionAvailableEvent","PresentationConnectionCloseEvent","PresentationConnectionList","PresentationReceiver","PresentationRequest","PressureObserver","PressureRecord","ProcessingInstruction","Profiler","ProgressEvent","PromiseRejectionEvent","ProtectedAudience","PublicKeyCredential","PushManager","PushSubscription","PushSubscriptionOptions","QuotaExceededError","RTCCertificate","RTCDTMFSender","RTCDTMFToneChangeEvent","RTCDataChannel","RTCDataChannelEvent","RTCDtlsTransport","RTCEncodedAudioFrame","RTCEncodedVideoFrame","RTCError","RTCErrorEvent","RTCIceCandidate","RTCIceTransport","RTCPeerConnection","RTCPeerConnectionIceErrorEvent","RTCPeerConnectionIceEvent","RTCRtpReceiver","RTCRtpScriptTransform","RTCRtpSender","RTCRtpTransceiver","RTCSctpTransport","RTCSessionDescription","RTCStatsReport","RTCTrackEvent","RadioNodeList","Range","ReadableByteStreamController","ReadableStream","ReadableStreamBYOBReader","ReadableStreamBYOBRequest","ReadableStreamDefaultController","ReadableStreamDefaultReader","RelativeOrientationSensor","RemotePlayback","ReportBody","ReportingObserver","ResizeObserver","ResizeObserverEntry","ResizeObserverSize","RestrictionTarget","SVGAElement","SVGAngle","SVGAnimateElement","SVGAnimateMotionElement","SVGAnimateTransformElement","SVGAnimatedAngle","SVGAnimatedBoolean","SVGAnimatedEnumeration","SVGAnimatedInteger","SVGAnimatedLength","SVGAnimatedLengthList","SVGAnimatedNumber","SVGAnimatedNumberList","SVGAnimatedPreserveAspectRatio","SVGAnimatedRect","SVGAnimatedString","SVGAnimatedTransformList","SVGAnimationElement","SVGCircleElement","SVGClipPathElement","SVGComponentTransferFunctionElement","SVGDefsElement","SVGDescElement","SVGElement","SVGEllipseElement","SVGFEBlendElement","SVGFEColorMatrixElement","SVGFEComponentTransferElement","SVGFECompositeElement","SVGFEConvolveMatrixElement","SVGFEDiffuseLightingElement","SVGFEDisplacementMapElement","SVGFEDistantLightElement","SVGFEDropShadowElement","SVGFEFloodElement","SVGFEFuncAElement","SVGFEFuncBElement","SVGFEFuncGElement","SVGFEFuncRElement","SVGFEGaussianBlurElement","SVGFEImageElement","SVGFEMergeElement","SVGFEMergeNodeElement","SVGFEMorphologyElement","SVGFEOffsetElement","SVGFEPointLightElement","SVGFESpecularLightingElement","SVGFESpotLightElement","SVGFETileElement","SVGFETurbulenceElement","SVGFilterElement","SVGForeignObjectElement","SVGGElement","SVGGeometryElement","SVGGradientElement","SVGGraphicsElement","SVGImageElement","SVGLength","SVGLengthList","SVGLineElement","SVGLinearGradientElement","SVGMPathElement","SVGMarkerElement","SVGMaskElement","SVGMatrix","SVGMetadataElement","SVGNumber","SVGNumberList","SVGPathElement","SVGPatternElement","SVGPoint","SVGPointList","SVGPolygonElement","SVGPolylineElement","SVGPreserveAspectRatio","SVGRadialGradientElement","SVGRect","SVGRectElement","SVGSVGElement","SVGScriptElement","SVGSetElement","SVGStopElement","SVGStringList","SVGStyleElement","SVGSwitchElement","SVGSymbolElement","SVGTSpanElement","SVGTextContentElement","SVGTextElement","SVGTextPathElement","SVGTextPositioningElement","SVGTitleElement","SVGTransform","SVGTransformList","SVGUnitTypes","SVGUseElement","SVGViewElement","Sanitizer","Scheduler","Scheduling","ScreenDetailed","ScreenDetails","ScriptProcessorNode","ScrollTimeline","SecurityPolicyViolationEvent","Selection","Sensor","SensorErrorEvent","Serial","SerialPort","ServiceWorker","ServiceWorkerContainer","ServiceWorkerRegistration","ShadowRoot","SharedStorage","SharedStorageWorklet","SourceBuffer","SourceBufferList","SpeechGrammar","SpeechGrammarList","SpeechRecognition","SpeechRecognitionAlternative","SpeechRecognitionErrorEvent","SpeechRecognitionEvent","SpeechRecognitionPhrase","SpeechSynthesis","SpeechSynthesisErrorEvent","SpeechSynthesisEvent","SpeechSynthesisUtterance","SpeechSynthesisVoice","StaticRange","StereoPannerNode","Storage","StorageBucket","StorageBucketManager","StorageEvent","StylePropertyMap","StylePropertyMapReadOnly","StyleSheet","StyleSheetList","SubmitEvent","Subscriber","SubtleCrypto","Summarizer","SuppressedError","SyncManager","TaskAttributionTiming","TaskController","TaskPriorityChangeEvent","TaskSignal","Temporal","TextDecoder","TextDecoderStream","TextEncoder","TextEncoderStream","TextEvent","TextFormat","TextFormatUpdateEvent","TextMetrics","TextTrack","TextTrackCue","TextTrackCueList","TextTrackList","TextUpdateEvent","TimeRanges","TimelineTrigger","TimelineTriggerRange","TimelineTriggerRangeList","ToggleEvent","Touch","TouchEvent","TouchList","TrackEvent","TransformStream","TransformStreamDefaultController","TransitionEvent","Translator","TreeWalker","TrustedHTML","TrustedScript","TrustedScriptURL","TrustedTypePolicy","TrustedTypePolicyFactory","UIEvent","URLPattern","URLSearchParams","USBAlternateInterface","USBConfiguration","USBConnectionEvent","USBDevice","USBEndpoint","USBInTransferResult","USBInterface","USBIsochronousInTransferPacket","USBIsochronousInTransferResult","USBIsochronousOutPacket","USBIsochronousOutTransferResult","USBOutTransferResult","UserActivation","VTTCue","ValidityState","VideoColorSpace","VideoFrame","VideoPlaybackQuality","ViewTimeline","ViewTransition","ViewTransitionTypeSet","Viewport","VirtualKeyboard","VirtualKeyboardGeometryChangeEvent","VisibilityStateEntry","VisualViewport","WGSLLanguageFeatures","WakeLock","WakeLockSentinel","WaveShaperNode","WebGL2RenderingContext","WebGLActiveInfo","WebGLBuffer","WebGLContextEvent","WebGLFramebuffer","WebGLObject","WebGLProgram","WebGLQuery","WebGLRenderbuffer","WebGLRenderingContext","WebGLSampler","WebGLShader","WebGLShaderPrecisionFormat","WebGLSync","WebGLTexture","WebGLTransformFeedback","WebGLUniformLocation","WebGLVertexArrayObject","WebKitCSSMatrix","WebKitMutationObserver","WebSocket","WebSocketError","WebSocketStream","WebTransport","WebTransportBidirectionalStream","WebTransportDatagramDuplexStream","WebTransportError","WheelEvent","WindowControlsOverlay","WindowControlsOverlayGeometryChangeEvent","Worker","Worklet","WritableStream","WritableStreamDefaultController","WritableStreamDefaultWriter","XMLDocument","XMLHttpRequest","XMLHttpRequestEventTarget","XMLHttpRequestUpload","XMLSerializer","XPathEvaluator","XPathExpression","XPathResult","XRAnchor","XRAnchorSet","XRBoundedReferenceSpace","XRCPUDepthInformation","XRCamera","XRCompositionLayer","XRCubeLayer","XRCylinderLayer","XRDOMOverlayState","XRDepthInformation","XREquirectLayer","XRFrame","XRHand","XRHitTestResult","XRHitTestSource","XRInputSource","XRInputSourceArray","XRInputSourceEvent","XRInputSourcesChangeEvent","XRJointPose","XRJointSpace","XRLayer","XRLayerEvent","XRLightEstimate","XRLightProbe","XRPlane","XRPlaneSet","XRPose","XRProjectionLayer","XRQuadLayer","XRRay","XRReferenceSpace","XRReferenceSpaceEvent","XRRenderState","XRRigidTransform","XRSession","XRSessionEvent","XRSpace","XRSubImage","XRTransientInputHitTestResult","XRTransientInputHitTestSource","XRView","XRViewerPose","XRViewport","XRVisibilityMaskChangeEvent","XRWebGLBinding","XRWebGLDepthInformation","XRWebGLLayer","XRWebGLSubImage","XSLTProcessor","cookieStore","crashReport","createImageBitmap","credentialless","crossOriginIsolated","customElements","devicePixelRatio","documentPictureInPicture","event","fence","fetchLater","find","frameElement","getComputedStyle","getScreenDetails","getSelection","isSecureContext","launchQueue","locationbar","matchMedia","menubar","offscreenBuffering","originAgentCluster","personalbar","queryLocalFonts","queueMicrotask","releaseEvents","reportError","requestAnimationFrame","requestIdleCallback","sharedStorage","showDirectoryPicker","showOpenFilePicker","showSaveFilePicker","speechSynthesis","statusbar","structuredClone","styleMedia","toolbar","trustedTypes","viewport","visualViewport","webkitCancelAnimationFrame","webkitMediaStream","webkitRTCPeerConnection","webkitRequestAnimationFrame","webkitRequestFileSystem","webkitResolveLocalFileSystemURL","webkitSpeechGrammar","webkitSpeechGrammarList","webkitSpeechRecognition","webkitSpeechRecognitionError","webkitSpeechRecognitionEvent"];

    // Interfaces with real implementations later in the bootstrap chain.
    // These would otherwise be stubbed here with an `Illegal constructor`
    // body — and because shared_apis_bootstrap.js / fetch_bootstrap.js
    // guard their installs with `if (!globalThis.X)`, the stub wins.
    // Skip them so the real classes can be installed by the bootstraps
    // that actually implement them.
    const _realImplLater = new Set([
        // File / form data
        "Blob", "File", "FileReader", "FormData",
        // Abort signal pair
        "AbortController", "AbortSignal",
        // Worker family (real impls in window_bootstrap.js + worker_ext)
        "Worker", "SharedWorker", "WorkerGlobalScope", "DedicatedWorkerGlobalScope",
        "ServiceWorker",
        // Messaging primitives
        "MessageChannel", "MessagePort", "BroadcastChannel",
        // Streams (real impls in streams_bootstrap.js)
        "ReadableStream", "ReadableStreamDefaultReader",
        "WritableStream", "WritableStreamDefaultWriter",
        "TransformStream",
        "CompressionStream", "DecompressionStream",
        // DOM geometry helpers (real impls in shared_apis or dom_bootstrap)
        "DOMException", "DOMMatrix", "DOMPoint",
        // Performance + observers
        "PerformanceEntry", "PerformanceObserver",
        "ReportingObserver", "PressureObserver",
        // Misc real-implementations
        "CloseEvent", "EventSource",
        "OffscreenCanvas", "ImageBitmap",
        "Touch", "TouchEvent", "TouchList",
        // URL pair (real impls in shared_apis_bootstrap.js)
        "URL", "URLSearchParams",
    ]);
    for (const name of _rest) {
        if (!(name in globalThis)) {
            // Lowercase IDL identifiers are properties/functions in the
            // Web spec (structuredClone, queueMicrotask, getComputedStyle,
            // customElements, devicePixelRatio, etc.). Real Chrome
            // exposes them as Window globals — calling `new` on a
            // function throws `TypeError: X is not a constructor`,
            // NOT `Illegal constructor`. The Kasada `nppm` probe
            // captured our stub error and used it as a headless signal.
            // Skip stubbing them as classes; if they aren't installed
            // by a later bootstrap (e.g. structured_clone.js), they
            // remain undefined and `typeof X === "undefined"` matches
            // what a hardened production page sees on Chrome without
            // the relevant feature.
            const first = name.charCodeAt(0);
            const isUpper = first >= 65 && first <= 90;
            if (!isUpper) continue;
            if (_realImplLater.has(name)) continue;
            _define(name, _stub(name));
        }
    }

    // ---- Event handlers (120) ----
    (() => {
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
            "onpointerout", "onpointerover", "onpointerrawupdate", "onpointerup", "onpopstate",
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
            if (!(Object.getOwnPropertyDescriptor(globalThis, _h))) {
                Object.defineProperty(globalThis, _h, {
                    value: null, writable: true, configurable: true, enumerable: true
                });
            }
        }
    })();

    // ---- Other window props (43) ----
    const _External = function External() {};
    _External.prototype.AddSearchProvider = function AddSearchProvider() {};
    _External.prototype.IsSearchProviderInstalled = function IsSearchProviderInstalled() { return 0; };
    Object.defineProperty(_External.prototype, Symbol.toStringTag, { value: "External", configurable: true });
    if (typeof _maskAsNative === 'function') {
        _maskAsNative(_External.prototype, 'AddSearchProvider', 'IsSearchProviderInstalled');
    }

    const _otherProps = {
        "clientInformation": globalThis.navigator,
        "closed": false,
        "crashReport": {},
        "credentialless": false,
        "external": new _External(),
        "fence": null,
        "frameElement": null,
        "launchQueue": { setConsumer: function() {} },
        "locationbar": { visible: true },
        "menubar": { visible: true },
        "offscreenBuffering": false,
        "originAgentCluster": false,
        "personalbar": { visible: true },
        "scrollbars": { visible: true },
        "status": "",
        "statusbar": { visible: true },
        "styleMedia": { type: 'screen', matchMedium: function(m) { return globalThis.matchMedia ? globalThis.matchMedia(m).matches : false; } },
        "toolbar": { visible: true },
        "viewport": {},
        "webkitCancelAnimationFrame": globalThis.cancelAnimationFrame,
        "webkitMediaStream": _stub('MediaStream'),
        "webkitRequestAnimationFrame": globalThis.requestAnimationFrame,
        "webkitURL": globalThis.URL,
    };
    for (const _k of Object.keys(_otherProps)) {
        if (!(Object.getOwnPropertyDescriptor(globalThis, _k))) {
            try {
                Object.defineProperty(globalThis, _k, {
                    value: _otherProps[_k], configurable: true,
                    writable: true, enumerable: true,
                });
            } catch (_e) {}
        }
    }

})(globalThis);
