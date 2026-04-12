/**
 * Final cleanup bootstrap — runs after all built-in and init scripts.
 * Hides internal globals (Deno, __bootstrap, etc.) from user JS
 * to ensure maximum stealth and pass bot detection batteries.
 */
((globalThis) => {
    // 1. Delete Deno global (the biggest tell)
    try {
        delete globalThis.Deno;
    } catch (e) {
        // If delete fails (e.g. non-configurable), set to undefined
        globalThis.Deno = undefined;
    }

    // 2. Hide internal oxide/bootstrap globals
    const internalKeys = [
        '__bootstrap',
        '__boxide',
        '__browserOxide',
        '__syncCookiesFromNet',
        '__jsCookies',
        '_customElementsRegistry',
        '_maskAsNative',
        '_maskFunction'
    ];

    for (const key of internalKeys) {
        try {
            // Delete if possible, else make non-enumerable
            if (!delete globalThis[key]) {
                Object.defineProperty(globalThis, key, {
                    enumerable: false,
                    configurable: true,
                    value: globalThis[key]
                });
            }
        } catch (e) {}
    }

    // 3. Ensure __pendingNavigation is non-enumerable (it should be already)
    try {
        Object.defineProperty(globalThis, '__pendingNavigation', {
            enumerable: false,
            configurable: true,
            value: globalThis.__pendingNavigation
        });
    } catch (e) {}

})(globalThis);
