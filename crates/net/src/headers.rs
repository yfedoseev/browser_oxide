//! Ordered browser header construction for Chrome 130.
//!
//! Anti-bot systems check both the presence and order of HTTP headers.
//! This module builds headers in the exact Chrome 130 order.

use stealth::StealthProfile;

/// Build ordered Chrome browser headers from a stealth profile.
///
/// Returns headers as ordered (name, value) pairs matching the exact
/// header set and order real Chrome sends on a first-visit navigation
/// request. **Thirteen headers** — NO high-entropy Client Hints
/// (those only appear on follow-up requests after the server
/// advertises `Accept-CH` in a response).
///
/// Empirical Chrome 146 order — captured from the developer's
/// machine via `tls.peet.ws/api/all` decoded HEADERS frame:
/// 1. upgrade-insecure-requests
/// 2. user-agent
/// 3. accept
/// 4. sec-ch-ua
/// 5. sec-ch-ua-mobile
/// 6. sec-ch-ua-platform
/// 7. sec-fetch-site
/// 8. sec-fetch-mode
/// 9. sec-fetch-user
/// 10. sec-fetch-dest
/// 11. accept-encoding
/// 12. accept-language
/// 13. priority
///
/// This order differs from a common misconception (documented
/// elsewhere as "sec-ch-ua first") — actual Chrome puts the
/// `upgrade-insecure-requests` / `user-agent` / `accept` triplet
/// before the `sec-ch-ua` group. Earlier in this session we had the
/// order reversed; the live capture corrected us.
pub fn chrome_headers(profile: &StealthProfile) -> Vec<(String, String)> {
    chrome_headers_impl(profile, false)
}

/// Build headers that match a JavaScript-initiated `location.reload()` /
/// same-origin assign — NOT a fresh user navigation. Differences from
/// `chrome_headers`:
///   - `sec-fetch-site: same-origin` (was `none`)
///   - `sec-fetch-user` is OMITTED (no user gesture)
///   - `Referer: <current_url>` is added
///
/// Used on post-challenge retries where the challenge engine may be
/// distinguishing fresh user navs from programmatic reloads.
pub fn chrome_headers_reload(profile: &StealthProfile, referer: &str) -> Vec<(String, String)> {
    let mut hdrs: Vec<(String, String)> = chrome_headers_impl(profile, false)
        .into_iter()
        .filter(|(k, _)| k != "sec-fetch-user")
        .map(|(k, v)| {
            if k == "sec-fetch-site" {
                (k, "same-origin".to_string())
            } else {
                (k, v)
            }
        })
        .collect();
    hdrs.push(("referer".to_string(), referer.to_string()));
    hdrs
}

/// Build headers that match a `window.fetch()` request from JS, NOT a
/// document navigation. Chrome's fetch API and its nav requests send
/// completely different header sets, and Kasada+friends use this
/// distinction as a strong bot signal when a "fetch" request arrives
/// carrying navigation headers.
///
/// Differences from navigation headers:
///   - `accept: */*` (not text/html...)
///   - NO `upgrade-insecure-requests`
///   - `sec-fetch-dest: empty` (not `document`)
///   - `sec-fetch-mode: cors` (default; caller can override via extra headers)
///   - `sec-fetch-site`: `same-origin` when target and origin match, else `cross-site`
///   - NO `sec-fetch-user`
///   - `priority: u=1, i` (fetch is default-interactive but lower priority than nav)
///   - Caller adds `origin` + `referer` separately (they depend on the current page).
pub fn chrome_headers_fetch(
    profile: &StealthProfile,
    target_url: &str,
    origin: Option<&str>,
) -> Vec<(String, String)> {
    let mut headers = Vec::with_capacity(12);

    headers.push(("user-agent".to_string(), profile.user_agent.clone()));
    headers.push(("accept".to_string(), "*/*".to_string()));

    let sec_ch_ua = build_sec_ch_ua(profile);
    headers.push(("sec-ch-ua".to_string(), sec_ch_ua));
    headers.push(("sec-ch-ua-mobile".to_string(), "?0".to_string()));
    headers.push((
        "sec-ch-ua-platform".to_string(),
        format!("\"{}\"", profile.os_name),
    ));

    // Compute sec-fetch-site from target vs origin.
    let site = match origin {
        Some(origin) => {
            let t = url::Url::parse(target_url).ok();
            let o = url::Url::parse(origin).ok();
            match (t, o) {
                (Some(tu), Some(ou)) => {
                    if tu.host_str() == ou.host_str() {
                        "same-origin"
                    } else if same_site(&tu, &ou) {
                        "same-site"
                    } else {
                        "cross-site"
                    }
                }
                _ => "cross-site",
            }
        }
        None => "cross-site",
    };
    headers.push(("sec-fetch-site".to_string(), site.to_string()));
    headers.push(("sec-fetch-mode".to_string(), "cors".to_string()));
    headers.push(("sec-fetch-dest".to_string(), "empty".to_string()));

    headers.push((
        "accept-encoding".to_string(),
        "gzip, deflate, br, zstd".to_string(),
    ));
    headers.push((
        "accept-language".to_string(),
        build_accept_language(&profile.languages),
    ));
    headers.push(("priority".to_string(), "u=1, i".to_string()));

    // Origin + Referer — always set for same-site + cross-site fetches
    if let Some(o) = origin {
        headers.push(("origin".to_string(), o.to_string()));
        headers.push(("referer".to_string(), format!("{}/", o.trim_end_matches('/'))));
    }

    headers
}

