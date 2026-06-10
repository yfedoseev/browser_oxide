# Configuration — environment variables

BrowserOxide reads a number of environment variables for runtime tuning,
stealth/identity, performance, and debugging. None are required — every one
has a sensible default. This page documents the user-facing ones.

`BROWSER_OXIDE_*` are read by the engine (the single `browser_oxide` crate
and its modules).

---

## Navigation & timeout budgets

The navigation loop runs each page under a **wall-clock budget** — a *ceiling*,
not a fixed wait: a page that renders early returns immediately. Heavy /
challenge pages need a larger ceiling. Defaults are chosen per host class; these
vars override them.

| Variable | Default | Effect |
|----------|---------|--------|
| `BROWSER_OXIDE_NAV_BUDGET_MS` | per-host tier (15s–90s) | **Global** override of the per-navigation budget ceiling for **every** site. Use to give all sites more (or less) time. |
| `BROWSER_OXIDE_SECCPT_BUDGET_MS` | `140000` (140s) | Budget ceiling for the **Akamai sec-cpt challenge class** (detected by marker, e.g. homedepot). The in-VM SHA-256 PoW lands ~131s on our V8, so the normal tiers are too tight. Set `30000` for a fast sweep that skips the PoW, `1000000` to never cap. Applies generically to any sec-cpt site — no per-host hardcode. |
| `BROWSER_OXIDE_NAV_BUDGET_EXTEND_MS` | `25000` (25s) | One-shot extension granted when iter 0 returns real content but is still mid-render (heavy SSR/SPA). |
| `BROWSER_OXIDE_BUILD_BUDGET_MS` | `25000` (25s) | Wall-clock budget for the build phase (parse + inline-script execution), preempts CPU-bound inline scripts. |

> When raising a budget, also give the sweep enough per-site wall-clock so
> the run isn't killed before the budget elapses — e.g. the 140s sec-cpt
> default needs a per-site timeout of ~170s or more.

## Stealth, identity & networking

| Variable | Effect |
|----------|--------|
| `BROWSER_OXIDE_PROFILE` / `BROWSER_OXIDE_TARGET` | Select the stealth profile (e.g. `chrome_148_macos`, `firefox_135_macos`) for tools/examples that honor it. |
| `BROWSER_OXIDE_PROXY` | Upstream proxy URL for the engine's HTTP/TLS stack. |
| `BROWSER_OXIDE_CSP_BYPASS` | If set, parse + report CSP but do not enforce it (useful for A/B-ing CSP effects). |
| `BROWSER_OXIDE_BLOCKER` / `BROWSER_OXIDE_BLOCKER_RULES` | Enable the optional ad/tracker blocker (the `blocker` feature) and point it at a rules file. |
| `BROWSER_OXIDE_BEHAVIOR_SEED` | Seed the humanized-behavior engine for deterministic mouse/key timing (reproducible runs). |
| `BROWSER_OXIDE_INIT_JS` | Path to a JS file injected **before** the page's own scripts (pre-app instrumentation/diagnostics). |

## Session, cookie & state sharing

By default the engine **shares** cookies, the HTTP session, and learned
`Accept-CH` hints across navigations in a process (realistic, like a browser
keeping state). These vars opt out — useful for strict per-navigation isolation
or for benchmark fairness.

| Variable | Effect |
|----------|--------|
| `BROWSER_OXIDE_COOKIE_JAR` | Path to a cookie file to **pre-load** as the initial jar (import an existing session). |
| `BROWSER_OXIDE_NO_SHARED_COOKIES` | If set, don't share the cookie jar across navigations. |
| `BROWSER_OXIDE_NO_SHARED_SESSION` | If set, don't reuse the HTTP session (fresh connections/state per nav). |
| `BROWSER_OXIDE_NO_SHARED_ACCEPT_CH` | If set, don't carry learned `Accept-CH` client-hint upgrades across navigations. |
| `BROWSER_OXIDE_NO_XCOM_ISOLATION` | If set, disable the x.com/twitter cookie-collision isolation workaround. |

