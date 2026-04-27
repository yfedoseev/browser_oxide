//! Per-origin Kasada session state.
//!
//! When a server's response includes Kasada headers (`x-kpsdk-st`,
//! `x-kpsdk-cr`), we cache an entry keyed by host. On subsequent requests
//! to that host, the client computes a fresh `x-kpsdk-cd` PoW header from
//! the cached state and the current wall clock.
//!
//! See `crates/stealth/src/kasada.rs` for the PoW algorithm and
//! `docs/TIER0_KASADA_RESULTS.md` for the diagnostic that motivated this.

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::collections::HashMap;
use std::sync::Arc;
use stealth::kasada::{
    generate_session_id, solve_default, solve_with_realistic_duration, KasadaSolution,
};
use tokio::sync::RwLock;

/// One Kasada session per origin host. We carry the server-time offset
/// (so `workTime` aligns with Kasada's clock) and a stable session id
/// (Kasada appears to tolerate either a fresh id per request or a stable
/// one — empirical, may need adjustment per deployment).
#[derive(Debug, Clone)]
struct KasadaSession {
    /// Server clock minus local clock at the time we observed `x-kpsdk-st`,
    /// in milliseconds. Used to derive `workTime = local_now + offset`.
    server_offset_ms: i64,
    /// Last observed `x-kpsdk-st` value (server timestamp ms). Echoed back
    /// in the `st` field of the `x-kpsdk-cd` JSON — some Kasada deployments
    /// require this for clock-validation.
    server_st_ms: i64,
    /// Cached session id (32 lowercase hex chars).
    id: String,
    /// Token from the `x-kpsdk-fc` response header of the `/mfc` endpoint
    /// fetch (Hyper-Solutions "Flow 2" — see
    /// <https://docs.hypersolutions.co/k4sada/flow-2-fingerprint-endpoint>).
    /// Stricter Kasada tenants (canadagoose, hyatt, VEVE — sharing the
    /// `149e9513.../2d206a39...` template) require us to fetch /mfc and
    /// echo this header on subsequent protected requests.
    fc_token: Option<String>,
    /// Session token from the `x-kpsdk-ct` response header of the `/tl`
    /// POST. **Required** as a request HEADER on subsequent navigation
    /// GETs to the same host — per Hyper-Solutions Go SDK docs, "cookies
    /// alone are not enough; the ct header binds the session". Without
    /// this, the post-PoW retry returns the same Kasada init page even
    /// with a valid x-kpsdk-cd PoW solution. Verified 2026-04-27 on
    /// hyatt.com — /tl returns 200 + x-kpsdk-ct, but retry without
    /// echoing x-kpsdk-ct keeps getting the 737-byte init page.
    ct_token: Option<String>,
    /// Tenant-prefix path discovered from the /tl POST URL — same prefix
    /// used for /mfc (e.g. `/149e9513-01fa-4fb0-aad4-566afd725d1b/2d206a39-8ed7-437e-a3be-862e0f06eea3`).
    tenant_prefix: Option<String>,
}

#[derive(Clone, Default)]
pub struct KasadaSessionStore {
    inner: Arc<RwLock<HashMap<String, KasadaSession>>>,
}

