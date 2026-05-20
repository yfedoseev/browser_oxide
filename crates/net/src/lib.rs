//! Stealth HTTP client with Chrome TLS/HTTP2 fingerprint impersonation.
//!
//! Built directly on tokio TCP + boring2 (BoringSSL) + http2 crate, replacing wreq.
//! Uses quinn+h3 for HTTP/3 (QUIC) with automatic Alt-Svc discovery and fallback.

pub mod alt_svc;
pub mod blocker;
pub mod compression;
pub mod cookies;
pub mod csp;
pub mod error;
pub mod h1_client;
pub mod h2_client;
pub mod h3_request;
pub mod headers;
// JA4H is patent-pending under FoxIO License 1.1 (non-commercial). The
// computer is test-gated so it never reaches a release binary, fitting the
// "internal testing/evaluation" carve-out. See ja4h.rs and LICENSE-NOTE.md.
#[cfg(test)]
pub(crate) mod ja4h;
pub mod kasada_session;
pub mod pool;
pub mod proxy;
pub mod quic;
pub mod tcp;
pub mod tls;

use alt_svc::AltSvcCache;
use kasada_session::KasadaSessionStore;
use boring2::ssl::SslConnector;
use bytes::Bytes;
use cookies::CookieJar;
use error::NetError;
use http2::client::SendRequest;
use pool::ConnectionPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use stealth::StealthProfile;
use tokio::sync::Mutex;
use url::Url;

#[derive(Debug, Clone)]
pub enum Method {
    Get,
    Post(Vec<u8>),
}

/// HTTP response.
#[derive(Debug, Clone, Default)]
pub struct TimingStats {
    pub dns_start_ms: f64,
    pub dns_end_ms: f64,
    pub connect_start_ms: f64,
    pub connect_end_ms: f64,
    pub tls_start_ms: f64,
    pub tls_end_ms: f64,
    pub request_start_ms: f64,
    pub response_start_ms: f64,
    pub response_end_ms: f64,
}

pub struct Response {
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    /// All Set-Cookie header values, preserved separately because HTTP responses
    /// can contain multiple Set-Cookie headers and a HashMap would collapse them.
    pub set_cookies: Vec<String>,
    pub body: Vec<u8>,
    pub url: String,
    /// Whether this response taught the client Accept-CH for the first time.
    /// Drives reloads in the navigation loop (Wildberries parity).
    pub accept_ch_upgrade: bool,
    pub timings: TimingStats,
}

impl Response {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    pub fn ok(&self) -> bool {
        self.status >= 200 && self.status < 300
    }
}

/// Stealth HTTP client configured with a browser fingerprint profile.
/// Supports HTTP/1.1, HTTP/2 (via boring2/http2) and HTTP/3 (via quinn/rustls).
/// Pool for QUIC connections, keyed by (host, port).
struct QuicPool {
    inner: Arc<Mutex<HashMap<(String, u16), quinn::Connection>>>,
}

impl Clone for QuicPool {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Default for QuicPool {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl QuicPool {
    async fn get(&self, host: &str, port: u16) -> Option<quinn::Connection> {
        let pool = self.inner.lock().await;
        let key = (host.to_string(), port);
        pool.get(&key).cloned()
    }

    async fn put(&self, host: &str, port: u16, conn: quinn::Connection) {
        let mut pool = self.inner.lock().await;
        pool.insert((host.to_string(), port), conn);
    }
}

#[derive(Clone)]
pub struct HttpClient {
    tls_connector: Arc<SslConnector>,
    profile: StealthProfile,
    cookies: Arc<Mutex<CookieJar>>,
    pool: ConnectionPool,
    quic_pool: QuicPool,
    dns_cache: tcp::DnsCache,
    quic_client: Option<quic::QuicClient>,
    alt_svc_cache: AltSvcCache,
    /// Per-origin Kasada session state. Populated when a response includes
    /// `x-kpsdk-cr: true` + `x-kpsdk-st`; consumed by attaching `x-kpsdk-cd`
    /// to subsequent requests to the same host. Solver lives in
    /// `stealth::kasada`.
    kasada_sessions: KasadaSessionStore,
    /// Origins that have sent `Accept-CH` in a response. When an origin is
    /// present, subsequent requests to it use `chrome_headers_with_accept_ch()`
    /// which adds the full set of high-entropy Client Hints. Mirrors Chrome's
    /// behaviour: baseline 13 headers on first visit, full hints after opt-in.
    accept_ch_origins: Arc<Mutex<HashSet<String>>>,
    /// Resolved proxy config. `BOXIDE_PROXY` env var overrides
    /// `profile.proxy`. None = direct connect (the existing path). T1C.
    proxy: Option<proxy::ProxyConfig>,
    /// Per-host Akamai web sensor_data session state. Populated when
    /// a response sets `_abck=`; consumed by `send_sensor_data`. T3A.
    pub akamai_sessions: akamai::AkamaiSessionStore,
}

impl HttpClient {
    /// Borrow this client's stealth profile (read-only). Useful for
    /// callers that need to spawn auxiliary clients with the same UA /
    /// locale / TLS profile (e.g., the sync-fetch op which builds a
    /// fresh client to avoid a connection-pool deadlock with the main
    /// runtime).
    pub fn profile(&self) -> &StealthProfile {
        &self.profile
    }

    pub fn cookies(&self) -> Arc<Mutex<CookieJar>> {
        self.cookies.clone()
    }

    pub fn kasada_sessions(&self) -> KasadaSessionStore {
        self.kasada_sessions.clone()
    }

    pub fn akamai_sessions(&self) -> akamai::AkamaiSessionStore {
        self.akamai_sessions.clone()
    }

    pub fn accept_ch_origins(&self) -> Arc<Mutex<HashSet<String>>> {
        self.accept_ch_origins.clone()
    }

    pub fn dns_cache(&self) -> tcp::DnsCache {
        self.dns_cache.clone()
    }

    pub fn alt_svc_cache(&self) -> AltSvcCache {
        self.alt_svc_cache.clone()
    }

    /// Connect TCP and apply profile-specific TCP fingerprinting (TTL).
    pub(crate) async fn connect_tcp(&self, host: &str, port: u16) -> Result<tokio::net::TcpStream, NetError> {
        let tcp_stream = tcp::connect_via_proxy(
            host,
            port,
            std::time::Duration::from_secs(10),
            Some(&self.dns_cache),
            self.proxy.as_ref(),
        )
        .await?;

        // Set TCP TTL to match claimed OS (Linux=64, Windows=128, macOS=64)
        // Advanced anti-bot systems check TCP SYN TTL vs User-Agent. Gap #8.
        let ttl = match self.profile.os_name.as_str() {
            "Windows" => 128,
            _ => 64,
        };
        let _ = tcp_stream.set_ttl(ttl);

        Ok(tcp_stream)
    }

    /// Create a new client with the given stealth profile.
    pub fn new(profile: &StealthProfile) -> Result<Self, NetError> {
        let connector = tls::chrome_connector(profile)?;

        // Create QUIC client for HTTP/3 (non-fatal if it fails)
        let quic_client = quic::QuicClient::new().ok();

        // Optionally load a persisted cookie jar so Kasada/Akamai trust
        // accumulates across runs. Set BOXIDE_COOKIE_JAR to the desired
        // file path. Without this env var, behavior is the same as before
        // (fresh in-memory jar each run).
        let initial_jar = if let Ok(path) = std::env::var("BOXIDE_COOKIE_JAR") {
            let p = std::path::PathBuf::from(&path);
            match CookieJar::load_from_file(&p) {
                Ok(jar) => {
                    eprintln!("[cookies] loaded persisted jar from {}", path);
                    jar
                }
                Err(e) => {
                    eprintln!("[cookies] failed to load {}: {} (starting fresh)", path, e);
                    CookieJar::new()
                }
            }
        } else {
            CookieJar::new()
        };

        Ok(Self {
            tls_connector: Arc::new(connector),
            profile: profile.clone(),
            cookies: Arc::new(Mutex::new(initial_jar)),
            pool: ConnectionPool::new(),
            quic_pool: QuicPool::default(),
            dns_cache: tcp::DnsCache::new(),
            quic_client,
            alt_svc_cache: AltSvcCache::new(),
            kasada_sessions: KasadaSessionStore::new(),
            akamai_sessions: akamai::AkamaiSessionStore::new(),
            accept_ch_origins: Arc::new(Mutex::new(HashSet::new())),
            // Resolve proxy: BOXIDE_PROXY env override, then profile.proxy.
            // Bad proxy URLs are non-fatal — log and continue without proxy.
            proxy: match proxy::ProxyConfig::resolve(profile.proxy.as_deref()) {
                Ok(p) => {
                    if let Some(ref pc) = p {
                        eprintln!("[proxy] active: scheme={}", match pc {
                            proxy::ProxyConfig::Http { tls: true, .. } => "https",
                            proxy::ProxyConfig::Http { tls: false, .. } => "http",
                            proxy::ProxyConfig::Socks5 { .. } => "socks5",
                        });
                    }
                    p
                }
                Err(e) => {
                    eprintln!("[proxy] WARN: failed to parse proxy URL: {e} (running direct)");
                    None
                }
            },
        })
    }

