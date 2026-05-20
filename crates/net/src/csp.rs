//! Content Security Policy (CSP3) — parser, matcher, and per-fetch
//! enforcement check used by both `crates/net::HttpClient` (top-level
//! navigations + sub-resource fetches) and the JS ops in
//! `crates/js_runtime::extensions::fetch_ext`.
//!
//! Implements the subset of [CSP3](https://www.w3.org/TR/CSP3/) needed
//! to match real Chrome's enforcement on the network path:
//!
//! - directives: `script-src`, `script-src-elem`, `connect-src`,
//!   `img-src`, `frame-src`, `child-src`, `font-src`, `media-src`,
//!   `style-src`, `default-src`
//! - source-list keywords: `'self'`, `'none'`, `'unsafe-inline'`,
//!   `'unsafe-eval'`, `'unsafe-hashes'`, `'strict-dynamic'`,
//!   `'report-sample'`, scheme-only sources, host sources with
//!   `*` wildcards, port wildcards, nonce sources, hash sources
//! - default-src fallback for missing fetch directives
//! - `'strict-dynamic'` semantics on script-src: when present, host
//!   allowlist is ignored; only nonce/hash-trusted scripts and their
//!   non-parser-inserted descendants are allowed
//!
//! Out of scope (deferred): `frame-ancestors`, `form-action`,
//! `base-uri`, `report-uri`/`report-to`, `require-trusted-types-for`,
//! upgrade-insecure-requests.

use std::collections::HashMap;

use url::Url;

// ---------------------------------------------------------------------
// Directive enum — every fetch directive we enforce, plus DefaultSrc as
// the fallback target. Strings stored verbatim from the policy so we
// can echo them back in `securitypolicyviolation` events.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Directive {
    DefaultSrc,
    ScriptSrc,
    ScriptSrcElem,
    ScriptSrcAttr,
    StyleSrc,
    StyleSrcElem,
    StyleSrcAttr,
    ImgSrc,
    ConnectSrc,
    FrameSrc,
    ChildSrc,
    FontSrc,
    MediaSrc,
    ObjectSrc,
    WorkerSrc,
    ManifestSrc,
    PrefetchSrc,
}

impl Directive {
    pub fn from_token(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "default-src" => Directive::DefaultSrc,
            "script-src" => Directive::ScriptSrc,
            "script-src-elem" => Directive::ScriptSrcElem,
            "script-src-attr" => Directive::ScriptSrcAttr,
            "style-src" => Directive::StyleSrc,
            "style-src-elem" => Directive::StyleSrcElem,
            "style-src-attr" => Directive::StyleSrcAttr,
            "img-src" => Directive::ImgSrc,
            "connect-src" => Directive::ConnectSrc,
            "frame-src" => Directive::FrameSrc,
            "child-src" => Directive::ChildSrc,
            "font-src" => Directive::FontSrc,
            "media-src" => Directive::MediaSrc,
            "object-src" => Directive::ObjectSrc,
            "worker-src" => Directive::WorkerSrc,
            "manifest-src" => Directive::ManifestSrc,
            "prefetch-src" => Directive::PrefetchSrc,
            _ => return None,
        })
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Directive::DefaultSrc => "default-src",
            Directive::ScriptSrc => "script-src",
            Directive::ScriptSrcElem => "script-src-elem",
            Directive::ScriptSrcAttr => "script-src-attr",
            Directive::StyleSrc => "style-src",
            Directive::StyleSrcElem => "style-src-elem",
            Directive::StyleSrcAttr => "style-src-attr",
            Directive::ImgSrc => "img-src",
            Directive::ConnectSrc => "connect-src",
            Directive::FrameSrc => "frame-src",
            Directive::ChildSrc => "child-src",
            Directive::FontSrc => "font-src",
            Directive::MediaSrc => "media-src",
            Directive::ObjectSrc => "object-src",
            Directive::WorkerSrc => "worker-src",
            Directive::ManifestSrc => "manifest-src",
            Directive::PrefetchSrc => "prefetch-src",
        }
    }

    /// CSP3 §6.6.1.1 — fallback chain for fetch directives. When a
    /// specific directive is missing, the matcher tries these in order.
    /// Scripts: `script-src-elem` → `script-src` → `default-src`.
    /// Styles: `style-src-elem` → `style-src` → `default-src`.
    /// Iframes: `frame-src` → `child-src` → `default-src`.
    /// Workers: `worker-src` → `child-src` → `script-src` → `default-src`.
    /// Most other fetch directives fall back directly to `default-src`.
    pub fn fallback_chain(&self) -> &'static [Directive] {
        use Directive::*;
        match self {
            ScriptSrcElem => &[ScriptSrcElem, ScriptSrc, DefaultSrc],
            ScriptSrcAttr => &[ScriptSrcAttr, ScriptSrc, DefaultSrc],
            ScriptSrc => &[ScriptSrc, DefaultSrc],
            StyleSrcElem => &[StyleSrcElem, StyleSrc, DefaultSrc],
            StyleSrcAttr => &[StyleSrcAttr, StyleSrc, DefaultSrc],
            StyleSrc => &[StyleSrc, DefaultSrc],
            FrameSrc => &[FrameSrc, ChildSrc, DefaultSrc],
            ChildSrc => &[ChildSrc, DefaultSrc],
            WorkerSrc => &[WorkerSrc, ChildSrc, ScriptSrc, DefaultSrc],
            ImgSrc => &[ImgSrc, DefaultSrc],
            ConnectSrc => &[ConnectSrc, DefaultSrc],
            FontSrc => &[FontSrc, DefaultSrc],
            MediaSrc => &[MediaSrc, DefaultSrc],
            ObjectSrc => &[ObjectSrc, DefaultSrc],
            ManifestSrc => &[ManifestSrc, DefaultSrc],
            PrefetchSrc => &[PrefetchSrc, DefaultSrc],
            DefaultSrc => &[DefaultSrc],
        }
    }
}

