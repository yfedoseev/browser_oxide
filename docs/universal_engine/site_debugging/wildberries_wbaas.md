# wildberries.ru — WBAAS (Wildberries Anti-Abuse Service)

**Status**: BLOCKED, but the closest of any of the 8 blockers.

**Engine**: WBAAS — Wildberries' in-house anti-bot. Custom challenge
solver written in JS, distributed under
`/__wbaas/challenges/antibot/statics/challenge_solver_v1.0.4.js` and
`/__wbaas/challenges/antibot/statics/challenge_fingerprint_v1.0.23.js`.

**Baseline response**: HTTP 498 (custom code), body 1447 bytes
containing the challenge bootstrap. The `498` status with header
`status-no-id: PG-03-DM` is the WBAAS challenge signature. (Note: the
IP has recently been seen returning `connection closed before headers`
instead of 498 — likely rate limited from this machine's egress.)

## What the solver does

1. GET wildberries.ru — receives 498 + 1447-byte challenge bootstrap.
2. `build_page_with_scripts` parses HTML, fetches and runs the inline
   bootstrap script.
3. The bootstrap script POSTs to
   `/__wbaas/challenges/antibot/api/v1/find-frontend-settings` and
   receives a settings object indicating which solver version to use.
4. Bootstrap fetches `challenge_solver_v1.0.4.js` and runs it.
5. Solver_v1.0.4 POSTs to `api/v1/create-token` — first attempt
   returns 498 (rejected, expected — needs the fingerprint solver
   too).
6. Bootstrap fetches `challenge_fingerprint_v1.0.23.js` and runs it.
7. Solver POSTs to `api/v1/create-token` again — **this time returns
   200** (accepted!).
8. Solver writes `x_wbaas_token=...` cookie via `document.cookie = ...`
   from JS.
9. Solver triggers a navigation (probably `location.reload()` or
   `location.href = ...`).
10. Our retry GET wildberries.ru — receives 498 again.

The key indicator that we're close: **the second `create-token` POST
returns 200**. WBAAS has accepted our solved fingerprint. The only
remaining issue is that the post-solve navigation isn't carrying the
token properly.

## What's wrong

The `x_wbaas_token` cookie was set via `document.cookie = ...` from
JS. Per task #8 (completed), our `document.cookie` setter should
propagate to the `net::HttpClient` cookie jar via the `op_cookie_set`
op. But the retry GET still returns 498, which means either:

1. The cookie isn't actually in the jar at retry time.
2. The cookie IS in the jar but WBAAS expects additional headers we
   don't send.
3. The token has a binding we're not satisfying (TLS session ID,
   client-side challenge result hash).

This is **task #10** — pending — labeled "WB retry GET accepted with
x_wbaas_token". Our session #10 was in progress before the rate-limit
issue obscured it.

## What was confirmed in this session

We added a `document.cookie` instrumentation in page.rs that captures
every cookie write during the solver run. For wildberries it printed:

```
[ 2] x_wbaas_token=1.1000.c1e8b5aae6324eb8b2a9f5f06ad4a1a8.MTAwfDIwMDE6
NTY5OjcyOGM6ZjYwMDoyMTY6M2VmZjpmZWVmOjhjZjN8TW96aWxsYS81LjAgKE1hY2ludG9zaD
```

So the cookie IS being set client-side. Whether it's in the jar at
retry time is the question.

## Recent observation: rate-limited

In the most recent run, our diagnostic (`wildberries_solver_diag.rs`)
got `Http("connection closed before headers")` on the very first GET
attempt. That's WBAAS's "you've been hammering us" response — the TLS
handshake completes but the server closes the connection before
sending any HTTP headers. It happens because of our repeated test
runs from this IP.

This is recoverable: wait 10-30 minutes and the rate limit lifts. But
it makes debugging slow because you can't iterate quickly.

## What to try next

