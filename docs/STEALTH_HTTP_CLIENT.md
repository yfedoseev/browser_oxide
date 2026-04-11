# Stealth HTTP Client — Requirements

browser_oxide needs its own HTTP client with Chrome-level TLS/HTTP2 fingerprint impersonation. The current dependency on wreq/rquest has proven unreliable (cert verification failures, IPv6 issues, opaque errors). Since we built a full browser from scratch, we should own the HTTP stack too.

## Why Build Our Own

| Problem with wreq/rquest | Our solution |
|--------------------------|-------------|
| BoringSSL cert verification fails on some sites (example.com) | Load webpki-root-certs into BoringSSL cert store ourselves |
| IPv6 connections fail silently | Use tokio TCP directly (proven to work) |
| Opaque errors (`client error (Connect)`) | Full control over error chain |
| Can't update Chrome fingerprint without waiting for upstream | Own the TLS config, update same day Chrome releases |
| Two competing forks (rquest vs wreq) with different bugs | One stable implementation we control |
| MIT/Apache-2.0 compliance concerns with transitive deps | Minimal deps, all permissive |

## Architecture

```
                    net::HttpClient (our public API — unchanged)
                           |
              +------------+------------+
              |                         |
        StealthTlsStream          H3 (QUIC)
     (boring2 + tokio TCP)       (existing quinn/h3)
              |
     +--------+--------+
     |                  |
  Http2Client       Http1Client
  (h2 crate)       (httparse)
     |                  |
     +--------+---------+
              |
      CookieJar + Compression
```

## Component Requirements

### 1. TCP Connection Layer

**Goal**: Connect to any host over IPv4 or IPv6 using tokio.

- Use `tokio::net::lookup_host()` for DNS (returns both IPv4 and IPv6)
- Use `tokio::net::TcpStream::connect()` for TCP
- Happy Eyeballs (RFC 6555): try IPv6 first, fall back to IPv4 after 250ms
- Connection timeout: configurable, default 10s
- TCP keepalive: configurable
- TCP nodelay: enabled by default

**Already proven to work** — our tests show tokio TCP connects to IPv6 `example.com` and IPv4 `httpbin.org` without issues.

### 2. TLS Layer (BoringSSL via boring2)

**Goal**: Establish TLS connections with Chrome-identical fingerprints.

**Dependencies**: `boring2`, `tokio-boring2` (direct BoringSSL bindings, permissive license)

#### Chrome 130 TLS Configuration

**Cipher Suites** (exact order matters for JA3):
```
TLS_AES_128_GCM_SHA256
TLS_AES_256_GCM_SHA384
TLS_CHACHA20_POLY1305_SHA256
TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA
TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA
TLS_RSA_WITH_AES_128_GCM_SHA256
TLS_RSA_WITH_AES_256_GCM_SHA384
TLS_RSA_WITH_AES_128_CBC_SHA
TLS_RSA_WITH_AES_256_CBC_SHA
```

**Elliptic Curves** (order matters for JA3):
```
X25519_KYBER768_DRAFT00  (post-quantum hybrid)
X25519
SECP256R1 (P-256)
SECP384R1 (P-384)
```

**Signature Algorithms**:
```
ecdsa_secp256r1_sha256
rsa_pss_rsae_sha256
rsa_pkcs1_sha256
ecdsa_secp384r1_sha384
rsa_pss_rsae_sha384
rsa_pkcs1_sha384
rsa_pss_rsae_sha512
rsa_pkcs1_sha512
```

**ALPN**: `["h2", "http/1.1"]`

**TLS Extensions** (must be present):
- GREASE: enabled (random GREASE values in ClientHello)
- OCSP stapling: enabled
- Signed Certificate Timestamps (SCT): enabled
- Certificate compression: Brotli
- Permute extensions: enabled (randomize extension order in ClientHello)
- Pre-shared key (PSK): enabled
- Session tickets: enabled
- ECH GREASE: disabled for Chrome 130
- ALPS: HTTP/2 application settings
- Min version: TLS 1.2
- Max version: TLS 1.3

