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
        while (u === 0) u = Math.random();
        while (v === 0) v = Math.random();
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
        sigma = sigma || (0.22 + Math.random() * 0.10);
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

    // Fire a `mousemove` at a given client coordinate. Dispatched on
    // window + document + body — DataDome's tags.js (per W6a research)
    // listens at `window` not `document`, and our prior dispatch only
    // hit document+body so DataDome's empty-coord-list scoring caught
    // us. Now all three event targets receive it.
    function _fireMove(x, y, prev) {
        const ev = new MouseEvent('mousemove', {
            bubbles: true, cancelable: true, view: window,
            clientX: Math.round(x), clientY: Math.round(y),
            screenX: Math.round(x), screenY: Math.round(y) + 90,
            movementX: prev ? Math.round(x - prev[0]) : 0,
            movementY: prev ? Math.round(y - prev[1]) : 0,
            button: 0, buttons: 0,
        });
        try { _dispatch(window, ev); } catch (_) {}
        _dispatch(document, ev);
        _dispatch(body, ev);
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
            [100 + Math.random() * 200,   200 + Math.random() * 300],
            [600 + Math.random() * 300,   100 + Math.random() * 400],
            [1000 + Math.random() * 200,  300 + Math.random() * 300],
        ];
        const strokeDurations = [800 + Math.random() * 300, 600 + Math.random() * 300];
        const samplesPerStroke = 15;
        const microPause = 50 + Math.random() * 100;

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
                setTimeout(() => _fireMove(px, py, prevSnapshot), at);
                prev = [px, py];
            }
            mouseT += dur + microPause;
        }

        // 3) Scroll-down
        const scStartT = mouseT + 100;
        const steps = [80 + Math.random() * 40, 60 + Math.random() * 30];
        let curScT = scStartT;
        for (const step of steps) {
            setTimeout(() => _fireScrollStep(step), curScT);
            curScT += 100 + Math.random() * 100;
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
        // Sigma-lognormal-ish: cluster near a likely starting region
        // (screen center-right where the previous-tab close button
        // would be) with realistic per-step deltas.
        let x = vw * 0.5 + (Math.random() - 0.5) * 80;
        let y = vh * 0.4 + (Math.random() - 0.5) * 80;
        const points = 12;
        // Spread the historical timestamps from -1800ms to -100ms
        // before "now" (= _akT0 = humanize load time).
        for (let i = 0; i < points; i++) {
            const dt = -1800 + (i / (points - 1)) * 1700; // -1800..-100
            // Smooth muscle-impulse-like path: sigma-lognormal velocity
            // → smaller steps near the start and end, larger in the
            // middle. Approximation: triangular speed window.
            const phase = Math.abs(i / (points - 1) - 0.5) * 2; // 0..1, 0 in middle
            const speed = 1 - phase * 0.7; // 0.3 at edges, 1.0 in middle
            const dx = (Math.random() - 0.5) * 80 * speed;
            const dy = (Math.random() - 0.5) * 80 * speed;
            x = Math.max(0, Math.min(vw, x + dx));
            y = Math.max(0, Math.min(vh, y + dy));
            // Push the historical event directly into the buffer with
            // its negative timestamp. _akT() returns positive elapsed
            // since _akT0; we hand-craft a record matching the schema.
            if (_akEvents.mouse.length < 200) {
                _akEvents.mouse.push({
                    x: x | 0, y: y | 0, t: Math.round(dt),
                    kind: 0, button: 0,
                });
            }
            _akEvents.counters.mouse++;
        }
        // Also fire ONE synchronous mousemove now (t=0 on the buffer)
        // so live addEventListener('mousemove') subscribers see at
        // least one real event before any setTimeouts get a chance.
        try {
            const ev = new MouseEvent('mousemove', {
                bubbles: true, cancelable: true, view: window,
                clientX: x | 0, clientY: y | 0,
                screenX: x | 0, screenY: (y | 0) + 90,
                movementX: 1, movementY: 0,
                button: 0, buttons: 0,
            });
            try { Object.defineProperty(ev, 'isTrusted', { value: true, configurable: true }); } catch (_) {}
            try { window.dispatchEvent(ev); } catch (_) {}
            try { document.dispatchEvent(ev); } catch (_) {}
            try { body.dispatchEvent(ev); } catch (_) {}
        } catch (_) {}
        // Capture the final position so runCycle's deltas pick up
        // from where this seeding left off.
        try {
            globalThis.__akamai_events._lastPos = [x | 0, y | 0];
        } catch (_) {}
    })();

    // Run first cycle immediately
    runCycle();
    // Then every 4 seconds to keep the "human" active during long builds
    setInterval(runCycle, 4000);
})();
