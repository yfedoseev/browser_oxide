# 22 — Production deployment guide

**Audience:** customer engineers wiring browser_oxide into a scraping or
automation pipeline. From "I have a URL list, how do I scrape it?" to
k8s / Lambda / Fargate / embedded-library patterns.

**One-paragraph thesis:** BO is an *in-process* Rust engine — there is
no browser subprocess to launch, no CDP socket to negotiate, no
Chromium tree to memory-budget. Pick a deployment shape based on
your workload (continuous queue vs bursty request-response vs
embedded), size memory at **400-800 MB per worker isolate**, and use
the `PagePool` cold-fallback pattern (§2.4) to combine throughput and
robustness. The pool path delivers ~14 pages/min/worker (per
`10_TIMING_OPTIMIZATION.md §1`); the cold path delivers 2.3-2.7
pages/min/worker (higher robustness; safe to throw away the
`Page` after extraction). Beyond that, everything is rotation (per
`11_PER_PROFILE_STRATEGY.md`) and retry shape (§2.5 of this doc).

---

## 1. Deployment shapes — when to use which

The five shapes below cover ~95% of real customers. The remainder is
some combination of these (e.g. "embedded library inside a k8s pod").

| Shape | Best for | Pool? | Profiles | Per-process memory | Per-page wall-time |
|---|---|---|---|---|---|
| **Single-process daemon** | low-volume continuous scraping (10-100 URLs/min) | yes | rotate per-URL list | 400-800 MB | 2.6 s pool / 15 s cold |
| **Long-running worker pool** | medium-volume queue worker (100-1000 URLs/min/worker) | yes, sized 1-10 | rotate per-URL | 400-1500 MB (with watchdog) | 2.6 s pool / 15 s cold |
| **k8s Job (one URL = one job)** | high-isolation, retryable, large fan-out | no (cold path) | one profile per job | 400-500 MB | 15 s cold (no warm-up to amortize) |
| **AWS Lambda (one URL = one invocation)** | bursty, serverless, event-driven | no (cold path) | profile derived from event payload | 1024 MB (Lambda min for headroom) | 15-20 s cold + ~150 ms init |
| **Cloud Run / Fargate (container per request)** | container-per-job, autoscaled | yes within container lifetime | rotate per-URL across the container's lifetime | 1 GB request / 2 GB limit | 2.6 s pool / 15 s cold after warm |
| **Embedded library** | dev tools, browser-as-a-library, agent loops | yes, per-thread | per-call | depends on host process | depends on host budget |

Numbers reference: cold RSS peaks 388-472 MB per profile (per
`09_MEMORY_OPTIMIZATION.md §2`); pool RSS reached 1365 MB on the
2026-05-24 sweep but has a known DOM-arena retain bug
(`09_MEMORY_OPTIMIZATION.md §6` — fix in flight for v0.1.0; target
≤ 800 MB).

Per-profile cold-start (in-process):

- **0 ms launch** — there is no subprocess; the first `Page` constructs
  in the calling tokio task
- **~150 ms first-isolate spin-up** — V8 isolate + bootstrap script
  execution + extension init (per `10_TIMING_OPTIMIZATION.md §1
  truth 1`)
- **0 ms thereafter on pool path** — every subsequent `PagePool::navigate`
  reuses the warm isolate

### 1.1 Single-process daemon

The simplest customer shape — one Rust binary, one tokio runtime, one
or more `PagePool` instances, a queue of URLs.

```rust
use browser::{Page, PagePool};
use stealth::presets::pixel_9_pro_chrome_148;
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let local = tokio::task::LocalSet::new();
    local.run_until(async {
        let pool = PagePool::new(4);
        let profile = pixel_9_pro_chrome_148();

        for url in std::env::args().skip(1) {
            let page = match pool.navigate(&url, profile.clone()).await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[{url}] pool err: {e}; falling back to cold");
                    Page::navigate(&url, profile.clone(), 3).await?
                }
            };
            let html = page.content();
            println!("{url}: {} bytes", html.len());
            pool.release(page);
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    }).await
}
```

Notes:
- `flavor = "current_thread"` is mandatory: V8 isolates are
  per-thread (per `CLAUDE.md` "Tests are single-threaded").
- `LocalSet::run_until` keeps `!Send` work pinned to the current OS
  thread — `Page` and `PagePool` hold V8 handles which are
  intrinsically `!Send`.
- The cold-fallback pattern is the recommended default for production
  use until the `wellsfargo` pool panic is fixed (per
  `10_TIMING_OPTIMIZATION.md §4`; tracked as **R-001** in
  `24_RISK_REGISTER.md`).

Memory budget: ~400-800 MB resident; CPU: 1 core saturated under load.

### 1.2 Long-running worker pool

Multi-worker shape — N OS threads, each with its own `tokio` current-
thread runtime and its own `PagePool`. The job queue is an mpsc
channel (cross-thread is fine because URLs are `String`s; only the
`Page` handle stays thread-local).

Why one tokio runtime per thread:

1. V8 isolates are per-thread (`CLAUDE.md`). One runtime per thread →
   one set of isolates per thread → no cross-thread isolate access.
2. tokio `current_thread` flavor avoids the work-stealing scheduler,
   which would otherwise occasionally try to `Send` futures
   between cores.
3. `PagePool` is `!Send` (its `Page`s are `!Send`), so it can't be
   shared across threads. Per-thread `PagePool` is the only correct
   topology.

