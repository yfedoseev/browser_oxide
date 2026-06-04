# Guide: challenge verdicts & solvers

Anti-bot endpoints answer with HTTP 200 and a *challenge* body, so status codes
don't tell you whether you got real content. BrowserOxide classifies the
result for you, and exposes a pluggable hook for custom challenge handling.

## The verdict

```rust
let mut page = Page::navigate(url, profile, 5).await?;
let v = page.challenge_verdict();   // browser::ChallengeVerdict
```

| Verdict | `is_challenge()` | Meaning |
|---|:--:|---|
| `Pass` | no | Real content rendered, no challenge markers |
| `ThinShell` | no | Rendered, but body is below the real-content floor (SPA pre-hydration stub) |
| `RenderIncomplete` | no | No markers, but a thin/empty body (e.g. redirect/captcha shell) |
| `EdgeBlock` | **yes** | Challenge marker + small body — an interstitial/deny page served before our JS earned trust |
| `SensorFail` | **yes** | Challenge marker + large body — the vendor JS ran and scored our telemetry as bot |
| `ChallengeIncomplete` | **yes** | A challenge is structurally present but its flow never completed (no clearance token) |

`v.as_str()` gives a stable lowercase tag for logs; `v.is_challenge()` is true for
the three challenge classes.

The same logic is available without a `Page` via the classifier:

```rust
let res = browser::engine_classify(&html);   // -> { verdict, tag, len }
```

## What the open-source engine does (and doesn't) clear

The engine renders through many protections **from the from-scratch TLS +
fingerprint + V8 stack alone** — most Cloudflare, Akamai, AWS WAF, and a chunk of
DataDome. It ships **no per-vendor bypass code**: `Page::navigate` registers an
empty solver set, so a hard challenge resolves to `ChallengeIncomplete` rather
than being auto-cleared. Per-vendor solving is deliberately out of scope (see
[../../SCOPE.md](../../SCOPE.md)). Measured pass rates: [../BENCHMARK.md](../BENCHMARK.md).

## Extension point: `ChallengeSolver`

Embedders can plug in their own per-vendor handling. The engine dispatches
registered solvers during navigation:

```rust
use browser::{ChallengeKind, ChallengeSolver, SolveOutcome, Page};

struct MySolver;

#[async_trait::async_trait(?Send)]   // !Send: runs on the engine thread
impl ChallengeSolver for MySolver {
    fn name(&self) -> &'static str { "my-vendor" }

    // Identify the challenge from the response/body.
    fn detect(&self, resp: &net::Response, html: &str) -> Option<ChallengeKind> {
        html.contains("my_marker").then(|| ChallengeKind::new("my-vendor", "type-a"))
    }

    // Do the work; return Solved to make the engine refetch the URL.
    async fn solve(&self, page: &mut Page, client: &net::HttpClient, kind: ChallengeKind)
        -> SolveOutcome
    {
        // ... acquire a clearance cookie / run the PoW / round-trip the interstitial ...
        SolveOutcome::Solved
    }
}

let solvers: std::sync::Arc<[std::sync::Arc<dyn ChallengeSolver>]> =
    std::sync::Arc::from(vec![std::sync::Arc::new(MySolver) as _]);
let mut page = Page::navigate_with_solvers(url, profile, 5, solvers).await?;
```

`SolveOutcome`: `NotApplicable` (not this vendor) · `InProgress` (keep iterating)
· `Solved` (cleared → refetch) · `Unsolvable` (surface `ChallengeIncomplete`).

This is the supported way to add bypass logic **in your own crate** without
forking the engine — the public tree stays vendor-neutral.
