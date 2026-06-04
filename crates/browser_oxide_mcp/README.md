# browser-oxide-mcp

**An MCP (Model Context Protocol) server that gives an AI agent a stealth
headless browser.** Backed by `browser_oxide` — a real HTML/CSS/DOM/JS engine
with its own BoringSSL TLS stack and a native fingerprint (no Chromium, no CDP).

## Tools

| Tool | Does |
|---|---|
| `fetch_page` | Render a URL (real browser engine) → text/html + the honest challenge verdict |
| `evaluate` | Render a URL, then run JavaScript in the page and return the result |
| `check_protection` | Render a URL and report **"is this behind Akamai/DataDome/Kasada, and did a real render get through?"** (verdict + is_challenge + bytes) |

`check_protection` is the differentiator: a real-but-stealth engine tells the
agent whether a page is bot-walled — something a plain HTTP fetch can't.

## Install & wire up

```bash
cargo install --git https://github.com/yfedoseev/browser_oxide browser_oxide_mcp
```

```json
{ "mcpServers": { "browser-oxide": { "command": "browser-oxide-mcp" } } }
```

Speaks JSON-RPC 2.0 over stdio (`initialize` / `tools/list` / `tools/call`).
Each tool takes `url` and an optional `profile` (`chrome` | `firefox` | `iphone`
| `pixel`). MIT OR Apache-2.0.
