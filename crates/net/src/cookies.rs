//! Simple cookie jar for persisting cookies across requests.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// A simple cookie jar that stores cookies per domain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CookieJar {
    /// Cookies keyed by domain → (name → Cookie)
    cookies: HashMap<String, HashMap<String, Cookie>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cookie {
    name: String,
    value: String,
    path: String,
    /// Domain= attribute parsed from the Set-Cookie header (leading dot
    /// stripped, lowercased). `None` means host-only cookie — only the
    /// exact request domain matches. `Some(d)` is the suffix-match scope.
    /// Phase 7 follow-up — fixes mail-ru cookie relay.
    #[serde(default)]
    domain: Option<String>,
    secure: bool,
    #[allow(dead_code)]
    http_only: bool,
    /// Expiry as unix timestamp, None = session cookie
    #[allow(dead_code)]
    expires: Option<u64>,
}

impl CookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total cookie count across all domains (diagnostics — FIX-COOKIE-SYNC).
    pub fn cookie_count(&self) -> usize {
        self.cookies.values().map(|m| m.len()).sum()
    }

    /// Parse and store cookies from Set-Cookie response headers.
    ///
    /// Honors the `Domain=` attribute when present so subdomain → parent-
    /// domain redirects carry cookies correctly (e.g., `e.mail.ru`
    /// setting `Domain=.mail.ru` makes the cookie visible on
    /// `mail.ru/?afterReload`). Cross-domain Set-Cookie attempts (e.g.,
    /// `e.mail.ru` setting `Domain=evil.com`) are rejected per RFC 6265
    /// §5.3 step 6 — host-suffix-match only.
    pub fn set_cookies(&mut self, url: &Url, set_cookie_headers: &[String]) {
        let request_domain = match url.host_str() {
            Some(d) => d.to_lowercase(),
            None => return,
        };

        let now = unix_now().max(0) as u64;
        for header in set_cookie_headers {
            if let Some(cookie) = parse_set_cookie(header, &request_domain, url.path()) {
                // Use the cookie's domain (Domain= attribute) if it's a
                // host-suffix-match of the request URL. Otherwise fall
                // back to the request domain (host-only cookie per spec).
                let store_domain = match &cookie.domain {
                    Some(d) if domain_matches(&request_domain, d) => d.clone(),
                    _ => request_domain.clone(),
                };
                // FIX-COOKIE-DELETE: a Set-Cookie whose expiry is in the past
                // is a DELETION (RFC 6265 §5.3 step 11) — remove the matching
                // cookie instead of storing an empty-value entry. Without
                // this, AWS-WAF's `aws-waf-token=; expires=1970` deletes left
                // an empty `aws-waf-token=` in the Cookie header that AWS read
                // before the real token → 202 stub on every reload.
                if cookie.expires.is_some_and(|e| e <= now) {
                    if let Some(bucket) = self.cookies.get_mut(&store_domain) {
                        bucket.remove(&cookie.name);
                        if bucket.is_empty() {
                            self.cookies.remove(&store_domain);
                        }
                    }
                    continue;
                }
                self.cookies
                    .entry(store_domain)
                    .or_default()
                    .insert(cookie.name.clone(), cookie);
            }
        }
    }

    /// Get the Cookie header value for a given URL.
    pub fn cookies_for(&self, url: &Url) -> Option<String> {
        let request_domain = url.host_str()?.to_lowercase();
        let path = url.path();
        let is_secure = url.scheme() == "https";

        let mut pairs = Vec::new();

        // Check exact domain and parent domains via the canonical
        // host-suffix-match. Honors host-only cookies (cookie.domain
        // is None) by requiring exact domain equality.
        for (stored_domain, cookies) in &self.cookies {
            if !domain_matches(&request_domain, stored_domain) {
                continue;
            }
            for cookie in cookies.values() {
                // Host-only cookies (no Domain= attribute parsed):
                // only the exact request domain matches per RFC 6265.
                if cookie.domain.is_none() && request_domain != *stored_domain {
                    continue;
                }
                if cookie.secure && !is_secure {
                    continue;
                }
                if !path.starts_with(&cookie.path) {
                    continue;
                }
                pairs.push(format!("{}={}", cookie.name, cookie.value));
            }
        }

        if pairs.is_empty() {
            None
        } else {
            Some(pairs.join("; "))
        }
    }

    /// Persist the jar to a JSON file. Atomic via tempfile + rename.
    /// Used to accumulate Kasada (and other) trust across pipeline runs:
    /// once /tl issues us a session, we keep tkrm_alpekz_*, AKA_A2 etc.
    /// for subsequent runs which lifts our reputation score.
    pub fn save_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Load a jar from a JSON file. Returns an empty jar if the file
    /// doesn't exist (first-run case). Returns an error only if the
    /// file exists but is malformed.
    pub fn load_from_file(path: &std::path::Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path)?;
        serde_json::from_slice(&bytes).map_err(std::io::Error::other)
    }

    /// Remove all stored cookies whose stored-domain key is a host-
    /// suffix-match of `target_domain` (i.e. the cookie belongs to
    /// `target_domain` itself or any of its subdomains).
    ///
    /// Used for the x.com / twitter.com rebrand-collision band-aid
    /// (Sprint 2.3 Path 3 / R-SHAREDSESSION-X-COM-COOKIES): clearing
    /// `twitter.com` before a fresh `x.com` nav prevents the
    /// cross-identity cookie poisoning that triggers x.com's WAF.
    /// Returns the number of (domain → cookie-map) buckets evicted —
    /// useful for the call-site to log when the band-aid fired.
    pub fn clear_for_domain(&mut self, target_domain: &str) -> usize {
        let target = target_domain.trim_start_matches('.').to_ascii_lowercase();
        let before = self.cookies.len();
        self.cookies
            .retain(|stored, _| !domain_matches(stored, &target));
        before - self.cookies.len()
    }
}

