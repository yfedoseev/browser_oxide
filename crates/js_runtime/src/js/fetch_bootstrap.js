((globalThis) => {
    const ops = Deno.core.ops;

    class Headers {
        #map;
        constructor(init = {}) {
            this.#map = {};
            if (init instanceof Headers) {
                init.forEach((v, k) => { this.#map[k] = v; });
            } else if (Array.isArray(init)) {
                for (const [k, v] of init) this.#map[k.toLowerCase()] = v;
            } else if (typeof init === "object") {
                for (const [k, v] of Object.entries(init)) this.#map[k.toLowerCase()] = String(v);
            }
        }
        get(name) { return this.#map[name.toLowerCase()] ?? null; }
        set(name, value) { this.#map[name.toLowerCase()] = String(value); }
        has(name) { return name.toLowerCase() in this.#map; }
        delete(name) { delete this.#map[name.toLowerCase()]; }
        forEach(cb) { for (const [k, v] of Object.entries(this.#map)) cb(v, k, this); }
        entries() { return Object.entries(this.#map)[Symbol.iterator](); }
        keys() { return Object.keys(this.#map)[Symbol.iterator](); }
        values() { return Object.values(this.#map)[Symbol.iterator](); }
        [Symbol.iterator]() { return this.entries(); }
    }

    class Response {
        #body; #rawBytes; #status; #statusText; #headers; #url; #ok; #bodyStream;
        constructor(body, init = {}) {
            this.#body = body ?? "";
            // Optional authoritative binary payload. When set,
            // arrayBuffer()/blob() hand back the exact bytes without
            // a TextEncoder round-trip — critical for blob: fetches
            // with non-UTF-8 content (e.g. PNG, WASM).
            this.#rawBytes = init._rawBytes ?? null;
            this.#status = init.status ?? 200;
            this.#statusText = init.statusText ?? "OK";
            this.#headers = new Headers(init.headers ?? {});
            this.#url = init.url ?? "";
            this.#ok = this.#status >= 200 && this.#status < 300;
            this.#bodyStream = null;
        }
        get status() { return this.#status; }
        get statusText() { return this.#statusText; }
        get ok() { return this.#ok; }
        get headers() { return this.#headers; }
        get url() { return this.#url; }
        // `body` is a ReadableStream per the Fetch spec. We build it
        // lazily so responses that never read `.body` don't pay for
        // stream construction. The stream yields the cached body as
        // one chunk then closes — enough to satisfy `reader.read()`
        // probes and async-iterator consumption.
        get body() {
            if (this.#bodyStream) return this.#bodyStream;
            if (typeof globalThis.ReadableStream !== "function") return null;
            const raw = this.#rawBytes;
            const textBody = this.#body;
            this.#bodyStream = new globalThis.ReadableStream({
                start(controller) {
                    let bytes = null;
                    if (raw instanceof Uint8Array) {
                        bytes = raw;
                    } else if (textBody != null && textBody !== "") {
                        bytes = new TextEncoder().encode(String(textBody));
                    }
                    if (bytes && bytes.byteLength > 0) controller.enqueue(bytes);
                    controller.close();
                },
            });
            return this.#bodyStream;
        }
        get bodyUsed() {
            return !!this.#bodyStream && this.#bodyStream.locked;
        }
        async text() { return this.#body; }
        async json() { return JSON.parse(this.#body); }
        async arrayBuffer() {
            if (this.#rawBytes && this.#rawBytes.buffer) {
                return this.#rawBytes.buffer.slice(
                    this.#rawBytes.byteOffset,
                    this.#rawBytes.byteOffset + this.#rawBytes.byteLength
                );
            }
            return new TextEncoder().encode(this.#body).buffer;
        }
        async blob() {
            if (this.#rawBytes) {
                const b = new Blob([]);
                b._data = this.#rawBytes;
                b.size = this.#rawBytes.byteLength;
                b.type = this.#headers.get('content-type') || '';
                return b;
            }
            return new Blob([this.#body]);
        }
        clone() {
            return new Response(this.#body, {
                _rawBytes: this.#rawBytes,
                status: this.#status,
                statusText: this.#statusText,
                headers: this.#headers,
                url: this.#url,
            });
        }
    }

    class Request {
        constructor(input, init = {}) {
            if (typeof input === "string") {
                this.url = input;
            } else if (input instanceof Request) {
                this.url = input.url;
                init = { method: input.method, headers: input.headers, body: input.body, ...init };
            }
            this.method = (init.method ?? "GET").toUpperCase();
            this.headers = new Headers(init.headers ?? {});
            this.body = init.body ?? null;
            this._signal = init.signal ?? null;
        }
        // Task#2: real Chrome's Request has a readonly `signal`
        // accessor ON Request.prototype (per the Fetch spec). Defined
        // as a class getter so `Object.hasOwnProperty.call(
        // Request.prototype,"signal")` is true — duolingo's
        // `supportsAbortController` capability gate requires exactly
        // that; without it the homepage self-redirects to
        // /errors/not-supported.html. Lazily backs an AbortSignal so
        // `request.signal` is a non-null AbortSignal like real Chrome.
        get signal() {
            if (this._signal == null
                && typeof globalThis.AbortController === "function") {
                try {
                    this._signal = new globalThis.AbortController().signal;
                } catch (_e) { /* leave null if AbortController throws */ }
            }
            return this._signal;
        }
    }

    // Pull the net::cookies jar snapshot for the current origin into
    // globalThis.__jsCookies so that document.cookie is always in sync.
    // Called after every fetch response and by page.rs after navigation.
    async function _syncCookiesFromNet(url) {
        try {
            url = url || globalThis.location?.href;
            if (!url || url === "about:blank") return;
            const cookieStr = await ops.op_cookie_get(url);
            if (!globalThis.__jsCookies) globalThis.__jsCookies = {};
            if (!cookieStr) return;
            for (const pair of cookieStr.split(";")) {
                const eq = pair.indexOf("=");
                if (eq < 0) continue;
                const k = pair.slice(0, eq).trim();
                const v = pair.slice(eq + 1).trim();
                if (k) globalThis.__jsCookies[k] = v;
            }
        } catch (e) { /* ignore */ }
    }
    globalThis.__syncCookiesFromNet = _syncCookiesFromNet;

    // Normalize any Headers/object/array init into a plain {lower-name: value} object
    function _flattenHeaders(h) {
        const out = {};
        if (!h) return out;
        if (h instanceof Headers) {
            h.forEach((v, k) => { out[k.toLowerCase()] = String(v); });
        } else if (Array.isArray(h)) {
            for (const [k, v] of h) out[String(k).toLowerCase()] = String(v);
        } else if (typeof h === "object") {
            for (const [k, v] of Object.entries(h)) out[String(k).toLowerCase()] = String(v);
        }
        return out;
    }

    // Real fetch() using Rust op
    globalThis.fetch = async function fetch(input, init = {}) {
        let url, method, body, headers;

        if (typeof input === "string") {
            url = input;
        } else if (input instanceof Request) {
            url = input.url;
            init = { method: input.method, headers: input.headers, body: input.body, ...init };
        } else {
            url = String(input);
        }

        // Resolve relative URLs against current location (like a real browser).
        // Note: URLs like "ftp:" (scheme with no authority) are treated by the
        // URL spec as a relative path component, so `new URL("ftp:", base)`
        // resolves to "<base_origin>/ftp:" — matching real Chrome exactly.
        if (url && !url.startsWith("http://") && !url.startsWith("https://") && !url.startsWith("data:") && !url.startsWith("blob:")) {
            try {
                const base = globalThis.location?.href || "about:blank";
                if (base && base !== "about:blank") {
                    url = new URL(url, base).href;
                }
            } catch (e) { /* keep original URL if resolution fails */ }
        }

        // Reject URLs that still can't be fetched after resolution.
        if (
            url &&
            !url.startsWith("http://") &&
            !url.startsWith("https://") &&
            !url.startsWith("data:") &&
            !url.startsWith("blob:")
        ) {
            throw new TypeError("Failed to fetch");
        }

        // blob: URLs short-circuit the HTTP client: look up the bytes
        // in the Rust BlobRegistry and synthesise a Response. Matches
        // Chrome's `fetch(URL.createObjectURL(blob))` shape — returns
        // 200 with the blob's bytes and content-type.
        if (url && url.startsWith("blob:")) {
            let resp;
            try {
                resp = ops.op_blob_fetch_bytes(url);
            } catch (e) {
                throw new TypeError("Failed to fetch: " + e.message);
            }
            if (!resp || !resp.found) {
                // Unknown blob URL — the spec says a network error.
                throw new TypeError("Failed to fetch: unknown blob URL");
            }
            // `resp.bytes` comes back from serde as an array of numbers;
            // coerce to Uint8Array so Response.arrayBuffer/blob hand
            // back exact bytes.
            const bytes = resp.bytes instanceof Uint8Array
                ? resp.bytes
                : new Uint8Array(resp.bytes);
            const contentType = resp.content_type || "application/octet-stream";
            // text() on the Response returns UTF-8 decoded bytes so
            // `fetch(blob:URL).text()` works for text blobs; binary
            // blobs should prefer `arrayBuffer()` / `blob()` which
            // use the raw byte path.
            const decoded = new TextDecoder("utf-8", { fatal: false }).decode(bytes);
            return new Response(decoded, {
                _rawBytes: bytes,
                status: 200,
                statusText: "OK",
                headers: { "content-type": contentType },
                url,
            });
        }

        method = (init.method ?? "GET").toUpperCase();
        // Body can be: string, ArrayBuffer, TypedArray (Uint8Array), Blob,
        // FormData, URLSearchParams, or null. We must preserve binary
        // fidelity for Kasada-style `application/octet-stream` POSTs.
        //
        // Rust op_fetch accepts the body as a marker-prefixed string:
        //   "s:<text>"   — plain UTF-8 string body
        //   "b:<base64>" — base64-encoded binary body
        const rawBody = init.body;
        // When the body is a FormData, the browser ALWAYS serializes it to
        // multipart/form-data with an auto-generated boundary and sets its
        // own Content-Type (a manually-set Content-Type is ignored for
        // FormData). Track the boundary so we can force the header below.
        let multipartBoundary = null;
        let urlencodedBody = false;
        const _isFormData =
            typeof FormData !== "undefined" && rawBody instanceof FormData;
        const _isUSP =
            typeof URLSearchParams !== "undefined" && rawBody instanceof URLSearchParams;
        if (rawBody == null) {
            body = "";
        } else if (typeof rawBody === "string") {
            body = "s:" + rawBody;
        } else if (rawBody instanceof ArrayBuffer || ArrayBuffer.isView(rawBody)) {
            // Convert typed array / ArrayBuffer → Uint8Array → base64.
            const u8 =
                rawBody instanceof Uint8Array
                    ? rawBody
                    : new Uint8Array(rawBody.buffer || rawBody, rawBody.byteOffset || 0, rawBody.byteLength);
            // btoa only handles latin-1; build a binary string first.
            let bin = "";
            for (let i = 0; i < u8.length; i++) bin += String.fromCharCode(u8[i]);
            body = "b:" + btoa(bin);
        } else if (_isFormData) {
            // FIX-FORMDATA (parity-workflows): serialize FormData → multipart
            // with a generated boundary. AWS-WAF's challenge.js POSTs its proof
            // as FormData; without this we sent the literal "[object FormData]"
            // and AWS rejected the POST with 400 "Invalid boundary for
            // multipart/form-data request" — blocking the amazon-TLD cluster.
            multipartBoundary =
                "----browserOxideFormBoundary" +
                Math.random().toString(36).slice(2) +
                Math.random().toString(36).slice(2);
            let mp = "";
            rawBody.forEach((value, name) => {
                mp += "--" + multipartBoundary + "\r\n";
                if (typeof Blob !== "undefined" && value instanceof Blob) {
                    const fn = value.name || "blob";
                    const ct = value.type || "application/octet-stream";
                    mp +=
                        'Content-Disposition: form-data; name="' + name +
                        '"; filename="' + fn + '"\r\n';
                    mp += "Content-Type: " + ct + "\r\n\r\n";
                    mp += String(value) + "\r\n";
                } else {
                    mp += 'Content-Disposition: form-data; name="' + name + '"\r\n\r\n';
                    mp += String(value) + "\r\n";
                }
            });
            mp += "--" + multipartBoundary + "--\r\n";
            body = "s:" + mp;
        } else if (_isUSP) {
            // URLSearchParams → application/x-www-form-urlencoded.
            body = "s:" + rawBody.toString();
            urlencodedBody = true;
        } else if (typeof Blob !== "undefined" && rawBody instanceof Blob) {
            // Blobs — best effort; we don't have a sync read, use toString.
            body = "s:" + String(rawBody);
        } else {
            body = "s:" + String(rawBody);
        }
        headers = _flattenHeaders(init.headers);

        // FIX-FORMDATA: a FormData body's Content-Type is browser-controlled —
        // FORCE our generated boundary, overriding any (boundaryless)
        // Content-Type the page set, exactly as Chrome does.
        if (multipartBoundary) {
            headers["content-type"] =
                "multipart/form-data; boundary=" + multipartBoundary;
        } else if (urlencodedBody && !headers["content-type"]) {
            headers["content-type"] = "application/x-www-form-urlencoded;charset=UTF-8";
        }

        // Auto-set Content-Type for POSTs with a body (mirrors Chrome fetch default)
        if (body && !headers["content-type"] && (method === "POST" || method === "PUT" || method === "PATCH")) {
            headers["content-type"] = "text/plain;charset=UTF-8";
        }

        // Pass the page's origin as a pseudo header so the net layer can
        // compute sec-fetch-site (same-origin vs cross-site) and set Origin /
        // Referer correctly. Chrome's fetch API always carries these.
        try {
            const loc = globalThis.location;
            if (loc && loc.origin && loc.origin !== "null") {
                headers["x-browser-oxide-origin"] = loc.origin;
            } else if (loc && loc.href && loc.href !== "about:blank") {
                const u = new URL(loc.href);
                headers["x-browser-oxide-origin"] = u.origin;
            }
        } catch {}

        try {
            const startTime = performance.now();
            const result = await ops.op_fetch(url, method, headers, body);
            
            const browser_oxide = globalThis._browser_oxide;
            const fetchLog = browser_oxide && browser_oxide.__fetchLog;
            if (fetchLog) {
                fetchLog.push({ method, url, status: result.status });
            }

            const entries = browser_oxide && browser_oxide.__perfResourceEntries;
            if (entries) {
                entries.push({ url, type: "fetch", startTime, duration: performance.now() - startTime, size: result.body ? result.body.length : 0 });
            }

            // Sync cookies from the net jar into document.cookie so subsequent JS
            // reads (including the WBAAS challenge polling loop) see Set-Cookie
            // values that arrived via this response.
            await _syncCookiesFromNet(url);
            return new Response(result.body, {
                status: result.status,
                statusText: result.status_text,
                headers: result.headers,
                url: result.url,
            });
        } catch (e) {
            // Log error for audit
            const browser_oxide = globalThis._browser_oxide;
            const fetchLog = browser_oxide && browser_oxide.__fetchLog;
            if (fetchLog) {
                fetchLog.push({ method, url, status: 0, error: e.message });
            }
            throw new TypeError("Failed to fetch: " + e.message);
        }
    };

    globalThis.Headers = Headers;
    globalThis.Response = Response;
    globalThis.Request = Request;

    // CSP violation drain — pulls violations the Rust gates queued and
    // dispatches `securitypolicyviolation` events for each on document
    // and window. Real Chrome dispatches the event synchronously at
    // the moment of block; we batch + drain because our gates run
    // off-OpState. Drain on a few timed checkpoints so listeners
    // installed early in page lifecycle catch the events.
    function _drainCspViolations() {
        try {
            const violations = ops.op_drain_csp_violations();
            if (!violations || !violations.length) return;
            for (const v of violations) {
                let ev;
                try {
                    ev = new SecurityPolicyViolationEvent("securitypolicyviolation", {
                        bubbles: true,
                        cancelable: false,
                        blockedURI: v.blockedURI,
                        violatedDirective: v.violatedDirective,
                        effectiveDirective: v.effectiveDirective,
                        disposition: v.disposition,
                    });
                } catch (_) {
                    // Fallback if SecurityPolicyViolationEvent is unavailable.
                    ev = new CustomEvent("securitypolicyviolation", {
                        bubbles: true, cancelable: false, detail: v,
                    });
                }
                try { if (globalThis.document) globalThis.document.dispatchEvent(ev); } catch (_) {}
                try { globalThis.dispatchEvent(ev); } catch (_) {}
                if (globalThis.console && typeof console.error === 'function') {
                    console.error(
                        "Refused to load '" + v.blockedURI +
                        "' because it violates the following Content Security Policy directive: \"" +
                        v.effectiveDirective + "\"."
                    );
                }
            }
        } catch (_) { /* best-effort */ }
    }

    Promise.resolve().then(_drainCspViolations);
    if (typeof setTimeout === "function") {
        setTimeout(_drainCspViolations, 0);
        setTimeout(_drainCspViolations, 50);
        setTimeout(_drainCspViolations, 250);
    }
    Object.defineProperty(globalThis, "__drainCspViolations", {
        value: _drainCspViolations, enumerable: false, configurable: true,
    });

    // Mask Function.prototype.toString so antibot probes (Kasada `sfc`
    // field) see `function fetch() { [native code] }` instead of our
    // literal source.
    if (typeof globalThis._maskFunction === 'function') {
        try { globalThis._maskFunction(globalThis.fetch, 'fetch'); } catch (_) {}
        if (globalThis.Request) try { globalThis._maskFunction(globalThis.Request, 'Request'); } catch (_) {}
        if (globalThis.Response) try { globalThis._maskFunction(globalThis.Response, 'Response'); } catch (_) {}
        if (globalThis.Headers) try { globalThis._maskFunction(globalThis.Headers, 'Headers'); } catch (_) {}
    }
})(globalThis);
