# Handoff — 2026-04-29 T3A close

> Second handoff of 2026-04-29. Picks up from
> `HANDOFF_2026_04_29_session_close.md` (13:16, Phase 7 close at
> 113-114/126). The remainder of the day drove network-stack hardening
> (T1A-T1C), Kasada root-cause investigation (T2A-T2C), bypass-landscape
> research (Kasada + Akamai), and the full T3A Akamai sensor_data
> foundation (A0-A5). Holistic sweep ends at **114/126** — same as the
> Phase 7 baseline; no regression, no movement. The score won't move
> until T3A-A6 wires `send_akamai_sensor_data` into `Page::navigate`.

## Headline numbers

| Run | PASS / 126 | Notes |
|---|---:|---|
| Phase 7 close (13:16 handoff) | 113-114 | leboncoin DataDome oscillates |
| **T3A-A5 close (this handoff)** | **114** | foundation shipped, no regression |

**Workspace test health**: 561+ tests green across `chrome_compat`,
`anti_bot`, `phase7_ab_probe`, `perimeterx_surface_parity`, `w3c_apis`,
`csp_enforcement`. New `akamai` crate adds 29 unit tests, all pass.

## What shipped this session

### T1 — Network stack hardening (3 commits, ~17:36-17:46)

Targeted gaps surfaced by the Phase 7 holistic sweep.

- **T1A — CookieJar Domain attribute parsing** (`e192332`).
  `crates/net/src/cookies.rs`. `Set-Cookie: ... Domain=.example.com`
  was previously dropped; now honored. Fixes mail.ru subdomain hop.
- **T1B — TLS profile verify + Chrome 130→147 relabel** (`bcf9764` +
  `f3a31f6`). `crates/net/src/tls.rs` + capture in `docs/`. Cipher
  list / extensions / GREASE all byte-exact vs Chrome 147.
- **T1C — Proxy support plumbed through tcp/h2_client** (`89f48a1`).
  `StealthProfile.proxy: Option<ProxyConfig>` (Basic auth + URL
  percent-decoding); honored at TCP connect for h1, h2, h3.

### T2 — Kasada investigation (3 commits, ~17:49-18:13)

Outcome: **Kasada is not solvable without server-side reputation**
(residential proxy / KaaS). Engineering pivot away from Kasada
to Akamai (T3A) for higher-ROI session.

All three appended to `docs/KASADA_CT_TOKEN_INVESTIGATION_2026_04_29.md`:

- **T2A — ct_token investigation** (`920d77f`). Traced full KPSDK flow on
  canadagoose: `/tl` is **never called** because the edge classifier
  returns 429 before SDK boots. Token logic is fine; the IP is the
  problem.
- **T2B — root-cause revision** (`3504cde`). Reframed earlier "JS-VM
  divergence" hypothesis as **edge classifier**: same machine passes
  via Playwright MCP, so JS surface is fine.
- **T2C — error-blob capture** (`7c4e56e`). Decoded Kasada bail signal
  payloads; confirms client-side never gets a chance to respond on
  reputation-flagged IPs.

### Research — Kasada + Akamai bypass landscape (1 commit, 18:38)

`docs/RESEARCH_KASADA_BYPASS_2026_04_29.md` (329 LOC) +
`docs/RESEARCH_AKAMAI_BMP_BYPASS_2026_04_29.md` (396 LOC). Catalogues
public OSS solvers, commercial KaaS, and protocol references.
Conclusion: **Kasada is 2-4 weeks of VM-devirt work (Path B)** —
defer; **Akamai web v2 is days of port work (Path A)** — proceed.

### T3A — Akamai web sensor_data foundation (6 commits, 19:14-19:48)

Per the plan in `.claude/plans/docs-research-2026-04-28-second-layer-b-enchanted-blossom.md`.
**Note**: plan said v3 (PRNG-shuffle); A0 capture proved bestbuy uses
**v2** (Fisher-Yates + alphabet substitution + SHA-256). Pivoted
reference impl from glizzykingdreko (v3-only) to DalphanDev/akamai-sensor
(manually-deobfuscated Go, MIT, 458 LOC).

- **A0** (`b365937`). 3 real Chrome 147 POSTs captured via Playwright MCP
  → `docs/akamai_sensor_reference_2026_04_29.txt`. Documents v2 envelope:
  `3;0;1;0;<seed>;<sha256-b64>;<counter>;<scrambled-body>`. Endpoint
  obfuscated per tenant: `/iBo5C/hYh/7w3a/...` for bestbuy.
