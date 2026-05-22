//! JA4H — HTTP-layer fingerprint hash. **TEST-ONLY** (see LICENSE-NOTE.md).
//!
//! JA4H is patent-pending under FoxIO License 1.1 (non-commercial). This
//! file is `#[cfg(test)]`-gated so the function is **never exposed to
//! production binaries**, fitting FoxIO's "internal testing/evaluation"
//! carve-out. Do NOT export this function or call it from production code.
//!
//! Used as a regression oracle: assert that our `chrome_headers()` produce
//! a stable JA4H per profile (locked-in baseline), and cross-check against
//! `tls.peet.ws/api/all` in network-gated tests.
//!
//! ## Spec recap (from FoxIO `python/ja4h.py`):
//!
//! ```text
//! JA4H = {method2}{ver2}{c|n}{r|n}{hdr_count2}{lang4}_{hdr_hash12}_{ck_hash12}_{ck_val_hash12}
//! ```
//!
//! - `method2`: lowercase 2 chars (ge, po, pu, de, ...)
//! - `ver2`: 10|11|20|30
//! - `c|n`: cookie present
//! - `r|n`: referer present
//! - `hdr_count2`: zero-padded count, EXCLUDING Cookie/Referer/pseudo-headers, capped at 99
//! - `lang4`: first accept-language token, `-` and `;` stripped, lowercased, padded with `0` to 4 chars
//! - `hdr_hash12`: first 12 hex of sha256(",".join(header_names_in_request_order_excluding_cookie_referer_pseudo))
//! - `ck_hash12`: first 12 hex of sha256(",".join(cookie_names_sorted_alphabetically)), or "000000000000"
//! - `ck_val_hash12`: first 12 hex of sha256(",".join("name=value"_sorted_by_name)), or "000000000000"

use sha2::{Digest, Sha256};

/// Compute JA4H for a request. `headers` is in wire order (insertion order).
/// `version` is one of 10, 11, 20, 30.
pub fn ja4h(method: &str, version: u8, headers: &[(String, String)]) -> String {
    let m = method.to_ascii_lowercase();
    let m2: String = m.chars().take(2).collect();
    let ver2 = match version {
        10 => "10",
        11 => "11",
        20 => "20",
        30 => "30",
        _ => "11",
    };

    let mut cookie_present = false;
    let mut referer_present = false;
    let mut cookie_str: Option<&str> = None;
    let mut lang: Option<&str> = None;
    let mut hdr_names_wire: Vec<String> = Vec::new();

    for (k, v) in headers {
        let lk = k.to_ascii_lowercase();
        if lk.starts_with(':') {
            continue; // skip HTTP/2 pseudo-headers
        }
        match lk.as_str() {
            "cookie" => {
                cookie_present = true;
                cookie_str = Some(v.as_str());
            }
            "referer" => {
                referer_present = true;
            }
            _ => {
                if lk == "accept-language" && lang.is_none() {
                    lang = Some(v.as_str());
                }
                hdr_names_wire.push(lk);
            }
        }
    }

    let cn = if cookie_present { 'c' } else { 'n' };
    let rn = if referer_present { 'r' } else { 'n' };
    let count = format!("{:02}", hdr_names_wire.len().min(99));
    let lang4 = lang.map(format_lang).unwrap_or_else(|| "0000".into());
    let hdr_hash = sha12(&hdr_names_wire.join(","));

    let (ck_hash, ck_val_hash) = if let Some(cs) = cookie_str {
        let mut pairs: Vec<(&str, &str)> = cs
            .split(';')
            .map(|p| {
                let p = p.trim();
                let (n, _) = p.split_once('=').unwrap_or((p, ""));
                (n, p)
            })
            .collect();
        pairs.sort_by_key(|(n, _)| *n);
        let names: Vec<&str> = pairs.iter().map(|(n, _)| *n).collect();
        let pairs_s: Vec<&str> = pairs.iter().map(|(_, p)| *p).collect();
        (sha12(&names.join(",")), sha12(&pairs_s.join(",")))
    } else {
        ("000000000000".into(), "000000000000".into())
    };

    format!("{m2}{ver2}{cn}{rn}{count}{lang4}_{hdr_hash}_{ck_hash}_{ck_val_hash}")
}

fn sha12(s: &str) -> String {
    let h = Sha256::digest(s.as_bytes());
    hex::encode(&h[..6]) // 6 bytes = 12 hex chars
}