/// Extract the base domain for cookie storage.
#[allow(dead_code)]
fn cookie_domain(domain: &str) -> String {
    domain.trim_start_matches('.').to_lowercase()
}

/// True iff `request_domain` is the same as `cookie_domain` or a subdomain
/// of it. Used both to validate a Set-Cookie's `Domain=` attribute (the
/// request URL must be a subdomain of the proposed Domain) and to match
/// stored cookies on outgoing requests.
fn domain_matches(request_domain: &str, cookie_domain: &str) -> bool {
    let r = request_domain.trim_start_matches('.').to_lowercase();
    let c = cookie_domain.trim_start_matches('.').to_lowercase();
    r == c || r.ends_with(&format!(".{c}"))
}

/// Current unix timestamp in seconds (signed so arithmetic with Max-Age
/// deltas can't underflow before the `.max(0)` clamp).
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Parse a cookie `Expires=` HTTP-date into a unix timestamp. Handles the
/// RFC 1123 / RFC 850 / asctime-ish forms browsers actually emit (incl. the
/// `… GMT` and `-` dotted-month deletion form). Returns `None` if unparseable
/// — an unparseable Expires is treated as "no expiry" (session cookie), never
/// as a deletion, so a parse miss can't silently drop a live cookie.
fn parse_http_date(s: &str) -> Option<u64> {
    let s = s.trim();
    const FORMATS: &[&str] = &[
        "%a, %d %b %Y %H:%M:%S GMT",
        "%a, %d-%b-%Y %H:%M:%S GMT",
        "%A, %d-%b-%y %H:%M:%S GMT",
        "%a %b %e %H:%M:%S %Y",
    ];
    for f in FORMATS {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(s, f) {
            return Some(ndt.and_utc().timestamp().max(0) as u64);
        }
    }
    chrono::DateTime::parse_from_rfc2822(s)
        .ok()
        .map(|dt| dt.timestamp().max(0) as u64)
}

