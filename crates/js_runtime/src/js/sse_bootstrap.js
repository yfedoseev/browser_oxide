// EventSource (Server-Sent Events) — W3C spec compliant
// https://html.spec.whatwg.org/multipage/server-sent-events.html
((globalThis) => {
  const CONNECTING = 0;
  const OPEN = 1;
  const CLOSED = 2;

  class EventSource {
    static CONNECTING = CONNECTING;
    static OPEN = OPEN;
    static CLOSED = CLOSED;

    constructor(url, options) {
      this.url = url;
      this.withCredentials = options?.withCredentials || false;
      this.readyState = CONNECTING;
      this._sseId = -1;
      this._listeners = {};

      // Event handlers (set by user)
      this.onopen = null;
      this.onmessage = null;
      this.onerror = null;

      // Start connection
      this._connect();
    }

    async _connect() {
      try {
        const result = await Deno.core.ops.op_sse_connect(this.url);
        if (!result.ok) {
          this.readyState = CLOSED;
          if (this.onerror) this.onerror(new Event('error'));
          this._dispatch('error', new Event('error'));
          return;
        }
        this._sseId = result.id;
        this.readyState = OPEN;
        if (this.onopen) this.onopen(new Event('open'));
        this._dispatch('open', new Event('open'));

        // Start reading events
        this._readLoop();
      } catch (e) {
        this.readyState = CLOSED;
        if (this.onerror) this.onerror(new Event('error'));
        this._dispatch('error', new Event('error'));
      }
    }

    async _readLoop() {
      while (this.readyState === OPEN && this._sseId >= 0) {
        try {
          const event = await Deno.core.ops.op_sse_recv(this._sseId);
          if (event.status === 'closed') {
            this.readyState = CLOSED;
            if (this.onerror) this.onerror(new Event('error'));
            this._dispatch('error', new Event('error'));
            break;
          }
          if (event.status === 'error') {
            if (this.onerror) this.onerror(new Event('error'));
            this._dispatch('error', new Event('error'));
            break;
          }

          // Create MessageEvent
          const msgEvent = {
            type: event.event || 'message',
            data: event.data,
            lastEventId: event.id,
            origin: this.url,
          };

          // Dispatch to specific event type listeners
          if (event.event && event.event !== 'message') {
            this._dispatch(event.event, msgEvent);
          }

          // Always dispatch to onmessage for 'message' events
          if (!event.event || event.event === 'message') {
            if (this.onmessage) this.onmessage(msgEvent);
            this._dispatch('message', msgEvent);
          }
        } catch (e) {
          this.readyState = CLOSED;
          break;
        }
      }
    }

    addEventListener(type, listener) {
      if (!this._listeners[type]) this._listeners[type] = [];
      this._listeners[type].push(listener);
    }

    removeEventListener(type, listener) {
      if (!this._listeners[type]) return;
      this._listeners[type] = this._listeners[type].filter(l => l !== listener);
    }

    _dispatch(type, event) {
      const listeners = this._listeners[type];
      if (listeners) {
        for (const listener of listeners) {
          try { listener(event); } catch (e) {}
        }
      }
    }

    close() {
      if (this._sseId >= 0) {
        Deno.core.ops.op_sse_close(this._sseId);
        this._sseId = -1;
      }
      this.readyState = CLOSED;
    }
  }

  globalThis.EventSource = EventSource;
})(globalThis);
