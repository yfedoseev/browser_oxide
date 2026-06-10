# net — HTTP/1.1 + HTTP/2 + HTTP/3 + Stealth TLS + WebSocket

The network layer. Handles all HTTP communication with stealth TLS/HTTP fingerprinting across all protocol versions.

## Network Stack

### HTTP/1.1 + HTTP/2: rquest + BoringSSL

| Property | Value |
|---|---|
| Crate | `rquest` (Apache-2.0) |
| TLS backend | BoringSSL (same as Chrome) |
| TLS impersonation | 100+ browser profiles (JA3/JA4 match) |
| HTTP/2 fingerprint | SETTINGS, WINDOW_UPDATE, PRIORITY frames match target browser |
| Header order | Preserves original case and order |

### HTTP/3 + QUIC: quinn + h3

| Property | Value |
|---|---|
| QUIC | `quinn` (MIT/Apache-2.0) — pure Rust, production-grade, 86M+ downloads |
| HTTP/3 | `h3` (MIT) + `h3-quinn` — HTTP/3 client generic over QUIC transport |
| QUIC fingerprint | Transport parameters (initial max data, max streams, idle timeout) configurable per profile |

### Why HTTP/3 Matters

- Chrome defaults to QUIC/HTTP/3 when available (2026)
- **QUIC fingerprinting is real**: JA4 covers QUIC connections. Cloudflare uses JA4 for QUIC
- Not supporting HTTP/3 is itself a signal — "this client doesn't even try HTTP/3"
- Transport parameters (initial window, max streams) are fingerprinted like HTTP/2 SETTINGS

### WebSocket Client: tokio-tungstenite

| Property | Value |
|---|---|
| Crate | `tokio-tungstenite` (MIT) |
| Purpose | WebSocket API for JS (SPAs, real-time apps, anti-bot telemetry) |
| TLS | Uses same BoringSSL backend for wss:// connections |

## Architecture

```
net/
├── src/
│   ├── lib.rs              # HttpClient — unified API across HTTP/1+2+3
│   ├── client.rs           # rquest wrapper + HTTP/3 fallback
│   ├── http3.rs            # quinn + h3 HTTP/3 client
│   ├── profile.rs          # Browser TLS/HTTP2/HTTP3 profiles
│   ├── headers.rs          # Browser-matching header templates (order, case, values)
│   ├── cookies.rs          # RFC 6265 cookie jar (cookie_store)
│   ├── redirect.rs         # Redirect policy (follow, max hops, same-origin)
│   ├── interceptor.rs      # Request/response interceptor trait
│   ├── websocket.rs        # WebSocket client (tokio-tungstenite)
│   ├── proxy.rs            # SOCKS5, HTTP CONNECT, HTTPS proxy support
│   └── error.rs
├── tests/
│   ├── tls_fingerprint.rs  # Verify JA4 hash matches target browser
│   ├── http2_fingerprint.rs # Verify SETTINGS/PRIORITY frames
│   ├── http3_tests.rs      # QUIC connection + HTTP/3 requests
│   ├── websocket_tests.rs
│   └── cookie_tests.rs
└── Cargo.toml
```

## Protocol Negotiation

Real Chrome negotiates protocols:

```
1. DNS lookup (may include HTTPS record with ALPN hint)
2. If QUIC/HTTP3 previously succeeded for this origin:
   → Try QUIC connection (0-RTT if possible)
   → Fall back to TCP if QUIC fails
3. Else:
   → TCP + TLS handshake
   → ALPN negotiates h2 (HTTP/2) or http/1.1
   → HTTP/2 SETTINGS exchange
4. Alt-Svc header may advertise HTTP/3 for future requests
```

BrowserOxide replicates this behavior, including Alt-Svc caching.

## TLS Fingerprint Matching (JA4)

JA4 is the 2025-2026 standard (replaced JA3). It fingerprints:

- TLS version
- Cipher suites (sorted alphabetically, defeating Chrome's randomization)
- Extensions (sorted)
- Signature algorithms
- ALPN values
- SNI presence

rquest handles this via BoringSSL browser profiles. Each `StealthProfile` maps to a specific rquest `Impersonate` variant.

## HTTP/2 Frame Fingerprint

Beyond TLS, anti-bot systems fingerprint the HTTP/2 connection setup:

| Frame | Chrome 130 Value | Notes |
|---|---|---|
| SETTINGS HEADER_TABLE_SIZE | 65536 | |
| SETTINGS MAX_CONCURRENT_STREAMS | 1000 | |
| SETTINGS INITIAL_WINDOW_SIZE | 6291456 | |
| SETTINGS MAX_HEADER_LIST_SIZE | 262144 | |
| WINDOW_UPDATE increment | 15663105 | Connection-level |
| PRIORITY frames | Specific tree structure | Chrome's priority scheme |
| Pseudo-header order | `:method :authority :scheme :path` | Different from other clients |

rquest emulates all of these per browser profile.

## Header Templates

Real Chrome 130 headers (exact order matters):

```http
:method: GET
:authority: example.com
:scheme: https
:path: /
sec-ch-ua: "Chromium";v="130", "Google Chrome";v="130", "Not?A_Brand";v="99"
sec-ch-ua-mobile: ?0
sec-ch-ua-platform: "Windows"
upgrade-insecure-requests: 1
user-agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ...
accept: text/html,application/xhtml+xml,...
sec-fetch-site: none
sec-fetch-mode: navigate
sec-fetch-user: ?1
sec-fetch-dest: document
accept-encoding: gzip, deflate, br, zstd
accept-language: en-US,en;q=0.9
```

### User-Agent Client Hints

Chrome's UA reduction means `sec-ch-ua-*` headers are now the primary browser identification mechanism. Server requests hints via `Accept-CH`, browser responds:

```http
sec-ch-ua: "Chromium";v="130", "Google Chrome";v="130"
sec-ch-ua-mobile: ?0
sec-ch-ua-platform: "Windows"
sec-ch-ua-platform-version: "10.0.0"
sec-ch-ua-arch: "x86"
sec-ch-ua-bitness: "64"
sec-ch-ua-model: ""
sec-ch-ua-full-version-list: "Chromium";v="130.0.6723.91", ...
```

## Integration Points

- **js_runtime**: `fetch()`, `XMLHttpRequest`, `WebSocket` call into this crate
- **html_parser**: Downloaded HTML fed to html5ever
- **browser**: Top-level navigation uses this for initial page load + subresources
- **workers**: Web Workers fetch scripts through this crate
- **stealth**: Browser profiles provided by stealth crate configure TLS/HTTP profiles
