((globalThis) => {
    const ops = Deno && Deno.core && Deno.core.ops;
    // -- Per-page secure-context gating (Phase 7) --------------------
    // The V8 snapshot bootstraps with is_secure_context=true so all
    // [SecureContext]-only Web Platform APIs are baked in. On insecure
    // pages (data:/http:/about:blank) we strip them here to match real
    // Chrome.
    try {
        const _ops = Deno && Deno.core && Deno.core.ops;
        const _isSecure = _ops && _ops.op_is_secure_context && _ops.op_is_secure_context();
        if (!_isSecure) {
            // Methods + globals registered as values in the snapshot.
            // Navigator getters (mediaDevices, clipboard, ...) gate
            // themselves lazily so they don't need stripping.
            try { delete globalThis.Navigator.prototype.getBattery; } catch (_e) {}
            for (const k of ['caches', 'cookieStore', 'IdleDetector', 'EyeDropper', 'WebTransport']) {
                try { delete globalThis[k]; } catch (_e) {}
            }
            // Phase 7 — also strip the constructor *interfaces* for the
            // [SecureContext] APIs. Real Chrome 147 hides these from
            // `Object.getOwnPropertyNames(window)` on insecure pages.
            // Anti-bot scripts hash the global namespace.
            // Also: ApplePaySession, SharedArrayBuffer, webkitAudioContext,
            // DedicatedWorkerGlobalScope, WorkerGlobalScope, CSSPseudoElement
            // are absent from Chrome 147's globalThis on insecure pages —
            // verified via fresh Playwright MCP capture.
            for (const k of [
                "SharedArrayBuffer", "webkitAudioContext",
                "DedicatedWorkerGlobalScope", "WorkerGlobalScope",
                "CSSPseudoElement",
                "ApplePaySession", "AuthenticatorAssertionResponse",
                "AuthenticatorAttestationResponse", "AuthenticatorResponse",
                "BatteryManager", "Bluetooth", "CacheStorage", "CookieStore",
                "Credential", "CredentialsContainer", "DevicePosture",
                "FederatedCredential", "FileSystemDirectoryHandle",
                "FileSystemFileHandle", "FileSystemHandle",
                "FileSystemWritableFileStream", "IdentityCredential",
                "IdentityProvider", "Keyboard", "KeyboardLayoutMap",
                "MediaDevices", "PasswordCredential", "PaymentRequest",
                "Presentation", "PresentationConnection",
                "PublicKeyCredential", "ServiceWorker",
                "ServiceWorkerContainer", "StorageManager", "SubtleCrypto",
                "VirtualKeyboard", "XRSession", "XRSystem",
                // Generic Sensor API — also [SecureContext]
                "Sensor", "Accelerometer", "AbsoluteOrientationSensor",
                "GravitySensor", "Gyroscope", "LinearAccelerationSensor",
                "Magnetometer", "OrientationSensor",
                "RelativeOrientationSensor",
            ]) {
                try { delete globalThis[k]; } catch (_e) {}
            }
            // crypto.subtle + crypto.randomUUID are [SecureContext]. They
            // come from deno_core's crypto extension and are non-configurable
            // own properties. `delete` fails — replace `globalThis.crypto`
            // with a Proxy that hides those two keys.
            if (globalThis.crypto) {
                const _origCrypto = globalThis.crypto;
                const _maskedCrypto = new Proxy(_origCrypto, {
                    get(target, prop, receiver) {
                        if (prop === 'subtle' || prop === 'randomUUID') return undefined;
                        const v = Reflect.get(target, prop, receiver);
                        return typeof v === 'function' ? v.bind(target) : v;
                    },
                    has(target, prop) {
                        if (prop === 'subtle' || prop === 'randomUUID') return false;
                        return Reflect.has(target, prop);
                    },
                    ownKeys(target) {
                        return Reflect.ownKeys(target).filter(
                            (k) => k !== 'subtle' && k !== 'randomUUID',
                        );
                    },
                    getOwnPropertyDescriptor(target, prop) {
                        if (prop === 'subtle' || prop === 'randomUUID') return undefined;
                        return Reflect.getOwnPropertyDescriptor(target, prop);
                    },
                });
                try {
                    Object.defineProperty(globalThis, 'crypto', {
                        value: _maskedCrypto, configurable: true, enumerable: true, writable: true,
                    });
                } catch (_e) {}
            }
        }
    } catch (_e) { /* secure-context cleanup is best-effort */ }

    // -- Profile-conditional installs --------------------------------
    // These run AFTER the V8 startup snapshot is restored, so the
    // stealth profile is loaded and op-based reads return real values.
    // (Snapshot-time bootstraps see profile=None and would mis-gate.)
    try {
        const _hasProfile = ops && ops.op_has_stealth_profile && ops.op_has_stealth_profile();
        const _osName = (_hasProfile && ops.op_get_profile_value)
            ? (ops.op_get_profile_value("os_name") || "Linux")
            : "Linux";

        // ApplePaySession — present only on macOS Chrome AND only on
        // secure contexts (Apple Pay requires https). Akamai's sensor
        // sends `ap=null` if the constructor is missing on a macOS UA;
        // that mismatch is one of the strongest single tells in the
        // pixel POST capture. Constructor + statics shaped to match
        // Chrome 147's ApplePaySession surface.
        const _ops2 = Deno && Deno.core && Deno.core.ops;
        const _isSecureForAP = _ops2 && _ops2.op_is_secure_context && _ops2.op_is_secure_context();
        if (_osName === "macOS" && _isSecureForAP && typeof globalThis.ApplePaySession === "undefined") {
            const _APP = function ApplePaySession(_version, _paymentRequest) {
                this.onvalidatemerchant = null;
                this.onpaymentauthorized = null;
                this.onpaymentmethodselected = null;
                this.onshippingcontactselected = null;
                this.onshippingmethodselected = null;
                this.oncouponcodechanged = null;
                this.oncancel = null;
            };
            _APP.prototype = {
                begin() {},
                abort() {},
                completeMerchantValidation() {},
                completePayment() {},
                completePaymentMethodSelection() {},
                completeShippingContactSelection() {},
                completeShippingMethodSelection() {},
                completeCouponCodeChange() {},
                addEventListener() {},
                removeEventListener() {},
            };
            _APP.STATUS_SUCCESS = 0;
            _APP.STATUS_FAILURE = 1;
            _APP.STATUS_INVALID_BILLING_POSTAL_ADDRESS = 2;
            _APP.STATUS_INVALID_SHIPPING_POSTAL_ADDRESS = 3;
            _APP.STATUS_INVALID_SHIPPING_CONTACT = 4;
            _APP.STATUS_PIN_REQUIRED = 5;
            _APP.STATUS_PIN_INCORRECT = 6;
            _APP.STATUS_PIN_LOCKOUT = 7;
            _APP.canMakePayments = function canMakePayments() { return true; };
            _APP.canMakePaymentsWithActiveCard = function canMakePaymentsWithActiveCard(_id) { return Promise.resolve(false); };
            _APP.openPaymentSetup = function openPaymentSetup(_id) { return Promise.resolve(false); };
            _APP.supportsVersion = function supportsVersion(version) { return version >= 1 && version <= 14; };
            Object.defineProperty(globalThis, 'ApplePaySession', {
                value: _APP,
                configurable: true,
                writable: true,
            });
        }

        // -- iOS Safari profile: strip 16 declined APIs + add iOS globals --
        // Per Apple's "16 web APIs declined for privacy" policy. The
        // single highest-ROI mobile patch — many leaks vanish at once.
        const _deviceClass = (_hasProfile && ops.op_get_profile_value)
            ? ops.op_get_profile_value("device_class")
            : "Desktop";
        if (_deviceClass === "MobileIOS") {
            // 1. Delete 16 declined APIs from globalThis
            const _iosDeleted = [
                "Bluetooth", "USB", "USBAlternateInterface", "USBConfiguration",
                "USBConnectionEvent", "USBDevice", "USBEndpoint",
                "USBInTransferResult", "USBInterface",
                "USBIsochronousInTransferPacket", "USBIsochronousInTransferResult",
                "USBIsochronousOutPacket", "USBIsochronousOutTransferResult",
                "USBOutTransferResult",
                "HID", "HIDConnectionEvent", "HIDDevice", "HIDInputReportEvent",
                "Serial", "SerialPort",
                "NetworkInformation", "BatteryManager",
                "IdleDetector", "EyeDropper",
                // W1.5 — Chrome-only interfaces real Safari does NOT expose.
                // PerimeterX `'X' in window` tells (research 05_PERIMETERX.md §6.3).
                "UserActivation", "Scheduling",
                "Sensor", "Accelerometer", "AbsoluteOrientationSensor",
                "GravitySensor", "Gyroscope", "LinearAccelerationSensor",
                "Magnetometer", "OrientationSensor", "RelativeOrientationSensor",
                // WebGPU is feature-flagged on iOS 18+ but defaults off
                "GPU", "GPUAdapter", "GPUDevice", "GPUQueue", "GPUBuffer",
                "GPUTexture", "GPUSampler", "GPUBindGroup", "GPUBindGroupLayout",
                "GPUPipelineLayout", "GPUShaderModule", "GPURenderPipeline",
                "GPUComputePipeline", "GPUCommandEncoder", "GPUCommandBuffer",
                "GPURenderPassEncoder", "GPUComputePassEncoder",
                "GPURenderBundleEncoder", "GPURenderBundle", "GPUCanvasContext",
                "GPUColorWrite", "GPUMapMode", "GPUTextureUsage",
                "GPUBufferUsage", "GPUShaderStage",
                // Speech recognition has limited iOS support, but webkit-prefixed
                // is the only form Safari ships
                "SpeechRecognition", "SpeechRecognitionEvent",
                "SpeechRecognitionErrorEvent",
            ];
            for (const k of _iosDeleted) {
                try { delete globalThis[k]; } catch (_e) {}
            }

            // 2. Strip Navigator.prototype methods/getters that iOS doesn't have.
            // Defense in depth: window_bootstrap.js W1.5 gate avoids
            // installing these on iOS profiles, but we also delete here in
            // case any prior pass re-installed them. Use `delete` (not
            // redefine-with-undefined-getter) so `'X' in navigator` returns
            // false — the descriptor must not be present.
            const _NavProto = globalThis.Navigator && globalThis.Navigator.prototype;
            if (_NavProto) {
                for (const k of [
                    "bluetooth", "usb", "serial", "hid", "requestMIDIAccess",
                    "getBattery", "connection", "getInstalledRelatedApps",
                    "scheduling", "userActivation",
                    // userAgentData absent on Safari (no UA-CH at all)
                    "userAgentData",
                    // deviceMemory absent on Safari
                    "deviceMemory",
                ]) {
                    try { delete _NavProto[k]; } catch (_e) {}
                }
            }

            // 3. PaymentRequest.prototype.hasEnrolledInstrument is Chrome/Edge-only
            //    Safari MUST NOT have it.
            if (globalThis.PaymentRequest && globalThis.PaymentRequest.prototype) {
                try { delete globalThis.PaymentRequest.prototype.hasEnrolledInstrument; } catch (_e) {}
            }

            // 4. window.orientation — legacy iOS-only property. Desktop browsers
            //    do NOT have this. Setting to 0 = portrait.
            try {
                Object.defineProperty(globalThis, "orientation", {
                    get: function() { return 0; },
                    configurable: true, enumerable: true,
                });
            } catch (_e) {}

            // 5. ontouchstart on window — every detection script's cheapest
            //    mobile-vs-desktop check
            try {
                Object.defineProperty(globalThis, "ontouchstart", {
                    value: null, configurable: true, writable: true, enumerable: true,
                });
            } catch (_e) {}

            // 6. DeviceMotionEvent.requestPermission + DeviceOrientationEvent.requestPermission
            //    iOS 13+ requires user-gesture-gated permission for these. The presence
            //    of these static methods is itself a strong iOS signal — Android does NOT
            //    expose these statics.
            if (globalThis.DeviceMotionEvent
                && typeof globalThis.DeviceMotionEvent.requestPermission !== "function") {
                try {
                    globalThis.DeviceMotionEvent.requestPermission =
                        function requestPermission() { return Promise.resolve("denied"); };
                } catch (_e) {}
            }
            if (globalThis.DeviceOrientationEvent
                && typeof globalThis.DeviceOrientationEvent.requestPermission !== "function") {
                try {
                    globalThis.DeviceOrientationEvent.requestPermission =
                        function requestPermission() { return Promise.resolve("denied"); };
                } catch (_e) {}
            }

            // 7. Sec-CH-UA-* JS surface absent on Safari — already handled
            //    above via userAgentData getter returning undefined.

            // 8. window.chrome must be absent on iOS Safari. PerimeterX and
            //    others explicitly probe `typeof window.chrome` — Chrome
            //    returns "object", Safari "undefined". Research 05_PERIMETERX
            //    §6.1 names this as one of the iOS kill signals.
            try { delete globalThis.chrome; } catch (_e) {}

            // 8b. navigator.permissions.query() — Safari 18 supports a much
            //     narrower permission name set than Chrome. Per WebKit:
            //     allowed = notifications, push, camera, microphone,
            //               geolocation, persistent-storage.
            //     Chrome-only names (midi, accelerometer, gyroscope,
            //     magnetometer, ambient-light-sensor, background-fetch,
            //     background-sync, clipboard-read, clipboard-write,
            //     display-capture, screen-wake-lock, system-wake-lock,
            //     window-management) must reject with TypeError on Safari
            //     to match real WebKit behavior. PLAN W1.5 (Plan §0 #6).
            try {
                if (globalThis.navigator && globalThis.navigator.permissions) {
                    const _safariAllowed = new Set([
                        'notifications', 'push', 'camera', 'microphone',
                        'geolocation', 'persistent-storage',
                    ]);
                    const _PProto = globalThis.navigator.permissions
                        && Object.getPrototypeOf(globalThis.navigator.permissions);
                    if (_PProto && typeof _PProto.query === 'function') {
                        const _origQuery = _PProto.query;
                        const safariQuery = function query(desc) {
                            const name = desc && typeof desc === 'object' ? desc.name : undefined;
                            if (typeof name !== 'string' || !_safariAllowed.has(name)) {
                                return Promise.reject(new TypeError(
                                    "Failed to execute 'query' on 'Permissions': "
                                    + (typeof name === 'string'
                                        ? "The provided value '" + name + "' is not a valid enum value of type PermissionName."
                                        : "parameter 1 is not of type 'PermissionDescriptor'.")
                                ));
                            }
                            return _origQuery.call(this, desc);
                        };
                        Object.defineProperty(_PProto, 'query', {
                            value: safariQuery, writable: true, enumerable: false, configurable: true,
                        });
                        // Preserve native-shape Function.prototype.toString output
                        // via the _nativeTag symbol installed by stealth_bootstrap.js.
                        const _tag = globalThis._nativeTag;
                        if (_tag) {
                            try { Object.defineProperty(safariQuery, _tag, { value: 'query', configurable: true }); } catch (_e) {}
                            try { Object.defineProperty(safariQuery, 'name', { value: 'query', configurable: true }); } catch (_e) {}
                        }
                    }
                }
            } catch (_e) {}

            // 9. navigator.plugins / navigator.mimeTypes empty on iOS
            //    (PluginArray length 0 is the canonical mobile-Safari shape).
            try {
                if (globalThis.navigator) {
                    const _emptyPlugins = Object.create(globalThis.PluginArray ? globalThis.PluginArray.prototype : null);
                    Object.defineProperty(_emptyPlugins, 'length', { get: () => 0, enumerable: true });
                    Object.defineProperty(_emptyPlugins, 'item', {
                        value: function item() { return null; },
                        writable: true, enumerable: false, configurable: true,
                    });
                    Object.defineProperty(_emptyPlugins, 'namedItem', {
                        value: function namedItem() { return null; },
                        writable: true, enumerable: false, configurable: true,
                    });
                    Object.defineProperty(_emptyPlugins, 'refresh', {
                        value: function refresh() {},
                        writable: true, enumerable: false, configurable: true,
                    });
                    Object.defineProperty(_emptyPlugins, Symbol.iterator, {
                        value: function* () {},
                        writable: true, enumerable: false, configurable: true,
                    });
                    Object.defineProperty(_NavProto, 'plugins', {
                        get: function() { return _emptyPlugins; },
                        configurable: true, enumerable: false,
                    });
                    const _emptyMimeTypes = Object.create(globalThis.MimeTypeArray ? globalThis.MimeTypeArray.prototype : null);
                    Object.defineProperty(_emptyMimeTypes, 'length', { get: () => 0, enumerable: true });
                    Object.defineProperty(_emptyMimeTypes, 'item', {
                        value: function item() { return null; },
                        writable: true, enumerable: false, configurable: true,
                    });
                    Object.defineProperty(_emptyMimeTypes, 'namedItem', {
                        value: function namedItem() { return null; },
                        writable: true, enumerable: false, configurable: true,
                    });
                    Object.defineProperty(_NavProto, 'mimeTypes', {
                        get: function() { return _emptyMimeTypes; },
                        configurable: true, enumerable: false,
                    });
                    // pdfViewerEnabled is false on mobile (no integrated PDF viewer)
                    Object.defineProperty(_NavProto, 'pdfViewerEnabled', {
                        get: function() { return false; },
                        configurable: true, enumerable: false,
                    });
                }
            } catch (_e) {}
        }
    } catch (_e) { /* profile-conditional installs are best-effort */ }

    // -- sfc native-source masking (Kasada `/tl` "sfc" probe) --------
    // Kasada's `/tl` sensor dumps `String(globalThis.<ctor>)` for a
    // rotating list of Web Platform constructors/functions and feeds
    // the result into the dominant ~30-40% browser-fingerprint ML
    // weight. A FRESH decrypted /tl capture (2026-05-15, post console
    // fix) showed 46 of 47 probed names leaking our polyfill source —
    // raw `class Worker {…}` / `function(input, init){…}` bodies, or
    // the wrong native name (constructors that extend our internal
    // EventTarget reported `function EventTarget() { [native code] }`,
    // `clearTimeout` reported `clearInterval`). Real Chrome returns
    // `function <Name>() { [native code] }` for every one of these.
    //
    // This MUST run here, not in stealth_bootstrap.js: the constructors
    // are defined by interfaces/shared_apis/streams/window/worker
    // bootstraps that are concatenated AFTER stealth_bootstrap.js (and
    // shared_apis/worker run at runtime, after the snapshot). This is
    // the universal last pass — it runs always for the page (even from
    // snapshot) and last for workers — and `_maskFunction` is still on
    // globalThis here (the `internals` purge below removes it after).
    try {
        const _mask = globalThis._maskFunction;
        if (typeof _mask === 'function') {
            // De-alias Chrome-distinct pairs our impl points at one
            // object. The fresh /tl `sfc` probe caught these: real
            // Chrome has clearTimeout!==clearInterval,
            // scroll!==scrollTo, DOMMatrix!==DOMMatrixReadOnly — each
            // is its own named native, so a single shared object can't
            // satisfy `String(globalThis[name])` for both names. We
            // split the secondary into a distinct delegator/subclass
            // (more Chrome-faithful; zero behavior change).
            try {
                if (typeof globalThis.clearTimeout === 'function'
                    && globalThis.clearInterval === globalThis.clearTimeout) {
                    const _ct = globalThis.clearTimeout;
                    globalThis.clearInterval = { clearInterval(id) { return _ct(id); } }.clearInterval;
                }
                if (typeof globalThis.scrollTo === 'function'
                    && globalThis.scroll === globalThis.scrollTo) {
                    const _st = globalThis.scrollTo;
                    globalThis.scroll = { scroll() { return _st.apply(this, arguments); } }.scroll;
                }
                if (typeof globalThis.DOMMatrix === 'function'
                    && globalThis.DOMMatrixReadOnly === globalThis.DOMMatrix) {
                    globalThis.DOMMatrixReadOnly = class DOMMatrixReadOnly extends globalThis.DOMMatrix {};
                }
            } catch (_e) {}

            // Native NON-constructor functions must have NO own
            // `prototype` and must be non-constructable (`new fetch()`
            // throws in Chrome). A CLEAN production probe
            // (kasada_native_fn_shape_clean_probe — no capture shim)
            // confirmed setTimeout/setInterval/clearTimeout/
            // clearInterval/queueMicrotask/structuredClone are plain
            // `function` decls → carry `.prototype` + are
            // constructable (CreepJS/Castle bot signal; doc-11 §line94
            // "✓ via _defProtoMethod" was stale for these GLOBALS).
            // `function f(){}`'s `.prototype` is non-configurable so
            // `delete` fails — the only fix is to REPLACE with a
            // method-shorthand (`{[k](){}}[k]`): no `.prototype`,
            // non-constructable, name===k. Forwarding wrapper
            // preserves behavior (none use `this`/`new`). Only the
            // probe-confirmed-broken set is touched; already-correct
            // async/shorthand natives (fetch/atob/btoa/scrollTo/
            // reportError/console.*) are left alone.
            const _natMethod = (holder, key, nm) => {
                try {
                    const o = holder && holder[key];
                    if (typeof o !== 'function') return;
                    if (!Object.prototype.hasOwnProperty.call(o, 'prototype')) {
                        _mask(o, nm || key);
                        return;
                    }
                    const w = { [key]() { return o.apply(this, arguments); } }[key];
                    _mask(w, nm || key);
                    try { holder[key] = w; } catch (_e2) {}
                } catch (_e2) {}
            };
            for (const _k of ['setTimeout', 'setInterval', 'clearTimeout',
                'clearInterval', 'queueMicrotask', 'structuredClone']) {
                _natMethod(globalThis, _k);
            }
            try {
                const _ca = globalThis.chrome && globalThis.chrome.app;
                if (_ca) {
                    for (const _m of ['getDetails', 'getIsInstalled',
                        'installState', 'runningState']) {
                        _natMethod(_ca, _m);
                    }
                }
            } catch (_e) {}

            // (chrome.app.* are handled by _natMethod above — it both
            // native-masks toString [Kasada `sbi` probe leaked
            // "function getDetails() { return null; }"] and removes the
            // illegal `.prototype`/constructability.)
            // The exact 47 names the fresh sensor probed, plus adjacent
            // standard constructors Kasada rotates through — all are
            // genuinely `[native code]` in real Chrome, so masking any
            // that exist on this profile is correct (missing ones are a
            // safe no-op via `_maskFunction`'s `if (!fn) return`).
            // [globalKey, maskName]. maskName differs from globalKey
            // only for the legacy webkit-prefixed aliases: in real
            // Chrome `webkitAudioContext === AudioContext` (same object),
            // so `String(webkitAudioContext)` is
            // `function AudioContext() { [native code] }`. Masking them
            // to their prefixed key would itself be a divergence.
            const _sfcNames = [
                ['webkitMediaStream', 'MediaStream'],
                ['webkitAudioContext', 'AudioContext'],
                ['webkitRTCPeerConnection', 'RTCPeerConnection'],
                'fetch', 'clearTimeout', 'clearInterval', 'setTimeout',
                'setInterval', 'TouchEvent', 'AudioContext', 'OffscreenCanvas',
                'Bluetooth', 'StorageManager', 'scrollTo', 'scroll', 'scrollBy',
                'Worker', 'SharedWorker', 'ServiceWorker', 'WorkerGlobalScope',
                'DedicatedWorkerGlobalScope', 'FileReader', 'ImageBitmap',
                'DOMMatrix', 'DOMMatrixReadOnly', 'PerformanceObserver',
                'PerformanceEntry', 'ReportingObserver', 'ReadableStream',
                'WritableStream', 'TransformStream', 'ReadableStreamDefaultReader',
                'WritableStreamDefaultWriter', 'ReadableStreamDefaultController',
                'BroadcastChannel', 'MessagePort', 'MessageChannel',
                'EventSource', 'CompressionStream', 'DecompressionStream',
                'Crypto', 'SubtleCrypto', 'CloseEvent', 'AbortController',
                'AbortSignal', 'DOMException', 'URL', 'URLSearchParams',
                'FormData', 'Blob', 'File', 'FileList', 'RTCPeerConnection',
                'PressureObserver', 'InputDeviceCapabilities', 'MediaSession',
                'Touch', 'TouchList', 'EyeDropper', 'XMLHttpRequest',
                'XMLHttpRequestUpload', 'WebSocket', 'Notification', 'Image',
                'Audio', 'Headers', 'Request', 'Response', 'createImageBitmap',
                'structuredClone', 'queueMicrotask', 'reportError', 'atob',
                'btoa', 'ResizeObserver', 'IntersectionObserver',
                'MutationObserver', 'TextEncoder', 'TextDecoder', 'EventTarget',
                'Event', 'CustomEvent', 'MediaStream', 'MediaStreamTrack',
                'MediaRecorder', 'DOMRect', 'DOMRectReadOnly', 'DOMPoint',
                'DOMPointReadOnly', 'DOMQuad',
            ];
            for (const _e of _sfcNames) {
                try {
                    const _key = Array.isArray(_e) ? _e[0] : _e;
                    const _nm = Array.isArray(_e) ? _e[1] : _e;
                    const _fn = globalThis[_key];
                    if (typeof _fn === 'function') _mask(_fn, _nm);
                } catch (_e2) {}
            }
        }
    } catch (_e) { /* sfc masking is best-effort */ }

    const internals = [
        'Deno',
        'ops',
        '_maskFunction',
        '_maskAsNative',
        '_nativeTag',
        '_customElementsRegistry',
        '__bootstrap',
        '__browser_oxide',
        '__syncCookiesFromNet',
        '__documentReadyState',
        '__drainCspViolations',
        '__onNodeInserted',
        '__errors',
    ];

    // -- Worker Scope Isolation (Phase 8) ---------------------------
    // Real Chrome Web Workers (DedicatedWorkerGlobalScope) have a very
    // clean namespace. They do NOT expose DOM, CSSOM, or Hardware APIs.
    // If we're in a worker, purge the illegal globals.
    const _isWorker = typeof DedicatedWorkerGlobalScope !== 'undefined' && 
                      globalThis instanceof DedicatedWorkerGlobalScope;
    if (_isWorker) {
        const _workerPurge = [
            'window', 'document', 'history', 'locationbar', 'menubar', 
            'personalbar', 'scrollbars', 'statusbar', 'toolbar', 'frames', 
            'parent', 'top', 'opener', 'frameElement', 'styleMedia', 
            'getComputedStyle', 'getSelection', 'matchMedia', 'alert', 
            'confirm', 'prompt', 'print', 'stop', 'open', 'close', 
            'focus', 'blur', 'moveBy', 'moveTo', 'resizeBy', 'resizeTo', 
            'scroll', 'scrollBy', 'scrollTo', 'requestAnimationFrame', 
            'cancelAnimationFrame', 'requestIdleCallback', 'cancelIdleCallback',
            // Constructors
            'Node', 'Element', 'HTMLElement', 'HTMLDocument', 'Document', 
            'CharacterData', 'Text', 'Comment', 'CDATASection', 'DocumentFragment', 
            'DocumentType', 'NamedNodeMap', 'Attr', 'NodeList', 'HTMLCollection', 
            'HTMLAllCollection', 'DOMTokenList', 'DOMImplementation', 'Range', 
            'Selection', 'DOMParser', 'XMLSerializer', 'XPathEvaluator', 
            'XPathExpression', 'XPathResult', 'XSLTProcessor', 'MutationObserver', 
            'MutationRecord', 'IntersectionObserver', 'ResizeObserver', 
            'PermissionStatus', 'Screen', 'ScreenOrientation', 'VisualViewport',
            'ViewTransition', 'Highlight', 'HighlightRegistry',
            // Hardware/Media (not allowed in workers)
            'Bluetooth', 'USB', 'HID', 'Serial', 'Gamepad', 'GamepadButton', 
            'GamepadEvent', 'GamepadHapticActuator', 'MediaStream', 'MediaStreamTrack', 
            'MediaRecorder', 'RTCPeerConnection', 'RTCDataChannel', 'RTCSessionDescription', 
            'RTCIceCandidate', 'RTCCertificate', 'Presentation', 'PresentationRequest',
            // CSS classes (100+)
            'CSS', 'CSSStyleSheet', 'CSSRule', 'CSSStyleRule', 'CSSMediaRule', 
            'CSSImportRule', 'CSSFontFaceRule', 'CSSPageRule', 'CSSKeyframesRule', 
            'CSSKeyframeRule', 'CSSNamespaceRule', 'CSSSupportsRule', 'CSSCounterStyleRule',
            // ... and all HTML*Element subclasses
        ];
        for (const k of Object.keys(globalThis)) {
            if (k.startsWith('HTML') || k.startsWith('SVG') || k.startsWith('CSS') || _workerPurge.includes(k)) {
                try { delete globalThis[k]; } catch (_) {}
            }
        }
    }

    if (ops && ops.op_cross_origin_isolated && !ops.op_cross_origin_isolated()) {
        internals.push('SharedArrayBuffer');
    }

    for (const name of internals) {
        [globalThis, globalThis.window].forEach(obj => {
            if (!obj || !(name in obj)) return;
            try {
                const success = delete obj[name];
                if (!success) {
                    Object.defineProperty(obj, name, { enumerable: false, configurable: true });
                }
            } catch (e) {
                try {
                    Object.defineProperty(obj, name, { enumerable: false, configurable: true });
                } catch (e2) {}
            }
        });
    }
})(globalThis);
