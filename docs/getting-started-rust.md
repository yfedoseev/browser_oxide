# Getting started (Rust)

`browser_oxide` is a **stealth headless browser engine** — a real HTML/CSS/DOM/JS
browser built from the parser up in Rust, with its own BoringSSL TLS stack and a
native fingerprint. No Chromium, no CDP driver. You drive it as a library: give it
a URL and a browser identity, get back a rendered page you can read and script.

## 1. Add the dependency

The crates are **not yet published to crates.io** (a rename pass is pending — see
the repo roadmap). Until then, depend on the workspace via git or a local path:

```toml
# Cargo.toml
[dependencies]
browser = { git = "https://github.com/<owner>/browser_oxide" }   # the top-level API
stealth = { git = "https://github.com/<owner>/browser_oxide" }   # browser profiles
tokio   = { version = "1", features = ["rt", "macros"] }
```

> Heads-up: V8 (via `deno_core`) downloads a ~130 MB prebuilt binary on first
> build. The first `cargo build` is slow; subsequent builds are cached.

## 2. The one rule that shapes all usage: the engine is `!Send`

V8 isolates are per-thread. `Page`, `PagePool`, and the event loop **cannot move
across threads**. So you run the engine on a **current-thread** runtime with a
`LocalSet` — not a naive `#[tokio::main]` (which is multi-thread by default):

```rust
use browser::{ChallengeVerdict, Page};

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all().build().unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&rt, async {
        let profile = stealth::presets::chrome_148_macos();
        let mut page = Page::navigate("https://example.com", profile, 5).await.unwrap();

        println!("{}", page.title());          // document.title
        println!("{} bytes", page.content().len());   // outerHTML
        println!("{}", page.evaluate("navigator.userAgent").unwrap());

        match page.challenge_verdict() {
            ChallengeVerdict::Pass => println!("real content"),
            v if v.is_challenge() => println!("blocked: {}", v.as_str()),
            v => println!("{}", v.as_str()),   // thin shell / render-incomplete
        }
    });
}
```

A complete, compiling version lives at
[`crates/browser/examples/getting_started.rs`](../crates/browser/examples/getting_started.rs):

```bash
cargo run --release -p browser --example getting_started -- https://example.com
```

## 3. Navigating

```rust
// signature: takes the profile by value; max_iterations bounds the
// redirect / challenge-retry loop (5 is a good default).
Page::navigate(url: &str, profile: StealthProfile, max_iterations: u8)
    -> Result<Page, AnyError>
```

- `Page::navigate` is **humanized by default** (synthetic mouse/scroll/key
  signals — sensor-based detectors score "zero input in the first 2 s" as a bot
  tell). `Page::navigate_pure` is the non-humanized variant for deterministic
  tests. `navigate_humanized` is a back-compat alias of `navigate`.
- `Page::from_html(html, Some(profile))` builds a page from a string (no network)
  — handy for tests and rendering local HTML.

## 4. Reading the page

| Method | Returns |
|---|---|
| `page.title()` | `document.title` |
| `page.content()` | `document.documentElement.outerHTML` |
| `page.text_content()` | `document.body.textContent` |
| `page.text_of(selector)` | `querySelector(sel)?.textContent` (`Option<String>`) |
| `page.has_element(selector)` | `bool` |
| `page.url()` | current URL (after redirects) |
| `page.evaluate(js)` | run JS in the page realm → `Result<String, _>` |

## 5. Did it actually work? — `challenge_verdict()`

Anti-bot pages return HTTP 200 with a challenge body, so status codes lie. Use the
engine's own classifier:

```rust
match page.challenge_verdict() {
    ChallengeVerdict::Pass            => {/* real content rendered */}
    ChallengeVerdict::ThinShell       => {/* rendered, but a small SPA shell */}
    ChallengeVerdict::RenderIncomplete=> {/* no markers, thin/empty body */}
    ChallengeVerdict::EdgeBlock       => {/* interstitial / deny page */}
    ChallengeVerdict::SensorFail      => {/* vendor JS scored us as a bot */}
    ChallengeVerdict::ChallengeIncomplete => {/* challenge ran, never cleared */}
}
// verdict.is_challenge() == true for EdgeBlock | SensorFail | ChallengeIncomplete
// verdict.as_str() gives a stable lowercase tag for logging
```

See [guides/CHALLENGES.md](guides/CHALLENGES.md) for the full semantics and the
pluggable `ChallengeSolver` extension point.

## 6. Reusing isolates: `PagePool`

Cold `Page::navigate` pays V8 isolate + snapshot setup each time. A warm pool
amortizes it (~150 ms saved per navigation):

```rust
let pool = browser::PagePool::new(4);
let mut page = pool.navigate("https://example.com", profile).await?;
let html = page.content();
pool.release(page);   // return the isolate to the pool for reuse
```

> **Warm-path caveat:** the pool skips the cold challenge-follow loop, so a JS
> *interstitial* site can warm-render a thin shell where `Page::navigate` would
> follow the challenge to the real page. **For challenged/protected targets, use
> the cold `Page::navigate`.** The pool is for high-throughput benign pages.

## 7. Picking a browser identity

`stealth::presets::*` ship 12 ready profiles (desktop + mobile), or load your own
from YAML/JSON. The profile you pick changes TLS fingerprint, headers, navigator,
GPU, canvas/audio seeds — everything. See [guides/PROFILES.md](guides/PROFILES.md).

## Next

- [guides/PROFILES.md](guides/PROFILES.md) — choosing & customizing identities
- [guides/CHALLENGES.md](guides/CHALLENGES.md) — verdicts & custom solvers
- [guides/STEALTH_FAQ.md](guides/STEALTH_FAQ.md) — what's native vs. not
- [guides/DEBUGGING.md](guides/DEBUGGING.md) — thin renders, fetch logs, JS errors
- [guides/CDP.md](guides/CDP.md) — Puppeteer/Playwright drop-in
- [BENCHMARK.md](BENCHMARK.md) — measured anti-bot pass rates
