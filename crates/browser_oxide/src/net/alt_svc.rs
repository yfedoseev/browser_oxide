//! Alt-Svc header parsing and caching for HTTP/3 discovery.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Entry in the Alt-Svc cache.
struct AltSvcEntry {
    port: u16,
    max_age: Duration,
    learned_at: Instant,
}

impl AltSvcEntry {
    fn is_expired(&self) -> bool {
        self.learned_at.elapsed() > self.max_age
    }
}

/// Thread-safe cache of hosts that support HTTP/3.
#[derive(Clone, Default)]
pub struct AltSvcCache {
    inner: Arc<RwLock<HashMap<String, AltSvcEntry>>>,
}

impl AltSvcCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Look up whether a host supports h3. Returns the port if cached and not expired.
    pub async fn lookup(&self, host: &str) -> Option<u16> {
        let cache = self.inner.read().await;
        let entry = cache.get(host)?;
        if entry.is_expired() {
            None
        } else {
            Some(entry.port)
        }
    }

    /// Insert or update h3 support for a host.
    pub async fn insert(&self, host: &str, port: u16, max_age: Duration) {
        let mut cache = self.inner.write().await;
        cache.insert(
            host.to_string(),
            AltSvcEntry {
                port,
                max_age,
                learned_at: Instant::now(),
            },
        );
    }
}

/// Parse an Alt-Svc header value for h3 support.
/// Example: `h3=":443"; ma=2592000, h3-29=":443"; ma=2592000`
/// Returns (port, max_age) for the first h3 entry found.
pub fn parse_alt_svc(header: &str) -> Option<(u16, Duration)> {
    for entry in header.split(',') {
        let entry = entry.trim();
        // Look for h3="..." (not h3-29 or other drafts)
        if !entry.starts_with("h3=") && !entry.starts_with("h3 =") {
            continue;
        }

        // Extract port from h3=":443"
        let port = extract_port(entry).unwrap_or(443);

        // Extract max-age from ma=N
        let max_age = extract_max_age(entry).unwrap_or(86400);

        return Some((port, Duration::from_secs(max_age)));
    }
    None
}

fn extract_port(entry: &str) -> Option<u16> {
    let start = entry.find("\":").map(|i| i + 2)?;
    let end = entry[start..].find('"').map(|i| start + i)?;
    entry[start..end].parse().ok()
}

fn extract_max_age(entry: &str) -> Option<u64> {
    let ma_pos = entry.find("ma=")?;
    let start = ma_pos + 3;
    let rest = &entry[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_alt_svc() {
        let (port, ma) = parse_alt_svc(r#"h3=":443"; ma=2592000"#).unwrap();
        assert_eq!(port, 443);
        assert_eq!(ma, Duration::from_secs(2592000));
    }

    #[test]
    fn parse_multiple_entries() {
        let (port, _) = parse_alt_svc(r#"h3-29=":443"; ma=2592000, h3=":443"; ma=86400"#).unwrap();
        assert_eq!(port, 443);
    }

    #[test]
    fn parse_custom_port() {
        let (port, _) = parse_alt_svc(r#"h3=":8443"; ma=3600"#).unwrap();
        assert_eq!(port, 8443);
    }

    #[test]
    fn parse_no_h3() {
        assert!(parse_alt_svc(r#"h2=":443"; ma=3600"#).is_none());
    }

    #[tokio::test]
    async fn cache_insert_and_lookup() {
        let cache = AltSvcCache::new();
        cache
            .insert("example.com", 443, Duration::from_secs(3600))
            .await;
        assert_eq!(cache.lookup("example.com").await, Some(443));
        assert_eq!(cache.lookup("other.com").await, None);
    }
}
