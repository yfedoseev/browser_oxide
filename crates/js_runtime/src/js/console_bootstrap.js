((globalThis) => {
    const core = Deno.core;

    function _stringify(arg) {
        try {
            if (arg === undefined) return "[undefined]";
            if (arg === null) return "[null]";
            const type = typeof arg;
            if (type !== "object") return `[${type}] ${String(arg)}`;

            const ctor = arg.constructor ? arg.constructor.name : "Object";
            if (arg instanceof Error) {
                return `[Error:${ctor}] ${arg.name}: ${arg.message}\n${arg.stack}`;
            }
            if (ctor === "DOMException") {
                return `[DOMException] ${arg.name} (${arg.code}): ${arg.message}\n${arg.stack}`;
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
})(globalThis);