1. **Print the cookies in the jar RIGHT before the retry GET** in
   page.rs (we added this for the JS retry but not for the Rust
   attempt-1 retry). Verify `x_wbaas_token` is there. If yes,
   problem is server-side (token binding); if no, problem is the
   cookie propagation path from `document.cookie` to the HttpClient
   jar — task #8 may not be complete after all.

2. **Try a fresh IP** to bypass the rate limit and run the diagnostic
   cleanly. Same task as #72 (clean IP for adidas Chrome reference)
   — solves both problems.

3. **Implement the generic refactor** in `04_refactor_plan.md`. The
   real `location.reload()` that the script is probably calling will
   trigger a reload through the cookie jar (assuming task #8 is
   correct). If WBAAS works after the refactor, the per-engine code
   was unnecessary — which is the architectural goal.

4. **Reverse-engineer `challenge_fingerprint_v1.0.23.js`** (task #21,
   pending). Understand what fingerprint values it sends. If our
   values are wrong but happen to be accepted on the second create-
   token POST, we may be hitting a soft check that lets us through
   for token issuance but not for the final navigation.

## What we know about challenge_fingerprint_v1.0.23.js

Not much. Task #21 (pending) is to RE the file. Quick observations:

- It's named with a version suffix that bumps occasionally
  (`v1.0.23` as of the most recent capture). They have at least 23
  iterations of this file.
- It's loaded as a regular script (not a module), runs synchronously.
- It POSTs to the same `create-token` endpoint as `challenge_solver_v1.
  0.4.js` but with additional fingerprint payload.
- We don't know the payload format. It's probably a JSON object with
  keys like `screen`, `nav`, `canvas`, `audio`, `webgl`, `fonts`,
  `timing`. Whatever values pass make WBAAS issue a 200 on the second
  POST.

If you tackle task #21, try:
```bash
curl -s 'https://www.wildberries.ru/__wbaas/challenges/antibot/statics/challenge_fingerprint_v1.0.23.js' > /tmp/wbaas_fp.js
wc -l /tmp/wbaas_fp.js
# Pretty-print:
node -e "console.log(require('prettier').format(require('fs').readFileSync('/tmp/wbaas_fp.js','utf8'),{parser:'babel'}))" > /tmp/wbaas_fp.pretty.js
```

## Reproducibility

```bash
cargo test -p browser --test wildberries_solver_diag -- --ignored \
  --test-threads=1 --nocapture
```

Or via the broader probe:

```bash
cargo test -p browser --test blocker_rigorous_probe tier05_blockers_all \
  -- --ignored --test-threads=1 --nocapture 2>&1 | grep -A 10 wildberries
```

Expect either `INTR (1915b)` (solver runs to completion) or `ERR (0b)`
(rate-limited / connection closed).

## Why WB is the highest-ROI of the 8 blockers

- **Closest to passing**: the `create-token` POST returns 200 after
  the fingerprint solver runs. Only the final navigation is missing.
- **Engine is least defended**: WBAAS is in-house Russian custom code,
  much smaller surface than Akamai BMP v3 or Kasada. If we
  reverse-engineer it, the knowledge is durable (Akamai's sensor VM
  is regenerated daily; WBAAS only bumps version numbers).
- **No public commercial solver**: WBAAS isn't covered by Hyper
  Solutions or RiskByPass — there's no SaaS shortcut. Whoever solves
  it first has a distinguishable capability.
- **Russian sites cluster**: Solving WBAAS may give us a template for
  similar Russian engines (QRATOR on dns-shop, DDoS-Guard on ozon).

## Related tasks

- #2 Confirm WB solver uses inline script injection [done]
- #6 Re-run WB challenge benchmark and verify [done]
- #10 WB retry GET accepted with x_wbaas_token [pending]
- #15-20 Step 0-5 of WB validation [done]
- #21 Reverse-engineer WBAAS challenge_fingerprint_v1.0.23.js [pending]
