# Post gap-analysis fixes — 2026-04-28

> Implementation diary for the fixes recommended by `docs/GAP_DEEP_ANALYSIS_2026_04_28.md`.
> **Honest result**: H2 byte-precision fix landed cleanly but did NOT flip any sites in
> a single re-run. The Akamai discriminator is deeper than WINDOW_UPDATE alone. Documenting
> exactly what happened so the next session has accurate ground truth.

---

## Result vs prediction

| Fix | Predicted unlock | Actual unlock |
|---|---:|---:|
| Fix 1 — H2 byte-precision | +9 Akamai sites | **0** |
| Fix 2 — mail-ru redirect trace | +1 | 0 (debug tracing landed; real fix needs more work) |
| Fix 3 — captcha-CHL trio body length | +0 to +3 | 0 (sites are SPA-shell renders, not classifier issue) |

Holistic sweep before / after: **98 / 126** in both runs (identical distribution).

This is **honest negative data** — important to record so the next session doesn't repeat the bet.

---

## What shipped

### Fix 1: H2 INITIAL_CONNECTION_WINDOW_SIZE byte-precision

**File**: `crates/net/src/h2_client.rs:34`

```rust
// Was 15_728_640 (15 MB even); now matches Chrome 147 exactly.
const INITIAL_CONNECTION_WINDOW_SIZE: u32 = 15_663_105;
```

This **is** correct — Chrome 147 emits `WINDOW_UPDATE = 15663105` after handshake, not the round 15 MB. The change is byte-coherent and shouldn't be reverted; it's strictly closer to Chrome's signature. **It just isn't sufficient by itself** to flip the 9 Akamai-CHL sites we hoped it would.

The stream-priority weight (255 wire byte = weight 256 per RFC 7540 §5.3) was reverted after I realized the wire encoding — we were already correct.

### Fix 2 (partial): `BOXIDE_DEBUG_REDIRECTS` tracing

**File**: `crates/net/src/lib.rs::get_follow`

Env-var-gated hop-by-hop trace. Useful diagnostic; not a fix itself.

```bash
BOXIDE_DEBUG_REDIRECTS=1 cargo test --release -p browser --test holistic_sweep \
    -- --ignored --test-threads=1 --nocapture h_ru_mail
```

Output for mail.ru:
```
[redirect] hop=0 GET https://mail.ru/
[redirect]   <- status=302 body_len=37 location="https://login.vk.ru/?act=autologin&app_id=7539952..." set-cookies=[]
[redirect] hop=1 GET https://login.vk.ru/?act=autologin&...
[redirect]   <- status=302 body_len=0 location="https://account.mail.ru/login?errorCode=11300&errorText=invalid+user&..." set-cookies=[]
[redirect] hop=2 GET https://account.mail.ru/login?...
[redirect]   <- status=200 body_len=936 location=None set-cookies=[]
```

Key finding: **mail.ru's autologin chain rejects us with `errorCode=11300&errorText=invalid+user`** — we end up at the login page, not the homepage. Patchright/pwstealth somehow get to the actual content.

Hypothesis: mail.ru's edge serves a different response based on TLS class (same root cause as Akamai). Or our cookie jar isn't seeing Set-Cookie from one of the hops (our trace shows `set-cookies=[]` on every hop; expected to see jar updates).

A second trace pass with the corrected `resp.set_cookies` field (vs `resp.headers.get("set-cookie")`) is in the diff but not yet re-run.

---

## Why Fix 1 alone didn't move the needle

Akamai BMP v3's edge fingerprint is computed from **many** signals; the WINDOW_UPDATE value is just one of them. Real Chrome 147's full Akamai-FP signature includes:

1. TLS ClientHello — cipher list order, extension order, supported_groups, etc.
2. ALPN values + order
3. HTTP/2 SETTINGS — values, **and order they're sent on the wire**
4. WINDOW_UPDATE (we now match)
5. PRIORITY frame on the implicit headers stream
6. Headers pseudo-order (`:method, :authority, :scheme, :path` — we match)

We match 4-5 out of 6 categories. The remaining mismatches are subtle but Akamai-FP is a hash — any single byte off → different bucket → `_abck=~~-1~...`.

**Without a tls.peet.ws-class diagnostic endpoint** that exposes Akamai's actual hash for our request, we are debugging blind. The next concrete step is **add a self-test that hits tls.peet.ws/api/all from browser_oxide and dumps the `akamai_fp` field**, then byte-bisect against Chrome's reference hash `52d84b11737d980aef856699f885ca86`.

---

## Updated estimate of post-Akamai-fix unlock

Was: "Fix 1 alone unlocks +9 Akamai sites" → **revised to**: "Akamai bypass requires several coordinated fixes":
1. H2 WINDOW_UPDATE precision (✓ shipped)
2. tls.peet.ws self-test endpoint to identify the actual mismatched field
3. Whatever the diagnostic surfaces — likely TLS extension order or one of the other signals

Effort: 1-2 d to build the diagnostic + iterate. Not the 30-minute fix originally hypothesized.

---

## What's still actionable for cheap

1. **Captcha-CHL trio (duolingo/substack/spotify) is a render-completeness issue, not stealth**. Bodies are 9-78 KB (SPA shells). Patchright's renderer fills them to 200KB-1MB. Possible quick fixes:
   - Increase `BOXIDE_NAV_BUDGET_MS` for these specific sites (or detect SPA shell pattern and extend budget)
   - Investigate why our V8+DOM doesn't render their JS to fuller content
   
   Effort: 4-8 h investigation, not yet attempted.

2. **mail-ru cookie carry**: re-run trace with corrected `resp.set_cookies` field. If still empty, the issue is downstream (our HTTP layer not parsing Set-Cookie from this server's specific response shape). 2-4 h.

3. **G.1 Akamai sensor solver** (already roadmapped) — bypasses the edge fingerprint entirely by solving the BMP challenge JS. Big effort but unblocks 9 sites independently of TLS work.

---

## Conclusions

- **Fix 1 shipped**: a strictly-correct byte-coherency improvement that doesn't move the needle on its own. Good engineering hygiene; not a PR for headlines.
- **The H2 fingerprint hypothesis was incomplete**: matching one byte value isn't enough; we need to match the full chain.
- **Path forward**: build the tls.peet.ws diagnostic, then iterate; OR commit to G.1 Akamai sensor as the bypass (avoids the fingerprint-precision arms race).
- **PASS count remains at 98 / 126** post-fix — same as Phase F.

Updated SOTA position: still leading on stealth (98 vs Camoufox's 51, Patchright's 92, nodriver's 90, pwstealth's 91), still 7.9 min wall-clock. The competitive picture is unchanged; we just don't get the +9 we hoped for from a 1-line change.