fn format_lang(v: &str) -> String {
    let first = v.split(',').next().unwrap_or("").trim();
    let stripped: String = first
        .chars()
        .filter(|c| *c != '-' && *c != ';')
        .flat_map(|c| c.to_lowercase())
        .collect();
    let mut s: String = stripped.chars().take(4).collect();
    while s.len() < 4 {
        s.push('0');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn ja4h_format_get_h2_no_cookie_no_referer_en_us() {
        let headers = h(&[
            (":method", "GET"),
            (":authority", "example.com"),
            (":scheme", "https"),
            (":path", "/"),
            ("user-agent", "Mozilla/5.0"),
            ("accept", "*/*"),
            ("accept-language", "en-US,en;q=0.9"),
        ]);
        let s = ja4h("GET", 20, &headers);
        // Format: ge20nn03enus_<hash>_000000000000_000000000000
        assert!(s.starts_with("ge20nn03enus_"), "got {s}");
        assert!(s.ends_with("_000000000000_000000000000"));
        assert_eq!(s.split('_').count(), 4);
    }

    #[test]
    fn ja4h_lang4_strips_dash_semicolon_and_pads() {
        // ru-RU,ru;q=0.9 → "ruru" (4 chars, all 4 used, no padding needed)
        let headers = h(&[("accept-language", "ru-RU,ru;q=0.9")]);
        let s = ja4h("GET", 20, &headers);
        assert!(s.contains("ruru"), "ru-RU should normalize to ruru: {s}");
    }

    #[test]
    fn ja4h_lang4_pads_short_lang() {
        // ja → "ja00"
        let headers = h(&[("accept-language", "ja,en;q=0.5")]);
        let s = ja4h("GET", 20, &headers);
        assert!(s.contains("ja00"), "ja should pad to ja00: {s}");
    }

    #[test]
    fn ja4h_no_lang_returns_zeros() {
        let headers = h(&[("accept", "*/*")]);
        let s = ja4h("GET", 20, &headers);
        // method=ge ver=20 cn=n rn=n count=01 lang=0000 → "ge20nn010000_..."
        assert!(s.starts_with("ge20nn010000_"), "got {s}");
    }

    #[test]
    fn ja4h_cookie_present_flag_and_hashes() {
        let headers = h(&[("user-agent", "x"), ("cookie", "z=2; a=1; m=3")]);
        let s = ja4h("GET", 20, &headers);
        // cookie present → 'c'; count excludes cookie so count=01 (user-agent only)
        assert!(s.contains("ge20cn010000_"), "got {s}");
        // Cookie hashes are non-zero
        let parts: Vec<&str> = s.split('_').collect();
        assert_ne!(parts[2], "000000000000");
        assert_ne!(parts[3], "000000000000");
    }

    #[test]
    fn ja4h_cookie_names_sorted_alphabetically() {
        // The cookie name hash should be the same regardless of input order
        // because sort happens before hashing.
        let h1 = h(&[("cookie", "z=2; a=1; m=3")]);
        let h2 = h(&[("cookie", "m=3; z=2; a=1")]);
        let s1 = ja4h("GET", 20, &h1);
        let s2 = ja4h("GET", 20, &h2);
        let p1: Vec<&str> = s1.split('_').collect();
        let p2: Vec<&str> = s2.split('_').collect();
        assert_eq!(p1[2], p2[2], "cookie name hash must be sort-invariant");
        assert_eq!(
            p1[3], p2[3],
            "cookie name=value hash must be sort-invariant"
        );
    }

    #[test]
    fn ja4h_referer_flag() {
        let headers = h(&[("user-agent", "x"), ("referer", "https://example.com/")]);
        let s = ja4h("GET", 20, &headers);
        // referer present → 'r'; count excludes referer so count=01
        assert!(s.contains("ge20nr010000_"), "got {s}");
    }

    #[test]
    fn ja4h_hdr_count_excludes_pseudo_cookie_referer() {
        let headers = h(&[
            (":method", "GET"),
            (":path", "/"),
            ("user-agent", "x"),
            ("accept", "*/*"),
            ("cookie", "k=v"),
            ("referer", "/"),
            ("x-foo", "1"),
        ]);
        let s = ja4h("GET", 20, &headers);
        // Counted: user-agent, accept, x-foo = 3
        assert!(s.contains("ge20cr030000_"), "got {s}");
    }

    #[test]
    fn ja4h_method_version_combos() {
        let h11 = h(&[("user-agent", "x")]);
        assert!(ja4h("POST", 11, &h11).starts_with("po11nn010000_"));
        assert!(ja4h("PUT", 11, &h11).starts_with("pu11nn010000_"));
        assert!(ja4h("DELETE", 30, &h11).starts_with("de30nn010000_"));
    }

    #[test]
    fn ja4h_hdr_order_matters_for_hash() {
        let a = h(&[("user-agent", "x"), ("accept", "*/*")]);
        let b = h(&[("accept", "*/*"), ("user-agent", "x")]);
        let sa = ja4h("GET", 20, &a);
        let sb = ja4h("GET", 20, &b);
        let pa: Vec<&str> = sa.split('_').collect();
        let pb: Vec<&str> = sb.split('_').collect();
        // The hdr_hash12 (segment 1) reflects wire order — different orders
        // produce different hashes.
        assert_ne!(pa[1], pb[1], "header hash must reflect wire order");
    }

    // ============================================================
    // Per-profile JA4H stability tests (P1.4c)
    // ============================================================
    // For each shipped Chrome 130 preset, compute the navigation JA4H
    // and assert:
    //   - The format prefix is `ge20nn13<lang4>_<hash>_000000000000_000000000000`
    //     (GET, h2, no-cookie, no-referer, 13 navigation headers, no
    //     cookie hashes — first-visit nav has no Cookie header).
    //   - The hdr_hash12 segment is identical across profiles because all
    //     profiles use the same 13 header *names* in the same order
    //     (lang4 differs because that's a header *value*).
    //   - lang4 differs across locales (us=enus, ru=ruru, cn=zhcn, etc).

    use crate::headers::{chrome_headers, chrome_headers_with_accept_ch};

    fn nav_ja4h_for(profile: &stealth::StealthProfile) -> String {
        ja4h("GET", 20, &chrome_headers(profile))
    }

    #[test]
    fn ja4h_chrome_148_windows_format() {
        let s = nav_ja4h_for(&stealth::presets::chrome_148_windows());
        // 13 nav headers, en-US locale
        assert!(s.starts_with("ge20nn13enus_"), "got {s}");
        assert!(s.ends_with("_000000000000_000000000000"));
    }

    #[test]
    fn test_ja4h_hdr_hash_reference() {
        let profile = stealth::presets::chrome_148_macos();
        let s = nav_ja4h_for(&profile);
        // Canonical hash for Chrome 133+ navigation header order
        // (sec-ch-ua trio FIRST per curl-impersonate
        // tests/signatures/chrome_142.0.7444.176.yaml).
        assert!(
            s.contains("_0c2c1d640f3e_"),
            "JA4H hash mismatch: expected 0c2c1d640f3e in {s}"
        );
    }

    #[test]
    fn ja4h_chrome_148_macos_format() {
        let s = nav_ja4h_for(&stealth::presets::chrome_148_macos());
        assert!(s.starts_with("ge20nn13enus_"), "got {s}");
    }

    #[test]
    fn ja4h_chrome_148_linux_format() {
        let s = nav_ja4h_for(&stealth::presets::chrome_148_linux());
        assert!(s.starts_with("ge20nn13enus_"), "got {s}");
    }

    #[test]
    fn ja4h_chrome_148_ru_uses_ruru_lang() {
        let s = nav_ja4h_for(&stealth::presets::chrome_148_ru());
        assert!(s.starts_with("ge20nn13ruru_"), "got {s}");
    }

    #[test]
    fn ja4h_chrome_148_cn_uses_zhcn_lang() {
        let s = nav_ja4h_for(&stealth::presets::chrome_148_cn());
        assert!(s.starts_with("ge20nn13zhcn_"), "got {s}");
    }

    #[test]
    fn ja4h_hdr_hash_identical_across_profiles_for_navigation() {
        // chrome_headers() produces the same 13 header names in the same
        // order across all profiles (only the values differ). So the
        // hdr_hash12 segment must match exactly.
        let win = nav_ja4h_for(&stealth::presets::chrome_148_windows());
        let mac = nav_ja4h_for(&stealth::presets::chrome_148_macos());
        let lin = nav_ja4h_for(&stealth::presets::chrome_148_linux());
        let ru = nav_ja4h_for(&stealth::presets::chrome_148_ru());
        let cn = nav_ja4h_for(&stealth::presets::chrome_148_cn());

        let hash = |s: &str| s.split('_').nth(1).unwrap().to_string();
        let h_win = hash(&win);
        assert_eq!(hash(&mac), h_win, "macos hdr hash differs");
        assert_eq!(hash(&lin), h_win, "linux hdr hash differs");
        assert_eq!(hash(&ru), h_win, "ru hdr hash differs");
        assert_eq!(hash(&cn), h_win, "cn hdr hash differs");
    }

    #[test]
    fn ja4h_accept_ch_variant_has_more_headers() {
        // chrome_headers_with_accept_ch() adds high-entropy Client Hints,
        // bringing total navigation headers from 13 to ~19. The hdr count
        // segment must change accordingly.
        let profile = stealth::presets::chrome_148_windows();
        let nav = ja4h("GET", 20, &chrome_headers(&profile));
        let acc = ja4h("GET", 20, &chrome_headers_with_accept_ch(&profile));
        // Both start with "ge20nn"; nav has count=13, acc has count > 13.
        assert!(nav.starts_with("ge20nn13"));
        let acc_count: u8 = acc[6..8].parse().expect("count is two digits");
        assert!(
            acc_count > 13,
            "Accept-CH variant should have >13 headers, got {acc_count}"
        );
        // Different header set → different hdr_hash12.
        let h_nav = nav.split('_').nth(1).unwrap();
        let h_acc = acc.split('_').nth(1).unwrap();
        assert_ne!(h_nav, h_acc);
    }

    #[test]
    fn ja4h_navigation_format_is_first_visit_no_cookies() {
        // First-visit navigation requests carry no Cookie header → both
        // cookie-hash segments must be the all-zeros sentinel.
        let s = nav_ja4h_for(&stealth::presets::chrome_148_windows());
        let parts: Vec<&str> = s.split('_').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[2], "000000000000");
        assert_eq!(parts[3], "000000000000");
    }
}