- **A1** (`d46e9b7`). `crates/akamai/` workspace member, foundation:
  `AkamaiSession` + `AkamaiSessionStore` (Arc<RwLock<HashMap>>, mirrors
  KasadaSessionStore), `AbckState` parser, MouseEvent/KeyEvent/TouchEvent
  types, `CounterTuple`.
- **A2** (`561d152`). `crates/akamai/src/crypto.rs`. 23-bit LCG
  (`(seed*65793 + 4282663) & 0x7FFFFF`), Fisher-Yates `shuffle_tokens`,
  `B6D` 91-char output alphabet, `P6D` 127-entry char→base-index
  lookup, `substitute_chars`, `sha256_b64`. `build_v2_dalphan` +
  `build_v2_bestbuy` envelope assemblers. 10 unit tests including
  byte-exact LCG reference vector.
- **A3** (`3337414`). `crates/akamai/src/payload.rs`. 58-element `tAD`
  array as 29 (marker, data) pairs (`-100/vAD`, `-110/zdD` mouse traj,
  `-129/LRD` canvas+WebGL hashes, etc.). Every field wires to
  `StealthProfile` / `GpuProfile` / canvas+audio seeds / mouse buffer.
  Top-level `build_sensor_data` entry point. 6 unit tests.
- **A4** (`d640018`). `crates/browser/src/js/humanize.js` taps push
  events into `globalThis.__akamai_events` per-page buffer
  (mouse/key/touch/scroll, capped at 200/200/100). New
  `crates/akamai/src/drain.rs`: `DRAIN_JS` constant (IIFE that
  snapshots & clears) + `parse_drained()` for the Rust HTTP client.
  4 unit tests.
- **A5** (`d296a38`). `crates/net/src/lib.rs`:
  `HttpClient.akamai_sessions: AkamaiSessionStore` + `learn_abck()`
  parses `Set-Cookie: _abck=` / `bm_sz=` and updates per-host trust
  state. Wired into all 4 response-handling sites. New public
  `pub async fn send_akamai_sensor_data(host, request_url, post_path,
  tenant_seed, drained) -> Result<AbckState, NetError>` builds +
  POSTs the body.

### T3A-A6 status doc (19:58, untracked until this commit)

`docs/T3A_AKAMAI_STATUS_2026_04_30.md` documents what's pending:
post_path discovery, per-tenant tenant_seed for homedepot,
`Page::navigate` scheduling, reference-vector pinning of crypto,
field-set holes.

## Why the score didn't move

`send_akamai_sensor_data` exists and is correct end-to-end —
**it's never called from `Page::navigate` yet**. Required for that:

1. **`post_path` discovery.** Per-tenant obfuscated path
   (`/iBo5C/hYh/7w3a/...` for bestbuy). Embedded in the Akamai
   challenge JS that's served on the page. Two options: static map
   (works until rotation, monthly-quarterly) or AST sniff. Ship
   static map first.
2. **Per-tenant `tenant_seed`.** bestbuy = `3_224_113`. homedepot
   unknown — needs A0-equivalent capture.
3. **`Page::navigate` scheduling.** After page settles (~500 ms
   post-load), check `HttpClient.akamai_sessions.abck_state(host)` —
   if `NeedsSensor`, drain `__akamai_events`, call
   `send_akamai_sensor_data`. Retry up to N=3 if response keeps
   `_abck` unfavorable.
4. **Reference-vector pinning.** Decrypt the captured ciphertext
   (we have all constants), diff our cleartext against it,
   iterate field set until they match. ~1 day.
5. **Field-set holes.** Several `tAD` slots are static placeholders
   (`-103/X8D`, `-127/g8D`, `-128/NRD`, `-70/fpValStr`). Need
   per-session derivation. bestbuy may accept loose; homedepot stricter.

Realistic path to flip bestbuy + homedepot: **3-5 days** of focused
work iterating against live `_abck` responses.

## Analysis: what the 114/126 gap actually contains

12 failures, broken down by root cause:

| Bucket | Sites | Fixable how |
|---|---|---|
| Akamai web v2 (T3A target) | bestbuy, homedepot | A6 wire-up, 3-5 days |
| Kasada strict | canadagoose, hyatt, realtor | residential proxy ($) OR T3B Kasada VM-devirt (2-4 weeks) |
| WBAAS IP-bound | wildberries | residential proxy (Russian) |
| In-house 403 | ozon | residential proxy (Russian) |
| HUMAN PaH | zillow | T3D PerimeterX press-and-hold (1-2 weeks, no public ref) |
| DataDome `tags.js` | etsy, tripadvisor, yelp, leboncoin | T3C DataDome (1-2 weeks) |
| Behavioural CAPTCHA | douyin | not solvable headless |

