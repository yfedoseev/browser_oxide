# 13 — File locations index

One-page lookup. Every file:line referenced anywhere in this release plan. Cross-checked against HEAD = `d00bcb2` on 2026-05-24 with uncommitted A/B/C fixes from this planning effort.

## Engine — navigation loop

| Path | Lines | What |
|---|--:|---|
| `crates/browser/src/page.rs` | 200-214 | `Page` struct (children, event_loop, url, solvers, …) |
| `crates/browser/src/page.rs` | 216-238 | `impl Drop for Page` — now reaps owned workers (fix C, uncommitted) |
| `crates/browser/src/page.rs` | 339, 853, 898, 1017, 1107, 1245 | Every entry point that uses `HttpClient::shared(&profile)` (D — SharedSession A/B candidates) |
| `crates/browser/src/page.rs` | 838-840 | `Page::default_solvers()` — returns empty `Arc<[]>` after the strip |
| `crates/browser/src/page.rs` | 1010-1097 | `navigate_with_init_solvers` — initial fetch + vendor detect logging + loop entry |
| `crates/browser/src/page.rs` | 1049-1057 | `[vendor-detect] aws-waf / datadome / wbaas` logging (detection only; no flow change) |
| `crates/browser/src/page.rs` | 1130-1208 | `reset_warm_state` — pool path warm-isolate cleanup (uncommitted) |
| `crates/browser/src/page.rs` | 1533+ | `navigate_loop_internal` — the per-iteration outer loop |
| `crates/browser/src/page.rs` | 1721-1742 | Per-host nav budget computation |
| `crates/browser/src/page.rs` | 1814-1837 | Outer drain (`run_until_idle(drain_timeout)` floored at 8 s) |
| `crates/browser/src/page.rs` | 1858-1890 | iter-0 fast-exit + budget-extend (body > 50 KB + readyState=complete) |
| `crates/browser/src/page.rs` | 1913-1938 | `SPA-fast-exit` — mount-children check, returns early when any common SPA mount has ≥1 child |
| `crates/browser/src/page.rs` | 1942-1949 | First `PENDING_NAV_JS` check after build_page |
| `crates/browser/src/page.rs` | 1955-2044 | Deferred pending-nav poll loop (gated on `is_anti_bot_challenge` / `started_as_dd_challenge` / `started_as_seccpt_challenge` / `started_as_cf_challenge`) |
| `crates/browser/src/page.rs` | 1981 | `rematerialize_iframes` — currently gated by the started_as_* challenge gates (07_DATADOME_PRIMITIVES.md plans to ungate) |
| `crates/browser/src/page.rs` | 2046-2080 | Selective CSP bypass for walmart/canadagoose/hyatt/realtor/footlocker/ticketmaster/udemy |
| `crates/browser/src/page.rs` | 2200-2360 | Cookie-delta retry path (V8 refetch + Rust-side refetch with reload headers + x-kpsdk harvest) |
| `crates/browser/src/page.rs` | 2367-2410 | `pending_info` re-check + iter+1 dispatch |
| `crates/browser/src/page.rs` | 3000-3500 | `build_page_with_scripts_*` — DOM build, script loop, drain, instrumentation install |
| `crates/browser/src/page.rs` | 3046-3061 | `__syncCookiesFromNet` drain (50 ms, intentionally short) |
| `crates/browser/src/page.rs` | 3270-3313 | Script execution loop (pre-fetched ext scripts + inline, 50 ms interleaved drain) |
| `crates/browser/src/page.rs` | 3324-3334 | DOMContentLoaded / load event firing via `setTimeout(0)` |
| `crates/browser/src/page.rs` | 3338-3366 | `<meta http-equiv="refresh">` scanner — sets `__pendingNavigation` |
| `crates/browser/src/page.rs` | 3389 | **Fix B (uncommitted)**: build_page final drain restored 200 ms → 8 s |

## Engine — pool path

| Path | Lines | What |
|---|--:|---|
| `crates/browser/src/pool.rs` | (all, ~60 lines after uncommitted +30) | `PagePool` — warm-isolate reuse path |
| `crates/dom/src/arena.rs` | 678 | DOM-walk cycle detector — triggers >100k-node panic on wellsfargo in pool mode |

## JS bootstrap scripts (ship to every page)

| Path | Lines | What |
|---|--:|---|
| `crates/js_runtime/src/js/stealth_bootstrap.js` | (all) | Function.prototype.toString patch + _nativeTag / _maskFunction / _maskAsNative helpers — must run FIRST |
| `crates/js_runtime/src/js/dom_bootstrap.js` | 1080-1110 | `<form>` submit / requestSubmit / `__pendingNavigation` setter |
| `crates/js_runtime/src/js/dom_bootstrap.js` | 1119-1135 | `_reflectStr` / `_reflectBool` IDL property ↔ attribute reflection helpers |
| `crates/js_runtime/src/js/window_bootstrap.js` | 1290-1320 | `location` setter parser + pending-nav signal helpers |
| `crates/js_runtime/src/js/window_bootstrap.js` | 1385-1398 | `location.reload` / `location.replace` / `location.assign` setters |
| `crates/js_runtime/src/js/cleanup_bootstrap.js` | (all) | Hides Deno + internal globals from user JS — runs LAST in build_page |
| `crates/js_runtime/src/js/timer_bootstrap.js` | (all + uncommitted +47) | `setTimeout`/`setInterval`/`clearTimeout` + new `__bgSetTimeout` unref'd helper + `__cancelAllTimers` for warm reset |
| `crates/browser/src/js/humanize.js` | 41-50 (uncommitted) | `_sched = __bgSetTimeout || setTimeout` selector |
| `crates/browser/src/js/humanize.js` | 268, 279 | Mouse/scroll setTimeouts routed through `_sched` |

