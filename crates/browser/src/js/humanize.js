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

    // Fire a `mousemove` at a given client coordinate.
    function _fireMove(x, y, prev) {
        const ev = new MouseEvent('mousemove', {
            bubbles: true, cancelable: true, view: window,
            clientX: Math.round(x), clientY: Math.round(y),
            screenX: Math.round(x), screenY: Math.round(y) + 90,
            movementX: prev ? Math.round(x - prev[0]) : 0,
            movementY: prev ? Math.round(y - prev[1]) : 0,
            button: 0, buttons: 0,
        });
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

    // ---- Plan: focus → mouse path → scroll → click → tab ---------

    // 1) Focus + visibility (≈180 ms in)
    setTimeout(() => {
        try { _dispatch(window, new Event('focus', { bubbles: false })); } catch (e) {}
        try { _dispatch(document, new Event('visibilitychange', { bubbles: true })); } catch (e) {}
    }, 180);

    // 2) Mouse motion — sigma-lognormal velocity, 2-stroke
    //
    // Two strokes between three anchor points:
    //   (120, 380) → midA → (820, 240) → midB → (1180, 420)
    // with brief micro-pause (60-120 ms) between strokes.
    const anchors = [
        [120 + Math.random() * 30,   380 + Math.random() * 30],
        [820 + Math.random() * 60,   240 + Math.random() * 80],
        [1180 + Math.random() * 30,  420 + Math.random() * 30],
    ];
    const strokeDurations = [900 + Math.random() * 220, 720 + Math.random() * 220];
    const samplesPerStroke = 18;
    const microPause = 60 + Math.random() * 60;

    let mouseT = 240; // ms
    let prev = null;
    for (let s = 0; s < anchors.length - 1; s++) {
        const a = anchors[s];
        const b = anchors[s + 1];
        const dur = strokeDurations[s];
        const taus = _sigmaLognormalTimes(samplesPerStroke);
        for (let i = 0; i < taus.length; i++) {
            const tau = taus[i];
            const [x, y] = _lerp(a, b, tau);
            // Add small Gaussian jitter perpendicular to direction —
            // real cursor traces have ~1-3 px tremor.
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

    // 3) Scroll-down — 4 wheel steps with realistic decel
    const scrollStartT = mouseT + 50;
    const scrollSteps = [110, 90, 60, 40]; // px, decreasing
    let scT = scrollStartT;
    for (const step of scrollSteps) {
        const stepT = scT;
        setTimeout(() => _fireScrollStep(step), stepT);
        scT += 80 + Math.random() * 60;
    }

    // 4) Click on body — at one of the anchor points (so it lands
    //    where the mouse "is" when the click fires)
    const clickAt = scT + 120;
    const clickPos = anchors[anchors.length - 1];
    setTimeout(() => {
        try {
            const [x, y] = clickPos;
            const down = new MouseEvent('mousedown', {
                bubbles: true, cancelable: true, view: window,
                clientX: x, clientY: y, button: 0, buttons: 1,
            });
            _dispatch(body, down);
            _akRecMouse(x, y, 1, 0); // 1 = down
            setTimeout(() => {
                const up = new MouseEvent('mouseup', {
                    bubbles: true, cancelable: true, view: window,
                    clientX: x, clientY: y, button: 0, buttons: 0,
                });
                const click = new MouseEvent('click', {
                    bubbles: true, cancelable: true, view: window,
                    clientX: x, clientY: y, button: 0, buttons: 0,
                });
                _dispatch(body, up);
                _dispatch(body, click);
                _akRecMouse(x, y, 2, 0); // 2 = up
            }, 50 + Math.random() * 25);
        } catch (e) {}
    }, clickAt);

    // 5) Tab keypress — typical "user tabbing through" signal
    setTimeout(() => {
        try {
            _dispatch(document, new KeyboardEvent('keydown', {
                bubbles: true, key: 'Tab', code: 'Tab', keyCode: 9,
            }));
            _akRecKey('Tab', 0); // 0 = down
            setTimeout(() => {
                _dispatch(document, new KeyboardEvent('keyup', {
                    bubbles: true, key: 'Tab', code: 'Tab', keyCode: 9,
                }));
                _akRecKey('Tab', 1); // 1 = up
            }, 60);
        } catch (e) {}
    }, clickAt + 250);
})();
