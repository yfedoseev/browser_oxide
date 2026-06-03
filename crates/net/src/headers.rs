//! Ordered browser header construction for Chrome 130.
//!
//! Anti-bot systems check both the presence and order of HTTP headers.
//! This module builds headers in the exact Chrome 130 order.

use stealth::{DeviceClass, StealthProfile};

/// Browser-aware nav header dispatch. Reads `profile.browser_name` and
/// returns the right header set for that browser family. Centralizes the
/// chrome / firefox / safari decision so callers (HttpClient, Page::navigate)
/// don't each need to repeat the match.
///
/// `accept_ch_upgraded` should be `true` only on requests that follow an
/// `Accept-CH` advertisement from the same origin. Chrome upgrades; Safari
/// and Firefox have no Client Hints so the flag is ignored for them.
pub fn nav_headers(profile: &StealthProfile, accept_ch_upgraded: bool) -> Vec<(String, String)> {
    match profile.browser_name.as_str() {
        "Firefox" => firefox_headers(profile),
        "Safari" => safari_headers(profile),
        _ if accept_ch_upgraded => chrome_headers_with_accept_ch(profile),
        _ => chrome_headers(profile),
    }
}

/// URL-aware nav header dispatch with per-region accept-language.
/// Same as `nav_headers` plus an `accept-language` override
/// when the target host's TLD has a well-known regional language
/// expectation (e.g. amazon-fr expects `fr-FR,fr;q=0.9,en-US;q=0.8`).
///
/// The override fires ONLY for TLDs in `region_languages_for_url`; for
/// TLDs without an entry (most), the profile's default accept-language is
/// preserved unchanged. The q-step matches the profile's browser family
/// (Chrome / Safari q=0.9; Firefox q=0.5).
pub fn nav_headers_for_url(
    profile: &StealthProfile,
    url: &str,
    accept_ch_upgraded: bool,
) -> Vec<(String, String)> {
    let mut hdrs = nav_headers(profile, accept_ch_upgraded);
    apply_region_accept_language(&mut hdrs, url, &profile.browser_name);
    hdrs
}

/// Replace `accept-language` in `hdrs` with the region-appropriate value
/// for `url`, using the browser family's q-step convention. No-op if the
/// URL's TLD has no regional override registered.
pub fn apply_region_accept_language(hdrs: &mut [(String, String)], url: &str, browser_name: &str) {
    let Some(langs) = region_languages_for_url(url) else {
        return;
    };
    let value = match browser_name {
        "Firefox" => build_firefox_accept_language(&langs),
        "Safari" => build_safari_accept_language(&langs),
        _ => build_accept_language(&langs),
    };
    for (k, v) in hdrs.iter_mut() {
        if k.eq_ignore_ascii_case("accept-language") {
            *v = value;
            return;
        }
    }
}

/// Per-TLD regional language list. Returns the language preference list
/// a real browser in that region would be expected to send.
///
/// Source of truth: real-Chrome captures from each region (e.g.
/// amazon.fr from a French Chrome installation sends
/// `fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7`). Returning `None` means the
/// TLD has no override — fall back to the profile's `languages` field.
///
/// English-language TLDs (.com, .net, .org, .uk, .com.au, .ca, etc.) are
/// intentionally NOT in this map: their visitors typically send the same
/// `en-US`-derived preference the profile already has, so overriding would
/// be a no-op at best and a profile-inconsistency at worst.
pub fn region_languages_for_url(url: &str) -> Option<Vec<String>> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host = host.trim_start_matches("www.");
    // Compound suffixes first (longest match wins).
    let tld = if host.ends_with(".co.jp") {
        ".co.jp"
    } else if host.ends_with(".com.br") {
        ".com.br"
    } else if host.ends_with(".com.mx") {
        ".com.mx"
    } else if host.ends_with(".com.tr") {
        ".com.tr"
    } else if host.ends_with(".com.cn") {
        ".com.cn"
    } else {
        let dot = host.rfind('.')?;
        &host[dot..]
    };
    let langs: &[&str] = match tld {
        ".fr" => &["fr-FR", "fr", "en-US", "en"],
        ".de" => &["de-DE", "de", "en-US", "en"],
        ".co.jp" | ".jp" => &["ja-JP", "ja", "en-US", "en"],
        ".it" => &["it-IT", "it", "en-US", "en"],
        ".es" => &["es-ES", "es", "en-US", "en"],
        ".nl" => &["nl-NL", "nl", "en-US", "en"],
        ".pl" => &["pl-PL", "pl", "en-US", "en"],
        ".se" => &["sv-SE", "sv", "en-US", "en"],
        ".no" => &["nb-NO", "no", "en-US", "en"],
        ".dk" => &["da-DK", "da", "en-US", "en"],
        ".fi" => &["fi-FI", "fi", "en-US", "en"],
        ".pt" => &["pt-PT", "pt", "en-US", "en"],
        ".com.br" => &["pt-BR", "pt", "en-US", "en"],
        ".com.mx" => &["es-MX", "es", "en-US", "en"],
        ".com.tr" | ".tr" => &["tr-TR", "tr", "en-US", "en"],
        ".com.cn" | ".cn" => &["zh-CN", "zh", "en-US", "en"],
        ".ru" => &["ru-RU", "ru", "en-US", "en"],
        ".kr" => &["ko-KR", "ko", "en-US", "en"],
        ".tw" => &["zh-TW", "zh", "en-US", "en"],
        ".vn" => &["vi-VN", "vi", "en-US", "en"],
        _ => return None,
    };
    Some(langs.iter().map(|s| s.to_string()).collect())
}

/// Browser-aware reload nav header dispatch.
pub fn nav_headers_reload(
    profile: &StealthProfile,
    referer: &str,
    accept_ch_upgraded: bool,
) -> Vec<(String, String)> {
    match profile.browser_name.as_str() {
        "Firefox" => firefox_headers_reload(profile, referer),
        "Safari" => safari_headers_reload(profile, referer),
        _ => chrome_headers_reload(profile, referer, accept_ch_upgraded),
    }
}