Memory: ~400-800 MB per worker × N workers. CPU: 1 core per worker
under full load. Throughput: 14 pages/min/worker (pool path) so
N=16 workers = 224 pages/min wall-clock.

Skeleton (spec — write at `crates/browser/examples/production_worker.rs`
per §10 acceptance):

```rust
// thread_count = num_cpus; each spawns its own LocalSet + PagePool
// receive URL on input channel, navigate, send result on output channel
//
// Per-worker:
//   tokio runtime (current-thread)
//     LocalSet
//       PagePool { max_size: 4 }   <-- 1 warm isolate + 3 cold capacity
//       loop {
//           let url = job_rx.recv().await;
//           let page = pool.navigate(...).await.or_else(cold_fallback).await?;
//           let html = page.content();
//           pool.release(page);
//           result_tx.send((url, html)).await?;
//       }
//
// Periodic recycle (per §3):
//   every 500 navigations on this worker, drop the pool + rebuild
//   prevents the DOM-arena retain (R-010 / 09 §6) from growing
//   resident set unboundedly until the §6 fix lands.
```

### 1.3 k8s Job (one URL = one job)

When you want OOM isolation per URL, or you want each URL retryable
via k8s `backoffLimit`, or your URLs have wildly different
profile requirements: model each URL as a Job.

- **Container**: `FROM scratch` + the Rust binary built with
  `cargo build --release` (we are statically linked; no system browser
  needed). On Linux, the binary is ~80-120 MB depending on whether the
  `webgl-render` feature is enabled (per `crates/browser/Cargo.toml:8-13`
  — off by default, adds ~15 MB).
- **Memory request**: 500 MB; **limit**: 1 GB. Cold path peaks at
  388-472 MB depending on profile (`09 §2`).
- **CPU request**: 500m; **limit**: 1.
- **Profiles**: one preset per Job. Pass via env var or argv.
- **Cold path only** — there's no warm path benefit when each pod
  navigates exactly one URL.

Reference manifest (place at `deployment/k8s/job-scrape-one.yaml`,
per §10 acceptance):

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: scrape-job
spec:
  backoffLimit: 2
  template:
    spec:
      restartPolicy: Never
      containers:
      - name: browser-oxide
        image: ghcr.io/yfedoseev/browser_oxide:v0.1.0-parity
        args: ["scrape", "$(URL)"]
        env:
        - name: URL
          value: "https://example.com/"
        - name: PROFILE
          value: "pixel_9_pro_chrome_148"
        - name: BROWSER_OXIDE_NAV_BUDGET_MS
          value: "20000"
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            memory: "500Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1"
```

### 1.4 AWS Lambda (one URL = one invocation)

Lambda is well-suited to BO's in-process model: no Chromium binary to
ship in a Layer, no headless browser cold-start, no `/tmp` write-out
of a profile directory.

- **Runtime**: `provided` (custom AL2). The Lambda binary IS the Rust
  binary, built with `cargo lambda build --release` (or vanilla
  `cargo build --release --target x86_64-unknown-linux-musl` per AWS
  Custom Runtime docs).
- **Memory**: 1024 MB. BO cold peaks at ~470 MB; Lambda gives 1024 MB
  the most cost-efficient CPU allocation; below that you starve V8.
- **Timeout**: 60 s. Per-page p99 cold is 115 s on the worst sites
  (`10 §1`) — but those are sites that wouldn't pass anyway; benign
  cold is ≤ 25 s.
- **VPC**: optional. **Caveat**: Lambda IP ranges are well-known and
  some anti-bot vendors blocklist them. If you're hitting Cloudflare /
  AWS WAF / DataDome, use a NAT-Gateway egress with rotating EIPs OR
  use a managed proxy (BrightData, Smartproxy, …). Per
  `12_COMPETITIVE_LANDSCAPE.md §3.5`, AWS WAF is one of BO's weak
  spots anyway — don't expect to flip amazon.* sites from Lambda.
- **Cold-start latency**: in-process means ~150 ms first isolate
  spin-up + your nav time. Lambda's own cold-start adds ~100-300 ms
  for a custom runtime. Total: ~3-15 s for a typical nav.

Handler skeleton (spec — write at `deployment/lambda/handler.rs` per
§10 acceptance):

```rust
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Req { url: String, profile: String }

#[derive(Serialize)]
struct Resp { url: String, body: String, len: usize, ms: u64 }

async fn handler(event: LambdaEvent<Req>) -> Result<Resp, Error> {
    let t0 = std::time::Instant::now();
    let profile = match event.payload.profile.as_str() {
        "chrome_148_macos" => stealth::presets::chrome_148_macos(),
        "pixel_9_pro_chrome_148" => stealth::presets::pixel_9_pro_chrome_148(),
        // ... per crates/stealth/src/presets.rs
        _ => stealth::presets::pixel_9_pro_chrome_148(),
    };
    let local = tokio::task::LocalSet::new();
    let body = local.run_until(async {
        let page = browser::Page::navigate(&event.payload.url, profile, 3).await?;
        Ok::<_, Error>(page.content())
    }).await?;
    Ok(Resp { url: event.payload.url, len: body.len(), body, ms: t0.elapsed().as_millis() as u64 })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}
