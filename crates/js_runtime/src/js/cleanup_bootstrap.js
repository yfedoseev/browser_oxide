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
        '__pendingNavigation'
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