**Certificate Verification**:
- Load `webpki-root-certs::TLS_SERVER_ROOT_CERTS` into BoringSSL's X509 store
- Verify server certificates against these roots
- Support intermediate CA chain validation
- Proper SNI (Server Name Indication) — strip `[]` brackets for IPv6 addresses

### 3. HTTP/2 Layer

**Goal**: Send HTTP/2 requests with Chrome-identical SETTINGS fingerprint.

**Dependency**: `h2` crate (pure Rust HTTP/2 implementation)

#### Chrome 130 HTTP/2 Configuration

**SETTINGS Frame** (order matters for Akamai fingerprint):
```
HEADER_TABLE_SIZE      = 65536      (64 KB)
ENABLE_PUSH            = 0          (disabled)
MAX_CONCURRENT_STREAMS = 1000
INITIAL_WINDOW_SIZE    = 6291456    (6 MB)
MAX_FRAME_SIZE         = 16384      (16 KB, default)
MAX_HEADER_LIST_SIZE   = 262144     (256 KB)
```

**SETTINGS Frame Order** (critical for HTTP/2 fingerprint):
1. HEADER_TABLE_SIZE (0x1)
2. ENABLE_PUSH (0x2)
3. MAX_CONCURRENT_STREAMS (0x3)
4. INITIAL_WINDOW_SIZE (0x4)
5. MAX_FRAME_SIZE (0x5)
6. MAX_HEADER_LIST_SIZE (0x6)

**Connection Window**: 15728640 bytes (15 MB) — sent as WINDOW_UPDATE after SETTINGS

**Pseudo-Header Order**: `:method`, `:authority`, `:scheme`, `:path`

**Header Priority**: Stream dependency 0, weight 255, exclusive true

### 4. HTTP/1.1 Fallback

**Goal**: Handle servers that don't support HTTP/2.

**Dependency**: `httparse` crate (zero-copy HTTP/1.1 parser)

- If ALPN negotiates `http/1.1`, fall back to HTTP/1.1
- Chunked transfer encoding support
- Keep-alive connection reuse
- Proper `Host` header generation

### 5. Cookie Management

**Goal**: Persist cookies across requests (required for auth flows and challenge solving).

**Options**: `cookie_store` crate or custom implementation

- Per-domain cookie storage
- Secure/HttpOnly flag handling
- Cookie expiration
- Path matching
- Send `Cookie` header automatically on matching requests
- Parse `Set-Cookie` response headers

### 6. Response Decompression

**Goal**: Decompress response bodies based on `Content-Encoding` header.

**Dependencies**: `flate2` (gzip/deflate), `brotli` (br), `zstd` (zstd)

- `gzip` / `deflate`: `flate2` crate
- `br`: `brotli` crate
- `zstd`: `zstd` crate
- Send `Accept-Encoding: gzip, deflate, br, zstd` header (Chrome order)
- Auto-detect from `Content-Encoding` response header

### 7. Header Ordering

**Goal**: Send headers in exact Chrome order (anti-bot systems check this).

Chrome 130 header order for navigation requests:
```
sec-ch-ua
sec-ch-ua-mobile
sec-ch-ua-platform
sec-ch-ua-platform-version
upgrade-insecure-requests
user-agent
accept
sec-fetch-site
sec-fetch-mode
sec-fetch-user
sec-fetch-dest
accept-encoding
accept-language
cookie (if present)
priority
```

Use `IndexMap` or ordered `Vec<(String, String)>` instead of `HashMap` for headers.

### 8. Connection Pooling

**Goal**: Reuse TLS+HTTP/2 connections for multiple requests to the same host.

- Pool keyed by `(host, port)`
- Max connections per host: configurable, default 6
- Idle timeout: configurable, default 90s
- Automatically multiplex HTTP/2 requests over a single connection

### 9. Redirect Following

**Goal**: Follow HTTP redirects while preserving cookies.

