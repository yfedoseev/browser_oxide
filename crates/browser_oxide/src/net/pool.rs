//! HTTP/2 connection pool keyed by (host, port).
//!
//! Reuses existing HTTP/2 connections for the same host via multiplexing.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http2::client::SendRequest;
use tokio::sync::Mutex;

/// Connection idle timeout — 5 minutes to avoid reconnection churn during scraping sessions.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// A pooled HTTP/2 sender.
struct PoolEntry {
    sender: SendRequest<Bytes>,
    last_used: Instant,
}

/// Connection pool for HTTP/2 multiplexing.
#[derive(Clone)]
pub struct ConnectionPool {
    inner: Arc<Mutex<HashMap<(String, u16), PoolEntry>>>,
}

impl ConnectionPool {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Try to get an existing HTTP/2 sender for the given host.
    /// Returns None if no connection exists or the existing one is stale.
    pub async fn get(&self, host: &str, port: u16) -> Option<SendRequest<Bytes>> {
        let mut pool = self.inner.lock().await;
        let key = (host.to_string(), port);

        if let Some(entry) = pool.get(&key) {
            if entry.last_used.elapsed() < IDLE_TIMEOUT {
                // Clone the sender (HTTP/2 senders are cloneable for multiplexing)
                return Some(entry.sender.clone());
            }
            // Stale connection, remove it
            pool.remove(&key);
        }
        None
    }

    /// Store an HTTP/2 sender in the pool.
    pub async fn put(&self, host: &str, port: u16, sender: SendRequest<Bytes>) {
        let mut pool = self.inner.lock().await;
        let key = (host.to_string(), port);
        pool.insert(
            key,
            PoolEntry {
                sender,
                last_used: Instant::now(),
            },
        );
    }

    /// Update the last-used timestamp for a connection.
    pub async fn touch(&self, host: &str, port: u16) {
        let mut pool = self.inner.lock().await;
        let key = (host.to_string(), port);
        if let Some(entry) = pool.get_mut(&key) {
            entry.last_used = Instant::now();
        }
    }

    /// Remove stale connections.
    pub async fn cleanup(&self) {
        let mut pool = self.inner.lock().await;
        pool.retain(|_, entry| entry.last_used.elapsed() < IDLE_TIMEOUT);
    }

    /// Explicitly evict a connection from the pool — used when we detect it
    /// has been closed by the server (GOAWAY / broken pipe / etc.).
    pub async fn evict(&self, host: &str, port: u16) {
        let mut pool = self.inner.lock().await;
        pool.remove(&(host.to_string(), port));
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