```

### 1.5 Cloud Run / Fargate (container per request)

Like Lambda but with longer-lived containers and more memory headroom.

- **Image**: same `FROM scratch` + Rust binary as §1.3.
- **Memory**: 1 GB request / 2 GB limit (pool path can grow under load
  until R-010 is resolved).
- **CPU**: 1 vCPU.
- **Request timeout**: 60 s (matches Lambda; the cold p99 doesn't
  matter because timeouts are retried).
- **Cold-start**: container start ~3-5 s + first nav 15 s = ~20 s p99
  on a cold container; subsequent requests are pool-fast (~2.6 s
  median).
- **Concurrency**: Cloud Run allows N concurrent requests per
  container (`--concurrency`). Don't set > 4 — every concurrent
  request needs its own `Page`, which means N isolates resident.
  V8's per-isolate cost (HEAP_INITIAL = 1 GB reservation per
  `09 §5`) means 4 concurrent requests reserve 4 GB. Set
  `--concurrency 4` and let the autoscaler spin up containers.

### 1.6 Embedded library

For dev tools, agent loops, or applications that already have a
tokio runtime.

```toml
# Cargo.toml of your application
[dependencies]
browser = { git = "https://github.com/yfedoseev/browser_oxide", tag = "v0.1.0-parity" }
stealth = { git = "https://github.com/yfedoseev/browser_oxide", tag = "v0.1.0-parity" }
```

(Crates are `publish = false` today — per
`crates/browser/Cargo.toml:8` and the 14 other `publish = false`
lines; "9 of 15 crate names collide on crates.io". Git-tag dependency
is the supported install method until names land.)

Caveats:
- V8 isolate per-thread — if your application is multi-threaded,
  drive BO from a dedicated worker thread or use `tokio::task::
  spawn_blocking` + `LocalSet`.
- Init takes ~150 ms — instantiate once, reuse via `PagePool`.
- First `cargo build` fetches ~130 MB of prebuilt V8 binaries
  (per `CLAUDE.md` "V8 via deno_core 0.311 (prebuilt binaries,
  ~130 MB on first fetch)"). Document this for your end users.

---

## 2. The customer onboarding playbook

A customer arrives with a URL list. Six steps, in order.

### Step 1 — Triage the URL list (detect anti-bot vendor per URL)

Most URLs in a typical scraping job fall into 4 buckets: benign (no
WAF), known-recoverable (AWS WAF / DataDome / SPA), known-hard
(Kasada), and unknown (need to fetch and look).

A quick triage script (zero authentication, just response headers):

```bash
# Per-URL vendor detection (returns the WAF flavour)
detect_vendor() {
    local url="$1"
    local hdr
    hdr=$(curl -s -D - -o /dev/null -m 10 "$url" 2>/dev/null)
    if   grep -qi 'x-amzn'        <<<"$hdr"; then echo "AWS"
    elif grep -qi 'x-datadome'    <<<"$hdr"; then echo "DataDome"
    elif grep -qi 'x-akamai'      <<<"$hdr"; then echo "Akamai"
    elif grep -qi 'cf-ray'        <<<"$hdr"; then echo "Cloudflare"
    elif grep -qi 'kasada'        <<<"$hdr"; then echo "Kasada"
    elif grep -qi 'x-perimeterx'  <<<"$hdr"; then echo "PerimeterX"
    else echo "none"
    fi
}

for url in $(cat /tmp/urls.txt); do
    printf '%s\t%s\n' "$(detect_vendor "$url")" "$url"
