//! ES-module loader for document scripts (P2 / thin-render fix).
//!
//! Without it, `<script type="module">` entries — how every modern Vite/React/
//! Vue SPA ships its app — throw `SyntaxError: Cannot use import statement
//! outside a module` under classic `v8::Script::compile`, so the whole bundle is
//! dropped and the engine serves only the server shell (the 1.8-13 KB "thin
//! render" that loses douyin/duolingo/adidas/ozon/wildberries to camoufox v150).
//!
//! This loader resolves relative specifiers against the referrer/document URL
//! and fetches the import graph **on demand** through the same shared HTTP
//! session (cookies + stealth profile) the navigation uses, so a module's
//! `import "./chunk-[hash].js"` is fetched and evaluated. `ModuleSourceFuture`
//! has no `Send` bound (deno_core drives it on the per-thread LocalSet), so the
//! Rc-based `HttpClient` can be used directly.

use deno_core::error::AnyError;
use deno_core::{
    resolve_import, ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode,
    ModuleSpecifier, ModuleType, RequestedModuleType, ResolutionKind,
};
use net::HttpClient;
use stealth::StealthProfile;

/// Minimal percent-decoder for non-base64 `data:` URL JS payloads.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Fetches ES-module sources through browser_oxide's shared HTTP session.
pub struct BrowserModuleLoader {
    profile: StealthProfile,
    /// Cached client on the shared session jar (cookie-consistent with the
    /// page nav). `None` only if the connector failed to build.
    client: Option<HttpClient>,
}

impl BrowserModuleLoader {
    pub fn new(profile: StealthProfile) -> Self {
        let client = HttpClient::shared(&profile).ok();
        Self { profile, client }
    }
}

impl ModuleLoader for BrowserModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, AnyError> {
        // Spec-compliant relative-specifier resolution against the referrer.
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let spec = module_specifier.clone();
        let url = module_specifier.to_string();
        let profile = self.profile.clone();
        let client = self.client.clone();
        let referer = maybe_referrer
            .map(|r| r.to_string())
            .unwrap_or_else(|| url.clone());

        // data: modules. deno_core 0.311 routes `import('data:…')` THROUGH the
        // loader (it is NOT inlined by V8 for dynamic import), so we MUST resolve
        // it. Rejecting it (the previous behaviour) left an UNHANDLED promise
        // rejection that aborted the event-loop drain — duolingo's React app runs
        // a native-dynamic-import capability probe `import('data:text/javascript;
        // base64,Cg==')`, and that abort killed React's MessageChannel commit so
        // `#root` stayed an empty shell. Resolve data: like Chrome.
        if let Some(rest) = url.strip_prefix("data:") {
            use base64::Engine as _;
            let (meta, payload) = rest.split_once(',').unwrap_or(("", rest));
            let code = if meta.contains(";base64") {
                base64::engine::general_purpose::STANDARD
                    .decode(payload.trim())
                    .ok()
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default()
            } else {
                // Percent-decoded text payload.
                percent_decode(payload)
            };
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(code.into()),
                &spec,
                None,
            )));
        }

        // Only http(s) modules are network-fetchable.
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            // Unknown scheme (blob:, about:, …): return an EMPTY module rather
            // than an error, so an unhandled rejection can't abort the drain.
            return ModuleLoadResponse::Sync(Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(String::new().into()),
                &spec,
                None,
            )));
        }

        let fut = async move {
            let client = client
                .ok_or_else(|| AnyError::msg("module loader: shared HTTP client unavailable"))?;
            let mut hdrs = net::headers::nav_headers_for_url(&profile, &referer, false);
            hdrs.push(("referer".to_string(), referer));
            hdrs.push(("accept".to_string(), "*/*".to_string()));
            // ESM fetches: Chrome emits dest=script, mode=cors.
            hdrs.push(("sec-fetch-dest".to_string(), "script".to_string()));
            hdrs.push(("sec-fetch-mode".to_string(), "cors".to_string()));
            let resp = client
                .get_follow_with_headers(&url, &hdrs, 5)
                .await
                .map_err(|e| AnyError::msg(format!("module fetch {url}: {e}")))?;
            if !resp.ok() {
                return Err(AnyError::msg(format!(
                    "module fetch {url} -> status {}",
                    resp.status
                )));
            }
            let code = resp.text();
            Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(code.into()),
                &spec,
                None,
            ))
        };
        ModuleLoadResponse::Async(Box::pin(fut))
    }
}