// ---------------------------------------------------------------------
// Source — one entry inside a directive's source list.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashAlgo {
    Sha256,
    Sha384,
    Sha512,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// `*` — match any URL with a network scheme (http/https/ws/wss/ftp).
    All,
    /// `'none'` — match nothing. Per CSP3 a `'none'` source list means
    /// no source matches; if any other source is present in the same
    /// list, `'none'` is ignored.
    None_,
    /// `'self'` — match the page's origin.
    Self_,
    /// `'unsafe-inline'` — allow inline scripts/styles. We don't run
    /// inline scripts through CSP today (everything we run is from the
    /// parsed HTML and treated as inline-like) but we recognize the
    /// keyword for spec compliance.
    UnsafeInline,
    /// `'unsafe-eval'` — allow `eval()` / `Function()` constructors.
    UnsafeEval,
    /// `'unsafe-hashes'` — allow hash-matched event handlers and
    /// `javascript:` URLs.
    UnsafeHashes,
    /// `'strict-dynamic'` — when present in script-src, ignore the host
    /// allowlist and trust nonce/hash-matched scripts plus their non-
    /// parser-inserted descendants. **Critical for Walmart parity**:
    /// Walmart's CSP includes `'strict-dynamic'` so Akamai's parser-
    /// injected `/akam/13/...` script is blocked even though same-origin.
    StrictDynamic,
    /// `'report-sample'` — opt-in to including a snippet of the
    /// blocked content in violation reports. Doesn't affect blocking.
    ReportSample,
    /// Scheme-only source like `https:` or `data:`.
    Scheme(String),
    /// Host source with optional scheme/port/wildcards.
    Host(HostSource),
    /// `'nonce-XXXX'` — base64-ish token. Matches scripts/styles whose
    /// `nonce` attribute equals the token (case-sensitive).
    Nonce(String),
    /// `'sha256-XXXX'` / `'sha384-...'` / `'sha512-...'` — base64 hash.
    Hash(HashAlgo, String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSource {
    pub scheme: Option<String>, // None ⇒ match any of http/https
    pub host: HostPattern,
    pub port: Option<PortPattern>,
    pub path: Option<String>, // path-prefix, optional
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    /// `*.example.com` — match any subdomain (NOT example.com itself).
    Wildcard(String), // suffix without leading dot, e.g. "example.com"
    Exact(String),    // ASCII-lowered host
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortPattern {
    Wildcard, // `:*`
    Exact(u16),
}

// ---------------------------------------------------------------------
// Policy — a parsed Content-Security-Policy. May represent the merge of
// multiple policies (one per response header + one per <meta> tag).
// CSP3 §3.2: when multiple policies apply, ALL must allow a request for
// it to be allowed. We model this by keeping each parsed Policy
// separate inside a `PolicySet`.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Policy {
    /// Directive → list of sources for that directive.
    pub directives: HashMap<Directive, Vec<Source>>,
    /// True if this came from `Content-Security-Policy-Report-Only` —
    /// then violations are reported but NOT enforced.
    pub report_only: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PolicySet {
    pub policies: Vec<Policy>,
}

impl PolicySet {
    pub fn is_empty(&self) -> bool {
        self.policies.iter().all(|p| p.directives.is_empty())
    }

    /// Add policies parsed from one or more response headers. Headers
    /// can carry multiple comma-separated policies; we split on `,`
    /// at the top level (not inside source-list tokens).
    pub fn push_header(&mut self, value: &str, report_only: bool) {
        for piece in split_top_level(value, ',') {
            let policy = Policy::parse_serialized(piece, report_only);
            if !policy.directives.is_empty() {
                self.policies.push(policy);
            }
        }
    }

    /// Add a policy parsed from a `<meta http-equiv="Content-Security-Policy">`
    /// content attribute. Per CSP3 §3.4.1, meta-CSP must always be
    /// enforced (cannot be report-only).
    pub fn push_meta(&mut self, content: &str) {
        for piece in split_top_level(content, ',') {
            let policy = Policy::parse_serialized(piece, false);
            if !policy.directives.is_empty() {
                self.policies.push(policy);
            }
        }
    }
}

// Split `s` on `delim` ignoring delimiters inside source-list values
// (CSP source lists don't actually contain commas, but be safe).
fn split_top_level(s: &str, delim: char) -> impl Iterator<Item = &str> {
    s.split(delim).map(str::trim).filter(|p| !p.is_empty())
}

impl Policy {
    /// Parse a single serialized policy (one directive list separated
    /// by `;`). e.g. `script-src 'self' 'strict-dynamic'; connect-src *`.
    pub fn parse_serialized(s: &str, report_only: bool) -> Policy {
        let mut policy = Policy {
            directives: HashMap::new(),
            report_only,
        };
        for raw in s.split(';') {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let mut tokens = raw.split_ascii_whitespace();
            let dir_token = match tokens.next() {
                Some(t) => t,
                None => continue,
            };
            let Some(directive) = Directive::from_token(dir_token) else {
                continue;
            };
            let mut sources = Vec::new();
            for tok in tokens {
                if let Some(src) = Source::parse(tok) {
                    sources.push(src);
                }
            }
            // CSP3 §6.6.2.1: a directive defined with empty source list
            // (eg `script-src ;`) means "no source matches" — equivalent
            // to `'none'`. Preserve as empty list; matcher treats empty
            // as block.
            policy.directives.entry(directive).or_default().extend(sources);
        }
        policy
    }

    pub fn parse_header(s: &str) -> PolicySet {
        let mut set = PolicySet::default();
        set.push_header(s, false);
        set
    }

    pub fn parse_meta_content(s: &str) -> PolicySet {
        let mut set = PolicySet::default();
        set.push_meta(s);
        set
    }
}

impl Source {
    fn parse(token: &str) -> Option<Source> {
        // Keyword sources (case-insensitive per CSP3 §3.1).
        let lc = token.to_ascii_lowercase();
        match lc.as_str() {
            "*" => return Some(Source::All),
            "'none'" => return Some(Source::None_),
            "'self'" => return Some(Source::Self_),
            "'unsafe-inline'" => return Some(Source::UnsafeInline),
            "'unsafe-eval'" => return Some(Source::UnsafeEval),
            "'unsafe-hashes'" => return Some(Source::UnsafeHashes),
            "'strict-dynamic'" => return Some(Source::StrictDynamic),
            "'report-sample'" => return Some(Source::ReportSample),
            _ => {}
        }
        // Nonce: 'nonce-XXXX'  — preserve original case for the nonce
        // value (server-issued tokens are case-sensitive).
        if let Some(rest) = token.strip_prefix("'nonce-") {
            if let Some(value) = rest.strip_suffix('\'') {
                return Some(Source::Nonce(value.to_string()));
            }
        }
        // Hash: 'sha256-...' / 'sha384-...' / 'sha512-...'
        for (algo, prefix) in [
            (HashAlgo::Sha256, "'sha256-"),
            (HashAlgo::Sha384, "'sha384-"),
            (HashAlgo::Sha512, "'sha512-"),
        ] {
            if let Some(rest) = token.strip_prefix(prefix) {
                if let Some(value) = rest.strip_suffix('\'') {
                    return Some(Source::Hash(algo, value.to_string()));
                }
            }
        }
        // Scheme-only: ends with ':' and contains no '/'.
        if let Some(scheme) = token.strip_suffix(':') {
            if !scheme.contains('/') && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
                return Some(Source::Scheme(scheme.to_ascii_lowercase()));
            }
        }
        // Host source: must NOT be wrapped in single quotes. Format:
        //   [scheme://]host[:port][/path]
        // host may be `*` (any), `*.example.com` (subdomain wildcard),
        // or an exact host. Port may be `*` or a number.
        if !token.starts_with('\'') {
            return parse_host_source(token).map(Source::Host);
        }
        None
    }
}

fn parse_host_source(token: &str) -> Option<HostSource> {
    let mut rest = token;
    // scheme://
    let mut scheme = None;
    if let Some(idx) = rest.find("://") {
        let s = rest[..idx].to_ascii_lowercase();
        if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') && !s.is_empty() {
            scheme = Some(s);
            rest = &rest[idx + 3..];
        }
    }
    // path
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], Some(rest[idx..].to_string())),
        None => (rest, None),
    };
    if host_port.is_empty() {
        return None;
    }
    // port — split off after the LAST colon, but only if the rhs is
    // numeric or `*`. (IPv6 not currently supported; CSP source list
    // syntax accepts bracketed IPv6 but real-world policies rarely use
    // it on the hot path. Add later if needed.)
    let (host_part, port) = match host_port.rfind(':') {
        Some(idx) => {
            let port_str = &host_port[idx + 1..];
            if port_str == "*" {
                (&host_port[..idx], Some(PortPattern::Wildcard))
            } else if let Ok(n) = port_str.parse::<u16>() {
                (&host_port[..idx], Some(PortPattern::Exact(n)))
            } else {
                (host_port, None)
            }
        }
        None => (host_port, None),
    };
    // host
    let host = if host_part == "*" {
        // bare "*" host is allowed only when no scheme/port/path; still
        // legal (matches anything network).
        HostPattern::Exact("*".to_string())
    } else if let Some(suffix) = host_part.strip_prefix("*.") {
        HostPattern::Wildcard(suffix.to_ascii_lowercase())
    } else {
        if host_part.is_empty()
            || host_part.contains('*')
            || !host_part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        {
            return None;
        }
        HostPattern::Exact(host_part.to_ascii_lowercase())
    };
    Some(HostSource { scheme, host, port, path })
}

