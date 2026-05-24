//! HTTP CONNECT and SOCKS5 proxy support.
//!
//! Phase 7 follow-up T1C — gives `StealthProfile.proxy` a real
//! implementation so we can route requests through residential
//! proxies / IP rotators when the direct path is reputation-blocked
//! (Cloudflare, DataDome, some Akamai).
//!
//! ## Supported schemes
//!
//! - `http://[user[:pass]@]host:port` — uses HTTP/1.1 `CONNECT host:port`
//!   tunneling. The TLS handshake then runs unchanged on the tunnel.
//! - `https://[user[:pass]@]host:port` — same as `http://` but the hop
//!   to the proxy is itself TLS-wrapped (TLS-in-TLS); we currently
//!   plain-CONNECT and warn — a tiny fraction of providers require it.
//! - `socks5://[user[:pass]@]host:port` — RFC 1928 SOCKS5 with optional
//!   RFC 1929 username/password auth.
//!
//! ## Selection
//!
//! Read in priority order:
//! 1. `BROWSER_OXIDE_PROXY` env var (override; useful for one-off CI runs)
//! 2. `StealthProfile.proxy` field on the active profile
//!
//! If both are absent, no proxy is used (direct connect via
//! `tcp::happy_eyeballs`).

use std::time::Duration;

use base64::Engine as _;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::NetError;
use crate::tcp::DnsCache;

