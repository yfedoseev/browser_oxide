//! W3.8 — DataDome interstitial detector + solver scaffolding.
//!
//! When DataDome scoring fails the silent path, the response body looks
//! like the canonical 1.4 KB shape (per docs/research_2026_05_14/03_DATADOME.md
//! §2):
//!
//! ```html
//! <html lang="en"><head><title><site></title>...</head>
//! <body>
//!   <p>Please enable JS and disable any ad blocker</p>
//!   <script>var dd={ 'rt':'i', 'cid':'...', 'hsh':'...', 'b':1005349,
//!                    's':43909, 'e':'...', 'qp':'',
//!                    'host':'geo.captcha-delivery.com',
//!                    'cookie':'...' }</script>
//!   <script src="https://ct.captcha-delivery.com/i.js"></script>
//! </body></html>
//! ```
//!
//! Parsing the `dd={...}` JS object literal extracts the challenge
//! parameters. Solving requires:
//!   - Loading `ct.captcha-delivery.com/i.js` (or `c.js` for slider)
//!   - Running the encrypted DataDome verification protocol
//!     (dual-XOR PRNG per 03_DATADOME.md §4)
//!   - POSTing the result to captcha-delivery.com
//!   - Receiving a new `datadome=` cookie
//!
//! This module implements the detector + parameter extractor. Full
//! solver is staged for a follow-up commit — the engine path needs
//! ~150 LOC of dual-XOR PRNG plus iframe-realm WASM execution support
//! for `boring_challenge` (Picasso canvas + audio fingerprint).
//!
//! For now we surface "DataDome interstitial detected" as a clear
//! telemetry signal so sweep diagnoses point at the right vendor.

/// Parsed DataDome interstitial challenge parameters extracted from the
/// inline `<script>var dd={...}</script>` block. Field names mirror
/// DataDome's wire-level keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdInterstitial {
    /// Response type: `'i'` for invisible (no slider) or `'c'` for
    /// captcha (slider widget).
    pub rt: String,
    /// Client id (per-tenant identifier).
    pub cid: String,
    /// Hash of the failed request signal vector.
    pub hsh: String,
    /// Brand/site code.
    pub b: i64,
    /// Server/edge code.
    pub s: i64,
    /// Encrypted verification token.
    pub e: String,
    /// Query parameters carried through to the challenge iframe.
    pub qp: String,
    /// Challenge host — typically `geo.captcha-delivery.com`.
    pub host: String,
    /// Pre-allocated datadome cookie used as challenge nonce input.
    pub cookie: String,
}

/// Detect and parse a DataDome interstitial response body.
///
/// Returns `Some(DdInterstitial)` iff the body is the canonical small
/// interstitial shape (≤ 2 KB, contains `captcha-delivery.com` and a
/// `var dd={...}` literal). Returns `None` otherwise — the caller
/// should then proceed normally.
pub fn detect_datadome_interstitial(body: &str) -> Option<DdInterstitial> {
    // Size gate — real DataDome interstitials are < 2 KB. Larger bodies
    // mentioning captcha-delivery.com are typically legitimate pages
    // that reference DataDome assets in their headers/scripts.
    if body.len() > 4096 {
        return None;
    }
    if !body.contains("captcha-delivery.com") {
        return None;
    }
    let dd_idx = body.find("var dd=").or_else(|| body.find("var dd ="))?;
    let after = &body[dd_idx + "var dd".len()..];
    let after = after.trim_start_matches(|c: char| c == '=' || c.is_whitespace());
    if !after.starts_with('{') {
        return None;
    }
    // Find the matching close brace. The DataDome interstitial uses no
    // nested braces in the dd literal (verified across all four target
    // sites in research §2), so a single-pass scan suffices.
    let mut depth = 0;
    let mut end = 0;
    for (i, c) in after.char_indices() {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                end = i + 1;
                break;
            }
        }
    }
    if end == 0 {
        return None;
    }
    let object_literal = &after[..end];

    Some(DdInterstitial {
        rt: extract_str(object_literal, "rt").unwrap_or_default(),
        cid: extract_str(object_literal, "cid").unwrap_or_default(),
        hsh: extract_str(object_literal, "hsh").unwrap_or_default(),
        b: extract_num(object_literal, "b").unwrap_or(0),
        s: extract_num(object_literal, "s").unwrap_or(0),
        e: extract_str(object_literal, "e").unwrap_or_default(),
        qp: extract_str(object_literal, "qp").unwrap_or_default(),
        host: extract_str(object_literal, "host").unwrap_or_default(),
        cookie: extract_str(object_literal, "cookie").unwrap_or_default(),
    })
}

