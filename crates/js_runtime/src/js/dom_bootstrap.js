((globalThis) => {
    const core = Deno.core;
    const ops = core.ops;
    const _nodeIds = new WeakMap();
    const _nodeCache = new Map();
    const _scrollState = new Map(); // nodeId -> {top, left}

    function _getNodeId(node) {
        if (node === null || node === undefined) return -1;
        if (node === globalThis || node === globalThis.window) return -999;
        // WeakMap.get on a non-object returns undefined per spec — no throw.
        const id = _nodeIds.get(node);
        if (id === undefined) {
            // node is not a registered DOM node. Returning 0 (the DOCUMENT
            // id) here used to be a "resilience" default, but it caused
            // every appendChild(weirdValue) to surface as
            // appendChild(parent, document) → cycle assertion fires.
            // -1 makes the Rust op layer's `dom.get(NodeId(u32::MAX))` miss
            // and silently no-op, which is the right behaviour for a JS
            // mutation against a non-node argument.
            return -1;
        }
        return id;
    }

    function _wrapNode(nodeId) {
        if (nodeId === null || nodeId === undefined || nodeId === -1) return null;
        const cached = _nodeCache.get(nodeId);
        if (cached) {
            const obj = cached.deref();
            if (obj) return obj;
        }
        const nodeType = ops.op_dom_get_node_type(nodeId);
        return _wrapNodeWithType(nodeId, nodeType);
    }

    function _wrapNodeWithType(nodeId, nodeType) {
        if (nodeId === null || nodeId === undefined || nodeId === -1) return null;
        const cached = _nodeCache.get(nodeId);
        if (cached) {
            const obj = cached.deref();
            if (obj) return obj;
        }
        let node;
        switch (nodeType) {
            case 1:
                node = new Element(nodeId);
                _retargetElementProto(node);
                break;
            case 3: node = new Text(nodeId); break;
            case 8: node = new Comment(nodeId); break;
            case 9: node = _document; break;
            case 11: node = new DocumentFragment(nodeId); break;
            default: node = new Node(nodeId); break;
        }
        _nodeCache.set(nodeId, new WeakRef(node));
        return node;
    }

    // Tracks base URLs (query-stripped) of scripts currently being sync-fetched.
    // Guards against re-entrant fetch loops: e.g. Yandex Metrika's bootstrap IIFE
    // inserts a new <script src="tag.js?timestamp"> while tag.js is still being
    // evaluated. Without this guard the fetch recurses infinitely.
    const _syncFetchInFlight = new Set();

    // Tracks nesting depth of sync eval chains. Each _onNodeInserted call that
    // fetches+evals a script increments this. Scripts beyond MAX nesting are
    // degraded to async — prevents C++ stack overflow when deeply-nested
    // third-party SDKs load more scripts during their own synchronous eval
    // (each pending eval adds a large V8 interpreter frame to the C stack;
    // 6-9 levels can overflow an 8 MB Rust thread stack).
    let _syncEvalDepth = 0;
    const _MAX_SYNC_EVAL_DEPTH = 4;

    // Guards against unbounded `document.write` chains. Two failure modes
    // we observed on bot.sannysoft.com:
    //   (a) A script does `document.write('<script>...</script>')` and the
    //       written script does the same — direct cycle. Caught by depth.
    //   (b) `document.write` dispatches every new node through
    //       `_onNodeInserted`, which evals scripts. If a written script
    //       calls `document.write` again during its eval (synchronously),
    //       we re-enter `_onNodeInserted` from inside its own call.
    let _onNodeInsertedDepth = 0;
    const _MAX_NODE_INSERT_DEPTH = 64;

    function _onNodeInserted(child, sync = true) {
        if (!child) return;
        if (_onNodeInsertedDepth >= _MAX_NODE_INSERT_DEPTH) {
            // Bail — log once and skip. This breaks document.write recursion
            // chains that would otherwise blow the C-stack via deep nested
            // eval -> op_dom_document_write -> _onNodeInserted.
            console.log(`[DOM] _onNodeInserted depth limit (${_MAX_NODE_INSERT_DEPTH}) — skipping`);
            return;
        }
        _onNodeInsertedDepth++;
        try {
            return _onNodeInsertedInner(child, sync);
        } finally {
            _onNodeInsertedDepth--;
        }
    }

    class DOMPointReadOnly {
        constructor(x = 0, y = 0, z = 0, w = 1) {
            this.x = x; this.y = y; this.z = z; this.w = w;
        }
        static fromPoint(p) { return new DOMPointReadOnly(p.x, p.y, p.z, p.w); }
        toJSON() { return { x: this.x, y: this.y, z: this.z, w: this.w }; }
    }
    globalThis.DOMPointReadOnly = DOMPointReadOnly;

    class DOMPoint extends DOMPointReadOnly {
        constructor(x = 0, y = 0, z = 0, w = 1) { super(x, y, z, w); }
    }
    globalThis.DOMPoint = DOMPoint;

    class DOMRectReadOnly {
        constructor(x = 0, y = 0, width = 0, height = 0) {
            this.x = x; this.y = y; this.width = width; this.height = height;
        }
        get top() { return this.y; }
        get left() { return this.x; }
        get right() { return this.x + this.width; }
        get bottom() { return this.y + this.height; }
        toJSON() { return { x: this.x, y: this.y, width: this.width, height: this.height, top: this.top, left: this.left, right: this.right, bottom: this.bottom }; }
    }
    globalThis.DOMRectReadOnly = DOMRectReadOnly;

    class DOMRect extends DOMRectReadOnly {
        constructor(x = 0, y = 0, width = 0, height = 0) { super(x, y, width, height); }
        static fromRect(r) { return new DOMRect(r.x, r.y, r.width, r.height); }
    }
    globalThis.DOMRect = DOMRect;

    if (typeof _maskFunction === 'function') {
        _maskFunction(DOMPointReadOnly, 'DOMPointReadOnly');
        _maskFunction(DOMPoint, 'DOMPoint');
        _maskFunction(DOMRectReadOnly, 'DOMRectReadOnly');
        _maskFunction(DOMRect, 'DOMRect');
    }

    function _onNodeInsertedInner(child, sync = true) {
        // 1. Dynamic script loading
        const childTag = (child.tagName || child.nodeName || "").toLowerCase();
        const type = (child.getAttribute?.('type') || '').toLowerCase();
        const isJs = !type || type === 'text/javascript' || type === 'application/javascript' || type === 'module';
        
        if (childTag === 'script' && !isJs) {
            return; // Skip non-JS scripts like application/ld+json
        }

        const childSrc = (childTag === 'script') ? (child.src || child.getAttribute?.('src')) : null;

        if (childTag === 'script' && !childSrc) {
            const code = child.textContent || child.innerText || '';
            if (code && code.trim()) {
                console.log(`[DOM] executing inline script (${code.length} bytes)`);
                try { (0, eval)(code); } catch (e) {
                    console.log(`[DOM] inline eval error: ${e.message}`);
                }
            }
        }

        if (childTag === 'script' && childSrc) {
            const src = childSrc;
            const scriptEl = child;
            let fullUrl = src;
            if (!src.startsWith('http') && !src.startsWith('data:')) {
                try {
                    const base = globalThis.location ? globalThis.location.href : 'about:blank';
                    fullUrl = new URL(src, base).href;
                } catch(e) {}
            }

            // Third-party trackers known to trigger uncontrolled C-stack recursion
            // inside their own VM (not in our shims). Skip them — they add no
            // signal to fingerprint scoring, and crashing the engine on them
            // costs us all subsequent tests on the page.
            // Known offenders identified via stack-overflow crashes on real
            // sites: bot.sannysoft.com loads Yandex Metrika; leboncoin.fr
            // loads it too.
            const _RECURSIVE_TRACKERS = [
                "mc.yandex.ru/metrika/tag.js",
                "mc.yandex.ru/metrika/watch.js",
                "mc.yandex.ru/webvisor/",
            ];
            for (const pat of _RECURSIVE_TRACKERS) {
                if (fullUrl.includes(pat)) {
                    if (scriptEl.onload) scriptEl.onload(new Event('load'));
                    scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('load'));
                    return;
                }
            }

            if (sync) {
                // Strip query params for in-flight dedup: scripts that reload themselves
                // with a cache-busting timestamp (e.g. Yandex Metrika tag.js?<timestamp>)
                // share the same base URL and would recurse infinitely without this guard.
                const baseUrl = fullUrl.split('?')[0];
                if (_syncFetchInFlight.has(baseUrl)) {
                    // Re-entrant same-URL fetch — fire load event and bail to break the cycle.
                    if (scriptEl.onload) scriptEl.onload(new Event('load'));
                    scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('load'));
                    return;
                }
                // Depth guard: if sync evals are already nested beyond the safe limit,
                // degrade to async. This prevents C++ stack overflow from chains like
                // tag.js → pixel.js → tracker.js → … where each level blocks the V8
                // thread inside op_net_fetch_sync while its eval frame stays on stack.
                if (_syncEvalDepth >= _MAX_SYNC_EVAL_DEPTH) {
                    console.log(`[DOM] sync eval depth limit (${_MAX_SYNC_EVAL_DEPTH}) — falling back to async: ${fullUrl}`);
                    (async () => {
                        try {
                            const resp = await globalThis.fetch(fullUrl);
                            if (resp.ok) {
                                const code = await resp.text();
                                try { (0, eval)(code); } catch(_) {}
                                if (scriptEl.onload) scriptEl.onload(new Event('load'));
                                scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('load'));
                            }
                        } catch(_) {
                            if (scriptEl.onerror) scriptEl.onerror(new Event('error'));
                            scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('error'));
                        }
                    })();
                    return;
                }
                _syncFetchInFlight.add(baseUrl);
                _syncEvalDepth++;
                console.log(`[DOM] sync fetching script (depth ${_syncEvalDepth}): ${fullUrl}`);
                try {
                    const code = ops.op_net_fetch_sync(fullUrl, globalThis.location?.href || "");
                    if (code) {
                        console.log(`[DOM] sync executing script (${code.length} bytes): ${fullUrl}`);
                        try {
                            (0, eval)(code);
                            console.log(`[DOM] sync execution SUCCESS: ${fullUrl}`);
                        } catch(e) {
                            console.log(`[DOM] sync eval ERROR for ${fullUrl}: ${e.message}\n${e.stack}`);
                            if (scriptEl.onerror) scriptEl.onerror(new Event('error'));
                            scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('error'));
                        }
                    } else {
                        console.log(`[DOM] sync fetch FAILED (empty) for ${fullUrl}`);
                        if (scriptEl.onerror) scriptEl.onerror(new Event('error'));
                        scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('error'));
                    }
                    if (scriptEl.onload) scriptEl.onload(new Event('load'));
                    scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('load'));
                } catch(e) {
                    console.log(`[DOM] sync fetch OP error for ${fullUrl}: ${e.message}`);
                    if (scriptEl.onerror) scriptEl.onerror(new Event('error'));
                    scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('error'));
                } finally {
                    _syncFetchInFlight.delete(baseUrl);
                    _syncEvalDepth--;
                }
            } else {
                console.log(`[DOM] async fetching script: ${fullUrl}`);
                (async () => {
                    try {
                        const resp = await globalThis.fetch(fullUrl);
                        if (resp.ok) {
                            const code = await resp.text();
                            console.log(`[DOM] async executing script (${code.length} bytes): ${fullUrl}`);
                            try {
                                (0, eval)(code);
                                console.log(`[DOM] async execution SUCCESS: ${fullUrl}`);
                                if (scriptEl.onload) scriptEl.onload(new Event('load'));
                                scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('load'));
                            } catch(e) {
                                console.log(`[DOM] async eval ERROR for ${fullUrl}: ${e.message}\n${e.stack}`);
                                if (scriptEl.onerror) scriptEl.onerror(new Event('error'));
                                scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('error'));
                            }
                        } else {
                            console.log(`[DOM] async fetch FAILED (status ${resp.status}) for ${fullUrl}`);
                            if (scriptEl.onerror) scriptEl.onerror(new Event('error'));
                            scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('error'));
                        }
                    } catch(e) {
                        console.log(`[DOM] async fetch ERROR for ${fullUrl}: ${e.message}`);
                        if (scriptEl.onerror) scriptEl.onerror(new Event('error'));
                        scriptEl.dispatchEvent && scriptEl.dispatchEvent(new Event('error'));
                    }
                })();
            }
        }

        // 2. Recursive check for children (handles <div><script>...</script></div>)
        if (child.childNodes && child.childNodes.length > 0) {
            for (let i = 0; i < child.childNodes.length; i++) {
                _onNodeInserted(child.childNodes[i], sync);
            }
        }
    }

    globalThis.__onNodeInserted = _onNodeInserted;

    class NodeList {
        constructor(data, isTyped = false) {
            if (isTyped) {
                this._ids = [];
                for (let i = 0; i < data.length; i += 2) {
                    const id = data[i];
                    const type = data[i+1];
                    this._ids.push(id);
                    this[i/2] = _wrapNodeWithType(id, type);
                }
            } else {
                this._ids = data;
                for (let i = 0; i < data.length; i++) {
                    this[i] = _wrapNode(data[i]);
                }
            }
        }
        get length() { return this._ids.length; }
        item(index) { return index < this._ids.length ? _wrapNode(this._ids[index]) : null; }
        forEach(cb, thisArg) {
            for (let i = 0; i < this._ids.length; i++) {
                cb.call(thisArg, this[i], i, this);
            }
        }
        entries() {
            const self = this;
            let i = 0;
            return { next() { return i < self.length ? { value: [i, self[i++]], done: false } : { done: true }; }, [Symbol.iterator]() { return this; } };
        }
        keys() {
            const self = this;
            let i = 0;
            return { next() { return i < self.length ? { value: i++, done: false } : { done: true }; }, [Symbol.iterator]() { return this; } };
        }
        values() { return this[Symbol.iterator](); }
        [Symbol.iterator]() {
            let i = 0;
            const self = this;
            return {
                next() {
                    if (i < self.length) return { value: self[i++], done: false };
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]() { return this; }
            };
        }
    }

    class DOMTokenList {
        #nodeId;
        constructor(nodeId) { this.#nodeId = nodeId; }
        add(cls) { ops.op_dom_class_list_add(this.#nodeId, cls); }
        remove(cls) { ops.op_dom_class_list_remove(this.#nodeId, cls); }
        toggle(cls) {
            if (this.contains(cls)) { this.remove(cls); return false; }
            this.add(cls); return true;
        }
        contains(cls) {
            const attr = ops.op_dom_get_attribute(this.#nodeId, "class");
            return attr ? attr.split(/\s+/).includes(cls) : false;
        }
        get value() { return ops.op_dom_get_attribute(this.#nodeId, "class") || ""; }
        get length() { return this.value.split(/\s+/).filter(Boolean).length; }
        toString() { return this.value; }
        item(index) {
            const tokens = this.value.split(/\s+/).filter(Boolean);
            return tokens[index] != null ? tokens[index] : null;
        }
        // Real Chrome DOMTokenList is iterable; iterating yields each token
        // string. Kasada's `ao` probe spreads element.classList — without
        // Symbol.iterator we throw "non-iterable" while Chrome returns the
        // token array.
        [Symbol.iterator]() {
            const tokens = this.value.split(/\s+/).filter(Boolean);
            let i = 0;
            return {
                next() {
                    if (i < tokens.length) return { value: tokens[i++], done: false };
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]() { return this; }
            };
        }
        entries() {
            const tokens = this.value.split(/\s+/).filter(Boolean);
            let i = 0;
            return {
                next() {
                    if (i < tokens.length) { const idx = i; return { value: [idx, tokens[i++]], done: false }; }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]() { return this; }
            };
        }
        keys() {
            const n = this.length;
            let i = 0;
            return {
                next() {
                    if (i < n) return { value: i++, done: false };
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]() { return this; }
            };
        }
        values() { return this[Symbol.iterator](); }
        forEach(cb, thisArg) {
            const tokens = this.value.split(/\s+/).filter(Boolean);
            for (let i = 0; i < tokens.length; i++) {
                cb.call(thisArg, tokens[i], i, this);
            }
        }
    }

    // EventTarget is the base of the DOM prototype chain in real Chrome:
    //   EventTarget ← Node ← Element ← HTMLElement ← HTMLDivElement etc.
    // Anti-bot probes check `document instanceof EventTarget === true`
    // and walk Object.getPrototypeOf chains expecting this layout.
    const EventTarget = globalThis.EventTarget || class EventTarget {
        constructor() {}
        addEventListener(type, listener, options) {}
        removeEventListener(type, listener, options) {}
        dispatchEvent(event) { return true; }
    };
    globalThis.EventTarget = EventTarget;

    class Node extends EventTarget {
        constructor(nodeId) {
            super();
            _nodeIds.set(this, nodeId);
        }
        // nodeType constants
        static ELEMENT_NODE = 1;
        static TEXT_NODE = 3;
        static COMMENT_NODE = 8;
        static DOCUMENT_NODE = 9;
        static DOCUMENT_FRAGMENT_NODE = 11;
        static DOCUMENT_TYPE_NODE = 10;
        static PROCESSING_INSTRUCTION_NODE = 7;
        static ATTRIBUTE_NODE = 2;
        static CDATA_SECTION_NODE = 4;

        get nodeType() { return ops.op_dom_get_node_type(_getNodeId(this)); }
        get nodeName() {
            const type = this.nodeType;
            if (type === 1) return ops.op_dom_get_tag_name(_getNodeId(this)).toUpperCase();
            if (type === 3) return "#text";
            if (type === 8) return "#comment";
            if (type === 9) return "#document";
            if (type === 11) return "#document-fragment";
            return "";
        }
        get nodeValue() {
            const type = this.nodeType;
            if (type === 3 || type === 8) return ops.op_dom_get_text_content(_getNodeId(this));
            return null;
        }
        set nodeValue(val) {
            const type = this.nodeType;
            if (type === 3 || type === 8) ops.op_dom_set_text_content(_getNodeId(this), String(val));
        }
        get ownerDocument() {
            return this.nodeType === 9 ? null : _document;
        }
        get isConnected() {
            let n = this;
            while (n) {
                if (n.nodeType === 9) return true;
                n = n.parentNode;
            }
            return false;
        }
        get baseURI() {
            return globalThis.location?.href || "about:blank";
        }
        get parentNode() { return _wrapNode(ops.op_dom_get_parent(_getNodeId(this))); }
        get parentElement() {
            const p = this.parentNode;
            return p && p.nodeType === 1 ? p : null;
        }
        get childNodes() { return new NodeList(ops.op_dom_get_children_with_types(_getNodeId(this)), true); }
        get firstChild() { return _wrapNode(ops.op_dom_get_first_child(_getNodeId(this))); }
        get lastChild() { return _wrapNode(ops.op_dom_get_last_child(_getNodeId(this))); }
        get nextSibling() { return _wrapNode(ops.op_dom_get_next_sibling(_getNodeId(this))); }
        get previousSibling() { return _wrapNode(ops.op_dom_get_prev_sibling(_getNodeId(this))); }
        get textContent() { return ops.op_dom_get_text_content(_getNodeId(this)); }
        set textContent(val) { ops.op_dom_set_text_content(_getNodeId(this), String(val)); }
        appendChild(child) {
            ops.op_dom_append_child(_getNodeId(this), _getNodeId(child));
            _onNodeInserted(child);
            return child;
        }
        removeChild(child) {
            _ceDisconnected(child);
            ops.op_dom_remove_child(_getNodeId(this), _getNodeId(child));
            return child;
        }
        replaceChild(newChild, oldChild) {
            const parent = _getNodeId(this);
            const oldId = _getNodeId(oldChild);
            const newId = _getNodeId(newChild);
            _ceDisconnected(oldChild);
            ops.op_dom_insert_before(parent, newId, oldId);
            ops.op_dom_remove_child(parent, oldId);
            _onNodeInserted(newChild);
            return oldChild;
        }
        insertBefore(newChild, refChild) {
            if (refChild === null || refChild === undefined) return this.appendChild(newChild);
            ops.op_dom_insert_before(_getNodeId(this), _getNodeId(newChild), _getNodeId(refChild));
            _onNodeInserted(newChild);
            return newChild;
        }
        cloneNode(deep = false) {
            const newId = ops.op_dom_clone_node(_getNodeId(this), !!deep);
            return _wrapNode(newId);
        }
        contains(other) {
            if (!other) return false;
            if (other === this) return true;
            let p = other.parentNode;
            while (p) {
                if (p === this) return true;
                p = p.parentNode;
            }
            return false;
        }
        hasChildNodes() { return ops.op_dom_get_children(_getNodeId(this)).length > 0; }
        getRootNode() {
            let n = this;
            while (n.parentNode) n = n.parentNode;
            return n;
        }
        normalize() {
            // Merge adjacent text nodes
            const children = ops.op_dom_get_children(_getNodeId(this));
            let prevTextId = null;
            for (const cid of children) {
                if (ops.op_dom_get_node_type(cid) === 3) {
                    if (prevTextId !== null) {
                        const prevText = ops.op_dom_get_text_content(prevTextId);
                        const curText = ops.op_dom_get_text_content(cid);
                        ops.op_dom_set_text_content(prevTextId, prevText + curText);
                        ops.op_dom_remove_child(_getNodeId(this), cid);
                    } else {
                        prevTextId = cid;
                    }
                } else {
                    prevTextId = null;
                }
            }
        }
        isEqualNode(other) {
            if (!other) return false;
            if (this === other) return true;
            if (this.nodeType !== other.nodeType) return false;
            if (this.nodeType === 1) return this.outerHTML === other.outerHTML;
            return this.textContent === other.textContent;
        }
        isSameNode(other) { return this === other; }
        compareDocumentPosition(other) {
            if (this === other) return 0;
            if (this.contains(other)) return 20; // DOCUMENT_POSITION_CONTAINED_BY | FOLLOWING
            if (other.contains(this)) return 10; // DOCUMENT_POSITION_CONTAINS | PRECEDING
            return 4; // DOCUMENT_POSITION_FOLLOWING
        }
    }

    // --- Internal Bridge ---
    if (!globalThis.__boxide) {
        Object.defineProperty(globalThis, '__boxide', { value: {}, enumerable: false, configurable: true });
    }
    globalThis.__boxide._getNodeId = _getNodeId;
    globalThis.__boxide._wrapNode = _wrapNode;
    globalThis.__boxide._setCurrentScript = _setCurrentScript;

    function _createStyleProxy(nodeId) {
        const cache = {};
        const raw = ops.op_dom_get_attribute(nodeId, "style") || "";
        for (const part of raw.split(";")) {
            const idx = part.indexOf(":");
            if (idx > 0) cache[part.slice(0, idx).trim()] = part.slice(idx + 1).trim();
        }
        function flush() {
            const parts = [];
            for (const k in cache) { if (cache[k] !== "") parts.push(k + ": " + cache[k]); }
            ops.op_dom_set_attribute(nodeId, "style", parts.join("; "));
        }
        const toKebab = (p) => p.replace(/[A-Z]/g, m => "-" + m.toLowerCase());
        const style = Object.create(globalThis.CSSStyleDeclaration.prototype || Object.prototype);
        return new Proxy(style, {
            get(target, prop) {
                if (prop === "setProperty") return (name, value) => { cache[name] = String(value); flush(); };
                if (prop === "getPropertyValue") return (name) => cache[name] || "";
                if (prop === "removeProperty") return (name) => { const old = cache[name] || ""; delete cache[name]; flush(); return old; };
                if (prop === "cssText") return ops.op_dom_get_attribute(nodeId, "style") || "";
                if (prop === "length") return Object.keys(cache).length;
                if (prop === Symbol.toStringTag) return "CSSStyleDeclaration";
                if (typeof prop === "string") {
                    if (/^\d+$/.test(prop)) return Object.keys(cache)[parseInt(prop, 10)];
                    return cache[toKebab(prop)] || "";
                }
                return undefined;
            },
            set(target, prop, value) {
                if (prop === "cssText") {
                    for (const k in cache) delete cache[k];
                    for (const part of String(value).split(";")) {
                        const idx = part.indexOf(":");
                        if (idx > 0) cache[part.slice(0, idx).trim()] = part.slice(idx + 1).trim();
                    }
                    flush();
                    return true;
                }
                cache[toKebab(prop)] = String(value);
                flush();
                return true;
            },
            // V8 Proxy invariant: has/ownKeys/getOwnPropertyDescriptor must
            // agree. Without explicit traps V8 reconciles against the empty
            // target object on every `prop in style` / Object.keys(style)
            // call — hot work that creepjs hits per WebIDL property under test.
            has(target, prop) {
                if (prop === "setProperty" || prop === "getPropertyValue" ||
                    prop === "removeProperty" || prop === "cssText") return true;
                if (typeof prop === "string") return Object.prototype.hasOwnProperty.call(cache, toKebab(prop));
                return false;
            },
            ownKeys() {
                return Object.keys(cache);
            },
            getOwnPropertyDescriptor(target, prop) {
                if (typeof prop !== "string") return undefined;
                const key = toKebab(prop);
                if (Object.prototype.hasOwnProperty.call(cache, key)) {
                    return { value: cache[key], enumerable: true, configurable: true, writable: true };
                }
                return undefined;
            }
        });
    }

    class Element extends Node {
        get tagName() { return ops.op_dom_get_tag_name(_getNodeId(this)).toUpperCase(); }
        get localName() { return ops.op_dom_get_tag_name(_getNodeId(this)); }
        get id() { return ops.op_dom_get_attribute(_getNodeId(this), "id") || ""; }
        set id(val) { ops.op_dom_set_attribute(_getNodeId(this), "id", String(val)); }
        get className() { return ops.op_dom_get_attribute(_getNodeId(this), "class") || ""; }
        set className(val) { ops.op_dom_set_attribute(_getNodeId(this), "class", String(val)); }
        // HTML attribute-backed properties (script.src, link.href, img.src, etc.)
        get src() { return this.getAttribute("src") || ""; }
        set src(val) { this.setAttribute("src", String(val)); }
        get href() { return this.getAttribute("href") || ""; }
        set href(val) { this.setAttribute("href", String(val)); }
        get type() { return this.getAttribute("type") || ""; }
        set type(val) { this.setAttribute("type", String(val)); }
        get rel() { return this.getAttribute("rel") || ""; }
        set rel(val) { this.setAttribute("rel", String(val)); }
        get async() { return this.hasAttribute("async"); }
        set async(val) { if (val) this.setAttribute("async", ""); else this.removeAttribute("async"); }
        get defer() { return this.hasAttribute("defer"); }
        set defer(val) { if (val) this.setAttribute("defer", ""); else this.removeAttribute("defer"); }
        get crossOrigin() { return this.getAttribute("crossorigin"); }
        set crossOrigin(val) { if (val != null) this.setAttribute("crossorigin", String(val)); else this.removeAttribute("crossorigin"); }
        get integrity() { return this.getAttribute("integrity") || ""; }
        set integrity(val) { this.setAttribute("integrity", String(val)); }
        get referrerPolicy() { return this.getAttribute("referrerpolicy") || ""; }
        set referrerPolicy(val) { this.setAttribute("referrerpolicy", String(val)); }
        get classList() { return new DOMTokenList(_getNodeId(this)); }
        get innerHTML() { return ops.op_dom_get_inner_html(_getNodeId(this)); }
        set innerHTML(val) { ops.op_dom_set_inner_html(_getNodeId(this), String(val)); }
        get outerHTML() { return ops.op_dom_get_outer_html(_getNodeId(this)); }
        get children() {
            return new NodeList(ops.op_dom_get_child_elements_with_types(_getNodeId(this)), true);
        }
        get firstElementChild() {
            const els = ops.op_dom_get_child_elements(_getNodeId(this));
            return els.length > 0 ? _wrapNode(els[0]) : null;
        }
        get lastElementChild() {
            const els = ops.op_dom_get_child_elements(_getNodeId(this));
            return els.length > 0 ? _wrapNode(els[els.length - 1]) : null;
        }
        getAttribute(name) { return ops.op_dom_get_attribute(_getNodeId(this), name); }
        setAttribute(name, value) { ops.op_dom_set_attribute(_getNodeId(this), name, String(value)); }
        removeAttribute(name) { ops.op_dom_remove_attribute(_getNodeId(this), name); }
        hasAttribute(name) { return ops.op_dom_has_attribute(_getNodeId(this), name); }
        querySelector(sel) {
            const id = ops.op_dom_query_selector(_getNodeId(this), sel);
            return id !== null ? _wrapNode(id) : null;
        }
        querySelectorAll(sel) {
            return new NodeList(ops.op_dom_query_selector_all(_getNodeId(this), sel));
        }
        matches(sel) {
            const all = ops.op_dom_query_selector_all(
                ops.op_dom_get_parent(_getNodeId(this)) || ops.op_dom_document_node(),
                sel
            );
            return all.includes(_getNodeId(this));
        }
        closest(sel) {
            let el = this;
            while (el) {
                if (el.matches && el.matches(sel)) return el;
                el = el.parentElement;
            }
            return null;
        }
        getElementsByTagName(tag) {
            return new NodeList(ops.op_dom_get_elements_by_tag_name(_getNodeId(this), tag));
        }
        getElementsByClassName(cls) {
            return new NodeList(ops.op_dom_get_elements_by_class_name(_getNodeId(this), cls));
        }
        // Layout APIs (wired to taffy via layout_ext ops)
        getBoundingClientRect() {
            const r = ops.op_layout_get_bounding_rect(_getNodeId(this));
            return new DOMRect(r.x, r.y, r.width, r.height);
        }
        getClientRects() { return [this.getBoundingClientRect()]; }
        get offsetWidth() { return ops.op_layout_get_offset_width(_getNodeId(this)); }
        get offsetHeight() { return ops.op_layout_get_offset_height(_getNodeId(this)); }
        get offsetTop() { return ops.op_layout_get_offset_top(_getNodeId(this)); }
        get offsetLeft() { return ops.op_layout_get_offset_left(_getNodeId(this)); }
        get clientWidth() { return this.offsetWidth; }
        get clientHeight() { return this.offsetHeight; }
        get scrollWidth() { return this.offsetWidth; }
        get scrollHeight() { return this.offsetHeight; }
        get scrollTop() {
            const s = _scrollState.get(_getNodeId(this));
            return s ? s.top : 0;
        }
        set scrollTop(v) {
            const id = _getNodeId(this);
            const n = Number(v);
            const top = Number.isFinite(n) ? n : 0;
            const cur = _scrollState.get(id);
            if (cur) cur.top = top; else _scrollState.set(id, { top, left: 0 });
        }
        get scrollLeft() {
            const s = _scrollState.get(_getNodeId(this));
            return s ? s.left : 0;
        }
        set scrollLeft(v) {
            const id = _getNodeId(this);
            const n = Number(v);
            const left = Number.isFinite(n) ? n : 0;
            const cur = _scrollState.get(id);
            if (cur) cur.left = left; else _scrollState.set(id, { top: 0, left });
        }
        scrollIntoView(_arg) { /* spec no-op when no scrollable ancestor; safe stub */ }
        scrollTo(xOrOpts, y) {
            if (typeof xOrOpts === "object" && xOrOpts !== null) {
                if (xOrOpts.left !== undefined) this.scrollLeft = xOrOpts.left;
                if (xOrOpts.top !== undefined) this.scrollTop = xOrOpts.top;
            } else {
                this.scrollLeft = xOrOpts;
                this.scrollTop = y;
            }
        }
        scrollBy(xOrOpts, y) {
            if (typeof xOrOpts === "object" && xOrOpts !== null) {
                if (xOrOpts.left !== undefined) this.scrollLeft = this.scrollLeft + xOrOpts.left;
                if (xOrOpts.top !== undefined) this.scrollTop = this.scrollTop + xOrOpts.top;
            } else {
                this.scrollLeft = this.scrollLeft + xOrOpts;
                this.scrollTop = this.scrollTop + y;
            }
        }
        get offsetParent() { return this.parentElement; }
        // --- Modern DOM manipulation ---
        remove() {
            const parent = ops.op_dom_get_parent(_getNodeId(this));
            if (parent !== -1 && parent !== null) {
                ops.op_dom_remove_child(parent, _getNodeId(this));
            }
        }
        append(...nodes) {
            for (const node of nodes) {
                if (typeof node === "string") {
                    this.appendChild(_document.createTextNode(node));
                } else {
                    this.appendChild(node);
                }
            }
        }
        prepend(...nodes) {
            const first = this.firstChild;
            for (const node of nodes) {
                const n = typeof node === "string" ? _document.createTextNode(node) : node;
                if (first) {
                    this.insertBefore(n, first);
                } else {
                    this.appendChild(n);
                }
            }
        }
        after(...nodes) {
            const parent = this.parentNode;
            if (!parent) return;
            const next = this.nextSibling;
            for (const node of nodes) {
                const n = typeof node === "string" ? _document.createTextNode(node) : node;
                if (next) {
                    parent.insertBefore(n, next);
                } else {
                    parent.appendChild(n);
                }
            }
        }
        before(...nodes) {
            const parent = this.parentNode;
            if (!parent) return;
            for (const node of nodes) {
                const n = typeof node === "string" ? _document.createTextNode(node) : node;
                parent.insertBefore(n, this);
            }
        }
        replaceWith(...nodes) {
            const parent = this.parentNode;
            if (!parent) return;
            const next = this.nextSibling;
            this.remove();
            for (const node of nodes) {
                const n = typeof node === "string" ? _document.createTextNode(node) : node;
                if (next) {
                    parent.insertBefore(n, next);
                } else {
                    parent.appendChild(n);
                }
            }
        }
        replaceChildren(...nodes) {
            // Remove all existing children
            while (this.firstChild) this.removeChild(this.firstChild);
            this.append(...nodes);
        }
        // --- insertAdjacent family ---
        insertAdjacentHTML(position, html) {
            ops.op_dom_insert_adjacent_html(_getNodeId(this), position, html);
        }
        insertAdjacentElement(position, element) {
            const parent = this.parentNode;
            switch (position) {
                case "beforebegin":
                    if (parent) parent.insertBefore(element, this);
                    break;
                case "afterbegin":
                    this.insertBefore(element, this.firstChild);
                    break;
                case "beforeend":
                    this.appendChild(element);
                    break;
                case "afterend":
                    if (parent) {
                        const next = this.nextSibling;
                        if (next) parent.insertBefore(element, next);
                        else parent.appendChild(element);
                    }
                    break;
            }
            return element;
        }
        insertAdjacentText(position, text) {
            const textNode = _document.createTextNode(text);
            this.insertAdjacentElement(position, textNode);
        }
        toggleAttribute(name, force) {
            if (force !== undefined) {
                if (force) { this.setAttribute(name, ""); return true; }
                else { this.removeAttribute(name); return false; }
            }
            if (this.hasAttribute(name)) { this.removeAttribute(name); return false; }
            this.setAttribute(name, ""); return true;
        }
        // --- Attribute helpers ---
        get attributes() {
            // NamedNodeMap-like object. Uses op_dom_get_attribute_names to
            // enumerate real attributes; previous shim hardcoded length: 0
            // which violates the V8 Proxy invariant ownKeys ⇔ has and made
            // creepjs's per-element attribute audit do redundant work.
            const el = this;
            const id = _getNodeId(this);
            const namesOf = () => ops.op_dom_get_attribute_names(id);
            const itemFor = (name) => {
                const val = ops.op_dom_get_attribute(id, name);
                return val ? { name, value: val, specified: true } : null;
            };
            return new Proxy([], {
                get(target, prop) {
                    if (prop === "length") return namesOf().length;
                    if (prop === "getNamedItem") return (name) => itemFor(String(name));
                    if (prop === "item") return (i) => {
                        const n = namesOf()[i];
                        return n ? itemFor(n) : null;
                    };
                    if (prop === Symbol.iterator) return function* () {
                        for (const n of namesOf()) yield itemFor(n);
                    };
                    if (typeof prop === "string" && /^\d+$/.test(prop)) {
                        const n = namesOf()[parseInt(prop, 10)];
                        return n ? itemFor(n) : undefined;
                    }
                    if (typeof prop === "string") return itemFor(prop);
                    return undefined;
                },
                has(target, prop) {
                    if (prop === "length" || prop === "getNamedItem" || prop === "item") return true;
                    if (typeof prop === "string" && /^\d+$/.test(prop)) {
                        return parseInt(prop, 10) < namesOf().length;
                    }
                    if (typeof prop === "string") return ops.op_dom_has_attribute(id, prop);
                    return false;
                },
                ownKeys() {
                    const names = namesOf();
                    const keys = [];
                    for (let i = 0; i < names.length; i++) keys.push(String(i));
                    return keys.concat(["length"]);
                },
                getOwnPropertyDescriptor(target, prop) {
                    if (prop === "length") {
                        return { value: namesOf().length, enumerable: false, configurable: false, writable: false };
                    }
                    if (typeof prop === "string" && /^\d+$/.test(prop)) {
                        const n = namesOf()[parseInt(prop, 10)];
                        if (n) return { value: itemFor(n), enumerable: true, configurable: true, writable: false };
                    }
                    return undefined;
                }
            });
        }
        get dataset() {
            const el = this;
            const id = _getNodeId(this);
            const toKebab = (p) => "data-" + p.replace(/[A-Z]/g, m => "-" + m.toLowerCase());
            const fromKebab = (a) => a.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
            const dataNames = () => ops.op_dom_get_attribute_names(id).filter(n => n.startsWith("data-"));
            return new Proxy({}, {
                get(target, prop) {
                    if (typeof prop !== "string") return undefined;
                    return ops.op_dom_get_attribute(id, toKebab(prop)) || undefined;
                },
                set(target, prop, value) {
                    el.setAttribute(toKebab(prop), String(value));
                    return true;
                },
                has(target, prop) {
                    if (typeof prop !== "string") return false;
                    return ops.op_dom_has_attribute(id, toKebab(prop));
                },
                deleteProperty(target, prop) {
                    if (typeof prop === "string") el.removeAttribute(toKebab(prop));
                    return true;
                },
                ownKeys() {
                    return dataNames().map(fromKebab);
                },
                getOwnPropertyDescriptor(target, prop) {
                    if (typeof prop !== "string") return undefined;
                    const attr = toKebab(prop);
                    if (ops.op_dom_has_attribute(id, attr)) {
                        return {
                            value: ops.op_dom_get_attribute(id, attr) || "",
                            enumerable: true, configurable: true, writable: true,
                        };
                    }
                    return undefined;
                }
            });
        }
        get nextElementSibling() {
            let n = this.nextSibling;
            while (n) {
                if (n.nodeType === 1) return n;
                n = n.nextSibling;
            }
            return null;
        }
        get previousElementSibling() {
            let n = this.previousSibling;
            while (n) {
                if (n.nodeType === 1) return n;
                n = n.previousSibling;
            }
            return null;
        }
        get childElementCount() {
            return ops.op_dom_get_child_elements(_getNodeId(this)).length;
        }
        // element.style — CSSStyleDeclaration proxy
        get style() {
            if (!this._style) this._style = _createStyleProxy(_getNodeId(this));
            return this._style;
        }
        // Interaction stubs
        click() { this.dispatchEvent(new Event("click", { bubbles: true })); }
        focus() { this.dispatchEvent(new Event("focus")); }
        blur() { this.dispatchEvent(new Event("blur")); }
        checkVisibility() { return true; }
        animate() { return { finished: Promise.resolve(), cancel() {}, play() {}, pause() {} }; }
        getAnimations() { return []; }
        attachShadow(init = {}) {
            const mode = init.mode || "open";
            const shadowId = ops.op_dom_attach_shadow(_getNodeId(this), mode);
            // Use _wrapNode — _wrap is not a defined helper. Was a stale
            // reference that threw `ReferenceError: _wrap is not defined`
            // whenever attachShadow was actually called. Caught by
            // Kasada's `sdt.c` field (decrypted blob 0, 2026-05-10).
            const shadowRoot = _wrapNode(shadowId);
            // ShadowRoot inherits Node methods (appendChild, querySelector, etc.)
            Object.defineProperties(shadowRoot, {
                mode: { value: mode, enumerable: true },
                host: { value: this, enumerable: true },
                innerHTML: {
                    get() { return ops.op_dom_get_inner_html(shadowId); },
                    set(html) { ops.op_dom_set_inner_html(shadowId, html); },
                },
            });
            if (mode === "open") this._shadowRoot = shadowRoot;
            return shadowRoot;
        }
        get shadowRoot() { return this._shadowRoot || null; }
    }

    // Full DOM prototype chain:
    //   EventTarget ← Node ← Element ← HTMLElement ← HTML*Element
    // Subclasses are mostly empty markers for instanceof checks. When an
    // element is created via _wrapNode, we do setPrototypeOf based on the
    // tag name to select the right specific class (HTMLDivElement etc.)
    // without having to create a dedicated Rust-side dispatch.
    class HTMLElement extends Element {}
    class HTMLDivElement extends HTMLElement {}
    class HTMLSpanElement extends HTMLElement {}
    class HTMLParagraphElement extends HTMLElement {}
    class HTMLHeadingElement extends HTMLElement {}
    class HTMLAnchorElement extends HTMLElement {}
    class HTMLImageElement extends HTMLElement {}
    Object.defineProperty(HTMLImageElement.prototype, "width", {
        get() {
            const attr = this.getAttribute("width");
            return attr ? parseInt(attr, 10) : 0;
        },
        enumerable: true, configurable: true
    });
    Object.defineProperty(HTMLImageElement.prototype, "height", {
        get() {
            const attr = this.getAttribute("height");
            return attr ? parseInt(attr, 10) : 0;
        },
        enumerable: true, configurable: true
    });
    Object.defineProperty(HTMLImageElement.prototype, "naturalWidth", {
        get() { return this.width; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(HTMLImageElement.prototype, "naturalHeight", {
        get() { return this.height; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(HTMLImageElement.prototype, "complete", {
        get() { return true; }, 
        enumerable: true, configurable: true
    });
    HTMLImageElement.prototype.decode = function() { return Promise.resolve(); };
    class HTMLInputElement extends HTMLElement {}
    class HTMLFormElement extends HTMLElement {
        submit() {
            const action = this.action || (globalThis.location ? globalThis.location.href : '');
            const method = (this.method || 'GET').toUpperCase();

            // Serialize form data
            const params = new URLSearchParams();
            const inputs = this.querySelectorAll('input, textarea, select');
            for (let i = 0; i < inputs.length; i++) {
                const el = inputs[i];
                const name = el.name;
                if (!name || el.disabled) continue;

                const type = (el.type || '').toLowerCase();
                if (type === 'submit' || type === 'button' || type === 'image') continue;
                if ((type === 'checkbox' || type === 'radio') && !el.checked) continue;

                params.append(name, el.value || '');
            }

            let finalUrl = action;
            let finalBody = null;

            if (method === 'GET') {
                const url = new URL(action, globalThis.location ? globalThis.location.href : 'about:blank');
                params.forEach((v, k) => url.searchParams.append(k, v));
                finalUrl = url.href;
            } else {
                finalBody = params.toString();
            }

            globalThis.__pendingNavigation = {
                url: finalUrl,
                method: method,
                body: finalBody,
                kind: 'assign'
            };
            // Signal the Rust event loop to short-circuit run_until_idle —
            // see crates/js_runtime/src/extensions/nav_ext.rs.
            try { ops.op_set_pending_nav(); } catch (_) {}
        }
        requestSubmit(submitter) {
            this.submit();
        }
    }

    // IDL property ↔ HTML attribute reflection. Scripts that configure form
    // fields via properties (el.name = 'x', form.action = url, form.method =
    // 'POST') expect the read-back to see what they set — which only works if
    // the property setter writes the underlying attribute. Without this,
    // programmatically-built forms look empty to our submit() serializer.
    // Universal primitive — matches HTML spec "reflect" behavior.
    const _reflectStr = (proto, prop, attr = prop, dflt = '') => {
        Object.defineProperty(proto, prop, {
            get() { const v = this.getAttribute(attr); return v == null ? dflt : v; },
            set(v) { this.setAttribute(attr, String(v)); },
            enumerable: true, configurable: true,
        });
    };
    const _reflectBool = (proto, prop, attr = prop) => {
        Object.defineProperty(proto, prop, {
            get() { return this.hasAttribute(attr); },
            set(v) {
                if (v) this.setAttribute(attr, '');
                else this.removeAttribute(attr);
            },
            enumerable: true, configurable: true,
        });
    };
    _reflectStr(HTMLInputElement.prototype, 'name');
    _reflectStr(HTMLInputElement.prototype, 'value');
    _reflectStr(HTMLInputElement.prototype, 'type', 'type', 'text');
    _reflectStr(HTMLInputElement.prototype, 'placeholder');
    _reflectBool(HTMLInputElement.prototype, 'checked');
    _reflectBool(HTMLInputElement.prototype, 'disabled');
    _reflectBool(HTMLInputElement.prototype, 'readOnly', 'readonly');
    _reflectBool(HTMLInputElement.prototype, 'required');
    _reflectStr(HTMLFormElement.prototype, 'action');
    _reflectStr(HTMLFormElement.prototype, 'method', 'method', 'get');
    _reflectStr(HTMLFormElement.prototype, 'enctype', 'enctype', 'application/x-www-form-urlencoded');
    _reflectStr(HTMLFormElement.prototype, 'target');
    _reflectStr(HTMLFormElement.prototype, 'name');
    _reflectBool(HTMLFormElement.prototype, 'noValidate', 'novalidate');

    class HTMLButtonElement extends HTMLElement {}
    class HTMLSelectElement extends HTMLElement {}
    class HTMLTextAreaElement extends HTMLElement {}
    class HTMLCanvasElement extends HTMLElement {}
    Object.defineProperty(HTMLCanvasElement.prototype, "width", {
        get() {
            const attr = this.getAttribute("width");
            return attr ? parseInt(attr, 10) : 300;
        },
        set(v) { this.setAttribute("width", v); },
        enumerable: true, configurable: true
    });
    Object.defineProperty(HTMLCanvasElement.prototype, "height", {
        get() {
            const attr = this.getAttribute("height");
            return attr ? parseInt(attr, 10) : 150;
        },
        set(v) { this.setAttribute("height", v); },
        enumerable: true, configurable: true
    });
    HTMLCanvasElement.prototype.toDataURL = function(type, quality) {
        if (!this._canvasId) {
            let osName = "Linux", canvasSeed = 0n;
            try {
                if (ops.op_has_stealth_profile && ops.op_has_stealth_profile()) {
                    osName = ops.op_get_profile_value("os_name") || "Linux";
                    canvasSeed = BigInt(ops.op_get_profile_value("canvas_seed") || "0");
                }
            } catch (_e) { /* fall back to defaults */ }
            this._canvasId = ops.op_canvas_create(this.width, this.height, osName, canvasSeed);
        }
        return ops.op_canvas_to_data_url(this._canvasId);
    };
    class HTMLScriptElement extends HTMLElement {}
    class HTMLStyleElement extends HTMLElement {}
    class HTMLLinkElement extends HTMLElement {}
    class HTMLMetaElement extends HTMLElement {}
    class HTMLTableElement extends HTMLElement {}
    class HTMLIFrameElement extends HTMLElement {}
    class HTMLVideoElement extends HTMLElement {}
    class HTMLAudioElement extends HTMLElement {}
    class HTMLBodyElement extends HTMLElement {}
    class HTMLHeadElement extends HTMLElement {}
    class HTMLHtmlElement extends HTMLElement {}
    class HTMLUListElement extends HTMLElement {}
    class HTMLOListElement extends HTMLElement {}
    class HTMLLIElement extends HTMLElement {}
    class HTMLTableRowElement extends HTMLElement {}
    class HTMLTableCellElement extends HTMLElement {}
    class HTMLTableSectionElement extends HTMLElement {}
    class HTMLLabelElement extends HTMLElement {}
    class HTMLOptionElement extends HTMLElement {}
    class HTMLTemplateElement extends HTMLElement {}
    class HTMLPreElement extends HTMLElement {}
    class HTMLQuoteElement extends HTMLElement {}

    // Tag → specific HTML*Element prototype map. Anything not listed falls
    // back to HTMLElement.prototype.
    const _tagToProto = {
        div: HTMLDivElement.prototype,
        span: HTMLSpanElement.prototype,
        p: HTMLParagraphElement.prototype,
        h1: HTMLHeadingElement.prototype,
        h2: HTMLHeadingElement.prototype,
        h3: HTMLHeadingElement.prototype,
        h4: HTMLHeadingElement.prototype,
        h5: HTMLHeadingElement.prototype,
        h6: HTMLHeadingElement.prototype,
        a: HTMLAnchorElement.prototype,
        img: HTMLImageElement.prototype,
        input: HTMLInputElement.prototype,
        form: HTMLFormElement.prototype,
        button: HTMLButtonElement.prototype,
        select: HTMLSelectElement.prototype,
        textarea: HTMLTextAreaElement.prototype,
        canvas: HTMLCanvasElement.prototype,
        script: HTMLScriptElement.prototype,
        style: HTMLStyleElement.prototype,
        link: HTMLLinkElement.prototype,
        meta: HTMLMetaElement.prototype,
        table: HTMLTableElement.prototype,
        iframe: HTMLIFrameElement.prototype,
        video: HTMLVideoElement.prototype,
        audio: HTMLAudioElement.prototype,
        body: HTMLBodyElement.prototype,
        head: HTMLHeadElement.prototype,
        html: HTMLHtmlElement.prototype,
        ul: HTMLUListElement.prototype,
        ol: HTMLOListElement.prototype,
        li: HTMLLIElement.prototype,
        tr: HTMLTableRowElement.prototype,
        td: HTMLTableCellElement.prototype,
        th: HTMLTableCellElement.prototype,
        thead: HTMLTableSectionElement.prototype,
        tbody: HTMLTableSectionElement.prototype,
        tfoot: HTMLTableSectionElement.prototype,
        label: HTMLLabelElement.prototype,
        option: HTMLOptionElement.prototype,
        template: HTMLTemplateElement.prototype,
        pre: HTMLPreElement.prototype,
        blockquote: HTMLQuoteElement.prototype,
        q: HTMLQuoteElement.prototype,
    };

    // Adjust an Element instance's prototype to the tag-specific subclass
    // so `el instanceof HTMLDivElement` works as in real Chrome.
    function _retargetElementProto(el) {
        try {
            const tag = ops.op_dom_get_tag_name(_getNodeId(el)).toLowerCase();
            const proto = _tagToProto[tag] || HTMLElement.prototype;
            Object.setPrototypeOf(el, proto);
        } catch {}
    }

    class Text extends Node {
        get data() { return ops.op_dom_get_text_content(_getNodeId(this)); }
        set data(val) { ops.op_dom_set_text_content(_getNodeId(this), String(val)); }
        get length() { return this.data.length; }
        get wholeText() { return this.data; }
    }

    class Comment extends Node {
        get data() { return ops.op_dom_get_text_content(_getNodeId(this)); }
        set data(val) { ops.op_dom_set_text_content(_getNodeId(this), String(val)); }
    }

    class DocumentFragment extends Node {}

    let _currentScript = null;
    function _setCurrentScript(el) { _currentScript = el; }

    class HTMLAllCollection {
        constructor(doc) {
            this._doc = doc;
        }
        get length() { return this._doc.querySelectorAll("*").length; }
        item(i) { return this._doc.querySelectorAll("*")[i] || null; }
        namedItem(n) {
            return this._doc.getElementById(n) || 
                   this._doc.querySelector(`[name="${CSS.escape(n)}"]`) || 
                   null;
        }
        [Symbol.iterator]() {
            const nodes = this._doc.querySelectorAll("*");
            let i = 0;
            return {
                next() {
                    return i < nodes.length ? { value: nodes[i++], done: false } : { value: undefined, done: true };
                },
                [Symbol.iterator]() { return this; }
            };
        }
    }

    class Document extends Node {
        constructor(nodeId) {
            // Forward the document node id to Node so _getNodeId returns
            // the real Rust-side Document. Without this, document.nodeType
            // resolved to 0 (the "no such node" sentinel), which broke
            // anything walking parentNode→isConnected. Phase 7 follow-up.
            super(nodeId);
            if (!globalThis.__boxide) {
                Object.defineProperty(globalThis, '__boxide', { value: {}, enumerable: false, configurable: true });
            }
            // Capture initial base URL from ops or a global hint
            globalThis.__boxide._baseUrl = ops.op_dom_get_base_url && ops.op_dom_get_base_url();

            const all = new HTMLAllCollection(this);
            // Hide 'all' from enumeration but keep it truthy
            Object.defineProperty(this, 'all', {
                get() { return all; },
                enumerable: false,
                configurable: true
            });
        }
        get scripts() { return this.getElementsByTagName("script"); }
        get currentScript() { return _currentScript; }
        get visibilityState() { return "visible"; }
        get hidden() { return false; }
        get webkitVisibilityState() { return "visible"; }
        get webkitHidden() { return false; }
        get fullscreenEnabled() { return true; }
        get webkitFullscreenEnabled() { return true; }
        get webkitIsFullScreen() { return false; }

        get documentElement() {
            const els = ops.op_dom_get_child_elements(ops.op_dom_document_node());
            return els.length > 0 ? _wrapNode(els[0]) : null;
        }
        get head() { return this.querySelector("head"); }
        get body() { return this.querySelector("body"); }
        get title() {
            const el = this.querySelector("title");
            return el ? el.textContent : "";
        }
        set title(val) {
            let el = this.querySelector("title");
            if (el) { el.textContent = val; }
        }
        getElementById(id) {
            const nodeId = ops.op_dom_get_element_by_id(id);
            return nodeId !== null ? _wrapNode(nodeId) : null;
        }
        getElementsByTagName(tag) {
            return new NodeList(ops.op_dom_get_elements_by_tag_name(ops.op_dom_document_node(), tag));
        }
        getElementsByClassName(cls) {
            return new NodeList(ops.op_dom_get_elements_by_class_name(ops.op_dom_document_node(), cls));
        }
        querySelector(sel) {
            const id = ops.op_dom_query_selector(ops.op_dom_document_node(), sel);
            return id !== null ? _wrapNode(id) : null;
        }
        querySelectorAll(sel) {
            return new NodeList(ops.op_dom_query_selector_all(ops.op_dom_document_node(), sel));
        }
        createElement(tag) {
            const el = _wrapNode(ops.op_dom_create_element(tag));
            if (tag.toLowerCase() === "script") {
                let _src = "";
                // Capture the real descriptor to avoid infinite recursion
                const proto = Object.getPrototypeOf(el);
                const origSrc = Object.getOwnPropertyDescriptor(proto, 'src');

                Object.defineProperty(el, "src", {
                    get: () => _src,
                    set: (v) => {
                        _src = v;
                        if (v.includes("akam") || v.includes("ips.js") || v.includes("kpsdk")) {
                            console.log(`[DOM] dynamic script: ${v}`);
                        }
                        if (origSrc && origSrc.set) {
                            origSrc.set.call(el, v);
                        } else {
                            el.setAttribute("src", v);
                        }
                    },
                    configurable: true,
                });
            }
            return el;
        }
        createElementNS(ns, tag) {
            // For now, treat namespaced elements same as regular ones.
            return this.createElement(tag);
        }
        createTextNode(text) {
            return _wrapNode(ops.op_dom_create_text_node(text));
        }
        createDocumentFragment() {
            return _wrapNode(ops.op_dom_create_document_fragment());
        }
        createComment(text) {
            // Comment nodes have nodeType 8 in the DOM; use text node with special handling
            const id = ops.op_dom_create_text_node(""); // TODO: proper comment op
            return _wrapNode(id);
        }
        createEvent(type) {
            // Legacy event factory
            return new Event(type);
        }
        createRange() {
            return new Range();
        }
        createTreeWalker(root, whatToShow, filter) {
            return { currentNode: root, nextNode() { return null; }, previousNode() { return null; } };
        }
        createNodeIterator(root, whatToShow, filter) {
            return { nextNode() { return null; }, previousNode() { return null; } };
        }
        importNode(node, deep) { return node.cloneNode(deep); }
        adoptNode(node) {
            // Detach from current parent, adopt into this document
            if (node.parentNode) node.parentNode.removeChild(node);
            return node;
        }
        createAttribute(name) {
            return { name, value: "", specified: true };
        }
        // document.open/close — reset and finalize document stream
        open() { return this; }
        close() {}
        write(html) {
            // Document.write in Chrome synchronously executes any <script> tags
            // it inserts. Since op_dom_document_write returns the IDs of the
            // newly created nodes, we wrap them and trigger our insertion logic.
            const newIds = ops.op_dom_document_write(String(html));
            if (Array.isArray(newIds)) {
                for (const id of newIds) {
                    const node = _wrapNode(id);
                    if (node) _onNodeInserted(node, true); // Always sync for document.write
                }
            }
        }
        writeln(html) {
            this.write(html + "\n");
        }
        // Selection and editing
        execCommand(command, showUI, value) { return false; }
        queryCommandSupported(command) { return false; }
        queryCommandEnabled(command) { return false; }
        getSelection() { return globalThis.getSelection ? globalThis.getSelection() : null; }
        // Point-based queries
        elementFromPoint(x, y) { return this.body; }
        elementsFromPoint(x, y) { return this.body ? [this.body] : []; }
        caretPositionFromPoint(x, y) { return null; }
        hasFocus() { return true; }  // Anti-bot: must return true
        get readyState() { 
            return (globalThis._boxide && globalThis._boxide.__documentReadyState) || "complete"; 
        }
        get URL() { return globalThis.location?.href || "about:blank"; }
        get documentURI() { return this.URL; }
        get domain() { return globalThis.location?.hostname || ""; }
        get location() { return globalThis.location; }
        set location(val) { if (globalThis.location) globalThis.location.href = val; }
        get referrer() { return ""; }
        get hidden() { return false; }
        get visibilityState() { return "visible"; }
        get cookie() {
            // Unified cookie jar: returns the mirror of net::cookies for this origin.
            // The mirror is refreshed synchronously on every page navigation and after
            // each fetch() response via _syncCookiesFromNet().
            if (!globalThis.__jsCookies) globalThis.__jsCookies = {};
            return Object.entries(globalThis.__jsCookies)
                .map(([k, v]) => `${k}=${v}`)
                .join("; ");
        }
        set cookie(val) {
            // Parse "name=value; path=/; ..." — update local mirror AND push to net::cookies.
            if (!globalThis.__jsCookies) globalThis.__jsCookies = {};
            const parts = String(val).split(";");
            const [name, ...rest] = (parts[0] || "").split("=");
            const key = name.trim();
            const value = rest.join("=").trim();
            if (!key) return;
            // Check for max-age=0 or expires in the past (delete cookie)
            const lower = String(val).toLowerCase();
            if (lower.includes("max-age=0") || lower.includes("max-age=-")) {
                delete globalThis.__jsCookies[key];
            } else {
                globalThis.__jsCookies[key] = value;
            }
            // Fire-and-forget propagation to the net layer.
            try {
                let url = globalThis.location?.href;
                if (!url || url === "about:blank" || url === "javascript:;" || url === "") {
                    url = globalThis.__boxide && globalThis.__boxide._baseUrl;
                }
                if (url && ops.op_cookie_set) {
                    ops.op_cookie_set(url, String(val));
                }
            } catch (e) { /* ignore */ }
        }
        // HTML legacy default per HTML Standard §2.4 — Chrome reports
        // "windows-1252" for HTML documents without an explicit
        // `<meta charset>` declaration. Verified via Playwright MCP
        // probe_mcp.json + probe_mcp_secure.json (both report
        // "windows-1252"). Phase 7.
        get characterSet() { return "windows-1252"; }
        get charset() { return "windows-1252"; }
        get contentType() { return "text/html"; }
        get compatMode() { return "CSS1Compat"; }
        // document.implementation — the DOMImplementation API. fpCollect and
        // several bot tests call createHTMLDocument() to verify the surface.
        get implementation() {
            return {
                createHTMLDocument(title) {
                    // Return a stub document with just enough of the Document
                    // API to satisfy fingerprinters. Real browsers return a
                    // fully functional Document, but our stubs never read it.
                    return {
                        title: title || "",
                        body: { innerHTML: "", appendChild: () => {} },
                        head: { appendChild: () => {} },
                        documentElement: { innerHTML: "" },
                        createElement(tag) {
                            return { tagName: tag.toUpperCase(), innerHTML: "", appendChild: () => {} };
                        },
                        createTextNode(t) { return { nodeValue: t }; },
                        querySelector() { return null; },
                        querySelectorAll() { return []; },
                    };
                },
                createDocument(ns, qualifiedName, doctype) {
                    return this.createHTMLDocument("");
                },
                createDocumentType(qualifiedName, publicId, systemId) {
                    return { name: qualifiedName, publicId, systemId };
                },
                hasFeature() { return true; },
            };
        }
        get doctype() { return null; }
        get defaultView() { return globalThis; }
        get activeElement() { return this.body; }
        get scripts() { return this.getElementsByTagName("script"); }
        get forms() { return this.getElementsByTagName("form"); }
        get images() { return this.getElementsByTagName("img"); }
        get links() { return this.getElementsByTagName("a"); }
        get embeds() { return this.getElementsByTagName("embed"); }
        get anchors() { return this.querySelectorAll("a[name]"); }
        get styleSheets() {
            const count = ops.op_dom_get_stylesheet_count();
            const sheets = [];
            for (let i = 0; i < count; i++) {
                sheets.push(new CSSStyleSheet(i));
            }
            return sheets;
        }
        get fullscreenElement() { return null; }
        get pointerLockElement() { return null; }
        exitFullscreen() { return Promise.resolve(); }
        exitPointerLock() {}
    }

    // --- CSSOM ---
    class CSSStyleSheet {
        constructor(index) { this._index = index; }
        get type() { return "text/css"; }
        get disabled() { return false; }
        get ownerNode() { return null; }
        get parentStyleSheet() { return null; }
        get title() { return null; }
        get media() { return { length: 0, mediaText: "" }; }
        get cssRules() {
            const raw = ops.op_dom_get_stylesheet_rules(this._index);
            return raw.map(r => new CSSStyleRule(r));
        }
        get rules() { return this.cssRules; }
        insertRule(_rule, _index) { return 0; }
        deleteRule(_index) {}
    }

    class CSSStyleRule {
        constructor({ selector_text, css_text, rule_type }) {
            this.selectorText = selector_text;
            this.cssText = css_text;
            this.type = rule_type;
            // Parse declarations into style-like object
            const styleObj = {};
            const declMatch = css_text.match(/\{([^}]*)\}/);
            if (declMatch) {
                for (const part of declMatch[1].split(";")) {
                    const [prop, ...vals] = part.split(":");
                    if (prop && vals.length) {
                        const p = prop.trim();
                        const v = vals.join(":").trim();
                        styleObj[p] = v;
                        // Also set camelCase version
                        const camel = p.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
                        if (camel !== p) styleObj[camel] = v;
                    }
                }
            }
            this.style = styleObj;
        }
    }

    // --- Range (minimal) ---
    class Range {
        constructor() {
            this.startContainer = null; this.startOffset = 0;
            this.endContainer = null; this.endOffset = 0;
            this.collapsed = true; this.commonAncestorContainer = null;
        }
        setStart(node, offset) { this.startContainer = node; this.startOffset = offset; this.collapsed = false; }
        setEnd(node, offset) { this.endContainer = node; this.endOffset = offset; }
        collapse(toStart) { this.collapsed = true; }
        cloneRange() { return new Range(); }
        getBoundingClientRect() { return new DOMRect(); }
        getClientRects() { return []; }
        createContextualFragment(html) {
            const div = _document.createElement("div");
            div.innerHTML = html;
            const frag = _document.createDocumentFragment();
            while (div.firstChild) frag.appendChild(div.firstChild);
            return frag;
        }
        toString() { return ""; }
    }

    // --- Selection (minimal) ---
    class Selection {
        get anchorNode() { return null; }
        get anchorOffset() { return 0; }
        get focusNode() { return null; }
        get focusOffset() { return 0; }
        get isCollapsed() { return true; }
        get rangeCount() { return 0; }
        getRangeAt(i) { return new Range(); }
        addRange(range) {}
        removeRange(range) {}
        removeAllRanges() {}
        collapse(node, offset) {}
        toString() { return ""; }
    }
    const _selection = new Selection();

    // Create the global document
    const _document = new Document(ops.op_dom_document_node());
    _nodeCache.set(ops.op_dom_document_node(), new WeakRef(_document));

    // Set globals
    // Symbol.toStringTag on every DOM class — Akamai BMP v3 and DataDome
    // check Object.prototype.toString.call(node) and expect the Chrome
    // WebIDL brand name like "[object HTMLDivElement]". Without these
    // tags every node shows as "[object Object]" which is an instant bot
    // signal.
    const _tag = (cls, name) => {
        try {
            Object.defineProperty(cls.prototype, Symbol.toStringTag, {
                value: name, configurable: true,
            });
        } catch {}
    };
    _tag(EventTarget, "EventTarget");
    _tag(Node, "Node");
    _tag(Element, "Element");
    _tag(HTMLElement, "HTMLElement");
    _tag(HTMLDivElement, "HTMLDivElement");
    _tag(HTMLSpanElement, "HTMLSpanElement");
    _tag(HTMLParagraphElement, "HTMLParagraphElement");
    _tag(HTMLHeadingElement, "HTMLHeadingElement");
    _tag(HTMLAnchorElement, "HTMLAnchorElement");
    _tag(HTMLImageElement, "HTMLImageElement");
    _tag(HTMLInputElement, "HTMLInputElement");
    _tag(HTMLFormElement, "HTMLFormElement");
    _tag(HTMLButtonElement, "HTMLButtonElement");
    _tag(HTMLSelectElement, "HTMLSelectElement");
    _tag(HTMLTextAreaElement, "HTMLTextAreaElement");
    _tag(HTMLCanvasElement, "HTMLCanvasElement");
    _tag(HTMLScriptElement, "HTMLScriptElement");
    _tag(HTMLStyleElement, "HTMLStyleElement");
    _tag(HTMLLinkElement, "HTMLLinkElement");
    _tag(HTMLMetaElement, "HTMLMetaElement");
    _tag(HTMLTableElement, "HTMLTableElement");
    _tag(HTMLIFrameElement, "HTMLIFrameElement");
    _tag(HTMLVideoElement, "HTMLVideoElement");
    _tag(HTMLAudioElement, "HTMLAudioElement");
    _tag(HTMLBodyElement, "HTMLBodyElement");
    _tag(HTMLHeadElement, "HTMLHeadElement");
    _tag(HTMLHtmlElement, "HTMLHtmlElement");
    _tag(HTMLUListElement, "HTMLUListElement");
    _tag(HTMLOListElement, "HTMLOListElement");
    _tag(HTMLLIElement, "HTMLLIElement");
    _tag(HTMLTableRowElement, "HTMLTableRowElement");
    _tag(HTMLTableCellElement, "HTMLTableCellElement");
    _tag(HTMLTableSectionElement, "HTMLTableSectionElement");
    _tag(HTMLLabelElement, "HTMLLabelElement");
    _tag(HTMLOptionElement, "HTMLOptionElement");
    _tag(HTMLTemplateElement, "HTMLTemplateElement");
    _tag(HTMLPreElement, "HTMLPreElement");
    _tag(HTMLQuoteElement, "HTMLQuoteElement");
    _tag(Text, "Text");
    _tag(Comment, "Comment");
    _tag(DocumentFragment, "DocumentFragment");
    // Chrome exposes document as HTMLDocument (which extends Document).
    _tag(Document, "HTMLDocument");
    _tag(NodeList, "NodeList");
    _tag(DOMTokenList, "DOMTokenList");

    // documentElement (HTMLHtmlElement) and body (HTMLBodyElement) layout
    // dimensions in standards mode are viewport-clipped, NOT full document.
    // Default Element getters return offsetWidth/Height = full document
    // (e.g. 1914 × 28638 on walmart) which is a damning headless tell — see
    // Akamai pixel POST `client` field and `sr.client` in sensor_data.
    // Real Chrome returns innerWidth × innerHeight (1440 × 789 on a typical
    // macOS 1440x900 viewport).
    {
        const _viewportW = () => (globalThis.innerWidth | 0) || 1440;
        const _viewportH = () => (globalThis.innerHeight | 0) || 789;
        Object.defineProperty(HTMLHtmlElement.prototype, 'clientWidth',  { get() { return _viewportW(); }, configurable: true });
        Object.defineProperty(HTMLHtmlElement.prototype, 'clientHeight', { get() { return _viewportH(); }, configurable: true });
        // documentElement.scrollWidth/Height are still full content size,
        // so leave the inherited offset-based getters in place for those.
    }

    globalThis.document = _document;
    globalThis.Document = Document;
    globalThis.HTMLDocument = Document;
    globalThis.Node = Node;
    globalThis.Element = Element;
    // Expose the real HTMLElement subclasses — the prototype chain is
    // EventTarget ← Node ← Element ← HTMLElement ← HTML*Element so that
    // `el instanceof HTMLDivElement` etc. works as in real Chrome.
    globalThis.HTMLElement = HTMLElement;
    globalThis.HTMLDivElement = HTMLDivElement;
    globalThis.HTMLSpanElement = HTMLSpanElement;
    globalThis.HTMLParagraphElement = HTMLParagraphElement;
    globalThis.HTMLHeadingElement = HTMLHeadingElement;
    globalThis.HTMLAnchorElement = HTMLAnchorElement;
    globalThis.HTMLImageElement = HTMLImageElement;
    globalThis.HTMLInputElement = HTMLInputElement;
    globalThis.HTMLFormElement = HTMLFormElement;
    globalThis.HTMLButtonElement = HTMLButtonElement;
    globalThis.HTMLSelectElement = HTMLSelectElement;
    globalThis.HTMLTextAreaElement = HTMLTextAreaElement;
    globalThis.HTMLCanvasElement = HTMLCanvasElement;
    globalThis.HTMLScriptElement = HTMLScriptElement;
    globalThis.HTMLStyleElement = HTMLStyleElement;
    globalThis.HTMLLinkElement = HTMLLinkElement;
    globalThis.HTMLMetaElement = HTMLMetaElement;
    globalThis.HTMLTableElement = HTMLTableElement;
    globalThis.HTMLIFrameElement = HTMLIFrameElement;
    globalThis.HTMLVideoElement = HTMLVideoElement;
    globalThis.HTMLAudioElement = HTMLAudioElement;
    globalThis.HTMLBodyElement = HTMLBodyElement;
    globalThis.HTMLHeadElement = HTMLHeadElement;
    globalThis.HTMLHtmlElement = HTMLHtmlElement;
    globalThis.HTMLUListElement = HTMLUListElement;
    globalThis.HTMLOListElement = HTMLOListElement;
    globalThis.HTMLLIElement = HTMLLIElement;
    globalThis.HTMLTableRowElement = HTMLTableRowElement;
    globalThis.HTMLTableCellElement = HTMLTableCellElement;
    globalThis.HTMLTableSectionElement = HTMLTableSectionElement;
    globalThis.HTMLLabelElement = HTMLLabelElement;
    globalThis.HTMLOptionElement = HTMLOptionElement;
    globalThis.HTMLTemplateElement = HTMLTemplateElement;
    globalThis.HTMLPreElement = HTMLPreElement;
    globalThis.HTMLQuoteElement = HTMLQuoteElement;
    globalThis.SVGElement = Element;
    globalThis.Text = Text;
    globalThis.Comment = Comment;
    globalThis.DocumentFragment = DocumentFragment;
    globalThis.Document = Document;
    globalThis.NodeList = NodeList;
    globalThis.DOMTokenList = DOMTokenList;
    globalThis.DOMRect = DOMRect;
    globalThis.DOMRectReadOnly = DOMRect;
    globalThis.Range = Range;
    globalThis.Selection = Selection;
    globalThis.getSelection = function() { return _selection; };

    // Image constructor — new Image(width, height). Returns an
    // HTMLImageElement whose naturalWidth/naturalHeight/complete are
    // accessors defined on the prototype (getters; not writable).
    // Constructor return of an object is the caller's `new Image(...)`.
    globalThis.Image = function Image(width, height) {
        const el = _document.createElement("img");
        if (width !== undefined) el.setAttribute("width", String(width));
        if (height !== undefined) el.setAttribute("height", String(height));
        return el;
    };

    // DOMParser
    globalThis.DOMParser = class DOMParser {
        parseFromString(str, type) {
            // Returns a minimal document-like object
            const frag = _document.createElement("div");
            frag.innerHTML = str;
            return {
                documentElement: frag,
                body: frag,
                querySelector(sel) { return frag.querySelector(sel); },
                querySelectorAll(sel) { return frag.querySelectorAll(sel); },
                getElementById(id) { return frag.querySelector("#" + id); },
            };
        }
    };

    // --- MutationObserver (real implementation) ---
    const _moObservers = []; // { observer, target, options }

    class MutationRecord {
        constructor(type, target) {
            this.type = type;
            this.target = target;
            this.addedNodes = [];
            this.removedNodes = [];
            this.attributeName = null;
            this.oldValue = null;
            this.previousSibling = null;
            this.nextSibling = null;
        }
    }

    class MutationObserver {
        constructor(callback) {
            this._callback = callback;
            this._records = [];
            this._active = false;
            this._targets = new Map(); // nodeId → options
        }
        observe(target, options = {}) {
            const nodeId = _getNodeId(target);
            this._targets.set(nodeId, { target, options });
            this._active = true;
            _moObservers.push(this);
        }
        disconnect() {
            this._active = false;
            this._targets.clear();
            const idx = _moObservers.indexOf(this);
            if (idx !== -1) _moObservers.splice(idx, 1);
        }
        takeRecords() {
            const r = this._records.slice();
            this._records = [];
            return r;
        }
        _notify(record) {
            if (!this._active) return;
            this._records.push(record);
            // Schedule microtask to deliver
            if (this._records.length === 1) {
                Promise.resolve().then(() => {
                    if (!this._active) return;
                    const batch = this._records.slice();
                    this._records = [];
                    if (batch.length > 0) this._callback(batch, this);
                });
            }
        }
    }

    // Notify matching observers of a mutation
    function _notifyMO(type, targetNodeId, init) {
        for (const obs of _moObservers) {
            if (!obs._active) continue;
            // Check if this observer watches this target (or subtree ancestor)
            let matched = obs._targets.has(targetNodeId);
            if (!matched) {
                // Check subtree: walk ancestors
                for (const [watchedId, { options }] of obs._targets) {
                    if (options.subtree) {
                        // Walk up from targetNodeId to see if watchedId is ancestor
                        let nid = targetNodeId;
                        while (nid !== -1 && nid !== null) {
                            if (nid === watchedId) { matched = true; break; }
                            nid = ops.op_dom_get_parent(nid);
                        }
                    }
                    if (matched) break;
                }
            }
            if (!matched) continue;

            // Check options match
            const opts = obs._targets.get(targetNodeId)?.options ||
                         [...obs._targets.values()].find(v => v.options.subtree)?.options || {};
            if (type === "childList" && !opts.childList) continue;
            if (type === "attributes" && !opts.attributes) continue;
            if (type === "characterData" && !opts.characterData) continue;

            const record = new MutationRecord(type, init.target || null);
            if (init.addedNodes) record.addedNodes = init.addedNodes;
            if (init.removedNodes) record.removedNodes = init.removedNodes;
            if (init.attributeName) record.attributeName = init.attributeName;
            obs._notify(record);
        }
    }

    // Custom element lifecycle helper
    function _ceConnected(el) {
        if (el && el._ceUpgraded && typeof el.connectedCallback === "function") {
            try { el.connectedCallback(); } catch (e) { console.error(e); }
        }
    }
    function _ceDisconnected(el) {
        if (el && el._ceUpgraded && typeof el.disconnectedCallback === "function") {
            try { el.disconnectedCallback(); } catch (e) { console.error(e); }
        }
    }

    // Window frame registry: tracks appended iframes so window[0], window[1], etc.
    // work correctly. Kasada's ifw probe accesses window.frames[0].navigator.webdriver
    // (which is window[0] since frames===window in our engine). Without this,
    // window[0] is undefined → TypeError "Cannot read properties of undefined
    // (reading 'webdriver')".
    const _appendedIframes = [];

    // Wrap DOM mutation methods to fire MO notifications
    const _origAppendChild = Node.prototype.appendChild;
    Node.prototype.appendChild = function(child) {
        const result = _origAppendChild.call(this, child);
        // Register iframes in the parent window's frame list (window[N] access)
        try {
            if (typeof HTMLIFrameElement !== 'undefined' && child instanceof HTMLIFrameElement) {
                const _fi = _appendedIframes.length;
                _appendedIframes.push(child);
                // Define lazy getter for window[N] — contentWindow is created on demand
                Object.defineProperty(globalThis, String(_fi), {
                    get: function() { return _getIframeWindow(_appendedIframes[_fi]); },
                    configurable: true, enumerable: false,
                });
                // Update window.length (= number of child frames)
                try {
                    Object.defineProperty(globalThis, 'length', {
                        value: _appendedIframes.length, configurable: true, writable: true,
                    });
                } catch (_) {}
            }
        } catch (_) {}
        if (_moObservers.length > 0) {
            _notifyMO("childList", _getNodeId(this), { target: this, addedNodes: [child] });
        }
        return result;
    };

    const _origRemoveChild = Node.prototype.removeChild;
    Node.prototype.removeChild = function(child) {
        const result = _origRemoveChild.call(this, child);
        if (_moObservers.length > 0) {
            _notifyMO("childList", _getNodeId(this), { target: this, removedNodes: [child] });
        }
        return result;
    };

    const _origInsertBefore = Node.prototype.insertBefore;
    Node.prototype.insertBefore = function(newChild, refChild) {
        const result = _origInsertBefore.call(this, newChild, refChild);
        if (_moObservers.length > 0) {
            _notifyMO("childList", _getNodeId(this), { target: this, addedNodes: [newChild] });
        }
        return result;
    };

    const _origSetAttribute = Element.prototype.setAttribute;
    Element.prototype.setAttribute = function(name, value) {
        const oldVal = this.getAttribute(name);
        _origSetAttribute.call(this, name, value);
        if (_moObservers.length > 0) {
            _notifyMO("attributes", _getNodeId(this), { target: this, attributeName: name });
        }
        // Custom element attributeChangedCallback
        if (this._ceUpgraded && typeof this.attributeChangedCallback === "function") {
            const observed = this.constructor.observedAttributes;
            if (Array.isArray(observed) && observed.includes(name)) {
                try { this.attributeChangedCallback(name, oldVal, value); } catch (e) { console.error(e); }
            }
        }
    };

    const _origRemoveAttribute = Element.prototype.removeAttribute;
    Element.prototype.removeAttribute = function(name) {
        const oldVal = this.getAttribute(name);
        _origRemoveAttribute.call(this, name);
        if (_moObservers.length > 0) {
            _notifyMO("attributes", _getNodeId(this), { target: this, attributeName: name });
        }
        // Custom element attributeChangedCallback
        if (this._ceUpgraded && typeof this.attributeChangedCallback === "function") {
            const observed = this.constructor.observedAttributes;
            if (Array.isArray(observed) && observed.includes(name)) {
                try { this.attributeChangedCallback(name, oldVal, null); } catch (e) { console.error(e); }
            }
        }
    };

    // Element.remove() also triggers childList on parent
    const _origRemove = Element.prototype.remove;
    Element.prototype.remove = function() {
        const parent = this.parentNode;
        _ceDisconnected(this);
        _origRemove.call(this);
        if (_moObservers.length > 0 && parent) {
            _notifyMO("childList", _getNodeId(parent), { target: parent, removedNodes: [this] });
        }
    };

    globalThis.MutationObserver = MutationObserver;
    globalThis.MutationRecord = MutationRecord;

    // --- iframe support (contentWindow / contentDocument) ---
    //
    // Kasada, Castle, CreepJS, and DataDome all perform iframe-realm checks:
    // they create or find an <iframe>, access `.contentWindow`, then pull
    // native constructors (TextEncoder, Function, Array, ...) from the iframe
    // window to compare against the main window's versions. A mismatch
    // reveals monkey-patching; an `undefined` contentWindow reveals a headless
    // browser that doesn't support iframes.
    //
    // We install `contentWindow` and `contentDocument` as GETTERS on
    // HTMLIFrameElement.prototype so EVERY iframe — whether parsed from HTML
    // or created via document.createElement — returns a valid window-shaped
    // Proxy that falls through to globalThis for any unknown property. The
    // per-iframe state is cached in a WeakMap keyed by the element.

    const _iframeState = new WeakMap();

    // Build a mirror realm: fresh constructors that mimic the parent's shape
    // but are reference-distinct, so cross-realm probes like
    //   iframe.contentWindow.Navigator !== Navigator
    //   iframe.contentWindow.Navigator.prototype !== Navigator.prototype
    // hold true while own-property-names lists remain identical. Each
    // mirrored function carries _nativeTag so Function.prototype.toString
    // produces "function NAME() { [native code] }" cross-realm.
    const _MIRRORED_CONSTRUCTORS = [
        "Navigator", "Window", "Document", "HTMLDocument",
        "EventTarget", "Node", "Element", "HTMLElement",
        "HTMLDivElement", "HTMLSpanElement", "HTMLBodyElement",
        "HTMLAnchorElement", "HTMLImageElement", "HTMLInputElement",
        "HTMLFormElement", "HTMLButtonElement", "HTMLSelectElement",
        "HTMLTextAreaElement", "HTMLCanvasElement", "HTMLScriptElement",
        "HTMLIFrameElement", "Event", "CustomEvent", "MouseEvent",
        "KeyboardEvent", "MessageEvent", "Array", "Object", "Function",
        "String", "Number", "Boolean", "Promise", "Error", "TypeError",
        "RangeError", "Map", "Set", "WeakMap", "WeakSet", "Date",
        "RegExp", "Symbol",
    ];

    // Capture the native-tag Symbol from the parent realm. stealth_bootstrap.js
    // exposes it as globalThis._nativeTag. We capture explicitly so the
    // freshToString and _mkNativeFn don't accidentally see undefined when
    // bare-identifier scope chain is shadowed by the IIFE parameter.
    const _NATIVE_TAG_SYMBOL = globalThis._nativeTag || Symbol.for('__boxide_native__');

    function _mkNativeFn(name) {
        const fn = function() {};
        try {
            Object.defineProperty(fn, "name", { value: name, configurable: true });
            Object.defineProperty(fn, _NATIVE_TAG_SYMBOL, { value: name, configurable: true });
            // Per-instance toString returning native shape — used when the
            // patched Function.prototype.toString is bypassed by direct
            // .toString() calls. Mirrors stealth_bootstrap's _maskFunction.
            const ts = function toString() { return "function " + name + "() { [native code] }"; };
            Object.defineProperty(ts, _NATIVE_TAG_SYMBOL, { value: "toString", configurable: true });
            Object.defineProperty(ts, "name", { value: "toString", configurable: true });
            Object.defineProperty(fn, "toString", { value: ts, configurable: true });
        } catch (_) {}
        return fn;
    }

    // Constructors where `new w.X(...)` is genuinely "Illegal constructor"
    // in real Chrome (DOM interfaces with no exposed constructor). Calls
    // to `new` on these throw `TypeError: Illegal constructor`.
    // Constructors NOT in this set are real callable types — for those we
    // delegate `new` to the parent realm's constructor via `Reflect.construct`
    // so e.g. `new iframe.contentWindow.Function("return 1")` returns a
    // function in the iframe realm, matching real Chrome. Kasada probes
    // `new w.Function(...)` to materialize a fresh-realm function; if we
    // throw where real Chrome succeeds, that's a definitive headless tell.
    const _ILLEGAL_CONSTRUCTORS = new Set([
        "Navigator", "Window", "Document", "HTMLDocument",
        "Node", "Element", "HTMLElement",
        "HTMLDivElement", "HTMLSpanElement", "HTMLBodyElement",
        "HTMLAnchorElement", "HTMLImageElement", "HTMLInputElement",
        "HTMLFormElement", "HTMLButtonElement", "HTMLSelectElement",
        "HTMLTextAreaElement", "HTMLCanvasElement", "HTMLScriptElement",
        "HTMLIFrameElement",
    ]);

    function _mkMirroredConstructor(parentCtor, name, freshGrandparentProto) {
        // Fresh constructor function — different identity than parent's.
        // For DOM-interface types real Chrome throws on `new`; for genuine
        // callable types (Function/Array/Map/Date/Event/...) we delegate to
        // the parent constructor via Reflect.construct so the result lives
        // in our fresh realm (via fresh.prototype = freshProto below).
        const isIllegal = _ILLEGAL_CONSTRUCTORS.has(name);
        const fresh = isIllegal
            ? function() {
                throw new TypeError("Failed to construct '" + name + "': Illegal constructor");
            }
            : function(...args) {
                try {
                    return Reflect.construct(parentCtor, args, fresh);
                } catch (e) {
                    // Symbol() throws on `new`; re-throw with the parent's
                    // exact shape (don't reword) so feature-detection that
                    // catches "Symbol is not a constructor" still matches.
                    throw e;
                }
            };
        try {
            Object.defineProperty(fresh, "name", { value: name, configurable: true });
            Object.defineProperty(fresh, _NATIVE_TAG_SYMBOL, { value: name, configurable: true });
            const ts = function toString() { return "function " + name + "() { [native code] }"; };
            Object.defineProperty(ts, _NATIVE_TAG_SYMBOL, { value: "toString", configurable: true });
            Object.defineProperty(ts, "name", { value: "toString", configurable: true });
            Object.defineProperty(fresh, "toString", { value: ts, configurable: true });
        } catch (_) {}

        // Build a fresh prototype mirroring own-property-names of parent's prototype.
        // Each method/getter/setter is a fresh function with native toString shape.
        let parentProto = null;
        try { parentProto = parentCtor && parentCtor.prototype; } catch (_) {}
        // The fresh prototype's own __proto__ must point at the FRESH grandparent
        // prototype (built earlier in _buildRemoteRealm's topological pass),
        // NOT at the parent realm's grandparent. Crossing realms here makes
        // creepjs's lie-detection walk the parent realm's full chain on top
        // of the fresh chain, multiplying its work O(N) → O(N²+).
        const freshProto = Object.create(freshGrandparentProto || Object.prototype);

        if (parentProto) {
            const propNames = Object.getOwnPropertyNames(parentProto);
            for (const propName of propNames) {
                if (propName === "constructor") continue;
                let desc;
                try { desc = Object.getOwnPropertyDescriptor(parentProto, propName); } catch (_) { continue; }
                if (!desc) continue;
                const newDesc = {
                    configurable: desc.configurable !== false,
                    enumerable: !!desc.enumerable,
                };
                if (desc.get || desc.set) {
                    if (desc.get) newDesc.get = _mkNativeFn("get " + propName);
                    if (desc.set) newDesc.set = _mkNativeFn("set " + propName);
                } else {
                    newDesc.writable = desc.writable !== false;
                    if (typeof desc.value === "function") {
                        // Function-valued props: replace with our fresh native-shape stub
                        // (so cross-realm Function.prototype.toString.call(this) returns
                        // "function NAME() { [native code] }").
                        newDesc.value = _mkNativeFn(propName);
                    } else {
                        newDesc.value = desc.value;
                    }
                }
                try { Object.defineProperty(freshProto, propName, newDesc); } catch (_) {}
            }
        }

        try {
            Object.defineProperty(freshProto, "constructor", {
                value: fresh, writable: true, enumerable: false, configurable: true,
            });
            Object.defineProperty(fresh, "prototype", {
                value: freshProto, writable: false, enumerable: false, configurable: false,
            });
        } catch (_) {}
        return fresh;
    }

    // For each mirrored constructor name, find the nearest ancestor in
    // _MIRRORED_CONSTRUCTORS by walking the real prototype chain. Returns
    // an array of names in topological order (ancestors before descendants)
    // and a name -> direct-parent-name map.
    function _topoSortMirrored(names) {
        const realCtors = {};
        for (const n of names) {
            try {
                const c = globalThis[n];
                if (typeof c === "function") realCtors[n] = c;
            } catch (_) {}
        }
        const directParent = {};
        for (const n of names) {
            const ctor = realCtors[n];
            if (!ctor) { directParent[n] = null; continue; }
            let proto = null;
            try { proto = Object.getPrototypeOf(ctor.prototype); } catch (_) {}
            let parentName = null;
            let guard = 0;
            while (proto && guard++ < 32) {
                for (const m of names) {
                    const mc = realCtors[m];
                    if (mc && mc.prototype === proto) { parentName = m; break; }
                }
                if (parentName) break;
                try { proto = Object.getPrototypeOf(proto); } catch (_) { break; }
            }
            directParent[n] = parentName;
        }
        const ordered = [];
        const remaining = new Set(names);
        while (remaining.size > 0) {
            let progress = false;
            for (const n of Array.from(remaining)) {
                const p = directParent[n];
                if (p == null || !remaining.has(p)) {
                    ordered.push(n);
                    remaining.delete(n);
                    progress = true;
                }
            }
            if (!progress) {
                // Defensive: cyclic dependency in the real prototype graph
                // shouldn't happen, but if it does, append remaining without
                // ordering rather than infinite-looping.
                for (const n of remaining) ordered.push(n);
                break;
            }
        }
        return { ordered: ordered, directParent: directParent };
    }

    // Module-level cache: every iframe in this realm shares the same set of
    // mirrored constructors. Kasada's VM tags function/descriptor objects on
    // first scope-chain walk and re-reads on a later walk; without this cache
    // every _getIframeWindow() call rebuilt the realm and Kasada's sentinel
    // property (`unjzomuybtbyyhwwkdpkxomylnab`) was lost on the second read.
    let _cachedRemoteRealm = null;

    function _buildRemoteRealm() {
        if (_cachedRemoteRealm) return _cachedRemoteRealm;
        const realm = {};
        const sorted = _topoSortMirrored(_MIRRORED_CONSTRUCTORS);
        for (const name of sorted.ordered) {
            try {
                const parentCtor = globalThis[name];
                if (typeof parentCtor !== "function") continue;
                const parentName = sorted.directParent[name];
                const freshGrandparentProto = parentName && realm[parentName]
                    ? realm[parentName].prototype
                    : Object.prototype;
                realm[name] = _mkMirroredConstructor(parentCtor, name, freshGrandparentProto);
            } catch (_) {}
        }
        _cachedRemoteRealm = realm;
        return realm;
    }

    // Monotonically-increasing ID for child realms; used as the Rust-side
    // cache key in IframeRealmStore (HashMap<u32, ...>).
    let _nextRealmId = 0;

    // Extract scheme+host+port from a URL without using new URL().
    // Returns "null" for non-http(s) URLs (data:, about:, etc.) or empty input.
    const _xOrigin = function(u) {
        var m = u && u.match(/^(https?:\/\/[^/?#:]+(?::\d+)?)/i);
        return m ? m[1].toLowerCase() : "null";
    };

    function _getIframeWindow(el) {
        let state = _iframeState.get(el);
        if (state) {
            // Kasada's crs probe: creates an iframe with no src, accesses contentWindow
            // (creates child realm), then sets src to a cross-origin URL and re-accesses.
            // When src changes to cross-origin, invalidate the cached realm and return a
            // SecurityError proxy — exactly what real Chrome does.
            try {
                const _cSrc = (el && typeof el.getAttribute === "function")
                    ? (el.getAttribute("src") || el.src || "")
                    : (el && el.src || "");
                if (_cSrc && _cSrc !== "about:blank" && !/^javascript:/i.test(_cSrc) && _cSrc !== "") {
                    const _pOrig = _xOrigin((globalThis.location && globalThis.location.href) || "");
                    const _sOrig = _xOrigin(_cSrc);
                    if (_sOrig !== _pOrig) {
                        const _xM = 'Blocked a frame with origin "' + _pOrig + '" from accessing a cross-origin frame.';
                        const _xo2 = new Proxy({}, {
                            get(t, p) { if (typeof p === 'symbol') return undefined; throw new DOMException(_xM, 'SecurityError'); },
                            set() { throw new DOMException(_xM, 'SecurityError'); },
                            has() { return false; },
                        });
                        const _xoS2 = { contentWindow: _xo2, contentDocument: null, _realmId: undefined, _processedSrcdoc: '' };
                        _iframeState.set(el, _xoS2);
                        return _xo2;
                    }
                }
            } catch (_) {}
            // Re-run srcdoc scripts if srcdoc was set after initial contentWindow access.
            // Kasada may set iframe.srcdoc = "..." before or after first contentWindow
            // access; in either case we must execute the scripts in the child realm.
            if (state._realmId !== undefined) {
                let _cur = "";
                try { _cur = el.getAttribute("srcdoc") || el.srcdoc || ""; } catch (_) {}
                if (_cur && _cur !== state._processedSrcdoc) {
                    state._processedSrcdoc = _cur;
                    try {
                        const _re = /<script[^>]*>([\s\S]*?)<\/script>/gi;
                        let _m2;
                        while ((_m2 = _re.exec(_cur)) !== null) {
                            const _s2 = _m2[1];
                            if (_s2 && _s2.trim()) {
                                try { ops.op_eval_in_child_realm(state._realmId, _s2); } catch (_) {}
                            }
                        }
                    } catch (_) {}
                }
            }
            return state.contentWindow;
        }

        // ── Cross-origin iframe detection (crs probe) ────────────────────
        // Kasada's crs probe creates an iframe with a different origin (e.g. a
        // data: URI or cross-origin https URL) and expects V8's SecurityError
        // when accessing contentWindow.document. Return a Proxy that throws
        // SecurityError on any property read — matches real Chrome behaviour.
        try {
            const _iSrc = (el && typeof el.getAttribute === "function")
                ? (el.getAttribute("src") || el.src || "")
                : (el && el.src || "");
            if (_iSrc && _iSrc !== "about:blank" && !/^javascript:/i.test(_iSrc) && _iSrc !== "") {
                const _pOrigin = _xOrigin((globalThis.location && globalThis.location.href) || "");
                const _srcOrigin = _xOrigin(_iSrc);
                if (_srcOrigin !== _pOrigin) {
                    const _xMsg = 'Blocked a frame with origin "' + _pOrigin + '" from accessing a cross-origin frame.';
                    const _xo = new Proxy({}, {
                        get(t, p) {
                            if (typeof p === 'symbol') return undefined;
                            throw new DOMException(_xMsg, 'SecurityError');
                        },
                        set() { throw new DOMException(_xMsg, 'SecurityError'); },
                        has() { return false; },
                    });
                    const _xoState = { contentWindow: _xo, contentDocument: null, _realmId: undefined, _processedSrcdoc: '' };
                    _iframeState.set(el, _xoState);
                    return _xo;
                }
            }
        } catch (_) {}

        // ── Build the iframe document shell ──────────────────────────────
        // W3.5 — srcdoc iframes: expose the source text for fingerprint-grade
        // reads (`iframe.contentDocument.body.innerHTML`).
        let _srcdoc = "";
        try {
            if (el && typeof el.getAttribute === "function") {
                _srcdoc = el.getAttribute("srcdoc") || "";
            }
            // Also check direct JS property (set via el.srcdoc = "...") since
            // property assignment may not update the HTML attribute in our DOM.
            if (!_srcdoc && el && typeof el.srcdoc === "string") {
                _srcdoc = el.srcdoc;
            }
        } catch (_) {}
        const _mkHtmlMirror = (tag, inner) => ({
            tagName: tag.toUpperCase(),
            nodeType: 1,
            innerHTML: inner,
            outerHTML: "<" + tag + ">" + inner + "</" + tag + ">",
            textContent: "",
            children: [],
            childNodes: [],
            firstChild: null, lastChild: null,
            parentNode: null,
            getAttribute() { return null; },
            setAttribute() {},
            hasAttribute() { return false; },
            appendChild(_c) {},
            removeChild(_c) {},
        });
        const _docEl = _srcdoc ? _mkHtmlMirror("html", _srcdoc) : null;
        const _body = _srcdoc ? _mkHtmlMirror("body", _srcdoc) : null;
        const _head = _srcdoc ? _mkHtmlMirror("head", "") : null;
        const iframeDoc = {
            documentElement: _docEl,
            head: _head,
            body: _body,
            title: "",
            readyState: "complete",
            visibilityState: "visible",
            hidden: false,
            hasFocus() { return false; },
            querySelector() { return null; },
            querySelectorAll() { return new NodeList([]); },
            getElementById() { return null; },
            getElementsByTagName(tag) {
                const t = String(tag).toLowerCase();
                if (_srcdoc && t === "html" && _docEl) return new NodeList([_docEl]);
                if (_srcdoc && t === "body" && _body) return new NodeList([_body]);
                if (_srcdoc && t === "head" && _head) return new NodeList([_head]);
                return new NodeList([]);
            },
            createElement(tag) { return _document.createElement(tag); },
            createElementNS(ns, tag) { return _document.createElementNS(ns, tag); },
            createEvent(type) { return _document.createEvent(type); },
            createRange() { return _document.createRange(); },
            createTextNode(text) { return _document.createTextNode(text); },
            write(html) { return _document.write(html); },
            writeln(html) { return _document.writeln(html); },
            open() { return _document.open(); },
            close() { return _document.close(); },
        };

        // ── Screen mirror ─────────────────────────────────────────────────
        const _parentScreen = globalThis.screen || {};
        const _iframeScreen = {
            availWidth:  _parentScreen.availWidth  || 1920,
            availHeight: _parentScreen.availHeight || 1080,
            width:       _parentScreen.width       || 1920,
            height:      _parentScreen.height      || 1080,
            availLeft:   _parentScreen.availLeft   || 0,
            availTop:    _parentScreen.availTop    || 0,
            colorDepth:  _parentScreen.colorDepth  || 24,
            pixelDepth:  _parentScreen.pixelDepth  || 24,
            orientation: _parentScreen.orientation,
        };
        if (!/Firefox\/|Gecko\/20100101/.test(
            (typeof navigator !== "undefined" && navigator.userAgent) || ""
        )) {
            _iframeScreen.isExtended = false;
        }

        // ── Obtain the child window object ───────────────────────────────
        // PRIMARY PATH: genuine v8::Context child realm (doc 26/27 §4).
        // op_create_child_realm returns the child global:
        //   - Real, realm-distinct native intrinsics (Object/Function/… ≠ parent)
        //   - constructor.name === "Window" (set up in Rust)
        //   - Genuine-native Function.prototype.toString in child realm
        //   - self/window/globalThis/frames self-refs (set in Rust)
        // Defeats Kasada's `addContentWindowProxy` detector + realm-divergence bail.
        const _realmId = _nextRealmId++;
        let cw = null;
        try {
            const _got = ops.op_create_child_realm(_realmId);
            if (_got && typeof _got === "object") cw = _got;
        } catch (_) {}

        if (cw) {
            // ── Populate child realm with DOM/FP properties ───────────────
            // CRITICAL: use op_set_child_realm_prop for properties that must be
            // visible to code running INSIDE the child realm (e.g. Kasada's srcdoc
            // eval). Direct `cw.x = v` from parent JS goes to the global PROXY's
            // own dict; code inside the realm reads from the INNER global.
            // op_set_child_realm_prop enters the child ContextScope and calls
            // child_global.set() which forwards via [[Set]] to the inner global.
            const _sp = (k, v) => {
                try { ops.op_set_child_realm_prop(_realmId, k, v); } catch (_) {}
            };

            // iframeDoc back-reference to default view (set before _sp calls)
            try { iframeDoc.defaultView = cw; } catch (_) {}

            // Document
            _sp("document", iframeDoc);

            // Location stub — about:blank inherits the parent origin per HTML spec.
            // Kasada's loc probe reads document.domain (= hostname) and
            // lli probe reads location.origin; empty values are bot signals.
            const _pLoc = globalThis.location || {};
            _sp("location", {
                href: "about:blank",
                origin: _pLoc.origin || "null",
                pathname: "/",
                hash: "", search: "",
                host: _pLoc.host || "",
                hostname: _pLoc.hostname || "",
                port: _pLoc.port || "",
                protocol: _pLoc.protocol || "https:",
                assign() {}, replace() {}, reload() {},
                toString() { return "about:blank"; },
            });

            // Parent / top / name
            _sp("parent", globalThis);
            _sp("top", globalThis);
            _sp("name", "");

            // Screen mirror (Kasada spd probe reads these from inside child realm)
            _sp("screen", _iframeScreen);
            _sp("availWidth",  _iframeScreen.availWidth);
            _sp("availHeight", _iframeScreen.availHeight);

            // Viewport dimensions (Kasada spd probe)
            _sp("innerWidth",   globalThis.innerWidth  || 1920);
            _sp("innerHeight",  globalThis.innerHeight || 1080);
            _sp("outerWidth",   globalThis.outerWidth  || 1920);
            _sp("outerHeight",  globalThis.outerHeight || 1080);
            _sp("scrollX", 0); _sp("scrollY", 0);
            _sp("pageXOffset", 0); _sp("pageYOffset", 0);
            // Window state properties expected by Kasada `bas` probe.
            _sp("closed", false);
            _sp("name", "");
            _sp("status", "");
            _sp("defaultStatus", "");
            _sp("screenTop", globalThis.screenTop || 0);
            _sp("screenLeft", globalThis.screenLeft || 0);
            _sp("screenX", globalThis.screenX || 0);
            _sp("screenY", globalThis.screenY || 0);
            // history stub — basic object so `.toString()` doesn't throw.
            _sp("history", { length: 0, state: null, scrollRestoration: "auto",
                back() {}, forward() {}, go() {}, pushState() {}, replaceState() {} });
            // Storage stubs — Kasada `bas` probe may call `.toString()` on these.
            const _storageStub = Object.create(null);
            Object.defineProperty(_storageStub, Symbol.toStringTag, { value: "Storage", configurable: true });
            _storageStub.length = 0;
            _storageStub.getItem = function getItem() { return null; };
            _storageStub.setItem = function setItem() {};
            _storageStub.removeItem = function removeItem() {};
            _storageStub.clear = function clear() {};
            _storageStub.key = function key() { return null; };
            try { _sp("localStorage", _storageStub); } catch (_) {}
            try { _sp("sessionStorage", _storageStub); } catch (_) {}
            // indexedDB — basic stub so typeof is "object".
            _sp("indexedDB", { open() {}, deleteDatabase() {}, databases() { return Promise.resolve([]); }, cmp() { return 0; } });
            // visualViewport — propagate from parent (Kasada `bas` probe may call .toString()).
            try { if (globalThis.visualViewport !== undefined) _sp("visualViewport", globalThis.visualViewport); } catch (_) {}

            // Event handler stubs — Chrome defines all on* handlers as null (data property,
            // enumerable:true) on the Window global. The child realm gets genuine V8 natives
            // but NOT these Window interface additions. Kasada `bas` probe iterates parent
            // window's enumerable properties and for each key checks it in the child realm;
            // calling .toString() on the undefined value throws, while null.toString()
            // would throw too but with the correct Chrome-matching TypeError shape.
            // Setting them null here makes child[key] !== undefined for all on* keys.
            const _onHandlers = [
                'onabort','onafterprint','onanimationcancel','onanimationend',
                'onanimationiteration','onanimationstart','onappinstalled','onauxclick',
                'onbeforeinput','onbeforeinstallprompt','onbeforematch','onbeforeprint',
                'onbeforetoggle','onbeforeunload','onbeforexrselect','onblur',
                'oncancel','oncanplay','oncanplaythrough','onchange',
                'onclick','onclose','oncommand','oncontentvisibilityautostatechange',
                'oncontextlost','oncontextmenu','oncontextrestored','oncuechange',
                'ondblclick','ondrag','ondragend','ondragenter',
                'ondragleave','ondragover','ondragstart','ondrop',
                'ondurationchange','onemptied','onended','onfocus',
                'onformdata','ongamepadconnected','ongamepaddisconnected','ongotpointercapture',
                'onhashchange','oninput','oninvalid','onkeydown',
                'onkeypress','onkeyup','onlanguagechange','onload',
                'onloadeddata','onloadedmetadata','onloadstart','onlostpointercapture',
                'onmessage','onmessageerror','onmousedown','onmouseenter',
                'onmouseleave','onmousemove','onmouseout','onmouseover',
                'onmouseup','onmousewheel','onoffline','ononline',
                'onpagehide','onpagereveal','onpageshow','onpageswap',
                'onpause','onplay','onplaying','onpointercancel',
                'onpointerdown','onpointerenter','onpointerleave','onpointermove',
                'onpointerout','onpointerover','onpointerrawupdate','onpointerup','onpopstate',
                'onprogress','onratechange','onrejectionhandled','onreset',
                'onresize','onscroll','onscrollend','onscrollsnapchange',
                'onscrollsnapchanging','onsearch','onsecuritypolicyviolation','onseeked',
                'onseeking','onselect','onselectionchange','onselectstart',
                'onslotchange','onstalled','onstorage','onsubmit',
                'onsuspend','ontimeupdate','ontoggle','ontransitioncancel',
                'ontransitionend','ontransitionrun','ontransitionstart','onunhandledrejection',
                'onunload','onvolumechange','onwaiting','onwebkitanimationend',
                'onwebkitanimationiteration','onwebkitanimationstart','onwebkittransitionend','onwheel',
            ];
            for (const _oh of _onHandlers) {
                try { _sp(_oh, null); } catch (_) {}
            }

            // Blanket-copy ALL remaining enumerable parent-window properties to child
            // realm. Kasada `bas` probe iterates parent window's enumerable props and
            // checks them in child; any that are undefined in child cause errors.
            // Real Chrome child frames have the same complete set as parent.
            // We skip child-specific properties (document, location, self-refs) that
            // are already set above or will be overridden below with correct values.
            const _basSkip = new Set([
                'window','self','globalThis','frames','top','parent',
                'document','location','opener',
                'length',
                // Carefully configured below (accessor or child-specific value):
                'devicePixelRatio','navigator','fetch','postMessage',
                // Already set above:
                'screen','availWidth','availHeight','innerWidth','innerHeight',
                'outerWidth','outerHeight','scrollX','scrollY','pageXOffset','pageYOffset',
                'screenTop','screenLeft','screenX','screenY',
                'closed','name','status','defaultStatus',
                'history','localStorage','sessionStorage','indexedDB','visualViewport',
            ]);
            try {
                for (const _bk of Object.keys(globalThis)) {
                    if (_basSkip.has(_bk)) continue;
                    // Skip numeric frame indices (not enumerable in real Chrome iframes)
                    if (_bk.length <= 4 && /^\d+$/.test(_bk)) continue;
                    try {
                        const _bv = globalThis[_bk];
                        _sp(_bk, _bv !== undefined ? _bv : null);
                    } catch (_) {}
                }
            } catch (_) {}

            // devicePixelRatio: define as a native-tagged accessor so that
            // Kasada's dpi probe sees both a proper descriptor (getter:fn, not
            // data) AND [native code] from Function.prototype.toString.
            // The eval runs inside the child realm so Symbol.for resolves via
            // the isolate-level global symbol registry (same symbol as parent).
            const _dprVal = globalThis.devicePixelRatio || 1;
            try {
                ops.op_eval_in_child_realm(_realmId,
                    `(function(){var _nt=Symbol.for('__boxide_native__');var _g=function(){return ${_dprVal};};Object.defineProperty(_g,_nt,{value:'get devicePixelRatio',configurable:true});Object.defineProperty(_g,'name',{value:'get devicePixelRatio',configurable:true});var _s=function(v){Object.defineProperty(this,'devicePixelRatio',{value:v,writable:true,enumerable:true,configurable:true});};Object.defineProperty(_s,_nt,{value:'set devicePixelRatio',configurable:true});Object.defineProperty(_s,'name',{value:'set devicePixelRatio',configurable:true});Object.defineProperty(globalThis,'devicePixelRatio',{get:_g,set:_s,enumerable:true,configurable:true});})();`
                );
            } catch (_) {
                _sp("devicePixelRatio", _dprVal);
            }

            // postMessage stub
            const _pm = function postMessage(msg, origin) {
                Promise.resolve().then(() => {
                    globalThis.dispatchEvent(new MessageEvent("message", { data: msg, origin: origin || "" }));
                });
            };
            _sp("postMessage", _pm);

            // Navigator: fresh instance proxying parent values.
            try {
                const _parentNav = globalThis.navigator;
                const _nav = Object.create(Object.prototype);
                for (const _k of [
                    "userAgent", "platform", "language", "languages",
                    "hardwareConcurrency", "deviceMemory", "maxTouchPoints",
                    "vendor", "vendorSub", "product", "productSub",
                    "appName", "appVersion", "appCodeName", "cookieEnabled",
                    "onLine", "doNotTrack", "pdfViewerEnabled",
                    "plugins", "mimeTypes",
                ]) {
                    try {
                        const _v = _parentNav[_k];
                        if (_v !== undefined) Object.defineProperty(_nav, _k, { value: _v, writable: true, configurable: true, enumerable: true });
                    } catch (_) {}
                }
                // webdriver: undefined in non-automated Chrome (property present, value undefined).
                // Kasada ifw probe checks i_nwd = cw.navigator.webdriver (falsy is fine).
                Object.defineProperty(_nav, 'webdriver', { value: undefined, writable: true, configurable: true, enumerable: true });
                _sp("navigator", _nav);
            } catch (_) {}

            // Own realm `fetch` — distinct reference (cw.fetch !== parent.fetch)
            try {
                const _ifetch = function fetch(...a) { return globalThis.fetch.apply(this, a); };
                Object.defineProperty(_ifetch, "name", { value: "fetch", configurable: true });
                Object.defineProperty(_ifetch, "length", { value: 1, configurable: true });
                Object.defineProperty(_ifetch, _NATIVE_TAG_SYMBOL, { value: "fetch", configurable: true });
                _sp("fetch", _ifetch);
            } catch (_) {}

            // Copy key browser APIs that Kasada reads from the child realm.
            // mrs probe reads MediaSource.isTypeSupported from inside child realm.
            const _apisToCopy = [
                'MediaSource', 'MediaSourceHandle', 'MediaCapabilities',
                'MediaRecorder', 'MediaStream', 'MediaStreamTrack',
                'HTMLVideoElement', 'HTMLAudioElement', 'HTMLMediaElement',
                'AudioContext', 'OfflineAudioContext',
                'RTCPeerConnection', 'RTCDataChannel',
                'Blob', 'File', 'FileReader',
                'URL', 'URLSearchParams',
                'WebSocket', 'Worker',
                'CSS', 'crypto', 'performance',
                'structuredClone', 'queueMicrotask', 'reportError',
                'crossOriginIsolated', 'isSecureContext', 'origin',
                'CustomEvent', 'Event', 'EventTarget',
                'PromiseRejectionEvent', 'ErrorEvent',
                'MessageChannel', 'MessagePort', 'MessageEvent',
                'MutationObserver', 'IntersectionObserver', 'ResizeObserver',
                'PerformanceObserver',
                'TextEncoder', 'TextDecoder',
                'AbortController', 'AbortSignal',
                'ReadableStream', 'WritableStream', 'TransformStream',
                'Request', 'Response', 'Headers', 'FormData',
                'XMLHttpRequest', 'DOMParser',
                'Node', 'Element', 'Document',
                'HTMLElement', 'DocumentFragment',
                'Notification',
            ];
            for (const _ak of _apisToCopy) {
                try {
                    const _v = globalThis[_ak];
                    if (_v !== undefined) _sp(_ak, _v);
                } catch (_) {}
            }

            // mrs/smc/dpv probes: Kasada reads MediaSource.isTypeSupported from inside
            // child realm. Wrap in IIFE to prevent __kms leaking into child realm globals
            // (Kasada's ltk probe detects unexpected global variables).
            // globalThis.X = Y inside an IIFE IS visible to subsequent op_eval_in_child_realm
            // calls because they all run in the same child v8::Context.
            try {
                ops.op_eval_in_child_realm(_realmId,
                    '(function(){\n' +
                    'var __kms=new Set(["video/mp4","video/webm","audio/mp4","audio/webm",' +
                    '"audio/mpeg","audio/aac","audio/x-m4a","audio/mp3","audio/x-wav",' +
                    '"audio/ogg","audio/acc","audio/mp4;codecs=\\"mp4a.40.2\\"",' +
                    '"video/mp4;codecs=\\"avc1.42E01E,mp4a.40.2\\"",' +
                    '"video/webm;codecs=\\"vp9\\""]);\n' +
                    'var _its=function isTypeSupported(t){if(typeof t!=="string")return false;var b=t.split(";")[0].trim();return __kms.has(t)||__kms.has(b);};\n' +
                    'if(typeof MediaSource==="undefined"||MediaSource===undefined){\n' +
                    'globalThis.MediaSource=function MediaSource(){throw new TypeError("Failed to construct \'MediaSource\': Illegal constructor");};\n' +
                    '}\n' +
                    'if(typeof MediaSource.isTypeSupported!=="function") MediaSource.isTypeSupported=_its;\n' +
                    'if(typeof MediaRecorder==="undefined"||MediaRecorder===undefined){\n' +
                    'globalThis.MediaRecorder=function MediaRecorder(){throw new TypeError("Failed to construct \'MediaRecorder\': Illegal constructor");};\n' +
                    '}\n' +
                    'if(typeof MediaRecorder.isTypeSupported!=="function") MediaRecorder.isTypeSupported=_its;\n' +
                    '})();\n'
                );
            } catch (_) {}

            // Align child realm globals with main window to prevent realm-divergence detection.
            // Chrome without COOP/COEP: SharedArrayBuffer is disabled in all frames.
            // Our V8 child context natively has SAB; delete it to match.
            try {
                ops.op_eval_in_child_realm(_realmId,
                    'if(typeof SharedArrayBuffer!=="undefined"&&typeof globalThis.SharedArrayBuffer!=="undefined")' +
                    '{try{delete globalThis.SharedArrayBuffer;}catch(_){globalThis.SharedArrayBuffer=undefined;}}'
                );
            } catch (_) {}

            // Execute srcdoc scripts in the child realm.
            // Kasada probes (ifw, spd) inject script content via srcdoc to
            // run code inside the iframe. A real browser executes those
            // scripts; we extract and eval them in the child realm context.
            if (_srcdoc) {
                try {
                    const _scriptRe = /<script[^>]*>([\s\S]*?)<\/script>/gi;
                    let _m;
                    while ((_m = _scriptRe.exec(_srcdoc)) !== null) {
                        const _src = _m[1];
                        if (_src && _src.trim()) {
                            try { ops.op_eval_in_child_realm(_realmId, _src); } catch (_) {}
                        }
                    }
                } catch (_) {}
            }

            state = { contentWindow: cw, contentDocument: iframeDoc, _realmId: _realmId, _processedSrcdoc: _srcdoc };
            _iframeState.set(el, state);
            return cw;
        }

        // ── FALLBACK: Proxy-based approach (if op unavailable) ───────────
        // Keeps existing behaviour when op_create_child_realm is not accessible
        // (e.g. worker runtime that doesn't load dom_extension).
        const remoteRealm = _buildRemoteRealm();
        const iframeLocals = {
            document: iframeDoc,
            location: { href: "about:blank" },
            parent: globalThis,
            top: globalThis,
            self: null,
            frames: [],
            screen: _iframeScreen,
            innerWidth:  globalThis.innerWidth  || 1920,
            innerHeight: globalThis.innerHeight || 1080,
            outerWidth:  globalThis.outerWidth  || 1920,
            outerHeight: globalThis.outerHeight || 1080,
            scrollX: 0, scrollY: 0, pageXOffset: 0, pageYOffset: 0,
            postMessage(msg, origin) {
                Promise.resolve().then(() => {
                    globalThis.dispatchEvent(new MessageEvent("message", { data: msg, origin: origin || "" }));
                });
            },
        };
        try {
            if (remoteRealm.Window && remoteRealm.Window.prototype) {
                Object.setPrototypeOf(iframeLocals, remoteRealm.Window.prototype);
            }
        } catch (_) {}
        try {
            const _ifetch = function fetch(...a) { return globalThis.fetch.apply(this, a); };
            Object.defineProperty(_ifetch, "name", { value: "fetch", configurable: true });
            Object.defineProperty(_ifetch, "length", { value: 1, configurable: true });
            Object.defineProperty(_ifetch, _NATIVE_TAG_SYMBOL, { value: "fetch", configurable: true });
            iframeLocals.fetch = _ifetch;
        } catch (_) {}
        try {
            const _dg = function () { return globalThis.devicePixelRatio || 1; };
            const _ds = function(v) {
                Object.defineProperty(iframeLocals, "devicePixelRatio", {
                    value: v, writable: true, enumerable: true, configurable: true,
                });
            };
            Object.defineProperty(_dg, _NATIVE_TAG_SYMBOL, { value: "get devicePixelRatio", configurable: true });
            Object.defineProperty(_dg, "name", { value: "get devicePixelRatio", configurable: true });
            Object.defineProperty(_ds, _NATIVE_TAG_SYMBOL, { value: "set devicePixelRatio", configurable: true });
            Object.defineProperty(_ds, "name", { value: "set devicePixelRatio", configurable: true });
            Object.defineProperty(iframeLocals, "devicePixelRatio", {
                get: _dg, set: _ds, enumerable: true, configurable: true,
            });
        } catch (_) {}
        const iframeWindow = new Proxy(iframeLocals, {
            get(target, prop) {
                if (prop in target) return target[prop];
                if (typeof prop === "string" && prop in remoteRealm) return remoteRealm[prop];
                try { return globalThis[prop]; } catch { return undefined; }
            },
            has(target, prop) {
                return prop in target || prop in remoteRealm || prop in globalThis;
            },
            getOwnPropertyDescriptor(target, prop) {
                if (prop in target) {
                    return Object.getOwnPropertyDescriptor(target, prop);
                }
                if (typeof prop === "string" && prop in remoteRealm) {
                    return { value: remoteRealm[prop], writable: true, enumerable: true, configurable: true };
                }
                return undefined;
            },
        });
        iframeLocals.self = iframeWindow;
        iframeLocals.window = iframeWindow;
        iframeLocals.globalThis = iframeWindow;
        iframeLocals.frames = iframeWindow;
        iframeLocals.length = 0;
        state = { contentWindow: iframeWindow, contentDocument: iframeDoc };
        _iframeState.set(el, state);
        return iframeWindow;
    }
    function _getIframeDocument(el) {
        _getIframeWindow(el); // ensure state is built
        return _iframeState.get(el).contentDocument;
    }

    // Install on HTMLIFrameElement.prototype — covers parsed AND created iframes.
    if (typeof HTMLIFrameElement !== 'undefined') {
        Object.defineProperty(HTMLIFrameElement.prototype, 'contentWindow', {
            get: function() { return _getIframeWindow(this); },
            configurable: true,
            enumerable: true,
        });
        Object.defineProperty(HTMLIFrameElement.prototype, 'contentDocument', {
            get: function() { return _getIframeDocument(this); },
            configurable: true,
            enumerable: true,
        });
        // srcdoc setter: when Kasada sets iframe.srcdoc = "..." BEFORE the first
        // contentWindow access, the value lands on the element's own property dict
        // (no setter exists, so JS creates an own data property). Our fallback in
        // _getIframeWindow reads el.srcdoc if getAttribute("srcdoc") is empty.
        //
        // When srcdoc is set AFTER the first contentWindow access (child realm
        // already cached), this setter fires immediately and re-executes the scripts.
        const _srcdocValues = new WeakMap();
        Object.defineProperty(HTMLIFrameElement.prototype, 'srcdoc', {
            get: function() { return _srcdocValues.get(this) || this.getAttribute('srcdoc') || ''; },
            set: function(v) {
                _srcdocValues.set(this, String(v));
                const _st = _iframeState.get(this);
                if (_st && _st._realmId !== undefined && v && String(v) !== _st._processedSrcdoc) {
                    _st._processedSrcdoc = String(v);
                    try {
                        const _re = /<script[^>]*>([\s\S]*?)<\/script>/gi;
                        let _m3;
                        while ((_m3 = _re.exec(String(v))) !== null) {
                            const _s3 = _m3[1];
                            if (_s3 && _s3.trim()) {
                                try { ops.op_eval_in_child_realm(_st._realmId, _s3); } catch (_) {}
                            }
                        }
                    } catch (_) {}
                }
            },
            configurable: true,
            enumerable: true,
        });
    }

    // Keep the createElement customElements-upgrade hook — still needed for
    // user-defined custom elements.
    const _origCreateElement = Document.prototype.createElement;
    Document.prototype.createElement = function(tag) {
        const el = _origCreateElement.call(this, tag);
        const ceEntry = globalThis._customElementsRegistry && globalThis._customElementsRegistry.get(tag.toLowerCase());
        if (ceEntry) {
            Object.setPrototypeOf(el, ceEntry.constructor.prototype);
            try { ceEntry.constructor.call(el); } catch (e) { console.error(e); }
            el._ceUpgraded = true;
        }
        return el;
    };

    // ================================================================
    // Native-code mask sweep for every JS-defined Web API method.
    //
    // Without this, Function.prototype.toString called on attachShadow,
    // queueMicrotask, Document.createElement, etc. returns the literal
    // JS source — including our deno_core op names like
    // `op_dom_attach_shadow`. Real Chrome returns
    // `function NAME() { [native code] }`. Kasada's `sfc` and `sdt`
    // probes (decrypted blob 0, 2026-05-10) catch us on this.
    //
    // Strategy: walk every named own property of every Web API
    // prototype we define, find any function-typed values + getters +
    // setters, and apply _maskFunction. Idempotent — re-masking a
    // tagged function is a no-op.
    if (typeof globalThis._maskFunction === 'function') {
        const _mask = globalThis._maskFunction;
        const _walkProto = (ctor, ctorName) => {
            if (!ctor) return;
            try { _mask(ctor, ctorName); } catch (_) {}
            const proto = ctor.prototype;
            if (!proto) return;
            for (const key of Object.getOwnPropertyNames(proto)) {
                if (key === 'constructor') continue;
                const desc = Object.getOwnPropertyDescriptor(proto, key);
                if (!desc) continue;
                try {
                    if (typeof desc.value === 'function') _mask(desc.value, key);
                    if (typeof desc.get === 'function') _mask(desc.get, `get ${key}`);
                    if (typeof desc.set === 'function') _mask(desc.set, `set ${key}`);
                } catch (_) {}
            }
        };
        // Every JS-defined Web API class in this bootstrap, plus
        // siblings from window_bootstrap, fetch_bootstrap,
        // canvas_bootstrap, etc. Listed by name so the sweep is
        // conservative — only masks what we've verified exists.
        const _toMask = [
            'EventTarget', 'Node', 'Element', 'HTMLElement',
            'Document', 'HTMLDocument', 'DocumentFragment',
            'ShadowRoot', 'Text', 'Comment', 'Attr',
            'NodeList', 'HTMLCollection', 'NamedNodeMap',
            'DOMTokenList', 'CSSStyleDeclaration',
            // Window-bootstrap-defined classes that previously leaked
            // their JS source via Function.prototype.toString.
            'Bluetooth', 'StorageManager', 'SharedWorker',
            'WorkerGlobalScope', 'NetworkInformation', 'MediaDevices',
            'ServiceWorkerContainer', 'Permissions', 'PermissionStatus',
            'Notification', 'Clipboard', 'CredentialsContainer',
            'PresentationConnection', 'XRSystem', 'GPUAdapter',
            // Canvas/Audio
            'AudioContext', 'BaseAudioContext', 'OfflineAudioContext',
            'AudioWorkletNode', 'OscillatorNode', 'GainNode',
            'AnalyserNode', 'BiquadFilterNode', 'DynamicsCompressorNode',
            // Workers
            'Worker', 'BroadcastChannel', 'MessageChannel', 'MessagePort',
            // Media
            'MediaRecorder', 'MediaSource', 'MediaSession',
            // HTML element subclasses (mostly empty markers, but their
            // class source still leaks via toString without masking).
            'HTMLDivElement', 'HTMLSpanElement', 'HTMLParagraphElement',
            'HTMLAnchorElement', 'HTMLImageElement', 'HTMLCanvasElement',
            'HTMLScriptElement', 'HTMLStyleElement', 'HTMLLinkElement',
            'HTMLMetaElement', 'HTMLTableElement', 'HTMLIFrameElement',
            'HTMLBodyElement', 'HTMLHtmlElement', 'HTMLHeadElement',
            'HTMLInputElement', 'HTMLButtonElement', 'HTMLSelectElement',
            'HTMLTextAreaElement', 'HTMLFormElement', 'HTMLLabelElement',
            'HTMLOptionElement', 'HTMLUListElement', 'HTMLOListElement',
            'HTMLLIElement', 'HTMLHeadingElement', 'HTMLHRElement',
            'HTMLBRElement', 'HTMLPreElement', 'HTMLBlockquoteElement',
            'HTMLVideoElement', 'HTMLAudioElement', 'HTMLMediaElement',
            'HTMLSourceElement', 'HTMLTrackElement', 'HTMLPictureElement',
            'HTMLTemplateElement', 'HTMLSlotElement', 'HTMLDialogElement',
            'HTMLDetailsElement', 'HTMLProgressElement', 'HTMLMeterElement',
        ];
        for (const name of _toMask) {
            const ctor = globalThis[name];
            if (typeof ctor === 'function') _walkProto(ctor, name);
        }

        // Top-level globalThis function-typed members that should be
        // native. queueMicrotask + fetch were the worst offenders in
        // the captured Kasada error report — both leaked their literal
        // JS source via Function.prototype.toString.
        const _topLevelFns = [
            'queueMicrotask', 'fetch', 'setTimeout', 'clearTimeout',
            'setInterval', 'clearInterval', 'requestAnimationFrame',
            'cancelAnimationFrame', 'requestIdleCallback', 'cancelIdleCallback',
            'structuredClone', 'reportError',
            'getComputedStyle', 'matchMedia', 'scroll', 'scrollTo', 'scrollBy',
            'alert', 'confirm', 'prompt', 'open', 'close', 'focus', 'blur',
            'postMessage', 'addEventListener', 'removeEventListener',
            'dispatchEvent',
        ];
        for (const name of _topLevelFns) {
            const fn = globalThis[name];
            if (typeof fn === 'function') {
                try { _mask(fn, name); } catch (_) {}
            }
        }
    }

    // Minimal window stub
    globalThis.window = globalThis;
    globalThis.self = globalThis;

    // Expose node-id resolution to sibling bootstrap files that need it
    // (event_bootstrap.js wires listeners by nodeId, not by Node identity).
    // Installed non-enumerable; cleanup_bootstrap.js deletes __boxide
    // before page scripts run. Callers must CAPTURE the helper during
    // their own bootstrap execution, not look it up per-call.
    Object.defineProperty(globalThis, '__boxide', {
        value: { _getNodeId },
        enumerable: false,
        configurable: true,
        writable: false,
    });
})(globalThis);