// ---------------------------------------------------------------------
// CheckCtx — what the caller knows when asking "may I fetch this?".
// Consumed by Policy::allows() in the next-day patch. Defining the type
// here so callers can prepare it once D1 lands.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CheckCtx<'a> {
    pub directive: Directive,
    pub url: &'a Url,
    pub page_origin: &'a Url,
    /// Script `nonce` attribute, if any.
    pub nonce: Option<&'a str>,
    /// Was this fetch initiated by HTML parsing, or by JS-driven DOM
    /// manipulation? Matters for `'strict-dynamic'`: parser-inserted
    /// scripts are NEVER trusted under strict-dynamic, no matter what
    /// the host allowlist says.
    pub parser_inserted: bool,
}

// ---------------------------------------------------------------------
// Matcher — `Policy::allows(ctx) -> AllowDecision`. Implements CSP3
// §6.6.2 fetch directive matching including the `'strict-dynamic'`
// twist that makes Walmart's CSP block parser-inserted Akamai scripts.
// ---------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AllowDecision {
    pub allowed: bool,
    /// The directive that ultimately governed the decision (after
    /// fallback resolution). Echoed in `securitypolicyviolation` events.
    pub matched_directive: Directive,
    /// True if the policy was report-only — caller should still allow
    /// the fetch but emit a violation report. (Unused on the enforce
    /// path but plumbed through for future parity with Chrome's
    /// `Content-Security-Policy-Report-Only` header.)
    pub report_only: bool,
}