## Worker subsystem

| Path | Lines | What |
|---|--:|---|
| `crates/js_runtime/src/extensions/worker_ext.rs` | 170-187 | `worker_registry()` — process-global slot map |
| `crates/js_runtime/src/extensions/worker_ext.rs` | 212-260 | `op_worker_spawn` — 64 MB stack + child JsRuntime; **uncommitted** push to `WorkerOwnership` (fix C) |
| `crates/js_runtime/src/extensions/worker_ext.rs` | 380-435 | `op_worker_terminate` + `terminate_worker_inner` + `drain_owned_workers` + `WorkerOwnership` struct (last 3 added by fix C) |
| `crates/js_runtime/src/extensions/worker_ext.rs` | 398-420 | `op_worker_await_message` — async op replacing the old 5 ms setInterval pump |
| `crates/js_runtime/src/extensions/worker_ext.rs` | 506+ | `worker_extension!` ops list |
| `crates/js_runtime/src/runtime.rs` | 109-111 | `HEAP_INITIAL = 1 GB`, `HEAP_MAX = 4 GB` (motivated by creepjs) |
| `crates/js_runtime/src/runtime.rs` | 145-162 | Main isolate OpState seeding — uncommitted line for `WorkerOwnership::default()` |
| `crates/js_runtime/src/runtime.rs` | 314-345 | Worker-isolate setup (separate code path for the child runtime) |

## State + DOM

| Path | Lines | What |
|---|--:|---|
| `crates/js_runtime/src/state.rs` | 1-80 | `DomState` (dom, layout_engine, base_url, console, storage, stylesheets, csp_policy, …) |
| `crates/dom/src/arena.rs` | (all) | Arena-allocated DOM. `NodeId` is `Copy` + `u32`. No `Rc<RefCell<>>`. |
| `crates/browser/src/classify.rs` | (all) | Verdict classifier — `Pass / ThinShell / CHL / ThinBody / Error` |

## HTTP / TLS / stealth

| Path | Lines | What |
|---|--:|---|
| `crates/net/src/lib.rs` | 140-174 | `SharedSession` — process-wide cookies + accept_ch + DNS + Alt-Svc (DNS/AltSvc intentionally NOT shared per leboncoin reasoning) |
| `crates/net/src/lib.rs` | (HttpClient::shared, HttpClient::new) | Per-profile HTTP client constructors |
| `crates/net/src/headers.rs` | 82+ | `nav_headers_reload` — JS-initiated `location.reload()` style headers |
| `crates/net/src/tls.rs` | (all) | boring2 (Cloudflare BoringSSL fork) — byte-perfect Chrome ClientHello + H2 fingerprint |
| `crates/stealth/profiles/chrome_148_macos.yaml` | (all) | Profile YAML schema reference — read this first when learning the system |
| `crates/stealth/profiles/*.yaml` | per profile | Per-profile UA, ClientHints, TLS impersonate codename, WebGL vendor/renderer, navigator.* values |
| `crates/stealth/src/presets/*.rs` | per profile | Programmatic profile constructors (`chrome_148_macos()`, etc.) |

## Extension ops (other)

| Path | Lines | What |
|---|--:|---|
| `crates/js_runtime/src/extensions/nav_ext.rs` | 1-50 | `op_set_pending_nav` (called from JS to signal nav-pending) |
| `crates/js_runtime/src/extensions/fetch_ext.rs` | (FETCH_CLIENT, ACTIVE_CSP, CSP_VIOLATIONS) | thread-local statics (a11044f) |
| `crates/event_loop/src/lib.rs` | 270-280 | `NavSignal` short-circuit for `run_until_idle` |
| `crates/event_loop/src/lib.rs` | 425-440 | `reset_nav_pending` — scrubs `_browser_oxide.__pendingNavigation` JS-side |
| `crates/event_loop/src/lib.rs` | 441 | `pub fn runtime_mut(&mut self) -> &mut BrowserJsRuntime` |

## Sweep harness + corpus

