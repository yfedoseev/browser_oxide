((globalThis) => {
    const ops = Deno.core.ops;
    const _cancelledTimers = new Set();
    // Timer generation — bumped by `globalThis.__cancelAllTimers()` so that
    // the warm-reuse path in `Page::navigate_warm` can mass-cancel every
    // in-flight `setTimeout`/`setInterval` callback from the previous page
    // in one O(1) op. Each callback captures the generation at schedule
    // time; on resolve, if `_timerGen` has moved past it, the callback
    // bails before running. Why this is needed: `op_timer_sleep` is just
    // `tokio::sleep(ms)` and isn't tied to `TimerState`, so the Rust-side
    // `replace_dom` doesn't preempt in-flight tokio sleeps — those still
    // fire and try to run their callbacks. Without the generation guard,
    // `humanize.js`'s 30 pending setTimeouts from the previous page would
    // dispatch synthetic mouse events into the newly-loaded DOM.
    let _timerGen = 0;
    globalThis.__cancelAllTimers = function __cancelAllTimers() {
        _timerGen++;
    };
    // W5b-PLUS (twitter/x.com hydration fix): unref every op_timer_sleep
    // promise. The deno_core event loop treats every in-flight async op as
    // "is_pending=true" (jsruntime.rs:2223 — has_pending_refed_ops).
    // A pre-fix profile of x.com showed 33 simultaneous pending ops, ALL
    // op_timer_sleep, cycling 33→18→1→33 driven by React's scheduler +
    // requestIdleCallback polyfill + IntersectionObserver fanout. The
    // loop never reached idle, so SPA hydration timed out at body=69 B.
    //
    // unrefOpPromise is the deno equivalent of node.js's `timer.unref()`:
    // the promise still fires when its time comes, but it doesn't block
    // the event loop from reporting "all work done". Real Chrome treats
    // setTimeout-scheduled work as background work that doesn't gate
    // navigation idle — this matches that semantics exactly.
    //
    // Side effect: if the page exits run_until_idle before a long timer
    // fires, that callback runs on the *next* run_until_idle invocation
    // (page.rs schedules several drains per navigate iteration). For SPA
    // homepage hydration, this is exactly the desired behavior — once
    // the React mount has children (page.rs:1252 early-exit), any
    // outstanding setTimeout(50000, retryAnalytics) is just background
    // noise we'd be killing on drop anyway.
    const _unrefRaw = Deno.core.unrefOpPromise;
    // Only unref *long-delay* timers (>= UNREF_THRESHOLD_MS). Short
    // timers (< threshold) carry render-critical work — React's
    // scheduler postTask, microtask-equivalent rIC fallback, etc. —
    // and unrefing them causes run_until_idle to exit before the
    // continuation runs (verified empirically on x.com 2026-05-10:
    // unref-everything → AllWorkDone in 4 s, body=69 B; unref nothing
    // → 90 s timeout, body=69 B).
    //
    // Threshold history:
    //   1000ms — twitter/x flipped to L3 but macys/ria/threads regressed
    //            to THIN-BODY (their hydration uses setTimeout(fn, ~1500)
    //            for delayed render steps; unref'd them too eagerly).
    //   2000ms — current. Keeps twitter/x (their pinning is cycles of
    //            hundreds of sub-second timers + occasional 5–60s
    //            analytics) while preserving macys/ria/threads
    //            ~1.5s-delay hydration callbacks as refed.
    const UNREF_THRESHOLD_MS = 2000;
    const _maybeUnref = _unrefRaw
        ? (p, ms) => { if (ms >= UNREF_THRESHOLD_MS) _unrefRaw(p); }
        : () => {};

    globalThis.setTimeout = function setTimeout(callback, delay = 0, ...args) {
        if (typeof callback !== "function") {
            callback = new Function(String(callback));
        }
        const ms = Math.max(0, delay | 0);
        const id = ops.op_set_timeout(ms);
        // Async ops in deno_core 0.311 are called directly and return Promise
        const p = ops.op_timer_sleep(ms);
        _maybeUnref(p, ms);
        const myGen = _timerGen;
        p.then(() => {
            if (myGen !== _timerGen) return; // post `__cancelAllTimers`, drop
            if (!_cancelledTimers.has(id)) {
                callback(...args);
            }
        });
        return id;
    };

    // Background-timer helper for engine-internal scripts that want their
    // setTimeout callbacks to fire eventually but DON'T want the timer to
    // pin `run_until_idle` open. Used by `crates/browser/src/js/humanize.js`
    // — its ~30 synthetic-input timers (50 ms-1.8 s spread) should not
    // gate engine "idle" because the page can be returned to the caller
    // the moment its own work settles; whatever humanize timers haven't
    // fired yet are background no-ops for benign pages, and anti-bot
    // pages keep the loop busy with their own challenge VMs so the
    // humanize timers still fire there. Same semantics as Node's
    // `setTimeout(...).unref()`.
    globalThis.__bgSetTimeout = function __bgSetTimeout(callback, delay = 0, ...args) {
        if (typeof callback !== "function") {
            callback = new Function(String(callback));
        }
        const ms = Math.max(0, delay | 0);
        const id = ops.op_set_timeout(ms);
        const p = ops.op_timer_sleep(ms);
        if (_unrefRaw) _unrefRaw(p);
        const myGen = _timerGen;
        p.then(() => {
            if (myGen !== _timerGen) return;
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

        const myGen = _timerGen;
        function tick() {
            const p = ops.op_timer_sleep(ms);
            // Intervals are by definition recurring — unref them at the
            // same threshold. A 5 s recurring analytics ping shouldn't
            // pin the loop any more than a 5 s setTimeout.
            _maybeUnref(p, ms);
            p.then(() => {
                if (myGen !== _timerGen) return;
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