impl AllowDecision {
    pub fn allow_no_policy() -> Self {
        Self {
            allowed: true,
            matched_directive: Directive::DefaultSrc,
            report_only: false,
        }
    }
}

impl PolicySet {
    /// Returns ALLOW only if every enforced policy in the set allows.
    /// Per CSP3 §3.2: when multiple policies apply, all must permit.
    /// Report-only policies never block — they only contribute to
    /// reports — so a report-only block is recorded but doesn't flip
    /// the overall decision.
    pub fn allows(&self, ctx: &CheckCtx<'_>) -> AllowDecision {
        if self.policies.is_empty() {
            return AllowDecision::allow_no_policy();
        }
        for policy in &self.policies {
            let decision = policy.allows(ctx);
            if !decision.allowed && !policy.report_only {
                return decision;
            }
        }
        AllowDecision::allow_no_policy()
    }
}

impl Policy {
    pub fn allows(&self, ctx: &CheckCtx<'_>) -> AllowDecision {
        // Walk fallback chain: first directive present in this policy
        // wins. If none present, the policy doesn't constrain this
        // directive type — allow.
        for &candidate in ctx.directive.fallback_chain() {
            if let Some(sources) = self.directives.get(&candidate) {
                let allowed = match_sources(sources, ctx);
                return AllowDecision {
                    allowed,
                    matched_directive: candidate,
                    report_only: self.report_only,
                };
            }
        }
        AllowDecision::allow_no_policy()
    }
}