**Code-only path** (no proxy): A6 (2 sites) + T3C (3-4 sites) + T3D
(1 site) = 6-7 sites recoverable. **3-6 weeks of work**.

**Proxy path**: Russian residential ($50-500/mo) likely unlocks
wildberries + ozon (2 sites) immediately. KaaS for Kasada
($X/mo) unlocks 3 more. **0 days of code, ~$100-1000/mo**.

Best mixed path: A6 (Akamai) + Russian proxy (WB+Ozon) → **+4 sites
in ~1 week + $50-500/mo**. Goes 114→118.

## Recommendation

**Highest ROI next: pivot to proxy validation, finish T3A-A6 in
parallel.**

Concrete sequence (1 week wall-clock):

1. **Day 1 morning**: rent a Russian residential proxy (Bright Data /
   Smartproxy / Soax, ~$50-100 trial). Run holistic sweep with
   `BOXIDE_PROXY=http://...` against the 4 IP-bound sites
   (wildberries, ozon, canadagoose, hyatt). **This is a $50 test
   with massive information value** — confirms or refutes the
   "reputation-bound, not code-bound" thesis for Kasada and
   WBAAS once and for all.
2. **Day 1-3**: in parallel, T3A-A6 step 1: capture homedepot
   reference (30 min) + decrypt bestbuy ciphertext, diff against
   `build_cleartext` output, fix field deltas (4-6 hours).
3. **Day 4-5**: T3A-A6 step 2: wire `Page::navigate` scheduler with
   hardcoded post_path for bestbuy + homedepot. Iterate against
   live `_abck` responses.
4. **Day 6-7**: holistic sweep, expect **114 → 116-118** depending
   on outcomes.

**If only one branch can run**: start with the proxy validation —
it's $50, takes hours not days, and either (a) unlocks 4 sites
immediately (huge win) or (b) tells us conclusively that
client-side improvements are now the only lever (also valuable).

**If user wants pure-code work**: start T3A-A6 step 1 (decrypt the
captured bestbuy ciphertext to pin the reference vector). That's
the load-bearing 4-6h that determines whether our crypto is
byte-exact. Without that, the Page::navigate wire-up is shooting
in the dark.

## Files touched this session

```
 Cargo.lock                                    |  14 +
 Cargo.toml                                    |   2 +
 crates/akamai/Cargo.toml                      |  19 ++
 crates/akamai/src/crypto.rs                   | 373 ++
 crates/akamai/src/drain.rs                    | 202 ++
 crates/akamai/src/lib.rs                      | 206 ++
 crates/akamai/src/payload.rs                  | 366 ++
 crates/akamai/src/session.rs                  | 213 ++
 crates/browser/src/js/humanize.js             |  46 +
 crates/net/Cargo.toml                         |   1 +
 crates/net/src/lib.rs                         | 104 +
 docs/RESEARCH_AKAMAI_BMP_BYPASS_2026_04_29.md | 396 ++
 docs/RESEARCH_KASADA_BYPASS_2026_04_29.md     | 329 ++
 docs/akamai_sensor_reference_2026_04_29.txt   |  84 +
 docs/KASADA_CT_TOKEN_INVESTIGATION_2026_04_29 |  ~160 (T2A+B+C)
 docs/T3A_AKAMAI_STATUS_2026_04_30.md          |  ~80
 14 files, ~2,355 LOC
```

## Cross-references

- `docs/HANDOFF_2026_04_29_session_close.md` — earlier handoff (Phase 7)
- `docs/T3A_AKAMAI_STATUS_2026_04_30.md` — A6 punch list
- `docs/RESEARCH_AKAMAI_BMP_BYPASS_2026_04_29.md` — full landscape
- `docs/RESEARCH_KASADA_BYPASS_2026_04_29.md` — Kasada Path B analysis
- `docs/akamai_sensor_reference_2026_04_29.txt` — A0 ground truth
- Plan: `.claude/plans/docs-research-2026-04-28-second-layer-b-enchanted-blossom.md`

## Out of scope (deferred)

- **T3A-A6 wire-up** — primary next-session target (3-5 days)
- **T3B Kasada** — Path B, 2-4 weeks VM-devirt (canadagoose/hyatt/realtor)
- **T3C DataDome** — `tags.js` recovery for etsy/tripadvisor/yelp/leboncoin
- **T3D PerimeterX press-and-hold** — zillow (no public reference)
- **Parity D11** — `document.elementFromPoint` viewport-aware (long-pending)
- **Mobile Akamai BMP**, **Akamai sec-cpt PoW**, **pixel challenge**