    /// Create a new client that shares session state (cookies, Kasada/Akamai
    /// tokens, DNS/H3 caches) with an existing one, but has its own
    /// connection pool to avoid deadlocks in synchronous contexts.
    pub fn new_with_shared_state(
        profile: &StealthProfile,
        cookies: Arc<Mutex<CookieJar>>,
        kasada: KasadaSessionStore,
        akamai: akamai::AkamaiSessionStore,
        accept_ch: Arc<Mutex<HashSet<String>>>,
        dns: tcp::DnsCache,
        alt_svc: AltSvcCache,
    ) -> Result<Self, NetError> {
        let connector = tls::chrome_connector(profile)?;
        let quic_client = quic::QuicClient::new().ok();

        Ok(Self {
            tls_connector: Arc::new(connector),
            profile: profile.clone(),
            cookies,
            pool: ConnectionPool::new(),
            quic_pool: QuicPool::default(),
            dns_cache: dns,
            quic_client,
            alt_svc_cache: alt_svc,
            kasada_sessions: kasada,
            akamai_sessions: akamai,
            accept_ch_origins: accept_ch,
            proxy: match proxy::ProxyConfig::resolve(profile.proxy.as_deref()) {
                Ok(p) => p,
                Err(_) => None,
            },
        })
    }

    pub async fn learn_kasada_prefix(&self, host: &str, prefix: &str) {
        self.kasada_sessions.learn_prefix(host, prefix).await;
    }

    /// Learn a Kasada session from response headers (called from response
    /// post-processing). If the response includes `x-kpsdk-cr: true` and
    /// `x-kpsdk-st`, we cache the server-time offset + a session id so
    /// subsequent requests to this host can attach a valid `x-kpsdk-cd`.
    async fn learn_kasada(
        &self,
        host: &str,
        headers: &HashMap<String, String>,
        request_url: Option<&str>,
    ) {
        if headers.contains_key("x-kpsdk-ct") || headers.contains_key("x-kpsdk-st") {
            eprintln!(
                "[kasada] learning from headers for {}: {:?}",
                host,
                headers
                    .keys()
                    .filter(|k| k.starts_with("x-kp"))
                    .collect::<Vec<_>>()
            );
        }
        self.kasada_sessions
            .learn(host, headers, request_url)
            .await;
    }

    /// Record that `host` has advertised `Accept-CH` so subsequent requests
    /// include the full high-entropy Client Hints set. Returns `true`
    /// if this is a new origin for which we just learned Accept-CH.
    async fn learn_accept_ch(&self, host: &str, headers: &HashMap<String, String>) -> bool {
        if headers.keys().any(|k| {
            let k = k.to_ascii_lowercase();
            k == "accept-ch" || k == "critical-ch"
        }) {
            let mut origins = self.accept_ch_origins.lock().await;
            if !origins.contains(host) {
                origins.insert(host.to_string());
                return true;
            }
        }
        false
    }

    /// Per W3 Client Hints Reliability spec
    /// (https://wicg.github.io/client-hints-infrastructure/#critical-ch),
    /// when a server sends `Critical-CH`, the browser MUST retry the
    /// request with the listed hints BEFORE rendering the response.
    /// Without this retry, the server treats the client as non-conformant
    /// (Cloudflare Managed Challenge serves the captcha; DataDome
    /// returns 403; Akamai BMP downgrades).
    fn needs_critical_ch_retry(headers: &HashMap<String, String>) -> bool {
        headers.keys().any(|k| k.eq_ignore_ascii_case("critical-ch"))
    }

    /// Learn Akamai web `_abck` state from response Set-Cookies. T3A.
    /// The trust-state (Favorable / NeedsSensor / NeedsSecCpt /
    /// NeedsPixel) is extracted from the cookie suffix; the consumer
    /// (Page::navigate scheduler) decides whether to POST sensor_data.
    async fn learn_abck(&self, host: &str, set_cookies: &[String]) {
        for v in set_cookies {
            let trimmed = v.trim_start();
            if let Some(rest) = trimmed.strip_prefix("_abck=") {
                let value = rest.split(';').next().unwrap_or("").trim();
                eprintln!("[akamai] learn_abck: _abck={}", value);
                if !value.is_empty() {
                    self.akamai_sessions
                        .with_session(host, |s| s.observe_abck(value))
                        .await;
                }
            } else if let Some(rest) = trimmed.strip_prefix("bm_sz=") {
                let value = rest.split(';').next().unwrap_or("").trim();
                eprintln!("[akamai] learn_abck: bm_sz={}", value);
                if !value.is_empty() {
                    self.akamai_sessions
                        .with_session(host, |s| s.bm_sz = Some(value.to_string()))
                        .await;
                }
            }
        }
    }

    /// Build and POST a sensor_data body to upgrade `_abck` for `host`.
    /// `drained_events` is the parsed output of `akamai::DRAIN_JS`
    /// from the page (mouse / key trajectory captured by humanize.js).
    /// `request_url` is the page URL the sensor_data is "for".
    /// `tenant_seed` is the seed observed in the challenge JS for this
    /// tenant (e.g. 3_224_113 for bestbuy); pass 0 if unknown.
    /// Returns Ok(new_abck_state) on success.
    pub async fn send_akamai_sensor_data(
        &self,
        host: &str,
        request_url: &str,
        post_path: &str,
        tenant_seed: i64,
        drained: akamai::Drained,
    ) -> Result<akamai::AbckState, NetError> {
        // Snapshot the session and build the body.
        let body = self
            .akamai_sessions
            .with_session(host, |s| {
                s.mouse_buf = drained.mouse;
                s.key_buf = drained.key;
                s.touch_buf = drained.touch;
                s.key_count = drained.key_count;
                s.mouse_count = drained.mouse_count;
                s.touch_count = drained.touch_count;
                s.scroll_count = drained.scroll_count;
                s.accel_count = drained.accel_count;
                s.sensor_counter = s.sensor_counter.saturating_add(1);
                let sensor = akamai::build_sensor_data(
                    &self.profile,
                    s,
                    request_url,
                    tenant_seed,
                );
                format!("{{\"sensor_data\":\"{}\"}}", sensor)
            })
            .await;

        // POST with the same Chrome 147 header set we use for any
        // other request, plus content-type: text/plain (per the A0
        // capture).
        let url = format!("https://{host}{post_path}");
        let mut headers = headers::chrome_headers(&self.profile);
        headers.push(("content-type".into(), "text/plain;charset=UTF-8".into()));
        headers.push(("origin".into(), format!("https://{host}")));
        headers.push(("referer".into(), request_url.to_string()));
        let resp = self.post_with_headers(&url, &body, &headers).await?;
        if resp.status == 429 {
            eprintln!("[akamai] sensor_data POST 429 body: {}", String::from_utf8_lossy(&resp.body));
        }
        // Re-learn _abck from the response.
        self.learn_abck(host, &resp.set_cookies).await;
        let new_state = self
            .akamai_sessions
            .abck_state(host)
            .await
            .unwrap_or(akamai::AbckState::Unknown);
        eprintln!(
            "[akamai] sensor_data POST {url} → status={} new_abck={new_state:?}",
            resp.status
        );
        Ok(new_state)
    }

    /// Return `true` if `host` has previously sent `Accept-CH`.
    pub async fn has_accept_ch(&self, host: &str) -> bool {
        self.accept_ch_origins.lock().await.contains(host)
    }