/// Heuristic same-site comparison: registered domain (eTLD+1) would be the
/// correct implementation; as a proxy, compare the last two labels.
fn same_site(a: &url::Url, b: &url::Url) -> bool {
    fn tail2(u: &url::Url) -> Option<String> {
        let host = u.host_str()?;
        let mut parts: Vec<&str> = host.rsplit('.').collect();
        if parts.len() < 2 {
            return Some(host.to_string());
        }
        parts.truncate(2);
        parts.reverse();
        Some(parts.join("."))
    }
    tail2(a) == tail2(b)
}

/// Build headers for a request that should include the high-entropy
/// Client Hints (sec-ch-ua-arch, -bitness, -full-version-list, -model,
/// -platform-version, -wow64). Only applicable on a follow-up request
/// AFTER the server sent `Accept-CH` in a previous response — real
/// Chrome does NOT send these on the first visit.
///
/// Sending them when the server didn't ask for them is a known
/// fingerprint tell flagged by Akamai Bot Manager v3 and Kasada.
pub fn chrome_headers_with_accept_ch(profile: &StealthProfile) -> Vec<(String, String)> {
    chrome_headers_impl(profile, true)
}

fn chrome_headers_impl(
    profile: &StealthProfile,
    include_high_entropy: bool,
) -> Vec<(String, String)> {
    let mut headers = Vec::with_capacity(if include_high_entropy { 20 } else { 13 });

    // upgrade-insecure-requests — FIRST per Chrome 146 live capture
    headers.push(("upgrade-insecure-requests".to_string(), "1".to_string()));

    // user-agent
    headers.push(("user-agent".to_string(), profile.user_agent.clone()));

    // accept
    headers.push((
        "accept".to_string(),
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7".to_string(),
    ));

    // sec-ch-ua (Client Hints, low-entropy — always sent)
    let sec_ch_ua = build_sec_ch_ua(profile);
    headers.push(("sec-ch-ua".to_string(), sec_ch_ua.clone()));
    headers.push(("sec-ch-ua-mobile".to_string(), "?0".to_string()));
    headers.push((
        "sec-ch-ua-platform".to_string(),
        format!("\"{}\"", profile.os_name),
    ));

    if include_high_entropy {
        // High-entropy hints — only valid on requests that follow an
        // `Accept-CH` response advertisement. Order matches Chrome's
        // alphabetical-ish sec-ch-ua-* sort after the low-entropy
        // basics.
        headers.push((
            "sec-ch-ua-arch".to_string(),
            format!("\"{}\"", cpu_arch_for(&profile.platform)),
        ));
        headers.push(("sec-ch-ua-bitness".to_string(), "\"64\"".to_string()));
        headers.push((
            "sec-ch-ua-full-version-list".to_string(),
            build_sec_ch_ua_full_version_list(profile),
        ));
        headers.push(("sec-ch-ua-model".to_string(), "\"\"".to_string()));
        headers.push((
            "sec-ch-ua-platform-version".to_string(),
            format!(
                "\"{}\"",
                chrome_platform_version(&profile.os_name, &profile.os_version)
            ),
        ));
        headers.push(("sec-ch-ua-wow64".to_string(), "?0".to_string()));
    }

    // sec-fetch headers
    headers.push(("sec-fetch-site".to_string(), "none".to_string()));
    headers.push(("sec-fetch-mode".to_string(), "navigate".to_string()));
    headers.push(("sec-fetch-user".to_string(), "?1".to_string()));
    headers.push(("sec-fetch-dest".to_string(), "document".to_string()));

    // accept-encoding (Chrome 124+ includes zstd)
    headers.push((
        "accept-encoding".to_string(),
        "gzip, deflate, br, zstd".to_string(),
    ));

    // accept-language
    let accept_language = build_accept_language(&profile.languages);
    headers.push(("accept-language".to_string(), accept_language));

    // priority (Chrome 130+)
    headers.push(("priority".to_string(), "u=0, i".to_string()));

    headers
}

