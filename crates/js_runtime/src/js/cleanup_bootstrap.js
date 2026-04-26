((globalThis) => {
    const internals = [
        'Deno',
        'ops',
        '_maskFunction',
        '_maskAsNative',
        '_customElementsRegistry',
        '__bootstrap',
        '__boxide',
        '__syncCookiesFromNet',
        '__documentReadyState',
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
