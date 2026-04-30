# T3A — Akamai web sensor_data: status snapshot 2026-04-30

## Shipped (this session)

| Phase | What | Where |
|---|---|---|
| **A0** | Reference capture from real Chrome 147 / bestbuy via Playwright MCP. 3 POST bodies, headers, format decoded. | `docs/akamai_sensor_reference_2026_04_29.txt` |
| **A1** | `crates/akamai/` scaffold: `AkamaiSession`, `AkamaiSessionStore`, `AbckState` parser, mouse/key/touch event types, counter tuple. | `crates/akamai/src/{lib,session}.rs` |
| **A2** | v2 crypto layer ported from DalphanDev: 23-bit LCG, Fisher-Yates `shuffle_tokens`, alphabet `substitute_chars` (P6D 127-entry lookup, B6D 91-char output), `sha256_b64`, `build_v2_dalphan` + `build_v2_bestbuy` envelope assemblers. | `crates/akamai/src/crypto.rs` |
| **A3** | 58-element `tAD` array — 29 (marker, data) pairs. Each field wires to oxide state (`StealthProfile`, `GpuProfile`, canvas/audio seeds, mouse buffer). Top-level `build_sensor_data` entry point. | `crates/akamai/src/{payload,lib}.rs` |
| **A4** | `humanize.js` taps push events into `globalThis.__akamai_events` per-page buffer (mouse/key/scroll, capped at 200/200/100). `crates/akamai/src/drain.rs` exposes `DRAIN_JS` constant + `parse_drained()` for the Rust HTTP client. | `crates/browser/src/js/humanize.js`, `crates/akamai/src/drain.rs` |
| **A5** | `HttpClient.akamai_sessions: AkamaiSessionStore` + `learn_abck()` parses `Set-Cookie: _abck=` / `bm_sz=` and updates per-host trust state. Wired into all 4 response-handling sites. `pub send_akamai_sensor_data(host, request_url, post_path, tenant_seed, drained)` builds + POSTs the body. | `crates/net/src/lib.rs` |

**Tests**: 29/29 akamai unit tests pass. 544 integration tests across chrome_compat / anti_bot / phase7_ab_probe / perimeterx_surface_parity / w3c_apis stay green. Workspace lib tests stay green. Holistic sweep: **114/126** — exactly matching the pre-T3A baseline (no regression).

## Not yet wired (A6 remaining work)

The plumbing is end-to-end ready, but **`send_akamai_sensor_data` is never CALLED from `Page::navigate` yet**. To actually flip bestbuy/homedepot to L3-RENDERED we need:

1. **`post_path` discovery.** The Akamai sensor_data POST URL is obfuscated per tenant (e.g. `/iBo5C/hYh/7w3a/LoSr/yK3l/...` for bestbuy, different for homedepot). The path is embedded in the challenge JS that Akamai serves on the page. Two options:
   - **Static map**: capture the path once via Playwright MCP per host, hardcode in a per-tenant config table. Works until Akamai rotates the obfuscation, which is monthly–quarterly.
   - **Dynamic sniff**: scan parsed scripts for `fetch("/<random>/<random>/...")` patterns, take the first match. More robust but needs an AST walk on Akamai's heavily-obfuscated bd-l-loader script.
   - Recommended: ship static map first; add sniffer later if rotations hurt.

2. **Per-tenant `tenant_seed`.** bestbuy uses `3_224_113`. homedepot's seed is in their challenge JS — not yet captured. Same static-map-first approach as post_path.

3. **`Page::navigate` scheduling.** After the initial page render + JS settles (~500ms post-load), check `HttpClient.akamai_sessions.abck_state(host)` — if `NeedsSensor`, drain `globalThis.__akamai_events` via `page.evaluate(akamai::DRAIN_JS)`, parse via `akamai::parse_drained`, call `HttpClient.send_akamai_sensor_data`. Retry up to N=3 if response keeps `_abck` unfavorable.

4. **Reference-vector pinning of crypto.** Our `build_v2_bestbuy` produces a *structurally* correct envelope but we haven't byte-matched the cleartext-pre-shuffle against a decrypted reference. Without that, our SHA-256 signature and shuffle output are unlikely to satisfy Akamai's verification. Two paths:
   - **Decrypt the captured ciphertext**. Reverse the substitution + shuffle (we have all the constants). Diff our cleartext against it; iterate the field set until they match. ~1 day.
   - **Treat the SHA-256 as "unknown but parseable"**. Akamai may verify only some fields server-side; partial correctness might already be accepted on borderline scores. Try first, decode if needed.

5. **Field set holes**. Several `tAD` slots are static placeholders (`-103`/X8D, `-127`/g8D, `-128`/NRD `,1,<sha-hex>`, `-70`/fpValStr structure) — they need per-session derivation. Bestbuy may accept loose matches; homedepot tends stricter.

## Recommended next-session sequence

1. Capture homedepot reference (parallel to bestbuy A0). 30 min.
2. Decode bestbuy's captured ciphertext via reverse-substitute + reverse-shuffle to get the **exact cleartext** real Chrome 147 emitted. Diff against `payload::build_cleartext` output. Fix field-set deltas. 4–6 hours.
3. Run the full `Page::navigate → send_akamai_sensor_data` flow against a live bestbuy page. Capture the response's new `_abck` suffix. Iterate until favorable. 1–2 days.
4. Repeat (3) for homedepot.
5. Re-run the holistic sweep. Expect **114 → 116** (or 115 if only one of the two flips).

Total estimate to actually flip both Akamai sites: **3–5 more days** of focused work, on top of the ~2 days T3A scaffolding shipped this session.

## Why the foundation is still net-positive shipped

Even without firing the POST, the `crates/akamai/` foundation:

- Locks in the v2 algorithm (LCG + shuffle + substitute + signature) with 29 unit tests + reference-architecture docstrings. A working solver atop this is now days, not weeks.
- Adds `_abck` trust-state parsing + observability across all responses (the holistic sweep logs now show every Akamai-protected site's `_abck` state in real time).
- Adds `bm_sz` cookie capture for v3 PRNG seeding (we'll need it for any v3 sites).
- Adds the humanize.js behavioural tap, which is independently useful for ANY future bot-protection that grades behavioural data (Akamai sec-cpt, DataDome, PerimeterX, etc.).

The "best stealth engine ever" goal is closer than before, even if the holistic-sweep number didn't move on this checkpoint.

## Cross-references

- `docs/RESEARCH_AKAMAI_BMP_BYPASS_2026_04_29.md` — full bypass landscape research
- `docs/akamai_sensor_reference_2026_04_29.txt` — the A0 capture
- `docs/RESEARCH_KASADA_BYPASS_2026_04_29.md` — Kasada equivalent (T3B, deferred)