done | sort | uniq -c | sort -rn
```

Heuristic only — some vendors hide entirely behind first-party origins
(e.g. Amazon's own AWS WAF doesn't always set `x-amzn`). For deeper
triage, run a one-off cold sweep with `sweep_metrics` (per
`crates/browser/examples/sweep_metrics.rs:1-100`) and read the
`tag` field on the per-site results.

The reference classification rules are in `crates/browser/src/classify.rs`
(referenced from `00_README.md` source-of-truth artifacts).

### Step 2 — Pick profile(s) per URL bucket

From `11_PER_PROFILE_STRATEGY.md §4.1` (the routing decision tree):

| Bucket | First-choice profile | Rationale |
|---|---|---|
| `amazon.*` family | `firefox_135_macos` | Firefox uniquely passes amazon-com; lowest WAF risk class for amazon retail |
| Known DataDome | `iphone_15_pro_safari_18` | iphone uniquely passes yelp |
| Known PerimeterX | `pixel_9_pro_chrome_148` | Firefox loses zillow; pixel cheapest at 388 MB |
| Cloudflare managed challenge | `chrome_148_macos` | iphone Cloudflare-CHL'd 6/10 known-CF sites; chrome desktop is safest |
| Mobile-friendly SPA | `pixel_9_pro_chrome_148` | mobile UA gets non-WAF serve |
| Kasada | `pixel_9_pro_chrome_148` (low success rate — try all 4) | per `08_KASADA_FRONTIER.md` |
| Benign / default | `pixel_9_pro_chrome_148` | single-best profile at 102 Pass + lowest RSS |

Implementation reference: `pick_first_profile()` /
`fallback_chain()` spec at `11_PER_PROFILE_STRATEGY.md §4` (to be
implemented at `crates/browser/src/router.rs` per the v0.1.0
acceptance).

### Step 3 — Budget the run

Sizing inputs:

| Variable | Source | Typical value |
|---|---|---|
| Per-URL wall-time | `10 §1` | 2.6 s pool / 15 s cold (median) |
| Per-worker memory | `09 §1`, `09 §6` | 400-800 MB |
| Pages per worker per minute | derived | 14 pool / 2.5 cold |
| Worker CPU | observed | 1 core saturated under load |

Worked example — 100k URLs, 16-vCPU host with 64 GB RAM:

- Worker count: 16 (1 per vCPU)
- Per-worker memory: 800 MB max → 16 × 0.8 = 12.8 GB used; 51 GB headroom
- Throughput per worker (pool): 14 pages/min → 224 pages/min total
- Wall-clock: 100k / 224 = **7.4 hours**
- Cost on $0.10/hr equivalent: ~$0.74 (per `12 §4.2`)
- Headroom: 5× memory unused → can run multiple isolates per worker
  if profile routing requires (one warm pool per profile)

### Step 4 — Write the worker loop

The pattern from `10 §3` (warm pool) with the cold fallback for
robustness (per `BENCHMARK_2026_05_24.md §7` due to the wellsfargo
pool panic — R-001 in `24_RISK_REGISTER.md`):

```rust
// Pool-first with cold fallback. Pool gives 5.8× speedup on benign
// sites; cold fallback handles the wellsfargo class until the panic
// is fixed (R-001).
async fn navigate_with_fallback(
    pool: &browser::PagePool,
    profile: stealth::StealthProfile,
    url: &str,
) -> Result<browser::Page, deno_core::error::AnyError> {
    match pool.navigate(url, profile.clone()).await {
        Ok(p) => Ok(p),
        Err(e) => {
            tracing::warn!(url, err=%e, "pool path failed; cold fallback");
            browser::Page::navigate(url, profile, 3).await
        }
    }
}
```

`3` is the iteration count for the cold path — enough to clear
typical challenge round-trips (per `crates/browser/src/page.rs:1758-1770`
outer iteration loop) without burning the worst-case 90 s budget on a
hard-blocked site.

### Step 5 — Error handling

The classifier (`crates/browser/src/classify.rs` — referenced from
`14_TESTING_VALIDATION.md`) emits five outcome tags:

| Tag prefix | Meaning | Recommended action |
|---|---|---|
| `L3-RENDERED` (body ≥ 15 KB) | Strict-pass | Process normally |
| `L3-RENDERED` (body < 15 KB) | Thin shell — SPA may not have hydrated | Retry once with a different profile (per `11 §4.3` fallback chain) |
| `*-CHL` (any vendor) | Vendor challenge not solved | Log + skip OR register `vendor_solvers::default_solvers()` (private crate; per `CLAUDE.md` scope) |
| `THIN-BODY` (body < 1 KB) | Connection drop or hard block | Retry once with a different profile; if still THIN-BODY, mark hard-blocked |
| `ERROR` | Process error (network, panic) | If panic in pool path, drop pool and respawn (R-001) |

Pseudo-handler:

```rust
let html = match navigate_with_fallback(&pool, profile.clone(), &url).await {
    Ok(p) => {
        let body = p.content();
        pool.release(p);
        body
    }
    Err(e) => {
        // PANIC in pool — must recreate the pool because the V8
        // isolate that backed it may be in an indeterminate state.
        pool = browser::PagePool::new(pool_size);
        match browser::Page::navigate(&url, profile.clone(), 3).await {
            Ok(p) => p.content(),
            Err(e2) => {
                tracing::error!(url, err=%e2, "cold fallback also failed");
                continue;
            }
        }
    }
};

// Classify
let body_kb = html.len() / 1024;
if body_kb < 1 {
    retry_with_next_profile(&url);
} else if body_kb < 15 {
    log_thin_shell(&url, body_kb);
}
```

Retry budget: max 2 retries with profile rotation per `24 §R-017`.
Don't retry forever — a hard-blocked URL stays hard-blocked, and
"retry until pass" is how you get IP-banned.

### Step 6 — Wire up observability

See §6 of this doc.

---

## 3. Memory + lifecycle management for long-running processes

The two memory failure modes to defend against:

1. **Pool grows unboundedly** — the DOM-arena retain (per `09 §6`).
   Until that fix lands, every warm-pool navigation can grow the
   isolate's resident set. The 2026-05-24 sweep showed 1365 MB at
   97 sites; trend was monotonic.
2. **Worker leak** — fixed but uncommitted on `main` as of 2026-05-24.
   Per `09 §4`. Without the fix, every site that spawns a `new
   Worker(...)` leaks a 64 MB OS thread + child JsRuntime until
   process exit.

### Recycle policy (defensive — until fixes land)

Per-worker, after every N successful navigations, drop the pool and
spawn a fresh one. The cost is one ~150 ms isolate spin-up per N
pages — negligible if N ≥ 100.

```rust
const RECYCLE_AFTER: usize = 500;

let mut pool = PagePool::new(4);
let mut count = 0usize;

loop {
    let url = rx.recv().await.unwrap_or_break();
    // ... navigate, extract ...
    count += 1;
    if count % RECYCLE_AFTER == 0 {
        drop(pool);
        // V8 isolate(s) freed here — calling thread reclaims the
        // reserved heap; OS will reap the resident pages on next
        // allocation pressure
        pool = PagePool::new(4);
        tracing::info!(count, "recycled pool");
    }
}
```

### Watchdog (belt-and-suspenders)

If the per-worker RSS exceeds a threshold, force-drop the oldest
pool isolate. Read `/proc/self/statm` periodically:

```rust
fn self_rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|s| s.split_whitespace().nth(1).map(|x| x.to_string()))
        .and_then(|s| s.parse::<u64>().ok())
        .map(|pages| pages as f64 * 4.0 / 1024.0)
        .unwrap_or(0.0)
}

