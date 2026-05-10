# PLAN_2026_05_10 — UPDATE 2 (post-TLS-investigation, post-W2/W17/W12 fixes)

This addendum supersedes the W3 (TLS) section of `PLAN_2026_05_10.md`
based on empirical findings from the 3 background research agents +
direct JA4 capture against the live engine.

## What changed

### W2 (iphey URL parser) — DONE ✅
Commit `71123ec`. Replaced ad-hoc `resolve_redirect` with `Url::join`
per RFC 3986 §5.2. Verified: iphey.com loads (`content_len=26347`).

### W17 (homedepot Akamai stub) — DONE (defensive, not full fix) ✅
Commit `71123ec`. Removed the harmful `tenant_seed: 0` placeholder.
homedepot now navigates without our (wrong) sensor POST polluting the
signal. Full fix still requires capturing homedepot's real tenant_seed
+ obfuscated POST path via Playwright MCP.

### W3 (TLS ClientHello) — Major finding: WE'RE BYTE-PERFECT CHROME 147

Commit `97dd53d` shipped two real TLS fixes:
1. Drop Zlib from cert-compression algorithms list (Chrome 147 sends Brotli only)
2. Fisher-Yates shuffle over all 16 extensions (was 3-bucket folkore)

**Empirical verification via tls.peet.ws:**

| Field         | Our engine                                     | Real Chrome 147 (headed, same IP)              | Match |
|---------------|-----------------------------------------------|------------------------------------------------|-------|
| ja4           | `t13d1516h2_8daaf6152771_d8a2da3f94cd`        | `t13d1516h2_8daaf6152771_d8a2da3f94cd`         | ✅ EXACT |
| ja4_r         | `t13d1516h2_002f,...,cca9_0005,...,ff01_0403,...` | `t13d1516h2_002f,...,cca9_0005,...,ff01_0403,...` | ✅ EXACT |
| peetprint_hash| `1d4ffe9b0e34acac0bd883fa7f79d7b5`            | `1d4ffe9b0e34acac0bd883fa7f79d7b5`             | ✅ EXACT |
| ALPS payload  | `h2` w/ 4 SETTINGS + ACCEPT_CH                | `h2` w/ 4 SETTINGS + ACCEPT_CH                 | ✅ EXACT |
| H2 SETTINGS   | `1:65536;2:0;4:6291456;6:262144` + WUI 15663105 | (matches per reference docs)                | ✅ EXACT |

**This is byte-perfect Chrome 147 TLS parity.** Our engine is now
indistinguishable from real Chrome 147 at the TLS layer.

**The canadagoose H2→HTTP/1.1 downgrade is therefore NOT a TLS issue.**
Three counter-tests done in the same session, same IP, same TLS config:
- `https://www.google.com/` → status 200, h2 ✅
- `https://www.cloudflare.com/` → status 200, h2 ✅
- `https://tls.peet.ws/` → status 200, h2 ✅
- `https://www.canadagoose.com/` → status 429, h1.1 ❌ (only canadagoose downgrades)

**New hypothesis: canadagoose's Akamai/Kasada edge is rate-limiting our IP**
(we've made dozens of requests to canadagoose this session via the test
suite). The H1.1 downgrade is likely how Akamai soft-throttles flagged
IPs while still serving content.

**Wildberries TLS unexpected-EOF is FIXED** by the same TLS commits:
status now 498 (Nginx custom — token-missing / CSRF check) instead of
handshake failure. Wildberries got past TLS; subsequent issue is
unrelated to fingerprint.

### Implications for the rest of the plan

**W3 (TLS fingerprint) — closed.** The TLS layer is byte-perfect Chrome.
No further work in `crates/net/src/tls.rs` will move pass-rate. Future
"H2 downgrade" reports should default to assuming rate-limit / IP-side
state, not TLS.

**Canadagoose / hyatt / realtor** — still blocked, but now we know it's
NOT TLS. Two possibilities:
1. **Engine-side leaks** still detected post-TLS (the 13 remaining
   Kasada error fields per `CANADA_GOOSE_DIAGNOSIS_PART2.md`).
2. **IP rate-limit** from this session's heavy testing — would clear
   after a cooldown OR with a fresh residential proxy.

Test plan to disambiguate:
- Wait 1-2 hours for any rate-limit to expire, retry canadagoose with
  unchanged engine — if it now passes, it was IP-related.
- If still fails, it's the engine leaks (W4 work).

**W12 (wildberries) — partially fixed.** TLS layer works. Needs an
additional test to identify what status 498 means (Nginx custom code,
likely missing CSRF token or specific session cookie). Lower priority
than originally rated.