/// CPU architecture string for `Sec-CH-UA-Arch`.
/// Derived from the StealthProfile's `platform` field (`Win32`, `MacIntel`,
/// `Linux x86_64`, `Linux aarch64`, etc.).
fn cpu_arch_for(platform: &str) -> &'static str {
    let p = platform.to_ascii_lowercase();
    if p.contains("arm") || p.contains("aarch") {
        "arm"
    } else {
        // Win32 / MacIntel / Linux x86_64 all report "x86" for the arch hint.
        // Chrome reports "x86" (not "x86_64") — the separate `bitness` hint
        // carries the 32-vs-64 distinction.
        "x86"
    }
}

/// Chrome platform-version string for `Sec-CH-UA-Platform-Version`.
/// Chrome uses a zero-padded triple even when the OS version is a single
/// number (e.g., Windows 10.0.0 → "10.0.0"; macOS 15.2 → "15.2.0").
fn chrome_platform_version(os_name: &str, os_version: &str) -> String {
    // If the profile's os_version already has enough components, use it verbatim.
    let parts: Vec<&str> = os_version.split('.').collect();
    if parts.len() >= 3 {
        return os_version.to_string();
    }
    match os_name {
        // Chrome on Windows uses the Windows "releaseId" as the major and
        // reports the full triple.
        "Windows" => {
            // "10.0" → "10.0.0", "11" → "11.0.0"
            match parts.len() {
                1 => format!("{}.0.0", parts[0]),
                2 => format!("{}.{}.0", parts[0], parts[1]),
                _ => os_version.to_string(),
            }
        }
        "macOS" => {
            // "15.2" → "15.2.0"
            match parts.len() {
                1 => format!("{}.0.0", parts[0]),
                2 => format!("{}.{}.0", parts[0], parts[1]),
                _ => os_version.to_string(),
            }
        }
        // Linux: Chrome typically reports "" (empty) for platform version on
        // Linux, since there's no canonical release number. Match that.
        "Linux" => String::new(),
        _ => os_version.to_string(),
    }
}

/// Build the `Sec-CH-UA-Full-Version-List` header value.
///
/// **Chrome 146 live capture** from the developer's machine:
/// ```text
/// "Chromium";v="146.0.0.0", "Not-A.Brand";v="24.0.0.0", "Google Chrome";v="146.0.0.0"
/// ```
/// Note the brand triple's order: `Chromium`, then `Not-A.Brand` in
/// the **middle**, then `Google Chrome`. The "Not" brand rotates
/// format across Chrome versions (was `Not?A_Brand` in Chrome 120-ish,
/// `Not/A)Brand` earlier, now `Not-A.Brand` in 146). We track the
/// current format; the brand ordering and version v=24 are also
/// from the live capture.
fn build_sec_ch_ua_full_version_list(profile: &StealthProfile) -> String {
    let v = &profile.browser_version;
    format!(
        "\"Chromium\";v=\"{v}\", \"Not-A.Brand\";v=\"24.0.0.0\", \"Google Chrome\";v=\"{v}\""
    )
}

/// Build the sec-ch-ua header value from the browser version.
///
/// **Chrome 146 live capture**:
/// ```text
/// "Chromium";v="146", "Not-A.Brand";v="24", "Google Chrome";v="146"
/// ```
/// Same brand triple as the full-version-list variant — only the
/// version numbers drop the `.0.0.0` suffix. `Not-A.Brand` in the
/// middle, not the end.
fn build_sec_ch_ua(profile: &StealthProfile) -> String {
    let major_version = profile
        .browser_version
        .split('.')
        .next()
        .unwrap_or("146");

    format!(
        "\"Chromium\";v=\"{v}\", \"Not-A.Brand\";v=\"24\", \"Google Chrome\";v=\"{v}\"",
        v = major_version
    )
}