impl KasadaSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Learn a Kasada session from a response's headers. Looks for
    /// `x-kpsdk-st` (server timestamp in ms) and computes the offset
    /// against the local clock now.
    ///
    /// `tl_url`, when provided, is the URL the response came from. If it
    /// looks like a Kasada `/tl` endpoint (path ending in `/tl`), we
    /// extract the tenant prefix so we can later fetch `/mfc` on the
    /// same path.
    pub async fn learn(
        &self,
        host: &str,
        headers: &HashMap<String, String>,
        tl_url: Option<&str>,
    ) {
        // Header lookup is case-insensitive in HTTP; our Response headers
        // are stored lower-cased per the existing convention.
        let server_st = headers
            .get("x-kpsdk-st")
            .and_then(|v| v.parse::<i64>().ok());
        let cr = headers.get("x-kpsdk-cr").map(|v| v.as_str()).unwrap_or("");
        if cr != "true" || server_st.is_none() {
            return;
        }
        let server_ms = server_st.unwrap();
        let local_ms = now_unix_ms();
        let offset = server_ms - local_ms;

        // Try to extract the Kasada tenant prefix (everything before /tl).
        // Example: `https://www.canadagoose.com/149e9513-.../2d206a39-.../tl?...`
        // → tenant_prefix = `/149e9513-.../2d206a39-...`
        let extracted_tenant_prefix: Option<String> = tl_url.and_then(|u| {
            url::Url::parse(u).ok().and_then(|parsed| {
                let path = parsed.path();
                path.strip_suffix("/tl").map(|p| p.to_string())
            })
        });

        let mut store = self.inner.write().await;
        // Only generate a new session id if we haven't seen this host yet.
        // Reusing the id across solves matches the public solver behavior
        // (per Humphryyy/Kasada-Deobfuscated `makeId()` is called once per
        // page session, not once per request).
        let entry = store.entry(host.to_string()).or_insert_with(|| {
            let mut rng = ChaCha20Rng::from_entropy();
            KasadaSession {
                server_offset_ms: offset,
                server_st_ms: server_ms,
                id: generate_session_id(&mut rng),
                fc_token: None,
                ct_token: None,
                tenant_prefix: None,
            }
        });
        // Refresh on every observation — clocks drift, server time moves on.
        entry.server_offset_ms = offset;
        entry.server_st_ms = server_ms;
        if entry.tenant_prefix.is_none() {
            entry.tenant_prefix = extracted_tenant_prefix;
        }
        // Cache x-kpsdk-ct (session token from /tl response). Required as
        // a request header on subsequent same-host navigations.
        if let Some(ct) = headers.get("x-kpsdk-ct") {
            eprintln!(
                "[kasada] LEARNED x-kpsdk-ct for {} (len={})",
                host,
                ct.len()
            );
            entry.ct_token = Some(ct.clone());
        } else {
            eprintln!(
                "[kasada] no x-kpsdk-ct in response from {} (header keys: {:?})",
                host,
                headers.keys().filter(|k| k.starts_with("x-kp")).collect::<Vec<_>>()
            );
        }
    }

    /// Returns the cached `x-kpsdk-ct` session token for `host`, if any.
    pub async fn ct_header(&self, host: &str) -> Option<(String, String)> {
        let store = self.inner.read().await;
        store
            .get(host)
            .and_then(|s| s.ct_token.clone())
            .map(|ct| ("x-kpsdk-ct".to_string(), ct))
    }

    /// Returns `(tenant_prefix, fc_already_known)` for a host so the HTTP
    /// client can decide whether to fetch /mfc. None if we have no session.
    pub async fn mfc_target(&self, host: &str) -> Option<(String, bool)> {
        let store = self.inner.read().await;
        let session = store.get(host)?;
        let prefix = session.tenant_prefix.clone()?;
        Some((prefix, session.fc_token.is_some()))
    }

    /// Stash the `x-kpsdk-fc` token from a /mfc response.
    pub async fn store_fc(&self, host: &str, fc: String) {
        let mut store = self.inner.write().await;
        if let Some(session) = store.get_mut(host) {
            session.fc_token = Some(fc);
        }
    }

    /// Returns the cached `x-kpsdk-fc` token for `host`, if any.
    pub async fn fc_header(&self, host: &str) -> Option<(String, String)> {
        let store = self.inner.read().await;
        store
            .get(host)
            .and_then(|s| s.fc_token.clone())
            .map(|fc| ("x-kpsdk-fc".to_string(), fc))
    }

    /// Compute an `x-kpsdk-cd` header value for `host`. Returns `None` if
    /// we have no Kasada session for that host. Each call solves a fresh
    /// PoW and stamps the **real wall-clock solve duration** into the
    /// `duration` field (per Apr 2026 research — Kasada validates duration
    /// against PoW difficulty, not against an injected random).
    ///
    /// Echoes back `x-kpsdk-st` as `st` and computes `rst = st + duration`
    /// (the request-send-time per the deobfuscated ips.js source). Required
    /// by stricter Kasada tenants like the `149e9513.../2d206a39...`
    /// template (canadagoose, hyatt, VEVE).
    pub async fn compute_cd_header(&self, host: &str) -> Option<String> {
        let store = self.inner.read().await;
        let session = store.get(host)?;
        let work_time = now_unix_ms() + session.server_offset_ms;
        // Use the base solver — it now measures real wall-clock duration.
        let mut solution: KasadaSolution = solve_default(work_time, &session.id);
        solution.st = Some(session.server_st_ms);
        // rst = server-time when client *sent* the protected request =
        // st + duration (so far the time we spent solving).
        if let Some(dur) = solution.duration {
            solution.rst = Some((session.server_st_ms + dur as i64) as f64);
        }
        Some(solution.to_header_value())
    }

    /// Test-only deterministic variant: solves with a fixed `duration` so
    /// repeated calls produce identical headers. Production should always
    /// use [`compute_cd_header`].
    #[cfg(test)]
    pub async fn compute_cd_header_deterministic(&self, host: &str) -> Option<String> {
        let store = self.inner.read().await;
        let session = store.get(host)?;
        let work_time = now_unix_ms() + session.server_offset_ms;
        let solution: KasadaSolution = solve_default(work_time, &session.id);
        Some(solution.to_header_value())
    }

    /// Whether we have a Kasada session for this host.
    pub async fn has_session(&self, host: &str) -> bool {
        self.inner.read().await.contains_key(host)
    }

    #[cfg(test)]
    async fn debug_session_count(&self) -> usize {
        self.inner.read().await.len()
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[tokio::test]
    async fn learns_session_when_x_kpsdk_cr_true() {
        let store = KasadaSessionStore::new();
        let now_ms = now_unix_ms();
        let server_ms = now_ms + 12_345; // simulate 12s server lead
        let headers = h(&[
            ("x-kpsdk-cr", "true"),
            ("x-kpsdk-st", &server_ms.to_string()),
            ("x-kpsdk-ct", "<token>"),
        ]);
        store.learn("www.canadagoose.com", &headers, None).await;
        assert!(store.has_session("www.canadagoose.com").await);
        assert_eq!(store.debug_session_count().await, 1);
    }

    #[tokio::test]
    async fn ignores_response_without_cr_true() {
        let store = KasadaSessionStore::new();
        let headers = h(&[
            ("x-kpsdk-cr", "false"),
            ("x-kpsdk-st", "1777000000000"),
        ]);
        store.learn("example.com", &headers, None).await;
        assert!(!store.has_session("example.com").await);
    }

    #[tokio::test]
    async fn ignores_response_without_st() {
        let store = KasadaSessionStore::new();
        let headers = h(&[("x-kpsdk-cr", "true")]);
        store.learn("example.com", &headers, None).await;
        assert!(!store.has_session("example.com").await);
    }

    #[tokio::test]
    async fn compute_cd_header_returns_valid_json_after_learning() {
        let store = KasadaSessionStore::new();
        let server_ms = now_unix_ms() + 1000;
        let headers = h(&[
            ("x-kpsdk-cr", "true"),
            ("x-kpsdk-st", &server_ms.to_string()),
        ]);
        store.learn("foo.com", &headers, None).await;
        let cd = store.compute_cd_header("foo.com").await.expect("session");
        let parsed: serde_json::Value = serde_json::from_str(&cd).unwrap();
        assert!(parsed["workTime"].is_i64());
        assert!(parsed["id"].is_string());
        assert_eq!(parsed["id"].as_str().unwrap().len(), 32);
        assert_eq!(parsed["answers"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn compute_cd_header_returns_none_for_unknown_host() {
        let store = KasadaSessionStore::new();
        assert!(store.compute_cd_header("never-seen.com").await.is_none());
    }

    #[tokio::test]
    async fn session_id_stable_across_calls_for_same_host() {
        // Reuse pattern: id generated once, retained across multiple
        // header-learn invocations + multiple compute_cd_header calls.
        let store = KasadaSessionStore::new();
        let server_ms = now_unix_ms() + 1000;
        let headers = h(&[
            ("x-kpsdk-cr", "true"),
            ("x-kpsdk-st", &server_ms.to_string()),
        ]);
        store.learn("a.com", &headers, None).await;
        let cd1 = store.compute_cd_header_deterministic("a.com").await.unwrap();
        store.learn("a.com", &headers, None).await; // re-learn shouldn't change id
        let cd2 = store.compute_cd_header_deterministic("a.com").await.unwrap();

        let p1: serde_json::Value = serde_json::from_str(&cd1).unwrap();
        let p2: serde_json::Value = serde_json::from_str(&cd2).unwrap();
        assert_eq!(p1["id"], p2["id"], "session id must be stable per host");
    }

    #[tokio::test]
    async fn compute_cd_header_includes_duration() {
        let store = KasadaSessionStore::new();
        let server_ms = now_unix_ms() + 1000;
        let headers = h(&[
            ("x-kpsdk-cr", "true"),
            ("x-kpsdk-st", &server_ms.to_string()),
        ]);
        store.learn("with-duration.com", &headers, None).await;
        let cd = store
            .compute_cd_header("with-duration.com")
            .await
            .expect("session");
        let parsed: serde_json::Value = serde_json::from_str(&cd).unwrap();
        // Duration is real wall-clock solve time. For difficulty=10 on a
        // modern laptop it's typically 0-5 ms; we just assert it's present.
        assert!(parsed["duration"].is_u64(), "duration field must be present");
    }

    #[tokio::test]
    async fn compute_cd_header_includes_st_and_rst() {
        let store = KasadaSessionStore::new();
        let server_ms = now_unix_ms() + 1000;
        let headers = h(&[
            ("x-kpsdk-cr", "true"),
            ("x-kpsdk-st", &server_ms.to_string()),
        ]);
        store.learn("with-rst.com", &headers, None).await;
        let cd = store.compute_cd_header("with-rst.com").await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&cd).unwrap();
        // st = server timestamp echo
        assert_eq!(parsed["st"].as_i64().unwrap(), server_ms);
        // rst = st + duration (request-send-time)
        let dur = parsed["duration"].as_u64().unwrap() as f64;
        let rst = parsed["rst"].as_f64().unwrap();
        assert!(
            (rst - (server_ms as f64 + dur)).abs() < 1.0,
            "rst {rst} should ≈ st + duration ({} + {})",
            server_ms,
            dur
        );
    }
}