    /// Fetch the Kasada `/mfc` endpoint for `host` if we have a session
    /// with a known tenant prefix and don't yet have an fc token.
    /// On success, stores `x-kpsdk-fc` from the response in the session.
    /// Previously raced the page's ips.js by fetching /mfc from Rust with a
    /// hardcoded `x-kpsdk-dt` literal. That token is per-session and derived
    /// inside the ips.js VM; hardcoding it caused Kasada to refuse to issue
    /// `x-kpsdk-fc` (and exposed an obvious bot signature: every session
    /// presented the same dt value).
    ///
    /// The page's own ips.js fetches /mfc with the correct session-derived
    /// headers via window.fetch(), and our `learn_from_headers` already
    /// extracts `x-kpsdk-fc` from any response that carries it. So the right
    /// behaviour is to do nothing here — let the page run.
    pub async fn fetch_kasada_mfc_if_needed(&self, _host: &str) {
        // Intentionally a no-op. See doc comment.
    }

    pub async fn evict_kasada_session(&self, host: &str) {
        self.kasada_sessions.evict(host).await;
    }

    /// Compute (and possibly inject) a Kasada `x-kpsdk-cd` header for an
    /// outgoing request to `host`. Returns the header pair if we have a
    /// session for that host; caller appends to its header list.
    ///
    /// **K1 (FP / nocdp-parity, 2026-05-17): deferred to the page's
    /// ips.js by default.** Same lesson as `fetch_kasada_mfc_if_needed`
    /// (made a deliberate no-op above): the correct session-derived
    /// `x-kpsdk-cd` is computed *inside the ips.js VM* and emitted by
    /// the page via `window.fetch()`; our `learn_from_headers` already
    /// captures the resulting session. A Rust-side PoW computed *in
    /// parallel* to ips.js produces a second, single-use `x-kpsdk-cd`
    /// the page's own ips.js did not author — a self-inflicted bot
    /// signature (a real Chrome emits exactly one ips.js-authored cd,
    /// never a parallel native one). nocdp real Chrome — which has NO
    /// such Rust path — passes canadagoose/hyatt/realtor from this IP,
    /// so deferring here makes us *more* real-browser-faithful, not
    /// less. The PoW impl is retained (not deleted) and can be
    /// re-enabled for the page-less/no-V8 client path via
    /// `BOXIDE_KASADA_RUST_CD=1`. Removes the K2-DIFF confound: the
    /// `/tl` differential must compare ips.js's authored payload, not a
    /// Rust/ips.js race. The call sites still `!has_header` so an
    /// ips.js-set cd is always preserved.
    pub async fn kasada_cd_header(&self, host: &str) -> Option<(String, String)> {
        if std::env::var("BOXIDE_KASADA_RUST_CD").as_deref() != Ok("1") {
            return None; // let the page's ips.js own x-kpsdk-cd
        }
        self.kasada_sessions
            .compute_cd_header(host)
            .await
            .map(|cd| ("x-kpsdk-cd".to_string(), cd))
    }

    /// Try HTTP/3 for an HTTPS URL. Returns Ok if successful, Err to fall back.
    async fn try_h3_request(
        &self,
        url: &str,
        method: Method,
        extra_headers: &[(String, String)],
    ) -> Result<Response, NetError> {
        // Belt-and-suspenders: even if something populates the cache, never
        // emit a QUIC handshake when allow_http3=false. See learn_alt_svc()
        // for the full rationale (gap #33).
        if !self.profile.allow_http3 {
            return Err(NetError::Quic("h3 disabled by profile".into()));
        }
        let parsed = Url::parse(url).map_err(|e| NetError::Quic(e.to_string()))?;
        if parsed.scheme() != "https" {
            return Err(NetError::Quic("not HTTPS".into()));
        }

        let host = parsed
            .host_str()
            .ok_or_else(|| NetError::Quic("no host".into()))?;
        let cached_port = self.alt_svc_cache.lookup(host).await;
        let port = cached_port.ok_or_else(|| NetError::Quic("not in alt-svc cache".into()))?;

        let quic = self
            .quic_client
            .as_ref()
            .ok_or_else(|| NetError::Quic("no quic client".into()))?;

        // Try pooled connection first, then create new
        let conn = if let Some(conn) = self.quic_pool.get(host, port).await {
            conn
        } else {
            let conn =
                tokio::time::timeout(std::time::Duration::from_secs(3), quic.connect(host, port))
                    .await
                    .map_err(|_| NetError::Quic("connect timeout".into()))?
                    .map_err(|e| NetError::Quic(e.to_string()))?;
            self.quic_pool.put(host, port, conn.clone()).await;
            conn
        };

        let (resp, alt_svc) =
            h3_request::h3_request(conn, &parsed, method, &self.profile, extra_headers).await?;

        // Update cache from response
        if let Some(alt_svc_header) = &alt_svc {
            if let Some((port, max_age)) = alt_svc::parse_alt_svc(alt_svc_header) {
                self.alt_svc_cache.insert(host, port, max_age).await;
            }
        }

        Ok(resp)
    }

    /// Learn h3 support from a response's Alt-Svc header.
    ///
    /// Gap #33 (2026-04-26): when `profile.allow_http3 = false` (the default)
    /// we DO NOT cache the h3 alternative. Reason: vanilla `quinn-proto 0.11`
    /// emits transport_parameters in a *random* order with a *random* GREASE
    /// TP per handshake. Real Chrome uses a deterministic fixed order — so
    /// upgrading to QUIC with our current stack would emit a uniquely-
    /// distinguishable browser_oxide signature. Until we vendor-fork
    /// quinn-proto with a Chrome-fixed-order patch, advertising h3 is worse
    /// than not speaking it at all.
    async fn learn_alt_svc(&self, url: &str, resp_headers: &HashMap<String, String>) {
        if !self.profile.allow_http3 {
            return;
        }
        if let Some(alt_svc_header) = resp_headers.get("alt-svc") {
            if let Some((port, max_age)) = alt_svc::parse_alt_svc(alt_svc_header) {
                if let Ok(parsed) = Url::parse(url) {
                    if let Some(host) = parsed.host_str() {
                        self.alt_svc_cache.insert(host, port, max_age).await;
                    }
                }
            }
        }
    }

    /// Inject cookies from external sources (e.g., JS document.cookie).
    pub async fn inject_cookies(&self, url: &Url, cookies: &[String]) {
        let mut jar = self.cookies.lock().await;
        jar.set_cookies(url, cookies);
    }

    /// Connect TCP+TLS and perform HTTP/2 handshake, returning a sender.
    /// Also spawns the connection driver task.
    async fn connect_h2(&self, host: &str, port: u16) -> Result<SendRequest<Bytes>, NetError> {
        let tcp_stream = self.connect_tcp(host, port).await?;

        let tls_stream = tls::connect_tls(&self.tls_connector, &self.profile, host, tcp_stream).await?;

        // Check ALPN
        let alpn = tls::negotiated_alpn(&tls_stream);
        if alpn != Some(b"h2") {
            // HTTP/1.1 fallback — not using pool for this
            return Err(NetError::Http("ALPN negotiated http/1.1, not h2".into()));
        }

        let (sender, conn) = h2_client::handshake(tls_stream, &self.profile).await?;

        // Spawn the connection driver
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                eprintln!("HTTP/2 connection error: {e}");
            }
        });

        // Store in pool for reuse
        self.pool.put(host, port, sender.clone()).await;

