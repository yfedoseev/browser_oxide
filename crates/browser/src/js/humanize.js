// Opt-in user-input humanizer.
//
// Dispatches a plausible pattern of mousemove / click / focus / keydown
// events into the page right after load. This is a workaround for
// sensor-based bot detectors that flag "zero user input in 2 s" as a
// signal. Real Chrome sessions accumulate dozens of mouse events before
// any sensor VM posts; headless runs see none. The ratios here mirror a
// real user — roughly 15 mousemoves per click with jittery inter-arrival
// times. Install this via `Page::navigate_humanized` rather than the
// default `Page::navigate`, which is input-free and faithful to a clean
// scripted navigation.
(function humanize() {
    const body = document.body || document.documentElement;
    if (!body) return;

    function _dispatch(target, event) {
        try { Object.defineProperty(event, 'isTrusted', { value: true, configurable: true }); }
        catch (e) {}
        target.dispatchEvent(event);
    }

    setTimeout(() => {
        try { _dispatch(window, new Event('focus', { bubbles: false })); } catch (e) {}
        try { _dispatch(document, new Event('visibilitychange', { bubbles: true })); } catch (e) {}
    }, 180);

    const start = 260;
    const count = 30;
    const duration = 2100;
    for (let i = 0; i < count; i++) {
        const delay = start + (i * duration / count) + (Math.random() * 15 - 7);
        const t = i / (count - 1);
        const mt = 1 - t;
        const x = mt * mt * 120 + 2 * mt * t * 700 + t * t * 1180;
        const y = mt * mt * 380 + 2 * mt * t * 120 + t * t * 420;
        setTimeout(() => {
            try {
                const ev = new MouseEvent('mousemove', {
                    bubbles: true,
                    cancelable: true,
                    view: window,
                    clientX: Math.round(x),
                    clientY: Math.round(y),
                    screenX: Math.round(x),
                    screenY: Math.round(y) + 90,
                    movementX: i > 0 ? 4 : 0,
                    movementY: i > 0 ? 2 : 0,
                    button: 0,
                    buttons: 0,
                });
                _dispatch(document, ev);
                _dispatch(body, ev);
            } catch (e) {}
        }, delay);
    }

    [1150, 1920].forEach((delay, idx) => {
        setTimeout(() => {
            try {
                const x = idx === 0 ? 640 : 970;
                const y = idx === 0 ? 180 : 380;
                const down = new MouseEvent('mousedown', {
                    bubbles: true, cancelable: true, view: window,
                    clientX: x, clientY: y, button: 0, buttons: 1,
                });
                _dispatch(body, down);
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
                }, 45 + Math.random() * 30);
            } catch (e) {}
        }, delay);
    });

    setTimeout(() => {
        try {
            _dispatch(document, new KeyboardEvent('keydown', {
                bubbles: true, key: 'Tab', code: 'Tab', keyCode: 9,
            }));
            setTimeout(() => {
                _dispatch(document, new KeyboardEvent('keyup', {
                    bubbles: true, key: 'Tab', code: 'Tab', keyCode: 9,
                }));
            }, 60);
        } catch (e) {}
    }, 900);
})();
