((globalThis) => {
    const ops = Deno.core.ops;
    const _browser_oxide = {
        __documentReadyState: "loading",
        __pendingNavigation: null,
        __perfResourceEntries: [],
        __fetchLog: [],
        __cspViolations: [],
        __drainCspViolations: () => {
            const v = [..._browser_oxide.__cspViolations];
            _browser_oxide.__cspViolations = [];
            return v;
        }
    };
    Object.defineProperty(globalThis, '_browser_oxide', { value: _browser_oxide, configurable: true, enumerable: false, writable: true });

    if (globalThis.WebAssembly) {
        // Streaming stubs
        WebAssembly.instantiateStreaming = async function(source, importObject) {
            const resp = await source;
            const bytes = await resp.arrayBuffer();
            return WebAssembly.instantiate(bytes, importObject);
        };
        WebAssembly.compileStreaming = async function(source) {
            const resp = await source;
            const bytes = await resp.arrayBuffer();
            return WebAssembly.compile(bytes);
        };
    }

    // Masking helpers are provided by stealth_bootstrap.js
    const _maskFunction = globalThis._maskFunction;
    const _maskAsNative = globalThis._maskAsNative;

    // ── Faithful global `Window` interface (doc 22 ESCALATION).
    // Real Chrome 147 (verified CDP-free): typeof Window==="function",
    // window.constructor.name==="Window", window instanceof Window,
    // window instanceof EventTarget, chain
    //   window → Window.prototype → EventTarget.prototype → Object.prototype.
    // Our engine had NO global Window → fails the most universal
    // real-browser invariant (every anti-bot checks it) and starved
    // _buildRemoteRealm so iframe contentWindow.constructor was "Object".
    (function () {
        if (typeof globalThis.Window === "function") return;
        const _ET = globalThis.EventTarget;
        function Window() {
            throw new TypeError("Illegal constructor");
        }
        try {
            if (typeof _ET === "function") {
                Object.setPrototypeOf(Window, _ET);
                Window.prototype = Object.create(_ET.prototype);
            } else {
                Window.prototype = Object.create(Object.prototype);
            }
            Object.defineProperty(Window.prototype, "constructor", {
                value: Window, writable: true, enumerable: false, configurable: true,
            });
            Object.defineProperty(Window, "name", { value: "Window", configurable: true });
            Object.defineProperty(Window, "prototype", {
                value: Window.prototype, writable: false, enumerable: false, configurable: false,
            });
            Object.defineProperty(Window.prototype, Symbol.toStringTag, {
                value: "Window", configurable: true,
            });
            if (typeof _maskFunction === "function") _maskFunction(Window, "Window");
            globalThis.Window = Window;
            // Symbol.hasInstance: any global proxy where .window===self is a Window
            // (covers both main realm and child realms for iframe instanceof Window)
            Object.defineProperty(Window, Symbol.hasInstance, {
                value: function(instance) {
                    if (instance == null) return false;
                    try { return instance.window === instance; } catch(_) { return false; }
                },
                configurable: true, writable: true
            });
            // NOTE: do NOT Object.setPrototypeOf(globalThis, Window.prototype)
            // here — deno_core's V8 global object is special; swapping its
            // [[Prototype]] breaks `ops`/secure-context resolution
            // (regressed Notification.permission default→denied,
            // chrome147_parity). Defining the global `Window` is enough for
            // `typeof Window==="function"` AND for _buildRemoteRealm to
            // mirror it so iframe contentWindow.constructor.name==="Window".
            // Main-window `window instanceof Window` parity needs a
            // deno_core-level global-prototype fix (Rust side) — tracked
            // separately (doc 22), NOT a JS proto swap.
        } catch (_) {}
    })();

    // --- Global self-references (window, self, top, parent, frames) ---
    // Chrome alignment — handled by Deno defaults for now to avoid snapshot conflicts.

    // Helper: read from stealth profile or use default
    const _p = (key, fallback) => {
        if (ops.op_has_stealth_profile()) {
            const v = ops.op_get_profile_value(key);
            return v !== "" ? v : fallback;
        }
        return fallback;
    };

    // Helper: is the document a secure context? Drives the IDL
    // `[SecureContext]` extended attribute — gates the ~18 modern
    // Web Platform APIs (mediaDevices, serviceWorker, clipboard,
    // credentials, usb, etc.) so they're undefined on
    // about:blank/data:/http: but defined on https:/wss:/file:/
    // http://localhost. Phase 7 fix.
    const _secure = () => ops.op_is_secure_context();
    const _pInt = (key, fallback) => {
        const v = _p(key, "");
        return v !== "" ? parseInt(v, 10) : fallback;
    };
    const _pFloat = (key, fallback) => {
        const v = _p(key, "");
        return v !== "" ? parseFloat(v) : fallback;
    };
    const _pJson = (key, fallback) => {
        const v = _p(key, "");
        if (v !== "") try { return JSON.parse(v); } catch {}
        return fallback;
    };

    // W1.5 — iOS surface gating. Real iOS Safari has no UA-Client-Hints,
    // no `chrome` global, and no NetworkInformation / UserActivation /
    // deviceMemory / scheduling / IdleDetector / getInstalledRelatedApps.
    // PerimeterX checks `('chrome' in window) || ('userActivation' in
    // navigator) || ('deviceMemory' in navigator) || ('connection' in
    // navigator)` against an iOS UA — any positive hit adds ~50 risk
    // points (research 05_PERIMETERX.md §6.3). These are Chrome-family
    // APIs that must not appear on the iPhone 15 Pro Safari profile.
    const _isMobileIOS = () => _p("device_class", "Desktop") === "MobileIOS";

    // ================================================================
    // Prototype-install helpers — kNoScriptId-safe layout
    // ================================================================
    const _defProtoGetter = (proto, name, getter, setter) => {
        Object.defineProperty(proto, name, {
            get: getter,
            set: setter,
            enumerable: false,
            configurable: true,
        });
        _maskFunction(getter, `get ${name}`);
        if (setter) _maskFunction(setter, `set ${name}`);
    };
    const _defProtoMethod = (proto, name, fn) => {
        // WebIDL methods are NOT constructors. Using object literal
        // method shorthand ensures the function lacks a [[Construct]]
        // internal slot. Preserve fn.length so Function.prototype.length
        // reflection matches real Chrome's WebIDL method arity (e.g.
        // addEventListener=1, getUserMedia=1, enumerateDevices=0).
        const wrapped = ({ [name](...args) { return fn.apply(this, args); } })[name];
        try {
            Object.defineProperty(wrapped, 'length', { value: fn.length, configurable: true });
        } catch (_) {}
        Object.defineProperty(proto, name, {
            value: wrapped, writable: true, enumerable: false, configurable: true,
        });
        _maskFunction(wrapped, name);
    };

    // ================================================================
    // Navigator class + prototype — kNoScriptId-safe layout
    // ================================================================
    const _NavProto = globalThis.Navigator.prototype;
    const _defNav = (name, getter) => _defProtoGetter(_NavProto, name, getter);
    const _defNavMethod = (name, fn) => _defProtoMethod(_NavProto, name, fn);

    // Stable-object references — object getters return the same reference
    // on every call, matching the behavior of real DOM-wrapped properties.
    let NetworkInformation = globalThis.NetworkInformation || class NetworkInformation extends EventTarget {
        constructor() { super(); }
    };
    const _navConnection = Object.create(NetworkInformation.prototype);
    Object.defineProperty(_navConnection, 'effectiveType', { get: () => _p("connection_effective_type", "4g"), enumerable: true });
    Object.defineProperty(_navConnection, 'rtt', { get: () => Math.round(_pInt("connection_rtt", 50) / 25) * 25, enumerable: true });
    Object.defineProperty(_navConnection, 'downlink', { get: () => Math.round(_pFloat("connection_downlink", 10) * 40) / 40, enumerable: true });
    Object.defineProperty(_navConnection, 'saveData', { get: () => false, enumerable: true });
    Object.defineProperty(_navConnection, 'downlinkMax', { get: () => Infinity, enumerable: true });
    _navConnection.onchange = null;

    let PluginArray = globalThis.PluginArray || class PluginArray {};
    let MimeTypeArray = globalThis.MimeTypeArray || class MimeTypeArray {};
    let Plugin = globalThis.Plugin || class Plugin {};
    let MimeType = globalThis.MimeType || class MimeType {};

    // 1. Setup Plugin.prototype
    const _PluginProto = Plugin.prototype;
    Object.defineProperty(_PluginProto, Symbol.toStringTag, { value: "Plugin", enumerable: false, configurable: true });
    _defProtoGetter(_PluginProto, 'name', function() { return this._name || ""; });
    _defProtoGetter(_PluginProto, 'description', function() { return this._desc || ""; });
    _defProtoGetter(_PluginProto, 'filename', function() { return this._file || ""; });
    _defProtoGetter(_PluginProto, 'length', function() { return 0; });
    _defProtoMethod(_PluginProto, 'item', function item() { return null; });
    _defProtoMethod(_PluginProto, 'namedItem', function namedItem() { return null; });

    // Setup MimeType.prototype
    const _MimeTypeProto = MimeType.prototype;
    Object.defineProperty(_MimeTypeProto, Symbol.toStringTag, { value: "MimeType", enumerable: false, configurable: true });
    _defProtoGetter(_MimeTypeProto, 'type', function() { return this._type || ""; });
    _defProtoGetter(_MimeTypeProto, 'description', function() { return this._desc || ""; });
    _defProtoGetter(_MimeTypeProto, 'suffixes', function() { return this._suffixes || ""; });
    _defProtoGetter(_MimeTypeProto, 'enabledPlugin', function() { return this._plugin || null; });

    const _makePlugin = (name, desc, file) => {
        const p = Object.create(_PluginProto);
        Object.defineProperty(p, '_name', { value: name });
        Object.defineProperty(p, '_desc', { value: desc });
        Object.defineProperty(p, '_file', { value: file });
        Object.defineProperty(p, '_mimeTypes', { value: [], writable: true });
        return p;
    };
    const _makeMime = (type, suffixes, desc, plugin) => {
        const m = Object.create(_MimeTypeProto);
        Object.defineProperty(m, '_type', { value: type });
        Object.defineProperty(m, '_suffixes', { value: suffixes });
        Object.defineProperty(m, '_desc', { value: desc });
        Object.defineProperty(m, '_plugin', { value: plugin });
        return m;
    };

    // Canonical Chrome 133 plugin set. All real Chrome 133 browsers ship
    // exactly these 5 plugins + 2 mime types; the profile fields
    // plugins_count / mime_types_count let a profile CLAIM a subset.
    //
    // IMPORTANT: the bootstrap runs at V8-snapshot-build time with NO
    // stealth profile installed, so any eager `_pInt` read here captures
    // the default (5/2) into the snapshot. Count resolution MUST happen
    // lazily via getters so each runtime navigator.plugins.length call
    // reads the live profile.
    const _PDF_DESC = "Portable Document Format";
    const _PDF_FILE = "internal-pdf-viewer";
    const _allPlugins = [
        _makePlugin("PDF Viewer", _PDF_DESC, _PDF_FILE),
        _makePlugin("Chrome PDF Viewer", _PDF_DESC, _PDF_FILE),
        _makePlugin("Chromium PDF Viewer", _PDF_DESC, _PDF_FILE),
        _makePlugin("Microsoft Edge PDF Viewer", _PDF_DESC, _PDF_FILE),
        _makePlugin("WebKit built-in PDF", _PDF_DESC, _PDF_FILE),
    ];
    const _allMimes = [
        _makeMime("application/pdf", "pdf", _PDF_DESC, _allPlugins[0]),
        _makeMime("text/pdf", "pdf", _PDF_DESC, _allPlugins[0]),
    ];
    // Cross-link: each plugin reports the same mime type list per Chrome.
    _allPlugins.forEach(p => { p._mimeTypes = _allMimes; });

    // Runtime count resolvers — clamped to physical array size so a probe
    // that walks plugins[i] for i<length never hits undefined.
    //
    // Memoized on first call so repeated probes (creepjs's per-property
    // lie-detection spreads `[...navigator.plugins]` for every WebIDL
    // member it audits) hit a cached number instead of re-reading the
    // profile and re-computing the clamp on each numeric-index getter.
    // The first call still happens at runtime — by which time the profile
    // IS installed (we cannot eager-cache here because window_bootstrap.js
    // is evaluated at snapshot build-time without a profile).
    let _pluginsLenCache = -1;
    const _pluginsLen = () => {
        if (_pluginsLenCache < 0) {
            _pluginsLenCache = Math.max(0, Math.min(_allPlugins.length, _pInt("plugins_count", _allPlugins.length)));
        }
        return _pluginsLenCache;
    };
    let _mimesLenCache = -1;
    const _mimesLen = () => {
        if (_mimesLenCache < 0) {
            _mimesLenCache = Math.max(0, Math.min(_allMimes.length, _pInt("mime_types_count", _allMimes.length)));
        }
        return _mimesLenCache;
    };

    // 2. Setup PluginArray.prototype — length + item() dispatch via live count.
    const _PluginArrayProto = PluginArray.prototype;
    Object.defineProperty(_PluginArrayProto, Symbol.toStringTag, { value: "PluginArray", enumerable: false, configurable: true });
    Object.defineProperty(_PluginArrayProto, 'length', { get: () => _pluginsLen(), enumerable: false, configurable: true });
    _defProtoMethod(_PluginArrayProto, 'item', function item(i) {
        const n = _pluginsLen();
        return (i >= 0 && i < n) ? _allPlugins[i] : null;
    });
    _defProtoMethod(_PluginArrayProto, 'namedItem', function namedItem(n) {
        const len = _pluginsLen();
        for (let i = 0; i < len; i++) if (_allPlugins[i].name === n) return _allPlugins[i];
        return null;
    });
    
    // (Phase J) Add numeric accessors for navigator.plugins[0] etc.
    for (let i = 0; i < 5; i++) {
        Object.defineProperty(_PluginArrayProto, i, {
            get: function() { return this.item(i); },
            enumerable: true, configurable: true
        });
    }

    _defProtoMethod(_PluginArrayProto, 'refresh', () => {});
    // Symbol.iterator iterates the live sliced range.
    Object.defineProperty(_PluginArrayProto, Symbol.iterator, {
        value: function iter() {
            const n = _pluginsLen();
            let i = 0;
            const self = this;
            return {
                next() {
                    if (i < n) return { value: self[i++], done: false };
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]() { return this; }
            };
        },
        configurable: true,
    });

    // Setup MimeTypeArray.prototype — same pattern.
    const _MimeTypeArrayProto = MimeTypeArray.prototype;
    Object.defineProperty(_MimeTypeArrayProto, Symbol.toStringTag, { value: "MimeTypeArray", enumerable: false, configurable: true });
    Object.defineProperty(_MimeTypeArrayProto, 'length', { get: () => _mimesLen(), enumerable: false, configurable: true });
    _defProtoMethod(_MimeTypeArrayProto, 'item', function item(i) {
        const n = _mimesLen();
        return (i >= 0 && i < n) ? _allMimes[i] : null;
    });
    _defProtoMethod(_MimeTypeArrayProto, 'namedItem', function namedItem(n) {
        const len = _mimesLen();
        for (let i = 0; i < len; i++) if (_allMimes[i].type === n) return _allMimes[i];
        return null;
    });
    // Numeric accessors for mimeTypes
    for (let i = 0; i < 2; i++) {
        Object.defineProperty(_MimeTypeArrayProto, i, {
            get: function() { return this.item(i); },
            enumerable: true, configurable: true
        });
    }
    Object.defineProperty(_MimeTypeArrayProto, Symbol.iterator, {
        value: function iter() {
            const n = _mimesLen();
            let i = 0;
            const self = this;
            return {
                next() {
                    if (i < n) return { value: self[i++], done: false };
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]() { return this; }
            };
        },
        configurable: true,
    });

    // Instance: install index accessors that gate on live count.
    const _navPlugins = Object.create(_PluginArrayProto);
    _allPlugins.forEach((p, i) => {
        Object.defineProperty(_navPlugins, i, {
            get: () => (i < _pluginsLen() ? p : undefined),
            enumerable: true,
            configurable: true,
        });
        // Named getter for plugin name
        Object.defineProperty(_navPlugins, p.name, {
            get: () => (i < _pluginsLen() ? p : undefined),
            enumerable: false,
            configurable: true,
        });
    });

    // Plugin instance behaves like a MimeTypeArray over its mime types.
    _allPlugins.forEach(p => {
        Object.defineProperty(p, 'length', { get: () => _mimesLen(), enumerable: false, configurable: true });
        p._mimeTypes.forEach((m, i) => {
             Object.defineProperty(p, i, {
                get: () => (i < _mimesLen() ? m : undefined),
                enumerable: true,
                configurable: true,
            });
            Object.defineProperty(p, m.type, {
                get: () => (i < _mimesLen() ? m : undefined),
                enumerable: false,
                configurable: true,
            });
        });
        Object.defineProperty(p, 'item', {
            value: function item(i) {
                const n = _mimesLen();
                return (i >= 0 && i < n) ? p._mimeTypes[i] : null;
            },
            enumerable: false, configurable: true,
        });
        Object.defineProperty(p, 'namedItem', {
            value: function namedItem(n) {
                const len = _mimesLen();
                for (let i = 0; i < len; i++) if (p._mimeTypes[i].type === n) return p._mimeTypes[i];
                return null;
            },
            enumerable: false, configurable: true,
        });
        // Mask the per-instance item/namedItem so toString returns
        // `function NAME() { [native code] }` instead of leaking source.
        // Kasada blob field `npn1` captured the unmasked source string.
        try { _maskAsNative(p, 'item', 'namedItem'); } catch (_) {}
    });

    const _navMimeTypes = Object.create(_MimeTypeArrayProto);
    _allMimes.forEach((m, i) => {
        Object.defineProperty(_navMimeTypes, i, {
            get: () => (i < _mimesLen() ? m : undefined),
            enumerable: true,
            configurable: true,
        });
        // Named getter for mime type
        Object.defineProperty(_navMimeTypes, m.type, {
            get: () => (i < _mimesLen() ? m : undefined),
            enumerable: false,
            configurable: true,
        });
    });

    let MediaDevices = globalThis.MediaDevices || class MediaDevices {};
    const _navMediaDevices = Object.create(MediaDevices.prototype);
    // enumerateDevices: apply the two spec behaviors real Chrome does and we
    // previously missed:
    //   (1) WebIDL camelCase on output (deviceId / groupId — NOT snake_case).
    //       The profile ships snake_case; we transform here.
    //   (2) label === "" until the corresponding permission is GRANTED
    //       (audioinput/audiooutput → microphone; videoinput → camera).
    //       Leaking populated labels pre-permission is a classic automation
    //       tell. _PERMISSION_STATE_MAP is defined below; reference resolves
    //       lazily when this function is called. §6.6 item 9 / item 7.
    _navMediaDevices.enumerateDevices = ({
        enumerateDevices() {
            const raw = _pJson("media_devices", []);
            const permFor = (kind) => {
                if (kind === "videoinput") return _PERMISSION_STATE_MAP["camera"] || "prompt";
                if (kind === "audioinput" || kind === "audiooutput") return _PERMISSION_STATE_MAP["microphone"] || "prompt";
                return "granted"; // unknown kinds — don't blank
            };
            const out = raw.map((d) => {
                const granted = permFor(d.kind) === "granted";
                const deviceId = granted ? (d.deviceId != null ? d.deviceId : (d.device_id || "")) : "";
                const groupId = granted ? (d.groupId != null ? d.groupId : (d.group_id || "")) : "";
                const label = granted ? (d.label || "") : "";
                return { deviceId, kind: d.kind || "", label, groupId };
            });
            return Promise.resolve(out);
        }
    }).enumerateDevices;
    _maskFunction(_navMediaDevices.enumerateDevices, 'enumerateDevices');

    _navMediaDevices.getUserMedia = ({ getUserMedia() { return Promise.reject(new Error("Permission denied")); } }).getUserMedia;
    _maskFunction(_navMediaDevices.getUserMedia, 'getUserMedia');

    _navMediaDevices.getDisplayMedia = ({ getDisplayMedia() { return Promise.reject(new Error("Permission denied")); } }).getDisplayMedia;
    _maskFunction(_navMediaDevices.getDisplayMedia, 'getDisplayMedia');

    _navMediaDevices.getSupportedConstraints = ({
        getSupportedConstraints() {
            return { aspectRatio: true, autoGainControl: true, brightness: true, channelCount: true, colorTemperature: true, contrast: true, deviceId: true, displaySurface: true, echoCancellation: true, exposureCompensation: true, exposureMode: true, exposureTime: true, facingMode: true, focusDistance: true, focusMode: true, frameRate: true, groupId: true, height: true, iso: true, latency: true, noiseSuppression: true, pan: true, pointsOfInterest: true, resizeMode: true, sampleRate: true, sampleSize: true, saturation: true, sharpness: true, suppressLocalAudioPlayback: true, tilt: true, torch: true, whiteBalanceMode: true, width: true, zoom: true };
        }
    }).getSupportedConstraints;
    _maskFunction(_navMediaDevices.getSupportedConstraints, 'getSupportedConstraints');

    _navMediaDevices.addEventListener = ({ addEventListener() {} }).addEventListener;
    _maskFunction(_navMediaDevices.addEventListener, 'addEventListener');

    _navMediaDevices.removeEventListener = ({ removeEventListener() {} }).removeEventListener;
    _maskFunction(_navMediaDevices.removeEventListener, 'removeEventListener');

    _navMediaDevices.dispatchEvent = ({ dispatchEvent() { return true; } }).dispatchEvent;
    _maskFunction(_navMediaDevices.dispatchEvent, 'dispatchEvent');

    // Permission name → state map matching headed Chrome defaults.
    // W3C PermissionState enum: 'granted' | 'denied' | 'prompt'. Headless
    // Chrome's well-known 'denied' return for notifications is the single
    // biggest fingerprint tell this function fixes.
    const _PERMISSION_STATE_MAP = {
        "notifications": "prompt",
        "geolocation": "prompt",
        "camera": "prompt",
        "microphone": "prompt",
        "midi": "prompt",
        "push": "prompt",
        "persistent-storage": "granted",
        "background-sync": "granted",
        "background-fetch": "granted",
        "clipboard-read": "prompt",
        "clipboard-write": "granted",
        "payment-handler": "granted",
        "accelerometer": "granted",
        "gyroscope": "granted",
        "magnetometer": "granted",
        "ambient-light-sensor": "granted",
        "screen-wake-lock": "granted",
        "nfc": "prompt",
        "display-capture": "prompt",
        "window-management": "prompt",
    };

    // Permissions that require secure context — on data:/http:/about:blank
    // these report "denied" instead of "prompt" per Permissions Policy
    // (geolocation, camera, mic, midi, etc. are all [SecureContext] APIs
    // and the corresponding permissions are unobtainable). Phase 7.
    const _SC_GATED_PERMISSIONS = new Set([
        "geolocation", "camera", "microphone", "midi", "push",
        "notifications", "clipboard-read",
        "nfc", "display-capture", "window-management",
    ]);

    class PermissionStatus extends EventTarget {
        constructor(name) { super(); this._name = name; }
        get name() { return this._name; }
        get state() {
            if (!_secure() && _SC_GATED_PERMISSIONS.has(this._name)) {
                return "denied";
            }
            return _PERMISSION_STATE_MAP[this._name] || "prompt";
        }
        get onchange() { return null; }
        set onchange(_v) {}
    }
    Object.defineProperty(PermissionStatus.prototype, Symbol.toStringTag, {
        value: "PermissionStatus", configurable: true,
    });
    globalThis.PermissionStatus = PermissionStatus;

    class Permissions {}
    Object.defineProperty(Permissions.prototype, Symbol.toStringTag, {
        value: "Permissions", configurable: true,
    });
    _defProtoMethod(Permissions.prototype, 'query', function query(desc) {
        if (desc == null || typeof desc !== 'object') {
            return Promise.reject(new TypeError(
                "Failed to execute 'query' on 'Permissions': parameter 1 is not of type 'PermissionDescriptor'."
            ));
        }
        const name = desc.name;
        if (typeof name !== 'string' || !(name in _PERMISSION_STATE_MAP)) {
            return Promise.reject(new TypeError(
                "Failed to execute 'query' on 'Permissions': The provided value '" +
                String(name) + "' is not a valid enum value of type PermissionName."
            ));
        }
        return Promise.resolve(new PermissionStatus(name));
    });
    globalThis.Permissions = Permissions;
    const _navPermissions = Object.create(Permissions.prototype);

    // ================================================================
    // WebAuthn + FedCM (detection-shape only)
    // ----------------------------------------------------------------
    // Anti-bot vendors (DataDome 2025+, Kasada 2024+) probe:
    //   typeof window.PublicKeyCredential
    //   PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()
    //   PublicKeyCredential.isConditionalMediationAvailable()
    //   PublicKeyCredential.getClientCapabilities()  (Chrome 133+)
    //   navigator.credentials.create({publicKey:...}) — must reject as
    //     NotAllowedError after a realistic delay (not synchronous TypeError)
    //   navigator.credentials.get({identity:...}) — FedCM branch
    //   typeof IdentityCredential, typeof IdentityProvider
    // No real authenticator is implemented — this is a shape stub. Profile
    // fields has_platform_authenticator and conditional_mediation drive the
    // resolved values.
    // ================================================================

    class AuthenticatorResponse {}
    Object.defineProperty(AuthenticatorResponse.prototype, Symbol.toStringTag,
        { value: "AuthenticatorResponse", configurable: true });
    class AuthenticatorAttestationResponse extends AuthenticatorResponse {}
    Object.defineProperty(AuthenticatorAttestationResponse.prototype, Symbol.toStringTag,
        { value: "AuthenticatorAttestationResponse", configurable: true });
    class AuthenticatorAssertionResponse extends AuthenticatorResponse {}
    Object.defineProperty(AuthenticatorAssertionResponse.prototype, Symbol.toStringTag,
        { value: "AuthenticatorAssertionResponse", configurable: true });
    globalThis.AuthenticatorResponse = AuthenticatorResponse;
    globalThis.AuthenticatorAttestationResponse = AuthenticatorAttestationResponse;
    globalThis.AuthenticatorAssertionResponse = AuthenticatorAssertionResponse;

    class PublicKeyCredential {
        constructor() { throw new TypeError("Illegal constructor"); }
    }
    Object.defineProperty(PublicKeyCredential.prototype, Symbol.toStringTag,
        { value: "PublicKeyCredential", configurable: true });
    PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable = ({
        isUserVerifyingPlatformAuthenticatorAvailable() {
            return Promise.resolve(_p("has_platform_authenticator", "false") === "true");
        }
    }).isUserVerifyingPlatformAuthenticatorAvailable;
    _maskFunction(PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable, 'isUserVerifyingPlatformAuthenticatorAvailable');

    PublicKeyCredential.isConditionalMediationAvailable = ({
        isConditionalMediationAvailable() {
            return Promise.resolve(_p("conditional_mediation", "true") === "true");
        }
    }).isConditionalMediationAvailable;
    _maskFunction(PublicKeyCredential.isConditionalMediationAvailable, 'isConditionalMediationAvailable');

    // Chrome 133+ surface — see web.dev/articles/webauthn-client-capabilities.
    PublicKeyCredential.getClientCapabilities = ({
        getClientCapabilities() {
            const uvpa = _p("has_platform_authenticator", "false") === "true";
            return Promise.resolve({
                conditionalCreate: false,
                conditionalGet: true,
                hybridTransport: true,
                passkeyPlatformAuthenticator: uvpa,
                userVerifyingPlatformAuthenticator: uvpa,
                relatedOrigins: true,
                signalAllAcceptedCredentials: true,
                signalCurrentUserDetails: true,
                signalUnknownCredential: true,
            });
        }
    }).getClientCapabilities;
    _maskFunction(PublicKeyCredential.getClientCapabilities, 'getClientCapabilities');
    globalThis.PublicKeyCredential = PublicKeyCredential;

    class IdentityCredential {
        constructor() { throw new TypeError("Illegal constructor"); }
    }
    Object.defineProperty(IdentityCredential.prototype, Symbol.toStringTag,
        { value: "IdentityCredential", configurable: true });
    globalThis.IdentityCredential = IdentityCredential;

    class IdentityProvider {}
    IdentityProvider.getUserInfo = ({
        getUserInfo() {
            return Promise.reject(new DOMException("Not allowed", "NotAllowedError"));
        }
    }).getUserInfo;
    _maskFunction(IdentityProvider.getUserInfo, 'getUserInfo');
    globalThis.IdentityProvider = IdentityProvider;

    function _fedcmGet(_identity) {
        // No real IdP wiring. Reject the way Chrome does after the user dismisses
        // (NotAllowedError) — anti-bot probes only assert reject-shape + delay.
        return new Promise((_, rej) => setTimeout(() =>
            rej(new DOMException("User declined or no eligible accounts.",
                "NotAllowedError")), 200));
    }

    class CredentialsContainer {}
    Object.defineProperty(CredentialsContainer.prototype, Symbol.toStringTag,
        { value: "CredentialsContainer", configurable: true });
    CredentialsContainer.prototype.create = ({
        create(opts) {
            if (!opts || typeof opts !== "object") {
                return Promise.reject(new TypeError(
                    "Failed to execute 'create' on 'CredentialsContainer': 1 argument required."));
            }
            if (opts.publicKey) {
                // Realistic ~120 ms delay then NotAllowedError — matches Chrome with no UV.
                return new Promise((_, rej) => setTimeout(() =>
                    rej(new DOMException(
                        "The operation either timed out or was not allowed; see https://www.w3.org/TR/webauthn-2/#sctn-privacy-considerations-client.",
                        "NotAllowedError")), 120));
            }
            return Promise.resolve(null);
        }
    }).create;
    _maskFunction(CredentialsContainer.prototype.create, 'create');

    CredentialsContainer.prototype.get = ({
        get(opts) {
            if (opts && opts.identity) return _fedcmGet(opts.identity);
            if (opts && opts.publicKey) {
                return new Promise((_, rej) => setTimeout(() =>
                    rej(new DOMException(
                        "The operation either timed out or was not allowed; see https://www.w3.org/TR/webauthn-2/#sctn-privacy-considerations-client.",
                        "NotAllowedError")), 120));
            }
            return Promise.resolve(null);
        }
    }).get;
    _maskFunction(CredentialsContainer.prototype.get, 'get');

    CredentialsContainer.prototype.store = ({ store() { return Promise.resolve(undefined); } }).store;
    _maskFunction(CredentialsContainer.prototype.store, 'store');

    CredentialsContainer.prototype.preventSilentAccess = ({ preventSilentAccess() { return Promise.resolve(undefined); } }).preventSilentAccess;
    _maskFunction(CredentialsContainer.prototype.preventSilentAccess, 'preventSilentAccess');

    globalThis.CredentialsContainer = CredentialsContainer;
    const _navCredentials = Object.create(CredentialsContainer.prototype);

    class Bluetooth extends EventTarget {
        constructor() { super(); }
    }
    Object.defineProperty(Bluetooth.prototype, Symbol.toStringTag, {
        value: "Bluetooth", configurable: true,
    });
    Bluetooth.prototype.getAvailability = ({ getAvailability() { return Promise.resolve(false); } }).getAvailability;
    _maskFunction(Bluetooth.prototype.getAvailability, 'getAvailability');

    Bluetooth.prototype.requestDevice = ({ requestDevice() { return Promise.reject(new DOMException("User denied", "NotFoundError")); } }).requestDevice;
    _maskFunction(Bluetooth.prototype.requestDevice, 'requestDevice');

    globalThis.Bluetooth = Bluetooth;
    const _navBluetooth = Object.create(Bluetooth.prototype);

    // Phase 7 — instances need Symbol.toStringTag so
    // `Object.prototype.toString.call(navigator.usb)` returns
    // `[object USB]` (not `[object Object]`). Real Chrome's IDL gives
    // each one a class identity even when they're effectively stubs.
    const _navUsb = (() => {
        const _UProto = globalThis.USB && globalThis.USB.prototype;
        const u = _UProto ? Object.create(_UProto) : {};
        u.getDevices = ({ getDevices() { return Promise.resolve([]); } }).getDevices;
        _maskFunction(u.getDevices, 'getDevices');
        u.requestDevice = ({ requestDevice() { return Promise.reject(new DOMException("User denied", "NotFoundError")); } }).requestDevice;
        _maskFunction(u.requestDevice, 'requestDevice');
        u.onconnect = null;
        u.ondisconnect = null;
        return u;
    })();
    const _navSerial = (() => {
        const _SProto = globalThis.Serial && globalThis.Serial.prototype;
        const s = _SProto ? Object.create(_SProto) : {};
        s.getPorts = ({ getPorts() { return Promise.resolve([]); } }).getPorts;
        _maskFunction(s.getPorts, 'getPorts');
        s.requestPort = ({ requestPort() { return Promise.reject(new DOMException("User denied", "NotFoundError")); } }).requestPort;
        _maskFunction(s.requestPort, 'requestPort');
        s.onconnect = null;
        s.ondisconnect = null;
        return s;
    })();
    const _navHid = (() => {
        const _HProto = globalThis.HID && globalThis.HID.prototype;
        const h = _HProto ? Object.create(_HProto) : {};
        h.getDevices = ({ getDevices() { return Promise.resolve([]); } }).getDevices;
        _maskFunction(h.getDevices, 'getDevices');
        h.requestDevice = ({ requestDevice() { return Promise.reject(new DOMException("User denied", "NotFoundError")); } }).requestDevice;
        _maskFunction(h.requestDevice, 'requestDevice');
        h.onconnect = null;
        h.ondisconnect = null;
        return h;
    })();
    const _navLocks = (() => {
        const _LProto = globalThis.LockManager && globalThis.LockManager.prototype;
        const l = _LProto ? Object.create(_LProto) : {};
        l.query = ({ query() { return Promise.resolve({ held: [], pending: [] }); } }).query;
        _maskFunction(l.query, 'query');
        l.request = ({ request() { return new Promise(() => {}); } }).request;
        _maskFunction(l.request, 'request');
        return l;
    })();

    Object.defineProperty(_navUsb, Symbol.toStringTag, { value: "USB", configurable: true });
    Object.defineProperty(_navSerial, Symbol.toStringTag, { value: "Serial", configurable: true });
    Object.defineProperty(_navHid, Symbol.toStringTag, { value: "HID", configurable: true });
    Object.defineProperty(_navLocks, Symbol.toStringTag, { value: "LockManager", configurable: true });

    // navigator.keyboard — Keyboard API (probed by CreepJS + DataDome).
    // Real Chrome exposes a Keyboard instance with getLayoutMap() returning a
    // KeyboardLayoutMap: a Map<string, string> of physical key code → character.
    // An empty {} or missing getLayoutMap is an immediate lie signal.
    const _qwertyLayout = new Map([
        ['Backquote', '`'], ['Digit1', '1'], ['Digit2', '2'], ['Digit3', '3'],
        ['Digit4', '4'], ['Digit5', '5'], ['Digit6', '6'], ['Digit7', '7'],
        ['Digit8', '8'], ['Digit9', '9'], ['Digit0', '0'], ['Minus', '-'],
        ['Equal', '='],
        ['KeyQ', 'q'], ['KeyW', 'w'], ['KeyE', 'e'], ['KeyR', 'r'], ['KeyT', 't'],
        ['KeyY', 'y'], ['KeyU', 'u'], ['KeyI', 'i'], ['KeyO', 'o'], ['KeyP', 'p'],
        ['BracketLeft', '['], ['BracketRight', ']'], ['Backslash', '\\'],
        ['KeyA', 'a'], ['KeyS', 's'], ['KeyD', 'd'], ['KeyF', 'f'], ['KeyG', 'g'],
        ['KeyH', 'h'], ['KeyJ', 'j'], ['KeyK', 'k'], ['KeyL', 'l'],
        ['Semicolon', ';'], ['Quote', "'"],
        ['KeyZ', 'z'], ['KeyX', 'x'], ['KeyC', 'c'], ['KeyV', 'v'], ['KeyB', 'b'],
        ['KeyN', 'n'], ['KeyM', 'm'],
        ['Comma', ','], ['Period', '.'], ['Slash', '/'],
        ['Space', ' '],
        ['F1', 'F1'], ['F2', 'F2'], ['F3', 'F3'], ['F4', 'F4'],
        ['F5', 'F5'], ['F6', 'F6'], ['F7', 'F7'], ['F8', 'F8'],
        ['F9', 'F9'], ['F10', 'F10'], ['F11', 'F11'], ['F12', 'F12'],
        ['Numpad0', '0'], ['Numpad1', '1'], ['Numpad2', '2'], ['Numpad3', '3'],
        ['Numpad4', '4'], ['Numpad5', '5'], ['Numpad6', '6'], ['Numpad7', '7'],
        ['Numpad8', '8'], ['Numpad9', '9'],
        ['NumpadAdd', '+'], ['NumpadSubtract', '-'], ['NumpadMultiply', '*'],
        ['NumpadDivide', '/'], ['NumpadDecimal', '.'],
    ]);

    class KeyboardLayoutMap {
        constructor(map) { this._m = map; }
        get size() { return this._m.size; }
        get(key) { return this._m.get(key); }
        has(key) { return this._m.has(key); }
        entries() { return this._m.entries(); }
        keys() { return this._m.keys(); }
        values() { return this._m.values(); }
        forEach(cb, thisArg) { return this._m.forEach(cb, thisArg); }
        [Symbol.iterator]() {
            const it = this._m[Symbol.iterator]();
            return {
                next() { return it.next(); },
                [Symbol.iterator]() { return this; }
            };
        }
    }
    Object.defineProperty(KeyboardLayoutMap.prototype, Symbol.toStringTag, {
        value: 'KeyboardLayoutMap', configurable: true,
    });
    globalThis.KeyboardLayoutMap = KeyboardLayoutMap;

    class Keyboard extends EventTarget {
        getLayoutMap() {
            return Promise.resolve(new KeyboardLayoutMap(_qwertyLayout));
        }
        lock(keyCodes) { return Promise.resolve(); }
        unlock() {}
    }
    Object.defineProperty(Keyboard.prototype, Symbol.toStringTag, {
        value: 'Keyboard', configurable: true,
    });
    globalThis.Keyboard = Keyboard;
    const _navKeyboard = new Keyboard();

    class StorageManager {}
    Object.defineProperty(StorageManager.prototype, Symbol.toStringTag, {
        value: "StorageManager", configurable: true,
    });
    StorageManager.prototype.estimate = ({
        estimate() {
            // Real Chrome on modern macOS/Windows desktops reports ~60% of
            // free disk as quota. ~120 GB is a typical-disk plausible value
            // — the previous 1 GB constant is a hard fingerprint tell because
            // Chrome quota is always many tens of GB. Usage breakdown matches
            // Chrome's documented `usageDetails` shape so iteration probes
            // (e.g. `for (k in details)`) see the same key set.
            return Promise.resolve({
                quota: 128849018880,                 // ~120 GB
                usage: 0,
                usageDetails: { indexedDB: 0, caches: 0, serviceWorkerRegistrations: 0 },
            });
        }
    }).estimate;
    _maskFunction(StorageManager.prototype.estimate, 'estimate');

    StorageManager.prototype.persist = ({ persist() { return Promise.resolve(false); } }).persist;
    _maskFunction(StorageManager.prototype.persist, 'persist');

    StorageManager.prototype.persisted = ({ persisted() { return Promise.resolve(false); } }).persisted;
    _maskFunction(StorageManager.prototype.persisted, 'persisted');

    globalThis.StorageManager = StorageManager;
    const _navStorage = Object.create(StorageManager.prototype);

    class ServiceWorkerContainer extends EventTarget {
        constructor() {
            super();
            this.controller = null;
            this.oncontrollerchange = null;
            this.onmessage = null;
            this.ready = Promise.resolve({
                active: null, installing: null, waiting: null, scope: "/",
                unregister() { return Promise.resolve(true); },
            });
        }
    }
    Object.defineProperty(ServiceWorkerContainer.prototype, Symbol.toStringTag, {
        value: "ServiceWorkerContainer", configurable: true,
    });
    ServiceWorkerContainer.prototype.register = ({
        register(scriptURL, options) {
            return Promise.resolve({
                scope: (options && options.scope) || "/",
                active: { scriptURL, state: "activated" },
                installing: null,
                waiting: null,
                updateViaCache: "imports",
                update() { return Promise.resolve(this); },
                unregister() { return Promise.resolve(true); },
                addEventListener() {},
                removeEventListener() {},
            });
        }
    }).register;
    _maskFunction(ServiceWorkerContainer.prototype.register, 'register');

    ServiceWorkerContainer.prototype.getRegistrations = ({ getRegistrations() { return Promise.resolve([]); } }).getRegistrations;
    _maskFunction(ServiceWorkerContainer.prototype.getRegistrations, 'getRegistrations');

    ServiceWorkerContainer.prototype.getRegistration = ({ getRegistration() { return Promise.resolve(undefined); } }).getRegistration;
    _maskFunction(ServiceWorkerContainer.prototype.getRegistration, 'getRegistration');

    ServiceWorkerContainer.prototype.startMessages = ({ startMessages() {} }).startMessages;
    _maskFunction(ServiceWorkerContainer.prototype.startMessages, 'startMessages');

    ServiceWorkerContainer.prototype.addEventListener = ({ addEventListener() {} }).addEventListener;
    _maskFunction(ServiceWorkerContainer.prototype.addEventListener, 'addEventListener');

    ServiceWorkerContainer.prototype.removeEventListener = ({ removeEventListener() {} }).removeEventListener;
    _maskFunction(ServiceWorkerContainer.prototype.removeEventListener, 'removeEventListener');

    globalThis.ServiceWorkerContainer = ServiceWorkerContainer;
    const _navServiceWorker = new ServiceWorkerContainer();
    const _navClipboard = (() => {
        const _CProto = globalThis.Clipboard && globalThis.Clipboard.prototype;
        const c = _CProto ? Object.create(_CProto) : {};
        c.readText = ({ readText() { return Promise.resolve(""); } }).readText;
        _maskFunction(c.readText, 'readText');

        c.writeText = ({ writeText() { return Promise.resolve(); } }).writeText;
        _maskFunction(c.writeText, 'writeText');

        return c;
    })();
    Object.defineProperty(_navClipboard, Symbol.toStringTag, { value: "Clipboard", configurable: true });
    const _navGeolocation = (() => {
        const _GProto = globalThis.Geolocation && globalThis.Geolocation.prototype;
        const g = _GProto ? Object.create(_GProto) : {};
        g.getCurrentPosition = ({
            getCurrentPosition(ok, err, options) {
                if (typeof err === "function") setTimeout(() => err({ code: 1, message: "User denied Geolocation" }), 0);
            }
        }).getCurrentPosition;
        _maskFunction(g.getCurrentPosition, 'getCurrentPosition');

        g.watchPosition = ({
            watchPosition(ok, err, options) {
                if (typeof err === "function") setTimeout(() => err({ code: 1, message: "User denied Geolocation" }), 0);
                return 0;
            }
        }).watchPosition;
        _maskFunction(g.watchPosition, 'watchPosition');

        g.clearWatch = ({ clearWatch() {} }).clearWatch;
        _maskFunction(g.clearWatch, 'clearWatch');

        return g;
    })();
    Object.defineProperty(_navGeolocation, Symbol.toStringTag, { value: "Geolocation", configurable: true });
    const _navWakeLock = {};
    Object.defineProperty(_navWakeLock, Symbol.toStringTag, { value: "WakeLock", configurable: true });
    const _navMediaSession = {};
    const _navScheduling = (() => {
        const _SProto = globalThis.Scheduling && globalThis.Scheduling.prototype;
        const s = _SProto ? Object.create(_SProto) : {};
        s.isInputPending = function isInputPending() { return false; };
        return s;
    })();

    Object.defineProperty(_navScheduling, Symbol.toStringTag, { value: "Scheduling", configurable: true });
    const _navUserActivation = (() => {
        const _UAProto = globalThis.UserActivation && globalThis.UserActivation.prototype;
        const u = _UAProto ? Object.create(_UAProto) : {};
        Object.defineProperties(u, {
            isActive: { get: () => false, enumerable: true },
            hasBeenActive: { get: () => false, enumerable: true },
        });
        return u;
    })();
    Object.defineProperty(_navUserActivation, Symbol.toStringTag, { value: "UserActivation", configurable: true });
    // navigator.languages is CACHED per runtime — Chrome returns the same
    // frozen array reference on every access, so we memoize after the first
    // lazy read (bootstrap time has no profile; the cache must be deferred).
    // Assertions tested elsewhere: Object.isFrozen === true, identity stable.
    let _navLanguagesCache = null;
    const _getNavLanguages = () => {
        if (_navLanguagesCache === null) {
            _navLanguagesCache = Object.freeze(_pJson("languages", ["en-US", "en"]));
        }
        return _navLanguagesCache;
    };

    // Scalar getters — read from stealth profile each call (idempotent).
    _defNav('userAgent', () => _p("user_agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36"));
    _defNav('platform', () => _p("platform", "Win32"));
    _defNav('vendor', () => "Google Inc.");
    _defNav('vendorSub', () => "");
    _defNav('productSub', () => "20030107");
    _defNav('appVersion', () => _p("user_agent", "").replace("Mozilla/", ""));
    _defNav('appCodeName', () => "Mozilla");
    _defNav('appName', () => "Netscape");
    _defNav('product', () => "Gecko");
    _defNav('language', () => _p("language", "ru-RU"));
    _defNav('languages', _getNavLanguages);
    _defNav('onLine', () => true);
    _defNav('cookieEnabled', () => true);
    _defNav('hardwareConcurrency', () => _pInt("hardware_concurrency", 8));
    // navigator.deviceMemory is [SecureContext] — undefined on
    // data:/http:/about:blank. Phase 7. Skip entirely on iOS (real
    // Safari has no NavigatorDeviceMemory interface).
    if (!_isMobileIOS()) {
        _defNav('deviceMemory', () => _secure() ? _pInt("device_memory", 8) : undefined);
    }
    _defNav('maxTouchPoints', () => _pInt("max_touch_points", 0));
    _defNav('pdfViewerEnabled', () => true);
    // webdriver: present on Navigator.prototype per W3C WebDriver spec.
    // Modern Chrome (>=89, incl. the Chrome-148 we impersonate) ALWAYS
    // defines navigator.webdriver: it returns `false` for normal
    // browsing (`undefined` is the old/headless tell). Confirmed by the
    // K2-DIFF decoded Kasada sensor (wdt.r="undefined" was flagged
    // anomalous) and consistent with worker_bootstrap.js (already
    // `false`). The prior "returns undefined" was a wrong assumption.
    // Kasada wdd probe checks the getter source — _maskFunction native.
    Object.defineProperty(Navigator.prototype, 'webdriver', {
        get: _maskFunction(function() { return false; }, 'get webdriver'),
        enumerable: true,
        configurable: true
    });
    _defNav('doNotTrack', () => null);
    _defNav('msDoNotTrack', () => undefined);
    _defNav('loadPurpose', () => undefined);
    _defNav('sayswho', () => undefined);

    // Object getters — stable references.
    // navigator.connection (NetworkInformation API) — Chrome-only. Real
    // Safari has no `connection` on Navigator. Skip on iOS.
    if (!_isMobileIOS()) {
        Object.defineProperty(_NavProto, 'connection', { get: () => _navConnection, enumerable: false, configurable: true });
    }
    Object.defineProperty(_NavProto, 'plugins', { get: () => _navPlugins, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'mimeTypes', { get: () => _navMimeTypes, enumerable: false, configurable: true });
    // Navigator getters. Properties marked /* SC */ are
    // [SecureContext]-only per their IDL — return undefined on
    // insecure contexts so the surface matches real Chrome on
    // data:/http:/about:blank URLs. Phase 7 fix.
    Object.defineProperty(_NavProto, 'mediaDevices', { get: () => _secure() ? _navMediaDevices : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'permissions', { get: () => _navPermissions, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'credentials', { get: () => _secure() ? _navCredentials : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'bluetooth', { get: () => _secure() ? _navBluetooth : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'usb', { get: () => _secure() ? _navUsb : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'serial', { get: () => _secure() ? _navSerial : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'hid', { get: () => _secure() ? _navHid : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'keyboard', { get: () => _secure() ? _navKeyboard : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'locks', { get: () => _secure() ? _navLocks : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'storage', { get: () => _secure() ? _navStorage : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'serviceWorker', { get: () => _secure() ? _navServiceWorker : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'clipboard', { get: () => _secure() ? _navClipboard : undefined, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'geolocation', { get: () => _navGeolocation, enumerable: false, configurable: true });
    Object.defineProperty(_NavProto, 'wakeLock', { get: () => _secure() ? _navWakeLock : undefined, enumerable: false, configurable: true });

    // Apply native masking to all getters
    _maskAsNative(_NavProto, 'userAgent', 'platform', 'vendor', 'vendorSub', 'productSub', 
        'appVersion', 'appCodeName', 'appName', 'product', 'language', 'languages', 
        'onLine', 'cookieEnabled', 'hardwareConcurrency', 'deviceMemory', 'maxTouchPoints', 
        'pdfViewerEnabled', 'webdriver', 'connection', 'plugins', 'mimeTypes', 
        'mediaDevices', 'permissions', 'credentials', 'bluetooth', 'usb', 'serial', 
        'hid', 'keyboard', 'locks', 'storage', 'serviceWorker', 'clipboard', 
        'geolocation', 'wakeLock');
    _defNav('mediaSession', () => _navMediaSession);
    // navigator.scheduling.isInputPending — Chrome-only. Real Safari has
    // no Scheduling interface. navigator.userActivation — Chrome 88+ only.
    if (!_isMobileIOS()) {
        _defNav('scheduling', () => _navScheduling);
        _defNav('userActivation', () => _navUserActivation);
    }

    // Prototype methods.
    _defNavMethod('javaEnabled', function javaEnabled() { return false; });
    // Real sendBeacon: fires a fetch with keepalive=true so the server
    // actually receives the payload. A no-op stub silently drops data that
    // challenge engines (Kasada, etc.) send on solve completion, blocking
    // the session from being upgraded.
    _defNavMethod('sendBeacon', function sendBeacon(url, data) {
        try {
            let absUrl = String(url);
            if (!/^https?:/i.test(absUrl)) {
                absUrl = new URL(absUrl, globalThis.location && globalThis.location.href || 'about:blank').href;
            }
            let init = { method: 'POST', keepalive: true, credentials: 'include' };
            if (data != null) {
                if (typeof data === 'string') {
                    init.body = data;
                    init.headers = { 'content-type': 'text/plain;charset=UTF-8' };
                } else if (data instanceof Blob) {
                    init.body = data;
                    if (data.type) init.headers = { 'content-type': data.type };
                } else if (data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
                    init.body = data;
                    init.headers = { 'content-type': 'application/octet-stream' };
                } else if (typeof FormData !== 'undefined' && data instanceof FormData) {
                    init.body = data;
                } else if (typeof URLSearchParams !== 'undefined' && data instanceof URLSearchParams) {
                    init.body = String(data);
                    init.headers = { 'content-type': 'application/x-www-form-urlencoded;charset=UTF-8' };
                } else {
                    init.body = String(data);
                    init.headers = { 'content-type': 'text/plain;charset=UTF-8' };
                }
            }
            // Fire and forget — sendBeacon is non-blocking by spec.
            Promise.resolve().then(() => fetch(absUrl, init).catch(() => {}));
            return true;
        } catch (_) {
            return false;
        }
    });
    // BatteryManager — must be a real class extending EventTarget so
    // `Object.getPrototypeOf(b).constructor.name === "BatteryManager"`
    // and `b instanceof EventTarget` both hold.
    class BatteryManager extends EventTarget {
        constructor() {
            super();
        }
    }
    Object.defineProperty(BatteryManager.prototype, Symbol.toStringTag, {
        value: "BatteryManager", configurable: true,
    });

    // Akamai's for..in probe (bt field) requires these to be enumerable.
    // Standard WebIDL members are non-enumerable, but Akamai's custom
    // sensor traversal expects to see these values. Moving them to 
    // the prototype as enumerable getters satisfies both:
    // 1. Instance has 0 own properties (parity).
    // 2. for..in on instance still finds them (Akamai parity).
    const _defBatGetter = (name, val) => {
        const getter = function() { return val; };
        Object.defineProperty(BatteryManager.prototype, name, {
            get: getter,
            enumerable: true,
            configurable: true,
        });
        _maskFunction(getter, `get ${name}`);
    };
    const _defBatProp = (name) => {
        let _val = null;
        const getter = function() { return _val; };
        const setter = function(v) { _val = v; };
        Object.defineProperty(BatteryManager.prototype, name, {
            get: getter,
            set: setter,
            enumerable: true,
            configurable: true,
        });
        _maskFunction(getter, `get ${name}`);
        _maskFunction(setter, `set ${name}`);
    };

    // Per-session randomized values. The canonical CreepJS tell is
    // `{level:1, charging:true, chargingTime:0, dischargingTime:Infinity}`
    // — every headless browser ships that exact combination because the
    // default constants are intuitive. Real Chrome on a laptop varies:
    // charging is false ~70% of typical sessions (battery-powered), level
    // is uniform-ish in [0.20, 0.95], and chargingTime/dischargingTime are
    // finite when on battery. We seed once at module init so reads are
    // stable within the page (real BatteryManager doesn't tick by the
    // second either — events fire on state change).
    const _batCharging = Math.random() < 0.3; // ~30% plugged in
    const _batLevel = (() => {
        const v = 0.20 + Math.random() * 0.75;
        // Round to 2 decimal places — real Chrome rounds level to 0.01 since
        // a 2021 privacy reduction (https://crbug.com/661792).
        return Math.round(v * 100) / 100;
    })();
    const _batChargingTime = _batCharging
        ? Math.round(1800 + Math.random() * 7200) // 30 min – 2.5 h to full
        : Infinity;
    const _batDischargingTime = _batCharging
        ? Infinity
        : Math.round(3600 + Math.random() * 21600); // 1 – 7 h remaining

    _defBatGetter('charging', _batCharging);
    _defBatGetter('chargingTime', _batChargingTime);
    _defBatGetter('dischargingTime', _batDischargingTime);
    _defBatGetter('level', _batLevel);
    _defBatProp('onchargingchange');
    _defBatProp('onchargingtimechange');
    _defBatProp('ondischargingtimechange');
    _defBatProp('onlevelchange');

    globalThis.BatteryManager = BatteryManager;
    const _batteryInstance = new BatteryManager();
    // getBattery is [SecureContext] — exists only on https/wss/file/
    // localhost. On data:/http:, real Chrome reports
    // `TypeError: navigator.getBattery is not a function`. Phase 7.
    if (_secure()) {
        _defNavMethod('getBattery', function getBattery() {
            return Promise.resolve(_batteryInstance);
        });
    }
    _defNavMethod('getUserMedia', function getUserMedia(constraints, success, error) {
        if (error) error(new Error("Permission denied"));
        return undefined;
    });
    _defNavMethod('webkitGetUserMedia', function webkitGetUserMedia(c, s, e) { if (e) e(new Error("Permission denied")); });
    _defNavMethod('mozGetUserMedia', function mozGetUserMedia(c, s, e) { if (e) e(new Error("Permission denied")); });
    // Additional Navigator.prototype methods that Akamai BMP v3 walks via
    // computed names (per the research agent). Missing any of these causes
    // `nqz[computed_name](...) is not a function` errors in the sensor VM.
    _defNavMethod('vibrate', function vibrate(pattern) { return false; });
    _defNavMethod('getGamepads', function getGamepads() { return [null, null, null, null]; });
    _defNavMethod('registerProtocolHandler', function registerProtocolHandler(scheme, url) {});
    _defNavMethod('unregisterProtocolHandler', function unregisterProtocolHandler(scheme, url) {});
    _defNavMethod('requestMediaKeySystemAccess', function requestMediaKeySystemAccess(keySystem, configs) {
        // org.w3.clearkey is required by the W3C EME spec on all platforms.
        // com.widevine.alpha is available on Windows and macOS (not Linux desktop).
        // com.microsoft.playready is Windows-only.
        // Returning NotSupportedError unconditionally is a bot signal probed by
        // Kasada and Akamai BMP.
        const _ks = String(keySystem);
        const _os = _p("os_name", "Windows");
        const _isWin = _os === "Windows";
        const _isMac = _os === "macOS";

        const _supported = (
            _ks === 'org.w3.clearkey' ||
            (_ks === 'com.widevine.alpha' && (_isWin || _isMac)) ||
            (_ks === 'com.microsoft.playready' && _isWin)
        );

        if (!_supported) {
            return Promise.reject(new DOMException(
                "Failed to execute 'requestMediaKeySystemAccess' on 'Navigator': " +
                "Requested configuration is not supported.",
                "NotSupportedError"
            ));
        }

        // Build a minimal MediaKeySystemAccess object.
        // Real Chrome exposes: keySystem (string), getConfiguration() (object),
        // createMediaKeys() (Promise<MediaKeys>).
        const _access = {
            keySystem: _ks,
            getConfiguration: function getConfiguration() {
                return configs && configs.length ? Object.assign({}, configs[0]) : {};
            },
            createMediaKeys: function createMediaKeys() {
                // Return a minimal MediaKeys stub; sufficient for capability probes.
                const _mk = {
                    createSession: function createSession() {
                        return {
                            sessionId: '', expiration: NaN, closed: Promise.resolve(),
                            keyStatuses: new Map(),
                            addEventListener: function() {}, removeEventListener: function() {},
                            generateRequest: function() { return Promise.resolve(); },
                            load: function() { return Promise.resolve(false); },
                            update: function() { return Promise.resolve(); },
                            close: function() { return Promise.resolve(); },
                            remove: function() { return Promise.resolve(); },
                        };
                    },
                    setServerCertificate: function() { return Promise.resolve(false); },
                };
                return Promise.resolve(_mk);
            },
        };
        return Promise.resolve(_access);
    });
    _defNavMethod('canShare', function canShare(data) { return false; });
    _defNavMethod('share', function share(data) {
        return Promise.reject(new DOMException("Permission denied", "NotAllowedError"));
    });
    _defNavMethod('clearAppBadge', function clearAppBadge() { return Promise.resolve(); });
    _defNavMethod('setAppBadge', function setAppBadge(count) { return Promise.resolve(); });

    // Symbol.toStringTag — Akamai BMP v3 checks Object.prototype.toString.call(navigator)
    // and expects "[object Navigator]". Without this, it returns "[object Object]".
    Object.defineProperty(_NavProto, Symbol.toStringTag, {
        value: "Navigator", configurable: true,
    });

    // Instantiate — zero own properties.
    const _navigator = globalThis.navigator || Object.create(_NavProto);
    Object.setPrototypeOf(_navigator, _NavProto);
    globalThis.navigator = _navigator;

    // location — Proxy-based, tracks URL components and navigation requests
    const _locationData = {
        href: "about:blank",
        protocol: "https:",
        host: "",
        hostname: "",
        port: "",
        pathname: "/",
        search: "",
        hash: "",
        origin: "null",
    };

    function _parseLocationUrl(url) {
        const s = String(url);
        // Special-scheme URLs (about:, data:, javascript:, blob:, mailto:,
        // tel:, chrome:, chrome-extension:, view-source:) are absolute — they
        // replace the current location entirely, they don't join against the
        // base. Our embedded URL constructor was joining `about:blank`
        // against an http(s) base and producing `https://host/about:blank`
        // (a path with a colon), which the navigate loop then tried to fetch.
        // Detect special schemes and short-circuit the join. Pinned by the
        // iphey.com regression where JS sets `location.href = 'about:blank'`
        // during a fingerprint check and broke same-page rendering.
        if (/^(about|data|javascript|blob|mailto|tel|chrome|chrome-extension|view-source):/i.test(s)) {
            _locationData.href = s;
            // Clear sub-fields so a downstream consumer doesn't see stale
            // pieces of the previous URL.
            _locationData.protocol = (s.split(':', 1)[0] || '') + ':';
            _locationData.host = '';
            _locationData.hostname = '';
            _locationData.port = '';
            _locationData.pathname = '';
            _locationData.search = '';
            _locationData.hash = '';
            _locationData.origin = 'null';
            return;
        }
        try {
            const u = new URL(s, _locationData.href !== "about:blank" ? _locationData.href : undefined);
            _locationData.href = u.href;
            _locationData.protocol = u.protocol;
            _locationData.host = u.host;
            _locationData.hostname = u.hostname;
            _locationData.port = u.port;
            _locationData.pathname = u.pathname;
            _locationData.search = u.search;
            _locationData.hash = u.hash;
            _locationData.origin = u.origin;
        } catch {
            _locationData.href = s;
        }
    }

    // Pending-navigation signal — generic primitive used by the Rust driver
    // to loop through challenge flows. Any location.reload/assign/replace or
    // location.href = ... sets this; <meta http-equiv="refresh"> does too.
    // Matches the behavior of a real browser's navigation algorithm without
    // any per-engine awareness.

    // Location class and instance
    const _LocProto = globalThis.Location.prototype;
    Object.defineProperty(_LocProto, Symbol.toStringTag, { value: "Location", enumerable: false, configurable: true });

    function _defLoc(prop, getter, setter) {
        _defProtoGetter(_LocProto, prop, getter, setter);
    }

    // Signal the Rust event loop that a navigation is pending. Without this,
    // `run_until_idle(30s)` runs to its full ceiling before the retry GET
    // fires — too late for Kasada's 5-second tolerance window. With it,
    // run_until_idle returns within ~150ms (just enough microtask tail to
    // let in-flight fetch().then(setCookie) land in the jar). See
    // crates/js_runtime/src/extensions/nav_ext.rs.
    const _signalNav = () => { try { ops.op_set_pending_nav(); } catch (_) {} };

    // Mirror `_browser_oxide.__pendingNavigation` onto `globalThis.__pendingNavigation`
    // — JS-side consumers (and the navigation_primitives tests) read it
    // off globalThis per the documented contract at the top of this
    // section. We keep _browser_oxide as the underlying store so the existing
    // Rust event-loop driver and per-call assignments keep working.
    Object.defineProperty(globalThis, '__pendingNavigation', {
        get: () => _browser_oxide.__pendingNavigation,
        set: (v) => { _browser_oxide.__pendingNavigation = v; },
        configurable: true,
        enumerable: false,
    });

    _defLoc('href', () => _locationData.href, (v) => {
        _parseLocationUrl(v);
        _browser_oxide.__pendingNavigation = { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('origin', () => _locationData.origin);
    _defLoc('protocol', () => _locationData.protocol, (v) => {
        _parseLocationUrl(v + "//" + _locationData.host + _locationData.pathname);
        _browser_oxide.__pendingNavigation = 
 { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('host', () => _locationData.host, (v) => {
        _parseLocationUrl(_locationData.protocol + "//" + v + _locationData.pathname);
        _browser_oxide.__pendingNavigation = 
 { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('hostname', () => _locationData.hostname, (v) => {
        _parseLocationUrl(_locationData.protocol + "//" + v + (_locationData.port ? ":" + _locationData.port : "") + _locationData.pathname);
        _browser_oxide.__pendingNavigation = 
 { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('port', () => _locationData.port);
    _defLoc('pathname', () => _locationData.pathname);
    _defLoc('search', () => _locationData.search);
    _defLoc('hash', () => _locationData.hash, (v) => {
        _locationData.hash = String(v).startsWith('#') ? v : '#' + v;
        _locationData.href = _locationData.origin + _locationData.pathname + _locationData.search + _locationData.hash;
    });
    _defLoc('ancestorOrigins', () => {
        const _ao = { length: 0, item: () => null, contains: () => false };
        _ao[Symbol.iterator] = function*() {};
        return _ao;
    });

    _defProtoMethod(_LocProto, 'assign', (url) => {
        _parseLocationUrl(url);
        _browser_oxide.__pendingNavigation = 
 { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defProtoMethod(_LocProto, 'replace', (url) => {
        _parseLocationUrl(url);
        _browser_oxide.__pendingNavigation = 
 { url: _locationData.href, kind: "replace" };
        _signalNav();
    });
    _defProtoMethod(_LocProto, 'reload', () => {
        _browser_oxide.__pendingNavigation = 
 { url: _locationData.href, kind: "reload" };
        _signalNav();
    });
    _defProtoMethod(_LocProto, 'toString', function() { return this.href; });
    _LocProto[Symbol.toPrimitive] = function() { return this.href; };

    _maskAsNative(_LocProto, 'assign', 'replace', 'reload', 'toString');

    const _locationInstance = Object.create(_LocProto);
    try {
        // Delete Deno's location getter if it exists
        delete globalThis.location;
    } catch(e) {}
    try {
        Object.defineProperty(globalThis, 'location', {
            value: _locationInstance,
            writable: true,
            enumerable: true,
            configurable: true
        });
    } catch(e) {
        globalThis.location = _locationInstance;
    }

    // Frame-tree globals: top/parent/frames/self all point to this window
    // window / self / frames / top / parent — self-references.
    for (const key of ['window', 'self', 'frames', 'top', 'parent']) {
        Object.defineProperty(globalThis, key, {
            value: globalThis,
            writable: false,
            configurable: false,
            enumerable: true
        });
    }
    globalThis.opener = null;
    // window.length = number of child frames. Starts at 0; dom_bootstrap.js
    // updates it when iframes are appended to the document.
    try { Object.defineProperty(globalThis, 'length', { value: 0, configurable: true, writable: true }); } catch (_) {}

    // screen — prototype-backed so own-descriptor probe returns undefined.
    const _ScreenProto = Screen.prototype;
    const _screenOrientation = { type: "landscape-primary", angle: 0, onchange: null };
    // ScreenOrientation is its own interface in Chrome, so expose it too.
    const _ScreenOrientationProto = ScreenOrientation.prototype;

    _defProtoGetter(_ScreenProto, 'width', () => _pInt("screen_width", 1920));
    _defProtoGetter(_ScreenProto, 'height', () => _pInt("screen_height", 1080));
    _defProtoGetter(_ScreenProto, 'availWidth', () => _pInt("screen_avail_width", 1920));
    _defProtoGetter(_ScreenProto, 'availHeight', () => _pInt("screen_avail_height", 1040));
    _defProtoGetter(_ScreenProto, 'availLeft', () => 0);
    _defProtoGetter(_ScreenProto, 'availTop', () => _pInt("screen_avail_top", 0));
    _defProtoGetter(_ScreenProto, 'colorDepth', () => _pInt("screen_color_depth", 24));
    _defProtoGetter(_ScreenProto, 'pixelDepth', () => _pInt("screen_color_depth", 24));
    _defProtoGetter(_ScreenProto, 'orientation', () => _screenOrientation);
    _defProtoGetter(_ScreenProto, 'isExtended', () => false);
    Object.defineProperty(_ScreenProto, Symbol.toStringTag, { value: "Screen", configurable: true });
    Object.defineProperty(ScreenOrientation.prototype, Symbol.toStringTag, { value: "ScreenOrientation", configurable: true });
    const _screenInstance = Object.create(_ScreenProto);
    Object.defineProperty(globalThis, 'screen', {
        get: function() { return _screenInstance; },
        enumerable: true,
        configurable: true
    });

    // Misc globals anti-bot checks for
    // isSecureContext: per-URL, computed from scheme on the Rust side
    // (https/wss/file or http://localhost). Drives the ~18
    // secure-context-only Web Platform APIs at IDL `[SecureContext]`.
    Object.defineProperty(globalThis, 'isSecureContext', {
        get: () => ops.op_is_secure_context(),
        configurable: true,
        enumerable: true,
    });
    // crossOriginIsolated must reflect actual COOP+COEP state from the
    // response headers — see crates/net/src/headers.rs and gap #30.
    // Backed by an op so it's true iff the runtime was constructed with
    // BrowserRuntimeOptions { cross_origin_isolated: true, .. }.
    Object.defineProperty(globalThis, 'crossOriginIsolated', {
        get: () => ops.op_cross_origin_isolated(),
        configurable: true,
        enumerable: true,
    });
    Object.defineProperty(globalThis, 'origin', {
        get() { return globalThis.location ? globalThis.location.origin : "null"; },
        configurable: true, enumerable: true,
    });
    // Window metrics must resolve LAZILY — bootstrap runs at V8-snapshot
    // build time with no profile installed; eager values get baked as
    // defaults and never update when the profile loads.
    Object.defineProperty(globalThis, 'innerWidth',  { get: () => _pInt("inner_width", 1920),   configurable: true, enumerable: true });
    Object.defineProperty(globalThis, 'innerHeight', { get: () => _pInt("inner_height", 1080),  configurable: true, enumerable: true });
    Object.defineProperty(globalThis, 'outerWidth',  { get: () => _pInt("outer_width", 1920),   configurable: true, enumerable: true });
    Object.defineProperty(globalThis, 'outerHeight', { get: () => _pInt("outer_height", 1080),  configurable: true, enumerable: true });
    Object.defineProperty(globalThis, 'devicePixelRatio', { get: _maskFunction(function() { return _pFloat("device_pixel_ratio", 2.0); }, 'get devicePixelRatio'), configurable: true, enumerable: true });

    // Scroll + screen position — OWN accessor properties on the window
    // instance (globalThis), per real Chrome (verified via Playwright
    // MCP capture):
    //
    //   Object.getOwnPropertyDescriptor(window, 'scrollX')
    //     → { get: f, set: f, enumerable: true, configurable: true }
    //   Object.getOwnPropertyDescriptor(Window.prototype, 'scrollX')
    //     → undefined (NOT on the prototype)
    //
    // The Phase 6 D2 attempt put them on Window.prototype based on a
    // wrong reading of the spec; Phase 7 reverts that.
    //
    // Backing storage is module-scope state, mutated by scrollTo()/
    // scrollBy(). screenX/Y are always 0 on a windowless engine.
    let _scrollX = 0;
    let _scrollY = 0;

    Object.defineProperty(globalThis, 'scrollX', {
        get: function() { return _scrollX; },
        set: function(_v) { /* read-only per spec; setter exists in descriptor */ },
        enumerable: true, configurable: true,
    });
    Object.defineProperty(globalThis, 'scrollY', {
        get: function() { return _scrollY; },
        set: function(_v) {},
        enumerable: true, configurable: true,
    });
    Object.defineProperty(globalThis, 'pageXOffset', {
        get: function() { return _scrollX; },
        set: function(_v) {},
        enumerable: true, configurable: true,
    });
    Object.defineProperty(globalThis, 'pageYOffset', {
        get: function() { return _scrollY; },
        set: function(_v) {},
        enumerable: true, configurable: true,
    });
    Object.defineProperty(globalThis, 'screenX', {
        get: function() { return 0; },
        set: function(_v) {},
        enumerable: true, configurable: true,
    });
    Object.defineProperty(globalThis, 'screenY', {
        get: function() { return 0; },
        set: function(_v) {},
        enumerable: true, configurable: true,
    });
    Object.defineProperty(globalThis, 'screenLeft', {
        get: function() { return 0; },
        set: function(_v) {},
        enumerable: true, configurable: true,
    });
    Object.defineProperty(globalThis, 'screenTop', {
        get: function() { return 0; },
        set: function(_v) {},
        enumerable: true, configurable: true,
    });

    globalThis.scrollTo = ({
        scrollTo(xOrOptions, y) {
            if (typeof xOrOptions === "object" && xOrOptions !== null) {
                _scrollX = xOrOptions.left || 0;
                _scrollY = xOrOptions.top || 0;
            } else {
                _scrollX = xOrOptions || 0;
                _scrollY = y || 0;
            }
        }
    }).scrollTo;
    _maskFunction(globalThis.scrollTo, 'scrollTo');

    globalThis.scroll = globalThis.scrollTo;

    globalThis.scrollBy = ({
        scrollBy(xOrOptions, y) {
            if (typeof xOrOptions === "object" && xOrOptions !== null) {
                globalThis.scrollTo(_scrollX + (xOrOptions.left || 0), _scrollY + (xOrOptions.top || 0));
            } else {
                globalThis.scrollTo(_scrollX + (xOrOptions || 0), _scrollY + (y || 0));
            }
        }
    }).scrollBy;
    _maskFunction(globalThis.scrollBy, 'scrollBy');

    // window.chrome (CRITICAL — every antibot system checks this object).
    //
    // Real Chrome: `window.chrome` is a special non-configurable plain
    // object; it is NOT wrapped in a named class like Navigator/Screen.
    // Its sub-namespaces (chrome.app, chrome.runtime, chrome.csi,
    // chrome.loadTimes) are also plain objects, but `chrome.loadTimes`
    // and `chrome.csi` are actual native functions whose .toString()
    // returns the native-code shape.
    //
    // Probes to defend:
    // 1. typeof chrome === 'object'
    // 2. chrome.loadTimes.toString() contains '[native code]'
    // 3. chrome.csi.toString() contains '[native code]'
    // 4. chrome.runtime.onMessage / onConnect presence for extension contexts
    // 5. Object.getOwnPropertyNames(chrome).length > 0 in real Chrome,
    //    so leaving chrome as a plain object here is CORRECT — we don't
    //    want zero own properties like navigator.
    const _chromeCsi = ({
        csi() {
            return { startE: Date.now(), onloadT: Date.now(), pageT: Date.now(), tran: 15 };
        }
    }).csi;
    _maskFunction(_chromeCsi, 'csi');

    const _chromeLoadTimes = ({
        loadTimes() {
            // For HTTP/2 pages these are true/"h2"; for about:blank/non-HTTP they are false/"".
            const _isHttp = globalThis.location && /^https?:/.test(globalThis.location.protocol);
            return {
                commitLoadTime: Date.now()/1000,
                connectionInfo: _isHttp ? "h2" : "",
                finishDocumentLoadTime: Date.now()/1000,
                finishLoadTime: Date.now()/1000,
                firstPaintAfterLoadTime: 0,
                firstPaintTime: Date.now()/1000,
                navigationType: "Other",
                npnNegotiatedProtocol: _isHttp ? "h2" : "",
                requestTime: Date.now()/1000,
                startLoadTime: Date.now()/1000,
                wasAlternateProtocolAvailable: _isHttp,
                wasFetchedViaSpdy: _isHttp,
                wasNpnNegotiated: _isHttp,
            };
        }
    }).loadTimes;
    _maskFunction(_chromeLoadTimes, 'loadTimes');

    // Real Chrome 147 on a regular page (no extensions): {app, csi, loadTimes}
    // chrome.runtime is ONLY present in extension contexts — absent on regular pages.
    // chrome.webstore was removed in Chrome 126.
    // Adding either is a classic bot detection signal (Kasada, Cloudflare, DataDome).
    // iOS Safari MUST NOT have `window.chrome` — PerimeterX `('chrome' in window)`
    // check (research 05_PERIMETERX.md §6.3) flags it instantly.
    if (!_isMobileIOS()) {
        globalThis.chrome = {
            app: {
                isInstalled: false,
                InstallState: {DISABLED:"disabled",INSTALLED:"installed",NOT_INSTALLED:"not_installed"},
                RunningState: {CANNOT_RUN:"cannot_run",READY_TO_RUN:"ready_to_run",RUNNING:"running"},
                // Chrome 147 exposes these functions on chrome.app (bot detectors check them):
                getDetails: function getDetails() { return null; },
                getIsInstalled: function getIsInstalled() { return false; },
                installState: function installState(cb) { if (typeof cb === 'function') setTimeout(() => cb('not_installed'), 0); },
                runningState: function runningState() { return 'cannot_run'; },
            },
            csi: _chromeCsi,
            loadTimes: _chromeLoadTimes,
        };
    }

    // --- Document visibility/hidden stubs ---
    Object.defineProperty(Document.prototype, 'visibilityState', { get() { return 'visible'; }, enumerable: false, configurable: true });
    Object.defineProperty(Document.prototype, 'hidden', { get() { return false; }, enumerable: false, configurable: true });
    Object.defineProperty(Document.prototype, 'webkitVisibilityState', { get() { return 'visible'; }, enumerable: false, configurable: true });
    Object.defineProperty(Document.prototype, 'webkitHidden', { get() { return false; }, enumerable: false, configurable: true });

    if (globalThis.navigator) {
        // Match Chrome 148's exact descriptor for webdriver:
        //   { get: ƒ, set: undefined, enumerable: true, configurable: true }
        // BotD detector #16 (and Castle) verifies the enumerable bit
        // specifically — the older `enumerable: false`
        // here was a divergence. Real Chrome's webdriver getter is
        // owned-on-prototype and IS enumerable (visible to for..in on
        // Navigator.prototype).
        // webdriver: defined identically to the Navigator.prototype block
        // above — `false` (Chrome-148-faithful; K2-DIFF wdt fix).
        Object.defineProperty(_NavProto, 'webdriver', {
            get: _maskFunction(function() { return false; }, 'get webdriver'),
            enumerable: true,
            configurable: true
        });

        // navigator.plugins / mimeTypes are defined at the top of this file
        // (search for _allPlugins). Count is driven by profile.plugins_count
        // and profile.mime_types_count. Do not override here.
    }

    // NOTE: real Chrome has NO `navigator.devicePixelRatio` — verified
    // CDP-free (`'devicePixelRatio' in navigator` === false,
    // getOwnPropertyDescriptor(navigator,'devicePixelRatio') === undefined).
    // devicePixelRatio is a Window-only property. The previous
    // `_defNav('devicePixelRatio', …)` added a property no real browser
    // exposes — exactly the object Kasada's `dpi` probe reads
    // (`getOwnPropertyDescriptor(navigator,'devicePixelRatio')`, 60/60
    // receiver=Navigator per kasada_dpi_receiver.rs). Removed for parity.

    if (globalThis.Screen) {
        const _ScreenProto = Screen.prototype;
        _defProtoGetter(_ScreenProto, 'availLeft', () => 0);
        _defProtoGetter(_ScreenProto, 'availTop', () => _pInt("screen_avail_top", 0));
        _defProtoGetter(_ScreenProto, 'colorDepth', () => _pInt("screen_color_depth", 24));
        _defProtoGetter(_ScreenProto, 'pixelDepth', () => _pInt("screen_color_depth", 24));
    }

    // Explicitly define documentMode as undefined to pass 'prop in document' checks quietly
    Object.defineProperty(Document.prototype, 'documentMode', { value: undefined, enumerable: false, configurable: true });

    const _hunt = (obj, name) => {
        return obj;
    };
    globalThis.navigator = _hunt(globalThis.navigator, 'navigator');
    globalThis.document = _hunt(globalThis.document, 'document');
    // Only re-bind chrome where it was actually installed — assigning
    // `undefined` would still create a `chrome` own property and trip
    // PerimeterX's `('chrome' in window)` check on iOS.
    if (!_isMobileIOS()) {
        globalThis.chrome = _hunt(globalThis.chrome, 'chrome');
    }
    globalThis.performance = _hunt(globalThis.performance, 'performance');

    // navigator.userAgentData (Client Hints API)
    //
    // Every hint reads from the StealthProfile at call-time so HTTP
    // Sec-CH-UA-* headers and the JS surface never diverge (a classic
    // FingerprintJS / CreepJS / Yandex Antirobot scoring axis). Eager
    // reads at bootstrap time would capture defaults because the V8
    // snapshot is built with no profile installed.
    //
    // Chrome exposes low-entropy fields synchronously (brands, mobile,
    // platform). High-entropy values go through getHighEntropyValues()
    // which returns a Promise and rejects on invalid descriptor shape.
    (function setupUaData() {
        // Chrome userAgentData.platform enum: "Windows" | "macOS" | "Linux"
        // | "Android" | "Chrome OS" | "Chromium OS" | "Fuchsia" | "iOS" | "".
        // Our os_name already uses these exact strings (with macOS not Mac OS X).
        const _uaPlatform = () => _p("os_name", "Windows");
        const _uaBrowserMajor = () => _p("browser_version", "130.0.6723.91").split(".")[0];
        const _uaBrowserFull = () => _p("browser_version", "130.0.6723.91");
        const _uaArch = () => _p("cpu_architecture", "x86");
        const _uaBitness = () => _p("cpu_bitness", "64");
        const _uaPlatformVersion = () => {
            // Chrome on Linux reports empty; profile already honors this.
            const v = _p("platform_version", "");
            if (v) return v;
            if (_uaPlatform() === "Linux") return "";
            // Fallback: zero-pad os_version to a triple when platform_version
            // not set (keeps legacy profiles working).
            const ver = _p("os_version", "");
            const parts = ver.split(".");
            if (parts.length >= 3) return ver;
            if (parts.length === 2) return parts[0] + "." + parts[1] + ".0";
            if (parts.length === 1 && parts[0]) return parts[0] + ".0.0";
            return "";
        };
        const _uaModel = () => _p("ua_model", "");
        const _uaWow64 = () => _p("ua_wow64", "false") === "true";

        // GREASE: Chrome's brand array lists Chromium / Google Chrome /
        // Not-A.Brand in RANDOM order per startup. Real Chrome draws from
        // a cryptographic RNG; Date.now() shuffling was pseudo-random
        // enough to be detected in some probes. Use crypto.getRandomValues
        // when available, fall back to Math.random.
        const _secureRand = (n) => {
            try {
                const a = new Uint32Array(1);
                crypto.getRandomValues(a);
                return a[0] % n;
            } catch (_) {
                return Math.floor(Math.random() * n);
            }
        };
        const _shuffled = (arr) => {
            const copy = arr.slice();
            for (let i = copy.length - 1; i > 0; i--) {
                const j = _secureRand(i + 1);
                [copy[i], copy[j]] = [copy[j], copy[i]];
            }
            return copy;
        };

        // Phase 7 — real Chrome 147 GREASE entry is
        // `{brand: "Not.A/Brand", version: "8"}`, not "24".
        // Chrome rotates the GREASE version periodically.
        const _makeLowBrands = () => Object.freeze(_shuffled([
            Object.freeze({ brand: "Chromium", version: _uaBrowserMajor() }),
            Object.freeze({ brand: "Google Chrome", version: _uaBrowserMajor() }),
            Object.freeze({ brand: "Not.A/Brand", version: "8" }),
        ]).map(Object.freeze));
        const _makeFullBrands = () => Object.freeze(_shuffled([
            Object.freeze({ brand: "Chromium", version: _uaBrowserFull() }),
            Object.freeze({ brand: "Google Chrome", version: _uaBrowserFull() }),
            Object.freeze({ brand: "Not.A/Brand", version: "8.0.0.0" }),
        ]).map(Object.freeze));
        // Chrome re-uses the same GREASE ordering across a userAgentData
        // object's lifetime; only randomized once per construction.
        let _lowBrands = null, _fullBrands = null;
        const _lowCached = () => (_lowBrands ||= _makeLowBrands());
        const _fullCached = () => (_fullBrands ||= _makeFullBrands());

        const _allowedHints = new Set([
            "architecture", "bitness", "brands", "formFactors", "fullVersionList",
            "mobile", "model", "platform", "platformVersion",
            "uaFullVersion", "wow64",
        ]);

        const _navUaData = (() => {
            const _UAProto = globalThis.NavigatorUAData && globalThis.NavigatorUAData.prototype;
            const u = _UAProto ? Object.create(_UAProto) : {};
            Object.defineProperties(u, {
                brands: { get: () => _lowCached(), enumerable: true },
                mobile: { get: () => false, enumerable: true },
                platform: { get: () => _uaPlatform(), enumerable: true },
            });
            u.getHighEntropyValues = ({
                getHighEntropyValues(hints) {
                    // Chrome rejects with TypeError on non-array (or missing).
                    if (!Array.isArray(hints)) {
                        return Promise.reject(new TypeError(
                            "Failed to execute 'getHighEntropyValues' on 'NavigatorUAData': " +
                            "The provided value cannot be converted to a sequence."
                        ));
                    }
                    const result = {
                        brands: _lowCached(),
                        mobile: false,
                        platform: _uaPlatform(),
                    };
                    for (const key of hints) {
                        if (typeof key !== "string") continue;
                        switch (key) {
                            case "architecture":      result.architecture = _uaArch(); break;
                            case "bitness":           result.bitness = _uaBitness(); break;
                            case "brands":            result.brands = _lowCached(); break;
                            case "formFactors":       result.formFactors = ["Desktop"]; break;
                            case "fullVersionList":   result.fullVersionList = _fullCached(); break;
                            case "mobile":            result.mobile = false; break;
                            case "model":             result.model = _uaModel(); break;
                            case "platform":          result.platform = _uaPlatform(); break;
                            case "platformVersion":   result.platformVersion = _uaPlatformVersion(); break;
                            case "uaFullVersion":     result.uaFullVersion = _uaBrowserFull(); break;
                            case "wow64":             result.wow64 = _uaWow64(); break;
                        }
                    }
                    return Promise.resolve(result);
                }
            }).getHighEntropyValues;
            _maskFunction(u.getHighEntropyValues, 'getHighEntropyValues');

            u.toJSON = ({
                toJSON() {
                    return {
                        brands: _lowCached().map(b => ({ brand: b.brand, version: b.version })),
                        mobile: false,
                        platform: _uaPlatform(),
                    };
                }
            }).toJSON;
            _maskFunction(u.toJSON, 'toJSON');
            return u;
        })();
        // userAgentData is [SecureContext] — return undefined on
        // insecure contexts so probes get a TypeError when reading
        // navigator.userAgentData.brands. Phase 7.
        _defNav('userAgentData', () => _secure() ? _navUaData : undefined);
    })();

    // Notification
    // globalThis.Notification = class Notification { static permission = "default"; };

    // Worker / SharedWorker / ServiceWorker classes. Our runtime has a
    // crates/workers module but doesn't auto-expose the constructor to JS.
    // Several fingerprint probes check `typeof Worker === 'function'` as a
    // presence test, and CreepJS spawns a Worker to cross-check navigator.
    // This is a minimal stub that lets fingerprint probes pass their
    // Real Worker — spawns an OS thread with its own V8 isolate, drives
    // a poll loop that delivers parent←worker messages to onmessage.
    if (!globalThis.Worker) {
        const _wops = Deno.core.ops;

        function _resolveWorkerScript(url) {
            const s = String(url);
            if (s.startsWith('blob:')) {
                try { return _wops.op_blob_fetch_text(s) || ''; } catch (e) { return ''; }
            }
            if (s.startsWith('http:') || s.startsWith('https:')) {
                try { return _wops.op_worker_sync_fetch(s) || ''; } catch (e) { return ''; }
            }
            // Fallback to relative URL resolution via base
            if (!s.includes(':')) {
                const base = (globalThis.__browser_oxide && globalThis.__browser_oxide._baseUrl) || '';
                if (base.startsWith('http')) {
                    const full = base.replace(/\/[^\/]*$/, '/') + s;
                    try { return _wops.op_worker_sync_fetch(full) || ''; } catch (e) { return ''; }
                }
            }
            return '';
        }

        globalThis.Worker = class Worker {
            constructor(scriptURL, options) {
                this._url = String(scriptURL);
                this._options = options || {};
                this._name = (options && options.name) || '';
                // `type: 'module'` enables ES module semantics for the
                // worker body (import.meta.url, async module eval,
                // top-level await). Default is 'classic'.
                this._type = (options && options.type) || 'classic';
                const isModule = this._type === 'module';
                this.onmessage = null;
                this.onerror = null;
                this.onmessageerror = null;
                this._listeners = { message: [], messageerror: [], error: [] };

                const script = _resolveWorkerScript(this._url);
                if (!script) {
                    // Script resolution failed; defer to next tick and fire error.
                    this._id = 0;
                    const self = this;
                    Promise.resolve().then(() => {
                        self._fireEvent('error', {
                            type: 'error',
                            message: 'Worker script could not be resolved: ' + self._url,
                            filename: self._url,
                            lineno: 0,
                            colno: 0,
                        });
                    });
                    return;
                }

                this._id = _wops.op_worker_spawn(script, this._name, isModule);
                if (this._id <= 0) {
                    this._id = 0;
                    return;
                }

                // W5b-deep fix (commit pending): replace the prior
                // setInterval(5) polling with an async-await chain
                // backed by op_worker_await_message. The old impl
                // pinned the V8 event loop's `is_pending=true` for the
                // lifetime of every Worker, blocking SPA hydration
                // completion detection (twitter, x.com, etc.). The new
                // pump suspends on a tokio::sync::Notify so the loop
                // is only marked pending while there's an actual
                // pending message — same correctness, no perpetual
                // pinning.
                const self = this;
                const _drainOnce = () => {
                    if (!self._id) return;
                    _wops.op_worker_await_message(self._id).then((raw) => {
                        if (!raw || !self._id) return; // worker died
                        const deserializer =
                            _browser_oxide && _browser_oxide.deserializeFromWire;
                        let payload = null;
                        try { payload = JSON.parse(raw); }
                        catch (e) { return _drainOnce(); }
                        const data = deserializer
                            ? deserializer(payload && payload.data)
                            : payload && payload.data;
                        const event = {
                            type: 'message',
                            data,
                            origin: '',
                            lastEventId: '',
                            source: null,
                            ports: [],
                            timeStamp: Date.now(),
                        };
                        try { self._fireEvent('message', event); }
                        catch (_) {}
                        _drainOnce(); // chain next await
                    }).catch(() => {});
                };
                _drainOnce();
            }

            _fireEvent(type, event) {
                const arr = this._listeners[type];
                if (arr) {
                    for (const fn of arr.slice()) {
                        try { fn.call(this, event); } catch (e) {}
                    }
                }
                const on = this['on' + type];
                if (typeof on === 'function') {
                    try { on.call(this, event); } catch (e) {}
                }
            }

            postMessage(message, transfer) {
                if (!this._id) return;
                // Transferables: accepted as an array. Each entry (an
                // ArrayBuffer or view) is reachable from the message
                // and will be serialized with it. Real browsers
                // detach the source after transfer — V8 detachment
                // isn't exposed here, so the source stays readable.
                // For fingerprint-shape probes this is acceptable.
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
                // Wire-serialize so ArrayBuffer/TypedArray/Map/Set/
                // Date/RegExp survive the JSON hop to the worker.
                let wire;
                try {
                    wire =
                        (_browser_oxide &&
                            _browser_oxide.serializeForWire &&
                            _browser_oxide.serializeForWire(message)) ||
                        message;
                } catch (e) {
                    // DataCloneError (e.g. function inside message).
                    // Propagate to the caller so they see the same
                    // error Chrome would throw.
                    throw e;
                }
                let payload;
                try {
                    payload = JSON.stringify({ data: wire });
                } catch (_e) {
                    payload = JSON.stringify({ data: null });
                }
                _wops.op_worker_post_to_worker(this._id, payload);
            }

            terminate() {
                if (this._id) {
                    try { _wops.op_worker_terminate(this._id); } catch (e) {}
                    this._id = 0;
                }
                if (this._pollTimer) {
                    clearInterval(this._pollTimer);
                    this._pollTimer = null;
                }
            }

            addEventListener(type, listener) {
                if (!this._listeners[type]) this._listeners[type] = [];
                this._listeners[type].push(listener);
            }
            removeEventListener(type, listener) {
                const arr = this._listeners[type];
                if (!arr) return;
                const i = arr.indexOf(listener);
                if (i >= 0) arr.splice(i, 1);
            }
            dispatchEvent(event) {
                this._fireEvent(event && event.type, event);
                return true;
            }
        };
        Object.defineProperty(globalThis.Worker.prototype, Symbol.toStringTag, {
            value: 'Worker',
            configurable: true,
        });
    }
    if (!globalThis.SharedWorker) {
        globalThis.SharedWorker = class SharedWorker {
            constructor(scriptURL, options) {
                this.port = {
                    onmessage: null,
                    postMessage() {},
                    start() {},
                    close() {},
                    addEventListener() {},
                    removeEventListener() {},
                };
                this.onerror = null;
                this._url = String(scriptURL);
            }
            addEventListener() {}
            removeEventListener() {}
            dispatchEvent() { return true; }
        };
    }
    if (!globalThis.ServiceWorker) {
        globalThis.ServiceWorker = class ServiceWorker extends EventTarget {
            constructor() {
                super();
                this.scriptURL = "";
                this.state = "activated";
                this.onstatechange = null;
            }
            postMessage() {}
        };
    }
    if (!globalThis.WorkerGlobalScope) {
        // WorkerGlobalScope is the class that Worker's `self` is an instance
        // of. fpCollect checks `typeof WorkerGlobalScope === 'function'`.
        globalThis.WorkerGlobalScope = class WorkerGlobalScope {};
    }
    if (!globalThis.DedicatedWorkerGlobalScope) {
        globalThis.DedicatedWorkerGlobalScope = class DedicatedWorkerGlobalScope extends globalThis.WorkerGlobalScope {};
    }

    // ================================================================
    // Batch 2: additional Web API stubs for fingerprint coverage
    // Chrome 131 exposes these as globals; fingerprint probes do
    // `typeof X === 'function'` checks against them.
    //
    // Bisect on 2026-04-10 confirmed these don't regress Akamai BMP sites
    // — homedepot's L3/L2 flip was Akamai's stochastic trust-profile
    // scoring, not our code.
    // ================================================================

    if (!globalThis.FileReader) {
        globalThis.FileReader = class FileReader extends EventTarget {
            static EMPTY = 0;
            static LOADING = 1;
            static DONE = 2;
            constructor() {
                super();
                this.readyState = 0;
                this.result = null;
                this.error = null;
                this.onload = null;
                this.onloadstart = null;
                this.onloadend = null;
                this.onprogress = null;
                this.onerror = null;
                this.onabort = null;
            }
            readAsText(blob) { this.readyState = 2; this.result = ""; if (this.onload) setTimeout(() => this.onload({ target: this }), 0); }
            readAsDataURL(blob) { this.readyState = 2; this.result = "data:application/octet-stream;base64,"; if (this.onload) setTimeout(() => this.onload({ target: this }), 0); }
            readAsArrayBuffer(blob) { this.readyState = 2; this.result = new ArrayBuffer(0); if (this.onload) setTimeout(() => this.onload({ target: this }), 0); }
            readAsBinaryString(blob) { this.readyState = 2; this.result = ""; if (this.onload) setTimeout(() => this.onload({ target: this }), 0); }
            abort() { this.readyState = 2; }
        };
    }

    if (!globalThis.ImageBitmap) {
        globalThis.ImageBitmap = class ImageBitmap {
            constructor() { this.width = 0; this.height = 0; }
            close() {}
        };
    }
    if (!globalThis.createImageBitmap) {
        globalThis.createImageBitmap = ({ createImageBitmap() { return Promise.resolve(new globalThis.ImageBitmap()); } }).createImageBitmap;
        _maskFunction(globalThis.createImageBitmap, 'createImageBitmap');
    }

    if (!globalThis.DOMPoint) {
        globalThis.DOMPoint = class DOMPoint {
            constructor(x, y, z, w) {
                this.x = x || 0;
                this.y = y || 0;
                this.z = z || 0;
                this.w = w === undefined ? 1 : w;
            }
            matrixTransform() { return new DOMPoint(this.x, this.y, this.z, this.w); }
            toJSON() { return { x: this.x, y: this.y, z: this.z, w: this.w }; }
            static fromPoint(p) { return new DOMPoint(p?.x, p?.y, p?.z, p?.w); }
        };
        globalThis.DOMPointReadOnly = globalThis.DOMPoint;
    }

    if (!globalThis.DOMMatrix) {
        globalThis.DOMMatrix = class DOMMatrix {
            constructor(init) {
                this.a = 1; this.b = 0; this.c = 0; this.d = 1; this.e = 0; this.f = 0;
                this.m11 = 1; this.m12 = 0; this.m13 = 0; this.m14 = 0;
                this.m21 = 0; this.m22 = 1; this.m23 = 0; this.m24 = 0;
                this.m31 = 0; this.m32 = 0; this.m33 = 1; this.m34 = 0;
                this.m41 = 0; this.m42 = 0; this.m43 = 0; this.m44 = 1;
                this.is2D = true;
                this.isIdentity = true;
            }
            multiply() { return new DOMMatrix(); }
            translate() { return new DOMMatrix(); }
            scale() { return new DOMMatrix(); }
            rotate() { return new DOMMatrix(); }
            inverse() { return new DOMMatrix(); }
            transformPoint(p) { return new DOMPoint(p?.x, p?.y, p?.z, p?.w); }
            toString() { return "matrix(1, 0, 0, 1, 0, 0)"; }
            toFloat32Array() { return new Float32Array(16); }
            toFloat64Array() { return new Float64Array(16); }
            static fromMatrix(m) { return new DOMMatrix(); }
            static fromFloat32Array() { return new DOMMatrix(); }
            static fromFloat64Array() { return new DOMMatrix(); }
        };
        globalThis.DOMMatrixReadOnly = globalThis.DOMMatrix;
        globalThis.WebKitCSSMatrix = globalThis.DOMMatrix;
    }

    // Path2D DELIBERATELY NOT STUBBED. Our JS-class stub creates non-native
    // method descriptors which Akamai BMP detects via `Object
    // .getOwnPropertyDescriptor(Path2D.prototype, 'addPath')` — a class
    // method is a data descriptor, real Chrome's is a native accessor.
    // Homedepot's Akamai config regresses from L3 → L2 interstitial when
    // a fake Path2D is present. Better to report `typeof Path2D ===
    // 'undefined'` than to lie unconvincingly. Verified via bisect
    // 2026-04-10 by toggling this single addition on/off.
    // PerformanceObserver / PerformanceEntry / ReportingObserver
    if (!globalThis.PerformanceObserver) {
        globalThis.PerformanceObserver = class PerformanceObserver {
            constructor(cb) { this._cb = cb; }
            observe() {}
            disconnect() {}
            takeRecords() { return []; }
        };
        // Real Chrome exposes supportedEntryTypes as a static GETTER, not a
        // data property. Fingerprinters do Object.getOwnPropertyDescriptor
        // and a data descriptor is distinctive.
        Object.defineProperty(globalThis.PerformanceObserver, "supportedEntryTypes", {
            get() {
                return ["element", "event", "first-input", "largest-contentful-paint",
                        "layout-shift", "longtask", "mark", "measure", "navigation",
                        "paint", "resource", "visibility-state"];
            },
            configurable: true,
            enumerable: true,
        });
    }
    if (!globalThis.PerformanceEntry) {
        globalThis.PerformanceEntry = class PerformanceEntry {
            constructor() { this.name = ""; this.entryType = ""; this.startTime = 0; this.duration = 0; }
            toJSON() { return { name: this.name, entryType: this.entryType, startTime: this.startTime, duration: this.duration }; }
        };
    }

    if (!globalThis.ReportingObserver) {
        globalThis.ReportingObserver = class ReportingObserver {
            constructor() {}
            observe() {}
            disconnect() {}
            takeRecords() { return []; }
        };
    }

    // Streams, channels, EventSource, compression — second batch-2 block
    if (!globalThis.ReadableStream) {
        globalThis.ReadableStream = class ReadableStream {
            constructor() { this.locked = false; }
            getReader() { return { read: () => Promise.resolve({ done: true, value: undefined }), releaseLock() {}, closed: Promise.resolve(), cancel: () => Promise.resolve() }; }
            pipeTo() { return Promise.resolve(); }
            pipeThrough(t) { return t.readable; }
            tee() { return [new ReadableStream(), new ReadableStream()]; }
            cancel() { return Promise.resolve(); }
        };
    }
    if (!globalThis.WritableStream) {
        globalThis.WritableStream = class WritableStream {
            constructor() { this.locked = false; }
            getWriter() { return { write: () => Promise.resolve(), close: () => Promise.resolve(), abort: () => Promise.resolve(), releaseLock() {}, closed: Promise.resolve(), ready: Promise.resolve() }; }
            close() { return Promise.resolve(); }
            abort() { return Promise.resolve(); }
        };
    }
    if (!globalThis.TransformStream) {
        globalThis.TransformStream = class TransformStream {
            constructor() {
                this.readable = new globalThis.ReadableStream();
                this.writable = new globalThis.WritableStream();
            }
        };
    }
    if (!globalThis.ReadableStreamDefaultReader) globalThis.ReadableStreamDefaultReader = class ReadableStreamDefaultReader {};
    if (!globalThis.WritableStreamDefaultWriter) globalThis.WritableStreamDefaultWriter = class WritableStreamDefaultWriter {};

    if (!globalThis.BroadcastChannel) {
        globalThis.BroadcastChannel = class BroadcastChannel extends EventTarget {
            constructor(name) { super(); this.name = name; this.onmessage = null; this.onmessageerror = null; }
            postMessage() {}
            close() {}
        };
    }

    if (!globalThis.MessagePort) {
        globalThis.MessagePort = class MessagePort extends EventTarget {
            constructor() { super(); this.onmessage = null; this.onmessageerror = null; }
            postMessage() {}
            start() {}
            close() {}
        };
    }

    if (!globalThis.MessageChannel) {
        globalThis.MessageChannel = class MessageChannel {
            constructor() {
                this.port1 = new MessagePort();
                this.port2 = new MessagePort();
            }
        };
    }

    if (!globalThis.EventSource) {
        globalThis.EventSource = class EventSource extends EventTarget {
            static CONNECTING = 0;
            static OPEN = 1;
            static CLOSED = 2;
            constructor(url) {
                super();
                this.url = String(url);
                this.readyState = 0;
                this.withCredentials = false;
                this.onopen = null;
                this.onmessage = null;
                this.onerror = null;
            }
            close() { this.readyState = 2; }
        };
    }

    // CompressionStream / DecompressionStream (Chrome 80+)
    if (!globalThis.CompressionStream) {
        globalThis.CompressionStream = class CompressionStream {
            constructor() {
                this.readable = new globalThis.ReadableStream();
                this.writable = new globalThis.WritableStream();
            }
        };
    }
    if (!globalThis.DecompressionStream) {
        globalThis.DecompressionStream = class DecompressionStream {
            constructor() {
                this.readable = new globalThis.ReadableStream();
                this.writable = new globalThis.WritableStream();
            }
        };
    }
    // end batch 2

    // speechSynthesis — prototype-backed; bot tests check getVoices().length > 0
    class SpeechSynthesis {}
    globalThis.SpeechSynthesis = SpeechSynthesis;
    const _SSProto = SpeechSynthesis.prototype;
    const _ssVoices = [
        {name:"Google US English",lang:"en-US",localService:false,default:true,voiceURI:"Google US English"},
        {name:"Google UK English Female",lang:"en-GB",localService:false,default:false,voiceURI:"Google UK English Female"},
        {name:"Google UK English Male",lang:"en-GB",localService:false,default:false,voiceURI:"Google UK English Male"},
    ];
    _defProtoGetter(_SSProto, 'pending', () => false);
    _defProtoGetter(_SSProto, 'speaking', () => false);
    _defProtoGetter(_SSProto, 'paused', () => false);
    _defProtoGetter(_SSProto, 'onvoiceschanged', () => null);
    _defProtoMethod(_SSProto, 'getVoices', function getVoices() { return _ssVoices.slice(); });
    _defProtoMethod(_SSProto, 'speak', function speak() {});
    _defProtoMethod(_SSProto, 'cancel', function cancel() {});
    _defProtoMethod(_SSProto, 'pause', function pause() {});
    _defProtoMethod(_SSProto, 'resume', function resume() {});
    Object.defineProperty(_SSProto, Symbol.toStringTag, { value: "SpeechSynthesis", configurable: true });
    globalThis.speechSynthesis = Object.create(_SSProto);

    // Performance stub state — installed on Performance.prototype below.
    const _perfMemory = {
        jsHeapSizeLimit: 4294705152,
        totalJSHeapSize: 10000000,
        usedJSHeapSize: 8000000,
    };

    // =========================================================
    // P1 FIX: Intl timezone consistency (§P1 item 13)
    // Yandex Antirobot and other scorers cross-check the IANA timezone
    // reported by `Intl.DateTimeFormat().resolvedOptions().timeZone` against
    // the IP geolocation and the `timezone` header hint. V8 uses the process
    // TZ env var (whatever the machine says), so a Moscow profile run from a
    // US datacenter would report `America/New_York`, which is an instant tell.
    //
    // Monkey-patch: override the default timeZone option on Intl.DateTimeFormat
    // so resolvedOptions() returns the profile timezone. Also patch
    // Date.prototype.getTimezoneOffset to return the profile's UTC offset.
    // =========================================================
    // IMPORTANT: the original gate `if (op_has_stealth_profile())` fired at
    // V8-snapshot-build time — returning false and skipping the patch. The
    // snapshot then froze stock V8 Intl, so no timezone override ever took
    // effect when a profile loaded. Install patches unconditionally; each
    // call reads the live profile via _p/_pInt, falling back to stock V8
    // (system timezone) when no profile is installed.
    if (globalThis.Intl) {
        const _profileTz = () => _p("timezone", "");
        const _profileLocale = () => _p("language", "");
        const _OrigDTF = globalThis.Intl.DateTimeFormat;

        const _patchIntl = (klass) => {
            if (!globalThis.Intl[klass]) return;
            const _Orig = globalThis.Intl[klass];
            const Patched = function(...args) {
                let locales = args[0];
                let options = args[1] || {};
                const pLoc = _profileLocale();
                const pTz = _profileTz();
                if (!locales && pLoc) locales = pLoc;
                if (klass === 'DateTimeFormat' && !options.timeZone && pTz) {
                    options = Object.assign({}, options, { timeZone: pTz });
                }
                return new _Orig(locales, options);
            };
            Patched.prototype = _Orig.prototype;
            if (_Orig.supportedLocalesOf) Patched.supportedLocalesOf = _Orig.supportedLocalesOf.bind(_Orig);
            Object.defineProperty(globalThis.Intl, klass, { value: Patched, writable: true, configurable: true });
        };

        for (const k of ['DateTimeFormat', 'NumberFormat', 'Collator', 'PluralRules', 'RelativeTimeFormat']) {
            _patchIntl(k);
        }

        // Deep prototype override: resolvedOptions() must return the
        // profile's claim regardless of how the instance was constructed.
        for (const klass of ['DateTimeFormat', 'NumberFormat', 'Collator', 'PluralRules', 'RelativeTimeFormat']) {
            if (!globalThis.Intl[klass]) continue;
            const proto = globalThis.Intl[klass].prototype;
            const origResolved = proto.resolvedOptions;
            proto.resolvedOptions = function() {
                const res = origResolved.call(this);
                const pTz = _profileTz();
                const pLoc = _profileLocale();
                if (pTz) res.timeZone = pTz;
                if (pLoc) res.locale = pLoc;
                return res;
            };
        }

        // Date.prototype.getTimezoneOffset — compute the offset from the
        // profile timezone at each call so DST transitions stay accurate.
        const _origGetTimezoneOffset = Date.prototype.getTimezoneOffset;
        Date.prototype.getTimezoneOffset = function () {
            const profileTz = _profileTz();
            if (!profileTz) return _origGetTimezoneOffset.call(this);
            try {
                const fmt = new _OrigDTF("en-US", {
                    timeZone: profileTz,
                    year: "numeric", month: "2-digit", day: "2-digit",
                    hour: "2-digit", minute: "2-digit", second: "2-digit",
                    hour12: false,
                });
                const parts = fmt.formatToParts(this);
                const get = (t) => parts.find((p) => p.type === t)?.value;
                const tzDate = new Date(Date.UTC(
                    parseInt(get("year"), 10),
                    parseInt(get("month"), 10) - 1,
                    parseInt(get("day"), 10),
                    parseInt(get("hour"), 10),
                    parseInt(get("minute"), 10),
                    parseInt(get("second"), 10),
                ));
                return Math.round((this.getTime() - tzDate.getTime()) / 60000);
            } catch (_e) {
                return _origGetTimezoneOffset.call(this);
            }
        };
        Object.defineProperty(Date.prototype.getTimezoneOffset, "toString", {
            value: () => "function getTimezoneOffset() { [native code] }",
            configurable: true,
        });
        Object.defineProperty(Date.prototype.getTimezoneOffset, _nativeTag, { value: 'getTimezoneOffset', configurable: true });

        // =========================================================
        // Date.prototype toString patches — print the profile's
        // timezone, not UTC. Detection libraries probe
        // `new Date().toString()` because it's the cheapest TZ probe
        // available; UTC output on a macOS profile is a hard tell.
        //
        // Real Chrome on macOS / America/Los_Angeles produces:
        //   "Tue Apr 29 2026 13:02:46 GMT-0700 (Pacific Daylight Time)"
        //
        // We patch four methods consistently:
        //   - toString()        full date+time+tz
        //   - toDateString()    date portion only
        //   - toTimeString()    time + tz portion only
        //   - toLocaleString()  default-locale form (when called w/o args)
        // =========================================================

        // Map IANA name → English long-form ("Pacific Daylight Time", etc.)
        // Chrome derives this from the longGeneric+timeZoneName fields that
        // Intl.DateTimeFormat exposes. Compute via the cached _OrigDTF.
        const _tzLongName = (date, tz) => {
            try {
                const fmt = new _OrigDTF("en-US", {
                    timeZone: tz, timeZoneName: "long",
                });
                const parts = fmt.formatToParts(date);
                const tzPart = parts.find(p => p.type === "timeZoneName");
                return tzPart ? tzPart.value : "";
            } catch (_e) { return ""; }
        };

        // Compute "GMT-0700" style offset string from the profile timezone
        // for a specific moment (DST-aware via our patched getTimezoneOffset).
        const _gmtOffsetString = (date) => {
            const offMin = date.getTimezoneOffset();
            const sign = offMin <= 0 ? "+" : "-";
            const abs = Math.abs(offMin);
            const hh = String(Math.floor(abs / 60)).padStart(2, "0");
            const mm = String(abs % 60).padStart(2, "0");
            return `GMT${sign}${hh}${mm}`;
        };

        // Pull profile-localized date+time parts so we can format the
        // "Tue Apr 29 2026 13:02:46" portion the way real Chrome does.
        // Returns { weekday, month, day, year, hh, mm, ss } strings.
        const _tzParts = (date, tz) => {
            try {
                const fmt = new _OrigDTF("en-US", {
                    timeZone: tz, weekday: "short", month: "short",
                    day: "2-digit", year: "numeric",
                    hour: "2-digit", minute: "2-digit", second: "2-digit",
                    hour12: false,
                });
                const parts = fmt.formatToParts(date);
                const get = (t) => parts.find(p => p.type === t)?.value || "";
                const hour = get("hour");
                // Chrome uses 24-hour; Intl may produce "24" for midnight in
                // some locales — normalize to "00".
                return {
                    weekday: get("weekday"),
                    month: get("month"),
                    day: get("day"),
                    year: get("year"),
                    hour: hour === "24" ? "00" : hour,
                    minute: get("minute"),
                    second: get("second"),
                };
            } catch (_e) { return null; }
        };

        const _origDateToString = Date.prototype.toString;
        Date.prototype.toString = function toString() {
            // `Date.prototype.toString.call(non-Date)` must throw TypeError —
            // mirror real Chrome by delegating to the original.
            if (!(this instanceof Date) || Number.isNaN(this.getTime?.())) {
                return _origDateToString.call(this);
            }
            const tz = _profileTz();
            if (!tz) return _origDateToString.call(this);
            const p = _tzParts(this, tz);
            if (!p) return _origDateToString.call(this);
            const longName = _tzLongName(this, tz);
            const offStr = _gmtOffsetString(this);
            // Format: "Tue Apr 29 2026 13:02:46 GMT-0700 (Pacific Daylight Time)"
            const longPart = longName ? ` (${longName})` : "";
            return `${p.weekday} ${p.month} ${p.day} ${p.year} ${p.hour}:${p.minute}:${p.second} ${offStr}${longPart}`;
        };
        Object.defineProperty(Date.prototype.toString, "toString", {
            value: () => "function toString() { [native code] }",
            configurable: true,
        });
        Object.defineProperty(Date.prototype.toString, _nativeTag, { value: 'toString', configurable: true });

        const _origDateToDateString = Date.prototype.toDateString;
        Date.prototype.toDateString = function toDateString() {
            if (!(this instanceof Date) || Number.isNaN(this.getTime?.())) {
                return _origDateToDateString.call(this);
            }
            const tz = _profileTz();
            if (!tz) return _origDateToDateString.call(this);
            const p = _tzParts(this, tz);
            if (!p) return _origDateToDateString.call(this);
            return `${p.weekday} ${p.month} ${p.day} ${p.year}`;
        };
        Object.defineProperty(Date.prototype.toDateString, "toString", {
            value: () => "function toDateString() { [native code] }",
            configurable: true,
        });
        Object.defineProperty(Date.prototype.toDateString, _nativeTag, { value: 'toDateString', configurable: true });

        const _origDateToTimeString = Date.prototype.toTimeString;
        Date.prototype.toTimeString = function toTimeString() {
            if (!(this instanceof Date) || Number.isNaN(this.getTime?.())) {
                return _origDateToTimeString.call(this);
            }
            const tz = _profileTz();
            if (!tz) return _origDateToTimeString.call(this);
            const p = _tzParts(this, tz);
            if (!p) return _origDateToTimeString.call(this);
            const longName = _tzLongName(this, tz);
            const offStr = _gmtOffsetString(this);
            const longPart = longName ? ` (${longName})` : "";
            return `${p.hour}:${p.minute}:${p.second} ${offStr}${longPart}`;
        };
        Object.defineProperty(Date.prototype.toTimeString, "toString", {
            value: () => "function toTimeString() { [native code] }",
            configurable: true,
        });
        Object.defineProperty(Date.prototype.toTimeString, _nativeTag, { value: 'toTimeString', configurable: true });

        // toLocaleString is already covered by the patched Intl.DateTimeFormat
        // (which Date.prototype.toLocaleString delegates to internally), but
        // V8's implementation calls the *original* Intl constructor directly
        // bypassing our patch. Force-route through our patched constructor.
        const _origDateToLocaleString = Date.prototype.toLocaleString;
        Date.prototype.toLocaleString = function toLocaleString(...args) {
            if (!(this instanceof Date) || Number.isNaN(this.getTime?.())) {
                return _origDateToLocaleString.apply(this, args);
            }
            const tz = _profileTz();
            const loc = _profileLocale() || "en-US";
            if (!tz) return _origDateToLocaleString.apply(this, args);
            try {
                const locales = args[0] !== undefined ? args[0] : loc;
                const options = Object.assign(
                    { timeZone: tz },
                    args[1] || {
                        year: "numeric", month: "numeric", day: "numeric",
                        hour: "numeric", minute: "numeric", second: "numeric",
                    },
                );
                return new _OrigDTF(locales, options).format(this);
            } catch (_e) {
                return _origDateToLocaleString.apply(this, args);
            }
        };
        Object.defineProperty(Date.prototype.toLocaleString, "toString", {
            value: () => "function toLocaleString() { [native code] }",
            configurable: true,
        });
        Object.defineProperty(Date.prototype.toLocaleString, _nativeTag, { value: 'toLocaleString', configurable: true });
    }

    // =========================================================
    // P1 FIX: PerformanceNavigationTiming + PerformanceResourceTiming
    // Akamai Bot Manager's sensor_data reads performance.getEntriesByType
    // ('navigation') and ('resource') to verify timing consistency against
    // expected Chrome distributions. Returning empty arrays is a tell.
    //
    // We synthesize a realistic set of entries based on the time the page
    // has been loaded, with sub-timings that look like a real Chrome on
    // broadband.
    // =========================================================
    if (globalThis.performance) {
        const _perfOrigin = Date.now();
        // Navigation timing constants (relative to the navigation start)
        const _perfNav = {
            name: globalThis.location?.href || "about:blank",
            entryType: "navigation",
            startTime: 0,
            duration: 0,
            initiatorType: "navigation",
            nextHopProtocol: "h2",
            workerStart: 0,
            redirectStart: 0,
            redirectEnd: 0,
            fetchStart: 0.1,
            domainLookupStart: 2.1,
            domainLookupEnd: 15.2,
            connectStart: 15.2,
            secureConnectionStart: 28.5,
            connectEnd: 78.3,
            requestStart: 78.4,
            responseStart: 145.6,
            responseEnd: 189.2,
            transferSize: 45678,
            encodedBodySize: 45000,
            decodedBodySize: 156789,
            serverTiming: [],
            unloadEventStart: 0,
            unloadEventEnd: 0,
            domInteractive: 320.5,
            domContentLoadedEventStart: 325.1,
            domContentLoadedEventEnd: 328.7,
            domComplete: 512.3,
            loadEventStart: 512.3,
            loadEventEnd: 515.9,
            type: "navigate",
            redirectCount: 0,
            activationStart: 0,
        };
        const _perfTimingStart = _perfOrigin - Math.round(_perfNav.loadEventEnd);
        const _perfTiming = {
            navigationStart: _perfTimingStart,
            unloadEventStart: 0,
            unloadEventEnd: 0,
            redirectStart: 0,
            redirectEnd: 0,
            fetchStart: _perfTimingStart + Math.round(_perfNav.fetchStart),
            domainLookupStart: _perfTimingStart + Math.round(_perfNav.domainLookupStart),
            domainLookupEnd: _perfTimingStart + Math.round(_perfNav.domainLookupEnd),
            connectStart: _perfTimingStart + Math.round(_perfNav.connectStart),
            connectEnd: _perfTimingStart + Math.round(_perfNav.connectEnd),
            secureConnectionStart: _perfTimingStart + Math.round(_perfNav.secureConnectionStart),
            requestStart: _perfTimingStart + Math.round(_perfNav.requestStart),
            responseStart: _perfTimingStart + Math.round(_perfNav.responseStart),
            responseEnd: _perfTimingStart + Math.round(_perfNav.responseEnd),
            domLoading: _perfTimingStart + Math.round(_perfNav.responseStart),
            domInteractive: _perfTimingStart + Math.round(_perfNav.domInteractive),
            domContentLoadedEventStart: _perfTimingStart + Math.round(_perfNav.domContentLoadedEventStart),
            domContentLoadedEventEnd: _perfTimingStart + Math.round(_perfNav.domContentLoadedEventEnd),
            domComplete: _perfTimingStart + Math.round(_perfNav.domComplete),
            loadEventStart: _perfTimingStart + Math.round(_perfNav.loadEventStart),
            loadEventEnd: _perfTimingStart + Math.round(_perfNav.loadEventEnd),
        };
        const _perfNavigation = {
            type: 0,
            redirectCount: 0,
            TYPE_NAVIGATE: 0,
            TYPE_RELOAD: 1,
            TYPE_BACK_FORWARD: 2,
            TYPE_RESERVED: 255,
        };

        const _buildResourceEntries = () => {
            const entries = [];
            const origin = globalThis.location?.origin || "https://example.com";
            const base = _perfNav.fetchStart;
            let offset = 10;
            const _internalEntries = _browser_oxide.__perfResourceEntries || [];
            const _rustEntries = (ops.op_perf_get_resource_timings && ops.op_perf_get_resource_timings()) || [];

            const mk = (name, startOffset, duration, type, size) => ({
                name,
                entryType: "resource",
                startTime: base + startOffset,
                duration,
                initiatorType: type,
                nextHopProtocol: "h2",
                workerStart: 0,
                redirectStart: 0,
                redirectEnd: 0,
                fetchStart: base + startOffset,
                domainLookupStart: base + startOffset,
                domainLookupEnd: base + startOffset,
                connectStart: base + startOffset,
                connectEnd: base + startOffset,
                secureConnectionStart: base + startOffset,
                requestStart: base + startOffset + 5,
                responseStart: base + startOffset + duration - 15,
                responseEnd: base + startOffset + duration,
                transferSize: size + 300,
                encodedBodySize: size,
                decodedBodySize: size * 3,
                serverTiming: [],
                renderBlockingStatus: "non-blocking",
            });

            if (globalThis.document) {
                const scripts = globalThis.document.scripts || [];
                for (let i = 0; i < scripts.length; i++) {
                    if (scripts[i].src) {
                        entries.push(mk(scripts[i].src, offset, 50, "script", 48600));
                        offset += 15;
                    }
                }
                const links = globalThis.document.getElementsByTagName('link') || [];
                for (let i = 0; i < links.length; i++) {
                    if (links[i].rel === 'stylesheet' && links[i].href) {
                        entries.push(mk(links[i].href, offset, 30, "link", 12500));
                        offset += 10;
                    }
                }
                const images = globalThis.document.images || [];
                for (let i = 0; i < images.length; i++) {
                    if (images[i].src) {
                        entries.push(mk(images[i].src, offset, 80, "img", 25000));
                        offset += 20;
                    }
                }
            }

            for (const req of _internalEntries) {
                const sTime = req.startTime || (base + offset);
                const e = mk(req.url, sTime - base, req.duration || 100, req.type || "xmlhttprequest", req.size || 1024);
                e.startTime = sTime;
                e.fetchStart = sTime;
                e.domainLookupStart = sTime;
                e.domainLookupEnd = sTime;
                e.connectStart = sTime;
                e.connectEnd = sTime;
                e.secureConnectionStart = sTime;
                e.requestStart = sTime + 5;
                e.responseStart = sTime + (req.duration || 100) - 15;
                e.responseEnd = sTime + (req.duration || 100);
                entries.push(e);
                offset += Math.max(10, (req.duration || 100) * 0.1);
            }

            for (const rt of _rustEntries) {
                const e = mk(rt.name, 0, rt.duration, "other", 0);
                e.startTime = rt.start_time;
                e.fetchStart = rt.fetch_start;
                e.domainLookupStart = rt.domain_lookup_start;
                e.domainLookupEnd = rt.domain_lookup_end;
                e.connectStart = rt.connect_start;
                e.connect_end = rt.connect_end;
                e.secureConnectionStart = rt.secure_connection_start;
                e.requestStart = rt.request_start;
                e.responseStart = rt.response_start;
                e.responseEnd = rt.response_end;
                entries.push(e);
            }

            if (entries.length === 0) {
                // Fingerprint scripts (Kasada/DataDome/Akamai) probe
                // `performance.getEntriesByType('resource').length` and
                // a near-empty list is a tell. Synthesize the typical
                // resource shape of a generic page: favicon + main JS
                // bundle + main CSS bundle + analytics ping + sw.
                entries.push(mk(`${origin}/favicon.ico`, 25, 42, "img", 1024));
                entries.push(mk(`${origin}/main.js`, 12, 87, "script", 58600));
                entries.push(mk(`${origin}/main.css`, 8, 33, "link", 14200));
                entries.push(mk(`${origin}/analytics.gif?t=` + (Date.now() % 1_000_000), 65, 18, "img", 35));
                entries.push(mk(`${origin}/sw.js`, 95, 14, "script", 1840));
            }
            return entries;
        };
        const _navEntry = () => {
            const entry = Object.assign({}, _perfNav);
            entry.duration = performance.now();
            
            if (globalThis.document && globalThis.document.readyState !== 'complete') {
                entry.domComplete = 0;
                entry.loadEventStart = 0;
                entry.loadEventEnd = 0;
            }
            if (globalThis.document && globalThis.document.readyState === 'loading') {
                entry.domInteractive = 0;
                entry.domContentLoadedEventStart = 0;
                entry.domContentLoadedEventEnd = 0;
            }
            
            return entry;
        };

        // ================================================================
        // Install on Performance.prototype — not on the instance.
        // ================================================================
        // deno_core only provides `performance.now()` natively; everything
        // else is our JS. Previously we assigned stubs directly to the
        // instance, which left 14 own properties (Chrome has zero) and
        // exposed raw JS source via `getEntries.toString()`. Now we build
        // a Performance class, install every accessor/method on the
        // prototype, reparent the existing performance instance to it,
        // and strip the legacy own properties.
        const _PerfProto = Performance.prototype;
        Object.defineProperty(_PerfProto, Symbol.toStringTag, { value: "Performance", configurable: true });

        // Preserve the native `now` before we reparent — deno_core installs
        // it as an own property on the instance with a native-code toString.
        const _origNow = globalThis.performance.now && globalThis.performance.now.bind(globalThis.performance);

        _defProtoGetter(_PerfProto, 'memory', () => {
            const jsHeapSizeLimit = 4294705152;
            const base = 10485760; // 10 MB
            const jitter = ((Date.now() * 0x9e3779b9) >>> 0) % 5000000;
            const totalJSHeapSize = base + jitter;
            const usedJSHeapSize = Math.floor(totalJSHeapSize * 0.85);
            return {
                jsHeapSizeLimit,
                totalJSHeapSize,
                usedJSHeapSize,
            };
        });
        _defProtoGetter(_PerfProto, 'timing', () => {
            const timing = Object.assign({}, _perfTiming);
            if (globalThis.document && globalThis.document.readyState !== 'complete') {
                timing.domComplete = 0;
                timing.loadEventStart = 0;
                timing.loadEventEnd = 0;
            }
            if (globalThis.document && globalThis.document.readyState === 'loading') {
                timing.domInteractive = 0;
                timing.domContentLoadedEventStart = 0;
                timing.domContentLoadedEventEnd = 0;
            }
            return timing;
        });
        _defProtoGetter(_PerfProto, 'timeOrigin', () => _perfTimingStart);
        _defProtoGetter(_PerfProto, 'navigation', () => _perfNavigation);
        _defProtoGetter(_PerfProto, 'onresourcetimingbufferfull', () => null);

        _defProtoMethod(_PerfProto, 'getEntries', function getEntries() {
            const entries = [_navEntry(), ..._buildResourceEntries()];
            const origin = globalThis.location ? globalThis.location.origin : "";
            
            // Add Qrator/WBAAS fallback if not present
            if (!entries.some(e => e.name.includes('qauth') || e.name.includes('wbaas'))) {
                const start = 12.5;
                const dur = 45.2;
                entries.push({
                    name: `${origin}/__qrator/qauth_utm_v2d_v9118.js`,
                    entryType: 'resource',
                    startTime: start,
                    duration: dur,
                    initiatorType: 'script',
                    nextHopProtocol: 'h2',
                    workerStart: 0,
                    redirectStart: 0,
                    redirectEnd: 0,
                    fetchStart: start,
                    domainLookupStart: start,
                    domainLookupEnd: start,
                    connectStart: start,
                    connectEnd: start,
                    secureConnectionStart: start,
                    requestStart: start + 1,
                    responseStart: start + 5,
                    responseEnd: start + dur,
                    transferSize: 349878,
                    encodedBodySize: 349800,
                    decodedBodySize: 349800,
                    serverTiming: []
                });
            }
            return entries;
        });
        _defProtoMethod(_PerfProto, 'getEntriesByType', function getEntriesByType(type) {
            if (type === "navigation") return [_navEntry()];
            if (type === "resource") {
                return globalThis.performance.getEntries().filter(e => e.entryType === 'resource');
            }
            if (type === "mark" || type === "measure") return [];
            if (type === "paint") {
                return [
                    { name: "first-paint", entryType: "paint", startTime: 156.3, duration: 0 },
                    { name: "first-contentful-paint", entryType: "paint", startTime: 189.7, duration: 0 },
                ];
            }
            return [];
        });
        _defProtoMethod(_PerfProto, 'getEntriesByName', function getEntriesByName(name, type) {
            return globalThis.performance
                .getEntries()
                .filter((e) => e.name === name && (!type || e.entryType === type));
        });
        _defProtoMethod(_PerfProto, 'mark', function mark(name) {
            return { name, entryType: "mark", startTime: performance.now(), duration: 0 };
        });
        _defProtoMethod(_PerfProto, 'measure', function measure(name, startMark, endMark) {
            return { name, entryType: "measure", startTime: 0, duration: 0 };
        });
        _defProtoMethod(_PerfProto, 'clearMarks', function clearMarks() {});
        _defProtoMethod(_PerfProto, 'clearMeasures', function clearMeasures() {});
        _defProtoMethod(_PerfProto, 'clearResourceTimings', function clearResourceTimings() {
            // No-op for now, as we dynamically fetch from document.
        });
        _defProtoMethod(_PerfProto, 'setResourceTimingBufferSize', function setResourceTimingBufferSize() {});
        _defProtoMethod(_PerfProto, 'toJSON', function toJSON() {
            return { timeOrigin: _perfTimingStart };
        });

        if (_origNow) {
            _defProtoMethod(_PerfProto, 'now', function now() { return _origNow(); });
        }

        // Reparent the existing performance instance onto Performance.prototype
        // and strip the legacy own properties. Matches real Chrome, which has
        // zero own properties on the performance instance.
        try {
            Object.setPrototypeOf(globalThis.performance, _PerfProto);
        } catch (e) { /* immutable proto — fall back below */ }
        for (const p of Object.getOwnPropertyNames(globalThis.performance)) {
            try { delete globalThis.performance[p]; } catch {}
        }
        // If setPrototypeOf failed (rare) we fall back to a fresh instance
        // that still exposes `now` via a captured closure.
        if (Object.getPrototypeOf(globalThis.performance) !== _PerfProto) {
            globalThis.performance = Object.create(_PerfProto);
        }
    }

    // ================================================================
    // Crypto / SubtleCrypto classes + prototype — kNoScriptId-safe.
    // ================================================================
    // Real Chrome exposes `window.crypto` as an instance of `Crypto`
    // with all methods on `Crypto.prototype` and a `subtle` accessor
    // returning an instance of `SubtleCrypto`. The `digest` method is
    // native-code backed; Kasada and DataDome both call it to hash
    // challenge payloads (SHA-256 over TextEncoder-produced bytes).
    //
    // Previously we had `globalThis.crypto = {}` with `getRandomValues`
    // and `randomUUID` as own properties, and NO `subtle` at all —
    // causing `crypto.subtle.digest` to throw "Cannot read properties
    // of undefined (reading 'digest')". Now we expose full classes
    // backed by Rust ops for real SHA-1/256/384/512 digest.
    class Crypto {}
    globalThis.Crypto = Crypto;
    const _CryptoProto = Crypto.prototype;

    class SubtleCrypto {}
    globalThis.SubtleCrypto = SubtleCrypto;
    const _SubtleProto = SubtleCrypto.prototype;
    const _subtleInstance = Object.create(_SubtleProto);

    // Coerce BufferSource → Uint8Array for op bridging.
    const _toBytes = (src) => {
        if (src == null) return new Uint8Array(0);
        if (src instanceof Uint8Array) return src;
        if (src instanceof ArrayBuffer) return new Uint8Array(src);
        if (ArrayBuffer.isView(src)) return new Uint8Array(src.buffer, src.byteOffset, src.byteLength);
        return new Uint8Array(src);
    };

    _defProtoMethod(_SubtleProto, 'digest', function digest(algorithm, data) {
        try {
            const algName = typeof algorithm === 'string' ? algorithm : (algorithm && algorithm.name) || "";
            const bytes = _toBytes(data);
            const out = ops.op_crypto_digest(String(algName), bytes);
            // Real Web Crypto returns a Promise<ArrayBuffer>.
            return Promise.resolve(out.buffer.slice(out.byteOffset, out.byteOffset + out.byteLength));
        } catch (e) {
            return Promise.reject(e);
        }
    });
    // Stubs for sign/verify/encrypt/decrypt/generateKey/importKey/exportKey/deriveKey/deriveBits/wrapKey/unwrapKey.
    // Real implementations are expensive; most antibots only call digest(),
    // so we expose the methods as native-shaped no-ops that reject.
    const _subtleNotImplemented = (name) => function (...args) {
        return Promise.reject(new DOMException(`${name} not implemented`, "NotSupportedError"));
    };
    for (const m of ['sign','verify','encrypt','decrypt','generateKey','importKey','exportKey','deriveKey','deriveBits','wrapKey','unwrapKey']) {
        _defProtoMethod(_SubtleProto, m, _subtleNotImplemented(m));
    }

    // Crypto.prototype.getRandomValues — backed by the Rust op.
    _defProtoMethod(_CryptoProto, 'getRandomValues', function getRandomValues(arr) {
        if (!ArrayBuffer.isView(arr)) {
            throw new TypeError("getRandomValues expects an ArrayBufferView");
        }
        if (arr.byteLength > 65536) {
            throw new DOMException("QuotaExceededError", "QuotaExceededError");
        }
        // We need a Uint8Array view to pass to the op.
        const u8 = new Uint8Array(arr.buffer, arr.byteOffset, arr.byteLength);
        ops.op_crypto_random_fill(u8);
        return arr;
    });
    _defProtoMethod(_CryptoProto, 'randomUUID', function randomUUID() {
        // Generate 16 random bytes, then format per RFC 4122 v4.
        const b = new Uint8Array(16);
        ops.op_crypto_random_fill(b);
        b[6] = (b[6] & 0x0f) | 0x40; // version
        b[8] = (b[8] & 0x3f) | 0x80; // variant
        const hex = [];
        for (let i = 0; i < 16; i++) hex.push(b[i].toString(16).padStart(2, '0'));
        return `${hex.slice(0,4).join('')}-${hex.slice(4,6).join('')}-${hex.slice(6,8).join('')}-${hex.slice(8,10).join('')}-${hex.slice(10,16).join('')}`;
    });
    _defProtoGetter(_CryptoProto, 'subtle', () => _subtleInstance);

    Object.defineProperty(_CryptoProto, Symbol.toStringTag, { value: "Crypto", configurable: true });
    Object.defineProperty(_SubtleProto, Symbol.toStringTag, { value: "SubtleCrypto", configurable: true });

    // Reparent or create the crypto instance.
    let _cryptoInstance = Object.create(_CryptoProto);
    globalThis.crypto = _cryptoInstance;

    // ================================================================
    // TextEncoder / TextDecoder — Chrome-shaped, kNoScriptId-safe.
    // ================================================================
    // deno_core at our version ships without deno_web, so we have no
    // native TextEncoder. Our JS stub must match Chrome's exact shape
    // because Kasada's ips.js (and CreepJS, and Castle) probe it:
    //
    //   1. new TextEncoder().encoding === "utf-8"   (a GETTER on proto)
    //   2. TextEncoder.prototype.encodeInto exists
    //   3. TextEncoder.toString().includes("[native code]")
    //   4. TextEncoder.prototype.encode.toString().includes("[native code]")
    //   5. Object.getOwnPropertyDescriptor(TextEncoder.prototype, 'encoding')
    //      returns an accessor descriptor ({ get: ƒ, set: undefined, ... })
    //
    // Prior bug: we exposed a plain `class TextEncoder { encode(){...} }`,
    // which failed every one of these probes. Kasada's TextEncoder probe
    // (the one that throws "Cannot read properties of undefined") was
    // most likely `new TextEncoder().encoding.charCodeAt(0)` — `encoding`
    // was undefined, so `.charCodeAt` threw.
    if (!globalThis.TextEncoder || !TextEncoder.prototype.encodeInto) {
        class TextEncoder {
            constructor() {}
            encode(str) {
                str = String(str == null ? "" : str);
                const buf = [];
                for (let i = 0; i < str.length; i++) {
                    let c = str.charCodeAt(i);
                    // UTF-16 surrogate pair handling
                    if (c >= 0xD800 && c <= 0xDBFF && i + 1 < str.length) {
                        const low = str.charCodeAt(i + 1);
                        if (low >= 0xDC00 && low <= 0xDFFF) {
                            c = 0x10000 + ((c - 0xD800) << 10) + (low - 0xDC00);
                            i++;
                        }
                    }
                    if (c < 0x80) {
                        buf.push(c);
                    } else if (c < 0x800) {
                        buf.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f));
                    } else if (c < 0x10000) {
                        buf.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f));
                    } else {
                        buf.push(
                            0xf0 | (c >> 18),
                            0x80 | ((c >> 12) & 0x3f),
                            0x80 | ((c >> 6) & 0x3f),
                            0x80 | (c & 0x3f),
                        );
                    }
                }
                return new Uint8Array(buf);
            }
            encodeInto(source, destination) {
                if (!(destination instanceof Uint8Array)) {
                    throw new TypeError("encodeInto destination must be a Uint8Array");
                }
                source = String(source == null ? "" : source);
                let read = 0;
                let written = 0;
                for (let i = 0; i < source.length; i++) {
                    let c = source.charCodeAt(i);
                    let extraChar = 0;
                    if (c >= 0xD800 && c <= 0xDBFF && i + 1 < source.length) {
                        const low = source.charCodeAt(i + 1);
                        if (low >= 0xDC00 && low <= 0xDFFF) {
                            c = 0x10000 + ((c - 0xD800) << 10) + (low - 0xDC00);
                            extraChar = 1;
                        }
                    }
                    let bytes;
                    if (c < 0x80) bytes = [c];
                    else if (c < 0x800) bytes = [0xc0 | (c >> 6), 0x80 | (c & 0x3f)];
                    else if (c < 0x10000) bytes = [0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f)];
                    else bytes = [0xf0 | (c >> 18), 0x80 | ((c >> 12) & 0x3f), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f)];
                    if (written + bytes.length > destination.length) break;
                    for (let j = 0; j < bytes.length; j++) destination[written + j] = bytes[j];
                    written += bytes.length;
                    read += 1 + extraChar;
                    if (extraChar) i++;
                }
                return { read, written };
            }
        }
        globalThis.TextEncoder = TextEncoder;
        // `encoding` is a GETTER on TextEncoder.prototype that always returns "utf-8".
        _defProtoGetter(TextEncoder.prototype, 'encoding', () => "utf-8");
        // Mask encode, encodeInto, and the constructor as native.
        _defProtoMethod(TextEncoder.prototype, 'encode', TextEncoder.prototype.encode);
        _defProtoMethod(TextEncoder.prototype, 'encodeInto', TextEncoder.prototype.encodeInto);
        try {
            Object.defineProperty(TextEncoder, 'toString', {
                value: function toString() { return 'function TextEncoder() { [native code] }'; },
                configurable: true,
            });
            Object.defineProperty(TextEncoder, _nativeTag, { value: 'TextEncoder', configurable: true });
            Object.defineProperty(TextEncoder, 'name', { value: 'TextEncoder', configurable: true });
        } catch {}
    }
    if (!globalThis.TextDecoder || !('encoding' in TextDecoder.prototype)) {
        class TextDecoder {
            constructor(label = "utf-8", options = {}) {
                this._label = String(label).toLowerCase();
                this._fatal = !!options.fatal;
                this._ignoreBOM = !!options.ignoreBOM;
            }
            decode(buf, options) {
                if (buf === undefined) return "";
                let bytes;
                if (buf instanceof ArrayBuffer) bytes = new Uint8Array(buf);
                else if (ArrayBuffer.isView(buf)) bytes = new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
                else bytes = new Uint8Array(buf);
                let str = "";
                let i = 0;
                // Skip BOM unless ignoreBOM is set
                if (!this._ignoreBOM && bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
                    i = 3;
                }
                while (i < bytes.length) {
                    const b0 = bytes[i];
                    if (b0 < 0x80) { str += String.fromCharCode(b0); i++; }
                    else if ((b0 & 0xe0) === 0xc0 && i + 1 < bytes.length) {
                        const cp = ((b0 & 0x1f) << 6) | (bytes[i+1] & 0x3f);
                        str += String.fromCharCode(cp); i += 2;
                    }
                    else if ((b0 & 0xf0) === 0xe0 && i + 2 < bytes.length) {
                        const cp = ((b0 & 0x0f) << 12) | ((bytes[i+1] & 0x3f) << 6) | (bytes[i+2] & 0x3f);
                        str += String.fromCharCode(cp); i += 3;
                    }
                    else if ((b0 & 0xf8) === 0xf0 && i + 3 < bytes.length) {
                        let cp = ((b0 & 0x07) << 18) | ((bytes[i+1] & 0x3f) << 12) | ((bytes[i+2] & 0x3f) << 6) | (bytes[i+3] & 0x3f);
                        cp -= 0x10000;
                        str += String.fromCharCode(0xD800 + (cp >> 10), 0xDC00 + (cp & 0x3ff));
                        i += 4;
                    }
                    else {
                        // Invalid byte — fatal throws, otherwise emits replacement char U+FFFD
                        if (this._fatal) throw new TypeError("The encoded data was not valid.");
                        str += "\uFFFD"; i++;
                    }
                }
                return str;
            }
        }
        globalThis.TextDecoder = TextDecoder;
        _defProtoGetter(TextDecoder.prototype, 'encoding', function encoding() { return this._label || "utf-8"; });
        _defProtoGetter(TextDecoder.prototype, 'fatal', function fatal() { return this._fatal; });
        _defProtoGetter(TextDecoder.prototype, 'ignoreBOM', function ignoreBOM() { return this._ignoreBOM; });
        _defProtoMethod(TextDecoder.prototype, 'decode', TextDecoder.prototype.decode);
        try {
            Object.defineProperty(TextDecoder, 'toString', {
                value: function toString() { return 'function TextDecoder() { [native code] }'; },
                configurable: true,
            });
            Object.defineProperty(TextDecoder, _nativeTag, { value: 'TextDecoder', configurable: true });
            Object.defineProperty(TextDecoder, 'name', { value: 'TextDecoder', configurable: true });
        } catch {}
    }

    // atob / btoa
    if (!globalThis.atob) {
        globalThis.atob = ({
            atob(s) {
                if (arguments.length === 0) {
                    throw new TypeError("Failed to execute 'atob' on 'Window': 1 argument required, but only 0 present.");
                }
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
            }
        }).atob;
        _maskFunction(globalThis.atob, 'atob');
    }
    const _origStringify = JSON.stringify;
    if (!globalThis.btoa) {
        globalThis.btoa = ({
            btoa(s) {
                if (arguments.length === 0) {
                    throw new TypeError("Failed to execute 'btoa' on 'Window': 1 argument required, but only 0 present.");
                }
                const str = String(s);
                for (let i = 0; i < str.length; i++) {
                    if (str.charCodeAt(i) > 255) {
                        throw new DOMException("Failed to execute 'btoa' on 'Window': The string to be encoded contains characters outside of the Latin1 range.", "InvalidCharacterError");
                    }
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
        }
        }).btoa;
        _maskFunction(globalThis.btoa, 'btoa');
    }

    // localStorage / sessionStorage persistent stubs (backed by Rust DomState)
    const LOCAL_STORAGE_QUOTA = 5242880; // 5 MB
    
    function getStorageAreaSize(type) {
        const keys = ops.op_dom_storage_keys(type);
        let size = 0;
        for (const k of keys) {
            const v = ops.op_dom_storage_get(type, k);
            size += String(k).length + (v ? String(v).length : 0);
        }
        return size;
    }

    function setStorageItem(type, key, value) {
        const valStr = String(value);
        const newSize = String(key).length + valStr.length;
        const oldVal = ops.op_dom_storage_get(type, key);
        const oldSize = oldVal ? String(key).length + String(oldVal).length : 0;
        
        const currentSize = getStorageAreaSize(type);
        if (currentSize - oldSize + newSize > LOCAL_STORAGE_QUOTA) {
            throw new DOMException(
                "Failed to execute 'setItem' on 'Storage': Setting the value of '" + key + "' exceeded the quota.",
                'QuotaExceededError'
            );
        }
        
        ops.op_dom_storage_set(type, key, valStr);
        return true;
    }

    function makeStorage(type) {
        const STORAGE_METHODS = ["getItem", "setItem", "removeItem", "clear", "key", "length"];
        return new Proxy({}, {
            get(target, key) {
                if (key === "getItem") return (k) => ops.op_dom_storage_get(type, String(k));
                if (key === "setItem") return (k, v) => { setStorageItem(type, String(k), v); };
                if (key === "removeItem") return (k) => { ops.op_dom_storage_remove(type, String(k)); };
                if (key === "clear") return () => { ops.op_dom_storage_clear(type); };
                if (key === "key") return (i) => ops.op_dom_storage_keys(type)[i] ?? null;
                if (key === "length") return ops.op_dom_storage_keys(type).length;

                // Fallback to getting the item directly if it's not a method
                return ops.op_dom_storage_get(type, String(key)) ?? undefined;
            },
            // V8 Proxy invariant: `has` must agree with `ownKeys` about what
            // keys exist. Without an explicit trap, V8 falls back to the empty
            // target object — which says "no keys" and contradicts ownKeys's
            // real list. The reconciliation is hot work that creepjs hits
            // repeatedly via `'name' in storage` style probes.
            has(target, key) {
                if (STORAGE_METHODS.includes(key)) return true;
                return ops.op_dom_storage_get(type, String(key)) !== null;
            },
            set(target, key, value) { return setStorageItem(type, String(key), value); },
            deleteProperty(target, key) {
                ops.op_dom_storage_remove(type, String(key));
                return true;
            },
            ownKeys() {
                return ops.op_dom_storage_keys(type);
            },
            getOwnPropertyDescriptor(target, key) {
                const val = ops.op_dom_storage_get(type, String(key));
                if (val !== null) {
                    return { value: val, enumerable: true, configurable: true, writable: true };
                }
            }
        });
    }
    globalThis.localStorage = makeStorage("local");
    globalThis.sessionStorage = makeStorage("session");

    // MutationObserver — real implementation in dom_bootstrap.js

    // Task#2: real Chrome exposes `IntersectionObserverEntry` as a
    // global whose PROTOTYPE carries `intersectionRatio` /
    // `isIntersecting` (readonly accessors). duolingo's
    // `supportsIntersectionObserver` capability gate checks exactly
    // `"IntersectionObserverEntry" in window && "intersectionRatio" in
    // window.IntersectionObserverEntry.prototype && "isIntersecting" in
    // …prototype`; without a real entry class the homepage
    // self-redirects to /errors/not-supported.html. Class getters land
    // on the prototype, satisfying the `in prototype` checks.
    globalThis.IntersectionObserverEntry = class IntersectionObserverEntry {
        constructor(init = {}) { this._i = init || {}; }
        get target() { return this._i.target ?? null; }
        get isIntersecting() { return this._i.isIntersecting ?? false; }
        get intersectionRatio() { return this._i.intersectionRatio ?? 0; }
        get boundingClientRect() { return this._i.boundingClientRect ?? null; }
        get intersectionRect() { return this._i.intersectionRect ?? null; }
        get rootBounds() { return this._i.rootBounds ?? null; }
        get time() { return this._i.time ?? 0; }
    };

    // IntersectionObserver — fires immediately since all elements are "in viewport" in headless
    globalThis.IntersectionObserver = class IntersectionObserver {
        constructor(callback, options = {}) {
            this._callback = callback;
            this._options = options;
            this._elements = new Set();
        }
        observe(target) {
            this._elements.add(target);
            // In headless mode, all elements are considered intersecting
            Promise.resolve().then(() => {
                if (!this._elements.has(target)) return;
                const entry = new globalThis.IntersectionObserverEntry({
                    target,
                    isIntersecting: true,
                    intersectionRatio: 1.0,
                    boundingClientRect: target.getBoundingClientRect ? target.getBoundingClientRect() : {},
                    intersectionRect: target.getBoundingClientRect ? target.getBoundingClientRect() : {},
                    rootBounds: null,
                    time: performance.now(),
                });
                this._callback([entry], this);
            });
        }
        unobserve(target) { this._elements.delete(target); }
        disconnect() { this._elements.clear(); }
        takeRecords() { return []; }
    };

    // ResizeObserver — fires on observe() with current dimensions
    globalThis.ResizeObserver = class ResizeObserver {
        constructor(callback) {
            this._callback = callback;
            this._elements = new Set();
        }
        observe(target) {
            this._elements.add(target);
            Promise.resolve().then(() => {
                if (!this._elements.has(target)) return;
                const entry = {
                    target,
                    contentRect: target.getBoundingClientRect ? target.getBoundingClientRect() : { x: 0, y: 0, width: 0, height: 0 },
                    borderBoxSize: [{ inlineSize: target.offsetWidth || 0, blockSize: target.offsetHeight || 0 }],
                    contentBoxSize: [{ inlineSize: target.offsetWidth || 0, blockSize: target.offsetHeight || 0 }],
                };
                this._callback([entry], this);
            });
        }
        unobserve(target) { this._elements.delete(target); }
        disconnect() { this._elements.clear(); }
    };

    // requestIdleCallback stub
    globalThis.requestIdleCallback = ({
        requestIdleCallback(cb) {
            return setTimeout(() => cb({ didTimeout: false, timeRemaining: () => 50 }), 1);
        }
    }).requestIdleCallback;
    _maskFunction(globalThis.requestIdleCallback, 'requestIdleCallback');

    globalThis.cancelIdleCallback = ({
        cancelIdleCallback(id) {
            return clearTimeout(id);
        }
    }).cancelIdleCallback;
    _maskFunction(globalThis.cancelIdleCallback, 'cancelIdleCallback');

    // getComputedStyle — reads inline style from actual element, falls back to CSS defaults.
    // CAPTURE _getNodeId at bootstrap time: cleanup_bootstrap.js deletes
    // __browser_oxide before page scripts run, so per-call lookup degrades to
    // nodeId=0 (same bug that broke event_stop_propagation). This was why
    // every getComputedStyle() call returned the same root-element defaults
    // regardless of which element was passed.
    const _compStyleCache = new WeakMap();
    const _getNodeIdForCompStyle = (globalThis.__browser_oxide && globalThis.__browser_oxide._getNodeId)
        ? globalThis.__browser_oxide._getNodeId
        : (() => 0);
    globalThis.getComputedStyle = ({
        getComputedStyle(element, pseudoElt) {
            if (!element) return null;
            let styleProxy = _compStyleCache.get(element);
            if (styleProxy) return styleProxy;

            const nodeId = _getNodeIdForCompStyle(element);
            // Create an instance of CSSStyleDeclaration.
            const style = Object.create(globalThis.CSSStyleDeclaration.prototype || Object.prototype);
        let cache = null;
        let keys = null;
        function ensureCache() {
            if (cache === null) {
                cache = ops.op_dom_get_all_computed_styles(nodeId);
                keys = Object.keys(cache);
            }
            return cache;
        }
        styleProxy = new Proxy(style, {
            get(target, prop) {
                if (prop === "getPropertyValue") {
                    return (name) => {
                        const c = ensureCache();
                        return c[name] || ops.op_dom_get_computed_style(nodeId, name);
                    };
                }
                if (prop === "setProperty" || prop === "removeProperty") {
                    return () => {}; // read-only
                }
                if (prop === "length") {
                    ensureCache();
                    return keys.length;
                }
                if (prop === Symbol.toStringTag) return "CSSStyleDeclaration";
                if (typeof prop === "string") {
                    if (/^\d+$/.test(prop)) {
                        ensureCache();
                        return keys[parseInt(prop, 10)];
                    }
                    const kebab = prop.replace(/[A-Z]/g, m => "-" + m.toLowerCase());
                    const c = ensureCache();
                    if (Object.prototype.hasOwnProperty.call(c, kebab)) return c[kebab];
                    // Fallback to single-op for inheritance/defaults
                    return ops.op_dom_get_computed_style(nodeId, kebab);
                }
                return undefined;
            }
        });
        _compStyleCache.set(element, styleProxy);
        return styleProxy;
        }
    }).getComputedStyle;
    _maskFunction(globalThis.getComputedStyle, 'getComputedStyle');

    // XMLHttpRequest stub (built on fetch)
    // XMLHttpRequest — must extend EventTarget and expose the full Chrome
    // shape. Akamai BMP v3 monkey-patches `XMLHttpRequest.prototype.send`
    // and walks computed-name chains through the instance, so any missing
    // property (upload, responseType, withCredentials, timeout, response,
    // responseXML, abort, dispatchEvent, etc.) surfaces as
    //   `nqz[<computed>.<computed>.<computed>.<computed>] is not a function`
    // during sensor execution.
    globalThis.XMLHttpRequest = class XMLHttpRequest extends EventTarget {
        constructor() {
            super();
            this.readyState = 0;
            this.status = 0;
            this.statusText = "";
            this.responseText = "";
            this.responseXML = null;
            this.response = "";
            this.responseType = "";
            this.responseURL = "";
            this.withCredentials = false;
            this.timeout = 0;
            this._method = "GET";
            this._url = "";
            this._async = true;
            this._headers = {};
            this._respHeaders = {};
            this._aborted = false;
            // Event handler properties — match Chrome's XHR interface.
            this.onreadystatechange = null;
            this.onload = null;
            this.onloadstart = null;
            this.onloadend = null;
            this.onerror = null;
            this.onabort = null;
            this.ontimeout = null;
            this.onprogress = null;
            // upload — XMLHttpRequestUpload, also an EventTarget.
            const _XHRU = class XMLHttpRequestUpload extends EventTarget {
                constructor() {
                    super();
                    this.onload = null;
                    this.onloadstart = null;
                    this.onloadend = null;
                    this.onerror = null;
                    this.onabort = null;
                    this.ontimeout = null;
                    this.onprogress = null;
                }
            };
            Object.defineProperty(_XHRU.prototype, Symbol.toStringTag, { value: "XMLHttpRequestUpload", configurable: true });
            this.upload = new _XHRU();
        }
        static UNSENT = 0;
        static OPENED = 1;
        static HEADERS_RECEIVED = 2;
        static LOADING = 3;
        static DONE = 4;
        open(method, url, async = true, user, password) {
            console.log(`[XHR] open ${method} ${url}`);
            this._method = String(method || "GET").toUpperCase();
            let urlStr = String(url || "");
            if (urlStr && !urlStr.startsWith('http') && !urlStr.startsWith('data:') && !urlStr.startsWith('blob:')) {
                try {
                    let base = globalThis.location ? globalThis.location.href : 'about:blank';
                    if (base === 'about:blank' || base === 'javascript:;' || base === '') {
                        try { base = globalThis.parent.location.href; } catch(e) {}
                    }
                    const old = urlStr;
                    urlStr = new URL(urlStr, base).href;
                } catch(e) {
                }
            }
            this._url = urlStr;
            this._async = async !== false;
            this._headers = {};
            this._respHeaders = {};
            this._aborted = false;
            this.readyState = 1;
            this.status = 0;
            this.statusText = "";
            this.responseText = "";
            this.response = "";
            this.responseURL = "";
            try {
                const ev = new Event("readystatechange");
                if (typeof this.dispatchEvent === 'function') this.dispatchEvent(ev);
                if (this.onreadystatechange) this.onreadystatechange.call(this, ev);
            } catch {}
        }
        setRequestHeader(name, value) {
            this._headers[String(name)] = String(value);
        }
        overrideMimeType(mime) { this._overrideMime = String(mime); }
        send(body) {
            console.log(`[XHR] send ${this._method} ${this._url}`);
            const xhr = this;
            if (xhr._aborted) return;
            const fireEvent = (type) => {
                try {
                    const ev = new Event(type);
                    if (typeof xhr.dispatchEvent === 'function') xhr.dispatchEvent(ev);
                    const handler = xhr['on' + type];
                    if (typeof handler === 'function') handler.call(xhr, ev);
                } catch {}
            };

            // Encode body for the sync op (marker-prefixed like op_fetch).
            let bodyEncoded = '';
            if (body !== null && body !== undefined) {
                if (body instanceof ArrayBuffer || ArrayBuffer.isView(body)) {
                    const bytes = body instanceof ArrayBuffer
                        ? new Uint8Array(body)
                        : new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
                    let bin = '';
                    for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
                    bodyEncoded = 'b:' + btoa(bin);
                } else {
                    bodyEncoded = 's:' + String(body);
                }
            }

            // For synchronous XHR (async=false) — required by Kasada KPSDK which calls
            // xhr.open('POST', '/tl', false) + xhr.send() and reads xhr.status immediately
            // after send() returns. The async fetch() path can never satisfy this because
            // it requires V8 to yield, which doesn't happen when a PoW busy-wait is running.
            if (!xhr._async && typeof ops !== 'undefined' && typeof ops.op_net_xhr_sync === 'function') {
                const startTime = performance.now();
                try {
                    const origin = (globalThis.location && globalThis.location.origin !== 'null')
                        ? globalThis.location.origin : '';
                    const headersJson = JSON.stringify(
                        Object.entries(xhr._headers).map(([k, v]) => [k, String(v)])
                    );
                    const resultJson = ops.op_net_xhr_sync(
                        xhr._url, xhr._method, headersJson, bodyEncoded, origin
                    );
                    const result = JSON.parse(resultJson);
                    
                    const _internalEntries = _browser_oxide.__perfResourceEntries;
                    if (_internalEntries) {
                        _internalEntries.push({ url: xhr._url, type: "xmlhttprequest", startTime, duration: performance.now() - startTime, size: result.body ? result.body.length : 0 });
                    }
                    
                    xhr.status = result.status || 0;
                    xhr.statusText = '';
                    xhr.responseURL = result.url || xhr._url;
                    if (Array.isArray(result.headers)) {
                        for (const [k, v] of result.headers) {
                            xhr._respHeaders[String(k).toLowerCase()] = String(v);
                        }
                    }
                    xhr.responseText = result.body || '';
                    xhr.response = xhr.responseText;
                    xhr.readyState = 2; fireEvent('readystatechange');
                    xhr.readyState = 3; fireEvent('readystatechange');
                    xhr.readyState = 4; fireEvent('readystatechange');
                    fireEvent('load');
                    fireEvent('loadend');
                } catch(e) {
                    xhr.readyState = 4;
                    fireEvent('readystatechange');
                    fireEvent('error');
                    fireEvent('loadend');
                }
                return;
            }

            // Fallback: async fetch() path (used only when op_net_xhr_sync is unavailable).
            fireEvent('loadstart');
            fetch(xhr._url, {
                method: xhr._method,
                headers: xhr._headers,
                body,
                credentials: xhr.withCredentials ? 'include' : 'same-origin',
            })
                .then(async (resp) => {
                    const _internalEntries = _browser_oxide.__perfResourceEntries;
                    if (_internalEntries && _internalEntries.length > 0) {
                        _internalEntries[_internalEntries.length - 1].type = "xmlhttprequest";
                    }
                    if (xhr._aborted) return;
                    xhr.status = resp.status;
                    xhr.statusText = resp.statusText || "";
                    xhr.responseURL = resp.url || xhr._url;
                    try {
                        if (resp.headers && typeof resp.headers.forEach === 'function') {
                            resp.headers.forEach((v, k) => { xhr._respHeaders[String(k).toLowerCase()] = String(v); });
                        }
                    } catch {}
                    xhr.readyState = 2;
                    fireEvent('readystatechange');
                    xhr.readyState = 3;
                    fireEvent('readystatechange');
                    xhr.responseText = await resp.text();
                    xhr.response = xhr.responseText;
                    xhr.readyState = 4;
                    fireEvent('readystatechange');
                    fireEvent('load');
                    fireEvent('loadend');
                })
                .catch((e) => {
                    if (xhr._aborted) return;
                    xhr.readyState = 4;
                    fireEvent('readystatechange');
                    fireEvent('error');
                    fireEvent('loadend');
                });
        }
        abort() {
            this._aborted = true;
            this.readyState = 0;
            try {
                const ev = new Event("abort");
                if (typeof this.dispatchEvent === 'function') this.dispatchEvent(ev);
                if (this.onabort) this.onabort.call(this, ev);
            } catch {}
        }
        getResponseHeader(name) {
            return this._respHeaders[String(name).toLowerCase()] || null;
        }
        getAllResponseHeaders() {
            return Object.entries(this._respHeaders)
                .map(([k, v]) => `${k}: ${v}`)
                .join("\r\n");
        }
    };
    Object.defineProperty(globalThis.XMLHttpRequest.prototype, Symbol.toStringTag, { value: "XMLHttpRequest", configurable: true });

    // WebSocket — real connections via tokio-tungstenite ops
    globalThis.WebSocket = class WebSocket extends EventTarget {
        static CONNECTING = 0;
        static OPEN = 1;
        static CLOSING = 2;
        static CLOSED = 3;
        constructor(url, protocols) {
            super();
            this.url = url;
            this.readyState = WebSocket.CONNECTING;
            this.onopen = null;
            this.onmessage = null;
            this.onclose = null;
            this.onerror = null;
            this._wsId = -1;

            // Connect asynchronously
            ops.op_ws_connect(String(url)).then((result) => {
                if (result.ok) {
                    this._wsId = result.id;
                    this.readyState = WebSocket.OPEN;
                    if (this.onopen) this.onopen(new Event("open"));
                    // Start receive loop
                    this._pollMessages();
                } else {
                    this.readyState = WebSocket.CLOSED;
                    if (this.onerror) this.onerror(new Event("error"));
                    if (this.onclose) this.onclose(new CloseEvent("close", { code: 1006, reason: result.error }));
                }
            }).catch((e) => {
                this.readyState = WebSocket.CLOSED;
                if (this.onerror) this.onerror(new Event("error"));
            });
        }
        async _pollMessages() {
            while (this.readyState === WebSocket.OPEN && this._wsId >= 0) {
                try {
                    const msg = await ops.op_ws_recv(this._wsId);
                    if (!msg && msg !== "") {
                        // Connection closed
                        this.readyState = WebSocket.CLOSED;
                        if (this.onclose) this.onclose(new CloseEvent("close", { code: 1000 }));
                        break;
                    }
                    if (msg !== "" && this.onmessage) {
                        this.onmessage(new MessageEvent("message", { data: msg }));
                    }
                } catch (e) {
                    this.readyState = WebSocket.CLOSED;
                    if (this.onerror) this.onerror(new Event("error"));
                    break;
                }
            }
        }
        send(data) {
            if (this.readyState === WebSocket.OPEN && this._wsId >= 0) {
                ops.op_ws_send(this._wsId, String(data));
            }
        }
        close(code, reason) {
            if (this._wsId >= 0) {
                ops.op_ws_close(this._wsId);
                this._wsId = -1;
            }
            this.readyState = WebSocket.CLOSED;
            if (this.onclose) this.onclose(new CloseEvent("close", { code: code || 1000, reason: reason || "" }));
        }
        get bufferedAmount() { return 0; }
        get extensions() { return ""; }
        get protocol() { return ""; }
        get binaryType() { return "blob"; }
        set binaryType(v) {}
    };

    // CloseEvent for WebSocket
    if (!globalThis.CloseEvent) {
        globalThis.CloseEvent = class CloseEvent extends Event {
            constructor(type, options = {}) {
                super(type, options);
                this.code = options.code || 1000;
                this.reason = options.reason || "";
                this.wasClean = options.wasClean !== undefined ? options.wasClean : true;
            }
        };
    }

    // --- history — prototype-backed ---
    const _historyStack = [{ state: null, title: "", url: globalThis.location?.href || "about:blank" }];
    let _historyIndex = 0;
    const _HistoryProto = History.prototype;
    _defProtoGetter(_HistoryProto, 'length', () => _historyStack.length);
    _defProtoGetter(_HistoryProto, 'state', () => _historyStack[_historyIndex]?.state || null);
    _defProtoGetter(_HistoryProto, 'scrollRestoration', () => "auto");
    _defProtoMethod(_HistoryProto, 'pushState', function pushState(state, title, url) {
        _historyStack.splice(_historyIndex + 1);
        _historyStack.push({ state, title, url: url || "" });
        _historyIndex = _historyStack.length - 1;
    });
    _defProtoMethod(_HistoryProto, 'replaceState', function replaceState(state, title, url) {
        _historyStack[_historyIndex] = { state, title, url: url || "" };
    });
    _defProtoMethod(_HistoryProto, 'back', function back() { if (_historyIndex > 0) _historyIndex--; });
    _defProtoMethod(_HistoryProto, 'forward', function forward() { if (_historyIndex < _historyStack.length - 1) _historyIndex++; });
    _defProtoMethod(_HistoryProto, 'go', function go(delta) {
        const idx = _historyIndex + (delta || 0);
        if (idx >= 0 && idx < _historyStack.length) _historyIndex = idx;
    });
    Object.defineProperty(_HistoryProto, Symbol.toStringTag, { value: "History", configurable: true });
    globalThis.history = Object.create(_HistoryProto);

    // =========================================================
    // matchMedia — covers the 12 standard CSS Media Queries Level 5
    // features that detection libraries probe. Profile-driven defaults
    // (light theme, fine pointer, hover-capable) for the desktop
    // chrome_130_* presets. Returns a real `class MediaQueryList
    // extends EventTarget` instead of a plain object literal so
    // `mql instanceof MediaQueryList` and
    // `Object.prototype.toString.call(mql) === "[object MediaQueryList]"`
    // both hold (the prior shim failed both probes).
    //
    // Supported features (matches Chrome 130+ on macOS desktop):
    //   prefers-color-scheme  : light | dark | no-preference
    //   prefers-reduced-motion: reduce | no-preference
    //   prefers-reduced-data  : reduce | no-preference
    //   prefers-reduced-transparency: reduce | no-preference
    //   prefers-contrast      : more | less | custom | no-preference
    //   inverted-colors       : inverted | none
    //   forced-colors         : active | none
    //   pointer / any-pointer : none | coarse | fine
    //   hover  / any-hover    : none | hover
    //   color  / color-gamut  / monochrome / dynamic-range
    //   orientation           : landscape | portrait
    //   resolution            : matching dppx
    //   width / height / device-width / device-height (min/max/exact)
    //   aspect-ratio / device-aspect-ratio
    //   display-mode          : browser | standalone | fullscreen | minimal-ui
    //   update / overflow-block / overflow-inline
    //   scripting / scan / grid
    // =========================================================
    {
        const _profileFeature = (key, fallback) => {
            try { return _p(key, fallback); } catch (_e) { return fallback; }
        };

        // Feature -> (value-string -> bool) for the chosen profile defaults.
        const _featureValue = (name) => {
            // Honor profile fields when they exist; else use desktop default.
            switch (name) {
                case "prefers-color-scheme":
                    return _profileFeature("prefers_color_scheme", "light");
                case "prefers-reduced-motion": return "no-preference";
                case "prefers-reduced-data": return "no-preference";
                case "prefers-reduced-transparency": return "no-preference";
                case "prefers-contrast": return "no-preference";
                case "inverted-colors": return "none";
                case "forced-colors": return "none";
                case "pointer":
                case "any-pointer":
                    return _profileFeature("pointer_type", "fine");
                case "hover":
                case "any-hover":
                    return _profileFeature("hover_capability", "hover");
                case "display-mode": return "browser";
                case "update": return "fast";
                case "overflow-block": return "scroll";
                case "overflow-inline": return "scroll";
                case "scripting": return "enabled";
                case "scan": return "progressive";
                case "grid": return "0";
                case "color-gamut":
                    // Per CreepJS / FingerprintJS Pro inconsistency probe:
                    // real macOS/iPhone Chrome reports "p3" (wide gamut);
                    // Win/Linux/Android typically report "srgb". Profile-
                    // driven default with srgb fallback.
                    return _profileFeature("color_gamut", "srgb");
                case "dynamic-range": return "standard";
                case "orientation": return _orientationValue();
                default: return null;
            }
        };

        // Numeric features
        const _numericFeature = (name) => {
            switch (name) {
                case "width":
                case "device-width":
                    return _pInt("inner_width", 1920);
                case "height":
                case "device-height":
                    return _pInt("inner_height", 1080);
                case "color":
                    // Bits per color channel; Chrome reports 8.
                    return 8;
                case "monochrome":
                    return 0;
                case "resolution":
                    // Reported as dppx; matches devicePixelRatio.
                    return _pFloat("device_pixel_ratio", 1);
                case "device-pixel-ratio":
                    return _pFloat("device_pixel_ratio", 1);
                case "aspect-ratio":
                case "device-aspect-ratio": {
                    const w = _pInt("inner_width", 1920);
                    const h = _pInt("inner_height", 1080);
                    return h > 0 ? w / h : 16 / 9;
                }
                default: return null;
            }
        };

        // Orientation is enum-valued ("landscape"/"portrait"), so it lives
        // in the enumerated-feature path even though the source is a
        // numeric comparison. Moved out of _numericFeature to avoid
        // tripping the `typeof num === "number"` check below.
        const _orientationValue = () => {
            const w = _pInt("inner_width", 1920);
            const h = _pInt("inner_height", 1080);
            return w >= h ? "landscape" : "portrait";
        };

        // Parse a single feature predicate like
        //   "prefers-color-scheme: light"
        //   "min-width: 1024px"
        //   "(pointer)"  (existence — true if any value is supported)
        const _evalSingle = (feat) => {
            feat = feat.trim().toLowerCase();
            if (!feat) return false;

            // (feature) without value → "is this feature supported with any
            // non-none value?"
            if (!feat.includes(":")) {
                const numeric = _numericFeature(feat);
                if (numeric !== null) {
                    if (typeof numeric === "number") return numeric > 0;
                    return true;
                }
                const enumVal = _featureValue(feat);
                if (enumVal !== null) return enumVal !== "none" && enumVal !== "no-preference" && enumVal !== "0";
                return false;
            }

            // "feature: value" — split, trim
            const colonIdx = feat.indexOf(":");
            let name = feat.slice(0, colonIdx).trim();
            const valueStr = feat.slice(colonIdx + 1).trim();

            // Range prefixes — min-* / max-*.
            let cmp = "eq";
            if (name.startsWith("min-")) { cmp = "min"; name = name.slice(4); }
            else if (name.startsWith("max-")) { cmp = "max"; name = name.slice(4); }

            // Numeric features (width / height / resolution / aspect-ratio)
            const num = _numericFeature(name);
            if (num !== null && typeof num === "number") {
                // Parse value: "1024px" / "1.5dppx" / "16/9" / "2"
                let target;
                if (valueStr.endsWith("px")) target = parseFloat(valueStr);
                else if (valueStr.endsWith("dppx")) target = parseFloat(valueStr);
                else if (valueStr.endsWith("dpi")) target = parseFloat(valueStr) / 96;
                else if (valueStr.includes("/")) {
                    const [a, b] = valueStr.split("/").map(parseFloat);
                    target = b > 0 ? a / b : NaN;
                }
                else target = parseFloat(valueStr);
                if (Number.isNaN(target)) return false;
                if (cmp === "min") return num >= target;
                if (cmp === "max") return num <= target;
                return Math.abs(num - target) < 1e-6;
            }

            // Enumerated features
            const enumVal = _featureValue(name);
            if (enumVal !== null) return enumVal === valueStr;

            return false;
        };

        // Evaluate a full media query string. Supports comma-separated
        // alternatives, `and`, `not`, `only`, parens.
        const _evalQuery = (query) => {
            if (typeof query !== "string") return false;
            const q = query.trim().toLowerCase();
            if (!q || q === "all" || q === "screen") return true;
            // Comma = OR.
            if (q.includes(",")) return q.split(",").some(_evalQuery);
            // Strip leading "only " — same semantics as the bare query.
            const stripped = q.startsWith("only ") ? q.slice(5).trim() : q;
            // Strip leading "not " — invert.
            if (stripped.startsWith("not ")) return !_evalQuery(stripped.slice(4));
            // Match "(features) and (more)" — split on AND.
            const tokens = stripped.split(/\s+and\s+/);
            return tokens.every(tok => {
                tok = tok.trim();
                // Bare media type like "screen" / "print" — accept screen.
                if (tok === "screen" || tok === "all") return true;
                if (tok === "print") return false;
                // Strip parens.
                if (tok.startsWith("(") && tok.endsWith(")")) tok = tok.slice(1, -1);
                return _evalSingle(tok);
            });
        };

        class MediaQueryList extends EventTarget {
            constructor(query) {
                super();
                this._media = String(query || "");
                this._matches = _evalQuery(this._media);
                this._onchange = null;
            }
            get matches() { return this._matches; }
            get media() { return this._media; }
            get onchange() { return this._onchange; }
            set onchange(v) { this._onchange = (typeof v === "function") ? v : null; }
            // Deprecated aliases — kept for legacy compat (Safari, etc.).
            addListener(cb) { try { this.addEventListener("change", cb); } catch(_) {} }
            removeListener(cb) { try { this.removeEventListener("change", cb); } catch(_) {} }
        }
        Object.defineProperty(MediaQueryList.prototype, Symbol.toStringTag, {
            value: "MediaQueryList", configurable: true,
        });
        globalThis.MediaQueryList = MediaQueryList;

        globalThis.matchMedia = ({
            matchMedia(query) {
                return new MediaQueryList(query);
            }
        }).matchMedia;
        if (typeof _maskFunction === "function") {
            _maskFunction(globalThis.matchMedia, "matchMedia");
        }
        }

        // --- window.open/close/postMessage ---
        globalThis.open = ({ open(url, target, features) { return null; } }).open;
        _maskFunction(globalThis.open, "open");

        globalThis.close = ({ close() {} }).close;
        _maskFunction(globalThis.close, "close");

        globalThis.postMessage = ({
        postMessage(message, targetOrigin, transfer) {
            // Use structuredClone if available to match browser behavior.
            // If not available (e.g. during very early bootstrap), fall back to reference.
            let cloned = message;
            try {
                if (typeof globalThis.structuredClone === 'function') {
                    cloned = globalThis.structuredClone(message, { transfer });
                }
            } catch (e) {
                // DataCloneError — propagate as-is (matches Chrome)
                throw e;
            }
            // Fire message event asynchronously
            Promise.resolve().then(() => {
                const event = new MessageEvent("message", {
                    data: cloned,
                    origin: targetOrigin || globalThis.location?.origin || "",
                });
                globalThis.dispatchEvent(event);
            });
        }
        }).postMessage;
        _maskFunction(globalThis.postMessage, "postMessage");

        globalThis.stop = ({ stop() {} }).stop;
        _maskFunction(globalThis.stop, "stop");

        globalThis.print = ({ print() {} }).print;
        _maskFunction(globalThis.print, "print");

        globalThis.confirm = ({ confirm(msg) { return true; } }).confirm;
        _maskFunction(globalThis.confirm, "confirm");

        globalThis.alert = ({ alert(msg) {} }).alert;
        _maskFunction(globalThis.alert, "alert");

        globalThis.prompt = ({ prompt(msg, def) { return def || null; } }).prompt;
        _maskFunction(globalThis.prompt, "prompt");


    // --- AbortController / AbortSignal ---
    class AbortSignal {
        constructor() {
            this.aborted = false;
            this.reason = undefined;
            this._listeners = [];
        }
        addEventListener(type, cb) {
            if (type === "abort") this._listeners.push(cb);
        }
        removeEventListener(type, cb) {
            if (type === "abort") this._listeners = this._listeners.filter(l => l !== cb);
        }
        throwIfAborted() {
            if (this.aborted) throw this.reason;
        }
        static abort(reason) {
            const sig = new AbortSignal();
            sig.aborted = true;
            sig.reason = reason || new DOMException("The operation was aborted.", "AbortError");
            return sig;
        }
        static timeout(ms) {
            const sig = new AbortSignal();
            setTimeout(() => {
                sig.aborted = true;
                sig.reason = new DOMException("The operation timed out.", "TimeoutError");
                for (const cb of sig._listeners) cb();
            }, ms);
            return sig;
        }
    }

    class AbortController {
        constructor() {
            this.signal = new AbortSignal();
        }
        abort(reason) {
            if (this.signal.aborted) return;
            this.signal.aborted = true;
            this.signal.reason = reason || new DOMException("The operation was aborted.", "AbortError");
            for (const cb of this.signal._listeners) cb();
        }
    }

    globalThis.AbortController = AbortController;
    globalThis.AbortSignal = AbortSignal;

    // --- DOMException ---
    if (!globalThis.DOMException) {
        globalThis.DOMException = class DOMException extends Error {
            constructor(message, name) {
                super(message);
                this.name = name || "Error";
                this.code = 0;
            }
        };
    }

    // --- URLSearchParams ---
    if (!globalThis.URLSearchParams) {
        globalThis.URLSearchParams = class URLSearchParams {
            #params;
            constructor(init) {
                this.#params = [];
                if (typeof init === "string") {
                    const s = init.startsWith("?") ? init.slice(1) : init;
                    for (const pair of s.split("&")) {
                        const [k, ...v] = pair.split("=");
                        if (k) this.#params.push([decodeURIComponent(k), decodeURIComponent(v.join("="))]);
                    }
                } else if (init && typeof init === "object") {
                    for (const [k, v] of Object.entries(init)) {
                        this.#params.push([String(k), String(v)]);
                    }
                }
            }
            get(name) { const p = this.#params.find(([k]) => k === name); return p ? p[1] : null; }
            getAll(name) { return this.#params.filter(([k]) => k === name).map(([, v]) => v); }
            has(name) { return this.#params.some(([k]) => k === name); }
            set(name, value) {
                let found = false;
                this.#params = this.#params.filter(([k]) => { if (k === name && !found) { found = true; return true; } return k !== name; });
                if (found) { this.#params.find(([k]) => k === name)[1] = String(value); }
                else { this.#params.push([name, String(value)]); }
            }
            append(name, value) { this.#params.push([String(name), String(value)]); }
            delete(name) { this.#params = this.#params.filter(([k]) => k !== name); }
            toString() { return this.#params.map(([k, v]) => encodeURIComponent(k) + "=" + encodeURIComponent(v)).join("&"); }
            forEach(cb, thisArg) { for (const [k, v] of this.#params) cb.call(thisArg, v, k, this); }
            keys() { return this.#params.map(([k]) => k)[Symbol.iterator](); }
            values() { return this.#params.map(([, v]) => v)[Symbol.iterator](); }
            entries() { return this.#params[Symbol.iterator](); }
            [Symbol.iterator]() { return this.entries(); }
            get size() { return this.#params.length; }
        };
    }

    // --- URL ---
    if (!globalThis.URL) {
        globalThis.URL = class URL {
            constructor(url, base) {
                let full = String(url);
                if (base && !full.match(/^[a-z]+:\/\//i)) {
                    const b = String(base);
                    if (full.startsWith('//')) {
                        const proto = b.match(/^([a-z]+:)/i);
                        full = (proto ? proto[1] : 'https:') + full;
                    } else if (full.startsWith('/')) {
                        const m = b.match(/^([a-z]+:\/\/[^/]+)/i);
                        full = m ? m[1] + full : full;
                    } else {
                        full = b.replace(/[^/]*$/, '') + full;
                    }
                }
                const m = full.match(/^([a-z]+):\/\/([^/:]+)(?::(\d+))?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/i);
                if (m) {
                    this.protocol = m[1].toLowerCase() + ':';
                    this.hostname = m[2];
                    this.port = m[3] || '';
                    this.pathname = m[4] || '/';
                    this.search = m[5] || '';
                    this.hash = m[6] || '';
                    this.host = this.port ? this.hostname + ':' + this.port : this.hostname;
                    this.origin = this.protocol + '//' + this.host;
                    this.href = this.origin + this.pathname + this.search + this.hash;
                } else {
                    this.href = full;
                    this.protocol = ''; this.hostname = ''; this.port = '';
                    this.pathname = full; this.search = ''; this.hash = '';
                    this.host = ''; this.origin = 'null';
                }
                this.username = ''; this.password = '';
                this.searchParams = new URLSearchParams(this.search);
            }
            toString() { return this.href; }
            toJSON() { return this.href; }
            static createObjectURL(obj) {
                // Allocate a stable blob URL backed by the Rust BlobRegistry
                // so Workers (and blob: fetches) can resolve to the bytes.
                const u = 'blob:' + (globalThis.location && globalThis.location.origin || 'null') + '/' + _randomUUID();
                let data;
                let contentType = '';
                if (obj && obj._data instanceof Uint8Array) {
                    data = obj._data;
                    // Preserve Blob.type so fetch(blob:URL) can echo it
                    // back as the Response's content-type header.
                    contentType = String(obj.type || '');
                } else if (obj instanceof Uint8Array) {
                    data = obj;
                } else if (typeof obj === 'string') {
                    data = new TextEncoder().encode(obj);
                } else {
                    data = new Uint8Array();
                }
                try { ops.op_blob_register(u, data, contentType); } catch (e) {}
                return u;
            }
            static revokeObjectURL(url) {
                try { ops.op_blob_revoke(url); } catch (e) {}
            }
        };
    }
    // Small helper for URL allocation
    function _randomUUID() {
        if (globalThis.crypto && typeof globalThis.crypto.randomUUID === 'function') {
            try { return globalThis.crypto.randomUUID(); } catch (e) {}
        }
        return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function (c) {
            const r = Math.random() * 16 | 0;
            const v = c === 'x' ? r : (r & 0x3 | 0x8);
            return v.toString(16);
        });
    }

    // --- FormData ---
    globalThis.FormData = class FormData {
        #data;
        constructor() { this.#data = []; }
        append(name, value) { this.#data.push([String(name), value]); }
        delete(name) { this.#data = this.#data.filter(([k]) => k !== name); }
        get(name) { const p = this.#data.find(([k]) => k === name); return p ? p[1] : null; }
        getAll(name) { return this.#data.filter(([k]) => k === name).map(([, v]) => v); }
        has(name) { return this.#data.some(([k]) => k === name); }
        set(name, value) { this.delete(name); this.append(name, value); }
        forEach(cb, thisArg) { for (const [k, v] of this.#data) cb.call(thisArg, v, k, this); }
        keys() { return this.#data.map(([k]) => k)[Symbol.iterator](); }
        values() { return this.#data.map(([, v]) => v)[Symbol.iterator](); }
        entries() { return this.#data[Symbol.iterator](); }
        [Symbol.iterator]() { return this.entries(); }
    };

    // --- customElements registry with lifecycle ---
    const _customElementsRegistry = new Map();
    const _whenDefinedPromises = new Map(); // name -> { promise, resolve }

    function _tryCallLifecycle(el, name, ...args) {
        try { if (typeof el[name] === "function") el[name](...args); } catch (e) { console.error(e); }
    }

    function _upgradeElement(el, entry) {
        if (el._ceUpgraded) return;
        el._ceUpgraded = true;
        // Set prototype to the custom element class
        Object.setPrototypeOf(el, entry.constructor.prototype);
        try { entry.constructor.call(el); } catch (e) { console.error(e); }
    }

    // CustomElementRegistry — prototype-backed, matches real Chrome class name.
    class CustomElementRegistry {}
    globalThis.CustomElementRegistry = CustomElementRegistry;
    const _CERProto = CustomElementRegistry.prototype;
    _defProtoMethod(_CERProto, 'define', function define(name, constructor, options) {
        const lowerName = name.toLowerCase();
        _customElementsRegistry.set(lowerName, { constructor, options });
        const pending = _whenDefinedPromises.get(lowerName);
        if (pending) { pending.resolve(constructor); _whenDefinedPromises.delete(lowerName); }
        try {
            const existing = document.querySelectorAll(lowerName);
            for (let i = 0; i < existing.length; i++) {
                const el = existing[i];
                _upgradeElement(el, { constructor });
                _tryCallLifecycle(el, "connectedCallback");
            }
        } catch (e) {}
    });
    _defProtoMethod(_CERProto, 'get', function get(name) {
        const entry = _customElementsRegistry.get(name.toLowerCase());
        return entry ? entry.constructor : undefined;
    });
    _defProtoMethod(_CERProto, 'whenDefined', function whenDefined(name) {
        const lowerName = name.toLowerCase();
        if (_customElementsRegistry.has(lowerName)) return Promise.resolve(_customElementsRegistry.get(lowerName).constructor);
        if (!_whenDefinedPromises.has(lowerName)) {
            let resolve;
            const promise = new Promise(r => { resolve = r; });
            _whenDefinedPromises.set(lowerName, { promise, resolve });
        }
        return _whenDefinedPromises.get(lowerName).promise;
    });
    _defProtoMethod(_CERProto, 'upgrade', function upgrade(root) {
        for (const [name, entry] of _customElementsRegistry) {
            try {
                const els = root.querySelectorAll(name);
                for (let i = 0; i < els.length; i++) _upgradeElement(els[i], entry);
            } catch (e) {}
        }
    });
    Object.defineProperty(_CERProto, Symbol.toStringTag, { value: "CustomElementRegistry", configurable: true });
    globalThis.customElements = Object.create(_CERProto);

    // Store reference for DOM hooks
    globalThis._customElementsRegistry = _customElementsRegistry;

    // --- Blob ---
    if (!globalThis.Blob) {
        const _encoder = new TextEncoder();
        const _decoder = new TextDecoder();

        globalThis.Blob = class Blob {
            constructor(parts = [], options = {}) {
                this.type = options.type || "";
                // Convert all parts to Uint8Array and concatenate
                const arrays = parts.map(p => {
                    if (typeof p === "string") return _encoder.encode(p);
                    if (p instanceof ArrayBuffer) return new Uint8Array(p);
                    if (ArrayBuffer.isView(p)) return new Uint8Array(p.buffer, p.byteOffset, p.byteLength);
                    if (p instanceof Blob) return p._data;
                    return _encoder.encode(String(p));
                });
                const totalLen = arrays.reduce((s, a) => s + a.byteLength, 0);
                const merged = new Uint8Array(totalLen);
                let offset = 0;
                for (const a of arrays) { merged.set(a, offset); offset += a.byteLength; }
                this._data = merged;
                this.size = totalLen;
            }
            text() { return Promise.resolve(_decoder.decode(this._data)); }
            arrayBuffer() {
                const buf = this._data.buffer.slice(
                    this._data.byteOffset,
                    this._data.byteOffset + this._data.byteLength
                );
                return Promise.resolve(buf);
            }
            slice(start = 0, end = this.size, type = "") {
                const sliced = this._data.slice(start, end);
                const b = new Blob([], { type });
                b._data = sliced;
                b.size = sliced.byteLength;
                return b;
            }
        };
    }

    // --- OffscreenCanvas ---
    // Chrome since 69 ships OffscreenCanvas as a global. Sensor VMs detect
    // its absence as a "not really Chrome" signal. We expose a minimal class
    // that satisfies constructor checks and typeof checks; `getContext` is a
    // no-op stub that returns null (the sensor VM falls through to fallback
    // paths when null is returned).
    if (!globalThis.OffscreenCanvas) {
        class OffscreenCanvas {
            constructor(width, height) {
                this.width = width | 0;
                this.height = height | 0;
            }
            getContext(_type, _opts) { return null; }
            transferToImageBitmap() {
                // Minimal ImageBitmap stub — not callable for real rendering.
                return { width: this.width, height: this.height, close() {} };
            }
            convertToBlob(options) {
                return Promise.resolve(new Blob([], { type: (options && options.type) || "image/png" }));
            }
        }
        Object.defineProperty(OffscreenCanvas.prototype, Symbol.toStringTag, {
            value: "OffscreenCanvas", configurable: true,
        });
        globalThis.OffscreenCanvas = OffscreenCanvas;
    }

    // --- File (extends Blob) ---
    if (!globalThis.File) {
        globalThis.File = class File extends Blob {
            constructor(parts, name, options = {}) {
                super(parts, options);
                this.name = name;
                this.lastModified = options.lastModified || Date.now();
            }
        };
    }

    // --- IndexedDB ---
    //
    // In-memory implementation backed by JS Maps plus a sorted keys
    // array per store for ordered cursor iteration. Spec compliance
    // is scoped to the fingerprint-probe subset: open → upgrade →
    // transaction → put/get/delete/clear/count/getAll/openCursor,
    // IDBKeyRange (bound/only/lower/upper), version upgrade lifecycle,
    // deep-clone isolation on put/get so stored values don't leak
    // mutations back to the caller. Persistent storage is NOT
    // implemented — every page load starts with an empty DB registry,
    // which is fine for scraping (no cross-session state).
    if (!globalThis.indexedDB || !globalThis.indexedDB._browserOxideReal) {
        // Process-global registry so the same origin's opens get the
        // same underlying state within a page load.
        const _dbRegistry = new Map();

        // Deep-clone via structuredClone when available, else a JSON
        // fallback. structuredClone isn't strictly installed yet at
        // IDB definition time in the bootstrap order, so we pick it
        // up lazily.
        function _clone(v) {
            if (typeof globalThis.structuredClone === "function") {
                try { return globalThis.structuredClone(v); } catch (_e) {}
            }
            try { return JSON.parse(JSON.stringify(v)); } catch (_e) { return v; }
        }

        // Compare keys per IndexedDB key ordering:
        // number < Date < string < array (with element-wise comparison).
        // Simplified to number/string here — sufficient for fingerprint
        // probes which key by number or small string.
        function _keyCmp(a, b) {
            if (typeof a === typeof b) {
                if (a < b) return -1;
                if (a > b) return 1;
                return 0;
            }
            // Cross-type: number < string.
            if (typeof a === "number") return -1;
            if (typeof b === "number") return 1;
            return 0;
        }

        function _extractKey(value, keyPath) {
            if (!keyPath) return undefined;
            if (Array.isArray(keyPath)) {
                return keyPath.map((p) => value?.[p]);
            }
            const parts = String(keyPath).split(".");
            let cur = value;
            for (const p of parts) {
                if (cur == null) return undefined;
                cur = cur[p];
            }
            return cur;
        }

        class IDBRequest {
            constructor(source) {
                this.result = undefined;
                this.error = null;
                this.source = source || null;
                this.transaction = null;
                this.readyState = "pending";
                this.onsuccess = null;
                this.onerror = null;
                this._listeners = { success: [], error: [] };
            }
            addEventListener(type, listener) {
                if (!this._listeners[type]) this._listeners[type] = [];
                this._listeners[type].push(listener);
            }
            removeEventListener(type, listener) {
                const arr = this._listeners[type];
                if (!arr) return;
                const i = arr.indexOf(listener);
                if (i >= 0) arr.splice(i, 1);
            }
            _fireSuccess() {
                this.readyState = "done";
                queueMicrotask(() => {
                    const ev = { target: this, type: "success" };
                    if (typeof this.onsuccess === "function") {
                        try { this.onsuccess(ev); } catch (_e) {}
                    }
                    for (const l of (this._listeners.success || []).slice()) {
                        try { l.call(this, ev); } catch (_e) {}
                    }
                });
            }
            _fireError(err) {
                this.readyState = "done";
                this.error = err;
                queueMicrotask(() => {
                    const ev = { target: this, type: "error" };
                    if (typeof this.onerror === "function") {
                        try { this.onerror(ev); } catch (_e) {}
                    }
                    for (const l of (this._listeners.error || []).slice()) {
                        try { l.call(this, ev); } catch (_e) {}
                    }
                });
            }
        }

        class IDBOpenDBRequest extends IDBRequest {
            constructor() {
                super();
                this.onupgradeneeded = null;
                this.onblocked = null;
            }
        }

        class IDBKeyRange {
            constructor(lower, upper, lowerOpen, upperOpen) {
                this.lower = lower;
                this.upper = upper;
                this.lowerOpen = !!lowerOpen;
                this.upperOpen = !!upperOpen;
            }
            includes(key) {
                if (this.lower !== undefined) {
                    const c = _keyCmp(key, this.lower);
                    if (c < 0) return false;
                    if (c === 0 && this.lowerOpen) return false;
                }
                if (this.upper !== undefined) {
                    const c = _keyCmp(key, this.upper);
                    if (c > 0) return false;
                    if (c === 0 && this.upperOpen) return false;
                }
                return true;
            }
            static bound(lower, upper, lowerOpen = false, upperOpen = false) {
                return new IDBKeyRange(lower, upper, lowerOpen, upperOpen);
            }
            static only(value) {
                return new IDBKeyRange(value, value, false, false);
            }
            static lowerBound(lower, open = false) {
                return new IDBKeyRange(lower, undefined, open, false);
            }
            static upperBound(upper, open = false) {
                return new IDBKeyRange(undefined, upper, false, open);
            }
        }

        class IDBCursor {
            constructor(store, range, direction) {
                this.source = store;
                this.direction = direction || "next";
                this._store = store;
                this._range = range;
                // Snapshot the store's key order at cursor creation.
                this._keys = store._sortedKeys().filter((k) =>
                    !range || range.includes(k)
                );
                if (this.direction === "prev" || this.direction === "prevunique") {
                    this._keys.reverse();
                }
                this._idx = -1;
                this.key = undefined;
                this.primaryKey = undefined;
                this.value = undefined;
            }
            _advanceTo(idx) {
                this._idx = idx;
                if (idx < this._keys.length) {
                    this.key = this._keys[idx];
                    this.primaryKey = this.key;
                    this.value = _clone(this._store._data.get(this.key));
                } else {
                    this.key = undefined;
                    this.primaryKey = undefined;
                    this.value = undefined;
                }
            }
            continue(_targetKey) {
                // Advance the request that produced this cursor.
                this._step();
            }
            advance(count) {
                for (let i = 0; i < count; i++) this._step();
            }
            _step() {
                this._advanceTo(this._idx + 1);
                if (this._request) {
                    const done = this._idx >= this._keys.length;
                    this._request.result = done ? null : this;
                    this._request._fireSuccess();
                }
            }
        }

        class IDBObjectStore {
            constructor(name, options, transaction) {
                this.name = name;
                this.keyPath = (options && options.keyPath) || null;
                this.indexNames = [];
                this.autoIncrement = !!(options && options.autoIncrement);
                this.transaction = transaction || null;
                this._data = new Map();
                this._nextKey = 1;
            }
            _sortedKeys() {
                return [...this._data.keys()].sort(_keyCmp);
            }
            _resolveKey(value, explicitKey) {
                if (this.keyPath) {
                    const extracted = _extractKey(value, this.keyPath);
                    if (extracted !== undefined) return extracted;
                    if (this.autoIncrement) return this._nextKey++;
                    return undefined;
                }
                if (explicitKey !== undefined) return explicitKey;
                if (this.autoIncrement) return this._nextKey++;
                return undefined;
            }
            put(value, key) {
                const r = new IDBRequest(this);
                const resolvedKey = this._resolveKey(value, key);
                if (resolvedKey === undefined) {
                    r._fireError(new Error("DataError: no key"));
                    return r;
                }
                this._data.set(resolvedKey, _clone(value));
                if (this.autoIncrement && typeof resolvedKey === "number" && resolvedKey >= this._nextKey) {
                    this._nextKey = resolvedKey + 1;
                }
                r.result = resolvedKey;
                r._fireSuccess();
                return r;
            }
            add(value, key) {
                const r = new IDBRequest(this);
                const resolvedKey = this._resolveKey(value, key);
                if (resolvedKey === undefined) {
                    r._fireError(new Error("DataError: no key"));
                    return r;
                }
                if (this._data.has(resolvedKey)) {
                    r._fireError(new Error("ConstraintError: key exists"));
                    return r;
                }
                this._data.set(resolvedKey, _clone(value));
                r.result = resolvedKey;
                r._fireSuccess();
                return r;
            }
            get(key) {
                const r = new IDBRequest(this);
                if (key instanceof IDBKeyRange) {
                    for (const k of this._sortedKeys()) {
                        if (key.includes(k)) {
                            r.result = _clone(this._data.get(k));
                            r._fireSuccess();
                            return r;
                        }
                    }
                    r.result = undefined;
                } else {
                    const v = this._data.get(key);
                    r.result = v === undefined ? undefined : _clone(v);
                }
                r._fireSuccess();
                return r;
            }
            getAll(queryOrRange, count) {
                const r = new IDBRequest(this);
                const out = [];
                const limit = count ?? Infinity;
                for (const k of this._sortedKeys()) {
                    if (out.length >= limit) break;
                    if (queryOrRange == null) {
                        out.push(_clone(this._data.get(k)));
                    } else if (queryOrRange instanceof IDBKeyRange) {
                        if (queryOrRange.includes(k)) out.push(_clone(this._data.get(k)));
                    } else if (_keyCmp(k, queryOrRange) === 0) {
                        out.push(_clone(this._data.get(k)));
                    }
                }
                r.result = out;
                r._fireSuccess();
                return r;
            }
            getAllKeys(queryOrRange, count) {
                const r = new IDBRequest(this);
                const out = [];
                const limit = count ?? Infinity;
                for (const k of this._sortedKeys()) {
                    if (out.length >= limit) break;
                    if (queryOrRange == null) {
                        out.push(k);
                    } else if (queryOrRange instanceof IDBKeyRange) {
                        if (queryOrRange.includes(k)) out.push(k);
                    } else if (_keyCmp(k, queryOrRange) === 0) {
                        out.push(k);
                    }
                }
                r.result = out;
                r._fireSuccess();
                return r;
            }
            delete(key) {
                const r = new IDBRequest(this);
                if (key instanceof IDBKeyRange) {
                    for (const k of this._sortedKeys()) {
                        if (key.includes(k)) this._data.delete(k);
                    }
                } else {
                    this._data.delete(key);
                }
                r._fireSuccess();
                return r;
            }
            clear() {
                const r = new IDBRequest(this);
                this._data.clear();
                r._fireSuccess();
                return r;
            }
            count(query) {
                const r = new IDBRequest(this);
                if (query == null) {
                    r.result = this._data.size;
                } else if (query instanceof IDBKeyRange) {
                    let n = 0;
                    for (const k of this._data.keys()) {
                        if (query.includes(k)) n++;
                    }
                    r.result = n;
                } else {
                    r.result = this._data.has(query) ? 1 : 0;
                }
                r._fireSuccess();
                return r;
            }
            openCursor(range, direction) {
                const r = new IDBRequest(this);
                let rangeObj = null;
                if (range instanceof IDBKeyRange) rangeObj = range;
                else if (range != null) rangeObj = IDBKeyRange.only(range);
                const cursor = new IDBCursor(this, rangeObj, direction);
                cursor._request = r;
                // First step — null if no keys, cursor otherwise.
                queueMicrotask(() => cursor._step());
                return r;
            }
            createIndex(name, _keyPath, _options) {
                if (!this.indexNames.includes(name)) this.indexNames.push(name);
                return {
                    name,
                    get: (k) => this.get(k),
                    getAll: (q, c) => this.getAll(q, c),
                };
            }
            index(name) {
                return {
                    name,
                    get: (k) => this.get(k),
                    getAll: (q, c) => this.getAll(q, c),
                };
            }
        }

        class IDBTransaction {
            constructor(db, storeNames, mode) {
                this._db = db;
                this._storeNames = storeNames;
                this.mode = mode || "readonly";
                this.db = db;
                this.error = null;
                this.oncomplete = null;
                this.onerror = null;
                this.onabort = null;
                this._listeners = { complete: [], error: [], abort: [] };
                this._active = true;
                // Fire oncomplete on the next microtask after the
                // caller's sync op chain finishes (matches Chrome).
                queueMicrotask(() => this._complete());
            }
            objectStore(name) {
                if (!this._db._stores.has(name)) {
                    // Transactions can't create stores — spec throws
                    // NotFoundError. The one exception is an
                    // upgrade transaction, where createObjectStore
                    // was called synchronously on the db.
                    throw new Error("NotFoundError: store " + name);
                }
                const store = this._db._stores.get(name);
                store.transaction = this;
                return store;
            }
            commit() {
                this._complete();
            }
            abort() {
                this._active = false;
                const ev = { target: this, type: "abort" };
                if (typeof this.onabort === "function") {
                    queueMicrotask(() => { try { this.onabort(ev); } catch (_e) {} });
                }
            }
            addEventListener(type, listener) {
                if (!this._listeners[type]) this._listeners[type] = [];
                this._listeners[type].push(listener);
            }
            removeEventListener(type, listener) {
                const arr = this._listeners[type];
                if (!arr) return;
                const i = arr.indexOf(listener);
                if (i >= 0) arr.splice(i, 1);
            }
            _complete() {
                if (!this._active) return;
                this._active = false;
                const ev = { target: this, type: "complete" };
                if (typeof this.oncomplete === "function") {
                    try { this.oncomplete(ev); } catch (_e) {}
                }
                for (const l of (this._listeners.complete || []).slice()) {
                    try { l.call(this, ev); } catch (_e) {}
                }
            }
        }

        class IDBDatabase {
            constructor(name, version) {
                this.name = name;
                this.version = version;
                this._stores = new Map();
                this.objectStoreNames = [];
                this.onclose = null;
                this.onversionchange = null;
                this.onabort = null;
                this.onerror = null;
            }
            createObjectStore(name, options) {
                if (this._stores.has(name)) {
                    throw new Error("ConstraintError: store already exists");
                }
                const store = new IDBObjectStore(name, options, null);
                this._stores.set(name, store);
                this.objectStoreNames.push(name);
                return store;
            }
            deleteObjectStore(name) {
                this._stores.delete(name);
                this.objectStoreNames = this.objectStoreNames.filter((n) => n !== name);
            }
            transaction(storeNames, mode) {
                const names = Array.isArray(storeNames) ? storeNames : [storeNames];
                return new IDBTransaction(this, names, mode);
            }
            close() {}
        }

        class IDBFactory {
            constructor() {
                this._browserOxideReal = true;
            }
            open(name, version) {
                const req = new IDBOpenDBRequest();
                const targetVersion = version || 1;
                let db = _dbRegistry.get(name);
                const oldVersion = db ? db.version : 0;
                if (!db) {
                    db = new IDBDatabase(name, targetVersion);
                    _dbRegistry.set(name, db);
                } else if (db.version < targetVersion) {
                    db.version = targetVersion;
                }
                req.result = db;

                // Fire onupgradeneeded before onsuccess if the version
                // jumped. Spec: the upgradeneeded handler runs in an
                // upgrade-mode transaction that commits before success.
                queueMicrotask(() => {
                    if (oldVersion < targetVersion) {
                        const tx = new IDBTransaction(db, [], "versionchange");
                        req.transaction = tx;
                        const ev = {
                            target: req,
                            oldVersion,
                            newVersion: targetVersion,
                            type: "upgradeneeded",
                        };
                        if (typeof req.onupgradeneeded === "function") {
                            try { req.onupgradeneeded(ev); } catch (_e) {}
                        }
                        // Override the auto-commit microtask — we
                        // complete the versionchange tx synchronously
                        // once the handler returns.
                        tx._complete();
                        req.transaction = null;
                    }
                    req._fireSuccess();
                });
                return req;
            }
            deleteDatabase(name) {
                const r = new IDBOpenDBRequest();
                _dbRegistry.delete(name);
                r._fireSuccess();
                return r;
            }
            databases() {
                return Promise.resolve(
                    [..._dbRegistry.entries()].map(([name, db]) => ({
                        name,
                        version: db.version,
                    }))
                );
            }
            cmp(a, b) {
                return _keyCmp(a, b);
            }
        }

        globalThis.indexedDB = new IDBFactory();
        globalThis.IDBFactory = IDBFactory;
        globalThis.IDBDatabase = IDBDatabase;
        globalThis.IDBTransaction = IDBTransaction;
        globalThis.IDBObjectStore = IDBObjectStore;
        globalThis.IDBRequest = IDBRequest;
        globalThis.IDBOpenDBRequest = IDBOpenDBRequest;
        globalThis.IDBKeyRange = IDBKeyRange;
        globalThis.IDBCursor = IDBCursor;
    }

    // ================================================================
    // WebRTC leak prevention — block real IP exposure via ICE candidates
    // ================================================================
    globalThis.RTCDataChannel = class RTCDataChannel extends EventTarget {
        constructor() { super(); this.label = ""; this.readyState = "connecting"; this.onopen = null; this.onmessage = null; this.onerror = null; this.onclose = null; }
        send() {}
        close() {}
    };
    globalThis.RTCPeerConnection = class RTCPeerConnection extends EventTarget {
        constructor(config) {
            super();
            this.localDescription = null;
            this.remoteDescription = null;
            this.signalingState = "stable";
            this.iceConnectionState = "new";
            this.iceGatheringState = "new";
            this.connectionState = "new";
            this.onicecandidate = null;
            this.oniceconnectionstatechange = null;
            this.onsignalingstatechange = null;
            this.ondatachannel = null;
            this.ontrack = null;
            this._channels = [];
        }
        createDataChannel(label, options) {
            const ch = new RTCDataChannel();
            ch.label = label;
            this._channels.push(ch);
            return ch;
        }
        createOffer() {
            return Promise.resolve({ type: "offer", sdp: "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n" });
        }
        createAnswer() {
            return Promise.resolve({ type: "answer", sdp: "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n" });
        }
        setLocalDescription(desc) {
            this.localDescription = desc;
            // Real Chrome (since 2019, mDNS-anonymized) emits an mDNS host
            // candidate followed by `null` to signal gathering complete.
            // Returning ONLY `{candidate: null}` is itself a tell — every
            // legitimate Chrome session yields at least one mDNS host.
            // The `<uuid>.local` form is privacy-preserving (no real IP).
            // CreepJS / FingerprintJS open-source both probe candidate
            // length; one mDNS host closes the parity gap without leaking.
            const _hex = (n) => Math.floor(Math.random() * 16).toString(16);
            const _uuid4 = () => {
                let s = '';
                for (let i = 0; i < 36; i++) {
                    if (i === 8 || i === 13 || i === 18 || i === 23) s += '-';
                    else if (i === 14) s += '4';
                    else if (i === 19) s += (8 + Math.floor(Math.random() * 4)).toString(16);
                    else s += _hex();
                }
                return s;
            };
            const mdnsHost = _uuid4() + '.local';
            const foundation = String(Math.floor(Math.random() * 4_000_000_000));
            const candidate = `candidate:${foundation} 1 udp 2113937151 ${mdnsHost} ${1024 + Math.floor(Math.random() * 60000)} typ host generation 0 network-cost 999`;
            const iceCandidate = new globalThis.RTCIceCandidate({
                candidate, sdpMid: '0', sdpMLineIndex: 0,
            });
            setTimeout(() => {
                if (this.onicecandidate) this.onicecandidate({ candidate: iceCandidate });
                setTimeout(() => {
                    if (this.onicecandidate) this.onicecandidate({ candidate: null });
                    this.iceGatheringState = "complete";
                }, 12);
            }, 8);
            return Promise.resolve();
        }
        setRemoteDescription(desc) { this.remoteDescription = desc; return Promise.resolve(); }
        addIceCandidate(c) { return Promise.resolve(); }
        addTrack() { return { track: null }; }
        addStream() {}
        removeTrack() {}
        getStats() { return Promise.resolve(new Map()); }
        getSenders() { return []; }
        getReceivers() { return []; }
        getTransceivers() { return []; }
        close() {
            this.signalingState = "closed";
            this.iceConnectionState = "closed";
            this.connectionState = "closed";
        }
        addEventListener() {}
        removeEventListener() {}
    };
    globalThis.RTCPeerConnection.generateCertificate = () => Promise.resolve({});
    globalThis.webkitRTCPeerConnection = globalThis.RTCPeerConnection;
    globalThis.RTCSessionDescription = class RTCSessionDescription { constructor(d) { this.type = d?.type; this.sdp = d?.sdp; } };
    globalThis.RTCIceCandidate = class RTCIceCandidate { constructor(c) { this.candidate = c?.candidate || ""; this.sdpMid = c?.sdpMid; this.sdpMLineIndex = c?.sdpMLineIndex; } };

    // ================================================================
    // Font enumeration spoofing — return OS-appropriate fonts
    // ================================================================
    {
        const _osName = _p("os_name", "Linux");
        // Chrome default fonts by OS
        const _fontsByOS = {
            "Windows": ["Arial","Arial Black","Calibri","Cambria","Comic Sans MS","Consolas","Courier New","Georgia","Impact","Lucida Console","Segoe UI","Tahoma","Times New Roman","Trebuchet MS","Verdana"],
            "macOS": ["Arial","Arial Black","Courier New","Georgia","Helvetica","Helvetica Neue","Lucida Grande","Menlo","Monaco","SF Pro","Times New Roman","Trebuchet MS","Verdana"],
            "Linux": ["Arial","Courier New","DejaVu Sans","DejaVu Sans Mono","DejaVu Serif","Liberation Mono","Liberation Sans","Liberation Serif","Noto Sans","Times New Roman","Ubuntu","Verdana"],
        };
        const _fonts = _fontsByOS[_osName] || _fontsByOS["Linux"];
        const _fontSet = new Set(_fonts.map(f => f.toLowerCase()));

        globalThis.FontFace = class FontFace {
            constructor(family, source) { this.family = family; this.status = "loaded"; }
            load() { return Promise.resolve(this); }
        };

        // document.fonts (FontFaceSet) — iterator yields actual FontFace
        // entries (real Chrome's `for (const f of document.fonts)` yields
        // FontFace instances for every loaded face). Pre-W3.7 our iterators
        // returned empty, which is the canonical headless "no faces ever
        // loaded" tell. PLAN identifies as a blind Kasada `ao` candidate.
        if (globalThis.document) {
            // Materialize FontFace instances once for the system font list.
            const _fontFaces = _fonts.map(family => {
                const f = new globalThis.FontFace(family, 'local("' + family + '")');
                f.status = 'loaded';
                f.weight = 'normal';
                f.style = 'normal';
                f.stretch = 'normal';
                f.unicodeRange = 'U+0-10FFFF';
                f.variant = 'normal';
                f.featureSettings = 'normal';
                f.display = 'auto';
                return f;
            });
            Object.defineProperty(globalThis.document, 'fonts', {
                value: {
                    check(font, text) {
                        // Parse font family from CSS font shorthand (e.g. "12px Arial", "bold 14px 'Times New Roman'")
                        // Strip size/weight prefix: everything before the last number+unit
                        const stripped = font.replace(/^[^"']*?\d+(\.\d+)?(px|pt|em|rem|%|vh|vw)\s*/, '');
                        const parts = stripped.split(',');
                        return parts.some(p => {
                            const name = p.replace(/["']/g, '').trim().toLowerCase();
                            return name.length > 0 && _fontSet.has(name);
                        });
                    },
                    ready: Promise.resolve(),
                    status: "loaded",
                    forEach(cb, thisArg) {
                        for (const f of _fontFaces) cb.call(thisArg, f, f, this);
                    },
                    entries() {
                        // FontFaceSet.entries yields [face, face] pairs per spec.
                        const arr = _fontFaces.map(f => [f, f]);
                        return arr[Symbol.iterator]();
                    },
                    keys() { return _fontFaces[Symbol.iterator](); },
                    values() { return _fontFaces[Symbol.iterator](); },
                    [Symbol.iterator]() { return _fontFaces[Symbol.iterator](); },
                    size: _fontFaces.length,
                    add(face) {
                        if (face && !_fontFaces.includes(face)) _fontFaces.push(face);
                        return this;
                    },
                    delete(face) {
                        const i = _fontFaces.indexOf(face);
                        if (i >= 0) { _fontFaces.splice(i, 1); return true; }
                        return false;
                    },
                    has(face) { return _fontFaces.includes(face); },
                    clear() { _fontFaces.length = 0; },
                    addEventListener() {},
                    removeEventListener() {},
                },
                configurable: true,
            });
        }
    }

    // Permissions API is defined at the top of this file (search for
    // _PERMISSION_STATE_MAP). That implementation is the single source of
    // truth — do not override navigator.permissions.query here.

    // ================================================================
    // Apple Pay — macOS-only window.ApplePaySession
    // ================================================================
    // ApplePaySession is installed POST-snapshot in cleanup_bootstrap.js
    // because the V8 startup snapshot is built without a stealth profile,
    // so a snapshot-time `_p("os_name")` would always read "Linux" and
    // skip the install for every page. cleanup runs once per JsRuntime
    // creation with the profile loaded, which is the correct gating point.

    // ================================================================
    // Battery API — realistic values (already exists but enhance)
    // ================================================================
    // Already defined above in navigator — the existing implementation is sufficient.

    // ================================================================
    // Speech synthesis — OS-specific voices
    // ================================================================
    {
        const _osName = _p("os_name", "Linux");
        const _voicesByOS = {
            "Windows": [
                {name:"Microsoft David",lang:"en-US",localService:true,default:true,voiceURI:"Microsoft David"},
                {name:"Microsoft Zira",lang:"en-US",localService:true,default:false,voiceURI:"Microsoft Zira"},
                {name:"Microsoft Mark",lang:"en-US",localService:true,default:false,voiceURI:"Microsoft Mark"},
                {name:"Google US English",lang:"en-US",localService:false,default:false,voiceURI:"Google US English"},
                {name:"Google UK English Female",lang:"en-GB",localService:false,default:false,voiceURI:"Google UK English Female"},
            ],
            "macOS": [
                {name:"Alex",lang:"en-US",localService:true,default:true,voiceURI:"com.apple.voice.compact.en-US.Samantha"},
                {name:"Samantha",lang:"en-US",localService:true,default:false,voiceURI:"com.apple.voice.compact.en-US.Samantha"},
                {name:"Victoria",lang:"en-US",localService:true,default:false,voiceURI:"com.apple.speech.synthesis.voice.Victoria"},
                {name:"Google US English",lang:"en-US",localService:false,default:false,voiceURI:"Google US English"},
            ],
            "Linux": [
                {name:"Google US English",lang:"en-US",localService:false,default:true,voiceURI:"Google US English"},
                {name:"Google UK English Female",lang:"en-GB",localService:false,default:false,voiceURI:"Google UK English Female"},
                {name:"Google UK English Male",lang:"en-GB",localService:false,default:false,voiceURI:"Google UK English Male"},
            ],
        };
        const _voices = _voicesByOS[_osName] || _voicesByOS["Linux"];
        // Override the existing speechSynthesis with OS-aware voices
        globalThis.speechSynthesis.getVoices = function() { return _voices; };
    }

    // ================================================================
    // Media codecs — Chrome-correct isTypeSupported / canPlayType
    // ================================================================
    {
        const _supportedTypes = new Set([
            "video/mp4", 'video/mp4; codecs="avc1.42E01E"', 'video/mp4; codecs="avc1.42E01E, mp4a.40.2"',
            'video/mp4; codecs="avc1.4D401E"', 'video/mp4; codecs="avc1.64001E"',
            "video/webm", 'video/webm; codecs="vp8"', 'video/webm; codecs="vp8, vorbis"',
            'video/webm; codecs="vp9"', 'video/webm; codecs="vp09.00.10.08"',
            "audio/mp4", 'audio/mp4; codecs="mp4a.40.2"',
            "audio/webm", 'audio/webm; codecs="opus"', 'audio/webm; codecs="vorbis"',
            "audio/mpeg", "audio/ogg", 'audio/ogg; codecs="vorbis"', 'audio/ogg; codecs="opus"',
            "audio/wav", 'audio/wav; codecs="1"', "audio/flac",
            // Chrome accepts these codec MIME aliases too. The Kasada
            // smc probe (decrypted blob 0, 2026-05-10) tests audio/x-m4a
            // and audio/aac (sic — they wrote "acc" too) and reads the
            // verdict from MediaSource.isTypeSupported. Without these
            // entries we return false where Chrome returns true.
            "audio/x-m4a", "audio/aac", "audio/acc",
            "audio/mp3", "audio/x-wav",
        ]);

        // Removed redundant MediaSource definition here; it is defined further down.

        // Patch HTMLMediaElement.canPlayType if document exists.
        // The shim must be _maskFunction'd or its raw source leaks via
        // `el.canPlayType + ""`, `el.canPlayType.toString()`, AND
        // cross-realm `iframe.contentWindow.Function.prototype.toString.call(el.canPlayType)`.
        // Discovered by check_tostring_audit_full (Tier 1.2 audit, 2026-05-12).
        if (globalThis.document) {
            const _canPlayTypeShim = function canPlayType(type) {
                if (_supportedTypes.has(type)) return "probably";
                const base = type.split(';')[0].trim();
                if (_supportedTypes.has(base)) return "maybe";
                return "";
            };
            if (typeof _maskFunction === "function") {
                _maskFunction(_canPlayTypeShim, "canPlayType");
            }
            const _origCreate = globalThis.document.createElement.bind(globalThis.document);
            const _patchedCreate = function createElement(tag) {
                const el = _origCreate(tag);
                if (tag === 'video' || tag === 'audio') {
                    el.canPlayType = _canPlayTypeShim;
                }
                return el;
            };
            if (typeof _maskFunction === "function") {
                _maskFunction(_patchedCreate, "createElement");
            }
            globalThis.document.createElement = _patchedCreate;
        }

        // MediaRecorder — Chrome ships this as a real constructor with
        // a static isTypeSupported(mimeType) for codec capability checks.
        // Without it, the Kasada `mrs` probe throws
        // "Cannot read properties of undefined (reading 'isTypeSupported')".
        // Stub class — produces no recordings but answers capability
        // probes correctly using the same _supportedTypes set.
        // Removed redundant MediaRecorder definition here; it is defined further down.
    }

    // ================================================================
    // Stubs for Web APIs that Kasada/DataDome probe. All defined
    // as globalThis classes + (where applicable) navigator/window
    // accessors. These return defined-but-functionally-stub objects
    // so antibot probes that read `.SOME_PROPERTY` get a non-undefined
    // receiver.
    // ================================================================

    // PressureObserver / PressureRecord — Compute Pressure API
    // (https://w3c.github.io/compute-pressure/). Chrome 125+. Probed by
    // Kasada esd.cpt.
    if (!globalThis.PressureObserver) {
        class PressureRecord {
            constructor(source = 'cpu', state = 'nominal') {
                this.source = source;
                this.state = state;
                this.time = performance.now();
            }
            toJSON() { return { source: this.source, state: this.state, time: this.time }; }
        }
        Object.defineProperty(PressureRecord.prototype, Symbol.toStringTag, {
            value: 'PressureRecord', configurable: true,
        });
        globalThis.PressureRecord = PressureRecord;

        class PressureObserver {
            constructor(callback, options = {}) {
                this._callback = callback;
                this._options = options;
                this._observing = new Set();
            }
            observe(source, _options) {
                this._observing.add(source);
                return Promise.resolve();
            }
            unobserve(source) { this._observing.delete(source); }
            disconnect() { this._observing.clear(); }
            takeRecords() { return []; }
            static knownSources = ['cpu'];
        }
        Object.defineProperty(PressureObserver.prototype, Symbol.toStringTag, {
            value: 'PressureObserver', configurable: true,
        });
        globalThis.PressureObserver = PressureObserver;
    }

    // MediaSourceHandle — wraps MediaSource for transfer to Worker.
    // (https://w3c.github.io/media-source/#mediasourcehandle-interface).
    // Probed by Kasada smc.o. Stub class with toString tag.
    if (!globalThis.MediaSourceHandle) {
        class MediaSourceHandle {}
        Object.defineProperty(MediaSourceHandle.prototype, Symbol.toStringTag, {
            value: 'MediaSourceHandle', configurable: true,
        });
        globalThis.MediaSourceHandle = MediaSourceHandle;
    }

    // DocumentPictureInPicture — Document Picture-in-Picture API
    // (https://wicg.github.io/document-picture-in-picture/). Chrome 116+.
    // Probed by Kasada dpv. Singleton on window.
    if (!globalThis.DocumentPictureInPicture) {
        class DocumentPictureInPicture extends EventTarget {
            constructor() { super(); this._window = null; }
            get window() { return this._window; }
            requestWindow(_options) {
                // We don't actually open a PiP window in headless. Reject
                // to match Chrome's headless behavior.
                return Promise.reject(new DOMException(
                    'Document PiP requires a user gesture',
                    'NotAllowedError'
                ));
            }
        }
        Object.defineProperty(DocumentPictureInPicture.prototype, Symbol.toStringTag, {
            value: 'DocumentPictureInPicture', configurable: true,
        });
        globalThis.DocumentPictureInPicture = DocumentPictureInPicture;
        const _docPip = new DocumentPictureInPicture();
        Object.defineProperty(globalThis, 'documentPictureInPicture', {
            get: () => _docPip, configurable: true, enumerable: true,
        });
    }

    // navigator.userActivation — UserActivation interface
    // (https://html.spec.whatwg.org/multipage/interaction.html#useractivation).
    // Reports whether the user has interacted with the page (gestures).
    // Probed by Kasada bot1225 / various others.
    // Chrome 88+ only — real Safari (any platform) has no UserActivation
    // interface. Skip the class install AND the navigator binding on iOS.
    if (!_isMobileIOS() && typeof globalThis.UserActivation === 'undefined') {
        class UserActivation {
            constructor() {
                this._hasBeenActive = false;
                this._isActive = false;
            }
            get hasBeenActive() { return this._hasBeenActive; }
            get isActive() { return this._isActive; }
        }
        Object.defineProperty(UserActivation.prototype, Symbol.toStringTag, {
            value: 'UserActivation', configurable: true,
        });
        globalThis.UserActivation = UserActivation;
        const _userAct = new UserActivation();
        // Wire onto navigator (and Navigator.prototype when accessor-defined).
        try {
            Object.defineProperty(navigator, 'userActivation', {
                get: () => _userAct, configurable: true, enumerable: true,
            });
        } catch (_e) {}
    }

    // --- Native code masking ---
    // Anti-bot detectors check Function.prototype.toString() for polyfilled APIs.
    // Real Chrome returns "function X() { [native code] }" for built-in functions.
    // Wrap our polyfills so toString() returns the native format.

    // Mask navigator methods
    _maskAsNative(navigator, 'javaEnabled', 'sendBeacon', 'getBattery');
    if (navigator.keyboard) _maskAsNative(navigator.keyboard, 'getLayoutMap', 'lock', 'unlock');
    _maskAsNative(PluginArray.prototype, 'item', 'namedItem', 'refresh');
    _maskAsNative(MimeTypeArray.prototype, 'item', 'namedItem');
    _maskAsNative(Plugin.prototype, 'item', 'namedItem');
    // Mask WebRTC
    _maskAsNative(globalThis.RTCPeerConnection.prototype, 'createOffer', 'createAnswer',
        'setLocalDescription', 'setRemoteDescription', 'addIceCandidate', 'close',
        'createDataChannel', 'getStats', 'getSenders', 'getReceivers', 'getTransceivers',
        'addTrack', 'removeTrack');
    _maskAsNative(globalThis.RTCPeerConnection, 'generateCertificate');
    
    // Mask document.write
    if (globalThis.document) {
        _maskAsNative(Object.getPrototypeOf(globalThis.document), 'write', 'writeln');
    }
    // Mask MediaSource
    if (globalThis.MediaSource) _maskAsNative(globalThis.MediaSource, 'isTypeSupported');
    // Mask speechSynthesis
    _maskAsNative(globalThis.speechSynthesis, 'getVoices', 'speak', 'cancel', 'pause', 'resume');
    if (navigator.permissions) _maskAsNative(navigator.permissions, 'query');
    if (navigator.mediaDevices) _maskAsNative(navigator.mediaDevices, 'enumerateDevices');
    if (navigator.clipboard) _maskAsNative(navigator.clipboard, 'readText', 'writeText');
    if (navigator.storage) _maskAsNative(navigator.storage, 'estimate');
    if (navigator.serviceWorker) _maskAsNative(navigator.serviceWorker, 'register', 'getRegistrations', 'getRegistration', 'startMessages');

    // Mask window methods
    _maskAsNative(globalThis, 'fetch', 'alert', 'confirm', 'prompt', 'open', 'close',
        'scrollTo', 'scroll', 'scrollBy', 'getComputedStyle', 'matchMedia',
        'getSelection', 'postMessage', 'requestIdleCallback', 'atob', 'btoa');

    // Mask document methods
    if (globalThis.document) {
        _maskAsNative(globalThis.document, 'createElement', 'createTextNode',
            'createDocumentFragment', 'createEvent', 'createRange',
            'getElementById', 'querySelector', 'querySelectorAll',
            'getElementsByTagName', 'getElementsByClassName',
            'write', 'writeln', 'execCommand', 'hasFocus',
            'elementFromPoint', 'elementsFromPoint', 'getSelection',
            'importNode', 'adoptNode');
    }

    // ================================================================
    // P0 FIX: Error stack trace filtering
    // Remove deno_core internal frames AND all browser_oxide bootstrap
    // script names from Error.stack. A captured VM trace previously
    // showed `at h (<init_script_0>:51:34)` — Kasada literally saw the
    // browser_oxide-internal script name. Real Chrome's stack frames
    // never show such tags; they show either real URLs or <anonymous>.
    //
    // Filter strategy: drop any frame whose filename starts with `<`
    // EXCEPT `<anonymous>` (which V8 legitimately emits for eval).
    // This catches `<bootstrap>`, `<cleanup>`, `<init_script_N>`,
    // `<structured_clone>`, `<canvas_bootstrap>`, `<timer_bootstrap>`,
    // `<fetch_bootstrap>`, `<streams_bootstrap>`, `<worker_bootstrap>`,
    // and any future bootstrap names without needing per-name additions.
    // ================================================================
    Error.prepareStackTrace = function(err, frames) {
        const filtered = frames.filter(f => {
            const file = f.getFileName() || '';
            if (file.startsWith('ext:') || file.startsWith('deno:')) return false;
            if (file.includes('core/')) return false;
            // Internal bootstrap script names — all match `<...>` shape.
            // Preserve V8's legitimate <anonymous> tag for eval/Function-
            // constructor frames; drop everything else angle-bracketed.
            if (file.startsWith('<') && file.endsWith('>') && file !== '<anonymous>') {
                return false;
            }
            return true;
        });
        if (filtered.length === 0) {
            return err.toString() + '\n    at <anonymous>:1:1';
        }
        return err.toString() + '\n' + filtered.map(f => {
            const fn = f.getFunctionName() || f.getMethodName() || '<anonymous>';
            const file = f.getFileName() || '<anonymous>';
            const line = f.getLineNumber() || 0;
            const col = f.getColumnNumber() || 0;
            // Format: "    at functionName (filename:line:col)"
            // or if no filename: "    at functionName (line:col)"
            // or if no function name: "    at filename:line:col"
            if (fn === '<anonymous>') {
                return `    at ${file}:${line}:${col}`;
            }
            return `    at ${fn} (${file}:${line}:${col})`;
        }).join('\n');
    };

    // ================================================================
    // performance.now() — humanized via op_perf_now_humanized (gap #31a).
    //
    // Real Chrome 130 quantizes to 100 µs but with hardware/scheduler jitter
    // around the step. A perfect 100 µs grid (Math.round * 10 / 10) gives
    // `set(diffs).size === 1` for hot loops — Kasada/DataDome flag this.
    //
    // The op applies LogNormal(μ=ln 8 µs, σ=0.4) jitter clamped [0,35] µs
    // plus rare exponential spike. Installed on Performance.prototype so the
    // own-descriptor probe still returns undefined on the instance.
    // ================================================================
    if (typeof globalThis.Performance === 'function' && globalThis.performance) {
        const _PProto = globalThis.Performance.prototype;
        _defProtoMethod(_PProto, 'now', function now() {
            return ops.op_perf_now_humanized();
        });
    }

    // ================================================================
    // VisualViewport — Chrome surface that fingerprinters probe to
    // detect mobile vs desktop AND to detect headless absence. Real
    // Chrome exposes a singleton instance accessible as
    // MediaSource + MediaRecorder.isTypeSupported in window realm.
    // Kasada's `mrs` probe (W4a 2026-05-11) reads .isTypeSupported.
    (() => {
        // Per the captured Kasada smc probe blob (2026-05-10), Kasada
        // tests audio/x-m4a + audio/aac + audio/acc and reads the
        // boolean verdict. Real Chrome returns true; without these
        // entries we return false → captured `v:false` was a real engine
        // gap. Bringing in line with the first _supportedTypes Set at
        // line 4987 (the canPlayType one).
        const _supportedTypes = new Set([
            "video/mp4", 'video/mp4;codecs="avc1.42E01E,mp4a.40.2"',
            'video/mp4;codecs="avc1.640028"', "video/webm",
            'video/webm;codecs="vp8,vorbis"', 'video/webm;codecs="vp9"',
            'video/webm;codecs="vp9,opus"', "audio/mp4",
            'audio/mp4;codecs="mp4a.40.2"', "audio/webm",
            'audio/webm;codecs=opus', 'audio/webm;codecs=vorbis',
            // Kasada smc probe — captured blob 2026-05-10:
            "audio/x-m4a", "audio/aac", "audio/acc",
            "audio/mpeg", "audio/ogg", "audio/wav", "audio/flac",
            "audio/mp3", "audio/x-wav",
        ]);
        
        const _isTypeSupported = ({
            isTypeSupported(type) {
                if (typeof type !== 'string') return false;
                if (_supportedTypes.has(type)) return true;
                const base = type.split(';')[0].trim();
                return _supportedTypes.has(base);
            }
        }).isTypeSupported;
        _maskFunction(_isTypeSupported, "isTypeSupported");

        class SourceBufferList extends EventTarget {
            constructor() { super(); this.length = 0; }
            [Symbol.iterator]() {
                let i = 0;
                const self = this;
                return {
                    next() {
                        if (i < self.length) return { value: self[i++], done: false };
                        return { value: undefined, done: true };
                    },
                    [Symbol.iterator]() { return this; }
                };
            }
        }
        _maskFunction(SourceBufferList, "SourceBufferList");

        // Replace stubs with real (non-throwing) constructors so Kasada VM
        // can call `new MediaSource()` during smc probe init without aborting.
        (() => {
            class MediaSource extends EventTarget {
                constructor() {
                    super();
                    this.readyState = 'closed';
                    this.duration = NaN;
                    this.sourceBuffers = new SourceBufferList();
                    this.activeSourceBuffers = new SourceBufferList();
                }
                addSourceBuffer() { throw new DOMException('InvalidStateError'); }
                removeSourceBuffer() { throw new DOMException('InvalidStateError'); }
                endOfStream() {}
                setLiveSeekableRange() {}
                clearLiveSeekableRange() {}
                static isTypeSupported(type) { return _isTypeSupported(type); }
                static canConstructInDedicatedWorker = false;
            }
            _maskFunction(MediaSource, "MediaSource");
            globalThis.MediaSource = MediaSource;
        })();
        (() => {
            class MediaRecorder extends EventTarget {
                constructor(stream, options) {
                    super();
                    this.stream = stream || null;
                    this.mimeType = (options && options.mimeType) || '';
                    this.state = 'inactive';
                    this.audioBitsPerSecond = 0;
                    this.videoBitsPerSecond = 0;
                }
                start() {}
                stop() {}
                pause() {}
                resume() {}
                requestData() {}
                static isTypeSupported(type) { return _isTypeSupported(type); }
            }
            _maskFunction(MediaRecorder, "MediaRecorder");
            globalThis.MediaRecorder = MediaRecorder;
        })();

        const _getCapabilities = ({
            getCapabilities(kind) {
                return {
                    codecs: kind === 'audio' ? [
                        { channels: 2, clockRate: 48000, mimeType: "audio/opus" },
                        { channels: 1, clockRate: 8000, mimeType: "audio/PCMU" },
                        { channels: 1, clockRate: 8000, mimeType: "audio/PCMA" }
                    ] : [
                        { clockRate: 90000, mimeType: "video/VP8" },
                        { clockRate: 90000, mimeType: "video/VP9", sdpFmtpLine: "profile-id=0" },
                        { clockRate: 90000, mimeType: "video/H264", sdpFmtpLine: "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f" }
                    ],
                    headerExtensions: []
                };
            }
        }).getCapabilities;
        _maskFunction(_getCapabilities, "getCapabilities");

        if (globalThis.RTCRtpReceiver) {
            Object.defineProperty(globalThis.RTCRtpReceiver, 'getCapabilities', {
                value: _getCapabilities, configurable: true, writable: true, enumerable: false
            });
            _maskFunction(globalThis.RTCRtpReceiver, "RTCRtpReceiver");
        }
        if (globalThis.RTCRtpSender) {
            Object.defineProperty(globalThis.RTCRtpSender, 'getCapabilities', {
                value: _getCapabilities, configurable: true, writable: true, enumerable: false
            });
            _maskFunction(globalThis.RTCRtpSender, "RTCRtpSender");
        }
    })();

    // `window.visualViewport`. Properties are layout-derived but for
    // a stationary viewport without pinch-zoom they equal the layout
    // viewport scaled by 1.0. Spec:
    // https://www.w3.org/TR/visual-viewport/
    // ================================================================
    {
        class VisualViewport extends EventTarget {
            get offsetLeft() { return 0; }
            get offsetTop() { return 0; }
            get pageLeft() { return 0; }
            get pageTop() { return 0; }
            get width() { return _pInt("inner_width", 1920); }
            get height() { return _pInt("inner_height", 1080); }
            get scale() { return 1; }
            get onresize() { return null; }
            set onresize(_v) {}
            get onscroll() { return null; }
            set onscroll(_v) {}
            get onscrollend() { return null; }
            set onscrollend(_v) {}
        }
        Object.defineProperty(VisualViewport.prototype, Symbol.toStringTag, {
            value: "VisualViewport", configurable: true,
        });
        globalThis.VisualViewport = VisualViewport;
        const _vv = new VisualViewport();
        Object.defineProperty(globalThis, 'visualViewport', {
            get() { return _vv; }, configurable: true, enumerable: true,
        });
    }

    // ================================================================
    // InputDeviceCapabilities — present on UIEvent.sourceCapabilities
    // in real Chrome. Sites probe `event.sourceCapabilities` on the
    // first user-input event to confirm a real input device fired it.
    // We define the constructor; integration with synthesized events
    // is deferred to the input_ext humanization layer.
    // ================================================================
    {
        class InputDeviceCapabilities {
            constructor(init) {
                this.firesTouchEvents = !!(init && init.firesTouchEvents);
            }
        }
        Object.defineProperty(InputDeviceCapabilities.prototype, Symbol.toStringTag, {
            value: "InputDeviceCapabilities", configurable: true,
        });
        globalThis.InputDeviceCapabilities = InputDeviceCapabilities;
    }

    // ================================================================
    // MediaSession — `navigator.mediaSession` is a real `MediaSession`
    // instance in Chrome, not a plain `{}`. Sites use its presence to
    // gate playback-state UI and to drive system media controls.
    // Spec: https://w3c.github.io/mediasession/
    // ================================================================
    {
        const _validStates = new Set(["none", "playing", "paused"]);
        class MediaMetadata {
            constructor(init) {
                init = init || {};
                this.title = String(init.title || "");
                this.artist = String(init.artist || "");
                this.album = String(init.album || "");
                this.artwork = Array.isArray(init.artwork) ? init.artwork.slice() : [];
            }
        }
        Object.defineProperty(MediaMetadata.prototype, Symbol.toStringTag, {
            value: "MediaMetadata", configurable: true,
        });
        globalThis.MediaMetadata = MediaMetadata;

        class MediaSession {
            constructor() {
                this._playbackState = "none";
                this._metadata = null;
                this._handlers = new Map();
            }
            get playbackState() { return this._playbackState; }
            set playbackState(v) {
                const s = String(v);
                if (_validStates.has(s)) this._playbackState = s;
            }
            get metadata() { return this._metadata; }
            set metadata(v) {
                this._metadata = v instanceof MediaMetadata ? v : null;
            }
            setActionHandler(action, handler) {
                if (handler == null) {
                    this._handlers.delete(String(action));
                } else if (typeof handler === "function") {
                    this._handlers.set(String(action), handler);
                }
            }
            setPositionState(_state) { /* spec accepts {duration, position, playbackRate} */ }
            setCameraActive(_active) { /* video conferencing extension */ }
            setMicrophoneActive(_active) { /* video conferencing extension */ }
        }
        Object.defineProperty(MediaSession.prototype, Symbol.toStringTag, {
            value: "MediaSession", configurable: true,
        });
        globalThis.MediaSession = MediaSession;
        const _ms = new MediaSession();
        // Override the placeholder navigator.mediaSession (was {}).
        try {
            Object.defineProperty(_NavProto, 'mediaSession', {
                get() { return _ms; }, configurable: true, enumerable: true,
            });
        } catch (_) {
            navigator.mediaSession = _ms;
        }
    }

    // ================================================================
    // MediaCapabilities — `navigator.mediaCapabilities` is a real
    // `MediaCapabilities` instance in every modern Chrome / Safari /
    // Firefox. Kasada (ips.js handler #45), DataDome, FingerprintJS and
    // CreepJS all probe it; when the property is `undefined` the probe
    // throws `Cannot read properties of undefined (reading '…')` and
    // the resulting error fingerprint is the "headless" signal.
    // Spec: https://w3c.github.io/media-capabilities/
    // Gated to non-Gecko UAs: Firefox's MediaCapabilities returns
    // {supported: true} for fewer codec families and exact shape match
    // is hard to fake. Pre-v3 firefox passes without this surface.
    // ================================================================
    if (!/Firefox\/|Gecko\/20100101/.test(_p("user_agent", ""))) {
        const _supportedDecodingTypes = new Set([
            "file", "media-source", "webrtc"
        ]);
        const _supportedEncodingTypes = new Set([
            "record", "webrtc"
        ]);
        const _normaliseMime = (s) => String(s || "").trim().toLowerCase();
        // Codec families that real Chrome reports as supported on
        // desktop. Conservative: only the common families Kasada lists
        // in its probe matrix (mp4/h264/h265, vp8/vp9, av1, opus, mp4a).
        const _supportedFamilies = [
            "video/mp4", "video/webm", "video/h264", "video/h265", "video/hevc",
            "video/avc", "video/vp8", "video/vp9", "video/av1", "video/avs3",
            "audio/mp4", "audio/webm", "audio/aac", "audio/mpeg", "audio/opus",
            "audio/vorbis", "audio/flac", "audio/wav", "audio/ogg",
            "application/x-mpegurl"
        ];
        function _supportsContentType(ct) {
            const t = _normaliseMime(ct);
            if (!t) return false;
            for (let i = 0; i < _supportedFamilies.length; i++) {
                if (t.indexOf(_supportedFamilies[i]) === 0) return true;
            }
            return false;
        }
        function _buildInfo(config, supportedDecodingType) {
            const cfg = config && typeof config === "object" ? config : {};
            const type = cfg.type;
            const audio = cfg.audio && typeof cfg.audio === "object" ? cfg.audio : null;
            const video = cfg.video && typeof cfg.video === "object" ? cfg.video : null;
            let supported = supportedDecodingType
                ? _supportedDecodingTypes.has(type)
                : _supportedEncodingTypes.has(type);
            if (supported && audio) supported = _supportsContentType(audio.contentType);
            if (supported && video) supported = _supportsContentType(video.contentType);
            return {
                supported,
                smooth: supported,
                powerEfficient: supported,
                configuration: cfg,
                // keyStatuses appears in EME-bound decodingInfo; absent
                // for plain configs — return undefined accessor to match
                // Chrome.
            };
        }
        class MediaCapabilities {
            constructor() { /* spec: no constructor args */ }
            decodingInfo(configuration) {
                if (configuration == null || typeof configuration !== "object") {
                    return Promise.reject(new TypeError(
                        "Failed to execute 'decodingInfo' on 'MediaCapabilities': " +
                        "1 argument required, but only 0 present."));
                }
                if (!_supportedDecodingTypes.has(configuration.type)) {
                    return Promise.reject(new TypeError(
                        "Failed to execute 'decodingInfo' on 'MediaCapabilities': " +
                        "The provided value '" + configuration.type +
                        "' is not a valid enum value of type MediaDecodingType."));
                }
                return Promise.resolve(_buildInfo(configuration, true));
            }
            encodingInfo(configuration) {
                if (configuration == null || typeof configuration !== "object") {
                    return Promise.reject(new TypeError(
                        "Failed to execute 'encodingInfo' on 'MediaCapabilities': " +
                        "1 argument required, but only 0 present."));
                }
                if (!_supportedEncodingTypes.has(configuration.type)) {
                    return Promise.reject(new TypeError(
                        "Failed to execute 'encodingInfo' on 'MediaCapabilities': " +
                        "The provided value '" + configuration.type +
                        "' is not a valid enum value of type MediaEncodingType."));
                }
                return Promise.resolve(_buildInfo(configuration, false));
            }
        }
        Object.defineProperty(MediaCapabilities.prototype, Symbol.toStringTag, {
            value: "MediaCapabilities", configurable: true,
        });
        globalThis.MediaCapabilities = MediaCapabilities;
        const _mc = new MediaCapabilities();
        try {
            Object.defineProperty(_NavProto, 'mediaCapabilities', {
                get() { return _mc; }, configurable: true, enumerable: true,
            });
        } catch (_) {
            navigator.mediaCapabilities = _mc;
        }
        // Mask methods as native so toString catalogue stays Chrome-shaped.
        try { _maskAsNative(MediaCapabilities.prototype, 'decodingInfo', 'encodingInfo'); } catch (_) {}
    }

    // ================================================================
    // HTMLVideoElement.prototype.requestVideoFrameCallback — Chrome/Safari.
    // Firefox added it in 132 but with subtly different metadata shape.
    // Adding our Chrome-shaped impl to a Firefox profile creates a tell
    // detected by PerimeterX (firefox v3 regressed wayfair). Gate to
    // non-Gecko profiles. Spec: https://wicg.github.io/video-rvfc/
    // ================================================================
    const _isGeckoUA = /Firefox\/|Gecko\/20100101/.test(
        _p("user_agent", "")
    );
    if (!_isGeckoUA && typeof globalThis.HTMLVideoElement !== "undefined" &&
        typeof globalThis.HTMLVideoElement.prototype.requestVideoFrameCallback !== "function") {
        let _rvfcSeq = 1;
        const _pendingRvfc = new Map();
        const _requestVideoFrameCallback = function requestVideoFrameCallback(cb) {
            if (typeof cb !== "function") {
                throw new TypeError(
                    "Failed to execute 'requestVideoFrameCallback' on 'HTMLVideoElement': " +
                    "The callback provided as parameter 1 is not a function.");
            }
            const id = _rvfcSeq++;
            // Schedule a single callback ~1 frame ahead. Real Chrome
            // fires when a new frame is presented; without playback we
            // approximate the rAF cadence.
            const handle = setTimeout(() => {
                _pendingRvfc.delete(id);
                try {
                    cb(performance.now(), {
                        presentationTime: performance.now(),
                        expectedDisplayTime: performance.now() + 16.67,
                        width: 0, height: 0,
                        mediaTime: 0, presentedFrames: 0,
                        processingDuration: 0,
                    });
                } catch (_) {}
            }, 16);
            _pendingRvfc.set(id, handle);
            return id;
        };
        const _cancelVideoFrameCallback = function cancelVideoFrameCallback(id) {
            const handle = _pendingRvfc.get(id);
            if (handle != null) {
                clearTimeout(handle);
                _pendingRvfc.delete(id);
            }
        };
        Object.defineProperty(globalThis.HTMLVideoElement.prototype, "requestVideoFrameCallback", {
            value: _requestVideoFrameCallback, configurable: true, writable: true,
        });
        Object.defineProperty(globalThis.HTMLVideoElement.prototype, "cancelVideoFrameCallback", {
            value: _cancelVideoFrameCallback, configurable: true, writable: true,
        });
        try {
            _maskAsNative(globalThis.HTMLVideoElement.prototype,
                "requestVideoFrameCallback", "cancelVideoFrameCallback");
        } catch (_) {}
    }

    // ================================================================
    // P2 STUBS: Emerging APIs that anti-bot scripts probe for existence
    // ================================================================

    // navigator.gpu (WebGPU) — prototype getter so own-descriptor probe
    // returns undefined on the instance (kNoScriptId-safe).
    if (!_NavProto.hasOwnProperty('gpu')) {
        const _navGpu = {
            requestAdapter() {
                return Promise.resolve({
                    name: _p("webgl_renderer", "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080)"),
                    features: new Set(),
                    limits: {},
                    isFallbackAdapter: false,
                    requestDevice() { return Promise.reject(new DOMException("Not supported", "NotSupportedError")); },
                });
            },
            getPreferredCanvasFormat() { return "bgra8unorm"; },
        };
        Object.defineProperty(_navGpu, Symbol.toStringTag, { value: "GPU", configurable: true });
        // WebGPU is [SecureContext] — undefined on data:/http:/about:blank.
        _defNav('gpu', () => _secure() ? _navGpu : undefined);
    }

    // Storage Access API
    if (globalThis.document) {
        if (!globalThis.document.hasStorageAccess) {
            globalThis.document.hasStorageAccess = function() { return Promise.resolve(false); };
        }
        if (!globalThis.document.requestStorageAccess) {
            globalThis.document.requestStorageAccess = function() { return Promise.reject(new DOMException("Not allowed", "NotAllowedError")); };
        }
    }

    // CSS.supports()
    if (!globalThis.CSS) globalThis.CSS = {};
    if (!globalThis.CSS.supports) {
        const _cssSupported = new Set([
            "display:grid", "display:flex", "display:block", "display:inline",
            "position:sticky", "position:fixed", "position:absolute",
            "gap:1px", "aspect-ratio:1", "container-type:inline-size",
            "color:oklch(0 0 0)", "color:color-mix(in srgb,red,blue)",
            "backdrop-filter:blur(1px)", "overflow:clip",
            "translate:none", "rotate:none", "scale:none",
            "accent-color:auto", "overscroll-behavior:contain",
        ]);
        globalThis.CSS.supports = function(prop, val) {
            if (val === undefined) {
                // Single argument: CSS.supports("display: grid")
                return _cssSupported.has(prop.replace(/\s+/g, '').toLowerCase()) || true;
            }
            return _cssSupported.has(`${prop.toLowerCase()}:${val.toLowerCase()}`) || true;
        };
    }

    // navigator.scheduling is already defined on Navigator.prototype above
    // with isInputPending() — no need to re-install on the instance.

    // crossOriginIsolated is now installed as an op-backed getter near the
    // top of window_bootstrap.js (search for op_cross_origin_isolated).
    // No fallback needed — defineProperty above runs before any user JS.

    // ================================================================
    // Trusted Types API (Chrome 83+)
    // Anti-bot scripts and CSP policies check window.trustedTypes presence.
    // ================================================================
    if (!globalThis.trustedTypes) {
        const _ttPolicies = new Map();
        const _TrustedHTML = function TrustedHTML(v) { this._v = v; };
        _TrustedHTML.prototype.toString = function() { return this._v; };
        const _TrustedScript = function TrustedScript(v) { this._v = v; };
        _TrustedScript.prototype.toString = function() { return this._v; };
        const _TrustedScriptURL = function TrustedScriptURL(v) { this._v = v; };
        _TrustedScriptURL.prototype.toString = function() { return this._v; };
        globalThis.trustedTypes = {
            createPolicy(name, rules) {
                const p = {
                    name,
                    createHTML: (s) => typeof rules.createHTML === 'function' ? new _TrustedHTML(rules.createHTML(s)) : new _TrustedHTML(s),
                    createScript: (s) => typeof rules.createScript === 'function' ? new _TrustedScript(rules.createScript(s)) : new _TrustedScript(s),
                    createScriptURL: (s) => typeof rules.createScriptURL === 'function' ? new _TrustedScriptURL(rules.createScriptURL(s)) : new _TrustedScriptURL(s),
                };
                _ttPolicies.set(name, p);
                if (name === 'default') globalThis.trustedTypes.defaultPolicy = p;
                return p;
            },
            isHTML(v) { return v instanceof _TrustedHTML; },
            isScript(v) { return v instanceof _TrustedScript; },
            isScriptURL(v) { return v instanceof _TrustedScriptURL; },
            getAttributeType() { return null; },
            getPropertyType() { return null; },
            defaultPolicy: null,
            emptyHTML: new _TrustedHTML(''),
            emptyScript: new _TrustedScript(''),
        };
        globalThis.TrustedHTML = _TrustedHTML;
        globalThis.TrustedScript = _TrustedScript;
        globalThis.TrustedScriptURL = _TrustedScriptURL;
        _maskAsNative(globalThis.trustedTypes, 'createPolicy', 'isHTML', 'isScript', 'isScriptURL', 'getAttributeType', 'getPropertyType');
    }

    // ================================================================
    // Scheduler API (Chrome 104+)
    // window.scheduler.postTask / scheduler.yield are checked by bot detectors.
    // ================================================================
    if (!globalThis.scheduler) {
        const _SProto = globalThis.Scheduler && globalThis.Scheduler.prototype;
        if (_SProto) {
            globalThis.scheduler = Object.create(_SProto);
            Object.assign(globalThis.scheduler, {
                postTask(callback, options) {
                    const delay = (options && options.delay) || 0;
                    return new Promise((resolve, reject) => {
                        setTimeout(() => {
                            try { resolve(callback()); } catch (e) { reject(e); }
                        }, delay);
                    });
                },
                yield() {
                    return new Promise(resolve => setTimeout(resolve, 0));
                },
            });
            _maskAsNative(globalThis.scheduler, 'postTask', 'yield');
        }
    }

    // ================================================================
    // reportError (Chrome 95+) — dispatches an ErrorEvent on window.
    // ================================================================
    if (!globalThis.reportError) {
        globalThis.reportError = ({
            reportError(err) {
                const evt = new ErrorEvent('error', { error: err, message: err && err.message || String(err), bubbles: true, cancelable: true });
                globalThis.dispatchEvent(evt);
            }
        }).reportError;
        _maskAsNative(globalThis, 'reportError');
    }

    // ================================================================
    // Touch / TouchEvent constructors — present in Chrome on all platforms.
    // Desktop Chrome defines them even though touch isn't available.
    // Anti-bot scripts check typeof Touch / typeof TouchEvent.
    // ================================================================
    if (!globalThis.Touch) {
        globalThis.Touch = function Touch(init) {
            if (!init || init.identifier === undefined || !init.target) {
                throw new TypeError("Failed to construct 'Touch': required members identifier and target");
            }
            this.identifier = init.identifier;
            this.target = init.target;
            this.clientX = init.clientX || 0;
            this.clientY = init.clientY || 0;
            this.screenX = init.screenX || 0;
            this.screenY = init.screenY || 0;
            this.pageX = init.pageX || 0;
            this.pageY = init.pageY || 0;
            this.radiusX = init.radiusX || 0;
            this.radiusY = init.radiusY || 0;
            this.rotationAngle = init.rotationAngle || 0;
            this.force = init.force || 0;
            this.altitudeAngle = init.altitudeAngle || 0;
            this.azimuthAngle = init.azimuthAngle || 0;
            this.touchType = init.touchType || 'direct';
        };
        globalThis.Touch.prototype = Object.create(Object.prototype, {
            constructor: { value: globalThis.Touch, configurable: true, writable: true },
            [Symbol.toStringTag]: { value: "Touch", configurable: true },
        });
        _maskAsNative(globalThis.Touch);
    }
    if (!globalThis.TouchEvent) {
        globalThis.TouchEvent = function TouchEvent(type, init) {
            const base = new Event(type || 'touchstart', init || {});
            base.touches = (init && init.touches) ? init.touches : new TouchList();
            base.targetTouches = (init && init.targetTouches) ? init.targetTouches : new TouchList();
            base.changedTouches = (init && init.changedTouches) ? init.changedTouches : new TouchList();
            base.altKey = (init && init.altKey) || false;
            base.ctrlKey = (init && init.ctrlKey) || false;
            base.metaKey = (init && init.metaKey) || false;
            base.shiftKey = (init && init.shiftKey) || false;
            return base;
        };
        globalThis.TouchEvent.prototype = Object.create(Event.prototype, {
            constructor: { value: globalThis.TouchEvent, configurable: true, writable: true },
        });
        _maskAsNative(globalThis.TouchEvent);
    }
    if (!globalThis.TouchList) {
        globalThis.TouchList = function TouchList() { this.length = 0; };
        globalThis.TouchList.prototype.item = function(i) { return this[i] || null; };
        _maskAsNative(globalThis.TouchList);
    }

    // ================================================================
    // SharedArrayBuffer — only available with cross-origin isolation.
    // Chrome hides it (returns undefined) without COOP+COEP headers.
    // Most sites don't set these headers, so SAB is undefined on most pages.
    // V8 doesn't let us delete built-in globals, so we shadow with a getter
    // that returns undefined, matching Chrome's non-isolated behavior.
    // ================================================================
    if (!ops.op_cross_origin_isolated()) {
        try {
            Object.defineProperty(globalThis, 'SharedArrayBuffer', {
                get: () => undefined,
                configurable: true,
                enumerable: false,
            });
        } catch(_) {}
    }

    // ================================================================
    // Phase 6 D4 — Missing-constructor batch (10 surfaces)
    //
    // Real Chrome 147 macOS exposes these surfaces. Most are
    // "present-but-doesn't-work" — Chrome ships the constructor /
    // instance with the right shape, but invoking the network/IO path
    // either rejects (network APIs) or no-ops (UI APIs). We mirror the
    // shape so detection probes don't see absence.
    //
    // ================================================================

    // (1) globalThis.caches — CacheStorage (Service Worker spec)
    // Spec: https://w3c.github.io/ServiceWorker/#cachestorage
    // [SecureContext] — undefined on insecure contexts. Phase 7.
    if (_secure() && typeof globalThis.caches === "undefined") {
        class CacheStorage {
            match(_request, _options) { return Promise.resolve(undefined); }
            has(_cacheName) { return Promise.resolve(false); }
            open(_cacheName) {
                return Promise.resolve({
                    match() { return Promise.resolve(undefined); },
                    matchAll() { return Promise.resolve([]); },
                    add() { return Promise.reject(new TypeError("Cache.add not supported")); },
                    addAll() { return Promise.reject(new TypeError("Cache.addAll not supported")); },
                    put() { return Promise.reject(new TypeError("Cache.put not supported")); },
                    delete() { return Promise.resolve(false); },
                    keys() { return Promise.resolve([]); },
                });
            }
            delete(_cacheName) { return Promise.resolve(false); }
            keys() { return Promise.resolve([]); }
        }
        Object.defineProperty(CacheStorage.prototype, Symbol.toStringTag, {
            value: "CacheStorage", configurable: true,
        });
        globalThis.CacheStorage = CacheStorage;
        Object.defineProperty(globalThis, "caches", {
            value: new CacheStorage(), configurable: true, enumerable: true, writable: false,
        });
    }

    // (2) globalThis.cookieStore — async Cookie Store API
    // Spec: https://wicg.github.io/cookie-store/
    // [SecureContext] — undefined on insecure contexts. Phase 7.
    // Real Chrome exposes a CookieStore INSTANCE on globalThis (not a
    // constructor) when secure. Our prior `_illegalCtor("CookieStore")`
    // from interfaces_bootstrap is replaced unconditionally so the
    // *constructor* exists on globalThis as a real class — but the
    // instance binding `globalThis.cookieStore` is gated on secure.
    {
        // Real Chrome's CookieStore is [Exposed] but has no public
        // constructor — `new CookieStore()` throws "Failed to construct
        // 'CookieStore': Illegal constructor". We mirror that, while
        // still being able to materialise the `globalThis.cookieStore`
        // instance via a private symbol that's only known to this file.
        const _internalBuild = Symbol("CookieStore.internalBuild");
        class CookieStore extends EventTarget {
            constructor(token) {
                super();
                if (token !== _internalBuild) {
                    throw new TypeError(
                        "Failed to construct 'CookieStore': Illegal constructor"
                    );
                }
            }
            get(_name) { return Promise.resolve(null); }
            getAll(_name) { return Promise.resolve([]); }
            set(_optionsOrName, _value) { return Promise.resolve(); }
            delete(_optionsOrName) { return Promise.resolve(); }
        }
        Object.defineProperty(CookieStore.prototype, Symbol.toStringTag, {
            value: "CookieStore", configurable: true,
        });
        // Override the earlier _illegalCtor binding from interfaces_bootstrap.
        Object.defineProperty(globalThis, "CookieStore", {
            value: CookieStore, configurable: true, writable: true,
        });
        if (_secure()) {
            Object.defineProperty(globalThis, "cookieStore", {
                value: new CookieStore(_internalBuild), configurable: true, enumerable: true,
            });
        }
    }

    // (3) performance.eventCounts — EventCounts Map
    // Spec: https://wicg.github.io/event-timing/#eventcounts
    // Real Chrome 147 pre-populates this with 36 known event-type keys
    // at value 0. Insertion order matches Chromium's EventTypeNames
    // enumeration — anti-bot scripts probe `eventCounts.size > 0` and
    // `Array.from(eventCounts.keys()).slice(0, 10)`. First-10 captured
    // from Playwright MCP confirm: pointerdown, touchend, input,
    // keydown, mouseleave, mouseenter, drop, beforeinput, pointerenter,
    // dragend. Phase 7.
    if (globalThis.performance && typeof globalThis.performance.eventCounts === "undefined") {
        class EventCounts {
            constructor() { this._inner = new Map(); }
            get size() { return this._inner.size; }
            get(name) { return this._inner.get(String(name)); }
            has(name) { return this._inner.has(String(name)); }
            entries() { return this._inner.entries(); }
            keys() { return this._inner.keys(); }
            values() { return this._inner.values(); }
            forEach(cb, thisArg) { this._inner.forEach(cb, thisArg); }
            [Symbol.iterator]() { return this._inner[Symbol.iterator](); }
        }
        Object.defineProperty(EventCounts.prototype, Symbol.toStringTag, {
            value: "EventCounts", configurable: true,
        });
        globalThis.EventCounts = EventCounts;
        const _ec = new EventCounts();
        for (const k of [
            "pointerdown", "touchend", "input", "keydown",
            "mouseleave", "mouseenter", "drop", "beforeinput",
            "pointerenter", "dragend", "dragstart", "dragenter",
            "dragover", "dragleave", "drag", "pointerout",
            "pointerleave", "pointercancel", "pointermove", "pointerup",
            "pointerover", "wheel", "click", "auxclick",
            "contextmenu", "dblclick", "mousedown", "mouseup",
            "mousemove", "mouseout", "mouseover", "keyup",
            "keypress", "compositionstart", "compositionupdate", "compositionend",
        ]) {
            _ec._inner.set(k, 0);
        }
        // Mount on Performance.prototype, not the instance — real Chrome
        // exposes eventCounts as a prototype getter so
        // Object.getOwnPropertyNames(performance) is empty. If we set it
        // as an own property, fingerprint scripts that count
        // performance's own props (CreepJS) flag it as "modified
        // performance".
        try {
            const _PerfProto = globalThis.Performance && globalThis.Performance.prototype
                ? globalThis.Performance.prototype
                : Object.getPrototypeOf(globalThis.performance);
            Object.defineProperty(_PerfProto, "eventCounts", {
                get: () => _ec, configurable: true, enumerable: true,
            });
        } catch (_e) {}
    }

    // (4) Notification.requestPermission — upgrade the existing minimal
    // Notification class to a full constructor + Promise-returning
    // requestPermission. Real Chrome's requestPermission returns a
    // Promise that resolves to "default" / "granted" / "denied".
    // Both Promise and legacy callback forms are supported per spec.
    {
        class Notification extends EventTarget {
            constructor(title, options) {
                super();
                if (arguments.length === 0) {
                    throw new TypeError("Failed to construct 'Notification': 1 argument required, but only 0 present.");
                }
                this.title = String(title);
                this.dir = (options && options.dir) || "auto";
                this.lang = (options && options.lang) || "";
                this.body = (options && options.body) || "";
                this.tag = (options && options.tag) || "";
                this.icon = (options && options.icon) || "";
                this.image = (options && options.image) || "";
                this.badge = (options && options.badge) || "";
                this.data = (options && options.data) ?? null;
                this.silent = (options && options.silent) ?? null;
                this.requireInteraction = !!(options && options.requireInteraction);
                this.actions = (options && options.actions) || [];
                this.timestamp = Date.now();
                this.onclick = null;
                this.onerror = null;
                this.onclose = null;
                this.onshow = null;
            }
            close() {}
        }
        Object.defineProperty(Notification.prototype, Symbol.toStringTag, {
            value: "Notification", configurable: true,
        });
        // Phase 7 — Chrome's "default" on secure contexts; "denied"
        // on insecure (data:/http:/about:blank) per Notification API
        // spec which gates the prompt UI on secure context.
        Object.defineProperty(Notification, "permission", {
            get: () => _secure() ? "default" : "denied", configurable: true,
        });
        Object.defineProperty(Notification, "maxActions", {
            value: 2, configurable: true,
        });
        Notification.requestPermission = ({
            requestPermission(deprecatedCallback) {
                // Always resolves to "default" — we never actually grant; matches
                // headless Chrome behaviour and avoids a tell when the user
                // never clicks the (non-existent) browser permission UI.
                const result = "default";
                const promise = Promise.resolve(result);
                // Legacy callback form support (Notification spec § Permission).
                if (typeof deprecatedCallback === "function") {
                    Promise.resolve().then(() => {
                        try { deprecatedCallback(result); } catch (_e) {}
                    });
                }
                return promise;
            }
        }).requestPermission;
        _maskFunction(Notification.requestPermission, "requestPermission");
        _maskFunction(Notification, "Notification");
        globalThis.Notification = Notification;
    }

    // (5) ApplePaySession — macOS/iOS only surface.
    (() => {
        const ApplePaySession = ({
            ApplePaySession() { throw new TypeError("Illegal constructor"); }
        }).ApplePaySession;
        ApplePaySession.canMakePayments = ({ canMakePayments() { return true; } }).canMakePayments;
        ApplePaySession.canMakePaymentsWithActiveCard = ({ canMakePaymentsWithActiveCard() { return Promise.resolve(true); } }).canMakePaymentsWithActiveCard;
        ApplePaySession.supportsVersion = ({ supportsVersion() { return true; } }).supportsVersion;
        _maskFunction(ApplePaySession, 'ApplePaySession');
        _maskFunction(ApplePaySession.canMakePayments, 'canMakePayments');
        _maskFunction(ApplePaySession.canMakePaymentsWithActiveCard, 'canMakePaymentsWithActiveCard');
        _maskFunction(ApplePaySession.supportsVersion, 'supportsVersion');
        Object.defineProperty(globalThis, 'ApplePaySession', {
            get: () => _p("os_name", "") === "macOS" ? ApplePaySession : undefined,
            configurable: true, enumerable: false
        });
    })();


    // (6) IdleDetector — User Idle Detection API (Chrome 94+)
    // Spec: https://wicg.github.io/idle-detection/
    // [SecureContext] — undefined on insecure contexts. Phase 7.
    // Chrome-only — real Safari has no IdleDetector. Skip on iOS.
    if (!_isMobileIOS() && _secure() && typeof globalThis.IdleDetector === "undefined") {
        class IdleDetector extends EventTarget {
            constructor() {
                super();
                this.userState = null;
                this.screenState = null;
                this.onchange = null;
            }
            start(_options) { return Promise.reject(new DOMException("Not allowed", "NotAllowedError")); }
            abort() {}
        }
        Object.defineProperty(IdleDetector.prototype, Symbol.toStringTag, {
            value: "IdleDetector", configurable: true,
        });
        IdleDetector.requestPermission = function requestPermission() {
            return Promise.resolve("default");
        };
        if (typeof _maskFunction === "function") {
            _maskFunction(IdleDetector.requestPermission, "requestPermission");
        }
        globalThis.IdleDetector = IdleDetector;
    }

    // (6) EyeDropper — Color picker constructor (Chrome 95+)
    // Spec: https://wicg.github.io/eyedropper-api/
    // [SecureContext] — undefined on insecure contexts. Phase 7.
    if (_secure() && typeof globalThis.EyeDropper === "undefined") {
        class EyeDropper {
            constructor() {}
            open(_options) {
                return Promise.reject(new DOMException("The user canceled the selection", "AbortError"));
            }
        }
        Object.defineProperty(EyeDropper.prototype, Symbol.toStringTag, {
            value: "EyeDropper", configurable: true,
        });
        globalThis.EyeDropper = EyeDropper;
    }

    // (7) navigator.virtualKeyboard — Virtual Keyboard API (Chrome 94+)
    // Spec: https://w3c.github.io/virtual-keyboard/
    {
        class VirtualKeyboard extends EventTarget {
            constructor() {
                super();
                this._overlaysContent = false;
                this._boundingRect = { x: 0, y: 0, width: 0, height: 0, top: 0, right: 0, bottom: 0, left: 0 };
                this.ongeometrychange = null;
            }
            get overlaysContent() { return this._overlaysContent; }
            set overlaysContent(v) { this._overlaysContent = !!v; }
            get boundingRect() { return this._boundingRect; }
            show() {}
            hide() {}
        }
        Object.defineProperty(VirtualKeyboard.prototype, Symbol.toStringTag, {
            value: "VirtualKeyboard", configurable: true,
        });
        globalThis.VirtualKeyboard = VirtualKeyboard;
        try {
            const _vk = new VirtualKeyboard();
            // VirtualKeyboard is [SecureContext]. Phase 7.
            Object.defineProperty(_NavProto, "virtualKeyboard", {
                get: () => _secure() ? _vk : undefined, configurable: true, enumerable: true,
            });
        } catch (_e) {}
    }

    // (8) navigator.devicePosture — Device Posture API (Chrome 132+)
    // Spec: https://w3c.github.io/device-posture/
    {
        class DevicePosture extends EventTarget {
            constructor() {
                super();
                this._type = "continuous"; // Desktop default
                this.onchange = null;
            }
            get type() { return this._type; }
        }
        Object.defineProperty(DevicePosture.prototype, Symbol.toStringTag, {
            value: "DevicePosture", configurable: true,
        });
        globalThis.DevicePosture = DevicePosture;
        try {
            const _dp = new DevicePosture();
            // DevicePosture is [SecureContext]. Phase 7.
            Object.defineProperty(_NavProto, "devicePosture", {
                get: () => _secure() ? _dp : undefined, configurable: true, enumerable: true,
            });
        } catch (_e) {}
    }

    // (9) navigator.windowControlsOverlay — PWA Window Controls Overlay
    // Spec: https://wicg.github.io/window-controls-overlay/
    // Outside an installed PWA context this object exists with
    // visible:false and an empty rect — match that.
    {
        class WindowControlsOverlay extends EventTarget {
            constructor() {
                super();
                this._visible = false;
                this.ongeometrychange = null;
            }
            get visible() { return this._visible; }
            getTitlebarAreaRect() {
                return { x: 0, y: 0, width: 0, height: 0, top: 0, right: 0, bottom: 0, left: 0,
                         toJSON() { return {x:0,y:0,width:0,height:0,top:0,right:0,bottom:0,left:0}; } };
            }
        }
        Object.defineProperty(WindowControlsOverlay.prototype, Symbol.toStringTag, {
            value: "WindowControlsOverlay", configurable: true,
        });
        globalThis.WindowControlsOverlay = WindowControlsOverlay;
        try {
            const _wco = new WindowControlsOverlay();
            Object.defineProperty(_NavProto, "windowControlsOverlay", {
                get: () => _wco, configurable: true, enumerable: true,
            });
        } catch (_e) {}
    }

    // (10) Document.prototype.startViewTransition — View Transitions API
    // Spec: https://drafts.csswg.org/css-view-transitions-1/
    // Real Chrome 111+ exposes this method on Document.prototype.
    if (globalThis.Document && typeof globalThis.Document.prototype.startViewTransition === "undefined") {
        class ViewTransition {
            constructor(updateCallback) {
                // The view-transition lifecycle: ready Promise resolves
                // when the snapshot is ready; finished resolves after
                // the transition completes. updateCallbackDone resolves
                // after the user's callback finishes.
                let cbResult = Promise.resolve();
                if (typeof updateCallback === "function") {
                    try { cbResult = Promise.resolve(updateCallback()); }
                    catch (e) { cbResult = Promise.reject(e); }
                }
                this.updateCallbackDone = cbResult;
                // No real animation in headless — resolve immediately.
                this.ready = cbResult.then(() => {});
                this.finished = cbResult.then(() => {});
                this.types = new Set();
            }
            skipTransition() {}
        }
        Object.defineProperty(ViewTransition.prototype, Symbol.toStringTag, {
            value: "ViewTransition", configurable: true,
        });
        globalThis.ViewTransition = ViewTransition;
        const _startViewTransition = function startViewTransition(updateCallback) {
            return new ViewTransition(updateCallback);
        };
        if (typeof _maskFunction === "function") {
            _maskFunction(_startViewTransition, "startViewTransition");
        }
        Object.defineProperty(globalThis.Document.prototype, "startViewTransition", {
            value: _startViewTransition, configurable: true, writable: true,
        });
    }

    // (11) Document.prototype.hasStorageAccess / requestStorageAccess (Storage Access API)
    // Chrome 130+. Cross-site trackers probe these heavily.
    if (globalThis.Document && typeof globalThis.Document.prototype.hasStorageAccess === "undefined") {
        const _hasStorageAccess = function hasStorageAccess() { return Promise.resolve(false); };
        const _requestStorageAccess = function requestStorageAccess() { 
            return Promise.reject(new DOMException("The request was denied.", "NotAllowedError")); 
        };
        if (typeof _maskFunction === "function") {
            _maskFunction(_hasStorageAccess, "hasStorageAccess");
            _maskFunction(_requestStorageAccess, "requestStorageAccess");
        }
        Object.defineProperty(globalThis.Document.prototype, "hasStorageAccess", {
            value: _hasStorageAccess, configurable: true, writable: true,
        });
        Object.defineProperty(globalThis.Document.prototype, "requestStorageAccess", {
            value: _requestStorageAccess, configurable: true, writable: true,
        });
    }

    // (12) Document.prototype.hasPrivateToken / hasRedemptionRecord (Trust Tokens API)
    // Chrome 130+ ad-fraud prevention APIs. Absence is a headless tell.
    if (globalThis.Document && typeof globalThis.Document.prototype.hasPrivateToken === "undefined") {
        const _hasPrivateToken = function hasPrivateToken() { 
            return Promise.reject(new DOMException("The Trust Token API is not supported.", "NotSupportedError"));
        };
        const _hasRedemptionRecord = function hasRedemptionRecord() {
            return Promise.reject(new DOMException("The Trust Token API is not supported.", "NotSupportedError"));
        };
        if (typeof _maskFunction === "function") {
            _maskFunction(_hasPrivateToken, "hasPrivateToken");
            _maskFunction(_hasRedemptionRecord, "hasRedemptionRecord");
        }
        Object.defineProperty(globalThis.Document.prototype, "hasPrivateToken", {
            value: _hasPrivateToken, configurable: true, writable: true,
        });
        Object.defineProperty(globalThis.Document.prototype, "hasRedemptionRecord", {
            value: _hasRedemptionRecord, configurable: true, writable: true,
        });
    }

    // (13) PaymentRequest — Payment Request API (W3C Recommendation, Sept 2022).
    // Spec: https://www.w3.org/TR/payment-request/
    // [SecureContext] — undefined on insecure contexts. cleanup_bootstrap.js
    // already deletes "PaymentRequest" from globalThis on insecure pages.
    //
    // canMakePayment() resolves true for "https://google.com/pay" and
    // "basic-card" methods — matches real Chrome with no enrolled card
    // (handler is registered, instrument is not). hasEnrolledInstrument()
    // resolves false: Chrome/Edge-only method that mirrors a fresh profile.
    // No public stealth framework ships PaymentRequest; anti-bot scripts
    // (Stripe, Sift, e-commerce sensors) feature-detect it as a real-browser
    // signal even when they don't drive the show() flow.
    //
    // interfaces_bootstrap.js installs an illegal-constructor stub first,
    // so we check for the .canMakePayment method (only present on a real
    // implementation) rather than typeof === undefined.
    const _PRStub = globalThis.PaymentRequest;
    if (_secure() && (typeof _PRStub !== "function"
        || typeof (_PRStub.prototype && _PRStub.prototype.canMakePayment) !== "function")) {
        class PaymentRequest extends EventTarget {
            #methods;
            constructor(methodData, details, options = {}) {
                super();
                if (!Array.isArray(methodData) || methodData.length === 0) {
                    throw new TypeError("Failed to construct 'PaymentRequest': At least one payment method is required");
                }
                if (!details || !details.total) {
                    throw new TypeError("Failed to construct 'PaymentRequest': required member total is undefined.");
                }
                this.#methods = methodData;
                const _id = (details && details.id)
                    || (globalThis.crypto && typeof globalThis.crypto.randomUUID === "function"
                        ? globalThis.crypto.randomUUID()
                        : Date.now().toString(36) + Math.random().toString(36).slice(2));
                Object.defineProperty(this, "id", { value: _id, enumerable: true, configurable: true });
                Object.defineProperty(this, "shippingAddress", { value: null, enumerable: true, configurable: true });
                Object.defineProperty(this, "shippingOption", { value: null, enumerable: true, configurable: true });
                Object.defineProperty(this, "shippingType", { value: null, enumerable: true, configurable: true });
                this.onshippingaddresschange = null;
                this.onshippingoptionchange = null;
                this.onpaymentmethodchange = null;
            }
            show(_detailsPromise) {
                // Real Chrome requires a user gesture and a registered
                // merchant. Without them, show() rejects with AbortError.
                // DOMException is unreliable at snapshot time (snapshot
                // builds don't always have it available); use Error with
                // .name set, which detectors check via e.name === "AbortError".
                const err = new Error("User closed the Payment Request UI.");
                err.name = "AbortError";
                return Promise.reject(err);
            }
            abort() {
                return Promise.resolve(undefined);
            }
            canMakePayment() {
                const ok = this.#methods.some(m =>
                    m && (m.supportedMethods === "https://google.com/pay"
                        || m.supportedMethods === "basic-card")
                );
                return Promise.resolve(ok);
            }
            hasEnrolledInstrument() {
                return Promise.resolve(false);
            }
        }
        Object.defineProperty(PaymentRequest.prototype, Symbol.toStringTag, {
            value: "PaymentRequest", configurable: true,
        });
        PaymentRequest.securePaymentConfirmationAvailability = function securePaymentConfirmationAvailability() {
            return Promise.resolve("unavailable-no-user-verifying-platform-authenticator");
        };
        if (typeof _maskFunction === "function") {
            _maskFunction(PaymentRequest, "PaymentRequest");
            _maskFunction(PaymentRequest.prototype.show, "show");
            _maskFunction(PaymentRequest.prototype.abort, "abort");
            _maskFunction(PaymentRequest.prototype.canMakePayment, "canMakePayment");
            _maskFunction(PaymentRequest.prototype.hasEnrolledInstrument, "hasEnrolledInstrument");
            _maskFunction(PaymentRequest.securePaymentConfirmationAvailability, "securePaymentConfirmationAvailability");
        }
        globalThis.PaymentRequest = PaymentRequest;

        class PaymentResponse extends EventTarget {
            constructor() {
                super();
                throw new TypeError("Illegal constructor");
            }
        }
        Object.defineProperty(PaymentResponse.prototype, Symbol.toStringTag, {
            value: "PaymentResponse", configurable: true,
        });
        if (typeof _maskFunction === "function") {
            _maskFunction(PaymentResponse, "PaymentResponse");
        }
        globalThis.PaymentResponse = PaymentResponse;

        class PaymentMethodChangeEvent extends Event {
            constructor(type, init) {
                super(type, init || {});
                const _init = init || {};
                Object.defineProperty(this, "methodName", { value: _init.methodName || "", enumerable: true, configurable: true });
                Object.defineProperty(this, "methodDetails", { value: _init.methodDetails || null, enumerable: true, configurable: true });
            }
        }
        Object.defineProperty(PaymentMethodChangeEvent.prototype, Symbol.toStringTag, {
            value: "PaymentMethodChangeEvent", configurable: true,
        });
        if (typeof _maskFunction === "function") {
            _maskFunction(PaymentMethodChangeEvent, "PaymentMethodChangeEvent");
        }
        globalThis.PaymentMethodChangeEvent = PaymentMethodChangeEvent;

        class PaymentRequestUpdateEvent extends Event {
            constructor(type, init) {
                super(type, init || {});
            }
            updateWith(_detailsPromise) {
                // Outside an active show() flow this silently no-ops,
                // matching Chrome behavior on stale events.
            }
        }
        Object.defineProperty(PaymentRequestUpdateEvent.prototype, Symbol.toStringTag, {
            value: "PaymentRequestUpdateEvent", configurable: true,
        });
        if (typeof _maskFunction === "function") {
            _maskFunction(PaymentRequestUpdateEvent, "PaymentRequestUpdateEvent");
            _maskFunction(PaymentRequestUpdateEvent.prototype.updateWith, "updateWith");
        }
        globalThis.PaymentRequestUpdateEvent = PaymentRequestUpdateEvent;
    }

    // (14) navigator.getInstalledRelatedApps — Get Installed Related Apps API.
    // Spec: https://wicg.github.io/get-installed-related-apps/
    // Chrome/Edge-only. Returns Promise<[]> on a fresh profile (no PWAs
    // installed). Absence under a Chrome UA is itself a tell — anti-bot
    // scripts can probe `'getInstalledRelatedApps' in navigator` against
    // the UA family. Skip on iOS (Safari has no such method).
    if (!_isMobileIOS() && typeof navigator !== "undefined" && typeof navigator.getInstalledRelatedApps !== "function") {
        _defNavMethod("getInstalledRelatedApps", function getInstalledRelatedApps() {
            return Promise.resolve([]);
        });
    }

    // fetch(), Headers, Request, Response are now provided by fetch_bootstrap.js
    // (wired to real net::HttpClient via op_fetch)

    // NOTE: secure-context API gating is split:
    // - Navigator getters (mediaDevices, clipboard, ...) lazily check
    //   _secure() at access time — they work directly off the snapshot.
    // - Globals + getBattery are always registered into the snapshot
    //   (snapshot bootstraps with is_secure_context=true) and then
    //   stripped per-page in cleanup_bootstrap.js when the actual page
    //   URL is insecure.
    // (Phase J) High-ROI Parity Gaps: VirtualKeyboard, DevicePosture, WindowControlsOverlay
    {
        class VirtualKeyboard extends EventTarget {
            constructor() { super(); }
            get boundingRect() { return { x: 0, y: 0, width: 0, height: 0 }; } // Avoid DOMRect dependency
            get overlaysContent() { return false; }
            show() {}
            hide() {}
        }
        Object.defineProperty(VirtualKeyboard.prototype, Symbol.toStringTag, {
            value: "VirtualKeyboard", configurable: true,
        });
        globalThis.VirtualKeyboard = VirtualKeyboard;
        const _vk = new VirtualKeyboard();
        _defNav('virtualKeyboard', () => _vk);
    }

    {
        class DevicePosture extends EventTarget {
            constructor() { super(); }
            get type() { return "continuous"; }
        }
        Object.defineProperty(DevicePosture.prototype, Symbol.toStringTag, {
            value: "DevicePosture", configurable: true,
        });
        globalThis.DevicePosture = DevicePosture;
        const _dp = new DevicePosture();
        _defNav('devicePosture', () => _dp);
    }

    {
        class WindowControlsOverlay extends EventTarget {
            constructor() { super(); }
            get visible() { return false; }
            getTitlebarAreaRect() { return { x: 0, y: 0, width: 0, height: 0 }; } // Avoid DOMRect dependency
        }
        Object.defineProperty(WindowControlsOverlay.prototype, Symbol.toStringTag, {
            value: "WindowControlsOverlay", configurable: true,
        });
        globalThis.WindowControlsOverlay = WindowControlsOverlay;
        const _wco = new WindowControlsOverlay();
        _defNav('windowControlsOverlay', () => _wco);
    }

    Object.defineProperty(globalThis, 'external', {
        value: {
            AddSearchProvider() {},
            IsSearchProviderInstalled() {},
        },
        configurable: true, enumerable: true, writable: true,
    });
    globalThis.clientInformation = globalThis.navigator;
    globalThis.offscreenBuffering = true;
    globalThis.defaultStatus = "";
    globalThis.name = "";
    globalThis.status = "";
    
    // (Phase J) Iframe indexing parity: define window[0], window[1] etc.
    // Real Chrome has numeric own-properties for each child frame.
    const _defineIframeGetter = (index) => {
        Object.defineProperty(globalThis, index, {
            get: () => {
                // If we have iframes, return the contentWindow of the i-th one.
                // Our Page layer manages the children.
                const iframes = document.querySelectorAll('iframe');
                return iframes[index] ? iframes[index].contentWindow : undefined;
            },
            configurable: true, enumerable: true
        });
    };
    // Pre-define for common counts.
    for (let i = 0; i < 5; i++) _defineIframeGetter(i);

    Object.defineProperty(globalThis, Symbol.toStringTag, { value: "Window", configurable: true });
})(globalThis);