/// Extract a single-quoted string field from a JS-object-literal.
/// Returns `None` if the key isn't present or its value isn't a
/// string literal in single quotes.
fn extract_str(obj: &str, key: &str) -> Option<String> {
    let patterns = [format!("'{key}':"), format!("'{key}' :")];
    for pat in &patterns {
        if let Some(idx) = obj.find(pat.as_str()) {
            let rest = &obj[idx + pat.len()..];
            let rest = rest.trim_start();
            if rest.starts_with('\'') {
                let after = &rest[1..];
                if let Some(close) = after.find('\'') {
                    return Some(after[..close].to_string());
                }
            }
        }
    }
    None
}

/// Extract a numeric field from a JS-object-literal.
fn extract_num(obj: &str, key: &str) -> Option<i64> {
    let patterns = [format!("'{key}':"), format!("'{key}' :")];
    for pat in &patterns {
        if let Some(idx) = obj.find(pat.as_str()) {
            let rest = &obj[idx + pat.len()..];
            let rest = rest.trim_start();
            let digits: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if !digits.is_empty() {
                return digits.parse().ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const REUTERS_BODY: &str = r#"<html lang="en"><head><title>reuters.com</title><style>#cmsg{animation: A 1.5s;}@keyframes A{0%{opacity:0;}99%{opacity:0;}100%{opacity:1;}}</style></head><body style="margin:0"><p id="cmsg">Please enable JS and disable any ad blocker</p><script data-cfasync="false">var dd={'rt':'i','cid':'AHrlqAAAAAMA2p7tCA1Tgs8A_wKXYw==','hsh':'2013457ADA70C67D6A4123E0A76873','b':1005349,'s':43909,'e':'3f4926171d07967ecd59ddd3407daa311822ccf5f9637f004a54310688a2bf13e3222c400280dede4b0a45a3915fd7c4','qp':'','host':'geo.captcha-delivery.com','cookie':'IWOF4uTClqNzi~mt5m9stJhpcuYNcLF1hkh6gi18ztqlEqwS4KNM6VPitAP36E7XSOlkIIrFYDBrFffFQlAcyNt8P3OhlKQcvUB7ljxlmYo5taThc5heveDZXyWe8m5P'}</script><script data-cfasync="false" src="https://ct.captcha-delivery.com/i.js"></script></body></html>"#;

    #[test]
    fn detects_real_reuters_interstitial() {
        let parsed = detect_datadome_interstitial(REUTERS_BODY).expect("parsed");
        assert_eq!(parsed.rt, "i");
        assert_eq!(parsed.cid, "AHrlqAAAAAMA2p7tCA1Tgs8A_wKXYw==");
        assert_eq!(parsed.hsh, "2013457ADA70C67D6A4123E0A76873");
        assert_eq!(parsed.b, 1_005_349);
        assert_eq!(parsed.s, 43_909);
        assert_eq!(parsed.host, "geo.captcha-delivery.com");
        assert!(!parsed.e.is_empty());
        assert!(parsed.cookie.starts_with("IWOF4uTC"));
    }

    #[test]
    fn ignores_large_body_with_captcha_delivery_substring() {
        // A rendered page that legitimately references captcha-delivery.com
        // in a 50 KB body must NOT be classified as an interstitial.
        let mut body = String::from(REUTERS_BODY);
        body.push_str(&"<div>filler</div>".repeat(2000));
        assert!(detect_datadome_interstitial(&body).is_none());
    }

    #[test]
    fn ignores_body_without_dd_literal() {
        let body = r#"<html><body><script>fetch('https://captcha-delivery.com/x')</script></body></html>"#;
        assert!(detect_datadome_interstitial(body).is_none());
    }

    #[test]
    fn ignores_body_without_captcha_delivery() {
        let body = r#"<html><body><script>var dd={'rt':'i','cid':'x'}</script></body></html>"#;
        assert!(detect_datadome_interstitial(body).is_none());
    }
}