// At top of worker loop, before each navigation:
if self_rss_mb() > 2000.0 {
    tracing::warn!(rss = self_rss_mb(), "RSS over threshold; recycling pool");
    drop(pool);
    pool = PagePool::new(4);
}
```

The `self_rss_mb()` helper is identical to
`crates/browser/examples/sweep_metrics.rs:73-83`.

### Long-lived process plan

Per `09_MEMORY_OPTIMIZATION.md §5` (HEAP_INITIAL = 1 GB) and §6
(DOM-arena retain), the v0.1.0 long-lived-process recipe is:

1. **Per-thread `PagePool`**, max_size 1-10
2. **Recycle after 500 navigations** (or after 1 hour, whichever first)
3. **RSS watchdog at 2× expected peak** — force-recycle if exceeded
4. **Process restart at memory limit + 25% headroom** — supervisor
   (k8s, systemd, supervisord) restarts the worker on
   OOM/threshold-cross; the queue redelivers the in-flight URLs

---

## 4. Concurrency model

The hard constraint from `CLAUDE.md`:

> "V8 isolates are per-thread. Running multi-threaded crashes the
> test process. CI enforces `--test-threads=1`."

This means within one OS process you can run many threads, **each with
its own V8 isolate(s)**, but you cannot send a `Page` or `PagePool`
between threads. Concretely:

```
process
├── thread A  →  tokio current_thread runtime  →  LocalSet
│                  └── PagePool (1-N isolates pinned to thread A)
├── thread B  →  tokio current_thread runtime  →  LocalSet
│                  └── PagePool (1-N isolates pinned to thread B)
└── thread C  →  job queue feeder (rx work, dispatch to A/B)
```

The job queue itself uses tokio mpsc (`Send`-able messages — URLs +
profile names). Each worker thread receives a URL, navigates within
its own thread-pinned `PagePool`, and sends the extracted HTML back
via another mpsc (also `Send` — strings).

`PagePool` itself uses `#[allow(clippy::arc_with_non_send_sync)]`
(per `crates/browser/src/pool.rs:14-18`) deliberately: the `Arc` is
intra-thread bookkeeping only. Do not try to clone+send.

### Spec — `crates/browser/examples/production_worker.rs`

Per §10 acceptance criteria, write a complete example. The shape:

```rust
//! Production worker pool — N OS threads, each pinned to its own
//! `tokio` current-thread runtime + `LocalSet` + `PagePool`. Jobs
//! arrive on `job_rx`, results go out on `result_tx`. Demonstrates:
//!   - the per-thread isolate constraint
//!   - cold-fallback after pool panic
//!   - periodic recycle (R-010 mitigation)
//!   - RSS watchdog
//!   - per-profile routing via `pick_first_profile()`

use browser::{Page, PagePool};
use std::sync::mpsc;
use std::thread;

fn worker_thread(id: usize, jobs: mpsc::Receiver<String>, results: mpsc::Sender<(String, String)>) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let mut pool = PagePool::new(4);
        let mut count = 0usize;
        while let Ok(url) = jobs.recv() {
            let profile = pick_first_profile(&url);
            let html = match pool.navigate(&url, profile.clone()).await {
                Ok(p) => { let b = p.content(); pool.release(p); b }
                Err(_) => {
                    pool = PagePool::new(4);  // R-001 — pool may be poisoned
                    match Page::navigate(&url, profile, 3).await {
                        Ok(p) => p.content(),
                        Err(e) => { eprintln!("[{id}] {url}: {e}"); continue; }
                    }
                }
            };
            count += 1;
            if count % 500 == 0 { drop(pool); pool = PagePool::new(4); }
            results.send((url, html)).ok();
        }
    });
}

fn main() {
    let num_workers = num_cpus::get();
    let (job_tx, job_rx) = mpsc::channel::<String>();
    let job_rx = std::sync::Arc::new(std::sync::Mutex::new(job_rx));
    let (result_tx, result_rx) = mpsc::channel::<(String, String)>();
    for id in 0..num_workers {
        let rx = job_rx.clone();
        let tx = result_tx.clone();
        thread::Builder::new()
            .name(format!("bo-worker-{id}"))
            .spawn(move || {
                // Convert shared rx into a per-thread Receiver via re-channeling,
                // OR use crossbeam-channel for native multi-receiver.
                // Spec-only; pick crossbeam in the real impl.
            })
            .unwrap();
    }
    // Feed URLs in...
    // Drain results...
}
```

---

## 5. Deployment recipes

### 5.1 k8s Deployment (long-running worker pool)

For continuous scraping with queue backpressure. Pair with KEDA +
RabbitMQ/SQS for autoscaling.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: browser-oxide-worker
spec:
  replicas: 4
  selector:
    matchLabels: { app: browser-oxide-worker }
  template:
    metadata:
      labels: { app: browser-oxide-worker }
    spec:
      containers:
      - name: worker
        image: ghcr.io/yfedoseev/browser_oxide:v0.1.0-parity
        args: ["worker", "--queue", "$(QUEUE_URL)", "--pool-size", "4"]
        env:
        - name: QUEUE_URL
          valueFrom:
            secretKeyRef: { name: queue, key: url }
        - name: RUST_LOG
          value: "info,browser=debug"
        - name: BROWSER_OXIDE_NAV_BUDGET_MS
          value: "20000"
        resources:
          requests: { memory: "800Mi", cpu: "1" }
          limits:   { memory: "2Gi",   cpu: "2" }
        livenessProbe:
          httpGet: { path: /healthz, port: 8080 }
          initialDelaySeconds: 30
          periodSeconds: 30
        readinessProbe:
          httpGet: { path: /readyz, port: 8080 }
          periodSeconds: 10
        ports:
        - containerPort: 8080
          name: metrics