        Ok(sender)
    }

    /// Pre-establish a TCP+TLS+HTTP/2 connection to a host.
    /// The connection is stored in the pool for future requests.
    pub async fn preconnect(&self, host: &str, port: u16) -> Result<(), NetError> {
        if self.pool.get(host, port).await.is_some() {
            return Ok(());
        }
        self.connect_h2(host, port).await?;
        Ok(())
    }

    /// Get or create an HTTP/2 sender for the given host.
    async fn get_sender(&self, host: &str, port: u16) -> Result<SendRequest<Bytes>, NetError> {
        // Check pool first
        if let Some(sender) = self.pool.get(host, port).await {
            self.pool.touch(host, port).await;
            return Ok(sender);
        }
        // Create new connection
        self.connect_h2(host, port).await
    }

    /// Perform a GET request. Tries HTTP/3 if available, falls back to HTTP/2.
    pub async fn get(&self, url: &str) -> Result<Response, NetError> {
        self.get_with_headers(url, &[]).await
    }

    /// Fetch-API-style GET: uses `chrome_headers_fetch` (accept: */*, no
    /// upgrade-insecure-requests, sec-fetch-dest: empty, etc.) as the base
    /// header set, with caller's extras merged in. `origin` is the page's
    /// origin string (e.g. "https://www.canadagoose.com"); if `None`, the
    /// request looks like it came from a `no-origin` context (first navigation).
    pub async fn fetch_get(
        &self,
        url: &str,
        extra_headers: &[(String, String)],
        origin: Option<&str>,
    ) -> Result<Response, NetError> {
        let mut hdrs = headers::nav_headers_fetch(&self.profile, url, origin);
        merge_headers(&mut hdrs, extra_headers);
        self.get_with_exact_headers(url, &hdrs).await
    }

    /// Fetch-API-style POST with raw bytes.
    pub async fn fetch_post_bytes(
        &self,
        url: &str,
        body: &[u8],
        extra_headers: &[(String, String)],
        origin: Option<&str>,
    ) -> Result<Response, NetError> {
        let mut hdrs = headers::nav_headers_fetch(&self.profile, url, origin);
        merge_headers(&mut hdrs, extra_headers);
        self.post_bytes_with_exact_headers(url, body, &hdrs).await
    }

    /// POST with the caller's exact header set — NO chrome_headers overlay.
    /// Counterpart to `get_with_exact_headers` for JS fetch POSTs.

    /// GET with the caller's exact header set — NO chrome_headers overlay.
    /// Used for "reload" flavors where sec-fetch-user must be omitted
    /// (chrome_headers always adds it). The caller is responsible for
    /// providing user-agent, accept, etc. Cookies are still auto-injected
    /// from the jar unless the caller already included a Cookie header.
    pub async fn get_with_exact_headers(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> Result<Response, NetError> {
        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| NetError::Http(format!("no host in URL: {url}")))?;
        let port = parsed.port().unwrap_or(443);

        let mut hdrs: Vec<(String, String)> = headers
            .iter()
            .filter(|(k, _)| {
                let lower = k.to_ascii_lowercase();
                !lower.starts_with(':') && lower != "host" && lower != "connection"
            })
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();

        if !has_header(&hdrs, "cookie") {
            let jar = self.cookies.lock().await;
            if let Some(cookie_str) = jar.cookies_for(&parsed) {
                insert_before_priority(&mut hdrs, "cookie".to_string(), cookie_str);
            }
        }

        // Inject Kasada x-kpsdk-cd if we have a session for this host (gap #Kasada).
        if !has_header(&hdrs, "x-kpsdk-cd") {
            if let Some((k, v)) = self.kasada_cd_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }
        // Inject Kasada x-kpsdk-fc on stricter tenants (Hyper-Solutions Flow 2).
        if !has_header(&hdrs, "x-kpsdk-fc") {
            if let Some((k, v)) = self.kasada_sessions.fc_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }
        // Inject Kasada x-kpsdk-ct (session token from /tl response). Without
        // this the server returns the same Kasada init page even with a valid
        // x-kpsdk-cd PoW. Verified 2026-04-27 on hyatt.com.
        if !has_header(&hdrs, "x-kpsdk-ct") {
            if let Some((k, v)) = self.kasada_sessions.ct_header(host).await {
                eprintln!("[kasada] INJECTING x-kpsdk-ct on GET {} (len={})", host, v.len());
                insert_before_priority(&mut hdrs, k, v);
            } else {
                eprintln!("[kasada] no ct_token to inject for {}", host);
            }
        }
        if !has_header(&hdrs, "x-kpsdk-h") {
            if let Some((k, v)) = self.kasada_sessions.h_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }
        if !has_header(&hdrs, "x-kpsdk-v") {
            if let Some((k, v)) = self.kasada_sessions.v_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }
        if !has_header(&hdrs, "x-kpsdk-im") {
            if let Some(v) = self.kasada_sessions.im_token(host).await {
                insert_before_priority(&mut hdrs, "x-kpsdk-im".to_string(), v);
            }
        }
        if !has_header(&hdrs, "x-kpsdk-dt") {
            if let Some(v) = self.kasada_sessions.dt_token(host).await {
                insert_before_priority(&mut hdrs, "x-kpsdk-dt".to_string(), v);
            }
        }
        if !has_header(&hdrs, "x-kpsdk-r") {
            if let Some((k, v)) = self.kasada_sessions.r_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }

        let response = 'h2: {
            for attempt in 0..2 {
                let sender_res = self.get_sender(host, port).await;
                let mut sender = match sender_res {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[net] H2 connection failed for {}: {}", host, e);
                        break 'h2 None;
                    }
                };
                let uri = parsed.as_str();
                if uri.contains("/mfc") || uri.contains("/akam/13") || uri.contains("/tl") || uri.contains("/r") {
                    eprintln!("[net] sending request to {} with headers: {:?}", uri, hdrs);
                }
                match h2_client::send_get(&mut sender, uri, host, &hdrs).await {
                    Ok((parts, body)) => {
                        let resp = self.build_response(parts, body, url).await?;
                        break 'h2 Some(resp);
                    }
                    Err(e) if attempt == 0 && is_stale_conn_error(&e) => {
                        self.pool.evict(host, port).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            None
        };
        let response = match response {
            Some(r) => r,
            None => {
                let tcp_stream = tcp::connect_via_proxy(
                    host,
                    port,
                    std::time::Duration::from_secs(10),
                    Some(&self.dns_cache),
                    self.proxy.as_ref(),
                )
                .await?;
                let mut tls_stream =
                    tls::connect_tls(&self.tls_connector, &self.profile, host, tcp_stream).await?;
                let path = if parsed.query().is_some() {
                    format!("{}?{}", parsed.path(), parsed.query().unwrap())
                } else {
                    parsed.path().to_string()
                };
                if url.contains("/mfc") || url.contains("/akam/13") || url.contains("/tl") || url.contains("/r") {
                    eprintln!("[net] sending H1 request to {} with headers: {:?}", url, hdrs);
                }
                let raw = h1_client::send_get(&mut tls_stream, host, &path, &hdrs).await?;
                self.build_response_from_raw(raw, url).await?
            }
        };
        self.learn_alt_svc(url, &response.headers).await;
        self.learn_kasada(host, &response.headers, Some(url)).await;
        self.learn_abck(host, &response.set_cookies).await;
        let upgrade = self.learn_accept_ch(host, &response.headers).await;
        self.store_set_cookies(&parsed, &response.set_cookies).await;

        let mut final_response = response;
        final_response.accept_ch_upgrade = upgrade;
        Ok(final_response)
    }

    /// GET follow for exact-header requests.
    pub async fn get_follow_exact_headers(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_redirects: u8,
    ) -> Result<Response, NetError> {
        let mut current_url = url.to_string();
        for _ in 0..max_redirects {
            let resp = self.get_with_exact_headers(&current_url, headers).await?;
            if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
                if let Some(loc) = resp.headers.get("location") {
                    current_url = resolve_redirect(&current_url, loc)?;
                    continue;
                }
            }
            return Ok(resp);
        }
        self.get_with_exact_headers(&current_url, headers).await
    }

    /// GET with caller-provided extra headers (e.g., from JS fetch init.headers).
    /// Extra headers override any matching profile headers (case-insensitive match).
    pub async fn get_with_headers(
        &self,
        url: &str,
        extra_headers: &[(String, String)],
    ) -> Result<Response, NetError> {
        // Try HTTP/3 first
        if let Ok(resp) = self.try_h3_request(url, Method::Get, extra_headers).await {
            return Ok(resp);
        }

        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| NetError::Http(format!("no host in URL: {url}")))?;
        let port = parsed.port().unwrap_or(443);

        // Browser-aware nav headers. For Chrome, may upgrade to high-entropy
        // Client Hints if this origin has sent Accept-CH. Firefox profiles
        // skip the upgrade (Firefox has no Client Hints).
        let accept_ch_upgraded = self.has_accept_ch(host).await;
        let mut hdrs = headers::nav_headers(&self.profile, accept_ch_upgraded);
        merge_headers(&mut hdrs, extra_headers);

        // Add cookies (unless caller already supplied one)
        if !has_header(&hdrs, "cookie") {
            let jar = self.cookies.lock().await;
            if let Some(cookie_str) = jar.cookies_for(&parsed) {
                insert_before_priority(&mut hdrs, "cookie".to_string(), cookie_str);
            }
        }

        // Inject Kasada `x-kpsdk-cd` PoW header if we have a session for
        // this host (gap #Kasada). The session is populated when a prior
        // response from this host included `x-kpsdk-cr: true` + `x-kpsdk-st`.
        // See `crates/stealth/src/kasada.rs` for the SHA-256 PoW algorithm
        // and `crates/net/src/kasada_session.rs` for the per-origin store.
        if !has_header(&hdrs, "x-kpsdk-cd") {
            if let Some((k, v)) = self.kasada_cd_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }

        // Try HTTP/2 with automatic stale-connection recovery. If the pooled
        // connection has been closed by the server (GOAWAY), retry once with
        // a fresh connection.
        let response = 'h2: {
            for attempt in 0..2 {
                let sender_res = self.get_sender(host, port).await;
                let mut sender = match sender_res {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[net] H2 connection failed for {}: {}", host, e);
                        break 'h2 None;
                    }
                };
                let uri = parsed.as_str();
                if uri.contains("/mfc") || uri.contains("/akam/13") || uri.contains("/tl") || uri.contains("/r") {
                    eprintln!("[net] sending request to {} with headers: {:?}", uri, hdrs);
                }
                match h2_client::send_get(&mut sender, uri, host, &hdrs).await {
                    Ok((parts, body)) => {
                        let resp = self.build_response(parts, body, url).await?;
                        break 'h2 Some(resp);
                    }
                    Err(e) if attempt == 0 && is_stale_conn_error(&e) => {
                        // Evict the dead connection from the pool and try once more.
                        self.pool.evict(host, port).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            None
        };

        let response = match response {
            Some(r) => r,
            None => {
                // HTTP/1.1 fallback
                let tcp_stream = tcp::connect_via_proxy(
                    host,
                    port,
                    std::time::Duration::from_secs(10),
                    Some(&self.dns_cache),
                    self.proxy.as_ref(),
                )
                .await?;
                let mut tls_stream =
                    tls::connect_tls(&self.tls_connector, &self.profile, host, tcp_stream).await?;
                let path = if parsed.query().is_some() {
                    format!("{}?{}", parsed.path(), parsed.query().unwrap())
                } else {
                    parsed.path().to_string()
                };
                if url.contains("/mfc") || url.contains("/akam/13") || url.contains("/tl") || url.contains("/r") {
                    eprintln!("[net] sending H1 request to {} with headers: {:?}", url, hdrs);
                }
                let raw = h1_client::send_get(&mut tls_stream, host, &path, &hdrs).await?;
                self.build_response_from_raw(raw, url).await?
            }
        };

        // Learn Alt-Svc from response
        self.learn_alt_svc(url, &response.headers).await;

        // Learn Kasada session from response (look for x-kpsdk-cr + x-kpsdk-st).
        self.learn_kasada(host, &response.headers, Some(url)).await;
        self.learn_abck(host, &response.set_cookies).await;
        let upgrade = self.learn_accept_ch(host, &response.headers).await;

        // Store Set-Cookie from response into jar
        self.store_set_cookies(&parsed, &response.set_cookies).await;

        // Critical-CH per W3 spec: when the server demands immediate-retry
        // hints AND we weren't already sending full CH, retry the same
        // URL once with the high-entropy hints. Cloudflare Managed
        // Challenge (udemy.com) and DataDome (yelp/leboncoin/etsy/wsj)
        // both send Critical-CH; without an immediate retry they treat
        // us as non-Chrome and serve the captcha. One-shot to avoid loops.
        if !accept_ch_upgraded && Self::needs_critical_ch_retry(&response.headers) {
            return Box::pin(self.get_with_headers(url, extra_headers)).await;
        }

        let mut final_response = response;
        final_response.accept_ch_upgrade = upgrade;
        Ok(final_response)
    }

    /// Store all Set-Cookie headers from a response into the cookie jar.
    async fn store_set_cookies(&self, url: &Url, set_cookies: &[String]) {
        if set_cookies.is_empty() {
            return;
        }
        let mut jar = self.cookies.lock().await;
        jar.set_cookies(url, set_cookies);
        // Persist if BOXIDE_COOKIE_JAR is set. Atomic write (tempfile +
        // rename) so concurrent runs don't tear the file.
        if let Ok(path) = std::env::var("BOXIDE_COOKIE_JAR") {
            let p = std::path::PathBuf::from(&path);
            if let Err(e) = jar.save_to_file(&p) {
                eprintln!("[cookies] save_to_file({}) failed: {}", path, e);
            }
        }
    }

    /// GET with explicit redirect following.
    /// Perform a GET request, following redirects up to `max_redirects`.
    /// Set `BOXIDE_DEBUG_REDIRECTS=1` for hop-by-hop tracing.
    pub async fn get_follow(&self, url: &str, max_redirects: u8) -> Result<Response, NetError> {
        let debug = std::env::var("BOXIDE_DEBUG_REDIRECTS").is_ok();
        let mut current_url = url.to_string();
        for hop in 0..max_redirects {
            if debug {
                eprintln!("[redirect] hop={} GET {}", hop, current_url);
            }
            let resp = self.get(&current_url).await?;
            if debug {
                let body_len = resp.body.len();
                let cookies: Vec<&str> = resp
                    .set_cookies
                    .iter()
                    .map(|s| s.split(';').next().unwrap_or("").trim())
                    .collect();
                eprintln!(
                    "[redirect]   <- status={} body_len={} location={:?} set-cookies={:?}",
                    resp.status,
                    body_len,
                    resp.headers.get("location"),
                    cookies
                );
            }

            if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
                if let Some(loc) = resp.headers.get("location") {
                    let next_url = resolve_redirect(&current_url, loc)?;
                    // For 301, 302, 303 redirects from a GET, we just continue with another GET.
                    // For 307, 308, we MUST preserve the method (GET), which self.get() does.
                    current_url = next_url;
                    continue;
                }
            }
            return Ok(resp);
        }
        if debug {
            eprintln!("[redirect] hit max_redirects={}, final GET {}", max_redirects, current_url);
        }
        self.get(&current_url).await
    }

    /// Perform a GET request, following redirects, with extra headers.
    pub async fn get_follow_with_headers(
        &self,
        url: &str,
        extra_headers: &[(String, String)],
        max_redirects: u8,
    ) -> Result<Response, NetError> {
        let mut current_url = url.to_string();
        for _ in 0..max_redirects {
            // Re-apply headers to each hop
            let resp = self.get_with_headers(&current_url, extra_headers).await?;

            if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
                if let Some(loc) = resp.headers.get("location") {
                    current_url = resolve_redirect(&current_url, loc)?;
                    continue;
                }
            }
            return Ok(resp);
        }
        self.get_with_headers(&current_url, extra_headers).await
    }

    /// Perform a POST request, following redirects.
    /// DDoS-Guard (ozon.ru) returns 307 on POST /abt/result, which requires
    /// re-POSTing the body to the new location.
    pub async fn post_follow(
        &self,
        url: &str,
        body: &str,
        max_redirects: u8,
    ) -> Result<Response, NetError> {
        self.post_bytes_follow(url, body.as_bytes(), &[], max_redirects)
            .await
    }

    /// POST with raw bytes and redirects.
    pub async fn post_bytes_follow(
        &self,
        url: &str,
        body: &[u8],
        extra_headers: &[(String, String)],
        max_redirects: u8,
    ) -> Result<Response, NetError> {
        let mut current_url = url.to_string();
        for _ in 0..max_redirects {
            let resp = self
                .post_bytes_with_headers(&current_url, body, extra_headers)
                .await?;

            if matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
                if let Some(loc) = resp.headers.get("location") {
                    let next_url = resolve_redirect(&current_url, loc)?;
                    if matches!(resp.status, 307 | 308) {
                        // 307/308: MUST re-POST the same body to the new location.
                        current_url = next_url;
                        continue;
                    } else {
                        // 301/302/303: Standard behavior is to switch to GET.
                        return self.get_follow(&next_url, max_redirects - 1).await;
                    }
                }
            }
            return Ok(resp);
        }
        self.post_bytes_with_headers(&current_url, body, extra_headers)
            .await
    }

    /// Perform a POST request.
    pub async fn post(&self, url: &str, body: &str) -> Result<Response, NetError> {
        self.post_with_headers(url, body, &[]).await
    }

    /// POST with caller-provided extra headers (e.g., Content-Type from JS fetch).
    pub async fn post_with_headers(
        &self,
        url: &str,
        body: &str,
        extra_headers: &[(String, String)],
    ) -> Result<Response, NetError> {
        self.post_bytes_with_headers(url, body.as_bytes(), extra_headers)
            .await
    }

    pub async fn post_bytes_with_exact_headers_direct(
        &self,
        url: &str,
        body: &[u8],
        headers: &[(String, String)],
    ) -> Result<Response, NetError> {
        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| NetError::Http(format!("no host in URL: {url}")))?;
        let port = parsed.port().unwrap_or(443);
        let path = if let Some(q) = parsed.query() {
            format!("{}?{}", parsed.path(), q)
        } else {
            parsed.path().to_string()
        };

        let mut hdrs: Vec<(String, String)> = headers
            .iter()
            .filter(|(k, _)| {
                let lower = k.to_ascii_lowercase();
                !lower.starts_with(':') && lower != "host" && lower != "connection"
            })
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();

        // Add cookies
        let jar = self.cookies.lock().await;
        if let Some(cookie_str) = jar.cookies_for(&parsed) {
            if !has_header(&hdrs, "cookie") {
                hdrs.push(("cookie".to_string(), cookie_str));
            }
        }
        drop(jar);

        let tcp_stream = self.connect_tcp(host, port).await?;
        let connector = tls::chrome_connector(&self.profile)?;
        let mut tls_stream = tls::connect_tls(&connector, &self.profile, host, tcp_stream).await?;

        let raw = h1_client::send_post(&mut tls_stream, host, &path, &hdrs, body).await?;
        self.build_response_from_raw(raw, url).await
    }

    /// POST with a raw byte body and ONLY the caller-provided headers plus cookies.
    pub async fn post_bytes_with_exact_headers(
        &self,
        url: &str,
        body: &[u8],
        headers: &[(String, String)],
    ) -> Result<Response, NetError> {
        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| NetError::Http(format!("no host in URL: {url}")))?;
        let port = parsed.port().unwrap_or(443);

        let mut hdrs: Vec<(String, String)> = headers
            .iter()
            .filter(|(k, _)| {
                let lower = k.to_ascii_lowercase();
                !lower.starts_with(':') && lower != "host" && lower != "connection"
            })
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
            .collect();

        // Add cookies (unless already supplied)
        if !has_header(&hdrs, "cookie") {
            let jar = self.cookies.lock().await;
            if let Some(cookie_str) = jar.cookies_for(&parsed) {
                hdrs.push(("cookie".to_string(), cookie_str));
            }
        }

        // Add Kasada headers
        if let Some(ct) = self.kasada_sessions.ct_token(host).await {
            if !has_header(&hdrs, "x-kpsdk-ct") {
                hdrs.push(("x-kpsdk-ct".to_string(), ct));
            }
        }
        if let Some(fc) = self.kasada_sessions.fc_token(host).await {
            if !has_header(&hdrs, "x-kpsdk-fc") {
                hdrs.push(("x-kpsdk-fc".to_string(), fc));
            }
        }
        if let Some(im) = self.kasada_sessions.im_token(host).await {
            if !has_header(&hdrs, "x-kpsdk-im") {
                hdrs.push(("x-kpsdk-im".to_string(), im));
            }
        }
        if let Some(dt) = self.kasada_sessions.dt_token(host).await {
            if !has_header(&hdrs, "x-kpsdk-dt") {
                hdrs.push(("x-kpsdk-dt".to_string(), dt));
            }
        }
        if !has_header(&hdrs, "x-kpsdk-h") {
            if let Some((k, v)) = self.kasada_sessions.h_header(host).await {
                hdrs.push((k, v));
            }
        }
        if !has_header(&hdrs, "x-kpsdk-v") {
            if let Some((k, v)) = self.kasada_sessions.v_header(host).await {
                hdrs.push((k, v));
            }
        }
        if !has_header(&hdrs, "x-kpsdk-r") {
            if let Some((k, v)) = self.kasada_sessions.r_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }
        // Inject Kasada x-kpsdk-cd if we have a session for this host (gap #Kasada).
        if !has_header(&hdrs, "x-kpsdk-cd") {
            if let Some((k, v)) = self.kasada_cd_header(host).await {
                hdrs.push((k, v));
            }
        }

        // Env-gated POST body dump
        if let Ok(dir) = std::env::var("BOXIDE_DUMP_POST_DIR") {
            use std::io::Write;
            let _ = std::fs::create_dir_all(&dir);
            let counter_path = format!("{}/.counter", dir);
            let next: usize = std::fs::read_to_string(&counter_path)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                + 1;
            let _ = std::fs::write(&counter_path, next.to_string());
            let stem = format!("{}/{:03}", dir, next);
            if let Ok(mut f) = std::fs::File::create(format!("{stem}.body")) {
                let _ = f.write_all(body);
            }
            let mut meta = String::new();
            meta.push_str("{\n");
            meta.push_str(&format!(
                "  \"url\": {},\n",
                serde_json::to_string(url).unwrap_or_else(|_| "\"\"".into())
            ));
            meta.push_str(&format!("  \"body_len\": {},\n", body.len()));
            meta.push_str("  \"headers\": {\n");
            for (i, (k, v)) in hdrs.iter().enumerate() {
                let trailing = if i + 1 == hdrs.len() { "" } else { "," };
                meta.push_str(&format!(
                    "    {}: {}{}\n",
                    serde_json::to_string(k).unwrap_or_else(|_| "\"\"".into()),
                    serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into()),
                    trailing
                ));
            }
            meta.push_str("  }\n}\n");
            let _ = std::fs::write(format!("{stem}.meta.json"), meta);
        }

        let response = 'h2: {
            for attempt in 0..2 {
                let sender_res = self.get_sender(host, port).await;
                let mut sender = match sender_res {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[net] H2 connection failed for {}: {}", host, e);
                        break 'h2 None;
                    }
                };
                let uri = parsed.as_str();
                if uri.contains("/mfc") || uri.contains("/akam/13") || uri.contains("/tl") || uri.contains("/r") {
                    eprintln!("[net] sending H2 request to {} with headers: {:?}", uri, hdrs);
                }
                match h2_client::send_post(&mut sender, uri, host, &hdrs, body).await {
                    Ok((parts, resp_body)) => {
                        let resp = self.build_response(parts, resp_body, url).await?;
                        break 'h2 Some(resp);
                    }
                    Err(e) if attempt == 0 && is_stale_conn_error(&e) => {
                        self.pool.evict(host, port).await;
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[net] H2 POST failed for {}: {}", uri, e);
                    }
                }
            }
            None
        };

        let response = match response {
            Some(r) => r,
            None => {
                let tcp_stream = tcp::connect_via_proxy(
                    host,
                    port,
                    std::time::Duration::from_secs(10),
                    Some(&self.dns_cache),
                    self.proxy.as_ref(),
                )
                .await?;
                let connector = tls::chrome_connector(&self.profile)?;
                let mut tls_stream = tls::connect_tls(&connector, &self.profile, host, tcp_stream).await?;
                let path = if parsed.query().is_some() {
                    format!("{}?{}", parsed.path(), parsed.query().unwrap())
                } else {
                    parsed.path().to_string()
                };
                if url.contains("/mfc") || url.contains("/akam/13") || url.contains("/tl") || url.contains("/r") {
                    eprintln!("[net] sending H1 request to {} with headers: {:?}", url, hdrs);
                }
                let raw = h1_client::send_post(&mut tls_stream, host, &path, &hdrs, body).await?;
                self.build_response_from_raw(raw, url).await?
            }
        };

        self.learn_kasada(host, &response.headers, Some(url)).await;
        self.learn_abck(host, &response.set_cookies).await;
        let upgrade = self.learn_accept_ch(host, &response.headers).await;
        self.store_set_cookies(&parsed, &response.set_cookies).await;

        let mut final_response = response;
        final_response.accept_ch_upgrade = upgrade;

        // If this was a successful /tl, we might need to kick off /mfc
        if url.contains("/tl") || url.contains("/r") {
            self.fetch_kasada_mfc_if_needed(host).await;
        }

        Ok(final_response)
    }

    /// POST with a raw byte body and caller-provided headers. This is the
    /// binary-safe path — Kasada's challenge solver sends
    /// `application/octet-stream` with a raw byte payload that must not be
    /// mangled by UTF-8 coercion.
    pub async fn post_bytes_with_headers(
        &self,
        url: &str,
        body: &[u8],
        extra_headers: &[(String, String)],
    ) -> Result<Response, NetError> {
        // Try HTTP/3 first
        if let Ok(resp) = self
            .try_h3_request(url, Method::Post(body.to_vec()), extra_headers)
            .await
        {
            return Ok(resp);
        }

        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| NetError::Http(format!("no host in URL: {url}")))?;
        let port = parsed.port().unwrap_or(443);

        // Browser-aware nav headers (Chrome may upgrade with high-entropy
        // Client Hints if origin sent Accept-CH; Firefox profiles skip).
        let accept_ch_upgraded = self.has_accept_ch(host).await;
        let mut hdrs = headers::nav_headers(&self.profile, accept_ch_upgraded);
        merge_headers(&mut hdrs, extra_headers);

        // Env-gated POST body dump (for sensor-payload diffing). Writes one
        // file per POST into BOXIDE_DUMP_POST_DIR with a numeric index, plus
        // a sidecar .meta.json holding the URL and request headers.
        if let Ok(dir) = std::env::var("BOXIDE_DUMP_POST_DIR") {
            use std::io::Write;
            let _ = std::fs::create_dir_all(&dir);
            let counter_path = format!("{}/.counter", dir);
            let next: usize = std::fs::read_to_string(&counter_path)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
                + 1;
            let _ = std::fs::write(&counter_path, next.to_string());
            let stem = format!("{}/{:03}", dir, next);
            if let Ok(mut f) = std::fs::File::create(format!("{stem}.body")) {
                let _ = f.write_all(body);
            }
            let mut meta = String::new();
            meta.push_str("{\n");
            meta.push_str(&format!(
                "  \"url\": {},\n",
                serde_json::to_string(url).unwrap_or_else(|_| "\"\"".into())
            ));
            meta.push_str(&format!("  \"body_len\": {},\n", body.len()));
            meta.push_str("  \"headers\": {\n");
            for (i, (k, v)) in hdrs.iter().enumerate() {
                let trailing = if i + 1 == hdrs.len() { "" } else { "," };
                meta.push_str(&format!(
                    "    {}: {}{}\n",
                    serde_json::to_string(k).unwrap_or_else(|_| "\"\"".into()),
                    serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into()),
                    trailing
                ));
            }
            meta.push_str("  }\n}\n");
            let _ = std::fs::write(format!("{stem}.meta.json"), meta);
        }

        if !has_header(&hdrs, "cookie") {
            let jar = self.cookies.lock().await;
            if let Some(cookie_str) = jar.cookies_for(&parsed) {
                insert_before_priority(&mut hdrs, "cookie".to_string(), cookie_str);
            }
        }

        // Inject Kasada x-kpsdk-cd if we have a session for this host.
        // The /tl POST itself doesn't need x-kpsdk-cd (it's the path that
        // *issues* the session), but POSTs after the initial token exchange
        // do — and we don't differentiate at this layer.
        if !has_header(&hdrs, "x-kpsdk-cd") {
            if let Some((k, v)) = self.kasada_cd_header(host).await {
                insert_before_priority(&mut hdrs, k, v);
            }
        }

        // Same stale-connection recovery as GET.
        let response = 'h2: {
            for attempt in 0..2 {
                let sender_res = self.get_sender(host, port).await;
                let mut sender = match sender_res {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[net] H2 connection failed for {}: {}", host, e);
                        break 'h2 None;
                    }
                };
                let uri = parsed.as_str();
                match h2_client::send_post(&mut sender, uri, host, &hdrs, body).await {
                    Ok((parts, resp_body)) => {
                        let resp = self.build_response(parts, resp_body, url).await?;
                        break 'h2 Some(resp);
                    }
                    Err(e) if attempt == 0 && is_stale_conn_error(&e) => {
                        self.pool.evict(host, port).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            None
        };

        let response = match response {
            Some(r) => r,
            None => {
                let tcp_stream = tcp::connect_via_proxy(
                    host,
                    port,
                    std::time::Duration::from_secs(10),
                    Some(&self.dns_cache),
                    self.proxy.as_ref(),
                )
                .await?;
                let mut tls_stream =
                    tls::connect_tls(&self.tls_connector, &self.profile, host, tcp_stream).await?;
                let path = match parsed.query() {
                    Some(q) => format!("{}?{}", parsed.path(), q),
                    None => parsed.path().to_string(),
                };
                let raw = h1_client::send_post(&mut tls_stream, host, &path, &hdrs, body).await?;
                self.build_response_from_raw(raw, url).await?
            }
        };

        // Store Set-Cookie from POST response — WBAAS sets x_wbaas_token here,
        // Kasada sets KP_UIDz / akm_bmfp_b2 session cookies here.
        // Also learn Kasada session: the /tl POST returns x-kpsdk-cr/st here.
        self.learn_kasada(host, &response.headers, Some(url)).await;
        self.learn_abck(host, &response.set_cookies).await;
        let upgrade = self.learn_accept_ch(host, &response.headers).await;
        self.store_set_cookies(&parsed, &response.set_cookies).await;

        let mut final_response = response;
        final_response.accept_ch_upgrade = upgrade;
        Ok(final_response)
    }

    /// Snapshot all cookies for a URL as a "name=value; name2=value2" string.
    /// Used by document.cookie getter to unify JS-visible cookies with the network jar.
    pub async fn cookies_for_url(&self, url: &Url) -> Option<String> {
        let jar = self.cookies.lock().await;
        jar.cookies_for(url)
    }

    /// Evict any pooled HTTP/2 connection to the given host. The next request
    /// will create a fresh TCP+TLS+H2 handshake. Used by challenge retries
    /// where the solver POSTs may have used an H2 session the server is now
    /// done with, or where the session has accumulated GOAWAY.
    pub async fn evict_connection(&self, host: &str, port: u16) {
        self.pool.evict(host, port).await;
    }

    /// Set cookies for a URL from a raw Set-Cookie-style string.
    /// Used by document.cookie setter.
    pub async fn set_cookie_str(&self, url: &Url, raw: &str) {
        let mut jar = self.cookies.lock().await;
        jar.set_cookies(url, &[raw.to_string()]);
    }

    /// Build a Response from HTTP/2 response parts and body.
    async fn build_response(
        &self,
        parts: http::response::Parts,
        body: Vec<u8>,
        url: &str,
    ) -> Result<Response, NetError> {
        let status = parts.status.as_u16();
        let status_text = parts.status.canonical_reason().unwrap_or("").to_string();

        // Split Set-Cookie out of the regular header map so multi-value
        // Set-Cookie headers aren't collapsed (HashMap would overwrite).
        let mut resp_headers = HashMap::new();
        let mut set_cookies = Vec::new();
        for (key, value) in &parts.headers {
            if let Ok(v) = value.to_str() {
                if key.as_str().eq_ignore_ascii_case("set-cookie") {
                    set_cookies.push(v.to_string());
                } else {
                    resp_headers.insert(key.to_string(), v.to_string());
                }
            }
        }

        // Decompress body
        let encoding = resp_headers
            .get("content-encoding")
            .map(|s| s.as_str())
            .unwrap_or("");
        let decompressed = compression::decompress(&body, encoding)?;

        Ok(Response {
            status,
            status_text,
            headers: resp_headers,
            set_cookies,
            body: decompressed,
            url: url.to_string(),
            accept_ch_upgrade: false,
            timings: TimingStats::default(),
        })
    }

    /// Build a Response from an HTTP/1.1 raw response.
    async fn build_response_from_raw(
        &self,
        raw: h1_client::RawResponse,
        url: &str,
    ) -> Result<Response, NetError> {
        let mut resp_headers = HashMap::new();
        let mut set_cookies = Vec::new();
        for (name, value) in &raw.headers {
            if name.eq_ignore_ascii_case("set-cookie") {
                set_cookies.push(value.clone());
            } else {
                resp_headers.insert(name.clone(), value.clone());
            }
        }

        let encoding = resp_headers
            .get("content-encoding")
            .map(|s| s.as_str())
            .unwrap_or("");
        let decompressed = compression::decompress(&raw.body, encoding)?;

        Ok(Response {
            status: raw.status,
            status_text: raw.status_text,
            headers: resp_headers,
            set_cookies,
            body: decompressed,
            url: url.to_string(),
            accept_ch_upgrade: false,
            timings: TimingStats::default(),
        })
    }
}