| Path | Lines | What |
|---|--:|---|
| `crates/browser/tests/holistic_sweep.rs` | 1-700 | The 126-site corpus definition (`site!` macro entries) |
| `crates/browser/examples/sweep_metrics.rs` | (all) | BO sweep harness (cold + pool, JSON output matching competitor format) |
| `crates/browser/examples/nav_timed.rs` | (all) | Per-engine timing micro-benchmark |
| `benchmarks/bench_corpus_v2.py` | 48-106 | RSS measurement helpers (`get_rss_mb`, `all_descendant_pids`, `tree_rss_mb`) |
| `benchmarks/bench_corpus_v2.py` | 109-160 | `visit()` — per-site visit (CDP for Chromium, route events for Firefox) |
| `benchmarks/bench_corpus_v2.py` | 195-211 | `run_playwright` |
| `benchmarks/bench_corpus_v2.py` | 214-235 | `run_patchright` |
| `benchmarks/bench_corpus_v2.py` | 240-283 | `run_camoufox` — fix A applied at 256+ (use `browser.process.pid`, was first-/proc-child) |
| `benchmarks/build_report.py` | (all) | Aggregator: per-engine JSON → markdown comparison report |
| `benchmarks/run_full_sweep.sh` | (all) | Serial orchestrator — 4 BO cold + 1 BO pool + 4 competitor engines |

## Detection markers

| Marker | Where used | Vendor |
|---|---|---|
| `x-amzn-waf-action` response header | `crates/browser/src/page.rs:1049` | AWS WAF |
| `x-datadome` response header | `crates/browser/src/page.rs:1052` | DataDome |
| `x-wbaas-token` response header | `crates/browser/src/page.rs:1055` | wbaas |
| `cf-mitigated` response header | (planned for 07 — not currently checked) | Cloudflare |
| Body contains `/ips.js` | `crates/browser/src/page.rs:2273` | Kasada |
| Body contains `/149e9513-` | `crates/browser/src/page.rs:2274` | DataDome |
| Body contains `kpsdk` | `crates/browser/src/page.rs:2275` | Kasada |
| Body contains `_abck` | `crates/browser/src/page.rs:2276` | Akamai |
| Body contains `bm_sz` | `crates/browser/src/page.rs:2277` | Akamai |
| Body contains `captcha-delivery.com` | `crates/browser/src/page.rs:2278` | DataDome |
| Body contains `dd-script` | `crates/browser/src/page.rs:2279` | DataDome |
| Body contains `dd_engagement` | `crates/browser/src/page.rs:2280` | DataDome |
| Body contains `/cdn-cgi/challenge-platform/` | `crates/browser/src/page.rs:2281` | Cloudflare |

## Workspace docs (existing)

| Path | What |
|---|---|
| `CLAUDE.md` | Workspace conventions for AI assistants |
| `CONTRIBUTING.md` | Human contributor guide |
| `SCOPE.md` | What this project is for / what it isn't |
| `SECURITY.md` | Vuln reporting |
| `docs/ARCHITECTURE.md` | Workspace layout + dependency graph |
| `docs/STEALTH.md` | Stealth profile configuration guide |
| `docs/BENCHMARK_2026_05_24.md` | Last full sweep narrative report |
| `docs/PERFORMANCE_2026_05_24.md` | Per-page perf root-cause investigation |
| `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` | ±5 noise floor characterization |
| `docs/BENCHMARK_2026_05_23.md` | Previous sweep (superseded by 2026-05-24) |
| `docs/<CRATE>.md` | Per-crate engineering reference |
| `deny.toml` | License + dependency policy (mechanically enforced by `deny` CI job) |

## Per-crate ownership

15 workspace members. Use `Cargo.toml` `[workspace.members]` for the full list. Quick reference of the biggest ones:

| Crate | Purpose |
|---|---|
| `browser` | Top-level Page / navigate / classify |
| `dom` | Arena-allocated DOM |
| `layout` | Layout engine |
| `css_parser`, `css_selectors`, `css_values`, `css_cascade` | CSS stack (from-scratch — we don't pull Servo MPL crates) |
| `html_parser` | HTML5 parser (has one `unsafe` block — `// SAFETY:` documented) |
| `js_runtime` | deno_core wrapper + extensions/bootstrap JS |
| `event_loop` | Tokio LocalSet driver + NavSignal |
| `net` | HTTP/2 + TLS via boring2 |
| `stealth` | Profile YAML loader + presets |
| `canvas` | Canvas 2D + WebGL (one `unsafe` for skia bindings) |

## Raw artefact paths (sweep outputs)

| Path | What |
|---|---|
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_cold.json` | BO chrome cold sweep results |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_cold.log` | Per-site trace output |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_pixel_9_pro_chrome_148_cold.json` | BO pixel cold |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_iphone_15_pro_safari_18_cold.json` | BO iphone cold |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_firefox_135_macos_cold.json` | BO firefox cold |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/bo_chrome_148_macos_pool.json` | BO chrome pool (97 sites, partial — wellsfargo panic) |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/comp_camoufox.json` | Camoufox results |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/comp_{playwright,patchright,playwright_stealth}.json` | Competitor results |
| `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/run.log` | Orchestrator log |
| `/tmp/spotcheck_corpus.json` | Curated 33-site stratified spot-check corpus (created during 2026-05-24 work) |
| `/tmp/gap_corpus.json` | Curated 11-site gap corpus (the Camoufox-only-pass set) |
| `/tmp/amazon_de_curl.html` | Captured 2011-byte AWS WAF stub |
| `/tmp/reddit_curl.html` | Captured 8424-byte reddit verify-page |