---
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: browser-oxide-scaler
spec:
  scaleTargetRef: { name: browser-oxide-worker }
  minReplicaCount: 1
  maxReplicaCount: 50
  triggers:
  - type: aws-sqs-queue
    metadata:
      queueURL: "..."
      queueLength: "5"   # ~5 backlog per replica before scaling up
```

Healthz/readyz are NOT yet implemented in BO — per §10 acceptance,
this is a v0.1.0 deliverable. Recommended minimal stub:

- `/healthz` — 200 OK if the worker thread loop is alive (atomic bool
  flipped at start, cleared on shutdown)
- `/readyz` — 200 OK if the per-thread `PagePool` has at least 1 warm
  isolate ready (so k8s doesn't route to a worker that's still on
  first-isolate spin-up)
- Plus `/metrics` Prometheus exporter (§6)

### 5.2 AWS Lambda (one URL = one invocation)

See §1.4 for runtime/memory/timeout sizing. Handler skeleton is in
§1.4 too. Key Lambda-specific gotchas:

- **Cold start optimisation** — Lambda Provisioned Concurrency
  pre-spins the runtime; without it, each invocation pays a custom-
  runtime cold-start (~100-300 ms) + V8 isolate spin-up (~150 ms) +
  nav time. For latency-sensitive customers, provisioned concurrency
  is worth the cost.
- **No /tmp persistence** — Lambda's `/tmp` is wiped per invocation.
  Don't write profile files there hoping to reuse them.
- **15-min max** — the longest Lambda can run is 15 minutes, which is
  >> any sensible nav budget; don't worry about it.
- **VPC + NAT cost** — if you put Lambda in a VPC for a static egress
  IP, the NAT-Gateway egress charges add up. Most BO Lambda customers
  go VPC-less and accept variable Lambda IPs (most sites don't
  blocklist the AWS Lambda IP space wholesale, but a few do).

### 5.3 Cloud Run / Fargate (container per request)

See §1.5. Identical container image to k8s deployments; the difference
is just the runtime platform.

Per-request lifecycle: container starts → first request lands →
PagePool seeds (~150 ms) → navigate → return → container stays warm
for subsequent requests (`--max-concurrency` controls how many in
parallel) → idle-down after N seconds of no traffic.

For a request-response API (e.g. "give me a screenshot of URL X"),
Cloud Run is often the cleanest fit: it scales to zero when idle and
each cold-start is in-process (no Chromium subprocess), so even cold
requests respond in ~5-20 s.

### 5.4 Embedded library

See §1.6. The customer integrates BO into their own Rust binary
(agent loop, dev tool, internal scraper). Caveats:

- Pin to the v0.1.0-parity tag (or a specific commit), not `branch =
  "main"`, so upstream breakages don't auto-pull
- Use a `cargo update -p browser` cadence (monthly review minimum) to
  pick up security fixes
- If your application is multi-threaded, drive BO from one dedicated
  worker thread and route work through a channel (per §4)

---

## 6. Observability

### 6.1 Per-nav telemetry

The fields from `sweep_metrics`' `SiteResult` struct
(`crates/browser/examples/sweep_metrics.rs:32-42`) are the right
shape for production telemetry too:

| Field | Source | What it tells you |
|---|---|---|
| `url` | input | which URL |
| `profile` | input | which preset (chrome/pixel/iphone/firefox) |
| `tag` | classify.rs | L3-RENDERED / CHL / THIN-BODY / ERROR |
| `len` | `Page::content().len()` | response body size in bytes |
| `ms` | `Instant::now().elapsed()` | wall-time for the navigation |
| `rss_mb` | `/proc/self/statm` | RSS at end-of-nav |
| `err` | nav result | error message if any |

Emit one of these per navigation as a structured log line (JSON) or
push to a metrics endpoint.

### 6.2 Sweep-mode aggregate metrics

From `sweep_metrics`' `Summary` struct
(`crates/browser/examples/sweep_metrics.rs:44-65`):

- `pass`, `thin_shell`, `chl`, `thin_body`, `error` — verdict counts
- `pass_pct` — strict-pass rate
- `rss_peak_mb` — high-water mark
- `ms_median`, `ms_p95`, `ms_p99` — latency distribution
- `throughput_pages_per_min` — production rate
- `by_category: HashMap<String, CategoryStats>` — per-category pass
  rate (when you tag your URLs by category)

For production rollout dashboards, surface these per-worker per-15-min
window. The `wall_total_ms` and `t_first_page_ready_ms` fields give
you cold-start and steady-state windows.

### 6.3 Log levels via `RUST_LOG`

Per `tracing` crate conventions:

| Level | What's logged | When to use |
|---|---|---|
| `error` | unrecoverable errors only | quietest; production-default |
| `warn` | retries, fallbacks, classifier surprises | recommended production |
| `info` | per-nav summary (URL, profile, tag, ms) | initial rollout, debugging |
| `debug` | per-iter detail (nav loop iter count, drain timings) | gap investigation |
| `trace` | per-script execution, per-op | only when locally chasing a bug |

Specific env-var combinations from the codebase:

```bash
# Production default
RUST_LOG=warn,browser=info

# Initial rollout / debugging
RUST_LOG=info,browser=debug,js_runtime=info