## V8 snapshot & performance

| Variable | Default | Effect |
|----------|---------|--------|
| `BROWSER_OXIDE_USE_SNAPSHOT` | unset (**off**) | Set to `1` to enable the V8 startup snapshot (faster cold start). **Disabled by default** on V8-149 — snapshot *restore* currently segfaults; the cold-bootstrap path is used instead. |
| `BROWSER_OXIDE_NO_SNAPSHOT_CACHE` | unset | Disable the on-disk snapshot cache (forces an in-memory build per process). |
| `BROWSER_OXIDE_SNAPSHOT_CACHE` | system temp dir | Override the snapshot cache directory. |

## Debugging & tracing

| Variable | Effect |
|----------|--------|
| `BROWSER_OXIDE_DEBUG_NAV` | Verbose navigation logging: `[net] sending request …`, budget/watcher events, challenge-poll + cookie-delta-retry traces, challenge-detect lines. |
| `BROWSER_OXIDE_FP_OUTDIR` | Directory to dump captured fingerprint artifacts. |
| `BROWSER_OXIDE_DUMP_POST_DIR` | Directory to dump outgoing POST bodies (request payload inspection). |
| Per-subsystem trace flags: `BROWSER_OXIDE_COOKIE_TRACE`, `BROWSER_OXIDE_CHALLENGE_TRACE`, `BROWSER_OXIDE_SECCPT_TRACE`, `BROWSER_OXIDE_DEBUG_REDIRECTS`, `BROWSER_OXIDE_DEBUG_CHILD_REALM`, `BROWSER_OXIDE_VARIANCE_LOGS` | Emit extra logs for cookies, the interstitial-challenge path, the sec-cpt path, redirects, child realms, and run-to-run variance. Dev/triage use. |

### Timing-profiling flags

These emit **wall-clock timing breakdowns** for performance work. Note: these
are *profiling* flags — they have nothing to do with stealth **profiles**
(`BROWSER_OXIDE_PROFILE` above).

| Variable | Effect |
|----------|--------|
| `BROWSER_OXIDE_BUILD_PROFILE` | Log build-phase (parse + inline-script) timing. |
| `BROWSER_OXIDE_WARM_PROFILE` | Log warm-navigation timing. |
| `BROWSER_OXIDE_EVENT_LOOP_PROFILE` / `BROWSER_OXIDE_EVENT_LOOP_PROFILE_LABEL` | Profile the event loop (the label tags the output for comparison). |
| `BROWSER_OXIDE_SAMPLE_PROFILE` | Enable sampling-profile output in `sweep_metrics` (chrome profile only). |

## Sweeps (`sweep_metrics` example)

The `sweep_metrics` example renders a corpus JSON and records per-site
timing, classifier tag, and body length:

```bash
cargo run --release -p browser_oxide --example sweep_metrics -- <profile> <corpus.json> <out.json>
```

These engine vars affect it:

| Variable | Effect |
|----------|--------|
| `BROWSER_OXIDE_SWEEP_POOL` | Use a **warm page pool** (faster). ⚠️ The warm path skips challenge-follow, so it **undercounts** challenge sites — headline numbers must be measured **cold** (pool off). |
| `BROWSER_OXIDE_PARALLEL_WORKERS` | Worker count for the in-process `holistic_sweep` test. |
| `BROWSER_OXIDE_SAMPLE_PROFILE` | Sampling-profile output (chrome profile only). |

Optionally tune `BROWSER_OXIDE_SECCPT_BUDGET_MS` (default `140000`): lower it
(e.g. `30000`) for a faster sweep that doesn't wait out the Akamai sec-cpt PoW.

---

> **Test-only vars** read solely by `#[ignore]` integration tests (e.g.
> `BROWSER_OXIDE_TEST_PROXY`, which points the proxy round-trip test at a live
> proxy) are intentionally omitted here — they don't affect normal runs.
