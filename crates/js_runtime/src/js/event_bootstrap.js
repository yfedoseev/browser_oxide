((globalThis) => {
    const _listeners = new Map(); // nodeId → Map<eventType, [{callback, capture, once}]>
    // ---- Trusted-event authenticity (v0.1.0 behavioral E1) ----------------
    // `isTrusted` MUST be both unforgeable and shaped like a real browser's:
    //   * a GETTER on Event.prototype — NOT an own data property. Anti-bots
    //     (DataDome/Akamai/Kasada/PerimeterX) read
    //     `getOwnPropertyDescriptor(evt,'isTrusted')` and flag an own-data
    //     `isTrusted` as synthetic (real browsers expose it via the prototype).
    //   * backed by a MODULE-PRIVATE WeakSet that page JS cannot reach. The
    //     old design keyed trust off `Symbol.for('__bo_trusted__')` — the
    //     GLOBAL symbol registry — so any page could re-derive the symbol and
    //     forge a trusted event (`new Event('x', {[Symbol.for(...)]: true})`).
    // Only our privileged init scripts mint trust, via `_markTrusted`, handed
    // off below through a temp global they capture-and-delete before any page
    // script runs. There is no in-band (options/symbol) path from page JS.
    const _trustedEvents = new WeakSet();
    const _markTrusted = (ev) => {
        try { if (ev && typeof ev === 'object') _trustedEvents.add(ev); } catch (_) {}
        return ev;
    };

    class Event {
        constructor(type, options = {}) {
            this.type = type;
            this.bubbles = !!options.bubbles;
            this.cancelable = !!options.cancelable;
            this.composed = !!options.composed;
            this.defaultPrevented = false;
            this.target = null;
            this.currentTarget = null;
            this.eventPhase = 0;
            // NOTE: `isTrusted` is intentionally NOT set here. It is a prototype
            // getter (installed below) reading the private WeakSet — default
            // false for page-constructed events; trusted only when our
            // privileged dispatch path calls `_markTrusted(ev)`.
            this.timeStamp = performance.now();
            this._stopped = false;
            this._stoppedImmediate = false;
        }
        preventDefault() {
            if (this.cancelable) this.defaultPrevented = true;
        }
        stopPropagation() { this._stopped = true; }
        stopImmediatePropagation() { this._stopped = true; this._stoppedImmediate = true; }
        composedPath() {
            const path = [];
            let node = this.target;
            while (node) { path.push(node); node = node.parentNode; }
            return path;
        }
        // Phase constants
        static NONE = 0;
        static CAPTURING_PHASE = 1;
        static AT_TARGET = 2;
        static BUBBLING_PHASE = 3;
    }

    // isTrusted as an inherited, native-masked prototype accessor backed by the
    // private WeakSet. Subclasses (CustomEvent, MouseEvent, …) inherit it. The
    // descriptor shape matches real Chrome: {get: ƒ, set: undefined,
    // enumerable: true, configurable: true}.
    Object.defineProperty(Event.prototype, 'isTrusted', {
        configurable: true,
        enumerable: true,
        get: (typeof _maskFunction === 'function')
            ? _maskFunction(function () { return _trustedEvents.has(this); }, 'get isTrusted')
            : function () { return _trustedEvents.has(this); },
    });

    class CustomEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.detail = options.detail !== undefined ? options.detail : null;
        }
        initCustomEvent(type, bubbles, cancelable, detail) {
            this.type = type;
            this.bubbles = bubbles;
            this.cancelable = cancelable;
            this.detail = detail;
        }
    }

    // --- UI Event hierarchy ---
    class UIEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.view = options.view || globalThis;
            this.detail = options.detail || 0;
        }
    }

    class MouseEvent extends UIEvent {
        constructor(type, options = {}) {
            super(type, { bubbles: true, cancelable: true, ...options });
            this.screenX = options.screenX || 0;
            this.screenY = options.screenY || 0;
            this.clientX = options.clientX || 0;
            this.clientY = options.clientY || 0;
            this.pageX = options.pageX || this.clientX;
            this.pageY = options.pageY || this.clientY;
            this.offsetX = options.offsetX || 0;
            this.offsetY = options.offsetY || 0;
            this.button = options.button || 0;
            this.buttons = options.buttons || 0;
            this.ctrlKey = !!options.ctrlKey;
            this.shiftKey = !!options.shiftKey;
            this.altKey = !!options.altKey;
            this.metaKey = !!options.metaKey;
            this.relatedTarget = options.relatedTarget || null;
            this.movementX = options.movementX || 0;
            this.movementY = options.movementY || 0;
        }
        getModifierState(key) { return false; }
    }

    class KeyboardEvent extends UIEvent {
        constructor(type, options = {}) {
            super(type, { bubbles: true, cancelable: true, ...options });
            this.key = options.key || "";
            this.code = options.code || "";
            this.keyCode = options.keyCode || 0;
            this.charCode = options.charCode || 0;
            this.which = options.which || options.keyCode || 0;
            this.ctrlKey = !!options.ctrlKey;
            this.shiftKey = !!options.shiftKey;
            this.altKey = !!options.altKey;
            this.metaKey = !!options.metaKey;
            this.repeat = !!options.repeat;
            this.isComposing = !!options.isComposing;
            this.location = options.location || 0;
        }
        getModifierState(key) { return false; }
    }

    class InputEvent extends UIEvent {
        constructor(type, options = {}) {
            super(type, { bubbles: true, cancelable: false, ...options });
            this.data = options.data || null;
            this.inputType = options.inputType || "";
            this.isComposing = !!options.isComposing;
        }
    }

    class FocusEvent extends UIEvent {
        constructor(type, options = {}) {
            super(type, options);
            this.relatedTarget = options.relatedTarget || null;
        }
    }

    class PointerEvent extends MouseEvent {
        constructor(type, options = {}) {
            super(type, options);
            this.pointerId = options.pointerId || 0;
            this.width = options.width || 1;
            this.height = options.height || 1;
            this.pressure = options.pressure || 0;
            this.tangentialPressure = options.tangentialPressure || 0;
            this.tiltX = options.tiltX || 0;
            this.tiltY = options.tiltY || 0;
            this.twist = options.twist || 0;
            this.pointerType = options.pointerType || "mouse";
            this.isPrimary = options.isPrimary !== undefined ? options.isPrimary : true;
        }
    }

    class WheelEvent extends MouseEvent {
        constructor(type, options = {}) {
            super(type, options);
            this.deltaX = options.deltaX || 0;
            this.deltaY = options.deltaY || 0;
            this.deltaZ = options.deltaZ || 0;
            this.deltaMode = options.deltaMode || 0;
        }
        static DOM_DELTA_PIXEL = 0;
        static DOM_DELTA_LINE = 1;
        static DOM_DELTA_PAGE = 2;
    }

    class TouchEvent extends UIEvent {
        constructor(type, options = {}) {
            super(type, { bubbles: true, cancelable: true, ...options });
            this.touches = options.touches || [];
            this.targetTouches = options.targetTouches || [];
            this.changedTouches = options.changedTouches || [];
            this.ctrlKey = !!options.ctrlKey;
            this.shiftKey = !!options.shiftKey;
            this.altKey = !!options.altKey;
            this.metaKey = !!options.metaKey;
        }
    }

    class MessageEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.data = options.data !== undefined ? options.data : null;
            this.origin = options.origin || "";
            this.lastEventId = options.lastEventId || "";
            this.source = options.source || null;
            this.ports = options.ports || [];
        }
    }

    class ErrorEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.message = options.message || "";
            this.filename = options.filename || "";
            this.lineno = options.lineno || 0;
            this.colno = options.colno || 0;
            this.error = options.error || null;
        }
    }

    class ProgressEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.lengthComputable = !!options.lengthComputable;
            this.loaded = options.loaded || 0;
            this.total = options.total || 0;
        }
    }

    class AnimationEvent extends Event {
        constructor(type, options = {}) {
            super(type, { bubbles: true, ...options });
            this.animationName = options.animationName || "";
            this.elapsedTime = options.elapsedTime || 0;
            this.pseudoElement = options.pseudoElement || "";
        }
    }

    class TransitionEvent extends Event {
        constructor(type, options = {}) {
            super(type, { bubbles: true, ...options });
            this.propertyName = options.propertyName || "";
            this.elapsedTime = options.elapsedTime || 0;
            this.pseudoElement = options.pseudoElement || "";
        }
    }

    class ClipboardEvent extends Event {
        constructor(type, options = {}) {
            super(type, { bubbles: true, cancelable: true, ...options });
            this.clipboardData = options.clipboardData || null;
        }
    }

    class PopStateEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.state = options.state !== undefined ? options.state : null;
        }
    }

    class HashChangeEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.oldURL = options.oldURL || "";
            this.newURL = options.newURL || "";
        }
    }

    class StorageEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.key = options.key || null;
            this.oldValue = options.oldValue || null;
            this.newValue = options.newValue || null;
            this.url = options.url || "";
            this.storageArea = options.storageArea || null;
        }
    }

    class PageTransitionEvent extends Event {
        constructor(type, options = {}) {
            super(type, options);
            this.persisted = !!options.persisted;
        }
    }

    class BeforeUnloadEvent extends Event {
        constructor(type, options = {}) {
            super(type, { cancelable: true, ...options });
            this.returnValue = "";
        }
    }

    class DragEvent extends MouseEvent {
        constructor(type, options = {}) {
            super(type, options);
            this.dataTransfer = options.dataTransfer || null;
        }
    }

    // --- EventTarget core logic ---
    const _nodeListeners = new Map(); // nodeId → Map<eventType, [{callback, capture, once}]>
    const _objListeners = new WeakMap(); // object → Map<eventType, [{callback, capture, once}]>

    const _getNodeIdOrMinusOne = (globalThis.__browser_oxide && globalThis.__browser_oxide._getNodeId)
        ? globalThis.__browser_oxide._getNodeId
        : (() => -1);

    function _getListenersMap(target) {
        const nodeId = _getNodeIdOrMinusOne(target);
        // Node IDs: >0 for elements/text, 0 for document (sometimes), -999 for window.
        // We use the Map for any node that has a stable ID.
        if (nodeId !== -1) {
            let m = _nodeListeners.get(nodeId);
            if (!m) { m = new Map(); _nodeListeners.set(nodeId, m); }
            return m;
        } else {
            let m = _objListeners.get(target);
            if (!m) { m = new Map(); _objListeners.set(target, m); }
            return m;
        }
    }

    function _getListeners(target, type) {
        const nodeMap = _getListenersMap(target);
        let arr = nodeMap.get(type);
        if (!arr) { arr = []; nodeMap.set(type, arr); }
        return arr;
    }

    const _addEventListener = function addEventListener(type, callback, options) {
        if (typeof callback !== "function" && typeof callback !== "object") return;
        const capture = typeof options === "boolean" ? options : !!(options && options.capture);
        const once = typeof options === "object" && options ? !!options.once : false;
        const passive = typeof options === "object" && options ? !!options.passive : false;
        const listeners = _getListeners(this, type);
        // Prevent duplicate
        if (listeners.some(l => l.callback === callback && l.capture === capture)) return;
        listeners.push({ callback, capture, once, passive });
    };

    const _removeEventListener = function removeEventListener(type, callback, options) {
        const capture = typeof options === "boolean" ? options : !!(options && options.capture);
        const listeners = _getListeners(this, type);
        const idx = listeners.findIndex(l => l.callback === callback && l.capture === capture);
        if (idx !== -1) listeners.splice(idx, 1);
    };

    const _dispatchEvent = function dispatchEvent(event) {
        if (!(event instanceof Event)) {
            throw new TypeError("Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.");
        }
        event.target = this;
        const nodeId = _getNodeIdOrMinusOne(this);

        // Build propagation path (target → root) if it's a DOM node.
        // Real Chrome's EventTarget.prototype.dispatchEvent handles the
        // tree-walk automatically if 'this' is a Node.
        const path = [];
        if (nodeId !== -1 && this.parentNode !== undefined) {
            let current = this;
            while (current) {
                path.push(current);
                current = current.parentNode;
            }
        }

        // Capture phase (root → target)
        if (path.length > 0 && !event._stopped) {
            for (let i = path.length - 1; i > 0; i--) {
                event.currentTarget = path[i];
                event.eventPhase = 1;
                _fireListeners(path[i], event, true);
                if (event._stopped) break;
            }
        }

        // Target phase
        if (!event._stopped) {
            event.currentTarget = this;
            event.eventPhase = 2;
            _fireListeners(this, event, false);
            _fireListeners(this, event, true);
        }

        // Bubble phase (target → root)
        if (path.length > 0 && !event._stopped && event.bubbles) {
            for (let i = 1; i < path.length; i++) {
                event.currentTarget = path[i];
                event.eventPhase = 3;
                _fireListeners(path[i], event, false);
                if (event._stopped) break;
            }
        }

        event.eventPhase = 0;
        event.currentTarget = null;
        return !event.defaultPrevented;
    };

    function _fireListeners(target, event, capturePhase) {
        // --- 1. Fire on* handler (Target phase only, not capture phase) ---
        if (!capturePhase && !event._stoppedImmediate) {
            const handlerName = `on${event.type}`;
            const handler = target[handlerName];
            if (typeof handler === "function") {
                try {
                    handler.call(target, event);
                } catch (e) {
                    console.error(e);
                }
            }
        }

        // --- 2. Fire registered listeners ---
        const listeners = _getListeners(target, event.type);
        const toRemove = [];
        for (let i = 0; i < listeners.length; i++) {
            const l = listeners[i];
            if (l.capture !== capturePhase) continue;
            if (event._stoppedImmediate) break;
            if (typeof l.callback === "function") {
                l.callback.call(target, event);
            } else if (l.callback && typeof l.callback.handleEvent === "function") {
                l.callback.handleEvent(event);
            }
            if (l.once) toRemove.push(i);
        }
        for (let i = toRemove.length - 1; i >= 0; i--) {
            listeners.splice(toRemove[i], 1);
        }
    }

    // Install on EventTarget.prototype — this is the canonical location.
    // Real Chrome has them as configurable/writable/enumerable=true.
    const _ET = globalThis.EventTarget;
    if (_ET && _ET.prototype) {
        const proto = _ET.prototype;
        Object.defineProperty(proto, 'addEventListener', {
            value: _addEventListener, writable: true, enumerable: true, configurable: true,
        });
        Object.defineProperty(proto, 'removeEventListener', {
            value: _removeEventListener, writable: true, enumerable: true, configurable: true,
        });
        Object.defineProperty(proto, 'dispatchEvent', {
            value: _dispatchEvent, writable: true, enumerable: true, configurable: true,
        });
    }

    // Ensure Node.prototype does NOT shadow these. Real Chrome's
    // Node.prototype does not have its own addEventListener.
    const origNodeProto = globalThis.Node.prototype;
    if (origNodeProto) {
        delete origNodeProto.addEventListener;
        delete origNodeProto.removeEventListener;
        delete origNodeProto.dispatchEvent;
    }

    // Native-code masking — PerimeterX/HUMAN run
    // `Function.prototype.toString.call(addEventListener)` against both
    // window-level and prototype-level methods. Each must serialize as
    // `function NAME() { [native code] }`.
    if (typeof _maskFunction === 'function') {
        _maskFunction(_addEventListener, 'addEventListener');
        _maskFunction(_removeEventListener, 'removeEventListener');
        _maskFunction(_dispatchEvent, 'dispatchEvent');
    }

    // Window (globalThis) inheritance: real Chrome's Window inherits from
    // EventTarget via the prototype chain. Our Window setup (Window →
    // WindowProperties → EventTarget) should already handle this, but
    // we ensure the global aliases are correct.
    const _winProto = Object.getPrototypeOf(globalThis);
    if (_winProto && _winProto !== Object.prototype) {
        // Just ensure they are there if not inherited.
        if (!('addEventListener' in _winProto)) {
            Object.defineProperty(_winProto, 'addEventListener', {
                value: _addEventListener, writable: true, enumerable: true, configurable: true,
            });
        }
        if (!('removeEventListener' in _winProto)) {
            Object.defineProperty(_winProto, 'removeEventListener', {
                value: _removeEventListener, writable: true, enumerable: true, configurable: true,
            });
        }
        if (!('dispatchEvent' in _winProto)) {
            Object.defineProperty(_winProto, 'dispatchEvent', {
                value: _dispatchEvent, writable: true, enumerable: true, configurable: true,
            });
        }
    } else {
        globalThis.addEventListener = _addEventListener;
        globalThis.removeEventListener = _removeEventListener;
        globalThis.dispatchEvent = _dispatchEvent;
    }

    // Export all event classes
    // SecurityPolicyViolationEvent — what real Chrome dispatches on
    // `document` (and propagates to `window`) when a CSP rule blocks
    // a fetch. Sites can listen for `securitypolicyviolation` to log
    // their own violations; we must surface the same shape so that
    // analytics/telemetry code probing the event fires correctly.
    // Spec: https://www.w3.org/TR/CSP3/#securitypolicyviolationevent
    class SecurityPolicyViolationEvent extends Event {
        constructor(type, init) {
            super(type, init || {});
            const i = init || {};
            this.blockedURI = String(i.blockedURI ?? "");
            this.documentURI = String(i.documentURI ?? (typeof location !== 'undefined' ? location.href : ""));
            this.referrer = String(i.referrer ?? (typeof document !== 'undefined' && document.referrer ? document.referrer : ""));
            this.violatedDirective = String(i.violatedDirective ?? "");
            this.effectiveDirective = String(i.effectiveDirective ?? this.violatedDirective);
            this.originalPolicy = String(i.originalPolicy ?? "");
            this.disposition = String(i.disposition ?? "enforce");
            this.sample = String(i.sample ?? "");
            this.sourceFile = String(i.sourceFile ?? "");
            this.statusCode = +i.statusCode || 0;
            this.lineNumber = +i.lineNumber || 0;
            this.columnNumber = +i.columnNumber || 0;
        }
    }

    globalThis.Event = Event;
    globalThis.CustomEvent = CustomEvent;
    globalThis.SecurityPolicyViolationEvent = SecurityPolicyViolationEvent;
    globalThis.UIEvent = UIEvent;
    globalThis.MouseEvent = MouseEvent;
    globalThis.KeyboardEvent = KeyboardEvent;
    globalThis.InputEvent = InputEvent;
    globalThis.FocusEvent = FocusEvent;
    globalThis.PointerEvent = PointerEvent;
    globalThis.WheelEvent = WheelEvent;
    globalThis.TouchEvent = TouchEvent;
    globalThis.MessageEvent = MessageEvent;
    globalThis.ErrorEvent = ErrorEvent;
    globalThis.ProgressEvent = ProgressEvent;
    globalThis.AnimationEvent = AnimationEvent;
    globalThis.TransitionEvent = TransitionEvent;
    globalThis.ClipboardEvent = ClipboardEvent;
    globalThis.PopStateEvent = PopStateEvent;
    globalThis.HashChangeEvent = HashChangeEvent;
    globalThis.StorageEvent = StorageEvent;
    globalThis.PageTransitionEvent = PageTransitionEvent;
    globalThis.BeforeUnloadEvent = BeforeUnloadEvent;
    globalThis.DragEvent = DragEvent;
    // EventTarget is already defined in dom_bootstrap.js as the base of
    // the Node prototype chain — do not reassign it here or the
    // `document instanceof EventTarget` check will break.

    // Privileged handoff of the trusted-event minter (behavioral E1/E2). Our
    // init scripts (humanize.js) capture this into a closure and `delete` it
    // synchronously at their top — before any page script runs — so page JS
    // never observes it. Non-enumerable to keep it off Object.keys scans even
    // in the brief window before capture.
    try {
        Object.defineProperty(globalThis, '__bo_mark_trusted', {
            value: _markTrusted,
            configurable: true,
            enumerable: false,
            writable: false,
        });
    } catch (_) { /* ignore */ }
})(globalThis);
