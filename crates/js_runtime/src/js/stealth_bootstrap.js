((globalThis) => {
    const ops = Deno.core.ops;
    const print = (msg) => {
        try { Deno.core.print(msg + "\n"); } catch {}
    };

    // --- Native code masking ---
    const _maskFunction = (fn, name) => {
        if (!fn) return;
        try {
            Object.defineProperty(fn, 'name', { value: name, configurable: true });
            const ts = function toString() { return `function ${name}() { [native code] }`; };
            Object.defineProperty(fn, 'toString', { value: ts, configurable: true });
            
            // Recursively mask the toString itself
            Object.defineProperty(ts, 'name', { value: 'toString', configurable: true });
            Object.defineProperty(ts, 'toString', {
                value: function toString() { return 'function toString() { [native code] }'; },
                configurable: true,
            });
        } catch (e) {}
    };

    const _maskAsNative = (obj, ...names) => {
        for (const name of names) {
            try {
                // Find where the property actually lives (own or prototype)
                let target = obj;
                let desc = Object.getOwnPropertyDescriptor(target, name);
                while (!desc && target && target !== Object.prototype) {
                    target = Object.getPrototypeOf(target);
                    if (target) desc = Object.getOwnPropertyDescriptor(target, name);
                }
                
                if (desc) {
                    if (desc.get) _maskFunction(desc.get, `get ${name}`);
                    if (desc.set) _maskFunction(desc.set, `set ${name}`);
                    if (typeof desc.value === 'function') _maskFunction(desc.value, name);
                } else {
                    // Fallback for direct prototype access
                    const val = obj[name];
                    if (typeof val === 'function') _maskFunction(val, name);
                }
            } catch (e) {}
        }
    };

    // Expose helpers globally for other bootstraps
    Object.defineProperty(globalThis, '_maskFunction', { value: _maskFunction, enumerable: false, configurable: true });
    Object.defineProperty(globalThis, '_maskAsNative', { value: _maskAsNative, enumerable: false, configurable: true });

})(globalThis);