/// Match a CheckCtx against a single directive's source list.
///
/// Empty list ⇒ block (CSP3: directive with no values acts like `'none'`).
/// `'strict-dynamic'` (script-src family only) overrides host/`'self'`/`*`
/// matching: only nonce or hash sources can authorize, AND
/// parser-inserted scripts are never authorized by `'strict-dynamic'`
/// alone — they must carry a matching nonce.
fn match_sources(sources: &[Source], ctx: &CheckCtx<'_>) -> bool {
    if sources.is_empty() {
        return false;
    }
    if sources.iter().any(|s| matches!(s, Source::None_)) && sources.len() == 1 {
        // CSP3: `'none'` as the sole entry blocks unconditionally.
        return false;
    }

    let strict_dynamic = is_script_directive(ctx.directive)
        && sources.iter().any(|s| matches!(s, Source::StrictDynamic));

    for src in sources {
        match src {
            // Keywords with no fetch effect.
            Source::None_
            | Source::UnsafeInline
            | Source::UnsafeEval
            | Source::UnsafeHashes
            | Source::ReportSample
            | Source::StrictDynamic => continue,

            // Under strict-dynamic, host/'self'/* are all ignored.
            Source::All if !strict_dynamic => {
                if is_network_scheme(ctx.url.scheme()) {
                    return true;
                }
            }
            Source::All => continue,

            Source::Self_ if !strict_dynamic => {
                if is_same_origin(ctx.url, ctx.page_origin) {
                    return true;
                }
            }
            Source::Self_ => continue,

            Source::Scheme(s) if !strict_dynamic => {
                if ctx.url.scheme().eq_ignore_ascii_case(s) {
                    return true;
                }
            }
            Source::Scheme(_) => continue,

            Source::Host(h) if !strict_dynamic => {
                if host_source_matches(h, ctx.url) {
                    return true;
                }
            }
            Source::Host(_) => continue,

            Source::Nonce(token) => {
                // Nonce match is case-sensitive. Under strict-dynamic
                // a nonce on a parser-inserted script IS still trusted
                // (this is how Walmart loads its own scripts: nonce
                // attribute on every <script> in the head).
                if let Some(supplied) = ctx.nonce {
                    if supplied == token {
                        return true;
                    }
                }
            }

            Source::Hash(_, _) => {
                // Hash sources match inline content (script body or
                // style body). For fetch enforcement we have nothing
                // to hash — the body hasn't been fetched yet. CSP3
                // permits hash sources to authorize external scripts
                // when `'unsafe-hashes'` is present, but only inline
                // event handlers and javascript: URLs use that path.
                // Skip — hash sources never match a fetch URL.
                continue;
            }
        }
    }
    false
}

fn is_script_directive(d: Directive) -> bool {
    matches!(
        d,
        Directive::ScriptSrc | Directive::ScriptSrcElem | Directive::ScriptSrcAttr
    )
}

fn is_network_scheme(scheme: &str) -> bool {
    matches!(
        scheme,
        "http" | "https" | "ws" | "wss" | "ftp" | "ftps"
    )
}

fn is_same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme() && a.host_str() == b.host_str() && a.port_or_known_default() == b.port_or_known_default()
}

fn host_source_matches(src: &HostSource, url: &Url) -> bool {
    // Scheme.
    if let Some(want) = &src.scheme {
        if !url.scheme().eq_ignore_ascii_case(want) {
            return false;
        }
    } else {
        // No scheme in source ⇒ match any network scheme.
        if !is_network_scheme(url.scheme()) {
            return false;
        }
    }
    // Host.
    let url_host = match url.host_str() {
        Some(h) => h.to_ascii_lowercase(),
        None => return false,
    };
    let host_ok = match &src.host {
        HostPattern::Exact(want) => want == "*" || want == &url_host,
        HostPattern::Wildcard(suffix) => {
            // `*.example.com` matches `foo.example.com` but NOT
            // `example.com` itself (CSP3 §6.6.2.4).
            url_host.ends_with(suffix)
                && url_host.len() > suffix.len()
                && url_host
                    .chars()
                    .nth(url_host.len() - suffix.len() - 1)
                    == Some('.')
        }
    };
    if !host_ok {
        return false;
    }
    // Port. CSP3 default-port handling: if source omits port, only
    // default ports match.
    let url_port = url.port_or_known_default();
    if let Some(p) = &src.port {
        match p {
            PortPattern::Wildcard => {} // any port ok
            PortPattern::Exact(n) => {
                if url_port != Some(*n) {
                    return false;
                }
            }
        }
    } else {
        // Source has no port → only matches URL on the default port
        // for the URL's scheme.
        let default_port = match url.scheme() {
            "http" | "ws" | "ftp" => Some(80),
            "https" | "wss" | "ftps" => Some(443),
            _ => None,
        };
        if url_port != default_port {
            return false;
        }
    }
    // Path-prefix is only enforced on top-level navigation per CSP3;
    // for sub-resource fetches the path is ignored. We're enforcing on
    // the network path, so skip path matching.
    true
}