/// Browser-aware fetch (XHR/`window.fetch`) header dispatch.
///
/// The accept-language override applies for sub-resource fetches
/// using the **parent doc's origin** when provided (real Chrome sends one
/// accept-language per session, not per-URL — keying sub-resources off
/// the origin keeps a French amazon.fr session consistent on its CDN
/// fetches too).
pub fn nav_headers_fetch(
    profile: &StealthProfile,
    target_url: &str,
    origin: Option<&str>,
) -> Vec<(String, String)> {
    let mut hdrs = match profile.browser_name.as_str() {
        "Firefox" => firefox_headers_fetch(profile, target_url, origin),
        "Safari" => safari_headers_fetch(profile, target_url, origin),
        _ => chrome_headers_fetch(profile, target_url, origin),
    };
    let key_url = origin.unwrap_or(target_url);
    apply_region_accept_language(&mut hdrs, key_url, &profile.browser_name);
    hdrs
}

/// Build ordered Chrome browser headers from a stealth profile.
///
/// Returns headers as ordered (name, value) pairs matching the exact
/// header set and order real Chrome sends on a first-visit navigation
/// request. **Thirteen headers** — NO high-entropy Client Hints
/// (those only appear on follow-up requests after the server
/// advertises `Accept-CH` in a response).
///
/// Canonical Chrome 133/136/142 order per reference Chrome captures
/// (cross-version consistent for at least Chrome 133+):
/// 1. sec-ch-ua
/// 2. sec-ch-ua-mobile
/// 3. sec-ch-ua-platform
/// 4. upgrade-insecure-requests
/// 5. user-agent
/// 6. accept
/// 7. sec-fetch-site
/// 8. sec-fetch-mode
/// 9. sec-fetch-user
/// 10. sec-fetch-dest
/// 11. accept-encoding
/// 12. accept-language
/// 13. priority
///
/// High-entropy Client Hints (when `Accept-CH`-upgraded) splice in
/// between sec-ch-ua-platform and upgrade-insecure-requests.
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
pub fn chrome_headers_reload(
    profile: &StealthProfile,
    referer: &str,
    accept_ch_upgraded: bool,
) -> Vec<(String, String)> {
    // Real Chrome on a same-origin reload sends ONLY low-entropy CH
    // unless the previous response advertised `Accept-CH`. Sending
    // high-entropy hints unconditionally diverges from real Chrome —
    // confirmed by header captures: real Chrome 147
    // never sends sec-ch-ua-arch/bitness/full-version-list/etc on
    // first visits OR same-origin reloads (only after the server
    // has explicitly opted in via Accept-CH).
    let mut hdrs: Vec<(String, String)> = chrome_headers_impl(profile, accept_ch_upgraded)
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
/// completely different header sets; a "fetch" request that arrives
/// carrying navigation headers is a strong inconsistency that
/// fingerprinting layers key on.
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
    // sec-ch-ua-mobile is a LOW-entropy hint sent on EVERY request (nav AND
    // fetch/XHR). Hardcoding "?0" here made mobile profiles emit "?1" on the
    // document navigation (chrome_headers) but "?0" on every JS-initiated
    // subresource — a within-session Client-Hints contradiction that
    // edge layers read as inconsistent (on affected sites this stalled
    // hydration and produced an empty/thin body).
    // Mirror the nav path: derive from device_class. (High-entropy hints like
    // sec-ch-ua-model are correctly NOT sent on fetch unless the origin
    // persisted Accept-CH, so we don't add them here.)
    let is_mobile = matches!(
        profile.device_class,
        DeviceClass::MobileAndroid | DeviceClass::MobileIOS
    );
    headers.push((
        "sec-ch-ua-mobile".to_string(),
        if is_mobile { "?1" } else { "?0" }.to_string(),
    ));
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
        headers.push((
            "referer".to_string(),
            format!("{}/", o.trim_end_matches('/')),
        ));
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
/// Sending them when the server didn't ask for them diverges from real
/// Chrome and is a known fingerprint inconsistency.
pub fn chrome_headers_with_accept_ch(profile: &StealthProfile) -> Vec<(String, String)> {
    chrome_headers_impl(profile, true)
}

fn chrome_headers_impl(
    profile: &StealthProfile,
    include_high_entropy: bool,
) -> Vec<(String, String)> {
    let mut headers = Vec::with_capacity(if include_high_entropy { 20 } else { 13 });

    // 1. sec-ch-ua (Client Hints, low-entropy — always sent, FIRST per Chrome 133+)
    let sec_ch_ua = build_sec_ch_ua(profile);
    headers.push(("sec-ch-ua".to_string(), sec_ch_ua.clone()));
    let is_mobile = matches!(
        profile.device_class,
        DeviceClass::MobileAndroid | DeviceClass::MobileIOS
    );
    // 2. sec-ch-ua-mobile
    headers.push((
        "sec-ch-ua-mobile".to_string(),
        if is_mobile { "?1" } else { "?0" }.to_string(),
    ));
    // 3. sec-ch-ua-platform
    headers.push((
        "sec-ch-ua-platform".to_string(),
        format!("\"{}\"", profile.os_name),
    ));

    if include_high_entropy {
        // High-entropy hints. Order matches Chrome 147's actual emission
        // order observed via tls.peet.ws + browserleaks.com captures.
        //
        // Arch and bitness MUST come from profile fields, not be derived
        // from `platform`. `navigator.platform` is "MacIntel" on both
        // Intel Macs (arch=x86) and Apple Silicon Macs (arch=arm) — a
        // legacy fossil. Real Chrome on M3 reports `Sec-CH-UA-Arch: arm`
        // while keeping `navigator.platform: MacIntel`. Deriving from
        // platform here would emit "x86" and contradict the JS-side
        // `navigator.userAgentData.getHighEntropyValues({hints:['architecture']})`
        // which reads `profile.cpu_architecture` directly — fingerprinting
        // scripts cross-check these and reject on mismatch.
        headers.push((
            "sec-ch-ua-arch".to_string(),
            format!("\"{}\"", profile.cpu_architecture),
        ));
        headers.push((
            "sec-ch-ua-bitness".to_string(),
            format!("\"{}\"", profile.cpu_bitness),
        ));
        headers.push((
            "sec-ch-ua-full-version-list".to_string(),
            build_sec_ch_ua_full_version_list(profile),
        ));
        // sec-ch-ua-full-version (singular) is deprecated in favor of
        // -full-version-list, but some servers still list it in
        // critical-ch. Send it for compatibility — Chrome 147 still emits
        // it when servers ask. Confirmed against live server responses.
        headers.push((
            "sec-ch-ua-full-version".to_string(),
            format!("\"{}\"", profile.browser_version),
        ));
        // sec-ch-ua-model: empty on desktop, real model name on mobile.
        // Profile field `ua_model` is the source of truth — desktop presets
        // leave it empty; Pixel/Galaxy presets set it to "Pixel 9 Pro" etc.
        headers.push((
            "sec-ch-ua-model".to_string(),
            format!("\"{}\"", profile.ua_model),
        ));
        headers.push((
            "sec-ch-ua-platform-version".to_string(),
            format!(
                "\"{}\"",
                chrome_platform_version(&profile.os_name, &profile.os_version)
            ),
        ));
        headers.push((
            "sec-ch-ua-wow64".to_string(),
            if profile.ua_wow64 { "?1" } else { "?0" }.to_string(),
        ));
        // sec-ch-ua-form-factors: Chrome 130+ added this hint. "Mobile"
        // on phones, "Desktop" on PC. Lacks for older Chrome but landing
        // it for Chrome 147+ is correct.
        headers.push((
            "sec-ch-ua-form-factors".to_string(),
            if is_mobile {
                "\"Mobile\""
            } else {
                "\"Desktop\""
            }
            .to_string(),
        ));
        // sec-ch-device-memory — some servers demand it via accept-ch.
        // Per the W3 Device Memory spec
        // (https://www.w3.org/TR/device-memory/) the value MUST be one of
        // {0.25, 0.5, 1, 2, 4, 8}. Chrome's GetApproximateDeviceMemory
        // (third_party/blink/renderer/core/frame/navigator_device_memory.cc)
        // rounds the OS's reported RAM DOWN to the largest spec value ≤
        // the reported amount (so 16 GB → 8; 6 GB → 4; 0.7 GB → 0.5).
        // Sending an unquantized value (e.g. "6" or "16") is a tell.
        headers.push((
            "sec-ch-device-memory".to_string(),
            format!("{}", quantize_device_memory(profile.device_memory as f64)),
        ));
    }

    // 4. upgrade-insecure-requests
    headers.push(("upgrade-insecure-requests".to_string(), "1".to_string()));

    // 5. user-agent
    headers.push(("user-agent".to_string(), profile.user_agent.clone()));

    // 6. accept
    headers.push((
        "accept".to_string(),
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7".to_string(),
    ));

    // 7. sec-fetch headers
    headers.push(("sec-fetch-site".to_string(), "none".to_string()));
    headers.push(("sec-fetch-mode".to_string(), "navigate".to_string()));
    headers.push(("sec-fetch-user".to_string(), "?1".to_string()));
    headers.push(("sec-fetch-dest".to_string(), "document".to_string()));

    // 6. accept-encoding (Chrome 124+ includes zstd)
    headers.push((
        "accept-encoding".to_string(),
        "gzip, deflate, br, zstd".to_string(),
    ));

    // 7. accept-language
    let accept_language = build_accept_language(&profile.languages);
    headers.push(("accept-language".to_string(), accept_language));

    // 8. priority (Chrome 130+)
    headers.push(("priority".to_string(), "u=0, i".to_string()));

    headers
}

/// Quantize a RAM value (in GB) to the W3 Device Memory spec set
/// `{0.25, 0.5, 1, 2, 4, 8}` GB. Returns the largest spec value
/// that is ≤ `gb` (matches Chrome's `GetApproximateDeviceMemory`).
/// `gb` below 0.25 quantizes to 0.25 (the spec floor).
fn quantize_device_memory(gb: f64) -> f64 {
    const SPEC: [f64; 6] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
    if gb < SPEC[0] {
        return SPEC[0];
    }
    let mut out = SPEC[0];
    for &v in &SPEC {
        if v <= gb {
            out = v;
        }
    }
    out
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
/// **Chrome 147 live capture**: real Chrome 147 only sends this header AFTER an
/// `Accept-CH` advertisement. When sent, the format is:
/// ```text
/// "Google Chrome";v="147.0.7727.117", "Not.A/Brand";v="8.0.0.0", "Chromium";v="147.0.7727.117"
/// ```
/// Order: `Google Chrome`, `Not.A/Brand` middle, `Chromium`. The "Not"
/// brand format and version rotates per Chrome major release — Chrome 147
/// uses `Not.A/Brand` v="8" (was `Not-A.Brand` v="24" in Chrome 130-146).
/// Brand strings here MUST match `build_sec_ch_ua` exactly.
fn build_sec_ch_ua_full_version_list(profile: &StealthProfile) -> String {
    let v = &profile.browser_version;
    format!("\"Google Chrome\";v=\"{v}\", \"Not.A/Brand\";v=\"8.0.0.0\", \"Chromium\";v=\"{v}\"")
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
    let major_version = profile.browser_version.split('.').next().unwrap_or("147");

    // Real Chrome 147 sec-ch-ua:
    //   "Google Chrome";v="147", "Not.A/Brand";v="8", "Chromium";v="147"
    // Brand order is [Google Chrome, Not.A/Brand, Chromium] (NOT
    // alphabetical and NOT what the W3C spec implies). The "Not."-style
    // dummy brand changes per Chrome version — we hardcode the v=8 / dot-slash
    // form that matches Chrome 147+. Earlier Chrome (130 era) used
    // "Not-A.Brand";v="24" with brands ordered [Chromium, Not-A.Brand, Google Chrome]
    // — that's what we used to emit, but it diverges from modern Chrome.
    format!(
        "\"Google Chrome\";v=\"{v}\", \"Not.A/Brand\";v=\"8\", \"Chromium\";v=\"{v}\"",
        v = major_version
    )
}

// ============================================================================
// Firefox 135 header builder
// ----------------------------------------------------------------------------
// Empirical Firefox 135 header order from a real Firefox network capture.
// Firefox sends a distinctly different header set than Chrome:
//   - NO sec-ch-ua / sec-ch-ua-mobile / sec-ch-ua-platform — these are
//     Chrome-only (User Agent Client Hints aren't implemented in Firefox).
//   - NO `priority` header.
//   - `accept` is shorter: no avif/webp/apng/signed-exchange.
//   - `accept-language` quality values: q=0.5 not q=0.9.
//   - HTTP/1-style headers (`connection: keep-alive`, explicit `host:`)
//     surface in the request capture; over the wire on H2 they
//     become pseudo-headers that the HTTP/2 stack handles automatically.
//
// Header order (from capture):
// 1. host
// 2. user-agent
// 3. accept
// 4. accept-language
// 5. accept-encoding
// 6. connection
// 7. upgrade-insecure-requests
// 8. sec-fetch-dest
// 9. sec-fetch-mode
// 10. sec-fetch-site
// 11. sec-fetch-user
//
// `host` and `connection` are connection-level — most HTTP/2 clients write
// them as pseudo-headers, but listing them here ensures byte-equivalence
// with the reference capture if we ever serialize for diagnostic comparison.

/// Build ordered Firefox 135 nav headers from a stealth profile.
pub fn firefox_headers(profile: &StealthProfile) -> Vec<(String, String)> {
    firefox_headers_impl(profile, "none", true)
}

/// Same-origin reload variant — sec-fetch-site flips to "same-origin" and
/// sec-fetch-user is omitted (no user gesture).
pub fn firefox_headers_reload(profile: &StealthProfile, referer: &str) -> Vec<(String, String)> {
    let mut hdrs = firefox_headers_impl(profile, "same-origin", false);
    hdrs.push(("referer".to_string(), referer.to_string()));
    hdrs
}

/// Build headers for a `window.fetch()` request from JS (Firefox-class).
pub fn firefox_headers_fetch(
    profile: &StealthProfile,
    target_url: &str,
    origin: Option<&str>,
) -> Vec<(String, String)> {
    let mut headers = Vec::with_capacity(10);

    headers.push(("user-agent".to_string(), profile.user_agent.clone()));
    headers.push(("accept".to_string(), "*/*".to_string()));
    headers.push((
        "accept-language".to_string(),
        build_firefox_accept_language(&profile.languages),
    ));
    headers.push((
        "accept-encoding".to_string(),
        "gzip, deflate, br, zstd".to_string(),
    ));

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
    headers.push(("sec-fetch-dest".to_string(), "empty".to_string()));
    headers.push(("sec-fetch-mode".to_string(), "cors".to_string()));
    headers.push(("sec-fetch-site".to_string(), site.to_string()));

    if let Some(o) = origin {
        headers.push(("origin".to_string(), o.to_string()));
        headers.push((
            "referer".to_string(),
            format!("{}/", o.trim_end_matches('/')),
        ));
    }

    headers
}

fn firefox_headers_impl(
    profile: &StealthProfile,
    sec_fetch_site: &str,
    include_sec_fetch_user: bool,
) -> Vec<(String, String)> {
    let mut headers = Vec::with_capacity(9);

    // user-agent
    headers.push(("user-agent".to_string(), profile.user_agent.clone()));

    // accept — Firefox shorter form (no avif/webp/apng/signed-exchange)
    headers.push((
        "accept".to_string(),
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
    ));

    // accept-language — Firefox uses q=0.5 not q=0.9
    headers.push((
        "accept-language".to_string(),
        build_firefox_accept_language(&profile.languages),
    ));

    // accept-encoding (Firefox 135+ includes zstd)
    headers.push((
        "accept-encoding".to_string(),
        "gzip, deflate, br, zstd".to_string(),
    ));

    // NOTE: `connection: keep-alive` appeared in the reference Firefox
    // capture, but it's a connection-specific header forbidden in
    // HTTP/2 (RFC 7540 §8.1.2.2). The HTTP/2 stack strips it before
    // sending, but our http2 lib rejects it as malformed at insertion
    // time. Omit it here. Same for `host` — pseudo-header on HTTP/2.

    // upgrade-insecure-requests
    headers.push(("upgrade-insecure-requests".to_string(), "1".to_string()));

    // sec-fetch-* — Firefox supports these (added in v92).
    headers.push(("sec-fetch-dest".to_string(), "document".to_string()));
    headers.push(("sec-fetch-mode".to_string(), "navigate".to_string()));
    headers.push(("sec-fetch-site".to_string(), sec_fetch_site.to_string()));
    if include_sec_fetch_user {
        headers.push(("sec-fetch-user".to_string(), "?1".to_string()));
    }

    headers
}

// =============================================================================
// Safari iOS 18 header builders
// =============================================================================
//
// Safari does NOT send sec-fetch-*, sec-ch-ua-*, priority, or
// upgrade-insecure-requests. Header set is much shorter than Chrome's.
// Per real iOS Safari 18 captures and reference Safari signatures.
//
// The `Accept` value uses Safari's specific MIME ordering (no avif/webp/apng,
// no signed-exchange). `Accept-Encoding` excludes zstd (Safari has not adopted
// it as of iOS 18).

/// Build Safari headers for a fresh user navigation.
pub fn safari_headers(profile: &StealthProfile) -> Vec<(String, String)> {
    safari_headers_impl(profile, /*referer*/ None)
}

/// Same-origin reload variant — adds Referer header (no other deltas
/// because Safari doesn't have sec-fetch-*).
pub fn safari_headers_reload(profile: &StealthProfile, referer: &str) -> Vec<(String, String)> {
    safari_headers_impl(profile, Some(referer))
}

/// Build headers for a `window.fetch()` request from JS in Safari.
pub fn safari_headers_fetch(
    profile: &StealthProfile,
    target_url: &str,
    origin: Option<&str>,
) -> Vec<(String, String)> {
    let mut headers = Vec::with_capacity(7);
    headers.push(("accept".to_string(), "*/*".to_string()));
    headers.push((
        "accept-language".to_string(),
        build_safari_accept_language(&profile.languages),
    ));
    headers.push((
        "accept-encoding".to_string(),
        "gzip, deflate, br".to_string(),
    ));
    headers.push(("user-agent".to_string(), profile.user_agent.clone()));
    if let Some(o) = origin {
        headers.push(("origin".to_string(), o.to_string()));
        headers.push((
            "referer".to_string(),
            format!("{}/", o.trim_end_matches('/')),
        ));
    }
    let _ = target_url;
    headers
}

fn safari_headers_impl(profile: &StealthProfile, referer: Option<&str>) -> Vec<(String, String)> {
    // Canonical Safari iOS 18.4 header order per reference Safari captures:
    //   1. sec-fetch-dest: document
    //   2. user-agent
    //   3. accept
    //   4. sec-fetch-site: none
    //   5. sec-fetch-mode: navigate
    //   6. accept-language
    //   7. priority: u=0, i
    //   8. accept-encoding
    // (Host is a pseudo-header on h2; Cookie is added by the HttpClient layer.)
    // Note: Safari DOES send sec-fetch-{dest,site,mode} on top-level
    // navigations (since 16.4) but NOT sec-fetch-user. zstd absent (iOS 18
    // hasn't adopted it; iOS 26 ships it).
    let mut headers = Vec::with_capacity(9);

    headers.push(("sec-fetch-dest".to_string(), "document".to_string()));
    headers.push(("user-agent".to_string(), profile.user_agent.clone()));
    headers.push((
        "accept".to_string(),
        // Safari's specific Accept ordering — no avif/webp/apng, no signed-exchange.
        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string(),
    ));
    let site = if referer.is_some() {
        "same-origin"
    } else {
        "none"
    };
    headers.push(("sec-fetch-site".to_string(), site.to_string()));
    headers.push(("sec-fetch-mode".to_string(), "navigate".to_string()));
    headers.push((
        "accept-language".to_string(),
        build_safari_accept_language(&profile.languages),
    ));
    headers.push(("priority".to_string(), "u=0, i".to_string()));
    headers.push((
        "accept-encoding".to_string(),
        "gzip, deflate, br".to_string(),
    ));
    if let Some(r) = referer {
        headers.push(("referer".to_string(), r.to_string()));
    }
    headers
}

/// Safari Accept-Language uses q=0.9 step (same as Chrome) but with a
/// different second-language padding pattern. Conservative impl: same as
/// chrome until we verify the iOS-specific quirk worth modeling.
fn build_safari_accept_language(languages: &[String]) -> String {
    build_accept_language(languages)
}

/// Build accept-language Firefox-style — q=0.5 step instead of q=0.9.
/// Verified from real Firefox 135 capture.
fn build_firefox_accept_language(languages: &[String]) -> String {
    if languages.is_empty() {
        return "en-US,en;q=0.5".to_string();
    }
    let mut parts = Vec::with_capacity(languages.len());
    for (i, lang) in languages.iter().enumerate() {
        if i == 0 {
            parts.push(lang.clone());
        } else {
            // Firefox uses fixed q=0.5 for the first secondary, q=0.3 for
            // the next, etc. — verified pattern from real Firefox 135.
            let q = 0.5 - ((i - 1) as f64 * 0.2);
            if q > 0.0 {
                parts.push(format!("{};q={:.1}", lang, q));
            }
        }
    }
    parts.join(",")
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

// ================================================================
// Cross-origin isolation — COOP / COEP response-header parsing
// ----------------------------------------------------------------
// Fingerprinting scripts probe `self.crossOriginIsolated` and
// `typeof SharedArrayBuffer`. The browser sets `crossOriginIsolated = true`
// only when both Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy
// response headers are present with restrictive values:
//   COOP: same-origin
//   COEP: require-corp | credentialless
//
// See web.dev/articles/coop-coep.
// ================================================================

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoopValue {
    UnsafeNone,
    SameOriginAllowPopups,
    SameOrigin,
    NoopenerAllowPopups,
    RestrictProperties,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoepValue {
    UnsafeNone,
    RequireCorp,
    Credentialless,
}

/// Parsed COOP+COEP for a document response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentPolicy {
    pub coop: CoopValue,
    pub coep: CoepValue,
}

impl Default for DocumentPolicy {
    fn default() -> Self {
        Self {
            coop: CoopValue::UnsafeNone,
            coep: CoepValue::UnsafeNone,
        }
    }
}

/// Case-insensitive lookup of a single header value in a HashMap.
fn lookup_header<'a>(headers: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    let lower = name.to_ascii_lowercase();
    headers
        .iter()
        .find(|(k, _)| k.to_ascii_lowercase() == lower)
        .map(|(_, v)| v.as_str())
}

/// Extract the bare value token from a header value, stripping any
/// `;directive=value` suffixes (e.g. `report-to=`) and surrounding quotes.
fn bare_value(raw: &str) -> &str {
    let head = raw.split(';').next().unwrap_or(raw).trim();
    head.trim_matches('"')
}

fn parse_coop(raw: &str) -> CoopValue {
    match bare_value(raw).to_ascii_lowercase().as_str() {
        "same-origin" => CoopValue::SameOrigin,
        "same-origin-allow-popups" => CoopValue::SameOriginAllowPopups,
        "noopener-allow-popups" => CoopValue::NoopenerAllowPopups,
        "restrict-properties" => CoopValue::RestrictProperties,
        _ => CoopValue::UnsafeNone,
    }
}

fn parse_coep(raw: &str) -> CoepValue {
    match bare_value(raw).to_ascii_lowercase().as_str() {
        "require-corp" => CoepValue::RequireCorp,
        "credentialless" => CoepValue::Credentialless,
        _ => CoepValue::UnsafeNone,
    }
}

/// Parse Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy from a
/// response's headers. Missing headers default to `unsafe-none`.
pub fn parse_document_policy(headers: &HashMap<String, String>) -> DocumentPolicy {
    DocumentPolicy {
        coop: lookup_header(headers, "cross-origin-opener-policy")
            .map(parse_coop)
            .unwrap_or(CoopValue::UnsafeNone),
        coep: lookup_header(headers, "cross-origin-embedder-policy")
            .map(parse_coep)
            .unwrap_or(CoepValue::UnsafeNone),
    }
}

/// True iff the document satisfies cross-origin isolation requirements.
/// Per [web.dev/articles/coop-coep], COI requires:
///   COOP = same-origin
///   COEP = require-corp OR credentialless
pub fn is_cross_origin_isolated(policy: &DocumentPolicy) -> bool {
    matches!(policy.coop, CoopValue::SameOrigin)
        && matches!(
            policy.coep,
            CoepValue::RequireCorp | CoepValue::Credentialless
        )
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
    fn fetch_headers_mobile_flag_matches_nav() {
        // Regression: chrome_headers_fetch hardcoded
        // sec-ch-ua-mobile: ?0, contradicting the ?1 the nav request sends on
        // mobile (a within-session Client-Hints flip = inconsistency). The
        // low-entropy mobile flag MUST agree across nav and fetch.
        let pixel = stealth::presets::pixel_9_pro_chrome_148();
        let fh: std::collections::HashMap<_, _> =
            chrome_headers_fetch(&pixel, "https://example.com/x.js", Some("https://example.com"))
                .into_iter()
                .collect();
        assert_eq!(
            fh.get("sec-ch-ua-mobile").map(String::as_str),
            Some("?1"),
            "mobile profile fetch must emit sec-ch-ua-mobile: ?1 (matches nav)"
        );
        assert_eq!(
            fh.get("sec-ch-ua-platform").map(String::as_str),
            Some("\"Android\""),
            "mobile profile fetch must emit Android platform"
        );
        // Desktop stays ?0.
        let desk = stealth::presets::chrome_148_macos();
        let dfh: std::collections::HashMap<_, _> =
            chrome_headers_fetch(&desk, "https://example.com/x.js", Some("https://example.com"))
                .into_iter()
                .collect();
        assert_eq!(
            dfh.get("sec-ch-ua-mobile").map(String::as_str),
            Some("?0"),
            "desktop profile fetch must stay sec-ch-ua-mobile: ?0"
        );
    }

    #[test]
    fn pixel_android_emits_mobile_client_hints() {
        // Verify the pixel_9_pro_chrome_148 preset wires through to
        // mobile-flavored Sec-CH-UA-* headers.
        let profile = stealth::presets::pixel_9_pro_chrome_148();
        assert_eq!(profile.device_class, DeviceClass::MobileAndroid);
        let headers = chrome_headers_with_accept_ch(&profile);
        let h: std::collections::HashMap<_, _> = headers.iter().cloned().collect();

        // sec-ch-ua-mobile MUST be ?1 on mobile
        assert_eq!(
            h.get("sec-ch-ua-mobile").map(String::as_str),
            Some("?1"),
            "Pixel preset must emit sec-ch-ua-mobile: ?1"
        );
        // sec-ch-ua-platform MUST be Android
        assert_eq!(
            h.get("sec-ch-ua-platform").map(String::as_str),
            Some("\"Android\""),
            "Pixel preset must emit sec-ch-ua-platform: \"Android\""
        );
        // sec-ch-ua-model MUST be the Pixel display name (not codename)
        assert_eq!(
            h.get("sec-ch-ua-model").map(String::as_str),
            Some("\"Pixel 9 Pro\""),
            "Pixel preset must emit sec-ch-ua-model: \"Pixel 9 Pro\""
        );
        // sec-ch-ua-form-factors MUST be Mobile (Chrome 130+ adds this)
        assert_eq!(
            h.get("sec-ch-ua-form-factors").map(String::as_str),
            Some("\"Mobile\""),
            "Pixel preset must emit sec-ch-ua-form-factors: \"Mobile\""
        );
        // UA string MUST contain "Mobile" token
        assert!(
            profile.user_agent.contains("Mobile"),
            "Pixel UA must contain Mobile token, got: {}",
            profile.user_agent
        );
    }

    #[test]
    fn desktop_chrome_emits_desktop_client_hints() {
        // Sanity gate: existing desktop behavior unchanged after Phase 2
        // (zero-behavior-change invariant).
        let profile = stealth::presets::chrome_148_macos();
        assert_eq!(profile.device_class, DeviceClass::Desktop);
        let headers = chrome_headers_with_accept_ch(&profile);
        let h: std::collections::HashMap<_, _> = headers.iter().cloned().collect();

        assert_eq!(
            h.get("sec-ch-ua-mobile").map(String::as_str),
            Some("?0"),
            "Desktop must keep emitting sec-ch-ua-mobile: ?0"
        );
        assert_eq!(
            h.get("sec-ch-ua-form-factors").map(String::as_str),
            Some("\"Desktop\"")
        );
        // Model is empty on desktop (it's "" in the profile)
        assert_eq!(h.get("sec-ch-ua-model").map(String::as_str), Some("\"\""));
    }

    #[test]
    fn firefox_headers_have_no_sec_ch_ua() {
        // Firefox doesn't implement User Agent Client Hints — no
        // sec-ch-ua* headers should appear.
        let profile = stealth::presets::firefox_135_macos();
        let headers = firefox_headers(&profile);
        for (k, _) in &headers {
            assert!(
                !k.starts_with("sec-ch-ua"),
                "Firefox headers must not contain {k}"
            );
        }
        // Also no `priority` header (Chrome-only).
        assert!(
            !headers.iter().any(|(k, _)| k == "priority"),
            "Firefox should not emit `priority` header"
        );
    }

    #[test]
    fn firefox_headers_have_correct_accept() {
        let profile = stealth::presets::firefox_135_macos();
        let headers = firefox_headers(&profile);
        let accept = headers.iter().find(|(k, _)| k == "accept").unwrap();
        // Firefox's accept lacks avif/webp/apng/signed-exchange that Chrome includes.
        assert_eq!(
            accept.1,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
        );
    }

    #[test]
    fn firefox_accept_language_uses_q_05() {
        let result = build_firefox_accept_language(&["en-US".to_string(), "en".to_string()]);
        // Firefox uses q=0.5 for the secondary language, not q=0.9.
        assert_eq!(result, "en-US,en;q=0.5");
    }

    #[test]
    fn firefox_headers_count_is_nine() {
        // 9 headers: user-agent, accept, accept-language, accept-encoding,
        // upgrade-insecure-requests, sec-fetch-dest/mode/site/user.
        // `host` and `connection` are HTTP/1-style — both are pseudo-headers
        // / forbidden in HTTP/2 (RFC 7540 §8.1.2.2), the HTTP/2 stack
        // handles them automatically. Omitting them avoids "malformed
        // headers" errors at the http2 lib insertion layer.
        let profile = stealth::presets::firefox_135_macos();
        let headers = firefox_headers(&profile);
        assert_eq!(headers.len(), 9);
    }

    #[test]
    fn chrome_headers_first_visit_is_low_entropy_only() {
        // Real Chrome 130 first-visit navigation has 13 headers and
        // does NOT include the high-entropy Client Hints. Those only
        // appear on requests that follow an `Accept-CH` advertisement.
        let profile = stealth::chrome_148_windows();
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
        let profile = stealth::chrome_148_windows();
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
        // Chrome 147+ live capture format:
        //   "Google Chrome";v="<ver>", "Not.A/Brand";v="8.0.0.0", "Chromium";v="<ver>"
        // The "Not" brand name rotates across major releases (was `Not-A.Brand`
        // v="24" in Chrome 130-146; changed to `Not.A/Brand` v="8" in Chrome 147+).
        let profile = stealth::chrome_148_linux();
        let value = build_sec_ch_ua_full_version_list(&profile);
        assert!(value.contains("Google Chrome"));
        assert!(value.contains(&profile.browser_version));
        assert!(value.contains("Not.A/Brand"));
        // Brand order: Google Chrome first, Not.A/Brand middle, Chromium last.
        let google_idx = value.find("Google Chrome").unwrap();
        let not_idx = value.find("Not.A/Brand").unwrap();
        let chromium_idx = value.find("Chromium").unwrap();
        assert!(google_idx < not_idx);
        assert!(not_idx < chromium_idx);
    }

    #[test]
    fn platform_version_triple_padded() {
        assert_eq!(chrome_platform_version("Windows", "10.0"), "10.0.0");
        assert_eq!(chrome_platform_version("Windows", "11"), "11.0.0");
        assert_eq!(chrome_platform_version("macOS", "15.2"), "15.2.0");
        assert_eq!(chrome_platform_version("Linux", "anything"), "");
    }

    #[test]
    fn device_memory_quantizes_to_w3_spec_set() {
        // Spec values must round-trip exactly
        assert_eq!(quantize_device_memory(8.0), 8.0);
        assert_eq!(quantize_device_memory(4.0), 4.0);
        assert_eq!(quantize_device_memory(2.0), 2.0);
        assert_eq!(quantize_device_memory(1.0), 1.0);
        assert_eq!(quantize_device_memory(0.5), 0.5);
        assert_eq!(quantize_device_memory(0.25), 0.25);
        // Above 8 quantizes down to 8 (Chrome caps at 8 even on 16/32 GB)
        assert_eq!(quantize_device_memory(16.0), 8.0);
        assert_eq!(quantize_device_memory(32.0), 8.0);
        // Non-spec intermediate values round DOWN to the next spec value
        assert_eq!(quantize_device_memory(6.0), 4.0);
        assert_eq!(quantize_device_memory(3.0), 2.0);
        assert_eq!(quantize_device_memory(1.5), 1.0);
        assert_eq!(quantize_device_memory(0.7), 0.5);
        assert_eq!(quantize_device_memory(0.3), 0.25);
        // Below spec floor → spec floor (Chrome reports 0.25 on a 256MB device)
        assert_eq!(quantize_device_memory(0.1), 0.25);
        assert_eq!(quantize_device_memory(0.0), 0.25);
    }

    #[test]
    fn sec_ch_device_memory_emits_quantized_value() {
        let mut profile = stealth::chrome_148_macos();
        profile.device_memory = 16; // common Apple Silicon spec
        let headers = chrome_headers_with_accept_ch(&profile);
        let dm = headers
            .iter()
            .find(|(k, _)| k == "sec-ch-device-memory")
            .expect("sec-ch-device-memory present in accept-ch variant");
        // 16 GB → quantized to 8
        assert_eq!(dm.1, "8");

        profile.device_memory = 6; // pretend mid-range
        let headers = chrome_headers_with_accept_ch(&profile);
        let dm = headers
            .iter()
            .find(|(k, _)| k == "sec-ch-device-memory")
            .unwrap();
        // 6 GB → quantized DOWN to 4 (Chrome rounds down to nearest spec value)
        assert_eq!(dm.1, "4");
    }

    #[test]
    fn sec_ch_ua_arch_reads_profile_cpu_architecture() {
        // Apple Silicon macOS: platform="MacIntel" (legacy fossil) but
        // cpu_architecture="arm". Real Chrome on M3 emits
        // `sec-ch-ua-arch: "arm"`, NOT "x86" — and the JS-side
        // `navigator.userAgentData.architecture` reads profile.cpu_architecture
        // directly. The HTTP header must agree with JS or fingerprinting scripts reject.
        let mut profile = stealth::chrome_148_macos();
        profile.cpu_architecture = "arm".into();
        let headers = chrome_headers_with_accept_ch(&profile);
        let arch = headers
            .iter()
            .find(|(k, _)| k == "sec-ch-ua-arch")
            .expect("sec-ch-ua-arch present in accept-ch variant");
        assert_eq!(
            arch.1, "\"arm\"",
            "arch must reflect profile.cpu_architecture"
        );

        // Intel Mac path:
        profile.cpu_architecture = "x86".into();
        let headers = chrome_headers_with_accept_ch(&profile);
        let arch = headers.iter().find(|(k, _)| k == "sec-ch-ua-arch").unwrap();
        assert_eq!(arch.1, "\"x86\"");
    }

    #[test]
    fn sec_ch_ua_bitness_reads_profile_cpu_bitness() {
        let mut profile = stealth::chrome_148_windows();
        profile.cpu_bitness = "32".into();
        // wow64 only valid when cpu_bitness=32 + os=Windows.
        profile.ua_wow64 = true;
        let headers = chrome_headers_with_accept_ch(&profile);
        let bitness = headers
            .iter()
            .find(|(k, _)| k == "sec-ch-ua-bitness")
            .unwrap();
        assert_eq!(bitness.1, "\"32\"");
        let wow = headers
            .iter()
            .find(|(k, _)| k == "sec-ch-ua-wow64")
            .unwrap();
        assert_eq!(wow.1, "?1", "wow64 hint must reflect profile.ua_wow64");
    }

    // ============================================================
    // COOP / COEP / cross-origin isolation tests (gap #30)
    // ============================================================

    #[test]
    fn coi_default_when_headers_absent() {
        let headers: HashMap<String, String> = HashMap::new();
        let policy = parse_document_policy(&headers);
        assert_eq!(policy.coop, CoopValue::UnsafeNone);
        assert_eq!(policy.coep, CoepValue::UnsafeNone);
        assert!(!is_cross_origin_isolated(&policy));
    }

    #[test]
    fn coi_true_with_same_origin_and_require_corp() {
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("cross-origin-opener-policy".into(), "same-origin".into());
        headers.insert("cross-origin-embedder-policy".into(), "require-corp".into());
        let policy = parse_document_policy(&headers);
        assert!(is_cross_origin_isolated(&policy));
    }

    #[test]
    fn coi_true_with_same_origin_and_credentialless() {
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("cross-origin-opener-policy".into(), "same-origin".into());
        headers.insert(
            "cross-origin-embedder-policy".into(),
            "credentialless".into(),
        );
        let policy = parse_document_policy(&headers);
        assert!(is_cross_origin_isolated(&policy));
    }

    #[test]
    fn coi_false_with_only_coop() {
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("cross-origin-opener-policy".into(), "same-origin".into());
        let policy = parse_document_policy(&headers);
        assert!(!is_cross_origin_isolated(&policy));
    }

    #[test]
    fn coi_false_with_same_origin_allow_popups() {
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert(
            "cross-origin-opener-policy".into(),
            "same-origin-allow-popups".into(),
        );
        headers.insert("cross-origin-embedder-policy".into(), "require-corp".into());
        let policy = parse_document_policy(&headers);
        // same-origin-allow-popups does NOT qualify per spec.
        assert!(!is_cross_origin_isolated(&policy));
    }

    #[test]
    fn coi_parser_strips_directives_and_quotes() {
        // "same-origin"; report-to=foo  --> CoopValue::SameOrigin
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert(
            "cross-origin-opener-policy".into(),
            "\"same-origin\"; report-to=\"foo\"".into(),
        );
        headers.insert("cross-origin-embedder-policy".into(), "require-corp".into());
        let policy = parse_document_policy(&headers);
        assert_eq!(policy.coop, CoopValue::SameOrigin);
        assert!(is_cross_origin_isolated(&policy));
    }

    #[test]
    fn coi_case_insensitive_header_lookup() {
        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("Cross-Origin-Opener-Policy".into(), "same-origin".into());
        headers.insert("Cross-Origin-Embedder-Policy".into(), "require-corp".into());
        let policy = parse_document_policy(&headers);
        assert!(is_cross_origin_isolated(&policy));
    }

    #[test]
    fn client_hints_match_profile_version() {
        // Invariant: the sec-ch-ua value and the sec-ch-ua-full-version-list value
        // must both reference the same major version, otherwise detection scripts
        // that cross-check the two get a free signal. Checked against the
        // Accept-CH variant because that's the one that carries both values.
        let profile = stealth::chrome_148_windows();
        let headers = chrome_headers_with_accept_ch(&profile);
        let sec_ch_ua = headers
            .iter()
            .find(|(k, _)| k == "sec-ch-ua")
            .unwrap()
            .1
            .clone();
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

    // ===== per-region accept-language =====

    #[test]
    fn region_languages_amazon_fr() {
        let langs = region_languages_for_url("https://www.amazon.fr/").unwrap();
        assert_eq!(langs, vec!["fr-FR", "fr", "en-US", "en"]);
    }

    #[test]
    fn region_languages_amazon_jp_compound_tld() {
        let langs = region_languages_for_url("https://www.amazon.co.jp/").unwrap();
        assert_eq!(langs, vec!["ja-JP", "ja", "en-US", "en"]);
    }

    #[test]
    fn region_languages_amazon_com_no_override() {
        // .com has no regional override — profile's default carries.
        assert!(region_languages_for_url("https://www.amazon.com/").is_none());
    }

    #[test]
    fn region_languages_amazon_co_uk_no_override() {
        // English-language TLDs intentionally omitted — they share en-US.
        assert!(region_languages_for_url("https://www.amazon.co.uk/").is_none());
    }

    #[test]
    fn nav_headers_for_url_overrides_amazon_fr() {
        let profile = stealth::chrome_148_macos();
        let hdrs = nav_headers_for_url(&profile, "https://www.amazon.fr/", false);
        let al = hdrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("accept-language"))
            .expect("accept-language present");
        assert_eq!(al.1, "fr-FR,fr;q=0.9,en-US;q=0.8,en;q=0.7");
    }

    #[test]
    fn nav_headers_for_url_overrides_amazon_de_with_firefox_q_step() {
        let profile = stealth::firefox_135_macos();
        let hdrs = nav_headers_for_url(&profile, "https://www.amazon.de/", false);
        let al = hdrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("accept-language"))
            .expect("accept-language present");
        // Firefox q-step: 0.5, 0.3, 0.1 — verified `build_firefox_accept_language`.
        assert_eq!(al.1, "de-DE,de;q=0.5,en-US;q=0.3,en;q=0.1");
    }

    #[test]
    fn nav_headers_for_url_no_change_on_amazon_com() {
        let profile = stealth::chrome_148_macos();
        let hdrs = nav_headers_for_url(&profile, "https://www.amazon.com/", false);
        let al = hdrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("accept-language"))
            .expect("accept-language present");
        // Profile default — must match what nav_headers produces directly.
        let baseline = nav_headers(&profile, false);
        let baseline_al = baseline
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("accept-language"))
            .unwrap();
        assert_eq!(al.1, baseline_al.1);
    }
}