/// Detect whether an error indicates a stale/closed pooled connection that can
/// be safely retried by evicting from the pool and reconnecting. This catches
/// HTTP/2 GOAWAY ("not a result of an error"), broken pipe, and ResetStream.
fn is_stale_conn_error(e: &NetError) -> bool {
    let msg = e.to_string();
    msg.contains("not a result of an error")       // h2 GOAWAY / NO_ERROR
        || msg.contains("broken pipe")
        || msg.contains("connection closed")
        || msg.contains("ResetStream")
        || msg.contains("stream was reset")
        || msg.contains("HTTP/2 not ready")
}

/// Check if a header name is already present (case-insensitive).
fn has_header(hdrs: &[(String, String)], name: &str) -> bool {
    hdrs.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
}

fn insert_before_priority(hdrs: &mut Vec<(String, String)>, name: String, value: String) {
    if let Some(pos) = hdrs
        .iter()
        .position(|(k, _)| k.eq_ignore_ascii_case("priority"))
    {
        hdrs.insert(pos, (name, value));
    } else {
        hdrs.push((name, value));
    }
}

/// Merge extra headers into the base list. Existing headers with the same name
/// (case-insensitive) are replaced in place so order is preserved.
fn merge_headers(base: &mut Vec<(String, String)>, extra: &[(String, String)]) {
    for (k, v) in extra {
        // Skip pseudo-headers and forbidden fetch headers that would corrupt H2.
        let lower = k.to_ascii_lowercase();
        if lower.starts_with(':') || lower == "host" || lower == "connection" {
            continue;
        }
        if let Some(slot) = base.iter_mut().find(|(bk, _)| bk.eq_ignore_ascii_case(k)) {
            slot.1 = v.clone();
        } else {
            base.push((lower, v.clone()));
        }
    }
}

