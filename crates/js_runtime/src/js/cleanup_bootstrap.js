((globalThis) => {
    // -- Per-page secure-context gating (Phase 7) --------------------
    // The V8 snapshot bootstraps with is_secure_context=true so all
    // [SecureContext]-only Web Platform APIs are baked in. On insecure
    // pages (data:/http:/about:blank) we strip them here to match real
    // Chrome — see docs/PHASE7_AB_PROBE_FINDINGS_2026_04_29.md.
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
        const _profileOps = Deno && Deno.core && Deno.core.ops;
        const _hasProfile = _profileOps && _profileOps.op_has_stealth_profile && _profileOps.op_has_stealth_profile();
        const _osName = (_hasProfile && _profileOps.op_get_profile_value)
            ? (_profileOps.op_get_profile_value("os_name") || "Linux")
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
    } catch (_e) { /* profile-conditional installs are best-effort */ }

    const internals = [
        'Deno',
        'ops',
        '_maskFunction',
        '_maskAsNative',
        // _nativeTag is the Symbol used to mark masked functions. Exposing it
        // lets anti-bot scripts read our masking mechanism directly.
        '_nativeTag',
        '_customElementsRegistry',
        '__bootstrap',
        '__boxide',
        '__syncCookiesFromNet',
        '__documentReadyState',
        '__drainCspViolations',
        // __onNodeInserted is a strong bot signal — real browsers don't expose
        // internal DOM mutation callbacks on globalThis.
        '__onNodeInserted',
        '__errors',
        // SharedArrayBuffer is exposed by V8 by default; real Chrome only
        // exposes it when crossOriginIsolated. For non-COI pages, hide it.
        // (deno_core may have it non-configurable; delete is best-effort.)
        'SharedArrayBuffer',
        // __pendingNavigation intentionally kept: it is a signal for the
        // Rust navigation driver. Synchronous inline scripts (form.submit,
        // location.href = ...) set it during the same tick cleanup runs,
        // so deleting it here loses the signal before run_until_idle and
        // the driver check. It is defined non-enumerable in
        // window_bootstrap.js so it does not leak via Object.keys.
    ];

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