- Follow 301, 302, 303, 307, 308 redirects
- Max redirect depth: configurable, default 10
- Preserve cookies across redirects (critical for challenge solving)
- Handle relative URLs in `Location` header

## Public API

The public API (`net::HttpClient`) stays unchanged:

```rust
pub struct HttpClient { ... }

impl HttpClient {
    pub fn new(profile: &StealthProfile) -> Result<Self, NetError>;
    pub async fn get(&self, url: &str) -> Result<Response, NetError>;
    pub async fn get_follow(&self, url: &str, max_redirects: u8) -> Result<Response, NetError>;
    pub async fn post(&self, url: &str, body: &str) -> Result<Response, NetError>;
}

pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub url: String,
}
```

No API changes needed — only the internals change.

## File Structure

```
crates/net/src/
├── lib.rs              # HttpClient public API (exists, update internals)
├── error.rs            # Error types (exists, keep)
├── tls.rs              # NEW: BoringSSL TLS config + Chrome fingerprint
├── tcp.rs              # NEW: TCP connect with Happy Eyeballs (IPv4/IPv6)
├── h2_client.rs        # NEW: HTTP/2 client over TLS stream
├── h1_client.rs        # NEW: HTTP/1.1 fallback client
├── cookies.rs          # NEW: Cookie jar
├── compression.rs      # NEW: Response decompression
├── pool.rs             # NEW: Connection pooling
├── alt_svc.rs          # Exists: Alt-Svc cache for H3
├── h3_request.rs       # Exists: HTTP/3 (QUIC) client
└── quic.rs             # Exists: QUIC transport
```

## Dependencies

### Keep
- `tokio` — async runtime + TCP
- `url` — URL parsing
- `thiserror` — error handling
- `serde`, `serde_json` — serialization
- `quinn`, `h3`, `h3-quinn` — HTTP/3 (QUIC)
- `rustls`, `webpki-roots` — for QUIC TLS (separate from BoringSSL)
- `http`, `bytes` — HTTP types

### Add
- `boring2` — BoringSSL bindings (direct, not via wreq)
- `tokio-boring2` — async TLS streams
- `h2` — HTTP/2 framing
- `httparse` — HTTP/1.1 parsing
- `flate2` — gzip/deflate decompression
- `brotli` — Brotli decompression
- `zstd` — Zstd decompression
- `cookie_store` — cookie management (or custom)
- `webpki-root-certs` — CA certificate roots for BoringSSL
- `indexmap` — ordered header map

### Remove
- `wreq` — replaced entirely
- `wreq-util` — replaced entirely
- `tokio-tungstenite` (from net crate) — if not needed for WebSocket

## Verification Tests

### TLS Fingerprint
```bash
# Hit tls.peet.ws and verify JA3/JA4 match Chrome 130
cargo test -p net --test tls_fingerprint -- --ignored --nocapture
```

### IPv6 Connectivity
```bash
# Connect to example.com (IPv6-only on some networks)
cargo test -p net -- --ignored --nocapture get_ipv6_example_com
```

### Certificate Verification
```bash
# Verify cert chain validation works for Cloudflare, Let's Encrypt, etc.
cargo test -p net -- --ignored --nocapture cert_verification
```

### HTTP/2 Fingerprint
```bash
# Verify SETTINGS frame matches Chrome via http2.tls.peet.ws
cargo test -p net -- --ignored --nocapture h2_fingerprint
```

### Anti-Bot Compatibility
```bash
# Run the full browser comparison suite with our new client
cargo test --release -p browser --test browser_comparison -- --ignored --nocapture
```

## Success Criteria

1. `example.com` loads over IPv6 with valid TLS cert verification
2. `tls.peet.ws` shows JA3/JA4 matching Chrome 130
3. All existing `cargo test --workspace` tests pass (573+ tests)
4. All browser comparison tests pass (evaluate, stealth, content, throughput)
5. Anti-bot sites (Cloudflare, DataDome, Akamai) still pass at same rate
6. No wreq/rquest dependency in Cargo.lock
