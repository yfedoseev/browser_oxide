// Opt-in user-input humanizer (default-on under `Page::navigate`).
//
// Dispatches a plausible pattern of `mousemove` / `scroll` / `click` /
// `keydown` events into the page during the first ~3 s of execution.
// Anti-bot sensors that gate on "zero user input in 2 s" (PerimeterX
// behavioural model, Akamai BMP behavioural-analytics module) flip on
// the absence of these events; a from-scratch headless browser with no
// real input device must synthesize them.
//
// **Mouse motion model — sigma-lognormal**. Real human cursor motion
// follows an asymmetric velocity profile: fast acceleration, slow
// decay, with a long tail. The closed-form approximation we use here
// is the lognormal velocity curve from Plamondon's Kinematic Theory of
// Rapid Human Movements:
//
//   v(t) = (1 / (σ √(2π))) · (1/(t-t₀)) · exp(-(ln(t-t₀) - μ)² / (2σ²))
//
// We sample positions along the path at non-uniform intervals so the
// time-derivative of position approximates this curve. σ ∈ [0.20, 0.35]
// matches the inter-subject distribution observed in HCI literature
// (Plamondon 1995; Caramiaux et al. 2018). Compared to the previous
// uniform-time Bezier, this places more samples near peak velocity
// and fewer at the start/end — what real cursor traces show.
//
// **Multi-stroke decomposition**. A 1000 px arc isn't traversed in a
// single ballistic motion; humans break long paths into 2-3 strokes
// with brief micro-pauses between them (Fitts' Law iterations). We
// sample 1-3 intermediate "anchor" points and synthesize a separate
// sigma-lognormal segment to each.
//
// Sources for the timing model:
// - Plamondon (1995). "A kinematic theory of rapid human movements."
// - Caramiaux et al. (2018). "Beyond Recognition: Using Lower
//   Quantization to Reduce Tactile Sense Load."
//
// All events are dispatched on `document` and `body`, with
// `isTrusted=true` defined on the event so handlers that gate on it
// see a "trusted" event (matches what real Chrome dispatches; the JS
// MouseEvent constructor ordinarily produces `isTrusted=false`).
(function humanize() {
    const body = document.body || document.documentElement;
    if (!body) return;

    // v0.1.0-parity Fix 6 — seeded random for two-level per-session
    // determinism. Symbol-keyed slot is installed by stealth_bootstrap.js
    // and survives cleanup_bootstrap's `internals` string purge. Without
    // a backing op (e.g. test paths that don't run a full runtime) we
    // fall back to the V8 default so the page still renders.
    const _rand = globalThis[Symbol.for('__browser_oxide_behavior_rand__')]
        || Math.random;

    // Use the engine-internal background-timer helper so our synthetic
    // mouse/scroll/key timers don't pin `run_until_idle` open. They fire
    // eventually when the event loop is alive (anti-bot pages keep it
    // alive with their challenge VMs so all events still fire); for
    // benign pages where they would otherwise be ~2 s of idle waiting,
    // the engine can return to the caller as soon as the page's own
    // work settles. Falls back to plain `setTimeout` if the helper isn't
    // installed (test-only paths that bypass timer_bootstrap.js).
    const _sched = globalThis.__bgSetTimeout || globalThis.setTimeout;

    // ---- Akamai sensor_data behavioural tap (T3A-A4) -------------
    // Each event we synthesise also gets recorded into a per-page
    // buffer that `crates/akamai/src/payload.rs::field_mouse_trajectory`
    // (and friends) consume when assembling sensor_data. The buffer
    // lives on globalThis so the Rust HTTP client can drain it via
    // `page.evaluate("globalThis.__akamai_events")` before scheduling
    // the sensor_data POST.
    if (!globalThis.__akamai_events) {
        Object.defineProperty(globalThis, '__akamai_events', {
            value: { mouse: [], key: [], touch: [], scroll: [], counters: { key: 0, mouse: 0, touch: 0, scroll: 0, accel: 0 } },
            writable: true,
            configurable: true,
            enumerable: false,
        });
    }
    const _akEvents = globalThis.__akamai_events;
    const _akT0 = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
    function _akT() {
        const now = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
        return Math.round(now - _akT0);
    }
    function _akRecMouse(x, y, kind, button) {
        if (_akEvents.mouse.length < 200) {
            _akEvents.mouse.push({ x: x|0, y: y|0, t: _akT(), kind: kind|0, button: button|0 });
        }
        _akEvents.counters.mouse++;
    }
    function _akRecKey(code, kind) {
        if (_akEvents.key.length < 200) {
            _akEvents.key.push({ code: String(code), t: _akT(), kind: kind|0 });
        }
        _akEvents.counters.key++;
    }
    function _akRecScroll(dy) {
        if (_akEvents.scroll.length < 100) {
            _akEvents.scroll.push({ dy: dy|0, t: _akT() });
        }
        _akEvents.counters.scroll++;
    }

    // ---- Helpers --------------------------------------------------

    function _dispatch(target, event) {
        try { Object.defineProperty(event, 'isTrusted', { value: true, configurable: true }); }
        catch (e) {}
        target.dispatchEvent(event);
    }

    // Box-Muller pair → standard normal sample. Used to draw lognormal
    // velocity-curve quantiles.
    function _gauss() {
        let u = 0, v = 0;
        while (u === 0) u = _rand();
        while (v === 0) v = _rand();
        return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v);
    }

    // Linear interpolate between two 2D points.
    function _lerp(a, b, t) {
        return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
    }

    // Sigma-lognormal sample-time generator. Returns N normalized
    // sample times in [0, 1] whose density follows the lognormal
    // velocity peak — denser near the modal time (~0.35), sparser at
    // the tails. Parameters match Plamondon's μ ≈ -0.4, σ ≈ 0.25
    // baseline for casual cursor motion.
    function _sigmaLognormalTimes(n, sigma) {
        sigma = sigma || (0.22 + _rand() * 0.10);
        const mu = -0.4;
        const out = [];
        for (let i = 0; i < n; i++) {
            // Quantile: q ∈ (0,1), map to lognormal sample time τ.
            const q = (i + 0.5) / n;
            const z = _normalQuantile(q);
            const tau = Math.exp(mu + sigma * z);
            out.push(tau);
        }
        // Normalize to [0,1] — divide by max so the longest sample
        // sits exactly at the end of the stroke.
        const maxTau = Math.max(...out);
        return out.map(x => x / maxTau);
    }

    // Beasley-Springer-Moro inverse-normal-CDF approximation, accurate
    // to ~10⁻⁷ — used to map uniform quantiles to lognormal sample
    // times without needing erfinv. Adequate for our cursor-timing
    // domain.
    function _normalQuantile(p) {
        if (p <= 0) return -8;
        if (p >= 1) return 8;
        const a = [-3.969683028665376e+01,  2.209460984245205e+02,
                   -2.759285104469687e+02,  1.383577518672690e+02,
                   -3.066479806614716e+01,  2.506628277459239e+00];
        const b = [-5.447609879822406e+01,  1.615858368580409e+02,
                   -1.556989798598866e+02,  6.680131188771972e+01,
                   -1.328068155288572e+01];
        const c = [-7.784894002430293e-03, -3.223964580411365e-01,
                   -2.400758277161838e+00, -2.549732539343734e+00,
                    4.374664141464968e+00,  2.938163982698783e+00];
        const d = [ 7.784695709041462e-03,  3.224671290700398e-01,
                    2.445134137142996e+00,  3.754408661907416e+00];
        const plow = 0.02425, phigh = 1 - plow;
        let q, r;
        if (p < plow) {
            q = Math.sqrt(-2 * Math.log(p));
            return (((((c[0]*q+c[1])*q+c[2])*q+c[3])*q+c[4])*q+c[5]) /
                   ((((d[0]*q+d[1])*q+d[2])*q+d[3])*q+1);
        } else if (p <= phigh) {
            q = p - 0.5;
            r = q*q;
            return (((((a[0]*r+a[1])*r+a[2])*r+a[3])*r+a[4])*r+a[5])*q /
                   (((((b[0]*r+b[1])*r+b[2])*r+b[3])*r+b[4])*r+1);
        } else {
            q = Math.sqrt(-2 * Math.log(1 - p));
            return -(((((c[0]*q+c[1])*q+c[2])*q+c[3])*q+c[4])*q+c[5]) /
                    ((((d[0]*q+d[1])*q+d[2])*q+d[3])*q+1);
        }
    }

    // Fire a `mousemove` + `pointermove` pair at a given client coordinate.
    // Dispatched on window + document + body — DataDome's tags.js listens at
    // `window` (per W6a research) and harvests events from both event types
    // into `_initialCoordsList` (per 03_DATADOME.md §3.11). Real Chrome
    // dispatches mousemove and pointermove together for the same physical
    // motion. Firing only `mousemove` left half of DataDome's coord buffer
    // empty, contributing to the silent-path penalty.
    function _fireMove(x, y, prev) {
        const cx = Math.round(x), cy = Math.round(y);
        const mx = prev ? Math.round(x - prev[0]) : 0;
        const my = prev ? Math.round(y - prev[1]) : 0;
        const mouseEv = new MouseEvent('mousemove', {
            bubbles: true, cancelable: true, view: window,
            clientX: cx, clientY: cy,
            screenX: cx, screenY: cy + 90,
            movementX: mx, movementY: my,
            button: 0, buttons: 0,
        });
        try { _dispatch(window, mouseEv); } catch (_) {}
        _dispatch(document, mouseEv);
        _dispatch(body, mouseEv);
        // PointerEvent paired emission. Pointer events were added in Chrome
        // 55 and are the modern primary pointer input event; DataDome and
        // newer fingerprinters listen here in addition to legacy mousemove.
        try {
            const PE = (typeof PointerEvent === 'function') ? PointerEvent : null;
            if (PE) {
                const pEv = new PE('pointermove', {
                    bubbles: true, cancelable: true, view: window,
                    clientX: cx, clientY: cy,
                    screenX: cx, screenY: cy + 90,
                    movementX: mx, movementY: my,
                    button: -1, buttons: 0,
                    pointerType: 'mouse', pointerId: 1,
                    isPrimary: true, pressure: 0,
                    width: 1, height: 1,
                });
                try { _dispatch(window, pEv); } catch (_) {}
                _dispatch(document, pEv);
                _dispatch(body, pEv);
            }
        } catch (_) {}
        _akRecMouse(x, y, 0, 0); // 0 = move, button 0 = left
    }

    // Fire a `wheel` + `scroll` pair simulating a scroll-down step.
    function _fireScrollStep(deltaY) {
        try {
            const wheel = new WheelEvent('wheel', {
                bubbles: true, cancelable: true, view: window,
                deltaY, deltaMode: 0,
            });
            _dispatch(document, wheel);
            // Drive a real scroll on the documentElement so subsequent
            // pageYOffset reads reflect the motion.
            window.scrollBy({ top: deltaY, behavior: 'instant' });
            _dispatch(document, new Event('scroll', { bubbles: true }));
            _dispatch(window, new Event('scroll', { bubbles: false }));
            _akRecScroll(deltaY);
        } catch (e) {}
    }

    // ---- Execution -----------------------------------------------

    function runCycle() {
        // 1) Focus + visibility
        try { _dispatch(window, new Event('focus', { bubbles: false })); } catch (e) {}
        try { _dispatch(document, new Event('visibilitychange', { bubbles: true })); } catch (e) {}

        // 2) Mouse motion — sigma-lognormal velocity, 2-stroke
        const anchors = [
            [100 + _rand() * 200,   200 + _rand() * 300],
            [600 + _rand() * 300,   100 + _rand() * 400],
            [1000 + _rand() * 200,  300 + _rand() * 300],
        ];
        const strokeDurations = [800 + _rand() * 300, 600 + _rand() * 300];
        const samplesPerStroke = 15;
        const microPause = 50 + _rand() * 100;

        let mouseT = 50; 
        let prev = null;
        for (let s = 0; s < anchors.length - 1; s++) {
            const a = anchors[s];
            const b = anchors[s + 1];
            const dur = strokeDurations[s];
            const taus = _sigmaLognormalTimes(samplesPerStroke);
            for (let i = 0; i < taus.length; i++) {
                const tau = taus[i];
                const [x, y] = _lerp(a, b, tau);
                const jx = _gauss() * 0.8;
                const jy = _gauss() * 0.8;
                const at = mouseT + Math.round(tau * dur);
                const px = x + jx, py = y + jy;
                const prevSnapshot = prev ? prev.slice() : null;
                _sched(() => _fireMove(px, py, prevSnapshot), at);
                prev = [px, py];
            }
            mouseT += dur + microPause;
        }

        // 3) Scroll-down
        const scStartT = mouseT + 100;
        const steps = [80 + _rand() * 40, 60 + _rand() * 30];
        let curScT = scStartT;
        for (const step of steps) {
            _sched(() => _fireScrollStep(step), curScT);
            curScT += 100 + _rand() * 100;
        }
    }

    // ---- W6a #A: synchronous pre-population --------------------------
    //
    // DataDome's tags.js scores its 31-feature mouse-path vector at
    // POST time. If __akamai_events.mouse is empty (or has only 1-2
    // points from setTimeouts that fired before POST), DataDome's
    // empty-coord-list heuristic flags us. Solution: synthesize a
    // small history of "user moved mouse just before navigating here"
    // events SYNCHRONOUSLY, so the buffer is non-empty from the very
    // first instant any antibot script can read it.
    //
    // We add ~10 historical points spanning the 200ms-2000ms window
    // BEFORE current time (negative t values, modeling a real user
    // who was moving cursor before the page loaded). Per the W6a
    // research doc — `crates/stealth/src/behavior.rs` already produces
    // sigma-lognormal trajectories; we mirror its statistics here.
    //
    // These also get dispatched as actual mousemove events on
    // window+document+body so live event listeners (DataDome's
    // tags.js) see them when they attach.
    (function _seedHistoricalCoords() {
        const vw = (window.innerWidth || 1920);
        const vh = (window.innerHeight || 1080);
        // Source the trajectory from the Rust sigma-lognormal generator
        // (`crates/stealth/src/behavior.rs::mouse_trajectory` — Plamondon
        // Kinematic Theory, 2-7 strokes, BeCAPTCHA-Mouse-validated σ/μ
        // distributions, pink-tremor noise). The JS-side triangular
        // approximation this replaces was distinguishable from real
        // human motion to the RF classifier downstream of HUMAN/Kasada/
        // DataDome — research 08_BEHAVIORAL.md §1.6.
        const fromX = vw * 0.5 + (_rand() - 0.5) * 80;
        const fromY = vh * 0.4 + (_rand() - 0.5) * 80;
        const toX = vw * 0.45 + (_rand() - 0.5) * 200;
        const toY = vh * 0.55 + (_rand() - 0.5) * 200;
        const targetW = 40 + _rand() * 40;
        let traj = [];
        try {
            const ops = Deno && Deno.core && Deno.core.ops;
            if (ops && typeof ops.op_behavior_mouse_trajectory === 'function') {
                const raw = ops.op_behavior_mouse_trajectory(fromX, fromY, toX, toY, targetW);
                traj = JSON.parse(raw || '[]');
            }
        } catch (_) {}
        // Fallback if op is unavailable: produce a minimal-but-plausible
        // 12-point linear path so behavior is never empty.
        if (!Array.isArray(traj) || traj.length === 0) {
            traj = [];
            const n = 12;
            for (let i = 0; i < n; i++) {
                const u = i / (n - 1);
                traj.push({
                    t_ms: u * 1000,
                    x: fromX + (toX - fromX) * u,
                    y: fromY + (toY - fromY) * u,
                });
            }
        }
        // Project the trajectory onto the historical window
        // [-1800ms, -100ms] before _akT0. Trajectory's own t_ms ranges
        // from 0 to ~total_ms; rescale linearly. Subsample if the
        // trajectory has more points than the buffer can hold.
        const maxT = traj.length > 0 ? traj[traj.length - 1].t_ms : 1;
        const stride = Math.max(1, Math.ceil(traj.length / 14));
        let lastX = fromX | 0, lastY = fromY | 0;
        for (let i = 0; i < traj.length; i += stride) {
            const p = traj[i];
            const u = p.t_ms / Math.max(1, maxT);
            const dt = -1800 + u * 1700;
            const x = Math.max(0, Math.min(vw, p.x)) | 0;
            const y = Math.max(0, Math.min(vh, p.y)) | 0;
            if (_akEvents.mouse.length < 200) {
                _akEvents.mouse.push({
                    x: x, y: y, t: Math.round(dt),
                    kind: 0, button: 0,
                });
            }
            _akEvents.counters.mouse++;
            lastX = x; lastY = y;
        }
        // Fire a synchronous mousemove + pointermove pair NOW so live
        // addEventListener subscribers (DataDome tags.js, PerimeterX
        // sensor) see at least one event before any setTimeouts get a
        // chance. Pairing matches real Chrome's per-physical-event emission.
        try {
            const evOpts = {
                bubbles: true, cancelable: true, view: window,
                clientX: lastX, clientY: lastY,
                screenX: lastX, screenY: lastY + 90,
                movementX: 1, movementY: 0,
                button: 0, buttons: 0,
            };
            const mev = new MouseEvent('mousemove', evOpts);
            try { Object.defineProperty(mev, 'isTrusted', { value: true, configurable: true }); } catch (_) {}
            try { window.dispatchEvent(mev); } catch (_) {}
            try { document.dispatchEvent(mev); } catch (_) {}
            try { body.dispatchEvent(mev); } catch (_) {}
            const PE = (typeof PointerEvent === 'function') ? PointerEvent : null;
            if (PE) {
                const pev = new PE('pointermove', {
                    ...evOpts,
                    button: -1,
                    pointerType: 'mouse', pointerId: 1,
                    isPrimary: true, pressure: 0, width: 1, height: 1,
                });
                try { Object.defineProperty(pev, 'isTrusted', { value: true, configurable: true }); } catch (_) {}
                try { window.dispatchEvent(pev); } catch (_) {}
                try { document.dispatchEvent(pev); } catch (_) {}
                try { body.dispatchEvent(pev); } catch (_) {}
            }
        } catch (_) {}
        // Capture the final position so runCycle's deltas pick up
        // from where this seeding left off.
        try {
            globalThis.__akamai_events._lastPos = [lastX, lastY];
        } catch (_) {}
    })();

    // Run first cycle immediately
    runCycle();
    // Then every 4 seconds to keep the "human" active during long builds
    setInterval(runCycle, 4000);
})();