/// Parse a Set-Cookie header value into a Cookie. Honors `Domain=`,
/// `Path=`, `Secure`, `HttpOnly`, `Max-Age`, and `Expires` attributes.
fn parse_set_cookie(header: &str, request_domain: &str, default_path: &str) -> Option<Cookie> {
    let mut parts = header.split(';');

    // First part is name=value
    let name_value = parts.next()?.trim();
    let (name, value) = name_value.split_once('=')?;
    let name = name.trim().to_string();
    let value = value.trim().to_string();

    if name.is_empty() {
        return None;
    }

    let mut path = default_path.to_string();
    let mut domain: Option<String> = None;
    let mut secure = false;
    let mut http_only = false;
    // FIX-COOKIE-DELETE (parity-workflows): parse Max-Age / Expires so the
    // jar can HONOR deletions. AWS-WAF's challenge.js deletes stale tokens
    // via `aws-waf-token=; expires=Thu, 01 Jan 1970 …` before setting the
    // real one; without parsing expiry we stored those as empty-value
    // cookies, poisoning the Cookie header (`aws-waf-token=; aws-waf-token=…`)
    // so AWS read an empty token and re-served the 202 stub. Verified on imdb.
    let mut max_age: Option<i64> = None;
    let mut expires_str: Option<String> = None;

    for attr in parts {
        let attr = attr.trim();
        let (attr_name, attr_value) = attr
            .split_once('=')
            .map(|(n, v)| (n.trim().to_lowercase(), Some(v.trim())))
            .unwrap_or_else(|| (attr.to_lowercase(), None));

        match attr_name.as_str() {
            "path" => {
                if let Some(v) = attr_value {
                    path = v.to_string();
                }
            }
            "max-age" => {
                if let Some(v) = attr_value {
                    max_age = v.parse::<i64>().ok();
                }
            }
            "expires" => {
                if let Some(v) = attr_value {
                    expires_str = Some(v.to_string());
                }
            }
            "domain" => {
                if let Some(v) = attr_value {
                    let v = v.trim_start_matches('.').to_lowercase();
                    if !v.is_empty() {
                        // Per RFC 6265 §5.3 step 6 — reject the cookie if
                        // the proposed Domain isn't a host-suffix of the
                        // request URL. This prevents `e.mail.ru` from
                        // setting cookies on `evil.com`.
                        if domain_matches(request_domain, &v) {
                            domain = Some(v);
                        }
                        // If the Domain attribute is present but invalid,
                        // the cookie is dropped entirely per spec.
                        else {
                            return None;
                        }
                    }
                }
            }
            "secure" => secure = true,
            "httponly" => http_only = true,
            _ => {}
        }
    }

    // Max-Age takes precedence over Expires (RFC 6265 §5.3). Resolve to an
    // absolute unix timestamp; `set_cookies` treats a past timestamp as a
    // deletion.
    let now = unix_now();
    let expires: Option<u64> = if let Some(ma) = max_age {
        Some((now + ma).max(0) as u64)
    } else if let Some(es) = &expires_str {
        parse_http_date(es)
    } else {
        None
    };

    Some(Cookie {
        name,
        value,
        path,
        domain,
        secure,
        http_only,
        expires,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_cookie() {
        let mut jar = CookieJar::new();
        let url = Url::parse("https://example.com/path").unwrap();
        jar.set_cookies(&url, &["session=abc123; Path=/; Secure".to_string()]);

        let cookies = jar.cookies_for(&url);
        assert_eq!(cookies, Some("session=abc123".to_string()));
    }

    #[test]
    fn no_cookies_for_different_domain() {
        let mut jar = CookieJar::new();
        let url1 = Url::parse("https://example.com/").unwrap();
        let url2 = Url::parse("https://other.com/").unwrap();
        jar.set_cookies(&url1, &["key=val".to_string()]);

        assert!(jar.cookies_for(&url2).is_none());
    }

    #[test]
    fn secure_cookie_not_sent_over_http() {
        let mut jar = CookieJar::new();
        let https_url = Url::parse("https://example.com/").unwrap();
        let http_url = Url::parse("http://example.com/").unwrap();
        jar.set_cookies(&https_url, &["token=secret; Secure".to_string()]);

        assert!(jar.cookies_for(&https_url).is_some());
        assert!(jar.cookies_for(&http_url).is_none());
    }

    #[test]
    fn multiple_cookies() {
        let mut jar = CookieJar::new();
        let url = Url::parse("https://example.com/").unwrap();
        jar.set_cookies(&url, &["a=1".to_string(), "b=2".to_string()]);

        let cookies = jar.cookies_for(&url).unwrap();
        assert!(cookies.contains("a=1"));
        assert!(cookies.contains("b=2"));
    }

    // ---- Phase 7 follow-up: Domain attribute parsing (T1A) ----

    #[test]
    fn domain_attribute_carries_to_parent() {
        // mail-ru repro: e.mail.ru sets Domain=.mail.ru, then
        // mail.ru/?afterReload should receive the cookie.
        let mut jar = CookieJar::new();
        let set_url = Url::parse("https://e.mail.ru/login").unwrap();
        jar.set_cookies(&set_url, &["t=hash; Domain=.mail.ru; Path=/".to_string()]);
        let parent = Url::parse("https://mail.ru/?afterReload").unwrap();
        let got = jar
            .cookies_for(&parent)
            .expect("cookie should be visible on parent domain");
        assert_eq!(got, "t=hash");
    }

    #[test]
    fn domain_attribute_carries_to_sibling_subdomain() {
        // e.mail.ru sets Domain=.mail.ru → also visible on m.mail.ru.
        let mut jar = CookieJar::new();
        let set_url = Url::parse("https://e.mail.ru/").unwrap();
        jar.set_cookies(&set_url, &["t=v; Domain=mail.ru".to_string()]);
        let sibling = Url::parse("https://m.mail.ru/").unwrap();
        assert_eq!(jar.cookies_for(&sibling), Some("t=v".to_string()));
    }

    #[test]
    fn domain_attribute_rejects_unrelated_origin() {
        // RFC 6265 §5.3 step 6: a Set-Cookie with Domain= that isn't a
        // suffix of the request URL is rejected entirely.
        let mut jar = CookieJar::new();
        let set_url = Url::parse("https://e.mail.ru/").unwrap();
        jar.set_cookies(&set_url, &["evil=1; Domain=evil.com".to_string()]);
        let evil = Url::parse("https://evil.com/").unwrap();
        assert!(
            jar.cookies_for(&evil).is_none(),
            "cross-origin Domain= must be rejected"
        );
        // Also not stored under e.mail.ru as a fallback — the cookie is
        // dropped entirely per spec.
        assert!(jar.cookies_for(&set_url).is_none());
    }

    #[test]
    fn host_only_cookie_does_not_carry_to_parent() {
        // Without a Domain= attribute, a Set-Cookie is host-only:
        // visible on e.mail.ru but NOT on mail.ru.
        let mut jar = CookieJar::new();
        let set_url = Url::parse("https://e.mail.ru/").unwrap();
        jar.set_cookies(&set_url, &["s=1".to_string()]);
        let parent = Url::parse("https://mail.ru/").unwrap();
        assert!(
            jar.cookies_for(&parent).is_none(),
            "host-only cookie must not leak to parent"
        );
        // But it IS visible on the exact host that set it.
        assert_eq!(jar.cookies_for(&set_url), Some("s=1".to_string()));
    }

    #[test]
    fn domain_attribute_strips_leading_dot() {
        // RFC 6265 ignores a leading dot on the Domain attribute. Both
        // forms should produce the same storage key.
        let mut jar1 = CookieJar::new();
        let mut jar2 = CookieJar::new();
        let url = Url::parse("https://e.mail.ru/").unwrap();
        jar1.set_cookies(&url, &["t=1; Domain=.mail.ru".to_string()]);
        jar2.set_cookies(&url, &["t=1; Domain=mail.ru".to_string()]);
        let parent = Url::parse("https://mail.ru/").unwrap();
        assert_eq!(jar1.cookies_for(&parent), jar2.cookies_for(&parent));
        assert_eq!(jar1.cookies_for(&parent), Some("t=1".to_string()));
    }

    #[test]
    fn domain_matches_helper() {
        assert!(domain_matches("e.mail.ru", "mail.ru"));
        assert!(domain_matches("mail.ru", "mail.ru"));
        assert!(domain_matches("a.b.c.example.com", "example.com"));
        assert!(domain_matches("e.mail.ru", ".mail.ru")); // leading dot tolerated
        assert!(!domain_matches("mail.ru", "e.mail.ru")); // wrong direction
        assert!(!domain_matches("evilmail.ru", "mail.ru")); // not a suffix at boundary
        assert!(!domain_matches("mail.ru", "evil.com"));
    }

    // ===== Sprint 2.3 Path 3 — clear_for_domain =====

    #[test]
    fn clear_for_domain_evicts_exact_and_subdomains() {
        let mut jar = CookieJar::new();
        let twitter = url::Url::parse("https://twitter.com/").unwrap();
        let mobile_twitter = url::Url::parse("https://mobile.twitter.com/").unwrap();
        let x = url::Url::parse("https://x.com/").unwrap();
        let other = url::Url::parse("https://example.com/").unwrap();

        jar.set_cookies(
            &twitter,
            &["guest_id=v1%3A123; Domain=.twitter.com".to_string()],
        );
        jar.set_cookies(&mobile_twitter, &["mobile_pref=dark".to_string()]);
        jar.set_cookies(&x, &["ct0=xyz".to_string()]);
        jar.set_cookies(&other, &["session=abc".to_string()]);

        let evicted = jar.clear_for_domain("twitter.com");

        // 2 stored-domain buckets had to go: ".twitter.com" (set via
        // Domain= attr) and "mobile.twitter.com" (host-only).
        assert_eq!(evicted, 2);
        // x.com and example.com cookies survive.
        assert!(jar.cookies_for(&x).is_some());
        assert!(jar.cookies_for(&other).is_some());
        // Twitter cookies gone — across both subdomains.
        assert!(jar.cookies_for(&twitter).is_none());
        assert!(jar.cookies_for(&mobile_twitter).is_none());
    }

    #[test]
    fn clear_for_domain_no_match_returns_zero() {
        let mut jar = CookieJar::new();
        let twitter = url::Url::parse("https://twitter.com/").unwrap();
        jar.set_cookies(
            &twitter,
            &["guest_id=v1%3A123; Domain=.twitter.com".to_string()],
        );
        // Clearing an unrelated domain should leave the jar untouched.
        let evicted = jar.clear_for_domain("example.com");
        assert_eq!(evicted, 0);
        assert!(jar.cookies_for(&twitter).is_some());
    }

    #[test]
    fn clear_for_domain_ignores_leading_dot_and_case() {
        let mut jar = CookieJar::new();
        let twitter = url::Url::parse("https://twitter.com/").unwrap();
        jar.set_cookies(
            &twitter,
            &["guest_id=v1%3A123; Domain=.twitter.com".to_string()],
        );
        // `.TWITTER.COM` (leading dot + uppercase) must still match.
        let evicted = jar.clear_for_domain(".TWITTER.COM");
        assert_eq!(evicted, 1);
        assert!(jar.cookies_for(&twitter).is_none());
    }

    // FIX-COOKIE-DELETE regression (parity-workflows): an Expires-in-the-past
    // Set-Cookie deletes the cookie instead of storing an empty-value entry.
    // This is the AWS-WAF imdb bug: challenge.js deletes stale `aws-waf-token`
    // before setting the real one; the empty `aws-waf-token=` poisoned the
    // Cookie header so AWS read an empty token and re-served the 202 stub.
    #[test]
    fn expired_set_cookie_deletes_instead_of_storing_empty() {
        let imdb = url::Url::parse("https://www.imdb.com/").unwrap();
        let mut jar = CookieJar::new();
        // Mirror AWS-WAF's exact flow: DELETE stale tokens across several
        // domain variants FIRST (jar empty), then SET the real token. The
        // deletes must NOT create empty `aws-waf-token=` entries.
        jar.set_cookies(
            &imdb,
            &[
                "aws-waf-token=; Path=/; Domain=www.imdb.com; Expires=Thu, 01 Jan 1970 00:00:01 GMT".to_string(),
                "aws-waf-token=; Path=/; Domain=imdb.com; Expires=Thu, 01 Jan 1970 00:00:01 GMT".to_string(),
                "aws-waf-token=; Path=/; Expires=Thu, 01 Jan 1970 00:00:01 GMT".to_string(),
            ],
        );
        assert!(
            jar.cookies_for(&imdb).is_none(),
            "deletes on an empty jar must not create empty entries, got {:?}",
            jar.cookies_for(&imdb)
        );
        // Now the real set.
        jar.set_cookies(&imdb, &["aws-waf-token=REALTOKEN123; Path=/".to_string()]);
        let hdr = jar.cookies_for(&imdb).unwrap_or_default();
        assert_eq!(
            hdr, "aws-waf-token=REALTOKEN123",
            "header must carry ONLY the real token (no empty-token poison): '{hdr}'"
        );

        // Max-Age=0 deletes the live token.
        jar.set_cookies(&imdb, &["aws-waf-token=; Path=/; Max-Age=0".to_string()]);
        assert!(
            jar.cookies_for(&imdb).is_none(),
            "Max-Age=0 must remove the cookie entirely"
        );

        // A future Expires keeps the cookie.
        jar.set_cookies(
            &imdb,
            &["sess=keepme; Path=/; Expires=Tue, 01 Jan 2999 00:00:01 GMT".to_string()],
        );
        assert_eq!(jar.cookies_for(&imdb).as_deref(), Some("sess=keepme"));
    }
}
