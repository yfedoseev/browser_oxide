((globalThis) => {
    const ops = Deno.core.ops;
    const _cancelledTimers = new Set();

    globalThis.setTimeout = function setTimeout(callback, delay = 0, ...args) {
        if (typeof callback !== "function") {
            callback = new Function(String(callback));
        }
        const ms = Math.max(0, delay | 0);
        const id = ops.op_set_timeout(ms);
        // Async ops in deno_core 0.311 are called directly and return Promise
        ops.op_timer_sleep(ms).then(() => {
            if (!_cancelledTimers.has(id)) {
                callback(...args);
            }
        });
        return id;
    };

    globalThis.setInterval = function setInterval(callback, delay = 0, ...args) {
        if (typeof callback !== "function") {
            callback = new Function(String(callback));
        }
        const ms = Math.max(4, delay | 0);
        const id = ops.op_set_interval(ms);

        function tick() {
            ops.op_timer_sleep(ms).then(() => {
                if (!_cancelledTimers.has(id)) {
                    callback(...args);
                    tick();
                }
            });
        }
        tick();
        return id;
    };

    globalThis.clearTimeout = function clearTimeout(id) {
        if (id !== undefined && id !== null) {
            _cancelledTimers.add(id);
            ops.op_clear_timer(id);
        }
    };

    globalThis.clearInterval = globalThis.clearTimeout;

    let _rafId = 0;
    const _rafCallbacks = new Map();

    globalThis.requestAnimationFrame = function requestAnimationFrame(callback) {
        const id = ++_rafId;
        _rafCallbacks.set(id, callback);
        // Fire at ~16ms (60fps) via real timer, not microtask.
        // Anti-bot systems (Kasada) measure rAF timing and flag instant firing.
        setTimeout(() => {
            const cb = _rafCallbacks.get(id);
            if (cb) {
                _rafCallbacks.delete(id);
                cb(performance.now());
            }
        }, 16);
        return id;
    };

    globalThis.cancelAnimationFrame = function cancelAnimationFrame(id) {
        _rafCallbacks.delete(id);
    };

    if (!globalThis.performance) {
        globalThis.performance = {};
    }
    if (!globalThis.performance.now) {
        const startTime = Date.now();
        globalThis.performance.now = function() {
            return Date.now() - startTime;
        };
    }

    // Native-code masking — PerimeterX/HUMAN, Akamai, and others probe
    // `Function.prototype.toString.call(setTimeout)` and friends. The
    // expected serialization is `function setTimeout() { [native code] }`;
    // a JS source body is a hard bot tell.
    if (typeof _maskFunction === 'function') {
        _maskFunction(globalThis.setTimeout, 'setTimeout');
        _maskFunction(globalThis.setInterval, 'setInterval');
        _maskFunction(globalThis.clearTimeout, 'clearTimeout');
        _maskFunction(globalThis.clearInterval, 'clearInterval');
        _maskFunction(globalThis.requestAnimationFrame, 'requestAnimationFrame');
        _maskFunction(globalThis.cancelAnimationFrame, 'cancelAnimationFrame');
        if (globalThis.performance && typeof globalThis.performance.now === 'function') {
            _maskFunction(globalThis.performance.now, 'now');
        }
    }
})(globalThis);
