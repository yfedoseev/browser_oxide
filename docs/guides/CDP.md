# Guide: CDP server (Puppeteer / Playwright drop-in)

`browser_oxide` exposes a **Chrome DevTools Protocol** surface, so existing
Puppeteer/Playwright clients can drive it over a WebSocket — no Chrome process,
no CDP detection vectors, the stealth engine underneath.

## Start a server

```rust
use protocol::CdpServer;

// empty page, ready for the client's Page.navigate:
let server = CdpServer::start_navigable(9222)?;
println!("connect to ws://127.0.0.1:{}", server.port());

// or seed it with a URL / HTML:
let server = CdpServer::start_with_url("https://example.com", 9222)?;
let server = CdpServer::start("<h1>hi</h1>", 9222)?;
let server = CdpServer::start_ephemeral("<h1>hi</h1>")?;   // OS-assigned port
```

The server runs on its own thread with its own runtime; `port()` returns the bound
port (useful with `start_ephemeral`'s OS-assigned port).

## Connect from Puppeteer

```js
const puppeteer = require('puppeteer-core');
const browser = await puppeteer.connect({
  browserWSEndpoint: 'ws://127.0.0.1:9222',
});
const page = await browser.newPage();
await page.goto('https://example.com');
console.log(await page.title());
```

Playwright connects the same way via `chromium.connectOverCDP('ws://127.0.0.1:9222')`.

## Supported surface

Core `Runtime.*` / `Page.*` commands (evaluate, navigate, content). It is a
*subset* aimed at scraping/automation, not the full CDP — check
[../PROTOCOL.md](../PROTOCOL.md) for the exact command coverage before porting a
complex script.

## When to use which API

- **In-process Rust** (`Page::navigate`) — lowest overhead, full access to
  `challenge_verdict()`, solvers, profiles. Prefer this for new Rust code.
- **CDP server** — when you have an existing Puppeteer/Playwright codebase you
  want to point at the stealth engine with minimal changes.
- **Python / MCP** — planned (see the distribution roadmap); will wrap the
  in-process API behind a dedicated engine thread.
