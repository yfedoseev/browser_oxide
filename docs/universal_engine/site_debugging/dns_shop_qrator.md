# dns-shop.ru — QRATOR

**Status**: BLOCKED. The PoW solver is producing empty output.

**Engine**: QRATOR (Russian DDoS protection / bot management).

**Baseline response**: HTTP 403 (or interstitial page), body 6319-7472
bytes containing the QRATOR challenge JavaScript and the bootstrap to
solve it.

## What the solver does

1. GET dns-shop.ru — receives the challenge page.
2. `build_page_with_scripts` parses HTML and runs scripts.
3. The QRATOR challenge script POSTs to `/__qrator/validate?pow=
   <difficulty>&nonce=<solved>&qsessid=<session>` — but in our run,
   the POST is `/__qrator/validate?pow=168&nonce=&qsessid=` with
   **empty nonce and qsessid**.
4. Server returns 403 because nonce/qsessid are empty.
5. We retry, get 403 again.

## What's wrong

The QRATOR script provides a Proof-of-Work challenge with difficulty
`168`. The script is supposed to:

1. Compute the PoW (find a nonce such that some hash of `(challenge_id
   + nonce)` has 168 leading zero bits — actually unlikely to be a
   leading-zero PoW; QRATOR uses something custom).
2. Set `nonce=<computed>` and `qsessid=<assigned>` query params.
3. POST to `/__qrator/validate`.

Our script runs but emits empty values for both. Possibilities:

1. **PoW computation needs an API we don't implement.** QRATOR may use
   `crypto.subtle.digest` (we have it via `op_crypto_digest`),
   `WebAssembly` (we have it via V8), or something more obscure like
   `BigInt` arithmetic primitives we don't support.

2. **The challenge script never finishes executing.** It might depend
   on a `setTimeout` callback or async iteration that doesn't run in
   our event loop's drain window.

3. **The script catches an exception and silently continues with
   empty defaults.** Common pattern: `try { computePow() } catch {}`
   leaves nonce undefined; the POST sends `nonce=` (empty).

4. **The script reads `qsessid` from a cookie or HTTP header we don't
   set.** If QRATOR's bootstrap sets `qsessid` via `Set-Cookie` and
   our HttpClient doesn't store it (or strips it), the script reads
   it as undefined.

## What to investigate

1. **Capture the full QRATOR script** to /tmp:
   ```bash
   curl -s 'https://www.dns-shop.ru/' > /tmp/dns-shop-challenge.html
   ```
   Then extract the `<script>` content. Pretty-print and search for
   `qsessid`, `nonce`, `pow` to find the computation site.

2. **Run the script under our API probe instrumentation** (the
   pattern in `crates/browser/tests/adidas_sensor_api_probes.rs`).
   Wrap `globalThis.crypto.subtle`, `BigInt`, `setTimeout`,
   `Promise.resolve`, `JSON.stringify` and see which the script
   uses. Whatever returns a wrong value or throws is the gap.

3. **Enable script error tracking** in the page bootstrap. The
   `__scriptErrors` mechanism in page.rs already captures uncaught
   errors. If QRATOR's PoW throws a `ReferenceError` or
   `TypeError`, we'd see it.

4. **Check the response headers and cookies on the initial GET**.
   Print every Set-Cookie header. Look for `qsessid` or similar.

## Why this might be tractable

- QRATOR is much smaller than Akamai/Kasada — single-file JS, no
  multi-stage pipeline.
- The empty nonce/qsessid is a clear, specific failure mode (not a
  fingerprint mismatch).
- If we can identify which API the script needs, the fix is usually
  to implement that API in a generic way.

## What to try next

1. Capture and read the QRATOR challenge script statically. ~1 hour.
2. Identify the missing capability via instrumentation. ~1-2 hours.
3. Implement the missing capability if it's small (e.g., a missing
   JS API) or a generic primitive. ~1-3 hours.
4. Re-run the probe and verify nonce/qsessid are non-empty. ~10
   minutes.

Total estimated effort: **4-8 hours** for the most likely scenarios.
This is the cheapest of the 8 blockers to investigate.

## Reproducibility

```bash
cargo test -p browser --test debug_blocked debug_dns_shop -- \
  --ignored --test-threads=1 --nocapture
```

## Related tasks

- #13 Debug probe dns-shop.ru [done — the initial probe is what
  identified the empty-nonce issue]
- #14 Pass QRATOR challenge on dns-shop.ru [pending]