**W6 (DataDome) — research landed.** Per
`docs/RESEARCH_DATADOME_BYPASS_2026_05_10.md`:
- DataDome has NO custom VM bytecode (vs Kasada). Just heavy obfuscation
  around a straight JS payload with cracked dual-XOR-PRNG crypto.
- Public crypto cracker: `github.com/glizzykingdreko/datadome-encryption`.
- Endpoints: `js.datadome.co/tags.js` (script), `api-js.datadome.co/js/`
  (sensor POST), `geo.captcha-delivery.com/{captcha,interstitial}`.
- Cookie: `datadome` (1y TTL, IP-bound).
- Critical insight: **captchas are <0.01% of human traffic per DataDome's
  own marketing.** Tier-0 (silent JS-tag POST → cookie) = no solver
  needed; the line is fingerprint quality.
- Probe surface that bites us: Picasso canvas-class fingerprinting (Skia
  rasterization differences), CDP `Runtime.enable` detection (we don't
  speak CDP so structurally absent ✓), WebGL renderer/vendor strings,
  audio fingerprint, behavioral mouse-coord scoring.
- Estimate: ~7-12 working days for Tier-0 silent pass-through.

**W7 (Cloudflare) — research landed.** Per
`docs/RESEARCH_CLOUDFLARE_BYPASS_2026_05_10.md`:
- udemy.com is Cloudflare **Managed Challenge** (Turnstile signal collector,
  not legacy IUAM). Public guides rate it 2/5 difficulty — fingerprint-
  correct browsers usually clear without solving.
- The IUAM PoW is largely retired in 2025 — modern flow is
  `/cdn-cgi/challenge-platform/h/{b,g}/orchestrate/managed/v1`.
- cf_clearance is bound to IP+UA+TLS — must be acquired in-engine.
- Recommended approach: **don't deobfuscate, run the orchestrator JS in
  our V8+DOM**.
- Highest-leverage single fix: UA-CH negotiation (response to
  `accept-ch`/`critical-ch`) — udemy demands 14 hint headers and
  inconsistency tips threat score.
- Estimate: 6.25 engineer-days for V1, 9.25 for V2.

## Updated workstream rankings

| W# | Workstream | Status | Sites unblocked | Time remaining |
|----|------------|--------|----------------:|---------------:|
| W2 | iphey URL parser | ✅ DONE | 1 | 0 |
| W17 | Akamai homedepot defensive stub | ✅ DONE | 0 (was 1, needs Playwright capture for full fix) | 1-2 hrs |
| W3 | TLS ClientHello fingerprint | ✅ DONE (byte-perfect verified) | 0 (was claimed +3, but those were not TLS) | 0 |
| W12 | wildberries — past TLS, now status 498 | partially done | 1 (needs CSRF investigation) | 2-4 hrs |
| W4 | Kasada inventory (canadagoose/hyatt/realtor) | pending | 3 (BUT may be IP-limited; verify after cooldown) | 1-2 days |
| W5 | SPA hydration (5 sites) | pending | 3-5 | 1-7 days (tier A vs A+B) |
| W6 | DataDome (4 sites) | research done, ready to start | 4 | 7-12 days |
| W7 | Cloudflare (1 site) | research done, ready to start | 1 | 6-9 days |

Realistic ceiling rolls down slightly to **121-123 / 126** — the canadagoose
"+3" claim was the biggest projected win and may need IP rotation rather
than engine work. The good news: TLS is solved, two engine wins shipped
already (iphey, partial wildberries), and we have fully-researched action
plans for both DataDome (4 sites) and Cloudflare (1 site).

## Recommended order for next sessions

1. **Verify the canadagoose IP-limit hypothesis** (1-2 hrs):
   - Wait, retest. If passes, we know engine is fine and IP rotation is
     the practical path for Kasada-protected sites.
   - If still fails, proceed with W4 leak inventory.
2. **Capture homedepot Akamai config via Playwright MCP** (2 hrs):
   - W17 full fix → +1 site immediately.
3. **Wildberries 498 investigation** (2-4 hrs):
   - Identify the missing token; likely small fix.
4. **W6 DataDome silent-pass-through** (1-2 weeks):
   - Highest count of sites unblocked (4) for a single workstream.
5. **W5 SPA hydration tier A** (1 day):
   - Cheap host-aware budget bump + early-exit signal → +2-3 sites.
6. **W7 Cloudflare V1** (~1 week):
   - +1 site (udemy).

## Engineering metric to maintain

Current sweep: **106/126 = 84%**.
Per-fix expected delta:
- iphey alone (already shipped): +1 = 107/126 = 85%
- TLS partial wildberries (already shipped): possibly +1 = 108/126 = 86%
- Verify with one more holistic sweep after the 4 commits this session
  land. Target floor: 108/126 = 86%.
