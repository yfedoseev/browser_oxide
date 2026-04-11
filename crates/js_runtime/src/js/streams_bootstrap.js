// WHATWG Streams — ReadableStream / WritableStream / TransformStream.
//
// Scoped to the shape and one-read-cycle behaviour fingerprint-sensitive
// sites probe:
//   • `typeof ReadableStream === 'function'` — exists
//   • `new ReadableStream({...}).getReader().read()` — returns chunks
//   • `response.body.getReader()` — wired up in fetch_bootstrap
//   • `tee()` returns two independently-readable branches
//   • `pipeTo` / `pipeThrough` — flow through a writable sink
//
// NOT implemented: byte streams (`ReadableStreamBYOBReader`), HWM-based
// backpressure, strict state machine transitions, custom queuing
// strategies. A probe that exercises those would see slightly off
// behaviour; one that just does read()/close()/tee() sees Chrome-like
// output.

((globalThis) => {
    // Avoid double-install on re-run of bootstraps.
    if (
        globalThis.ReadableStream &&
        globalThis.ReadableStream.prototype &&
        globalThis.ReadableStream.prototype._browserOxideReal
    ) {
        return;
    }

    // -----------------------------------------------------------------
    // ReadableStream
    // -----------------------------------------------------------------

    class ReadableStreamDefaultController {
        constructor(stream) {
            this._stream = stream;
        }
        get desiredSize() {
            // Unbounded queue — always "room for more".
            return 1;
        }
        enqueue(chunk) {
            if (this._stream._state !== "readable") {
                throw new TypeError(
                    "ReadableStreamDefaultController.enqueue called on " +
                        this._stream._state +
                        " stream"
                );
            }
            this._stream._queue.push(chunk);
            this._stream._drain();
        }
        close() {
            if (this._stream._state !== "readable") return;
            this._stream._state = "closed";
            this._stream._drain();
        }
        error(reason) {
            if (this._stream._state !== "readable") return;
            this._stream._state = "errored";
            this._stream._error = reason;
            this._stream._drain();
        }
    }

    class ReadableStreamDefaultReader {
        constructor(stream) {
            if (stream._locked) {
                throw new TypeError(
                    "ReadableStream is locked to another reader"
                );
            }
            stream._locked = true;
            this._stream = stream;
            // `_pending` is a FIFO of resolvers waiting for read()s
            // that arrived before a chunk was ready.
            this._pending = [];
            this._closedResolve = null;
            this._closedReject = null;
            this.closed = new Promise((resolve, reject) => {
                this._closedResolve = resolve;
                this._closedReject = reject;
            });
            // If the stream is already closed/errored when the reader
            // attaches, settle `closed` immediately so awaiters unblock.
            if (stream._state === "closed") {
                this._closedResolve();
            } else if (stream._state === "errored") {
                this._closedReject(stream._error);
            }
        }
        read() {
            if (!this._stream) {
                return Promise.reject(
                    new TypeError("reader released")
                );
            }
            const stream = this._stream;
            if (stream._state === "errored") {
                return Promise.reject(stream._error);
            }
            if (stream._queue.length > 0) {
                // Kick the pull machinery to refill — noop for
                // one-shot streams, useful for pull-based ones.
                queueMicrotask(() => stream._pull());
                return Promise.resolve({
                    value: stream._queue.shift(),
                    done: false,
                });
            }
            if (stream._state === "closed") {
                return Promise.resolve({ value: undefined, done: true });
            }
            // Queue is empty but stream is still readable — wait for
            // the next enqueue / close.
            return new Promise((resolve, reject) => {
                this._pending.push({ resolve, reject });
                queueMicrotask(() => stream._pull());
            });
        }
        cancel(reason) {
            if (!this._stream) return Promise.resolve();
            const stream = this._stream;
            stream._state = "closed";
            stream._queue.length = 0;
            if (typeof stream._underlyingSource?.cancel === "function") {
                try {
                    const r = stream._underlyingSource.cancel(reason);
                    return Promise.resolve(r);
                } catch (e) {
                    return Promise.reject(e);
                }
            }
            this._drain();
            return Promise.resolve();
        }
        releaseLock() {
            if (!this._stream) return;
            this._stream._locked = false;
            this._stream = null;
        }
        _drain() {
            // Called by the stream when state changes — resolve any
            // pending reads that can now be answered.
            if (!this._stream) return;
            const stream = this._stream;
            while (this._pending.length > 0) {
                if (stream._queue.length > 0) {
                    const p = this._pending.shift();
                    p.resolve({ value: stream._queue.shift(), done: false });
                } else if (stream._state === "closed") {
                    const p = this._pending.shift();
                    p.resolve({ value: undefined, done: true });
                } else if (stream._state === "errored") {
                    const p = this._pending.shift();
                    p.reject(stream._error);
                } else {
                    break;
                }
            }
            // Settle `closed` once the stream terminates.
            if (stream._state === "closed" && this._closedResolve) {
                this._closedResolve();
                this._closedResolve = null;
                this._closedReject = null;
            } else if (stream._state === "errored" && this._closedReject) {
                this._closedReject(stream._error);
                this._closedResolve = null;
                this._closedReject = null;
            }
        }
    }

    class ReadableStream {
        constructor(underlyingSource, _strategy) {
            this._underlyingSource = underlyingSource || {};
            this._queue = [];
            this._state = "readable";
            this._error = null;
            this._locked = false;
            this._reader = null;
            this._pullInFlight = false;
            this._started = false;
            // Kick the `start` callback on a microtask so it sees a
            // fully-constructed controller + stream object.
            queueMicrotask(() => this._start());
        }
        // Marker so future bootstrap re-runs don't replace the real
        // implementation with a stub.
        get _browserOxideReal() {
            return true;
        }
        get locked() {
            return this._locked;
        }
        getReader(_options) {
            const reader = new ReadableStreamDefaultReader(this);
            this._reader = reader;
            // Immediate drain in case the stream was already closed.
            reader._drain();
            return reader;
        }
        cancel(reason) {
            if (this._state === "closed") return Promise.resolve();
            this._state = "closed";
            this._queue.length = 0;
            if (this._reader) this._reader._drain();
            if (typeof this._underlyingSource.cancel === "function") {
                try {
                    const r = this._underlyingSource.cancel(reason);
                    return Promise.resolve(r);
                } catch (e) {
                    return Promise.reject(e);
                }
            }
            return Promise.resolve();
        }
        tee() {
            // Spec: two independent ReadableStream branches that each
            // receive every chunk the source produces. We implement
            // this push-style: `pump()` reads from the source and
            // enqueues each chunk onto both branches' controllers.
            //
            // Controllers are captured via `start(c)` at construction,
            // and `pump()` only runs AFTER both branches' start
            // callbacks have fired — we queueMicrotask the pump so
            // the controller assignments happen first.
            const source = this;
            let b1Controller = null;
            let b2Controller = null;
            const sourceReader = source.getReader();

            const pump = () => {
                sourceReader.read().then(
                    ({ done, value }) => {
                        if (done) {
                            if (b1Controller) try { b1Controller.close(); } catch (_) {}
                            if (b2Controller) try { b2Controller.close(); } catch (_) {}
                            return;
                        }
                        if (b1Controller) try { b1Controller.enqueue(value); } catch (_) {}
                        if (b2Controller) try { b2Controller.enqueue(value); } catch (_) {}
                        pump();
                    },
                    (err) => {
                        if (b1Controller) try { b1Controller.error(err); } catch (_) {}
                        if (b2Controller) try { b2Controller.error(err); } catch (_) {}
                    }
                );
            };

            const b1s = new ReadableStream({
                start(c) { b1Controller = c; },
                cancel() { sourceReader.cancel(); },
            });
            const b2s = new ReadableStream({
                start(c) { b2Controller = c; },
                cancel() { sourceReader.cancel(); },
            });
            // Wait for both branches' `start` callbacks (queued in
            // their constructors) to run before pumping.
            queueMicrotask(() => queueMicrotask(pump));
            return [b1s, b2s];
        }
        pipeTo(destination, _options) {
            if (!(destination instanceof WritableStream)) {
                return Promise.reject(
                    new TypeError("pipeTo requires a WritableStream")
                );
            }
            const reader = this.getReader();
            const writer = destination.getWriter();
            return new Promise((resolve, reject) => {
                const step = () => {
                    reader.read().then(
                        ({ done, value }) => {
                            if (done) {
                                writer.close().then(resolve, reject);
                                return;
                            }
                            writer.write(value).then(step, reject);
                        },
                        (err) => {
                            writer.abort(err).then(
                                () => reject(err),
                                () => reject(err)
                            );
                        }
                    );
                };
                step();
            });
        }
        pipeThrough(transform, options) {
            if (!transform || !transform.readable || !transform.writable) {
                throw new TypeError("pipeThrough requires a TransformStream");
            }
            // Fire and forget — the caller consumes `transform.readable`.
            this.pipeTo(transform.writable, options).catch(() => {});
            return transform.readable;
        }
        [Symbol.asyncIterator]() {
            const reader = this.getReader();
            return {
                next() {
                    return reader.read();
                },
                return() {
                    reader.releaseLock();
                    return Promise.resolve({ value: undefined, done: true });
                },
                [Symbol.asyncIterator]() {
                    return this;
                },
            };
        }
        _start() {
            if (this._started) return;
            this._started = true;
            const controller = new ReadableStreamDefaultController(this);
            this._controller = controller;
            if (typeof this._underlyingSource.start === "function") {
                try {
                    const r = this._underlyingSource.start(controller);
                    if (r && typeof r.then === "function") {
                        r.catch((e) => controller.error(e));
                    }
                } catch (e) {
                    controller.error(e);
                    return;
                }
            }
            // Trigger an initial pull so pull-based sources produce
            // their first chunk even if read() hasn't been called yet.
            this._pull();
        }
        _pull() {
            if (this._pullInFlight) return;
            if (this._state !== "readable") return;
            if (typeof this._underlyingSource.pull !== "function") return;
            this._pullInFlight = true;
            try {
                const r = this._underlyingSource.pull(this._controller);
                Promise.resolve(r)
                    .then(() => {
                        this._pullInFlight = false;
                    })
                    .catch((e) => {
                        this._pullInFlight = false;
                        if (this._controller) this._controller.error(e);
                    });
            } catch (e) {
                this._pullInFlight = false;
                this._controller && this._controller.error(e);
            }
        }
        _drain() {
            if (this._reader) this._reader._drain();
        }
    }

    // Convenience: build a ReadableStream from a single Uint8Array chunk.
    // Used by Response.body when the full body is already in memory.
    ReadableStream.from = function fromIterable(iterable) {
        if (!iterable) {
            return new ReadableStream({
                start(c) {
                    c.close();
                },
            });
        }
        const iter =
            typeof iterable[Symbol.asyncIterator] === "function"
                ? iterable[Symbol.asyncIterator]()
                : typeof iterable[Symbol.iterator] === "function"
                ? iterable[Symbol.iterator]()
                : null;
        if (!iter) {
            return new ReadableStream({
                start(c) {
                    c.enqueue(iterable);
                    c.close();
                },
            });
        }
        return new ReadableStream({
            async pull(controller) {
                const step = await iter.next();
                if (step.done) controller.close();
                else controller.enqueue(step.value);
            },
        });
    };

    // -----------------------------------------------------------------
    // WritableStream
    // -----------------------------------------------------------------

    class WritableStreamDefaultWriter {
        constructor(stream) {
            if (stream._locked) {
                throw new TypeError(
                    "WritableStream is locked to another writer"
                );
            }
            stream._locked = true;
            this._stream = stream;
            this.ready = Promise.resolve();
            this.closed = new Promise((resolve, reject) => {
                stream._closedResolve = resolve;
                stream._closedReject = reject;
            });
        }
        get desiredSize() {
            return 1;
        }
        write(chunk) {
            if (!this._stream) return Promise.reject(new TypeError("released"));
            if (this._stream._state !== "writable") {
                return Promise.reject(
                    new TypeError(
                        "write on " + this._stream._state + " stream"
                    )
                );
            }
            const sink = this._stream._underlyingSink;
            if (typeof sink.write === "function") {
                try {
                    const r = sink.write(chunk, this._stream._controller);
                    return Promise.resolve(r);
                } catch (e) {
                    return Promise.reject(e);
                }
            }
            return Promise.resolve();
        }
        close() {
            if (!this._stream) return Promise.reject(new TypeError("released"));
            const stream = this._stream;
            if (stream._state !== "writable") {
                return Promise.reject(
                    new TypeError("close on " + stream._state + " stream")
                );
            }
            stream._state = "closed";
            const sink = stream._underlyingSink;
            const result =
                typeof sink.close === "function"
                    ? Promise.resolve(sink.close())
                    : Promise.resolve();
            return result.then(() => {
                stream._closedResolve && stream._closedResolve();
            });
        }
        abort(reason) {
            if (!this._stream) return Promise.resolve();
            const stream = this._stream;
            stream._state = "errored";
            const sink = stream._underlyingSink;
            const result =
                typeof sink.abort === "function"
                    ? Promise.resolve(sink.abort(reason))
                    : Promise.resolve();
            return result.then(() => {
                stream._closedReject && stream._closedReject(reason);
            });
        }
        releaseLock() {
            if (!this._stream) return;
            this._stream._locked = false;
            this._stream = null;
        }
    }

    class WritableStreamDefaultController {
        constructor(stream) {
            this._stream = stream;
        }
        error(reason) {
            this._stream._state = "errored";
            this._stream._error = reason;
        }
    }

    class WritableStream {
        constructor(underlyingSink, _strategy) {
            this._underlyingSink = underlyingSink || {};
            this._state = "writable";
            this._error = null;
            this._locked = false;
            this._controller = new WritableStreamDefaultController(this);
            this._closedResolve = null;
            this._closedReject = null;
            if (typeof this._underlyingSink.start === "function") {
                try {
                    this._underlyingSink.start(this._controller);
                } catch (e) {
                    this._state = "errored";
                    this._error = e;
                }
            }
        }
        get _browserOxideReal() {
            return true;
        }
        get locked() {
            return this._locked;
        }
        getWriter() {
            return new WritableStreamDefaultWriter(this);
        }
        abort(reason) {
            this._state = "errored";
            this._error = reason;
            const sink = this._underlyingSink;
            return typeof sink.abort === "function"
                ? Promise.resolve(sink.abort(reason))
                : Promise.resolve();
        }
        close() {
            if (this._state !== "writable") return Promise.resolve();
            this._state = "closed";
            const sink = this._underlyingSink;
            return typeof sink.close === "function"
                ? Promise.resolve(sink.close())
                : Promise.resolve();
        }
    }

    // -----------------------------------------------------------------
    // TransformStream
    // -----------------------------------------------------------------

    class TransformStream {
        constructor(transformer, _writableStrategy, _readableStrategy) {
            transformer = transformer || {};
            let readableController = null;
            let readableResolved;
            this.readable = new ReadableStream({
                start(c) {
                    readableController = c;
                },
            });
            // Make sure `readable._pull` runs so `start` assigns the
            // controller before any write() comes in.
            // `writable.write(chunk)` calls `transformer.transform(chunk,
            // controller)` which is free to call `controller.enqueue` on
            // the readable side.
            this.writable = new WritableStream({
                async write(chunk) {
                    if (typeof transformer.transform === "function") {
                        await transformer.transform(chunk, readableController);
                    } else {
                        readableController &&
                            readableController.enqueue(chunk);
                    }
                },
                close() {
                    if (typeof transformer.flush === "function") {
                        try {
                            transformer.flush(readableController);
                        } catch (_e) {}
                    }
                    readableController && readableController.close();
                },
                abort(e) {
                    readableController && readableController.error(e);
                },
            });
            if (typeof transformer.start === "function") {
                try {
                    transformer.start(readableController);
                } catch (_e) {}
            }
        }
        get _browserOxideReal() {
            return true;
        }
    }

    // -----------------------------------------------------------------
    // Install globals — overwrite the earlier stubs.
    // -----------------------------------------------------------------
    globalThis.ReadableStream = ReadableStream;
    globalThis.ReadableStreamDefaultReader = ReadableStreamDefaultReader;
    globalThis.ReadableStreamDefaultController = ReadableStreamDefaultController;
    globalThis.WritableStream = WritableStream;
    globalThis.WritableStreamDefaultWriter = WritableStreamDefaultWriter;
    globalThis.WritableStreamDefaultController = WritableStreamDefaultController;
    globalThis.TransformStream = TransformStream;

    // Response.body integration lives in fetch_bootstrap.js — it's
    // defined as a getter on the Response class because the private
    // fields there aren't reachable from an external monkey-patch.
    // By the time any Response's body getter fires, ReadableStream is
    // installed via this script (which runs before any user JS).
})(globalThis);
