((globalThis) => {
    const core = Deno.core;

    globalThis.console = {
        log(...args) {
            core.ops.op_console_log(args.map(String).join(" "));
        },
        warn(...args) {
            core.ops.op_console_warn(args.map(String).join(" "));
        },
        error(...args) {
            core.ops.op_console_error(args.map(String).join(" "));
        },
        info(...args) {
            core.ops.op_console_log(args.map(String).join(" "));
        },
        debug(...args) {
            core.ops.op_console_log(args.map(String).join(" "));
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
