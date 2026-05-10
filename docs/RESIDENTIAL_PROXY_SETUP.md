# Residential proxy setup

`browser_oxide` can route every TCP connection through an HTTP CONNECT or
SOCKS5 proxy, preserving the upstream Chrome 147 TLS fingerprint to the
*origin*. This is intended for sites that gate on IP reputation
(Cloudflare, DataDome, some Akamai deployments, footwear retailers) where
direct datacenter egress is rate-limited or blocked outright.

## Quick start

Three ways to configure, listed in priority order (highest first):

1. **`BOXIDE_PROXY` environment variable** — overrides per-page settings.
   Useful for one-off CI runs or scripted bulk testing without rebuilding.

   ```bash
   export BOXIDE_PROXY='http://user:pass@proxy.example.com:8080'
   # or:
   export BOXIDE_PROXY='socks5://user:pass@residential.example.com:1080'
   cargo run --release -- ...
   ```

2. **`StealthProfile.proxy`** — per-page proxy. Each `Page` is constructed
   from a `StealthProfile`, so giving each page its own profile gives each
   page its own egress IP. This is the right place for production code:

   ```rust
   use stealth::presets;
   let mut profile = presets::chrome_130_windows();
   profile.proxy = Some("http://user:pass@proxy.example.com:8080".into());
   let page = Page::new(profile, ...).await?;
   ```

3. **No proxy** — direct connect via Happy Eyeballs (RFC 6555).

## Supported URL schemes

| Scheme           | Notes                                                      |
| ---------------- | ---------------------------------------------------------- |
| `http://`        | HTTP/1.1 `CONNECT host:port` tunnel + optional Basic auth. |
| `https://`       | Currently treated as `http://` with a warning.<br>TLS-in-TLS hop is not implemented; the vast majority of residential providers don't require it. |
| `socks5://`      | RFC 1928 + RFC 1929 USER/PASS auth. ATYP=DOMAINNAME so the proxy resolves DNS (correct for IP-rotation flows). |
| `socks5h://`     | Alias for `socks5://` (we always send DOMAINNAME). |

Username and password may be percent-encoded (`%3A` for `:`, `%40` for
`@`, etc).

## How the TLS fingerprint is preserved

The CONNECT tunnel — both HTTP and SOCKS5 — establishes a *byte-clean*
pipe to the origin TCP endpoint. The BoringSSL handshake (Chrome 147 JA4
fingerprint, custom cipher order, GREASE values, ECH config, cert
compression) then runs on that pipe. The remote sees:

- TLS ClientHello: identical to direct connect (Chrome 147 fingerprint).
- ALPN negotiation: identical.
- HTTP/2 SETTINGS + HEADERS frame ordering: identical.
- HTTP/3 (QUIC): **does not work through proxy** — QUIC is UDP and the
  proxy is TCP. The HTTP/3 path is auto-disabled when a proxy is active
  (effectively, because Alt-Svc cache misses fall back to TCP+TLS).
  See `crates/net/src/quic.rs`.

Only the **proxy hop itself** (the `CONNECT` exchange or SOCKS5
handshake) uses our internal HTTP/1.1 client without TLS impersonation.
That handshake is invisible to the origin.

## Per-page proxies

Each `Page` carries its own `StealthProfile`, which carries its own
proxy URL. To run N pages through N different exits, build N profiles
each with a different proxy URL. The `HttpClient` is built fresh per
profile and resolves the proxy field at construction time (`HttpClient::new`).

Connection pooling is per-`HttpClient`, so connections are *not* shared
between pages — which is what you want, otherwise a request from page A
could go out the page-B exit IP.

## Verifying the setup

The hermetic round-trip tests (always run):

```bash
cargo test -p net --test proxy_roundtrip -- --test-threads=1
```

These spin up local in-process HTTP CONNECT and origin servers and
verify the tunnel transports bytes correctly + Basic auth is sent
correctly + 407 produces a structured error.

The live test (requires a real proxy):

```bash
export BOXIDE_TEST_PROXY='http://user:pass@your-proxy:8080'
cargo test --release -p net --test proxy_roundtrip live_proxy_chain \
    -- --ignored --test-threads=1 --nocapture
```

Hits `https://api.ipify.org/?format=text` once direct, once through
the proxy, asserts both succeed and the IPs differ. If they're equal,
the proxy is misconfigured (or transparent — some "anonymous" proxies
forward `X-Forwarded-For` and pass the original client IP straight
through).

You can also test against any `mitmproxy --mode regular` instance for
local development:

```bash
mitmproxy --mode regular --listen-port 8080
BOXIDE_PROXY=http://127.0.0.1:8080 cargo run --release -- ...
```

`mitmproxy` won't preserve the upstream TLS fingerprint (it MITMs the
TLS), so that's only useful for verifying the CONNECT path itself, not
for stealth testing.

## Tested providers

**TODO** — none yet validated end-to-end against gated sites. When a
provider has been confirmed working, fill in:

| Provider              | Scheme  | Sticky session | Notes |
| --------------------- | ------- | -------------- | ----- |
| _(none yet)_          |         |                |       |

Operator-supplied benchmarks are welcome — drop them in this table
along with the date tested, the gated site bypassed, and the JA4
fingerprint that the site received (the latter to confirm we're still
sending Chrome 147, not the proxy's substitute).

## Known limitations

1. **No HTTP/3 through proxy.** QUIC is UDP; proxies are TCP. The
   client transparently falls back to HTTP/2 over the tunnel. Sites
   that gate on "Chrome should support h3" will see the same H2-only
   behaviour as a Chrome instance with QUIC disabled — minor tell.
2. **No TLS-in-TLS for `https://` proxies.** A small fraction of
   providers (some commercial sticky-session offerings) require the hop
   to the proxy itself be TLS-wrapped before the inner CONNECT. Our
   `https://` URL form prints a warning and falls through to plain
   CONNECT. If you hit a provider that requires this, file an issue —
   the BoringSSL stack is already imported, it's a wrapping step in
   `proxy::connect`.
3. **No proxy chaining.** Single hop only.
4. **No automatic retries / proxy-list rotation.** If the proxy hop
   fails, the request fails. Higher-level rotation logic lives in the
   caller (e.g. round-robin profile selection in `crates/browser/src/pool.rs`).
5. **DNS leakage on HTTP CONNECT.** With HTTP CONNECT, the *proxy*
   resolves DNS (we send `CONNECT host:port`). With SOCKS5 we send
   ATYP=DOMAINNAME so the proxy still resolves. Either way, no DNS
   query goes out the local interface for a proxied request — but the
   proxy operator necessarily learns the host you're visiting.
6. **No UDP / WebRTC.** We don't implement a UDP relay, so any feature
   that needs UDP (currently only HTTP/3) bypasses the proxy. Real
   Chrome with a proxy configured exhibits the same behaviour.

## Implementation reference

- `crates/net/src/proxy.rs` — URL parsing, HTTP CONNECT, SOCKS5 handshake.
- `crates/net/src/tcp.rs` — `connect_via_proxy` dispatcher; falls
  through to `connect_with_cache` when no proxy is configured.
- `crates/net/src/lib.rs` — `HttpClient` resolves the proxy at
  construction; every TCP connect site in the file uses
  `connect_via_proxy`.
- `crates/stealth/src/profile.rs` — `StealthProfile.proxy: Option<String>`.
- `crates/net/tests/proxy_roundtrip.rs` — hermetic + live tests.
