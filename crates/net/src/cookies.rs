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

        for header in set_cookie_headers {
            if let Some(cookie) = parse_set_cookie(header, &request_domain, url.path()) {
                // Use the cookie's domain (Domain= attribute) if it's a
                // host-suffix-match of the request URL. Otherwise fall
                // back to the request domain (host-only cookie per spec).
                let store_domain = match &cookie.domain {
                    Some(d) if domain_matches(&request_domain, d) => d.clone(),
                    _ => request_domain.clone(),
                };
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

/// Parse a Set-Cookie header value into a Cookie. Honors `Domain=`,
/// `Path=`, `Secure`, and `HttpOnly` attributes.
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
    let expires = None;

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
        jar.set_cookies(
            &set_url,
            &["t=hash; Domain=.mail.ru; Path=/".to_string()],
        );
        let parent = Url::parse("https://mail.ru/?afterReload").unwrap();
        let got = jar.cookies_for(&parent).expect("cookie should be visible on parent domain");
        assert_eq!(got, "t=hash");
    }

    #[test]
    fn domain_attribute_carries_to_sibling_subdomain() {
        // e.mail.ru sets Domain=.mail.ru → also visible on m.mail.ru.
        let mut jar = CookieJar::new();
        let set_url = Url::parse("https://e.mail.ru/").unwrap();
        jar.set_cookies(
            &set_url,
            &["t=v; Domain=mail.ru".to_string()],
        );
        let sibling = Url::parse("https://m.mail.ru/").unwrap();
        assert_eq!(jar.cookies_for(&sibling), Some("t=v".to_string()));
    }

    #[test]
    fn domain_attribute_rejects_unrelated_origin() {
        // RFC 6265 §5.3 step 6: a Set-Cookie with Domain= that isn't a
        // suffix of the request URL is rejected entirely.
        let mut jar = CookieJar::new();
        let set_url = Url::parse("https://e.mail.ru/").unwrap();
        jar.set_cookies(
            &set_url,
            &["evil=1; Domain=evil.com".to_string()],
        );
        let evil = Url::parse("https://evil.com/").unwrap();
        assert!(jar.cookies_for(&evil).is_none(), "cross-origin Domain= must be rejected");
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
        assert!(jar.cookies_for(&parent).is_none(), "host-only cookie must not leak to parent");
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
}
