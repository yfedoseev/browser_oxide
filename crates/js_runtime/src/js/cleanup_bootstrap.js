((globalThis) => {
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

        // ApplePaySession — present only on macOS Chrome. Akamai's sensor
        // sends `ap=null` if the constructor is missing on a macOS UA;
        // that mismatch is one of the strongest single tells in the
        // pixel POST capture. Constructor + statics shaped to match
        // Chrome 147's ApplePaySession surface.
        if (_osName === "macOS" && typeof globalThis.ApplePaySession === "undefined") {
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
        // __onNodeInserted is a strong bot signal — real browsers don't expose
        // internal DOM mutation callbacks on globalThis.
        '__onNodeInserted',
        '__errors'
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
