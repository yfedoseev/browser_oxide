// WHATWG Structured Clone Algorithm.
//
// Implements `globalThis.structuredClone(value, options?)` per
// https://html.spec.whatwg.org/multipage/structured-data.html#structured-cloning
//
// Supported types: primitives (number, string, boolean, null, undefined,
// bigint), Date, RegExp, Map, Set, Array, plain Object, ArrayBuffer,
// all TypedArray variants + DataView, Blob (via our Blob class). Cycles
// are preserved via a WeakMap of seen → cloned objects.
//
// Throws `DOMException("...", "DataCloneError")` on functions, symbols,
// and DOM nodes. Throws for anything with a Proxy trap that rejects the
// read (we don't try to defeat the proxy).
//
// This is used by:
//   1. `Worker.postMessage(msg)` — serializes before the thread hop so
//      the receiver sees a real deep copy, not a JSON round-trip that
//      loses TypedArrays / Dates / Maps.
//   2. `IndexedDB` stored values — values must survive put→get cycles.
//   3. Any site code calling `structuredClone()` directly.

((globalThis) => {
    // Wire serialization (serializeForWire / deserializeFromWire) must always
    // be registered for Worker.postMessage to survive the JSON thread-hop.
    // These are independent of whether structuredClone is natively present.
    // The structuredClone polyfill guard is applied separately below.

    const TAG = "__boxsc";

    function _b64encodeBytes(u8) {
        let bin = "";
        for (let i = 0; i < u8.length; i++) bin += String.fromCharCode(u8[i]);
        return (typeof globalThis.btoa === "function"
            ? globalThis.btoa(bin)
            : bin);
    }

    function _b64decodeToUint8(str) {
        const bin =
            typeof globalThis.atob === "function"
                ? globalThis.atob(str)
                : str;
        const out = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
        return out;
    }

    function _serializeForWire(value, seen) {
        if (value === null) return null;
        const t = typeof value;
        if (t === "undefined") return { [TAG]: "undefined" };
        if (t === "bigint") return { [TAG]: "bigint", v: value.toString() };
        if (t === "number" || t === "string" || t === "boolean") return value;
        if (t === "function" || t === "symbol") {
            const err = new Error(t + " values cannot be transferred via postMessage");
            err.name = "DataCloneError";
            throw err;
        }
        seen = seen || new WeakMap();
        if (seen.has(value)) return null;
        if (value instanceof Date) {
            return { [TAG]: "Date", v: value.getTime() };
        }
        if (value instanceof RegExp) {
            return { [TAG]: "RegExp", source: value.source, flags: value.flags };
        }
        if (value instanceof ArrayBuffer) {
            const u8 = new Uint8Array(value);
            return { [TAG]: "ArrayBuffer", b: _b64encodeBytes(u8) };
        }
        if (ArrayBuffer.isView(value) && !(value instanceof DataView)) {
            const u8 = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
            return { [TAG]: "TypedArray", ctor: value.constructor.name, b: _b64encodeBytes(u8) };
        }
        if (value instanceof DataView) {
            const u8 = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
            return { [TAG]: "DataView", b: _b64encodeBytes(u8) };
        }
        if (value instanceof Map) {
            seen.set(value, true);
            const entries = [];
            for (const [k, v] of value) {
                entries.push([_serializeForWire(k, seen), _serializeForWire(v, seen)]);
            }
            return { [TAG]: "Map", entries };
        }
        if (value instanceof Set) {
            seen.set(value, true);
            const items = [];
            for (const v of value) items.push(_serializeForWire(v, seen));
            return { [TAG]: "Set", items };
        }
        if (Array.isArray(value)) {
            seen.set(value, true);
            const out = new Array(value.length);
            for (let i = 0; i < value.length; i++) out[i] = _serializeForWire(value[i], seen);
            return out;
        }
        seen.set(value, true);
        const out = {};
        for (const key of Object.keys(value)) out[key] = _serializeForWire(value[key], seen);
        return out;
    }

    function _deserializeFromWire(value) {
        if (value === null) return null;
        const t = typeof value;
        if (t === "number" || t === "string" || t === "boolean") return value;
        if (t !== "object") return value;
        if (Array.isArray(value)) {
            const out = new Array(value.length);
            for (let i = 0; i < value.length; i++) out[i] = _deserializeFromWire(value[i]);
            return out;
        }
        const tag = value[TAG];
        if (tag) {
            switch (tag) {
                case "undefined": return undefined;
                case "bigint": return BigInt(value.v);
                case "Date": return new Date(value.v);
                case "RegExp": return new RegExp(value.source, value.flags);
                case "ArrayBuffer": {
                    const u8 = _b64decodeToUint8(value.b);
                    const ab = new ArrayBuffer(u8.byteLength);
                    new Uint8Array(ab).set(u8);
                    return ab;
                }
                case "TypedArray": {
                    const u8 = _b64decodeToUint8(value.b);
                    const ab = new ArrayBuffer(u8.byteLength);
                    new Uint8Array(ab).set(u8);
                    const Ctor = globalThis[value.ctor] || Uint8Array;
                    try { return new Ctor(ab); } catch (_) { return new Uint8Array(ab); }
                }
                case "DataView": {
                    const u8 = _b64decodeToUint8(value.b);
                    const ab = new ArrayBuffer(u8.byteLength);
                    new Uint8Array(ab).set(u8);
                    return new DataView(ab);
                }
                case "Map": {
                    const m = new Map();
                    for (const [k, v] of value.entries) m.set(_deserializeFromWire(k), _deserializeFromWire(v));
                    return m;
                }
                case "Set": {
                    const s = new Set();
                    for (const v of value.items) s.add(_deserializeFromWire(v));
                    return s;
                }
                default: break;
            }
        }
        const out = {};
        for (const key of Object.keys(value)) out[key] = _deserializeFromWire(value[key]);
        return out;
    }

    if (!globalThis.__boxide) globalThis.__boxide = {};
    globalThis.__boxide.serializeForWire = _serializeForWire;
    globalThis.__boxide.deserializeFromWire = _deserializeFromWire;

    // structuredClone polyfill — only install if V8 doesn't provide it natively.
    if (typeof globalThis.structuredClone === "function") {
        return;
    }

    // Lazy lookup of the DOMException constructor. window_bootstrap.js
    // installs it, so by the time structuredClone runs at page load
    // it exists; but during bootstrap execution order it may not.
    function _dataCloneError(msg) {
        try {
            return new DOMException(msg, "DataCloneError");
        } catch (_e) {
            const err = new Error(msg);
            err.name = "DataCloneError";
            return err;
        }
    }

    function _isTypedArray(v) {
        return ArrayBuffer.isView(v) && !(v instanceof DataView);
    }

    function clone(value, seen) {
        // Primitives + null + undefined — return as-is.
        if (value === null) return null;
        const t = typeof value;
        if (t === "undefined" || t === "number" || t === "string" || t === "boolean" || t === "bigint") {
            return value;
        }
        if (t === "function" || t === "symbol") {
            throw _dataCloneError(
                t + " values cannot be serialized by structuredClone"
            );
        }
        // From here on, `value` is an object.
        if (seen.has(value)) {
            return seen.get(value);
        }

        // Date — clone with same time value.
        if (value instanceof Date) {
            const c = new Date(value.getTime());
            seen.set(value, c);
            return c;
        }
        // RegExp — preserve source and flags, reset lastIndex to 0 per spec.
        if (value instanceof RegExp) {
            const c = new RegExp(value.source, value.flags);
            seen.set(value, c);
            return c;
        }
        // ArrayBuffer — copy all bytes.
        if (value instanceof ArrayBuffer) {
            const c = value.slice(0);
            seen.set(value, c);
            return c;
        }
        // DataView — clone the underlying buffer and construct a new view
        // over the same byte range.
        if (value instanceof DataView) {
            const bufCopy = value.buffer.slice(
                value.byteOffset,
                value.byteOffset + value.byteLength
            );
            const c = new DataView(bufCopy);
            seen.set(value, c);
            return c;
        }
        // TypedArray — `new value.constructor(value)` copies elements.
        // For Uint8Array.from a typed array, this produces a fresh
        // backing ArrayBuffer matching Chrome's structuredClone.
        if (_isTypedArray(value)) {
            const c = new value.constructor(value);
            seen.set(value, c);
            return c;
        }
        // Map — preserve insertion order, clone keys and values.
        if (value instanceof Map) {
            const c = new Map();
            seen.set(value, c);
            for (const [k, v] of value) {
                c.set(clone(k, seen), clone(v, seen));
            }
            return c;
        }
        // Set — same idea.
        if (value instanceof Set) {
            const c = new Set();
            seen.set(value, c);
            for (const v of value) {
                c.add(clone(v, seen));
            }
            return c;
        }
        // Array — clone in order. Sparse arrays preserve holes via
        // `i in value` tests (real Chrome does this).
        if (Array.isArray(value)) {
            const c = new Array(value.length);
            seen.set(value, c);
            for (let i = 0; i < value.length; i++) {
                if (i in value) {
                    c[i] = clone(value[i], seen);
                }
            }
            return c;
        }
        // Blob — copy the underlying bytes + type. `new Blob([blob])`
        // clones the contents but drops .type, so we reattach it.
        if (typeof Blob !== "undefined" && value instanceof Blob) {
            const c = new Blob([value], { type: value.type || "" });
            seen.set(value, c);
            return c;
        }
        // Error objects — structured clone preserves name + message;
        // stack is implementation-defined (Chrome preserves it, we do too).
        if (value instanceof Error) {
            const Ctor = value.constructor || Error;
            let c;
            try {
                c = new Ctor(value.message);
            } catch (_e) {
                c = new Error(value.message);
            }
            c.name = value.name;
            if (value.stack) c.stack = value.stack;
            seen.set(value, c);
            return c;
        }
        // DOM nodes / Windows / other host objects — DataCloneError.
        // We detect the most common host types by name or internal slots.
        const ctorName = value.constructor && value.constructor.name;
        if (
            ctorName &&
            (ctorName.startsWith("HTML") ||
                ctorName === "Node" ||
                ctorName === "Element" ||
                ctorName === "Document" ||
                ctorName === "Window" ||
                ctorName === "PluginArray" ||
                ctorName === "MimeTypeArray" ||
                ctorName === "Plugin" ||
                ctorName === "MimeType" ||
                ctorName === "AudioContext" ||
                ctorName === "OfflineAudioContext" ||
                ctorName === "BaseAudioContext" ||
                ctorName === "AudioNode" ||
                ctorName === "AudioParam")
        ) {
            throw _dataCloneError(
                `Failed to execute 'structuredClone' on 'Window': ${ctorName} object could not be cloned.`
            );
        }
        // Plain object — enumerable own string keys, in insertion order
        // (Object.keys matches Chrome's behaviour for plain objects).
        // Symbols are NOT cloned (spec).
        const proto = Object.getPrototypeOf(value);
        if (proto !== null && proto !== Object.prototype) {
            // Subclasses of Object / instances with a custom prototype:
            // the spec says to still clone the own enumerable string
            // properties as a plain object, discarding the prototype.
            // Real Chrome follows this for simple cases; anything more
            // exotic (Proxy, getters throwing, etc.) falls through to
            // the same path.
        }
        const c = {};
        seen.set(value, c);
        for (const key of Object.keys(value)) {
            c[key] = clone(value[key], seen);
        }
        return c;
    }

    globalThis.structuredClone = function structuredClone(value, options) {
        const _transfer = (options && options.transfer) || [];
        // Transferables are not yet implemented — cloning them just
        // copies their contents. This matches what the existing
        // Worker.postMessage path already does, so no regression.
        // TODO(A6 / B2): neuter transferred buffers after clone.
        if (!Array.isArray(_transfer)) {
            throw new TypeError("structuredClone: transfer must be an array");
        }
        return clone(value, new WeakMap());
    };
    // Mask as native — Kasada's `kasada_function_toString_audit` greps for
    // raw JS bodies of polyfilled built-ins. Without this, `structuredClone
    // .toString()` returns the function source and identifies the engine
    // as non-Chrome. Helpers are registered in stealth_bootstrap.js.
    try {
        if (typeof globalThis._maskFunction === 'function') {
            globalThis._maskFunction(globalThis.structuredClone, 'structuredClone');
        }
    } catch (_e) {}

})(globalThis);
