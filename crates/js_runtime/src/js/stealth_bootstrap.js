((globalThis) => {
    const ops = Deno.core.ops;
    const print = (msg) => {
        try { Deno.core.print(msg + "\n"); } catch {}
    };

    // --- Function.prototype.toString bypass patch ---
    // CreepJS and other detectors call Function.prototype.toString.call(fn)
    // directly, which bypasses any instance-level fn.toString override and
    // returns the raw JS source of polyfilled functions. We patch
    // Function.prototype.toString itself to consult a private Symbol tag
    // we set on masked functions.
    const _nativeTag = Symbol.for('__boxide_native__');
    const _origFnToStr = Function.prototype.toString;

    // Re-entrant guard: prevents infinite recursion when this[_nativeTag] access
    // triggers a Proxy get trap that itself calls Function.prototype.toString.
    let _inPatchedToStr = false;
    const _patchedFnToStr = function toString() {
        if (_inPatchedToStr) return _origFnToStr.call(this);
        _inPatchedToStr = true;
        try {
            if (this !== null && this !== undefined) {
                try {
                    const tag = this[_nativeTag];
                    if (tag) return `function ${tag}() { [native code] }`;
                } catch (_) {}
            }
            return _origFnToStr.call(this);
        } finally {
            _inPatchedToStr = false;
        }
    };
    // Tag the patched toString itself so recursive calls also appear native
    Object.defineProperty(_patchedFnToStr, _nativeTag, { value: 'toString', configurable: true });
    Object.defineProperty(_patchedFnToStr, 'name', { value: 'toString', configurable: true });

    Object.defineProperty(Function.prototype, 'toString', {
        value: _patchedFnToStr,
        writable: true,
        configurable: true,
    });

    // --- Native code masking ---
    const _maskFunction = (fn, name) => {
        if (!fn) return;
        try {
            Object.defineProperty(fn, 'name', { value: name, configurable: true });
            // Symbol tag — used by the patched Function.prototype.toString above
            Object.defineProperty(fn, _nativeTag, { value: name, configurable: true });
            // Instance toString — used by direct fn.toString() calls
            const ts = function toString() { return `function ${name}() { [native code] }`; };
            Object.defineProperty(fn, 'toString', { value: ts, configurable: true });
            // Mask the toString wrapper itself
            Object.defineProperty(ts, 'name', { value: 'toString', configurable: true });
            Object.defineProperty(ts, _nativeTag, { value: 'toString', configurable: true });
            // innerTs must also carry _nativeTag — otherwise Function.prototype.toString.call(innerTs)
            // would return its raw source code instead of a native-looking string.
            const innerTs = function toString() { return 'function toString() { [native code] }'; };
            Object.defineProperty(innerTs, _nativeTag, { value: 'toString', configurable: true });
            Object.defineProperty(ts, 'toString', {
                value: innerTs,
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
    Object.defineProperty(globalThis, '_nativeTag', { value: _nativeTag, enumerable: false, configurable: true });
    Object.defineProperty(globalThis, '_maskFunction', { value: _maskFunction, enumerable: false, configurable: true });
    Object.defineProperty(globalThis, '_maskAsNative', { value: _maskAsNative, enumerable: false, configurable: true });

    // BotD `eval_length.ts` detector: `eval.toString().length === 33` for Chromium.
    // V8 natively produces "function eval() { [native code] }" (33 chars), so this
    // is usually a no-op. We tag `eval` defensively so any V8 build drift is
    // self-corrected to Chrome's canonical shape.
    try { _maskFunction(eval, 'eval'); } catch (_) {}

    // Native-mask every console method. console_bootstrap.js is
    // concatenated BEFORE this file in the V8 snapshot (snapshot.rs),
    // so it could not call _maskAsNative itself (undefined then) —
    // `globalThis.console` already exists here, and _maskAsNative is
    // now defined, so this is the correct place. Kasada's `ofc` probe
    // dumps `console.<method>.toString()` for all ~19 methods into the
    // /tl sensor (decrypted_blob_0_pretty.json `/ofc/r` showed ours
    // leaking `log(...args) { core.ops.op_console_log(...) }` — a
    // non-Chrome tell feeding the dominant 30-40% browser-fingerprint
    // weight of Kasada's server ML score). Real Chrome returns
    // `function log() { [native code] }` for every console method.
    try {
        if (globalThis.console) {
            _maskAsNative(
                globalThis.console,
                'log', 'warn', 'error', 'info', 'debug', 'dir', 'dirxml',
                'trace', 'group', 'groupCollapsed', 'groupEnd', 'clear',
                'count', 'countReset', 'assert', 'table', 'time',
                'timeLog', 'timeEnd',
            );
        }
    } catch (_) {}

})(globalThis);