#[derive(Debug, Clone, PartialEq)]
pub enum ProxyAuth {
    None,
    UserPass(String, String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProxyConfig {
    Http {
        host: String,
        port: u16,
        auth: ProxyAuth,
        /// True for `https://` proxy URLs (TLS-wrapped hop).
        tls: bool,
    },
    Socks5 {
        host: String,
        port: u16,
        auth: ProxyAuth,
    },
}

impl ProxyConfig {
    /// Resolve the active proxy config:
    ///
    /// 1. `BROWSER_OXIDE_PROXY` env var (override)
    /// 2. `profile_proxy` (the `StealthProfile.proxy` string)
    ///
    /// Returns `None` if neither is set.
    pub fn resolve(profile_proxy: Option<&str>) -> Result<Option<Self>, NetError> {
        if let Ok(env) = std::env::var("BROWSER_OXIDE_PROXY") {
            if !env.is_empty() {
                return Self::parse(&env).map(Some);
            }
        }
        match profile_proxy {
            Some(s) if !s.is_empty() => Self::parse(s).map(Some),
            _ => Ok(None),
        }
    }

    /// Parse a proxy URL string. Accepts the shapes documented at the
    /// top of this module.
    pub fn parse(s: &str) -> Result<Self, NetError> {
        let url = url::Url::parse(s)
            .map_err(|e| NetError::Http(format!("invalid proxy URL {s:?}: {e}")))?;

        let host = url
            .host_str()
            .ok_or_else(|| NetError::Http(format!("proxy URL {s:?} has no host")))?
            .to_string();
        // url.port() returns None when the port matches the scheme default.
        // For http/https/ws/wss the url crate resolves defaults; for
        // socks5/socks5h we apply the IANA-registered 1080 ourselves.
        let port = url
            .port()
            .or_else(|| match url.scheme() {
                "http" | "ws" => Some(80),
                "https" | "wss" => Some(443),
                "socks5" | "socks5h" => Some(1080),
                _ => None,
            })
            .ok_or_else(|| NetError::Http(format!("proxy URL {s:?} must include a port")))?;

        let auth = match (url.username(), url.password()) {
            ("", _) => ProxyAuth::None,
            (u, Some(p)) => ProxyAuth::UserPass(percent_decode(u)?, percent_decode(p)?),
            (u, None) => ProxyAuth::UserPass(percent_decode(u)?, String::new()),
        };

        match url.scheme() {
            "http" => Ok(Self::Http {
                host,
                port,
                auth,
                tls: false,
            }),
            "https" => Ok(Self::Http {
                host,
                port,
                auth,
                tls: true,
            }),
            "socks5" | "socks5h" => Ok(Self::Socks5 { host, port, auth }),
            other => Err(NetError::Http(format!(
                "unsupported proxy scheme {other:?} in {s:?} (use http://, https://, or socks5://)"
            ))),
        }
    }

    fn host(&self) -> &str {
        match self {
            Self::Http { host, .. } | Self::Socks5 { host, .. } => host,
        }
    }
    fn port(&self) -> u16 {
        match self {
            Self::Http { port, .. } | Self::Socks5 { port, .. } => *port,
        }
    }
}

fn percent_decode(s: &str) -> Result<String, NetError> {
    percent_encoding::percent_decode_str(s)
        .decode_utf8()
        .map(|c| c.into_owned())
        .map_err(|e| NetError::Http(format!("invalid percent-encoding in proxy auth: {e}")))
}

/// Connect to `target_host:target_port` via the given `proxy`. Returns
/// a TcpStream that's already tunneled to the target and ready for the
/// caller's TLS handshake (caller still passes target SNI to the TLS
/// layer; the tunnel is bytes-only).
pub async fn connect(
    target_host: &str,
    target_port: u16,
    timeout: Duration,
    dns_cache: Option<&DnsCache>,
    proxy: &ProxyConfig,
) -> Result<TcpStream, NetError> {
    // Connect to the proxy itself with happy-eyeballs + DNS cache.
    // (Proxy hops are direct TCP — no proxy-of-proxy chaining yet.)
    let mut stream =
        crate::tcp::connect_with_cache(proxy.host(), proxy.port(), timeout, dns_cache).await?;

    match proxy {
        ProxyConfig::Http { auth, tls, .. } => {
            if *tls {
                // TLS-in-TLS: most residential providers don't require
                // it. Emit a one-shot warning rather than silently failing
                // the handshake.
                eprintln!(
                    "[proxy] WARN: https:// proxy hop requested for {} but \
                     TLS-in-TLS isn't implemented; trying plain CONNECT \
                     (works for most providers)",
                    proxy.host()
                );
            }
            http_connect(&mut stream, target_host, target_port, auth).await?;
        }
        ProxyConfig::Socks5 { auth, .. } => {
            socks5_handshake(&mut stream, target_host, target_port, auth).await?;
        }
    }

    Ok(stream)
}

/// Send `CONNECT host:port HTTP/1.1` to the upstream proxy and read
/// the 200 response. The TCP stream is then bytes-pipe to the target.
async fn http_connect(
    stream: &mut TcpStream,
    target_host: &str,
    target_port: u16,
    auth: &ProxyAuth,
) -> Result<(), NetError> {
    let mut req = format!(
        "CONNECT {target_host}:{target_port} HTTP/1.1\r\nHost: {target_host}:{target_port}\r\n"
    );
    if let ProxyAuth::UserPass(user, pass) = auth {
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
        req.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    req.push_str("Proxy-Connection: keep-alive\r\n\r\n");

    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| NetError::Http(format!("proxy CONNECT write failed: {e}")))?;

    // Read response headers until \r\n\r\n. CONNECT replies fit easily
    // in a few hundred bytes; bound at 8 KiB to defeat a malicious proxy.
    let mut buf = Vec::with_capacity(512);
    let mut tmp = [0u8; 256];
    loop {
        let n = stream
            .read(&mut tmp)
            .await
            .map_err(|e| NetError::Http(format!("proxy CONNECT read failed: {e}")))?;
        if n == 0 {
            return Err(NetError::Http(
                "proxy CONNECT: server closed before response".into(),
            ));
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            return Err(NetError::Http("proxy CONNECT: response too large".into()));
        }
    }

    let head = String::from_utf8_lossy(&buf);
    let status_line = head.lines().next().unwrap_or_default();
    // Expect "HTTP/1.1 200 ..." (or HTTP/1.0). Anything else is a failure.
    let ok = status_line
        .split_whitespace()
        .nth(1)
        .map(|c| c == "200")
        .unwrap_or(false);
    if !ok {
        return Err(NetError::Http(format!(
            "proxy CONNECT denied: {status_line}"
        )));
    }
    Ok(())
}

/// RFC 1928 SOCKS5 + RFC 1929 user/pass auth.
async fn socks5_handshake(
    stream: &mut TcpStream,
    target_host: &str,
    target_port: u16,
    auth: &ProxyAuth,
) -> Result<(), NetError> {
    // === Phase 1: greeting ===
    // VER=5, NMETHODS, METHODS...
    let methods: &[u8] = match auth {
        ProxyAuth::None => &[0x00],           // NO AUTH
        ProxyAuth::UserPass(_, _) => &[0x02], // USER/PASS
    };
    let mut greet = vec![0x05, methods.len() as u8];
    greet.extend_from_slice(methods);
    stream
        .write_all(&greet)
        .await
        .map_err(|e| NetError::Http(format!("SOCKS5 greet write failed: {e}")))?;

    let mut resp = [0u8; 2];
    stream
        .read_exact(&mut resp)
        .await
        .map_err(|e| NetError::Http(format!("SOCKS5 greet read failed: {e}")))?;
    if resp[0] != 0x05 {
        return Err(NetError::Http(format!(
            "SOCKS5 unexpected version {}",
            resp[0]
        )));
    }
    if resp[1] == 0xFF {
        return Err(NetError::Http(
            "SOCKS5 server rejected all auth methods".into(),
        ));
    }

    // === Phase 2: auth (if selected) ===
    if resp[1] == 0x02 {
        let (user, pass) = match auth {
            ProxyAuth::UserPass(u, p) => (u.as_bytes(), p.as_bytes()),
            _ => {
                return Err(NetError::Http(
                    "SOCKS5 server selected USER/PASS but no creds configured".into(),
                ));
            }
        };
        if user.len() > 255 || pass.len() > 255 {
            return Err(NetError::Http(
                "SOCKS5 USER/PASS fields must each be ≤255 bytes".into(),
            ));
        }
        let mut auth_req = vec![0x01, user.len() as u8];
        auth_req.extend_from_slice(user);
        auth_req.push(pass.len() as u8);
        auth_req.extend_from_slice(pass);
        stream
            .write_all(&auth_req)
            .await
            .map_err(|e| NetError::Http(format!("SOCKS5 auth write failed: {e}")))?;
        let mut auth_resp = [0u8; 2];
        stream
            .read_exact(&mut auth_resp)
            .await
            .map_err(|e| NetError::Http(format!("SOCKS5 auth read failed: {e}")))?;
        if auth_resp[1] != 0x00 {
            return Err(NetError::Http(format!(
                "SOCKS5 auth denied (status={})",
                auth_resp[1]
            )));
        }
    }

    // === Phase 3: CONNECT request ===
    // VER=5, CMD=1 (CONNECT), RSV=0, ATYP, DST.ADDR, DST.PORT
    // Use ATYP=3 (DOMAINNAME) so the proxy resolves DNS — better for IP
    // rotation flows where target_host is e.g. "etsy.com" not an IP.
    if target_host.len() > 255 {
        return Err(NetError::Http(format!(
            "SOCKS5 DOMAINNAME must be ≤255 bytes, got {}",
            target_host.len()
        )));
    }
    let mut req = vec![0x05, 0x01, 0x00, 0x03, target_host.len() as u8];
    req.extend_from_slice(target_host.as_bytes());
    req.extend_from_slice(&target_port.to_be_bytes());
    stream
        .write_all(&req)
        .await
        .map_err(|e| NetError::Http(format!("SOCKS5 CONNECT write failed: {e}")))?;

    // Reply: VER, REP, RSV, ATYP, BND.ADDR, BND.PORT
    let mut head = [0u8; 4];
    stream
        .read_exact(&mut head)
        .await
        .map_err(|e| NetError::Http(format!("SOCKS5 CONNECT read failed: {e}")))?;
    if head[0] != 0x05 {
        return Err(NetError::Http(format!(
            "SOCKS5 unexpected reply version {}",
            head[0]
        )));
    }
    if head[1] != 0x00 {
        return Err(NetError::Http(format!(
            "SOCKS5 CONNECT denied (rep=0x{:02x})",
            head[1]
        )));
    }
    // Drain BND.ADDR + BND.PORT so the stream is positioned at the start
    // of the tunneled bytes.
    let bnd_len = match head[3] {
        0x01 => 4,  // IPv4
        0x04 => 16, // IPv6
        0x03 => {
            let mut len_buf = [0u8; 1];
            stream
                .read_exact(&mut len_buf)
                .await
                .map_err(|e| NetError::Http(format!("SOCKS5 BND read failed: {e}")))?;
            len_buf[0] as usize
        }
        other => {
            return Err(NetError::Http(format!(
                "SOCKS5 unknown ATYP 0x{other:02x} in CONNECT reply"
            )));
        }
    };
    let mut bnd = vec![0u8; bnd_len + 2];
    stream
        .read_exact(&mut bnd)
        .await
        .map_err(|e| NetError::Http(format!("SOCKS5 BND tail read failed: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_http_no_auth() {
        let p = ProxyConfig::parse("http://proxy.example.com:8080").unwrap();
        assert_eq!(
            p,
            ProxyConfig::Http {
                host: "proxy.example.com".into(),
                port: 8080,
                auth: ProxyAuth::None,
                tls: false,
            }
        );
    }

    #[test]
    fn parse_socks5_with_auth() {
        let p = ProxyConfig::parse("socks5://alice:s3cret@residential.example.com:1080").unwrap();
        assert_eq!(
            p,
            ProxyConfig::Socks5 {
                host: "residential.example.com".into(),
                port: 1080,
                auth: ProxyAuth::UserPass("alice".into(), "s3cret".into()),
            }
        );
    }

    #[test]
    fn parse_https_proxy() {
        let p = ProxyConfig::parse("https://proxy.example.com:443").unwrap();
        match p {
            ProxyConfig::Http { tls, .. } => assert!(tls),
            _ => panic!("expected Http with tls=true"),
        }
    }

    #[test]
    fn parse_rejects_unknown_scheme() {
        assert!(ProxyConfig::parse("ftp://proxy:21").is_err());
    }

    #[test]
    fn parse_uses_scheme_default_port() {
        // http://...without :port → 80 (scheme default).
        let p = ProxyConfig::parse("http://proxy.example.com").unwrap();
        match p {
            ProxyConfig::Http { port, .. } => assert_eq!(port, 80),
            _ => panic!("expected Http"),
        }
        // socks5 default is 1080.
        let p = ProxyConfig::parse("socks5://proxy.example.com").unwrap();
        match p {
            ProxyConfig::Socks5 { port, .. } => assert_eq!(port, 1080),
            _ => panic!("expected Socks5"),
        }
    }

    #[test]
    fn parse_percent_decoded_password() {
        // Password with a colon needs percent-encoding in the URL.
        let p = ProxyConfig::parse("http://u:a%3Ab@proxy:8080").unwrap();
        let auth = match &p {
            ProxyConfig::Http { auth, .. } => auth.clone(),
            _ => panic!(),
        };
        assert_eq!(auth, ProxyAuth::UserPass("u".into(), "a:b".into()));
    }

    #[test]
    fn resolve_env_overrides_profile() {
        // Save and restore env to avoid affecting other tests.
        let saved = std::env::var("BROWSER_OXIDE_PROXY").ok();
        std::env::set_var("BROWSER_OXIDE_PROXY", "socks5://env.example:1080");
        let r = ProxyConfig::resolve(Some("http://profile.example:8080")).unwrap();
        match r {
            Some(ProxyConfig::Socks5 { host, .. }) => assert_eq!(host, "env.example"),
            other => panic!("expected env override Socks5, got {other:?}"),
        }
        match saved {
            Some(v) => std::env::set_var("BROWSER_OXIDE_PROXY", v),
            None => std::env::remove_var("BROWSER_OXIDE_PROXY"),
        }
    }

    /// Integration test: spin up a fake SOCKS5 proxy server in-process,
    /// drive a connect through it, and verify the bytes that arrive at
    /// the proxy match RFC 1928. Catches handshake drift end-to-end
    /// without depending on a live external proxy.
    #[tokio::test]
    async fn socks5_handshake_roundtrip() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_port = listener.local_addr().unwrap().port();

        // Fake SOCKS5 proxy: greet, accept NO_AUTH, send CONNECT reply
        // success (BND.ADDR=0.0.0.0:0). Then close.
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            // Greet: VER=5, NMETHODS=1, METHOD=NO_AUTH(0)
            let mut greet = [0u8; 3];
            sock.read_exact(&mut greet).await.unwrap();
            assert_eq!(greet, [0x05, 0x01, 0x00]);
            // Reply: VER=5, METHOD=NO_AUTH(0)
            sock.write_all(&[0x05, 0x00]).await.unwrap();
            // CONNECT: VER=5, CMD=1, RSV=0, ATYP=3, LEN, host bytes, port (BE)
            let mut head = [0u8; 5];
            sock.read_exact(&mut head).await.unwrap();
            assert_eq!(head[0], 0x05);
            assert_eq!(head[1], 0x01); // CONNECT
            assert_eq!(head[3], 0x03); // DOMAINNAME
            let host_len = head[4] as usize;
            let mut host_buf = vec![0u8; host_len + 2];
            sock.read_exact(&mut host_buf).await.unwrap();
            let host_str = std::str::from_utf8(&host_buf[..host_len])
                .unwrap()
                .to_string();
            assert_eq!(host_str, "target.example.com");
            // Reply: VER=5, REP=0 (SUCCESS), RSV=0, ATYP=1, BND.ADDR=0.0.0.0, BND.PORT=0
            sock.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .unwrap();
        });

        let proxy = ProxyConfig::Socks5 {
            host: "127.0.0.1".into(),
            port: proxy_port,
            auth: ProxyAuth::None,
        };
        let _stream = connect(
            "target.example.com",
            443,
            std::time::Duration::from_secs(2),
            None,
            &proxy,
        )
        .await
        .expect("SOCKS5 round-trip should succeed");
        server.await.unwrap();
    }

    #[test]
    fn resolve_profile_when_no_env() {
        let saved = std::env::var("BROWSER_OXIDE_PROXY").ok();
        std::env::remove_var("BROWSER_OXIDE_PROXY");
        let r = ProxyConfig::resolve(Some("http://profile.example:8080")).unwrap();
        assert!(matches!(r, Some(ProxyConfig::Http { .. })));
        let r = ProxyConfig::resolve(None).unwrap();
        assert!(r.is_none());
        let r = ProxyConfig::resolve(Some("")).unwrap();
        assert!(r.is_none());
        if let Some(v) = saved {
            std::env::set_var("BROWSER_OXIDE_PROXY", v);
        }
    }
}