/// Build accept-language with quality values.
fn build_accept_language(languages: &[String]) -> String {
    if languages.is_empty() {
        return "en-US,en;q=0.9".to_string();
    }

    let mut parts = Vec::with_capacity(languages.len());
    for (i, lang) in languages.iter().enumerate() {
        if i == 0 {
            parts.push(lang.clone());
        } else {
            // Decrease quality: 0.9, 0.8, 0.7, ...
            let q = 1.0 - (i as f64 * 0.1);
            if q > 0.0 {
                parts.push(format!("{};q={:.1}", lang, q));
            }
        }
    }

    parts.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_language_single() {
        let result = build_accept_language(&["en-US".to_string()]);
        assert_eq!(result, "en-US");
    }

    #[test]
    fn accept_language_multiple() {
        let result = build_accept_language(&["en-US".to_string(), "en".to_string()]);
        assert_eq!(result, "en-US,en;q=0.9");
    }

    #[test]
    fn accept_language_empty() {
        let result = build_accept_language(&[]);
        assert_eq!(result, "en-US,en;q=0.9");
    }

    #[test]
    fn chrome_headers_first_visit_is_low_entropy_only() {
        // Real Chrome 130 first-visit navigation has 13 headers and
        // does NOT include the high-entropy Client Hints. Those only
        // appear on requests that follow an `Accept-CH` advertisement.
        let profile = stealth::chrome_130_windows();
        let headers = chrome_headers(&profile);
        let names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            headers.len(),
            13,
            "first-visit headers must match Chrome 130's count (13), got {}",
            headers.len()
        );

        // Low-entropy basics are present.
        for required in &["sec-ch-ua", "sec-ch-ua-mobile", "sec-ch-ua-platform"] {
            assert!(
                names.contains(required),
                "expected header '{required}' missing",
            );
        }

        // High-entropy hints must NOT be present.
        for forbidden in &[
            "sec-ch-ua-arch",
            "sec-ch-ua-bitness",
            "sec-ch-ua-full-version-list",
            "sec-ch-ua-model",
            "sec-ch-ua-platform-version",
            "sec-ch-ua-wow64",
        ] {
            assert!(
                !names.contains(forbidden),
                "header '{forbidden}' leaked onto first-visit request — Chrome only sends this after Accept-CH",
            );
        }
    }

    #[test]
    fn chrome_headers_with_accept_ch_includes_high_entropy() {
        // After the server advertises Accept-CH in a prior response,
        // Chrome upgrades subsequent requests on the same origin with
        // the full high-entropy client-hint set. This is the variant
        // callers reach for when they see `Accept-CH` in a response.
        let profile = stealth::chrome_130_windows();
        let headers = chrome_headers_with_accept_ch(&profile);
        let names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        for required in &[
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "sec-ch-ua-platform",
            "sec-ch-ua-arch",
            "sec-ch-ua-bitness",
            "sec-ch-ua-full-version-list",
            "sec-ch-ua-model",
            "sec-ch-ua-platform-version",
            "sec-ch-ua-wow64",
        ] {
            assert!(
                names.contains(required),
                "expected header '{required}' missing from accept-ch variant",
            );
        }
    }

    #[test]
    fn sec_ch_ua_full_version_list_has_chrome_version() {
        // Chrome 146 live capture format:
        //   "Chromium";v="<ver>", "Not-A.Brand";v="24.0.0.0", "Google Chrome";v="<ver>"
        // The "Not" brand rotates across Chrome versions (was
        // `Not?A_Brand` in earlier ones); we track the current one.
        let profile = stealth::chrome_130_linux();
        let value = build_sec_ch_ua_full_version_list(&profile);
        assert!(value.contains("Google Chrome"));
        assert!(value.contains(&profile.browser_version));
        assert!(value.contains("Not-A.Brand"));
        // Brand order: Chromium first, Not-A.Brand in the middle,
        // Google Chrome last.
        let chromium_idx = value.find("Chromium").unwrap();
        let not_idx = value.find("Not-A.Brand").unwrap();
        let google_idx = value.find("Google Chrome").unwrap();
        assert!(chromium_idx < not_idx);
        assert!(not_idx < google_idx);
    }

    #[test]
    fn platform_version_triple_padded() {
        assert_eq!(chrome_platform_version("Windows", "10.0"), "10.0.0");
        assert_eq!(chrome_platform_version("Windows", "11"), "11.0.0");
        assert_eq!(chrome_platform_version("macOS", "15.2"), "15.2.0");
        assert_eq!(chrome_platform_version("Linux", "anything"), "");
    }

    #[test]
    fn cpu_arch_recognizes_arm() {
        assert_eq!(cpu_arch_for("arm64"), "arm");
        assert_eq!(cpu_arch_for("Linux aarch64"), "arm");
        assert_eq!(cpu_arch_for("Win32"), "x86");
        assert_eq!(cpu_arch_for("MacIntel"), "x86");
        assert_eq!(cpu_arch_for("Linux x86_64"), "x86");
    }

    #[test]
    fn client_hints_match_profile_version() {
        // Invariant: the sec-ch-ua value and the sec-ch-ua-full-version-list value
        // must both reference the same major version, otherwise detection scripts
        // that cross-check the two get a free signal. Checked against the
        // Accept-CH variant because that's the one that carries both values.
        let profile = stealth::chrome_130_windows();
        let headers = chrome_headers_with_accept_ch(&profile);
        let sec_ch_ua = headers.iter().find(|(k, _)| k == "sec-ch-ua").unwrap().1.clone();
        let fvl = headers
            .iter()
            .find(|(k, _)| k == "sec-ch-ua-full-version-list")
            .unwrap()
            .1
            .clone();
        let major = profile.browser_version.split('.').next().unwrap();
        assert!(sec_ch_ua.contains(major));
        assert!(fvl.contains(&profile.browser_version));
    }
}
