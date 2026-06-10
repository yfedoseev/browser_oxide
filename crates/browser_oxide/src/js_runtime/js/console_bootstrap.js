((globalThis) => {
    const core = Deno.core;

    // Read a property WITHOUT invoking a page-defined accessor. Real
    // V8 `Error.stack`/`message` are own *data* properties and
    // `name`/`constructor` are data properties on the prototype chain;
    // a planted `Object.defineProperty(e,'stack',{get(){…}})` is an own
    // *accessor*. console.* eagerly stringifying via `arg.stack` would
    // invoke that getter — exactly the svebaa CDP/inspector tell a
    // non-CDP engine must not exhibit (master plan §4 Phase 1 G9).
    // Descriptor inspection never runs the accessor; we only use the
    // value of a data descriptor.
    function _safeOwn(o, k) {
        try {
            let p = o;
            while (p !== null && p !== undefined) {
                const d = Object.getOwnPropertyDescriptor(p, k);
                if (d) return ("value" in d) ? d.value : undefined;
                p = Object.getPrototypeOf(p);
            }
        } catch (_) {}
        return undefined;
    }

    function _stringify(arg) {
        try {
            if (arg === undefined) return "[undefined]";
            if (arg === null) return "[null]";
            const type = typeof arg;
            if (type !== "object") return `[${type}] ${String(arg)}`;

            const ctorFn = _safeOwn(arg, "constructor");
            const ctor = (typeof ctorFn === "function" &&
                typeof ctorFn.name === "string" && ctorFn.name)
                ? ctorFn.name : "Object";
            if (arg instanceof Error) {
                const nm = _safeOwn(arg, "name");
                const msg = _safeOwn(arg, "message");
                const stk = _safeOwn(arg, "stack");
                return `[Error:${ctor}] ${nm}: ${msg}` +
                    (stk !== undefined ? `\n${stk}` : "");
            }
            if (ctor === "DOMException") {
                const nm = _safeOwn(arg, "name");
                const code = _safeOwn(arg, "code");
                const msg = _safeOwn(arg, "message");
                const stk = _safeOwn(arg, "stack");
                return `[DOMException] ${nm} (${code}): ${msg}` +
                    (stk !== undefined ? `\n${stk}` : "");
            }
            try {
                return `[Object:${ctor}] ${JSON.stringify(arg)}`;
            } catch (e) {
                return `[Object:${ctor}] (non-serializable: ${String(arg)})`;
            }
        } catch (e) {
            return `[StringifyError] ${e.message}`;
        }
    }

    globalThis.console = {
        log(...args) {
            core.ops.op_console_log(args.map(_stringify).join(" "));
        },
        warn(...args) {
            core.ops.op_console_warn(args.map(_stringify).join(" "));
        },
        error(...args) {
            core.ops.op_console_error(args.map(_stringify).join(" "));
        },
        info(...args) {
            core.ops.op_console_log(args.map(_stringify).join(" "));
        },
        debug(...args) {
            core.ops.op_console_log(args.map(_stringify).join(" "));
        },
        dir() {},
        dirxml() {},
        trace() {},
        group() {},
        groupCollapsed() {},
        groupEnd() {},
        clear() {},
        count() {},
        countReset() {},
        assert(cond, ...args) {
            if (!cond) {
                core.ops.op_console_error("Assertion failed: " + args.map(String).join(" "));
            }
        },
        table() {},
        time() {},
        timeLog() {},
        timeEnd() {},
    };
    // Native-masking of these methods is applied by stealth_bootstrap.js
    // (concatenated AFTER this file in the V8 snapshot, where
    // _maskAsNative is defined). Doing it here would no-op because
    // _maskAsNative does not exist yet at this point in the snapshot.
})(globalThis);
