# 08 — CSR / First-Render Blockers (per-site synthesis)

Date: 2026-05-30. Source: per-site root-cause findings (duolingo, douyin, ozon.ru,
wildberries.ru). `adidas.com` is **ALREADY FIXED** (MessagePort async drain) and is
listed only as the verified-working reference fix — that fix already works on
duolingo, so duolingo's remaining gap is a *different* root cause.

## Ranked blocker table

| # | Site | Blocker class | Root cause (file:line) | Fix | Confidence | Flippable |
|---|------|---------------|------------------------|-----|------------|-----------|
| 1 | **duolingo.com** | framework-API | Uncaught `import('data:…')` rejection aborts the drain tick → React scheduler MessageChannel macrotask never fires → `#root` empty. `crates/js_runtime/src/module_loader.rs:67-71` rejects all non-http(s) specifiers (the "data: inlined by V8" comment is false for `import()` in deno_core 0.311). | **PRIMARY:** `module_loader.rs:65` — add a `data:` branch *before* the http(s) guard that parses `data:text/javascript[;base64],<payload>`, decodes, returns `ModuleSource` Sync(Ok). Ensure unhandled-rejection policy does not terminate the event loop. **SECONDARY:** add `Blob.prototype.stream()` (`shared_apis_bootstrap.js:339`); back `CompressionStream`/`DecompressionStream` (`window_bootstrap.js:2487-2501`) with real gzip. | high | **yes** — isolation test proves one uncaught `import('data:…')` aborts `run_until_idle` and starves the React macrotask; resolving data: URLs removes the trigger. Framework-agnostic bundler idiom → likely shared by other thin SPAs. |
| 2 | **ozon.ru** | data-fetch-gate | 2-digit-year `Set-Cookie` Expires (`Mon, 31-May-27`) mis-parsed to epoch-0 by format-list ordering (`crates/net/src/cookies.rs:205-210`), then deleted by the `expires<=now` guard (`cookies.rs:73`). Cookie never stored/echoed → antibot 307 loop never breaks → THIN/156. | `cookies.rs:205` — add `"%a, %d-%b-%y %H:%M:%S GMT"` to FORMATS **before** the 4-digit form. Harden: `parse_http_date` return `None` on negative ts (degrade to session cookie). Flag latent H3 bug: `lib.rs:768` `try_h3_request` early-returns before cookie-attach; `h3_request.rs:49` never reads jar. | high | **yes** past the redirect gate — 1-line fix flips 307-loop → 403 challenge page (the necessary first step). Full PASS needs the downstream "fab" challenge VM to self-solve (separate concern, cannot even start until cookie survives). |
| 3 | **wildberries.ru** | data-fetch-gate | WBAAS in-house antibot: 1.4KB HTTP-498 challenge shell on first hit; real 1.57MB SPA only after `x_wbaas_token` cookie. BO killed mid-flight — `V8DeadlineWatcher` fires at 6567ms under the `_ => 15_000` default budget (`page.rs:2025`); no WBAAS marker so `is_anti_bot_challenge()` is false on the shell, no `started_as_wbaas_challenge` flag → 90s reload poll never arms (`page.rs:2265-2270`); reload never happens. | **Plumbing only** (vendor fingerprint solve is out of scope): (1) WBAAS detector (header `x-wbaas-token`/`server: wbaas`/498 + `/__wbaas/challenges/antibot` + `data-site-key`) at `page.rs:1939` + marker in `classify.rs`; (2) set `started_as_wbaas_challenge`, OR into poll guard `page.rs:2265-2270`; (3) host-budget tier 45-60s replacing `_ => 15_000` at `page.rs:2025`; (4) `is_wbaas_solved` at `page.rs:238`. | high (plumbing) | **maybe** — plumbing lets the SDK self-solve + reload; flips only if `challenge_fingerprint_v1.0.23.js` passes BO's FP surfaces (unverified, gated behind the same 498). If WBAAS scoring rejects BO, a vendor_solver is required. |
| 4 | **douyin.com** | data-fetch-gate (vendor) | NOT a CSR gap. First hit is ByteDance `__ac` acrawler interstitial; BO drives `byted_acrawler.sign()` → cookie → `location.reload()` correctly (`window_bootstrap.js:1402`, `lib.rs:237`), 0 JS errors. Reload returns the 6313B 验证码中间页 **interactive slide captcha** (TTGCaptcha, region:cn, server_type:whale). Whale risk engine routes datacenter/headless IP to the captcha. Reproduced engine-independently via curl. | **No render fix exists** — per-vendor challenge (acrawler + TTGCaptcha slide), out of scope per CLAUDE.md. Render path already correct. Only lever = higher FP-entropy fidelity so `sign()` is accepted — FP work, not CSR; region:cn + IP likely keep the captcha regardless. **Classify as vendor-challenge / out-of-scope.** | high | **maybe, not via render** — only via FP fidelity; region:cn/whale + datacenter IP likely keep the captcha. Confirm with same-IP v150 delta first to rule out a pure CN/IP wall. |

## Next implementation order (ROI × confidence)

1. **ozon.ru cookie date-format fix** (`cookies.rs:205`) — 1 line, high confidence,
   public-engine, definitively clears the redirect loop (the sole current blocker).
   Highest ROI: smallest change, surest win. Also fixes a whole *class* of
   2-digit-year cookie-gate sites. Bundle the negative-timestamp hardening.
2. **duolingo data: module-loader fix** (`module_loader.rs:65`) — small, high
   confidence, proven by isolation to flip the site, and framework-agnostic so it
   likely lifts other thin-render SPAs. Add the Blob.stream/CompressionStream
   robustness as a follow-up.
3. **wildberries.ru WBAAS plumbing** (4 edits, page.rs/classify.rs) — medium effort,
   high confidence in the plumbing but flip is conditional on FP scoring. Worth doing
   because it's pure plumbing mirroring existing AWS-WAF/DataDome arms; verification
   gate decides whether the residual is vendor_solvers scope.
4. **douyin** — DO NOT spend render-path budget. Classify out-of-scope
   (vendor challenge). Only revisit after a same-IP v150 delta confirms a real
   FP/signature gap rather than a CN/IP wall.
