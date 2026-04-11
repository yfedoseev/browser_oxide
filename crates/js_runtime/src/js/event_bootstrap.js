((globalThis) => {
    const _listeners = new Map(); // nodeId → Map<eventType, [{callback, capture, once}]>

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
            this.isTrusted = false;
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

    // --- EventTarget on Node.prototype ---
    const origNodeProto = globalThis.Node.prototype;

    function _getListeners(nodeId, type) {
        let nodeMap = _listeners.get(nodeId);
        if (!nodeMap) { nodeMap = new Map(); _listeners.set(nodeId, nodeMap); }
        let arr = nodeMap.get(type);
        if (!arr) { arr = []; nodeMap.set(type, arr); }
        return arr;
    }

    origNodeProto.addEventListener = function(type, callback, options) {
        if (typeof callback !== "function" && typeof callback !== "object") return;
        const nodeId = typeof this._getNodeId === "function" ? this._getNodeId() : 0;
        const capture = typeof options === "boolean" ? options : !!(options && options.capture);
        const once = typeof options === "object" && options ? !!options.once : false;
        const passive = typeof options === "object" && options ? !!options.passive : false;
        const listeners = _getListeners(nodeId, type);
        // Prevent duplicate
        if (listeners.some(l => l.callback === callback && l.capture === capture)) return;
        listeners.push({ callback, capture, once, passive });
    };

    origNodeProto.removeEventListener = function(type, callback, options) {
        const nodeId = typeof this._getNodeId === "function" ? this._getNodeId() : 0;
        const capture = typeof options === "boolean" ? options : !!(options && options.capture);
        const listeners = _getListeners(nodeId, type);
        const idx = listeners.findIndex(l => l.callback === callback && l.capture === capture);
        if (idx !== -1) listeners.splice(idx, 1);
    };

    origNodeProto.dispatchEvent = function(event) {
        event.target = this;
        const nodeId = typeof this._getNodeId === "function" ? this._getNodeId() : 0;

        // Build propagation path (target → root)
        const path = [];
        let current = this;
        while (current) {
            path.push(current);
            current = current.parentNode;
        }

        // Capture phase (root → target)
        if (!event._stopped) {
            for (let i = path.length - 1; i > 0; i--) {
                event.currentTarget = path[i];
                event.eventPhase = 1;
                const nid = typeof path[i]._getNodeId === "function" ? path[i]._getNodeId() : 0;
                _fireListeners(nid, event, true);
                if (event._stopped) break;
            }
        }

        // Target phase
        if (!event._stopped) {
            event.currentTarget = this;
            event.eventPhase = 2;
            _fireListeners(nodeId, event, false);
            _fireListeners(nodeId, event, true);
        }

        // Bubble phase (target → root)
        if (!event._stopped && event.bubbles) {
            for (let i = 1; i < path.length; i++) {
                event.currentTarget = path[i];
                event.eventPhase = 3;
                const nid = typeof path[i]._getNodeId === "function" ? path[i]._getNodeId() : 0;
                _fireListeners(nid, event, false);
                if (event._stopped) break;
            }
        }

        event.eventPhase = 0;
        event.currentTarget = null;
        return !event.defaultPrevented;
    };

    function _fireListeners(nodeId, event, capturePhase) {
        const listeners = _getListeners(nodeId, event.type);
        const toRemove = [];
        for (let i = 0; i < listeners.length; i++) {
            const l = listeners[i];
            if (l.capture !== capturePhase) continue;
            if (event._stoppedImmediate) break;
            if (typeof l.callback === "function") {
                l.callback.call(event.currentTarget, event);
            } else if (l.callback && typeof l.callback.handleEvent === "function") {
                l.callback.handleEvent(event);
            }
            if (l.once) toRemove.push(i);
        }
        for (let i = toRemove.length - 1; i >= 0; i--) {
            listeners.splice(toRemove[i], 1);
        }
    }

    // Also make globalThis (window) an EventTarget
    globalThis.addEventListener = origNodeProto.addEventListener.bind({ _getNodeId: () => -999 });
    globalThis.removeEventListener = origNodeProto.removeEventListener.bind({ _getNodeId: () => -999 });
    globalThis.dispatchEvent = function(event) {
        event.target = globalThis;
        event.currentTarget = globalThis;
        event.eventPhase = 2;
        _fireListeners(-999, event, false);
        event.eventPhase = 0;
        event.currentTarget = null;
        return !event.defaultPrevented;
    };

    // Export all event classes
    globalThis.Event = Event;
    globalThis.CustomEvent = CustomEvent;
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
})(globalThis);