# Deep debug for a specific gap site
RUST_LOG=trace,js_runtime=trace,dom=trace,browser=debug
```

Additional engine-specific knobs (from
`grep -rn "BROWSER_OXIDE" crates/`):

- `BROWSER_OXIDE_EVENT_LOOP_PROFILE=1` — per-iteration event-loop
  timings (`crates/event_loop/src/lib.rs:11-39`)
- `BROWSER_OXIDE_WARM_PROFILE=1` — warm-path step timings
  (`crates/browser/src/page.rs:1227`)
- `BROWSER_OXIDE_BUILD_PROFILE=1` — build_page step timings
  (`crates/browser/src/page.rs:2824`)
- `BROWSER_OXIDE_DEBUG_NAV=1` — navigation-loop trace
  (`crates/browser/src/page.rs:1037,1124`)
- `BROWSER_OXIDE_DD_TRACE=1` — DataDome handler trace
  (`crates/browser/src/page.rs:2908`)
- `BROWSER_OXIDE_SC_TRACE=1` — script-content trace
  (`crates/browser/src/page.rs:2916`)
- `BROWSER_OXIDE_CSP_BYPASS=1` — disable CSP enforcement on the
  rendered page (`crates/browser/src/page.rs:433,1296,1581`)
- `BROWSER_OXIDE_SWEEP_POOL=1` — `sweep_metrics` uses pool path
  (`crates/browser/examples/sweep_metrics.rs:101`)

---

## 7. Production hardening checklist

Per-host class tuning (do these once at deployment time):

- [ ] **Set `BROWSER_OXIDE_NAV_BUDGET_MS` per host class.** Defaults
      live in `crates/browser/src/page.rs:1697-1742` (Kasada 45 s,
      SPA 90 s, Akamai sec-cpt 45 s, Akamai BMP 25 s, default 15 s).
      For a custom workload, set the env var; per
      `crates/browser/src/page.rs:1743-1747` it overrides the
      defaults.
- [ ] **Set `BROWSER_OXIDE_NAV_BUDGET_EXTEND_MS` if needed.** Default
      25 s (`crates/browser/src/page.rs:1749-1754`).
- [ ] **Enable pool path with cold fallback.** Per §2.4.
- [ ] **Set memory watchdog.** Per §3. Threshold at 2× expected peak.
- [ ] **Configure profile routing per URL/category.** Per
      `11_PER_PROFILE_STRATEGY.md §4`.
- [ ] **Wire up logging + metrics.** Per §6. Prometheus exporter
      recommended for k8s; CloudWatch EMF for Lambda.
- [ ] **Set retry strategy: max 2 retries with profile rotation.** Per
      §2.5. No retry-forever loops.
- [ ] **Build with `cargo build --release --workspace`** per
      `CLAUDE.md`. Don't ship debug builds; perf gap is 5-10×.
- [ ] **Pin the BO version** in your Cargo.toml (git tag, not branch).
- [ ] **Set `RUST_LOG` appropriately** for the environment (`warn` in
      prod, `info` for initial rollout, `debug` for gap-debugging).
- [ ] **Ship a process supervisor** (k8s liveness probe, systemd
      Restart=always, supervisord) so that an OOM or panic auto-restarts.

---

## 8. Cost model (per 1M pages)

Assumes $0.10/hr equivalent compute (representative for AWS m6i.large
spot, GCP e2-standard-2, etc.). For exact pricing, plug your hourly
rate in:

```
cost_per_1M = (1_000_000 / pages_per_min) / 60 / parallel_workers * hourly_rate
```

From `12_COMPETITIVE_LANDSCAPE.md §4.2` (extended with k8s and Lambda
shapes):

| Shape | Throughput (pages/min) | Parallel workers per $0.10/hr | Cost per 1M pages |
|---|--:|--:|--:|
| BO pool path (single-process daemon) | 14.0 | 1 | $119 / worker; **$0.74 at 100 parallel** |
| BO cold path (k8s Job, one URL/Job) | 2.5 | 1 per pod | $667 / 1M (pod overhead amortized) |
| BO Lambda (1024 MB, 15 s typical) | — | — | ~$0.42 per 1M-invocation (compute) + data egress |
| Camoufox | 8.4 | 1 | $1.24 at 100 parallel |
| Patchright | 13.6 | 1 (5.7 GB / worker) | $12.25 (only 1 worker per 8 GB host) |
| Playwright | 12.6 | 1 (5.6 GB / worker) | $13.23 |
| Managed scraping APIs (BrightData / ScraperAPI / etc.) | — | — | $1-$10 per 1k = **$1,000-$10,000 per 1M** |

BO pool is **10-1000× cheaper than managed scraping services** for the
same page set. The tradeoff is: managed services include the proxy /
IP rotation / geo routing for you; BO does not.

Verification: the §10 acceptance criterion is to verify these cost
numbers against an actual customer deployment before tagging v0.1.0.

---

## 9. Files referenced

### Engine entrypoints

- `crates/browser/src/page.rs:200-214` — `Page` struct
- `crates/browser/src/page.rs:216-233` — `Drop for Page` (worker reap)
- `crates/browser/src/page.rs:823-848` — `Page::with_solvers`
- `crates/browser/src/page.rs:955-960` — `Page::navigate_with_solvers`
- `crates/browser/src/page.rs:1037,1124` — `BROWSER_OXIDE_DEBUG_NAV`
- `crates/browser/src/page.rs:1227` — `BROWSER_OXIDE_WARM_PROFILE`
- `crates/browser/src/page.rs:1697-1742` — per-host nav budget table
- `crates/browser/src/page.rs:1743-1754` — `BROWSER_OXIDE_NAV_BUDGET_MS`
  + `_EXTEND_MS` env knobs
- `crates/browser/src/page.rs:2824` — `BROWSER_OXIDE_BUILD_PROFILE`
- `crates/browser/src/page.rs:2908` — `BROWSER_OXIDE_DD_TRACE`
- `crates/browser/src/page.rs:2916` — `BROWSER_OXIDE_SC_TRACE`
- `crates/browser/src/page.rs:3385-3402` — build_page final 8 s drain
- `crates/browser/src/pool.rs:1-87` — `PagePool` API (full file)
- `crates/browser/src/pool.rs:14-18` — the `!Send/!Sync` clippy allow
  + rationale
- `crates/browser/src/challenge.rs:1-43` — `ChallengeSolver` trait doc
- `crates/browser/src/challenge.rs:103-170` — trait definition
- `crates/browser/src/classify.rs` — verdict classifier (Pass /
  ThinShell / CHL / ThinBody / Error)

### Stealth presets (per-profile constructors)

- `crates/stealth/src/presets.rs:120-196` — `chrome_148_macos`
- `crates/stealth/src/presets.rs:413-495` — `firefox_135_macos`
- `crates/stealth/src/presets.rs:672-772` — `pixel_9_pro_chrome_148`
- `crates/stealth/src/presets.rs:795-875` — `iphone_15_pro_safari_18`
- `crates/stealth/src/profile.rs:33-180` — `StealthProfile` schema

### Sweep + tooling

- `crates/browser/examples/sweep_metrics.rs:1-100` — sweep harness
  (the customer-style entry point)
- `crates/browser/examples/sweep_metrics.rs:32-65` — `SiteResult` +
  `Summary` structs (telemetry shape)
- `crates/browser/examples/sweep_metrics.rs:73-83` — `self_rss_mb()`
  (the RSS-watchdog reference impl)
- `crates/browser/examples/sweep_metrics.rs:101` — `BROWSER_OXIDE_SWEEP_POOL`
  toggle
- `crates/browser/tests/holistic_sweep.rs:1-700` — 126-site corpus
  definition

### Event loop / observability

- `crates/event_loop/src/lib.rs:11-39` — `BROWSER_OXIDE_EVENT_LOOP_PROFILE`
- `crates/event_loop/src/lib.rs:378` — `BROWSER_OXIDE_EVENT_LOOP_PROFILE_LABEL`

### Blocker (off by default, MPL-2.0)

- `crates/net/src/blocker.rs:1-115` — `BROWSER_OXIDE_BLOCKER` +
  `BROWSER_OXIDE_BLOCKER_RULES`
- `crates/net/Cargo.toml:12-49` — `blocker` feature gate + rationale

### CI / build policy

- `.github/workflows/ci.yml` — fmt / clippy -D warnings / test
  (single-threaded) / msrv 1.83 / deny check all
- `Cargo.toml:69-72` — release profile (opt-level=3, lto=thin,
  codegen-units=1)
- `deny.toml` — license + advisory policy

### Sibling chapters

- `00_README.md` — release plan overview
- `09_MEMORY_OPTIMIZATION.md` — RSS / heap / worker leak / pool retain
- `10_TIMING_OPTIMIZATION.md` — cold vs pool, drain, wellsfargo panic
- `11_PER_PROFILE_STRATEGY.md` — routing decision tree + per-profile
  pass/loss table
- `12_COMPETITIVE_LANDSCAPE.md` — Camoufox / Playwright / Patchright /
  PW+Stealth comparison + cost model
- `14_TESTING_VALIDATION.md` — regression gates + CI integration
- `15_OPEN_QUESTIONS.md` — Q3 SharedSession bleed, Q5 wellsfargo panic
- `24_RISK_REGISTER.md` — R-001 wellsfargo, R-010 DOM-arena retain,
  R-017 WAF variance retries
- `CLAUDE.md` — workspace conventions (per-thread V8, license rules,
  vendor solver scope)

---

## 10. Acceptance for v0.1.0

- [ ] **Production worker example exists** at
      `crates/browser/examples/production_worker.rs` — implements the
      §4 skeleton (per-thread runtime + LocalSet + PagePool + cold
      fallback + RSS watchdog + periodic recycle)
- [ ] **k8s Deployment YAML reference** at `deployment/k8s/` — at
      minimum `deployment.yaml`, `scaledobject.yaml` (KEDA), and a
      `Dockerfile` based on `FROM scratch` + the release binary
- [ ] **k8s Job YAML reference** at `deployment/k8s/job-scrape-one.yaml`
      — the one-URL-per-Job pattern from §1.3
- [ ] **Lambda runtime + handler skeleton** at
      `deployment/lambda/handler.rs` + `Cargo.toml` — buildable with
      `cargo lambda build --release`
- [ ] **Healthz/readyz endpoint stubs** in
      `crates/browser/examples/production_worker.rs` — minimal
      `axum` or `hyper` HTTP server on `:8080`; bound atomic-bool
      flipped on worker startup
- [ ] **Prometheus exporter stub** in the same example — `/metrics`
      endpoint emitting per-nav counters + the §6.2 aggregate fields
      as gauges
- [ ] **Cost model verified** against actual customer deployment —
      one real customer with > 100k pages scraped, real $ cost
      tracked, ratio vs §8 table within 1.5×
- [ ] **README.md customer section** updated with a "deploying to
      production" pointer to this doc
- [ ] **Dockerfile** at repo root or `deployment/Dockerfile` — `FROM
      scratch` (or `FROM gcr.io/distroless/cc-debian12` if you need
      glibc) + the statically-linked release binary; verified to
      produce a < 200 MB image
