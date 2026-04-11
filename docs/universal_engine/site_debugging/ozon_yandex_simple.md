# ozon.ru and ya.ru — likely fixable with redirect-following

These two are bundled because both look like they're not actually
bot-blocked — we're just not handling something simple correctly.

## ozon.ru — 307 Temporary Redirect loop

**Status**: BLOCKED but it's not a real bot block.

**Engine**: None visible. Uses HTTP 307 with `__rr` query parameter
for round-robin server selection.

**Baseline response**: HTTP 307, body 156-164 bytes:

```html
<html>
<head><title>307 Temporary Redirect</title></head>
<body>
<center><h1>307 Temporary Redirect</h1></center>
<hr><center>nginx</center>
</body>
</html>
```

With response headers:
```
location: https://www.ozon.ru/?__rr=2
content-length: 164
```

The `__rr` query param increments each time. ozon's load balancer is
asking us to follow the redirect to land on a specific server. Real
browsers follow this automatically.

## What's wrong

Our `Page::navigate_with_challenges` calls `client.get(url)` which
does NOT follow redirects. So we receive the 307, see no real content,
and return the small body.

## The fix

Use `client.get_follow(url, 10)` instead of `client.get(url)` in
`navigate_with_challenges`. This is part of the refactor in
`04_refactor_plan.md` — the new generic `navigate(url, max_iterations)`
function will use `get_follow` and ozon should immediately start
working.

**Quick check** — to verify ozon works under follow:

```bash
cargo test -p browser --test debug_blocked debug_ozon_rr1 -- \
  --ignored --test-threads=1 --nocapture
```

If you change `client.get` to `client.get_follow(url, 10)` in the
debug probe and re-run, you should see a real ozon home page in the
response body.

**Estimated effort to fix ozon**: 5 minutes (one line change in
`page.rs`) once the refactor is in.

## ya.ru — inconsistent results, possibly broken markers

**Status**: BLOCKED in the regression probe but probably actually
passing.

**Engine**: Yandex SmartCaptcha (sometimes) or no challenge (most
of the time).

**Baseline response**: 0 bytes (empty body) on a fresh GET.

**Solver response**: highly variable. In one session run we got
**488,356 bytes** of real content (a full Yandex search page). In
other runs we got 39 bytes. The variance is suspicious.

## What's wrong

Two issues:

1. **Empty baseline GET**. ya.ru responds with an empty body to a raw
   GET that doesn't look like a real browser. Not a bot block per se;
   they just gate content on having the right Accept headers / fetch
   metadata. Real browsers send headers we don't.

2. **Probe markers don't match**. When the solver returns 488 KB of
   real content, the probe's positive markers (`['ya.ru', 'yandex']`)
   don't match the actual page text. Our probe says INTR even though
   the content is real.

## What to try next

1. **Fix the probe markers**. Look at the actual 488 KB body and
   identify a definitive marker. Cyrillic strings like `Яндекс` or
   `поиск` might appear. Or HTML structure markers like `<html
   data-platform="desktop"`.

2. **Investigate the empty-baseline case**. Capture the request
   headers and compare against a real Chrome request to ya.ru. Find
   the specific header that causes the response to be empty vs
   non-empty.

3. **Use `get_follow`** (same as ozon — same fix).

**Estimated effort**: 30 minutes for marker fix + 1-2 hours for
header investigation if needed.

## Reproducibility

```bash
cargo test -p browser --test debug_blocked debug_yandex -- \
  --ignored --test-threads=1 --nocapture
```

## These are the cheapest wins

If the next contributor wants to put a quick win on the board, ozon
and yandex are the targets. ozon requires literally one line change
(use `get_follow`); yandex requires fixing the probe markers and
maybe one header tweak. Together they could move us from 22/24
deep-path-passing to 24/24 plus add 2 more sites to the L3 PASS set.