// ---------------------------------------------------------------------
// Tests — Walmart's actual CSP plus a representative spec fixture.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Walmart's `<meta http-equiv="Content-Security-Policy">` content
    /// captured 2026-04-29. Trimmed to representative directives; the
    /// real string is much longer with ~200 host sources.
    const WALMART_CSP: &str = "child-src 'self' blob:; \
        connect-src 'self' *.akamaihd.net *.perimeterx.net; \
        script-src 'self' 'strict-dynamic' 'nonce-MRjHHgrLk9lNoNBv' *.walmartimages.com; \
        style-src 'self' 'unsafe-inline' *.walmartimages.com; \
        img-src 'self' data: *.walmartimages.com *.scene7.com; \
        frame-src 'self' *.youtube.com";

    #[test]
    fn parses_walmart_directives() {
        let set = Policy::parse_meta_content(WALMART_CSP);
        assert_eq!(set.policies.len(), 1);
        let p = &set.policies[0];
        assert!(!p.report_only);
        assert!(p.directives.contains_key(&Directive::ScriptSrc));
        assert!(p.directives.contains_key(&Directive::ConnectSrc));
        assert!(p.directives.contains_key(&Directive::FrameSrc));
    }

    #[test]
    fn parses_strict_dynamic_and_nonce() {
        let set = Policy::parse_meta_content(WALMART_CSP);
        let script_src = &set.policies[0].directives[&Directive::ScriptSrc];
        assert!(
            script_src.iter().any(|s| matches!(s, Source::StrictDynamic)),
            "must parse 'strict-dynamic' keyword"
        );
        assert!(
            script_src
                .iter()
                .any(|s| matches!(s, Source::Nonce(n) if n == "MRjHHgrLk9lNoNBv")),
            "must parse 'nonce-...'"
        );
        assert!(
            script_src.iter().any(|s| matches!(s, Source::Self_)),
            "must parse 'self'"
        );
    }

    #[test]
    fn parses_host_source_with_subdomain_wildcard() {
        let set = Policy::parse_meta_content("connect-src *.example.com:8443");
        let cs = &set.policies[0].directives[&Directive::ConnectSrc];
        assert_eq!(cs.len(), 1);
        let Source::Host(h) = &cs[0] else { panic!("expected host source") };
        assert_eq!(h.host, HostPattern::Wildcard("example.com".to_string()));
        assert_eq!(h.port, Some(PortPattern::Exact(8443)));
    }

    #[test]
    fn parses_scheme_only_source() {
        let set = Policy::parse_meta_content("img-src data: blob: https:");
        let img = &set.policies[0].directives[&Directive::ImgSrc];
        assert_eq!(img.len(), 3);
        assert!(img.iter().any(|s| matches!(s, Source::Scheme(x) if x == "data")));
        assert!(img.iter().any(|s| matches!(s, Source::Scheme(x) if x == "blob")));
        assert!(img.iter().any(|s| matches!(s, Source::Scheme(x) if x == "https")));
    }

    #[test]
    fn parses_hash_sources() {
        let set = Policy::parse_meta_content(
            "script-src 'sha256-abc123==' 'sha384-XYZ' 'sha512-q+w'",
        );
        let ss = &set.policies[0].directives[&Directive::ScriptSrc];
        assert_eq!(ss.len(), 3);
        assert!(matches!(&ss[0], Source::Hash(HashAlgo::Sha256, h) if h == "abc123=="));
        assert!(matches!(&ss[1], Source::Hash(HashAlgo::Sha384, h) if h == "XYZ"));
        assert!(matches!(&ss[2], Source::Hash(HashAlgo::Sha512, h) if h == "q+w"));
    }

    #[test]
    fn parses_none_keyword() {
        let set = Policy::parse_meta_content("object-src 'none'");
        let os = &set.policies[0].directives[&Directive::ObjectSrc];
        assert_eq!(os.len(), 1);
        assert!(matches!(&os[0], Source::None_));
    }

    #[test]
    fn parses_multiple_policies_from_one_header() {
        let mut set = PolicySet::default();
        set.push_header("script-src 'self', script-src https:", false);
        assert_eq!(set.policies.len(), 2);
    }

    #[test]
    fn report_only_flag_propagates() {
        let mut set = PolicySet::default();
        set.push_header("script-src 'self'", true);
        assert!(set.policies[0].report_only);
    }

    #[test]
    fn unknown_directive_is_dropped_silently() {
        let set = Policy::parse_meta_content("script-src 'self'; bogus-thing 'self'");
        assert_eq!(set.policies[0].directives.len(), 1);
    }

    #[test]
    fn fallback_chain_for_script_src_elem_includes_default() {
        let chain = Directive::ScriptSrcElem.fallback_chain();
        assert_eq!(chain, &[Directive::ScriptSrcElem, Directive::ScriptSrc, Directive::DefaultSrc]);
    }

    #[test]
    fn fallback_chain_for_frame_src_includes_child_then_default() {
        let chain = Directive::FrameSrc.fallback_chain();
        assert_eq!(chain, &[Directive::FrameSrc, Directive::ChildSrc, Directive::DefaultSrc]);
    }

    #[test]
    fn parses_bare_star_host() {
        let set = Policy::parse_meta_content("img-src *");
        let img = &set.policies[0].directives[&Directive::ImgSrc];
        assert!(matches!(&img[0], Source::All));
    }

    #[test]
    fn parses_directive_with_no_sources_as_block_all() {
        // `script-src` with no tokens after — empty source list.
        let set = Policy::parse_meta_content("script-src");
        let ss = &set.policies[0].directives[&Directive::ScriptSrc];
        assert_eq!(ss.len(), 0);
    }

    // ----- matcher tests (Day 2) -----

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn ctx<'a>(
        directive: Directive,
        u: &'a Url,
        origin: &'a Url,
        nonce: Option<&'a str>,
        parser_inserted: bool,
    ) -> CheckCtx<'a> {
        CheckCtx { directive, url: u, page_origin: origin, nonce, parser_inserted }
    }

    #[test]
    fn empty_policy_set_allows_everything() {
        let set = PolicySet::default();
        let u = url("https://akamai.com/sensor.js");
        let origin = url("https://www.walmart.com/");
        let d = set.allows(&ctx(Directive::ScriptSrcElem, &u, &origin, None, true));
        assert!(d.allowed);
    }

    #[test]
    fn self_matches_same_origin() {
        let set = Policy::parse_meta_content("script-src 'self'");
        let origin = url("https://example.com/");
        let same = url("https://example.com/app.js");
        let other = url("https://other.com/x.js");
        assert!(set.allows(&ctx(Directive::ScriptSrcElem, &same, &origin, None, true)).allowed);
        assert!(!set.allows(&ctx(Directive::ScriptSrcElem, &other, &origin, None, true)).allowed);
    }

    #[test]
    fn host_wildcard_matches_subdomain_only() {
        let set = Policy::parse_meta_content("img-src *.example.com");
        let origin = url("https://example.com/");
        let sub = url("https://images.example.com/a.png");
        let bare = url("https://example.com/a.png");
        assert!(set.allows(&ctx(Directive::ImgSrc, &sub, &origin, None, false)).allowed);
        // Bare host does NOT match `*.example.com` per CSP3.
        assert!(!set.allows(&ctx(Directive::ImgSrc, &bare, &origin, None, false)).allowed);
    }

    #[test]
    fn scheme_only_source_matches_any_host_on_that_scheme() {
        let set = Policy::parse_meta_content("img-src data: https:");
        let origin = url("https://example.com/");
        let data = url("data:image/png;base64,iVBORw0K");
        let any_https = url("https://random.cdn.net/x.png");
        let http = url("http://random.cdn.net/x.png");
        assert!(set.allows(&ctx(Directive::ImgSrc, &data, &origin, None, false)).allowed);
        assert!(set.allows(&ctx(Directive::ImgSrc, &any_https, &origin, None, false)).allowed);
        assert!(!set.allows(&ctx(Directive::ImgSrc, &http, &origin, None, false)).allowed);
    }

    #[test]
    fn none_blocks_everything() {
        let set = Policy::parse_meta_content("object-src 'none'");
        let origin = url("https://example.com/");
        let any = url("https://example.com/x.swf");
        assert!(!set.allows(&ctx(Directive::ObjectSrc, &any, &origin, None, false)).allowed);
    }

    #[test]
    fn fallback_chain_uses_default_src_when_specific_missing() {
        let set = Policy::parse_meta_content("default-src 'self'");
        let origin = url("https://example.com/");
        let self_url = url("https://example.com/x.png");
        let other = url("https://other.com/x.png");
        assert!(set.allows(&ctx(Directive::ImgSrc, &self_url, &origin, None, false)).allowed);
        assert!(!set.allows(&ctx(Directive::ImgSrc, &other, &origin, None, false)).allowed);
    }

    #[test]
    fn nonce_authorizes_under_normal_policy() {
        let set = Policy::parse_meta_content("script-src 'nonce-abc123'");
        let origin = url("https://example.com/");
        let any = url("https://cdn.elsewhere.com/app.js");
        assert!(
            set.allows(&ctx(Directive::ScriptSrcElem, &any, &origin, Some("abc123"), true)).allowed
        );
        assert!(
            !set.allows(&ctx(Directive::ScriptSrcElem, &any, &origin, Some("WRONG"), true)).allowed
        );
        assert!(!set.allows(&ctx(Directive::ScriptSrcElem, &any, &origin, None, true)).allowed);
    }

    /// **The load-bearing case**: Walmart's CSP. Akamai's parser-injected
    /// `/akam/13/3e35295b` script is same-origin but has no matching
    /// nonce — `'strict-dynamic'` causes Chrome to ignore the host
    /// allowlist (including 'self'), so the script is blocked. We must
    /// match Chrome on this exact decision or we keep firing the
    /// parser-injected fetch real Chrome doesn't.
    #[test]
    fn walmart_strict_dynamic_blocks_parser_injected_akamai() {
        let set = Policy::parse_meta_content(WALMART_CSP);
        let origin = url("https://www.walmart.com/");
        let akamai = url("https://www.walmart.com/akam/13/3e35295b");

        // Parser-injected with no nonce: BLOCKED (the live Walmart bug we hit).
        let d = set.allows(&ctx(Directive::ScriptSrcElem, &akamai, &origin, None, true));
        assert!(!d.allowed, "Akamai parser-injected script must be blocked under strict-dynamic");
        assert_eq!(d.matched_directive, Directive::ScriptSrc);

        // Parser-injected WITH the page's nonce: allowed (this is how
        // walmart.com's own legitimate inline scripts get through).
        let d = set.allows(&ctx(
            Directive::ScriptSrcElem,
            &akamai,
            &origin,
            Some("MRjHHgrLk9lNoNBv"),
            true,
        ));
        assert!(d.allowed, "matching nonce overrides strict-dynamic block");
    }

    #[test]
    fn strict_dynamic_ignores_self_and_host_allowlist() {
        // Even though *.walmartimages.com is in the script-src allowlist,
        // strict-dynamic causes that allowlist to be IGNORED. The only
        // way through is matching nonce or hash.
        let set = Policy::parse_meta_content(WALMART_CSP);
        let origin = url("https://www.walmart.com/");
        let images = url("https://i5.walmartimages.com/foo.js");
        // Without nonce: blocked despite host being in allowlist.
        assert!(!set
            .allows(&ctx(Directive::ScriptSrcElem, &images, &origin, None, true))
            .allowed);
        // With nonce: allowed.
        assert!(set
            .allows(&ctx(
                Directive::ScriptSrcElem,
                &images,
                &origin,
                Some("MRjHHgrLk9lNoNBv"),
                true,
            ))
            .allowed);
    }

    #[test]
    fn strict_dynamic_does_not_apply_to_non_script_directives() {
        // `'strict-dynamic'` only changes script-src semantics. Other
        // directives in the same policy are unaffected — eg img-src
        // host allowlist still works normally.
        let set = Policy::parse_meta_content(WALMART_CSP);
        let origin = url("https://www.walmart.com/");
        let img = url("https://i5.walmartimages.com/foo.png");
        assert!(set.allows(&ctx(Directive::ImgSrc, &img, &origin, None, false)).allowed);
    }

    #[test]
    fn host_with_wildcard_port_matches_any_port() {
        let set = Policy::parse_meta_content("connect-src example.com:*");
        let origin = url("https://other.com/");
        let p443 = url("https://example.com/x");
        let p8443 = url("https://example.com:8443/x");
        assert!(set.allows(&ctx(Directive::ConnectSrc, &p443, &origin, None, false)).allowed);
        assert!(set.allows(&ctx(Directive::ConnectSrc, &p8443, &origin, None, false)).allowed);
    }

    #[test]
    fn host_without_port_matches_only_default_port() {
        let set = Policy::parse_meta_content("connect-src example.com");
        let origin = url("https://other.com/");
        let p443 = url("https://example.com/x"); // default https port
        let p8443 = url("https://example.com:8443/x"); // non-default → block
        assert!(set.allows(&ctx(Directive::ConnectSrc, &p443, &origin, None, false)).allowed);
        assert!(!set.allows(&ctx(Directive::ConnectSrc, &p8443, &origin, None, false)).allowed);
    }

    #[test]
    fn report_only_policy_never_blocks() {
        let mut set = PolicySet::default();
        set.push_header("script-src 'none'", true);
        let origin = url("https://example.com/");
        let any = url("https://example.com/x.js");
        // Even with 'none', report_only=true means the overall decision
        // is allow.
        assert!(set.allows(&ctx(Directive::ScriptSrcElem, &any, &origin, None, true)).allowed);
    }

    #[test]
    fn multiple_policies_intersect_most_restrictive() {
        let mut set = PolicySet::default();
        set.push_header("script-src 'self' https://cdn.com", false);
        set.push_header("script-src 'self'", false); // narrower
        let origin = url("https://example.com/");
        let cdn = url("https://cdn.com/x.js");
        let self_url = url("https://example.com/x.js");
        // First policy allows cdn, second doesn't → blocked.
        assert!(!set.allows(&ctx(Directive::ScriptSrcElem, &cdn, &origin, None, true)).allowed);
        // Both allow self → allowed.
        assert!(set.allows(&ctx(Directive::ScriptSrcElem, &self_url, &origin, None, true)).allowed);
    }
}