/// Resolve a redirect Location header to an absolute URL.
fn resolve_redirect(current_url: &str, location: &str) -> Result<String, NetError> {
    // RFC 3986 §5.2 — resolve `location` against `current_url` as base.
    // Url::join correctly handles all three cases:
    //   - absolute URL ("https://b.com/x")
    //   - root-relative ("/x")
    //   - relative ("x.html", "../y")
    // The previous impl returned `location` verbatim for the third case,
    // which then failed downstream "no host in URL" — caught on iphey.com
    // (holistic sweep 2026-05-10, FAILURE_ROOT_CAUSES.md bucket A).
    let base = Url::parse(current_url).map_err(|e| NetError::Request(e.to_string()))?;
    let resolved = base
        .join(location)
        .map_err(|e| NetError::Request(format!("redirect resolve: {e} (base={current_url}, loc={location})")))?;
    Ok(resolved.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_creates_successfully() {
        let profile = stealth::chrome_130_linux();
        let client = HttpClient::new(&profile);
        assert!(client.is_ok());
    }

    // K1 regression: with a live Kasada session present (compute_cd_header
    // WOULD return Some), kasada_cd_header still returns None by default
    // — the Rust PoW is deferred to the page's ips.js (nocdp-parity:
    // a real Chrome emits no parallel native cd). Opt-in env restores
    // the retained impl for the page-less client path.
    #[tokio::test]
    async fn k1_kasada_cd_deferred_to_ips_js_by_default() {
        let profile = stealth::chrome_130_linux();
        let client = HttpClient::new(&profile).expect("client");
        let host = "k1-test.example";
        let future_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
            + 1000)
            .to_string();
        let mut hm = std::collections::HashMap::new();
        hm.insert("x-kpsdk-cr".to_string(), "true".to_string());
        hm.insert("x-kpsdk-st".to_string(), future_ms);
        client.kasada_sessions().learn(host, &hm, None).await;

        // Session exists ⇒ compute_cd_header itself yields Some …
        assert!(
            client.kasada_sessions().compute_cd_header(host).await.is_some(),
            "precondition: a learned session must be cd-capable"
        );
        // … but K1 defers to ips.js: no parallel Rust cd by default.
        std::env::remove_var("BOXIDE_KASADA_RUST_CD");
        assert_eq!(
            client.kasada_cd_header(host).await,
            None,
            "K1: Rust x-kpsdk-cd must be deferred to the page's ips.js"
        );
        // Retained impl still reachable for the page-less path.
        std::env::set_var("BOXIDE_KASADA_RUST_CD", "1");
        assert!(
            client.kasada_cd_header(host).await.is_some(),
            "opt-in env must restore the retained Rust PoW path"
        );
        std::env::remove_var("BOXIDE_KASADA_RUST_CD");
    }

    #[test]
    fn redirect_resolve_handles_all_three_rfc3986_cases() {
        // Absolute — pass through.
        assert_eq!(
            resolve_redirect("https://a.com/x", "https://b.com/y").unwrap(),
            "https://b.com/y"
        );
        // Root-relative — replaces path on same origin.
        assert_eq!(
            resolve_redirect("https://a.com/x/y", "/z").unwrap(),
            "https://a.com/z"
        );
        // Relative-no-leading-slash — resolves against current path's
        // directory. THIS WAS THE IPHEY BUG (returned "z.html" verbatim
        // → "no host in URL" downstream).
        assert_eq!(
            resolve_redirect("https://a.com/x/y", "z.html").unwrap(),
            "https://a.com/x/z.html"
        );
        // Dot segments per RFC 3986 §5.2.4.
        assert_eq!(
            resolve_redirect("https://a.com/x/y/", "../z.html").unwrap(),
            "https://a.com/x/z.html"
        );
        // Scheme-relative.
        assert_eq!(
            resolve_redirect("https://a.com/x", "//b.com/y").unwrap(),
            "https://b.com/y"
        );
        // Query-only — preserves path.
        assert_eq!(
            resolve_redirect("https://a.com/x?old=1", "?new=2").unwrap(),
            "https://a.com/x?new=2"
        );
    }

    #[test]
    fn response_text() {
        let resp = Response {
            status: 200,
            status_text: "OK".into(),
            headers: HashMap::new(),
            set_cookies: Vec::new(),
            body: b"Hello world".to_vec(),
            url: "https://example.com".into(),
            accept_ch_upgrade: false,
            timings: TimingStats::default(),
        };
        assert_eq!(resp.text(), "Hello world");
        assert!(resp.ok());
    }

    #[test]
    fn response_not_ok() {
        let resp = Response {
            status: 404,
            status_text: "Not Found".into(),
            headers: HashMap::new(),
            set_cookies: Vec::new(),
            body: vec![],
            url: "https://example.com/missing".into(),
            accept_ch_upgrade: false,
            timings: TimingStats::default(),
        };
        assert!(!resp.ok());
    }

    #[tokio::test]
    #[ignore]
    async fn get_request() {
        let profile = stealth::chrome_130_linux();
        let client = HttpClient::new(&profile).unwrap();
        let resp = client.get("https://httpbin.org/get").await.unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.text().contains("httpbin"));
    }

    #[tokio::test]
    #[ignore]
    async fn get_ipv6_example_com() {
        let profile = stealth::chrome_130_linux();
        let client = HttpClient::new(&profile).unwrap();
        let resp = client.get("https://example.com").await.unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.text().contains("Example Domain"));
    }

    #[tokio::test]
    #[ignore]
    async fn get_hacker_news() {
        let profile = stealth::chrome_130_linux();
        let client = HttpClient::new(&profile).unwrap();
        let resp = client.get("https://news.ycombinator.com").await.unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.text().contains("Hacker News"));
    }

    #[tokio::test]
    #[ignore]
    async fn headers_include_ua() {
        let profile = stealth::chrome_130_windows();
        let client = HttpClient::new(&profile).unwrap();
        let resp = client.get("https://httpbin.org/headers").await.unwrap();
        let body = resp.text();
        assert!(
            body.contains("Chrome/130"),
            "Response should show our UA: {}",
            body
        );
    }

    #[tokio::test]
    async fn accept_ch_starts_false_then_true_after_learn() {
        let profile = stealth::chrome_130_windows();
        let client = HttpClient::new(&profile).unwrap();

        // No response seen yet → no Accept-CH for this origin.
        assert!(!client.has_accept_ch("example.com").await);

        // Simulate a response that includes Accept-CH.
        let mut headers = HashMap::new();
        headers.insert(
            "accept-ch".to_string(),
            "Sec-CH-UA-Full-Version-List, Sec-CH-UA-Platform-Version".to_string(),
        );
        client.learn_accept_ch("example.com", &headers).await;

        assert!(client.has_accept_ch("example.com").await);
        // Other origins are not affected.
        assert!(!client.has_accept_ch("other.com").await);
    }

    #[tokio::test]
    async fn accept_ch_header_name_is_case_insensitive() {
        let profile = stealth::chrome_130_linux();
        let client = HttpClient::new(&profile).unwrap();

        // Mixed-case header name (e.g. from an HTTP/1.1 server that sends
        // it with canonical capitalisation).
        let mut headers = HashMap::new();
        headers.insert("Accept-CH".to_string(), "Sec-CH-UA-Arch".to_string());
        client.learn_accept_ch("site.example", &headers).await;

        assert!(client.has_accept_ch("site.example").await);
    }

    #[tokio::test]
    async fn response_without_accept_ch_does_not_upgrade_origin() {
        let profile = stealth::chrome_130_linux();
        let client = HttpClient::new(&profile).unwrap();

        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "text/html".to_string());
        client.learn_accept_ch("boring.example", &headers).await;

        assert!(!client.has_accept_ch("boring.example").await);
    }
}
