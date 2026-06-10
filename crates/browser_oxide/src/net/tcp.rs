//! Happy Eyeballs (RFC 6555) TCP connection over tokio.
//!
//! Resolves DNS to both IPv4 and IPv6 addresses, tries IPv6 first,
//! and falls back to IPv4 after 250ms if IPv6 hasn't connected yet.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::net::error::NetError;

/// Default connection timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Happy Eyeballs delay: how long to wait for IPv6 before starting IPv4.
const HAPPY_EYEBALLS_DELAY: Duration = Duration::from_millis(250);

/// DNS cache TTL.
const DNS_TTL: Duration = Duration::from_secs(300);

/// In-memory DNS cache to avoid repeated lookups for the same host.
#[derive(Clone, Default)]
pub struct DnsCache {
    inner: Arc<Mutex<HashMap<String, DnsEntry>>>,
}

struct DnsEntry {
    addrs: Vec<SocketAddr>,
    resolved_at: Instant,
}

impl DnsCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a host, returning cached results if available and fresh.
    pub async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, NetError> {
        // Check cache
        {
            let cache = self.inner.lock().await;
            if let Some(entry) = cache.get(host) {
                if entry.resolved_at.elapsed() < DNS_TTL {
                    return Ok(entry
                        .addrs
                        .iter()
                        .map(|a| SocketAddr::new(a.ip(), port))
                        .collect());
                }
            }
        }

        // Cache miss — resolve
        let addr_str = format!("{host}:{port}");
        let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&addr_str)
            .await
            .map_err(|e| NetError::Tcp(format!("DNS lookup failed for {host}: {e}")))?
            .collect();

        // Store in cache
        {
            let mut cache = self.inner.lock().await;
            cache.insert(
                host.to_string(),
                DnsEntry {
                    addrs: addrs.clone(),
                    resolved_at: Instant::now(),
                },
            );
        }

        Ok(addrs)
    }
}

/// Connect to a host using Happy Eyeballs (RFC 6555).
///
/// Resolves DNS (with optional cache), tries IPv6 first, falls back to IPv4 after 250ms.
/// Sets TCP_NODELAY on the connected socket.
pub async fn connect(host: &str, port: u16, timeout: Duration) -> Result<TcpStream, NetError> {
    connect_with_cache(host, port, timeout, None).await
}

/// Connect to `host:port` via the configured proxy, if any. Falls back
/// to a direct connect when `proxy` is `None`. Phase 7 follow-up T1C —
/// gives `StealthProfile.proxy` a real implementation path.
pub async fn connect_via_proxy(
    host: &str,
    port: u16,
    timeout: Duration,
    dns_cache: Option<&DnsCache>,
    proxy: Option<&crate::net::proxy::ProxyConfig>,
) -> Result<TcpStream, NetError> {
    if let Some(proxy) = proxy {
        return tokio::time::timeout(
            timeout,
            crate::net::proxy::connect(host, port, timeout, dns_cache, proxy),
        )
        .await
        .map_err(|_| {
            NetError::Tcp(format!(
                "proxy connect to {host}:{port} timed out (timeout={}s)",
                timeout.as_secs()
            ))
        })?;
    }
    connect_with_cache(host, port, timeout, dns_cache).await
}

/// Connect with an optional DNS cache and OS-specific TCP settings.
pub async fn connect_with_cache(
    host: &str,
    port: u16,
    timeout: Duration,
    dns_cache: Option<&DnsCache>,
) -> Result<TcpStream, NetError> {
    let addrs: Vec<SocketAddr> = if let Some(cache) = dns_cache {
        cache.resolve(host, port).await?
    } else {
        let addr_str = format!("{host}:{port}");
        let resolved: Vec<SocketAddr> = tokio::net::lookup_host(&addr_str)
            .await
            .map_err(|e| NetError::Tcp(format!("DNS lookup failed for {host}: {e}")))?
            .collect();
        resolved
    };

    if addrs.is_empty() {
        return Err(NetError::Tcp(format!("no addresses found for {host}")));
    }

    // Separate IPv6 and IPv4 addresses
    let mut ipv6: Vec<SocketAddr> = Vec::new();
    let mut ipv4: Vec<SocketAddr> = Vec::new();
    for addr in &addrs {
        if addr.is_ipv6() {
            ipv6.push(*addr);
        } else {
            ipv4.push(*addr);
        }
    }

    let stream = tokio::time::timeout(timeout, happy_eyeballs(&ipv6, &ipv4))
        .await
        .map_err(|_| NetError::Tcp(format!("connection to {host}:{port} timed out")))??;

    // Enable TCP_NODELAY for lower latency and set TTL
    stream
        .set_nodelay(true)
        .map_err(|e| NetError::Tcp(format!("failed to set TCP_NODELAY: {e}")))?;

    Ok(stream)
}

/// Convenience function using the default timeout.
pub async fn connect_default(host: &str, port: u16) -> Result<TcpStream, NetError> {
    connect(host, port, DEFAULT_TIMEOUT).await
}

/// Happy Eyeballs: try IPv6 first, start IPv4 after a delay.
async fn happy_eyeballs(ipv6: &[SocketAddr], ipv4: &[SocketAddr]) -> Result<TcpStream, NetError> {
    // If only one family is available, just try that
    if ipv6.is_empty() {
        return try_addrs(ipv4).await;
    }
    if ipv4.is_empty() {
        return try_addrs(ipv6).await;
    }

    // Try IPv6 first with a timeout, then race with IPv4
    tokio::select! {
        result = try_addrs(ipv6) => {
            match result {
                Ok(stream) => Ok(stream),
                // IPv6 failed entirely, try IPv4
                Err(_) => try_addrs(ipv4).await,
            }
        }
        _ = tokio::time::sleep(HAPPY_EYEBALLS_DELAY) => {
            // IPv6 is taking too long, race both
            tokio::select! {
                result = try_addrs(ipv6) => {
                    match result {
                        Ok(stream) => Ok(stream),
                        Err(_) => try_addrs(ipv4).await,
                    }
                }
                result = try_addrs(ipv4) => {
                    result
                }
            }
        }
    }
}

/// Try connecting to each address in sequence, return first success.
async fn try_addrs(addrs: &[SocketAddr]) -> Result<TcpStream, NetError> {
    let mut last_err = None;
    for addr in addrs {
        match TcpStream::connect(addr).await {
            Ok(stream) => return Ok(stream),
            Err(e) => last_err = Some(e),
        }
    }
    Err(NetError::Tcp(format!(
        "all addresses failed: {}",
        last_err.map_or_else(|| "no addresses".to_string(), |e| e.to_string())
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // requires network
    async fn connect_ipv4_httpbin() {
        let stream = connect("httpbin.org", 443, Duration::from_secs(10)).await;
        assert!(stream.is_ok(), "Failed to connect: {:?}", stream.err());
    }

    #[tokio::test]
    #[ignore] // requires network
    async fn connect_ipv6_example_com() {
        let stream = connect("example.com", 443, Duration::from_secs(10)).await;
        assert!(stream.is_ok(), "Failed to connect: {:?}", stream.err());
    }

    #[tokio::test]
    async fn connect_invalid_host() {
        let result = connect(
            "this.host.does.not.exist.example",
            443,
            Duration::from_secs(2),
        )
        .await;
        assert!(result.is_err());
    }
}
