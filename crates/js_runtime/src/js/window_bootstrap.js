((globalThis) => {
    const ops = Deno.core.ops;
    const _boxide = globalThis.__boxide;

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

    // Helper: read from stealth profile or use default
    const _p = (key, fallback) => {
        if (ops.op_has_stealth_profile()) {
            const v = ops.op_get_profile_value(key);
            return v !== "" ? v : fallback;
        }
        return fallback;
    };
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

    // ================================================================
    // Prototype-install helpers — kNoScriptId-safe layout
    // ================================================================
    const _defProtoGetter = (proto, name, getter, setter) => {
        Object.defineProperty(proto, name, {
            get: getter,
            set: setter,
            enumerable: true,
            configurable: true,
        });
        _maskFunction(getter, `get ${name}`);
        if (setter) _maskFunction(setter, `set ${name}`);
    };
    const _defProtoMethod = (proto, name, fn) => {
        Object.defineProperty(proto, name, {
            value: fn, writable: true, enumerable: true, configurable: true,
        });
        _maskFunction(fn, name);
    };

    // ================================================================
    // Navigator class + prototype — kNoScriptId-safe layout
    // ================================================================
    const _NavProto = globalThis.Navigator.prototype;
    const _defNav = (name, getter) => _defProtoGetter(_NavProto, name, getter);
    const _defNavMethod = (name, fn) => _defProtoMethod(_NavProto, name, fn);

    // Stable-object references — object getters return the same reference
    // on every call, matching the behavior of real DOM-wrapped properties.
    let NetworkInformation = globalThis.NetworkInformation || class NetworkInformation {};
    const _navConnection = Object.create(NetworkInformation.prototype);
    Object.defineProperty(_navConnection, 'effectiveType', { get: () => _p("connection_effective_type", "4g"), enumerable: true });
    Object.defineProperty(_navConnection, 'rtt', { get: () => Math.round(_pInt("connection_rtt", 50) / 25) * 25, enumerable: true });
    Object.defineProperty(_navConnection, 'downlink', { get: () => Math.round(_pFloat("connection_downlink", 10) * 40) / 40, enumerable: true });
    Object.defineProperty(_navConnection, 'saveData', { get: () => false, enumerable: true });
    Object.defineProperty(_navConnection, 'downlinkMax', { get: () => Infinity, enumerable: true });
    _navConnection.addEventListener = function addEventListener() {};
    _navConnection.removeEventListener = function removeEventListener() {};
    _navConnection.dispatchEvent = function dispatchEvent() { return true; };
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
    const _pluginsLen = () => Math.max(0, Math.min(_allPlugins.length, _pInt("plugins_count", _allPlugins.length)));
    const _mimesLen = () => Math.max(0, Math.min(_allMimes.length, _pInt("mime_types_count", _allMimes.length)));

    // 2. Setup PluginArray.prototype — length + item() dispatch via live count.
    const _PluginArrayProto = PluginArray.prototype;
    Object.defineProperty(_PluginArrayProto, Symbol.toStringTag, { value: "PluginArray", enumerable: false, configurable: true });
    Object.defineProperty(_PluginArrayProto, 'length', { get: () => _pluginsLen(), enumerable: true, configurable: true });
    _defProtoMethod(_PluginArrayProto, 'item', function item(i) {
        const n = _pluginsLen();
        return (i >= 0 && i < n) ? _allPlugins[i] : null;
    });
    _defProtoMethod(_PluginArrayProto, 'namedItem', function namedItem(n) {
        const len = _pluginsLen();
        for (let i = 0; i < len; i++) if (_allPlugins[i].name === n) return _allPlugins[i];
        return null;
    });
    _defProtoMethod(_PluginArrayProto, 'refresh', () => {});
    // Symbol.iterator iterates the live sliced range.
    Object.defineProperty(_PluginArrayProto, Symbol.iterator, {
        value: function* iter() {
            const n = _pluginsLen();
            for (let i = 0; i < n; i++) yield _allPlugins[i];
        },
        configurable: true,
    });

    // Setup MimeTypeArray.prototype — same pattern.
    const _MimeTypeArrayProto = MimeTypeArray.prototype;
    Object.defineProperty(_MimeTypeArrayProto, Symbol.toStringTag, { value: "MimeTypeArray", enumerable: false, configurable: true });
    Object.defineProperty(_MimeTypeArrayProto, 'length', { get: () => _mimesLen(), enumerable: true, configurable: true });
    _defProtoMethod(_MimeTypeArrayProto, 'item', function item(i) {
        const n = _mimesLen();
        return (i >= 0 && i < n) ? _allMimes[i] : null;
    });
    _defProtoMethod(_MimeTypeArrayProto, 'namedItem', function namedItem(n) {
        const len = _mimesLen();
        for (let i = 0; i < len; i++) if (_allMimes[i].type === n) return _allMimes[i];
        return null;
    });
    Object.defineProperty(_MimeTypeArrayProto, Symbol.iterator, {
        value: function* iter() {
            const n = _mimesLen();
            for (let i = 0; i < n; i++) yield _allMimes[i];
        },
        configurable: true,
    });

    // Instance: install numeric index accessors that gate on live count.
    const _navPlugins = Object.create(_PluginArrayProto);
    _allPlugins.forEach((p, i) => {
        Object.defineProperty(_navPlugins, i, {
            get: () => (i < _pluginsLen() ? p : undefined),
            enumerable: true,
            configurable: true,
        });
    });

    // Plugin instance behaves like a MimeTypeArray over its mime types.
    _allPlugins.forEach(p => {
        Object.defineProperty(p, 'length', { get: () => _mimesLen(), enumerable: true, configurable: true });
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
        p._mimeTypes.forEach((m, i) => {
            Object.defineProperty(p, i, {
                get: () => (i < _mimesLen() ? m : undefined),
                enumerable: true, configurable: true,
            });
        });
    });

    const _navMimeTypes = Object.create(_MimeTypeArrayProto);
    _allMimes.forEach((m, i) => {
        Object.defineProperty(_navMimeTypes, i, {
            get: () => (i < _mimesLen() ? m : undefined),
            enumerable: true, configurable: true,
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
    _navMediaDevices.enumerateDevices = function enumerateDevices() {
        const raw = _pJson("media_devices", []);
        const permFor = (kind) => {
            if (kind === "videoinput") return _PERMISSION_STATE_MAP["camera"] || "prompt";
            if (kind === "audioinput" || kind === "audiooutput") return _PERMISSION_STATE_MAP["microphone"] || "prompt";
            return "granted"; // unknown kinds — don't blank
        };
        const out = raw.map((d) => {
            const deviceId = d.deviceId != null ? d.deviceId : (d.device_id || "");
            const groupId = d.groupId != null ? d.groupId : (d.group_id || "");
            const label = permFor(d.kind) === "granted" ? (d.label || "") : "";
            return { deviceId, kind: d.kind || "", label, groupId };
        });
        return Promise.resolve(out);
    };
    _navMediaDevices.getUserMedia = function () { return Promise.reject(new Error("Permission denied")); };
    _navMediaDevices.getDisplayMedia = function () { return Promise.reject(new Error("Permission denied")); };
    _navMediaDevices.getSupportedConstraints = function () {
        return { aspectRatio: true, autoGainControl: true, brightness: true, channelCount: true, colorTemperature: true, contrast: true, deviceId: true, displaySurface: true, echoCancellation: true, exposureCompensation: true, exposureMode: true, exposureTime: true, facingMode: true, focusDistance: true, focusMode: true, frameRate: true, groupId: true, height: true, iso: true, latency: true, noiseSuppression: true, pan: true, pointsOfInterest: true, resizeMode: true, sampleRate: true, sampleSize: true, saturation: true, sharpness: true, suppressLocalAudioPlayback: true, tilt: true, torch: true, whiteBalanceMode: true, width: true, zoom: true };
    };
    _navMediaDevices.addEventListener = function () {};
    _navMediaDevices.removeEventListener = function () {};
    _navMediaDevices.dispatchEvent = function () { return true; };

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

    class PermissionStatus {
        constructor(name) { this._name = name; }
        get name() { return this._name; }
        get state() {
            return _PERMISSION_STATE_MAP[this._name] || "prompt";
        }
        get onchange() { return null; }
        set onchange(_v) {}
        addEventListener() {}
        removeEventListener() {}
        dispatchEvent() { return true; }
    }
    Object.defineProperty(PermissionStatus.prototype, Symbol.toStringTag, {
        value: "PermissionStatus", configurable: true,
    });
    globalThis.PermissionStatus = PermissionStatus;

    class Permissions {}
    Object.defineProperty(Permissions.prototype, Symbol.toStringTag, {
        value: "Permissions", configurable: true,
    });
    Permissions.prototype.query = function query(desc) {
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
    };
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
    // resolved values. See docs/SOTA_ROADMAP_2026.md §1.1.
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
    PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable = function () {
        return Promise.resolve(_p("has_platform_authenticator", "false") === "true");
    };
    PublicKeyCredential.isConditionalMediationAvailable = function () {
        return Promise.resolve(_p("conditional_mediation", "true") === "true");
    };
    // Chrome 133+ surface — see web.dev/articles/webauthn-client-capabilities.
    PublicKeyCredential.getClientCapabilities = function () {
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
    };
    globalThis.PublicKeyCredential = PublicKeyCredential;

    class IdentityCredential {
        constructor() { throw new TypeError("Illegal constructor"); }
    }
    Object.defineProperty(IdentityCredential.prototype, Symbol.toStringTag,
        { value: "IdentityCredential", configurable: true });
    globalThis.IdentityCredential = IdentityCredential;

    class IdentityProvider {}
    IdentityProvider.getUserInfo = function () {
        return Promise.reject(new DOMException("Not allowed", "NotAllowedError"));
    };
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
    CredentialsContainer.prototype.create = function (opts) {
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
    };
    CredentialsContainer.prototype.get = function (opts) {
        if (opts && opts.identity) return _fedcmGet(opts.identity);
        if (opts && opts.publicKey) {
            return new Promise((_, rej) => setTimeout(() =>
                rej(new DOMException(
                    "The operation either timed out or was not allowed; see https://www.w3.org/TR/webauthn-2/#sctn-privacy-considerations-client.",
                    "NotAllowedError")), 120));
        }
        return Promise.resolve(null);
    };
    CredentialsContainer.prototype.store = function () { return Promise.resolve(undefined); };
    CredentialsContainer.prototype.preventSilentAccess = function () { return Promise.resolve(undefined); };
    globalThis.CredentialsContainer = CredentialsContainer;
    const _navCredentials = Object.create(CredentialsContainer.prototype);

    _maskAsNative(PublicKeyCredential, 'isUserVerifyingPlatformAuthenticatorAvailable',
        'isConditionalMediationAvailable', 'getClientCapabilities');
    _maskAsNative(IdentityProvider, 'getUserInfo');
    _maskAsNative(CredentialsContainer.prototype, 'create', 'get', 'store', 'preventSilentAccess');

    class Bluetooth {}
    Object.defineProperty(Bluetooth.prototype, Symbol.toStringTag, {
        value: "Bluetooth", configurable: true,
    });
    Bluetooth.prototype.getAvailability = function () { return Promise.resolve(false); };
    Bluetooth.prototype.requestDevice = function () { return Promise.reject(new DOMException("User denied", "NotFoundError")); };
    Bluetooth.prototype.addEventListener = function () {};
    Bluetooth.prototype.removeEventListener = function () {};
    Bluetooth.prototype.dispatchEvent = function () { return true; };
    globalThis.Bluetooth = Bluetooth;
    const _navBluetooth = Object.create(Bluetooth.prototype);

    const _navUsb = {};
    const _navSerial = {};
    const _navHid = {};
    const _navLocks = {};

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
        [Symbol.iterator]() { return this._m[Symbol.iterator](); }
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
    StorageManager.prototype.estimate = function () {
        return Promise.resolve({ quota: 1073741824, usage: 0 });
    };
    StorageManager.prototype.persist = function () { return Promise.resolve(false); };
    StorageManager.prototype.persisted = function () { return Promise.resolve(false); };
    globalThis.StorageManager = StorageManager;
    const _navStorage = Object.create(StorageManager.prototype);

    class ServiceWorkerContainer {
        constructor() {
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
    ServiceWorkerContainer.prototype.register = function (scriptURL, options) {
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
    };
    ServiceWorkerContainer.prototype.getRegistrations = function () { return Promise.resolve([]); };
    ServiceWorkerContainer.prototype.getRegistration = function () { return Promise.resolve(undefined); };
    ServiceWorkerContainer.prototype.startMessages = function () {};
    ServiceWorkerContainer.prototype.addEventListener = function () {};
    ServiceWorkerContainer.prototype.removeEventListener = function () {};
    globalThis.ServiceWorkerContainer = ServiceWorkerContainer;
    const _navServiceWorker = new ServiceWorkerContainer();
    const _navClipboard = { readText() { return Promise.resolve(""); }, writeText() { return Promise.resolve(); } };
    const _navGeolocation = {};
    const _navWakeLock = {};
    const _navMediaSession = {};
    const _navScheduling = { isInputPending() { return false; } };
    const _navUserActivation = { isActive: false, hasBeenActive: false };
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
    _defNav('deviceMemory', () => _pInt("device_memory", 8));
    _defNav('maxTouchPoints', () => _pInt("max_touch_points", 0));
    _defNav('pdfViewerEnabled', () => true);
    _defNav('webdriver', () => false);
    _defNav('doNotTrack', () => null);
    _defNav('msDoNotTrack', () => undefined);
    _defNav('loadPurpose', () => undefined);
    _defNav('sayswho', () => undefined);

    // Object getters — stable references.
    Object.defineProperty(_NavProto, 'connection', { get: () => _navConnection, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'plugins', { get: () => _navPlugins, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'mimeTypes', { get: () => _navMimeTypes, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'mediaDevices', { get: () => _navMediaDevices, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'permissions', { get: () => _navPermissions, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'credentials', { get: () => _navCredentials, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'bluetooth', { get: () => _navBluetooth, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'usb', { get: () => _navUsb, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'serial', { get: () => _navSerial, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'hid', { get: () => _navHid, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'keyboard', { get: () => _navKeyboard, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'locks', { get: () => _navLocks, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'storage', { get: () => _navStorage, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'serviceWorker', { get: () => _navServiceWorker, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'clipboard', { get: () => _navClipboard, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'geolocation', { get: () => _navGeolocation, enumerable: true, configurable: true });
    Object.defineProperty(_NavProto, 'wakeLock', { get: () => _navWakeLock, enumerable: true, configurable: true });

    // Apply native masking to all getters
    _maskAsNative(_NavProto, 'userAgent', 'platform', 'vendor', 'vendorSub', 'productSub', 
        'appVersion', 'appCodeName', 'appName', 'product', 'language', 'languages', 
        'onLine', 'cookieEnabled', 'hardwareConcurrency', 'deviceMemory', 'maxTouchPoints', 
        'pdfViewerEnabled', 'webdriver', 'connection', 'plugins', 'mimeTypes', 
        'mediaDevices', 'permissions', 'credentials', 'bluetooth', 'usb', 'serial', 
        'hid', 'keyboard', 'locks', 'storage', 'serviceWorker', 'clipboard', 
        'geolocation', 'wakeLock');
    _defNav('mediaSession', () => _navMediaSession);
    _defNav('scheduling', () => _navScheduling);
    _defNav('userActivation', () => _navUserActivation);

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
    _defNavMethod('getBattery', function getBattery() {
        return Promise.resolve({
            charging: true, chargingTime: 0, dischargingTime: Infinity, level: 1.0,
            addEventListener() {}, removeEventListener() {},
            onchargingchange: null, onchargingtimechange: null,
            ondischargingtimechange: null, onlevelchange: null,
        });
    });
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
        try {
            const u = new URL(url, _locationData.href !== "about:blank" ? _locationData.href : undefined);
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
            _locationData.href = String(url);
        }
    }

    // Pending-navigation signal — generic primitive used by the Rust driver
    // to loop through challenge flows. Any location.reload/assign/replace or
    // location.href = ... sets this; <meta http-equiv="refresh"> does too.
    // Matches the behavior of a real browser's navigation algorithm without
    // any per-engine awareness.
    Object.defineProperty(globalThis, '__pendingNavigation', {
        value: null,
        writable: true,
        enumerable: false,
        configurable: true
    });

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

    _defLoc('href', () => _locationData.href, (v) => {
        try { Deno.core.print('[BOOTSTRAP] SETTING LOCATION HREF TO ' + v + '\n'); } catch(e) {}
        _parseLocationUrl(v);
        globalThis.__pendingNavigation = { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('origin', () => _locationData.origin);
    _defLoc('protocol', () => _locationData.protocol, (v) => {
        _parseLocationUrl(v + "//" + _locationData.host + _locationData.pathname);
        globalThis.__pendingNavigation = { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('host', () => _locationData.host, (v) => {
        _parseLocationUrl(_locationData.protocol + "//" + v + _locationData.pathname);
        globalThis.__pendingNavigation = { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('hostname', () => _locationData.hostname, (v) => {
        _parseLocationUrl(_locationData.protocol + "//" + v + (_locationData.port ? ":" + _locationData.port : "") + _locationData.pathname);
        globalThis.__pendingNavigation = { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defLoc('port', () => _locationData.port);
    _defLoc('pathname', () => _locationData.pathname);
    _defLoc('search', () => _locationData.search);
    _defLoc('hash', () => _locationData.hash, (v) => {
        _locationData.hash = String(v).startsWith('#') ? v : '#' + v;
        _locationData.href = _locationData.origin + _locationData.pathname + _locationData.search + _locationData.hash;
    });
    _defLoc('ancestorOrigins', () => ({ length: 0, item: () => null, contains: () => false }));

    _defProtoMethod(_LocProto, 'assign', (url) => {
        _parseLocationUrl(url);
        globalThis.__pendingNavigation = { url: _locationData.href, kind: "assign" };
        _signalNav();
    });
    _defProtoMethod(_LocProto, 'replace', (url) => {
        _parseLocationUrl(url);
        globalThis.__pendingNavigation = { url: _locationData.href, kind: "replace" };
        _signalNav();
    });
    _defProtoMethod(_LocProto, 'reload', () => {
        globalThis.__pendingNavigation = { url: _locationData.href, kind: "reload" };
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
    // (we only emulate a single browsing context). Real browsers: when a page
    // is not in a frame, top === parent === window.
    globalThis.top = globalThis;
    globalThis.parent = globalThis;
    globalThis.frames = globalThis;
    globalThis.opener = null;

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
    _defProtoGetter(_ScreenProto, 'pixelDepth', () => 24);
    _defProtoGetter(_ScreenProto, 'orientation', () => _screenOrientation);
    _defProtoGetter(_ScreenProto, 'isExtended', () => false);
    Object.defineProperty(_ScreenProto, Symbol.toStringTag, { value: "Screen", configurable: true });
    Object.defineProperty(ScreenOrientation.prototype, Symbol.toStringTag, { value: "ScreenOrientation", configurable: true });
    globalThis.screen = Object.create(_ScreenProto);

    // Misc globals anti-bot checks for
    globalThis.isSecureContext = true;
    // crossOriginIsolated must reflect actual COOP+COEP state from the
    // response headers — see crates/net/src/headers.rs and gap #30.
    // Backed by an op so it's true iff the runtime was constructed with
    // BrowserRuntimeOptions { cross_origin_isolated: true, .. }.
    Object.defineProperty(globalThis, 'crossOriginIsolated', {
        get: () => ops.op_cross_origin_isolated(),
        configurable: true,
        enumerable: true,
    });
    globalThis.origin = "null";
    // Window metrics must resolve LAZILY — bootstrap runs at V8-snapshot
    // build time with no profile installed; eager values get baked as
    // defaults and never update when the profile loads.
    Object.defineProperty(globalThis, 'innerWidth',  { get: () => _pInt("inner_width", 1920),   configurable: true });
    Object.defineProperty(globalThis, 'innerHeight', { get: () => _pInt("inner_height", 1080),  configurable: true });
    Object.defineProperty(globalThis, 'outerWidth',  { get: () => _pInt("outer_width", 1920),   configurable: true });
    Object.defineProperty(globalThis, 'outerHeight', { get: () => _pInt("outer_height", 1080),  configurable: true });
    Object.defineProperty(globalThis, 'devicePixelRatio', { get: () => _pFloat("device_pixel_ratio", 1), configurable: true });
    globalThis.screenX = 0;
    globalThis.screenY = 0;
    globalThis.pageXOffset = 0;
    globalThis.pageYOffset = 0;
    globalThis.scrollX = 0;
    globalThis.scrollY = 0;

    globalThis.scrollTo = function(xOrOptions, y) {
        if (typeof xOrOptions === "object" && xOrOptions !== null) {
            globalThis.scrollX = xOrOptions.left || 0;
            globalThis.pageXOffset = globalThis.scrollX;
            globalThis.scrollY = xOrOptions.top || 0;
            globalThis.pageYOffset = globalThis.scrollY;
        } else {
            globalThis.scrollX = xOrOptions || 0;
            globalThis.pageXOffset = globalThis.scrollX;
            globalThis.scrollY = y || 0;
            globalThis.pageYOffset = globalThis.scrollY;
        }
    };
    globalThis.scroll = globalThis.scrollTo;
    globalThis.scrollBy = function(xOrOptions, y) {
        if (typeof xOrOptions === "object" && xOrOptions !== null) {
            globalThis.scrollTo(globalThis.scrollX + (xOrOptions.left || 0), globalThis.scrollY + (xOrOptions.top || 0));
        } else {
            globalThis.scrollTo(globalThis.scrollX + (xOrOptions || 0), globalThis.scrollY + (y || 0));
        }
    };

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
    const _chromeCsi = function csi() {
        return { startE: Date.now(), onloadT: Date.now(), pageT: Date.now(), tran: 15 };
    };
    const _chromeLoadTimes = function loadTimes() {
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
    };
    Object.defineProperty(_chromeCsi, 'toString', {
        value: function toString() { return 'function csi() { [native code] }'; },
        configurable: true,
    });
    Object.defineProperty(_chromeCsi, _nativeTag, { value: 'csi', configurable: true });
    Object.defineProperty(_chromeLoadTimes, 'toString', {
        value: function toString() { return 'function loadTimes() { [native code] }'; },
        configurable: true,
    });
    Object.defineProperty(_chromeLoadTimes, _nativeTag, { value: 'loadTimes', configurable: true });

    // Real Chrome 147 on a regular page (no extensions): {app, csi, loadTimes}
    // chrome.runtime is ONLY present in extension contexts — absent on regular pages.
    // chrome.webstore was removed in Chrome 126.
    // Adding either is a classic bot detection signal (Kasada, Cloudflare, DataDome).
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

    // --- Document visibility/hidden stubs ---
    Object.defineProperty(Document.prototype, 'visibilityState', { get() { return 'visible'; }, enumerable: true, configurable: true });
    Object.defineProperty(Document.prototype, 'hidden', { get() { return false; }, enumerable: true, configurable: true });
    Object.defineProperty(Document.prototype, 'webkitVisibilityState', { get() { return 'visible'; }, enumerable: true, configurable: true });
    Object.defineProperty(Document.prototype, 'webkitHidden', { get() { return false; }, enumerable: true, configurable: true });

    if (globalThis.navigator) {
        // Match Chrome's exact descriptor for webdriver: false, non-enumerable.
        // (All other navigator.* getters are installed once above, reading
        // lazily from the stealth profile — do NOT re-install them here.
        // A duplicate _defNav('languages', () => _pJson(...)) returned a
        // fresh unfrozen array each call, breaking nav_languages_is_frozen.)
        Object.defineProperty(_NavProto, 'webdriver', {
            get: () => false,
            enumerable: false,
            configurable: true
        });

        // navigator.plugins / mimeTypes are defined at the top of this file
        // (search for _allPlugins). Count is driven by profile.plugins_count
        // and profile.mime_types_count. Do not override here.
    }

    if (globalThis.navigator) {
        // ... (webdriver/online/etc)
        _defNav('devicePixelRatio', () => 2.0);
    }

    if (globalThis.Screen) {
        const _ScreenProto = Screen.prototype;
        _defProtoGetter(_ScreenProto, 'availLeft', () => 0);
        _defProtoGetter(_ScreenProto, 'availTop', () => _pInt("screen_avail_top", 0));
        _defProtoGetter(_ScreenProto, 'colorDepth', () => 24);
        _defProtoGetter(_ScreenProto, 'pixelDepth', () => 24);
    }

    globalThis.devicePixelRatio = 2.0;

    // Explicitly define documentMode as undefined to pass 'prop in document' checks quietly
    Object.defineProperty(Document.prototype, 'documentMode', { value: undefined, enumerable: false, configurable: true });

    const _hunt = (obj, name) => {
        return obj;
    };
    globalThis.navigator = _hunt(globalThis.navigator, 'navigator');
    globalThis.document = _hunt(globalThis.document, 'document');
    globalThis.chrome = _hunt(globalThis.chrome, 'chrome');
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

        const _makeLowBrands = () => Object.freeze(_shuffled([
            Object.freeze({ brand: "Chromium", version: _uaBrowserMajor() }),
            Object.freeze({ brand: "Google Chrome", version: _uaBrowserMajor() }),
            Object.freeze({ brand: "Not-A.Brand", version: "24" }),
        ]).map(Object.freeze));
        const _makeFullBrands = () => Object.freeze(_shuffled([
            Object.freeze({ brand: "Chromium", version: _uaBrowserFull() }),
            Object.freeze({ brand: "Google Chrome", version: _uaBrowserFull() }),
            Object.freeze({ brand: "Not-A.Brand", version: "24.0.0.0" }),
        ]).map(Object.freeze));
        // Chrome re-uses the same GREASE ordering across a userAgentData
        // object's lifetime; only randomized once per construction.
        let _lowBrands = null, _fullBrands = null;
        const _lowCached = () => (_lowBrands ||= _makeLowBrands());
        const _fullCached = () => (_fullBrands ||= _makeFullBrands());

        const _allowedHints = new Set([
            "architecture", "bitness", "brands", "fullVersionList",
            "mobile", "model", "platform", "platformVersion",
            "uaFullVersion", "wow64",
        ]);

        const _navUaData = {
            get brands() { return _lowCached(); },
            get mobile() { return false; },
            get platform() { return _uaPlatform(); },
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
                    if (typeof key !== "string" || !_allowedHints.has(key)) continue;
                    switch (key) {
                        case "architecture":      result.architecture = _uaArch(); break;
                        case "bitness":           result.bitness = _uaBitness(); break;
                        case "brands":            result.brands = _lowCached(); break;
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
            },
            toJSON() {
                return {
                    brands: _lowCached().map(b => ({ brand: b.brand, version: b.version })),
                    mobile: false,
                    platform: _uaPlatform(),
                };
            },
        };
        _defNav('userAgentData', () => _navUaData);
    })();

    // Notification
    globalThis.Notification = class Notification { static permission = "default"; };

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
            // Non-blob URLs are not supported in the MVP.
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

                // Drain parent←worker messages every 5ms. We use setInterval
                // so the poll keeps firing while the main event loop is live.
                const self = this;
                this._pollTimer = setInterval(() => {
                    if (!self._id) return;
                    const deserializer =
                        _boxide && _boxide.deserializeFromWire;
                    for (let i = 0; i < 32; i++) {
                        const raw = _wops.op_worker_poll_from_worker(self._id);
                        if (!raw) return;
                        let payload = null;
                        try { payload = JSON.parse(raw); } catch (e) { continue; }
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
                        self._fireEvent('message', event);
                    }
                }, 5);
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
                        (_boxide &&
                            _boxide.serializeForWire &&
                            _boxide.serializeForWire(message)) ||
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
        globalThis.ServiceWorker = class ServiceWorker {
            constructor() {
                this.scriptURL = "";
                this.state = "activated";
                this.onstatechange = null;
            }
            postMessage() {}
            addEventListener() {}
            removeEventListener() {}
            dispatchEvent() { return true; }
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
        globalThis.FileReader = class FileReader {
            static EMPTY = 0;
            static LOADING = 1;
            static DONE = 2;
            constructor() {
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
            addEventListener() {}
            removeEventListener() {}
            dispatchEvent() { return true; }
        };
    }

    if (!globalThis.ImageBitmap) {
        globalThis.ImageBitmap = class ImageBitmap {
            constructor() { this.width = 0; this.height = 0; }
            close() {}
        };
    }
    if (!globalThis.createImageBitmap) {
        globalThis.createImageBitmap = function() { return Promise.resolve(new globalThis.ImageBitmap()); };
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
        globalThis.BroadcastChannel = class BroadcastChannel {
            constructor(name) { this.name = name; this.onmessage = null; this.onmessageerror = null; }
            postMessage() {}
            close() {}
            addEventListener() {}
            removeEventListener() {}
            dispatchEvent() { return true; }
        };
    }

    if (!globalThis.MessageChannel) {
        globalThis.MessageChannel = class MessageChannel {
            constructor() {
                this.port1 = { onmessage: null, postMessage() {}, start() {}, close() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent() { return true; } };
                this.port2 = { onmessage: null, postMessage() {}, start() {}, close() {}, addEventListener() {}, removeEventListener() {}, dispatchEvent() { return true; } };
            }
        };
    }
    if (!globalThis.MessagePort) {
        globalThis.MessagePort = class MessagePort {
            constructor() { this.onmessage = null; this.onmessageerror = null; }
            postMessage() {}
            start() {}
            close() {}
            addEventListener() {}
            removeEventListener() {}
            dispatchEvent() { return true; }
        };
    }

    if (!globalThis.EventSource) {
        globalThis.EventSource = class EventSource {
            static CONNECTING = 0;
            static OPEN = 1;
            static CLOSED = 2;
            constructor(url) {
                this.url = String(url);
                this.readyState = 0;
                this.withCredentials = false;
                this.onopen = null;
                this.onmessage = null;
                this.onerror = null;
            }
            close() { this.readyState = 2; }
            addEventListener() {}
            removeEventListener() {}
            dispatchEvent() { return true; }
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

        let _resourceEntries = null;
        const _buildResourceEntries = () => {
            if (_resourceEntries) return _resourceEntries;
            const base = _perfNav.fetchStart;
            const origin = globalThis.location?.origin || "https://example.com";
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
            _resourceEntries = [
                mk(`${origin}/favicon.ico`, 25, 42, "img", 1024),
                mk(`${origin}/static/main.css`, 30, 55, "link", 12500),
                mk(`${origin}/static/main.js`, 35, 88, "script", 48600),
                mk(`${origin}/static/vendor.js`, 40, 142, "script", 185400),
                mk(`${origin}/static/fonts/Inter.woff2`, 60, 32, "css", 24800),
            ];
            return _resourceEntries;
        };
        const _navEntry = () => {
            _perfNav.duration = Math.max(performance.now(), _perfNav.loadEventEnd);
            return _perfNav;
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
        _defProtoGetter(_PerfProto, 'timing', () => _perfTiming);
        _defProtoGetter(_PerfProto, 'timeOrigin', () => _perfTimingStart);
        _defProtoGetter(_PerfProto, 'navigation', () => _perfNavigation);
        _defProtoGetter(_PerfProto, 'onresourcetimingbufferfull', () => null);

        _defProtoMethod(_PerfProto, 'getEntries', function getEntries() {
            const entries = [_navEntry(), ..._buildResourceEntries()];
            const origin = globalThis.location ? globalThis.location.origin : "";
            const qratorUrl = `${origin}/__qrator/qauth_utm_v2d_v9118.js`;
            
            if (!entries.some(e => e.name.includes('qauth'))) {
                const start = 10 + (Math.random() * 5);
                const dur = 40 + (Math.random() * 20);
                entries.push({
                    name: qratorUrl,
                    entryType: 'resource',
                    startTime: Number(start.toFixed(13)),
                    duration: Number(dur.toFixed(13)),
                    initiatorType: 'script',
                    nextHopProtocol: 'h2',
                    workerStart: 0,
                    redirectStart: 0,
                    redirectEnd: 0,
                    fetchStart: Number(start.toFixed(13)),
                    domainLookupStart: Number(start.toFixed(13)),
                    domainLookupEnd: Number(start.toFixed(13)),
                    connectStart: Number(start.toFixed(13)),
                    connectEnd: Number(start.toFixed(13)),
                    secureConnectionStart: Number(start.toFixed(13)),
                    requestStart: Number((start + 1).toFixed(13)),
                    responseStart: Number((start + 5).toFixed(13)),
                    responseEnd: Number((start + dur).toFixed(13)),
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
                const entries = _buildResourceEntries();
                const origin = globalThis.location ? globalThis.location.origin : "";
                entries.push({
                    name: `${origin}/__qrator/qauth_utm_v2d_v9118.js`,
                    entryType: 'resource',
                    startTime: 12.5,
                    duration: 45.2,
                    initiatorType: 'script',
                    transferSize: 349878,
                    nextHopProtocol: 'h2'
                });
                return entries;
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
            _resourceEntries = null;
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
        globalThis.atob = function atob(s) {
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
        };
    }
    const _origStringify = JSON.stringify;
    if (!globalThis.btoa) {
        globalThis.btoa = function btoa(s) {
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
        };
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
                const entry = {
                    target,
                    isIntersecting: true,
                    intersectionRatio: 1.0,
                    boundingClientRect: target.getBoundingClientRect ? target.getBoundingClientRect() : {},
                    intersectionRect: target.getBoundingClientRect ? target.getBoundingClientRect() : {},
                    rootBounds: null,
                    time: performance.now(),
                };
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
    globalThis.requestIdleCallback = function(cb) {
        return setTimeout(() => cb({ didTimeout: false, timeRemaining: () => 50 }), 1);
    };
    globalThis.cancelIdleCallback = clearTimeout;

    // getComputedStyle — reads inline style from actual element, falls back to CSS defaults.
    // CAPTURE _getNodeId at bootstrap time: cleanup_bootstrap.js deletes
    // __boxide before page scripts run, so per-call lookup degrades to
    // nodeId=0 (same bug that broke event_stop_propagation). This was why
    // every getComputedStyle() call returned the same root-element defaults
    // regardless of which element was passed.
    const _getNodeIdForCompStyle = (globalThis.__boxide && globalThis.__boxide._getNodeId)
        ? globalThis.__boxide._getNodeId
        : (() => 0);
    globalThis.getComputedStyle = function(element, pseudoElt) {
        const nodeId = _getNodeIdForCompStyle(element);
        return new Proxy({}, {
            get(target, prop) {
                if (prop === "getPropertyValue") {
                    return (name) => ops.op_dom_get_computed_style(nodeId, name);
                }
                if (prop === "setProperty" || prop === "removeProperty") {
                    return () => {}; // read-only
                }
                if (typeof prop === "string") {
                    const kebab = prop.replace(/[A-Z]/g, m => "-" + m.toLowerCase());
                    return ops.op_dom_get_computed_style(nodeId, kebab);
                }
                return undefined;
            }
        });
    };

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
            this.upload = new (class XMLHttpRequestUpload extends EventTarget {
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
            })();
        }
        static UNSENT = 0;
        static OPENED = 1;
        static HEADERS_RECEIVED = 2;
        static LOADING = 3;
        static DONE = 4;
        open(method, url, async = true, user, password) {
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

    // WebSocket — real connections via tokio-tungstenite ops
    globalThis.WebSocket = class WebSocket {
        static CONNECTING = 0;
        static OPEN = 1;
        static CLOSING = 2;
        static CLOSED = 3;
        constructor(url, protocols) {
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

    // --- matchMedia ---
    globalThis.matchMedia = function(query) {
        // Evaluate common media queries based on our stealth profile
        let matches = false;
        if (query.includes("prefers-color-scheme: light")) matches = true;
        if (query.includes("prefers-reduced-motion: no-preference")) matches = true;
        const widthMatch = query.match(/\(min-width:\s*(\d+)px\)/);
        if (widthMatch && globalThis.innerWidth >= parseInt(widthMatch[1])) matches = true;
        const maxWidthMatch = query.match(/\(max-width:\s*(\d+)px\)/);
        if (maxWidthMatch && globalThis.innerWidth <= parseInt(maxWidthMatch[1])) matches = true;
        return {
            matches,
            media: query,
            onchange: null,
            addListener(cb) {},  // deprecated
            removeListener(cb) {},  // deprecated
            addEventListener(type, cb) {},
            removeEventListener(type, cb) {},
            dispatchEvent(event) { return true; },
        };
    };

    // --- window.open/close/postMessage ---
    globalThis.open = function(url, target, features) { return null; };
    globalThis.close = function() {};
    globalThis.postMessage = function(message, targetOrigin, transfer) {
        // Fire message event asynchronously
        Promise.resolve().then(() => {
            const event = new MessageEvent("message", {
                data: message,
                origin: targetOrigin || globalThis.location?.origin || "",
            });
            globalThis.dispatchEvent(event);
        });
    };
    globalThis.stop = function() {};
    globalThis.print = function() {};
    globalThis.confirm = function(msg) { return true; };
    globalThis.alert = function(msg) {};
    globalThis.prompt = function(msg, def) { return def || null; };

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
    globalThis.RTCPeerConnection = class RTCPeerConnection {
        constructor(config) {
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
            const ch = { label, readyState: "connecting", send() {}, close() {}, onopen: null, onmessage: null, onerror: null, onclose: null };
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
            // Fire empty ICE candidate (signals gathering complete without leaking IP)
            setTimeout(() => {
                if (this.onicecandidate) this.onicecandidate({ candidate: null });
                this.iceGatheringState = "complete";
            }, 10);
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

        // document.fonts (FontFaceSet)
        if (globalThis.document) {
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
                    forEach() {},
                    entries() { return [][Symbol.iterator](); },
                    keys() { return [][Symbol.iterator](); },
                    values() { return [][Symbol.iterator](); },
                    [Symbol.iterator]() { return [][Symbol.iterator](); },
                    size: _fonts.length,
                    add() {},
                    delete() { return false; },
                    has() { return true; },
                    clear() {},
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
        ]);

        globalThis.MediaSource = class MediaSource {
            static isTypeSupported(type) {
                if (_supportedTypes.has(type)) return true;
                // Partial match on base type
                const base = type.split(';')[0].trim();
                return _supportedTypes.has(base);
            }
            addEventListener() {}
            removeEventListener() {}
        };
        globalThis.MediaSource.isTypeSupported = MediaSource.isTypeSupported;

        // Patch HTMLMediaElement.canPlayType if document exists
        if (globalThis.document) {
            const _origCreate = globalThis.document.createElement.bind(globalThis.document);
            const _patchedCreate = function(tag) {
                const el = _origCreate(tag);
                if (tag === 'video' || tag === 'audio') {
                    el.canPlayType = function(type) {
                        if (_supportedTypes.has(type)) return "probably";
                        const base = type.split(';')[0].trim();
                        if (_supportedTypes.has(base)) return "maybe";
                        return "";
                    };
                }
                return el;
            };
            globalThis.document.createElement = _patchedCreate;
        }
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
    // Remove deno_core internal frames (ext:, deno:) from Error.stack
    // CreepJS and fingerprinters read new Error().stack
    // ================================================================
    Error.prepareStackTrace = function(err, frames) {
        const filtered = frames.filter(f => {
            const file = f.getFileName() || '';
            return !file.startsWith('ext:') && !file.startsWith('deno:')
                && !file.includes('bootstrap') && !file.includes('core/');
        });
        if (filtered.length === 0) {
            return err.toString() + '\n    at <anonymous>:1:1';
        }
        return err.toString() + '\n' + filtered.map(f => '    at ' + f.toString()).join('\n');
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
    // See docs/SOTA_ROADMAP_2026.md §1.3.
    // ================================================================
    if (typeof globalThis.Performance === 'function' && globalThis.performance) {
        const _PProto = globalThis.Performance.prototype;
        _defProtoMethod(_PProto, 'now', function now() {
            return ops.op_perf_now_humanized();
        });
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
        _defNav('gpu', () => _navGpu);
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
        globalThis.scheduler = {
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
        };
        _maskAsNative(globalThis.scheduler, 'postTask', 'yield');
    }

    // ================================================================
    // reportError (Chrome 95+) — dispatches an ErrorEvent on window.
    // ================================================================
    if (!globalThis.reportError) {
        globalThis.reportError = function reportError(err) {
            const evt = new ErrorEvent('error', { error: err, message: err && err.message || String(err), bubbles: true, cancelable: true });
            globalThis.dispatchEvent(evt);
        };
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

    // fetch(), Headers, Request, Response are now provided by fetch_bootstrap.js
    // (wired to real net::HttpClient via op_fetch)
})(globalThis);
