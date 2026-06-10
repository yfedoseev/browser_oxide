# Guide: debugging renders

When a page comes back thin, empty, or blocked, these are the tools to find out
why. All are real examples in `crates/browser_oxide/examples/`.

## Is it a thin/shell render? — `thin_probe`

Navigates with the production `Page::navigate` path, then dumps *why* a React/Vue
SPA mounted a shell and stopped: captured script errors / unhandled rejections
(`window.__scriptErrors`), mount-point child counts, `document.readyState`,
pending-resource hints, and a tail sample of `document.body`.

```bash
cargo run --release -p browser_oxide --example thin_probe -- https://example.com chrome_148_macos
# run a probe expression after nav and dump the body:
PROBE_JS='(async()=>document.querySelectorAll("*").length)()' \
DUMP_BODY=/tmp/body.html \
  cargo run --release -p browser_oxide --example thin_probe -- <url>
```

Most "blank SPA" bugs are a single JS exception thrown during hydration that
trips the framework's error boundary — `__scriptErrors` is where you'll see it.

## Is it the network, before any JS? — `fetch_probe`

Raw HTTP through the stealth `net` stack (BoringSSL TLS + H2): status, final URL
after redirects, key headers, body length. Use it to separate a transport/TLS
problem from a render problem.

```bash
cargo run --release -p browser_oxide --example fetch_probe -- https://example.com
```

## In-code introspection

```rust
let mut page = Page::navigate(url, profile, 5).await?;

// drain console.log/warn/error the page emitted
page.consume_and_print_logs();

// inspect anything in the realm
let n   = page.evaluate("document.querySelectorAll('*').length")?;
let err = page.evaluate("JSON.stringify(window.__scriptErrors || [])")?;
let rs  = page.evaluate("document.readyState")?;

// honest outcome
println!("{}", page.challenge_verdict().as_str());
```

## Other probes

| Example | Purpose |
|---|---|
| `parse_probe` | parse-only (no JS) — isolate parser/DOM issues |
| `classify_stdin` | pipe HTML in, get the `engine_classify` verdict |
| `ja4_capture` | capture the emitted TLS JA4 fingerprint |
| `nav_timed` | per-phase navigation timing (fetch / bootstrap / drain) |
| `sweep_metrics` | run the corpus, emit per-site JSON (tag/len/ms/rss) |

## Reading the verdict tags

`PASS` (real) · `THIN_BODY` / `THIN_SHELL` (rendered but small) · `EDGE_BLOCK`
(interstitial) · `SENSOR_FAIL` (scored bot) · `CHL_INCOMPLETE` (challenge never
cleared). See [CHALLENGES.md](CHALLENGES.md) for the mapping to `ChallengeVerdict`.

> Scoring tip: a single `L3-RENDERED` *tag* is not a pass on its own — a 13 KB SPA
> shell tags `L3-RENDERED` too. The honest gate is **tag `L3-RENDERED` AND body
> ≥ 15 KB**. This is exactly how [../BENCHMARK.md](../BENCHMARK.md) scores.
