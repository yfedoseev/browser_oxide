//! Simple cookie jar for persisting cookies across requests.

use std::collections::HashMap;
use url::Url;

/// A simple cookie jar that stores cookies per domain.
#[derive(Debug, Clone, Default)]
pub struct CookieJar {
    /// Cookies keyed by domain → (name → Cookie)
    cookies: HashMap<String, HashMap<String, Cookie>>,
}

#[derive(Debug, Clone)]
struct Cookie {
    name: String,
    value: String,
    path: String,
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
    pub fn set_cookies(&mut self, url: &Url, set_cookie_headers: &[String]) {
        let domain = match url.host_str() {
            Some(d) => d.to_lowercase(),
            None => return,
        };

        for header in set_cookie_headers {
            if let Some(cookie) = parse_set_cookie(header, &domain, url.path()) {
                self.cookies
                    .entry(cookie_domain(&domain))
                    .or_default()
                    .insert(cookie.name.clone(), cookie);
            }
        }
    }

    /// Get the Cookie header value for a given URL.
    pub fn cookies_for(&self, url: &Url) -> Option<String> {
        let domain = url.host_str()?.to_lowercase();
        let path = url.path();
        let is_secure = url.scheme() == "https";

        let mut pairs = Vec::new();

        // Check exact domain and parent domains
        for (cookie_domain, cookies) in &self.cookies {
            if domain == *cookie_domain || domain.ends_with(&format!(".{cookie_domain}")) {
                for cookie in cookies.values() {
                    if cookie.secure && !is_secure {
                        continue;
                    }
                    if !path.starts_with(&cookie.path) {
                        continue;
                    }
                    pairs.push(format!("{}={}", cookie.name, cookie.value));
                }
            }
        }

        if pairs.is_empty() {
            None
        } else {
            Some(pairs.join("; "))
        }
    }
}

/// Extract the base domain for cookie storage.
fn cookie_domain(domain: &str) -> String {
    domain.trim_start_matches('.').to_lowercase()
}

/// Parse a Set-Cookie header value into a Cookie.
fn parse_set_cookie(header: &str, _default_domain: &str, default_path: &str) -> Option<Cookie> {
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
            "secure" => secure = true,
            "httponly" => http_only = true,
            _ => {}
        }
    }

    Some(Cookie {
        name,
        value,
        path,
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
        jar.set_cookies(
            &url,
            &[
                "a=1".to_string(),
                "b=2".to_string(),
            ],
        );

        let cookies = jar.cookies_for(&url).unwrap();
        assert!(cookies.contains("a=1"));
        assert!(cookies.contains("b=2"));
    }
}
