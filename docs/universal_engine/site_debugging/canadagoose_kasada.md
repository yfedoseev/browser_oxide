# canadagoose.com — Kasada

**Status**: BLOCKED. Solver runs end-to-end but trust not upgraded.

**Engine**: Kasada KPSDK v3.

**Baseline response**: HTTP 429 (typical Kasada baseline rejection
pattern), body 701 bytes containing the Kasada interstitial scaffold:
a `<script src="/149e9513-01fa-4fb0-aad4-566afd725d1b/2d206a39-8ed7-
437e-a3be-862e0f06eea3/ips.js">` tag (the Kasada session script) and
some inline JS to bootstrap it.

## What the solver does

1. GET canadagoose.com — receives the 429 + interstitial.
2. `build_page_with_scripts` parses the HTML, finds the `<script
   src="ips.js">` tag, fetches and runs `ips.js`.
3. `ips.js` is the Kasada client SDK. It performs:
   - Self-integrity check via FNV-1a hash on its own source.
   - DOM and JS environment fingerprinting (canvas, audio, navigator,
     screen, plugins, performance.memory, etc.)
   - Computes a Proof-of-Work using the script-provided difficulty.
   - POSTs the result to `/{uuid}/{uuid}/tl` with `content-type:
     application/json`.
4. The `/tl` POST response (HTTP 200) returns three headers:
   - `x-kpsdk-cr: true` (proof-of-work accepted)
   - `x-kpsdk-ct: <base64 token>` (Kasada session token)
   - `x-kpsdk-st: <timestamp>` (token issued-at)
5. Our current solver extracts these headers from `__fetchLog` and
   forwards them as request headers on the retry. The retry GET still
   returns the 429 + interstitial.

The session log shows this clearly:

```
POST 200 /149e9513-01fa-4fb0-aad4-566afd725d1b/2d206a39-8ed7-437e-a3be-862e0f06eea3/tl
  x-kpsdk-st: 1775874545188
  x-kpsdk-r: 1-AQ
  x-kpsdk-cr: true
  x-kpsdk-ct: 03x0xcsb1tGIccwliFueHCemLhN9CJ2YWbWn7jUrZ8nKrh2eROVd4UkQctMybVSqn240e8o6j7wQJ6lH2hOkWvEJwpdZauEi41F2UVhpVHpn6kSf4kWFT6BZ4C5Rzjo7jVWA0th5Wt1fRXQwqewcYvL2tOVDzrR1rq7caA3dpjwbRR
POST 200 https://reporting.cdndex.io/error
KPSDK state: {"now":"fn","start":"9","scriptStart":"12"}
```

The KPSDK state of `{"now":"fn","start":"9","scriptStart":"12"}` is
`window.KPSDK` after ips.js runs. `now: "fn"` means the script accessed
`KPSDK.now` (function ref). `start: "9"` and `scriptStart: "12"` are
millisecond timestamps relative to script load. This is normal and
indicates ips.js executed cleanly.

## What's wrong

Despite getting `x-kpsdk-cr: true` (PoW accepted) and a valid session
token, the retry with the token in the header still returns 429. There
are several possibilities:

1. **Token must be sent as a cookie, not a header.** Kasada has multiple
   token transport modes. The `x-kpsdk-ct` header is the public API for
   manual integration but the session token may also need to land in a
   `KP_UIDz` or `KP_UIDz-ssn` cookie. Worth checking the response
   `Set-Cookie` headers from `/tl` and verifying our cookie jar picks
   them up.

2. **Token forwarding misses the IP-binding context.** Kasada's token is
   bound to (origin IP, TLS fingerprint, user-agent). If our retry uses
   a slightly different connection state (different HTTP/2 connection,
   different connection ID), the token is rejected. Real Chrome uses
   the same TLS session for both requests. We use the same TLS pool but
   maybe not the same exact session.

3. **Sensor VM expects a `location.reload()`-driven navigation.** In
   real Chrome, after ips.js finishes, it calls `location.reload()`. The
   same Document is replaced, the new GET is on the same TLS session,
   and the patched fetch (which ips.js installed before the reload)
   fires the request with all the right state. Our retry is a fresh
   `client.get()` that doesn't go through the patched fetch — so the
   request is missing whatever client-side state ips.js set up.

(3) is the most likely root cause and is exactly what the refactor in
`04_refactor_plan.md` fixes generically. After the refactor, the retry
should run the patched fetch via the script's own `location.reload()`,
which should produce a request that Kasada accepts.

## Other Kasada sites in the same boat

- `hyatt.com` — same shape (HTTP 429 baseline, 686-byte interstitial),
  same solver behavior, same blocked verdict. Verified Kasada KPSDK v3
  with the same session-token mechanism.
- `kick.com` — similar Kasada engine but the baseline is HTTP 200 (not
  429), and the solver used to work in earlier sessions. May or may
  not still work now; needs re-running.
- `homedepot.com` — primarily Akamai but has a Kasada layer too on some
  paths.

## What was confirmed by the captured ips.js

In a prior session we fetched and disassembled the Kasada ips.js (task
#53, completed). Findings:

- Heavy use of `Function.prototype.toString` for self-integrity
  (FNV-1a hash on the script body).
- Patches `XMLHttpRequest.prototype.send` and `window.fetch` to inject
  `x-kpsdk-ct`/`x-kpsdk-st`/`x-kpsdk-h` headers on all subsequent
  requests automatically.
- Computes a PoW using SHA-256 (we ship `op_crypto_digest` for this).
- Reads canvas via standard methods (`getContext`, `fillText`,
  `toDataURL`), unlike Akamai's adidas variant which doesn't extract
  pixels.
- Requires `Worker` to exist as a function (typeof check) — we have
  this since T1.5.

## What we've tried

- **T1.5 real Workers** — Kasada's Worker check passes (`typeof Worker
  === 'function'`). Doesn't change the verdict because Kasada doesn't
  actually spawn one in the captured variant.
- **Per-engine token forwarding via `solver_session_tokens`** — what
  the current code does. Gets accepted but the retry still 429s.
- **Reload-shape headers** (`sec-fetch-site: same-origin`, `referer:
  current_url`) on the retry — already in the code.
- **JS-level XHR retry inside the same V8 isolate** — already in the
  code. The XHR uses the patched fetch (since ips.js patched it), so
  it should carry the right session headers. But the response is
  still 429.

## What to try next

1. **Check if `/tl` POST sets a `Set-Cookie: KP_UIDz=...` response
   header.** If yes, verify our HttpClient parses and stores it. If
   it does, the retry should automatically include it via the cookie
   jar — no token forwarding needed.

2. **Implement the generic refactor** (`04_refactor_plan.md`) and see
   if Kasada starts working via `location.reload()` instead of token
   forwarding. The refactor specifically targets the architectural
   pattern Kasada relies on.

3. **Compare connection state** between the `/tl` POST and our retry.
   Are they on the same H2 stream? The same TCP connection? Kasada
   may bind the token to the connection ID.

4. **Get a Playwright capture** of canadagoose with the `/tl` POST and
   the subsequent successful navigation. Inspect the request headers
   on the post-`/tl` GET in real Chrome to see what's there that we
   don't send.

## Reproducibility

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
  -- --ignored --test-threads=1 --nocapture 2>&1 | grep -A 5 canadagoose
```

Or use the dedicated probe:

```bash
cargo test -p browser --test tier0_kasada \
  kasada_poc_canadagoose_full_browser -- --ignored --test-threads=1 \
  --nocapture
```
