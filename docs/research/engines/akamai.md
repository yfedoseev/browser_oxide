# Akamai Bot Manager — Deep Engine Analysis (browser_oxide)

**Author:** deep-research agent (Akamai) · **Date:** 2026-05-16 ·
**Baseline git HEAD:** `fd98bfa` · **Contract:** follows
`docs/research/engines/00_INVENTORY_AND_METHOD.md` §0–§12 verbatim.

Legend for claim provenance (mandated by §1 rules of engagement):
**[MECH]** mechanism fact, externally cited · **[CODE]** our-code fact,
file:line I read this session · **[HYP]** hypothesis, labelled.

---

## 0. Executive summary & pass-guarantee thesis

**How Akamai decides bot-vs-human in 2026 [MECH].** Akamai Bot Manager
is an *additive, two-phase, cross-layer* score. **Phase 1** (pre-JS):
TCP/TLS/HTTP-frame fingerprint on the very first request — JA4(+),
HTTP/2 SETTINGS + frame/pseudo-header order, ALPN, header order, and —
**new on 2026-01-31** — the presence of the X25519MLKEM768
post-quantum key share (PQ became the Akamai client-connection default;
its *absence* "operates outside the baseline Akamai defines as normal
browser behavior"
[scrapfly post-quantum, accessed 2026-05-16]). **Phase 2**: the page
runs `bmak.js`, collects ~30 named JSON fields + behavior, and POSTs
`{"sensor_data":"<encrypted>"}` to a per-tenant obfuscated path; the
server re-derives the keys, decrypts, JSON-parses, ML-scores the
content, and upgrades `_abck`. If Phase-1 score is good enough, "phase
two is mostly ceremonial" and `_abck` reaches its trusted state after
1–2 requests with no valid sensor at all
[xkiian/asadfix 2026, accessed 2026-05-16]. When the score is over the
strict threshold Akamai escalates to **sec-cpt** (428 PoW/behavioral
interstitial) or **SBSD** (429, proactive pre-`_abck` sensor cycle).

**The single highest-leverage gap blocking us.** For the *one* truly
blocked Akamai site (homedepot, a sec-cpt interstitial), the blocker is
that the rotating obfuscated sec-cpt bundle must **self-solve in our
V8** — a Phase-5 capability we do not have, and our offline
`sec_cpt::solve_crypto` is **dead code** that cannot even be fed
(no parseable 428 JSON in the body). For the *other 10* Akamai sites
the highest-leverage fact is **they are not blocked at all** — they
render; our entire `crates/akamai/` sensor pipeline is **not the thing
keeping us out**, and most of it is dead/unreached. The honest
top lever is therefore **measurement integrity + not regressing the 10
that already pass**, not "ship a better sensor".

**Honest verdict — which guarded sites we pass / block / FP today**
(Akamai = 11 sites: bestbuy, costco, disneyplus, expedia, hm,
homedepot, hulu, uniqlo, walmart, washingtonpost, weather):

| Site | 2026-05-16 verdict | Truth |
|---|---|---|
| bestbuy | `pass` (7.8 KB) | renders; **thin-shell caveat** — verify content depth |
| costco, disneyplus, weather, washingtonpost, uniqlo, hulu | `pass` (0.4–3 MB) | **real renders** (not FP) |
| hm, expedia, walmart | `pass` (0.3–0.5 MB) | renders |
| homedepot | **flipped → `L3-RENDERED`** at `b623d5d` | genuine sec-cpt; doc-20 fix made the in-page bundle the sole actor — *intermediate* 2.5 KB page, challenge cleared, not yet full homepage |
| (the "10 of 11 FP" finding) | were historically mislabelled blocked | classifier over-match, **falsified** by Phase-0.2 typed re-baseline |

**Net:** 10/11 render; homedepot was the only genuine Akamai block and
is now flipped *by the directive's sanctioned metric* (challenge
cleared, classifies RENDERED) but is a post-sec-cpt intermediate page,
not a verified full homepage render. **Zero of the 11 are a confirmed
IP ban.** Our sensor crate is not load-bearing for any of them.

---

## 1. Vendor surface & 2026 deployment

**Product tiers [MECH].** `BOT_MANAGER` (baseline → 200 + `ak_p`
score header), `BOT_MANAGER` Cautious (200, score ≥ 50, cookie
poisoned slot 1 = `0`), `BOT_MANAGER_PREMIER` Strict (428 + sec-cpt),
`BOT_MANAGER_PREMIER` Aggressive (429 + SBSD, or 403 hard-block)
(02_AKAMAI.md §8 [CODE/doc]).

**Script names / paths [MECH].** Two obfuscated per-tenant paths
(rotate ≈ 24–48 h, often faster): a deep `<script src="/<seg>/<seg>/…">`
(≥4–8 path segments, e.g. homedepot capture
`/Wjv3muMJul/a-27ijBVRX/a7EmGXDhfkNJzh/Lk8hTm9wYQE/Ay/BqRSxpZwgB`)
which is the **bmak.js bootstrap + sensor POST endpoint**, plus a
deferred telemetry variant with `?v=<uuid>`
(`/Wjv3muMJul/.../wLCgd7c14u?v=5a7c1a19-…`); and the
`/akam/13/<hash>` pixel bootstrap (JS-off fallback). Verified in the
1 MB rendered homedepot capture at
`ab_harness/shots/https_www_homedepot_com_.html` [CODE]. The per-tenant
seed lives in `bazadebezolkohpepadr="<digits>"` in HTML. Auxiliary
endpoint observed live: `GET /_bm/get_params?type=get-akid&v=<hash>`
returns a 44-char base64 `akid` token the VM injects into the sensor
body (`docs/.../adidas_akamai_bmp_v3.md` lines 100–103, 172–179 [CODE]).

**sensor_data versions [MECH].** v1 (legacy XOR, stragglers), v2
(DalphanDev 8-field, fixed-seed LCG shuffle+substitute), **v3**
(standard on premier; LCG seeded by `bm_sz`-derived `cookieHash` + a
`fileHash` extracted from the live bmak.js; cleartext is a ~30-key
JSON object, not the v2 CSV). 2026 reality (xkiian/asadfix): "Akamai
BMP v3 is solvable without a sensor_data generator for the majority of
public targets" because Phase-1 (TLS/H2) dominates.

**Cookie zoo [MECH]** (02_AKAMAI.md §1): `_abck` (bot-trust token,
`Max-Age=1y`), `bm_sz` (4 h PRNG-seed cookie, v3-critical),
`ak_bmsc` (2 h server sensor cookie, forward-only), `bm_sv`
(cache-validation), `bm_ss/bm_so/bm_s/bm_mi` (sub-cookies; `bm_so`
feeds SBSD), `sec_cpt` (only under 428; `~3~` suffix = solved),
`sbsd_o/sbsd` (only under SBSD), `akacd_*`/`AKA_A2`/`akavpau_*`
(CDN affinity, forward-only).

**Which target sites use which tier [MECH/CODE].** All 11 are
`BOT_MANAGER`/`BOT_MANAGER_PREMIER`. homedepot today serves the
**sec-cpt rotating-bundle** variant (`<div id="sec-if-cpt-container">`
+ `<script src="/Wjv3…">`, ≈2.6 KB body). The other 10 serve normal
v3-protected pages that **render for us** (Phase-0.2 re-baseline). None
of our 11 currently exhibits SBSD (hotels.com — not in our 11 — was
the SBSD example in 02_AKAMAI.md §10.4).

---

## 2. Detection pipeline — stage by stage

| Stage | Signal collected [MECH] | Scoring | kill vs soft |
|---|---|---|---|
| **Edge: TLS** | JA4(+); cipher/curve/sigalg/ALPN/extension order; **X25519MLKEM768 PQ key share presence** (Akamai default since 2026-01-31; +1088 B ClientHello, fragments across TCP packets) | hash-bucket vs known-good-Chrome DB; mismatch ⇒ immediate bmak path | **near-kill** (datacenter +30 baseline; non-PQ "outside normal browser baseline") |
| **Edge: HTTP/2** | SETTINGS `1:65536;2:0;4:6291456;6:262144`, no `3`/`5`; window `15663105`; pseudo-header order `m,a,s,p`; HEADERS priority | compared to Chrome reference; curl/Go/Python H2 default ⇒ stream RST(0x2) | near-kill |
| **Edge: headers/IP** | header order, `Sec-Fetch-*`, Accept-Language, ASN family + reputation hot-list | datacenter +30, residential 0, mobile −5 | soft-score, additive |
| **Loader** | `/akam/13/<hash>` + deep obfuscated bootstrap injected; `bm_sz`/`_abck` seeded | sets up Phase-2 | n/a |
| **Fingerprint JS (`bmak.js`)** | ~30-key JSON: `wsl` (heap/plugin canaries — **top scoring vector**), `din`/`fpt`/`ajr` (navigator/screen/tz), canvas/audio paint, `permissions.query` enum-error string, `Function.toString` natives, WebGL vendor/renderer, `navigator.webdriver` (`true` = instant +100) | server decrypts envelope, JSON-parses, ML-scores fields | webdriver/plugins.length=0 = hard fail; rest additive |
| **Behavioral** | mouse-trajectory variance + scroll-velocity dist. (highest-weighted *behavioral*), key down/up split, accel | only invoked if envelope parses (v3 JSON valid) | soft-score |
| **Server ML** | aggregate → `_abck` slot-1 stop-signal / slot-3 invalidation; `Server-Timing: ak_p; desc="rid_ts_a_b_c_d_e_f-"` 6 sub-scores | trust threshold | gates `_abck` upgrade |
| **Escalation** | strict ⇒ 428 sec-cpt PoW; aggressive ⇒ 429 SBSD | separate challenge surfaces | hard gate |

Source: 02_AKAMAI.md §§2,7,8,9 [doc]; scrapfly/xkiian/asadfix 2026
[MECH]; H2 values verified in `crates/net/src/h2_client.rs:11,39-42`
[CODE].

---

## 3. Challenge / script anatomy (bytes & structure)

**Normal v3 page.** Renders; carries the two obfuscated script tags +
`bazadebezolkohpepadr` + `_abck=<hex>~-1~<420B-blob>~-1~-1~-1~-1~-1`
on first response. Success signal = `_abck` slot-1 becomes a positive
int and `sensor_counter ≥ slot1` with slot-3 == −1 (server-ML driven;
in practice 2026 sites also flip to a trusted `~0~`-style marker after
1–2 good Phase-1 requests with no valid sensor [MECH]).

**sec-cpt interstitial (homedepot, 2026-05-16) [MECH/CODE].** ≈2.6 KB
HTML: `<div id="sec-if-cpt-container">` + a rotating obfuscated
`<script src="/Wjv3…">` bundle. **Critically: no parseable 428 JSON,
no inline `nonce`·`difficulty`·`verify_url`** — the classic
`<iframe id="sec-cpt-if" challenge="<base64-json>" data-duration=N>`
shape that `sec_cpt::solve_crypto` expects is **not** what homedepot
serves (master plan §Phase-3 [doc]; `SecCptChallenge` schema vs body).
The crypto-provider PoW (verified vs Hyper SDK Python
`hyper_sdk/akamai/sec_cpt.py`, accessed 2026-05-16):

```
answer       = "0." + random_hex
hash_input   = sec + timestamp + nonce + difficulty + answer
output = 0; for byte in sha256(hash_input): output=(output<<8)|byte; output%=difficulty
# accept answer when output==0, then difficulty += 1 for next answer
submit {"token", "answers"} → POST /_sec/verify?provider=crypto
verify GET  /_sec/cp_challenge/verify ; success ⇔ sec_cpt cookie ends "~3~"
wait chlg_duration seconds first (server-enforced, NOT skippable)
```

Our `crates/akamai/src/sec_cpt.rs:80-120` implements **exactly this**
algorithm (byte-faithful: `sec+timestamp+nonce+(difficulty+i)`,
rolling sha256 `((output<<8)|b)&0xFFFFFFFF; %= difficulty`,
`0.{:013x}` answers) [CODE] — but it is dead (see §8/§10b).

**SBSD (not in our 11, documented for completeness) [MECH].** 429 +
`{"t":"<token>"}`; proactive sensor cycle gated *before* `_abck`; no
public OSS generator (02_AKAMAI.md §4).

**Pixel `/akam/13/<hash>` [MECH].** JS-off fallback; XOR(htmlVar,
scriptVar, UA) over a 91-char alphabet, base64-wrapped; low value, we
skip it.

---

## 4. Fingerprint / sensor payload — field by field

The v3 cleartext is a JSON object. Per
`docs/research_2026_05_14/10_AKAMAI_V3_ENVELOPE_DEEP_2026_05_14.md` §2
[doc] and our `crates/akamai/src/v3_payload.rs::V3Payload` [CODE], the
~30 keys and their expected-real-Chrome value vs **what we emit**:

| Key | Real-Chrome [MECH] | What we emit [CODE v3_payload.rs] | Mismatch scoring |
|---|---|---|---|
| `ver` | SHA-256(bmak.js source) b64 | **static literal** `wS5Kmee…UH4E=` (:98) | edge cross-checks vs envelope field 6; mismatch ⇒ content invalid |
| `wsl` | live: `perf.memory.{limit,total,used}`, `connection.rtt`, `speechSynthesis voices`, `plugins[0][0].enabledPlugin`, `plugins.refresh`, `plugins.item(2^32)`, `File.prototype.path`, `SharedArrayBuffer` — **TOP scoring vector** | **fully hardcoded** `4294705152,42000000,18000000,100,8,1,1,1,0,1,…` (:161-168) | highest-weighted "is real browser" canary; constant ⇒ bot-bucketable |
| `din` | 23 device-info {k:v}: language, productSub, screen, `Math.random()`, UA, window dims, plugins.length | mostly static constants; only UA from profile (:233-259) | medium |
| `fpt` | tz offset, plugins flag, storage, colorDepth, dnt | static template, tz hardcoded `-420`, colorDepth `24` (:103-108, 212-221) | medium |
| `ajr` | UA + inner/outer/screen dims + DPR + gecko flags | UA from profile; **all dims hardcoded** `1280,720,…,1920,1080` (:223-231) | medium |
| `mst` | live event counters, perf timings | static giant constants; only `kc`/`mc`/`tc` from session (:279-316) | low–med |
| `mev`/`kev`/`dme` | mouse/key/DOM-mutation streams | `mev` from `session.mouse_buf`; **`kev` always ""** (:261-263); `dme` always "" (TODO) | behavioral |
| `per` | 20-digit per-session rand | correctly random (:343-350) | ok |
| `dsi`/`fwd`/`hls`/`sde` | per-session | static constants/hashes (:318-341, 143, 170) | low |

**Field-5 / envelope.** `crypto::build_v3_envelope` (:315-326) emits
`3;0;1;0;<cookieHash>;wS5K…UH4E=;141659;<body>` where
`body = substitute_chars_v3(shuffle_tokens_v3(cleartext, fileHash),
cookieHash)` [CODE]. `cookieHash` = `bm_sz` index-2 (`parse_bm_sz`
lib.rs:170-173, default `8_888_888`); `fileHash` = `known_file_hash`
registry (lib.rs:229-252) — **only bestbuy/macys/homedepot hardcoded;
the other 8 fall back to `cookieHash` ⇒ edge cannot reverse-shuffle ⇒
201, `_abck` never flips** [CODE]. Crucially **none of those 3
hardcoded hosts overlaps the 8 unmanned target sites**; bestbuy and
homedepot are in the 11 but bestbuy renders without needing the sensor
and homedepot is sec-cpt (BMP POST suppressed).

**The deeper truth (verified against the live VM behavior) [CODE].**
`adidas_akamai_bmp_v3.md` probed the *real* sensor VM: it fetches an
`akid` token via `/_bm/get_params`, builds 30–48 `;`-delimited
sections, computes its own script hash via `Function.prototype.toString`
on its obfuscated top-level fn, and the **real POST body shape is
nothing like our `build_v3_envelope` output**. Even byte-perfect v3
crypto cannot reproduce a body the VM itself assembles from
runtime-only inputs. This is the core reason a static payload cannot
pass a v3-scored site — confirmed by xkiian 2026 ("the deobfuscator is
a trap": the VM dispatch + akid + script-hash are runtime-bound).

---

## 5. Crypto / encoding

**v3 (what the live path uses) [CODE].** `crypto.rs:271-326`:
`shuffle_tokens_v3` splits on `:`, advances a 23-bit LCG
(`state = state*65793 + 4282663 & 0x7FFFFF`) **twice per swap-pair**,
swaps `tokens[first]`/`tokens[second]`; `substitute_chars_v3` maps each
byte via `B6D` 91-char alphabet `(found+shift)%91` with shift =
`(state>>8)&0xFFFF` *before* advancing. **Byte-verified end-to-end**
against glizzykingdreko's JS reference by
`build_v3_envelope_matches_glizzy_reference_end_to_end`
(crypto.rs:413-421) and the two component reference vectors
(`a:b:c:d:e`,12345→`a:e:d:c:b`; `Hello World`,12345→`yc*{XkAaZk[`)
[CODE]. **The crypto is correct; the content is fake** (§4).

**Key derivation [MECH/CODE].** `cookieHash` = decimal int at
`bm_sz.split('~')[2]`; default `8_888_888` only before first `bm_sz`.
`fileHash` = AST-extracted constant from the deployed `bmak.js`
(glizzy Babel-AST walker; rotates 24–48 h). The edge re-derives both
(cookieHash from the `bm_sz` it issued, fileHash from its stored
bmak.js), reverse-shuffles + reverse-substitutes, then `JSON.parse`s.
A wrong fileHash ⇒ reverse-shuffle yields garbage ⇒ 201, no `_abck`
flip (10_AKAMAI_V3 §0 [doc]).

**Anti-replay / rotation [MECH].** `bm_sz` rotates every 4 h (forces a
fresh sensor cycle); `bmak.js`/`fileHash` rotate ≈24–48 h (often
faster — homedepot rotated 8806534→2900615 in ~40 min,
lib.rs:243-249 comment [CODE]); per-tenant `bazadebezolkohpepadr`
rotates every few weeks. Static registries structurally cannot keep up
(02_AKAMAI.md §13 [doc]).

**Server-side TLS/JA4 cross-check [MECH].** The sensor body is scored
*alongside* the JA4 of the connection that delivered it; a JA4-vs-UA
mismatch poisons even a perfect payload (asadfix 2026). Our desktop
profile sends `X25519_MLKEM768` first + `set_key_shares_limit(2)`
(`crates/net/src/tls.rs:94-100,299-300`) so we are **PQ-correct for
the 2026-01-31 Akamai default — this is a verified NON-gap** [CODE].
Residual: TLS pinned to Chrome **147**, preset UAs are Chrome **148**
(G7) — a JA4-vs-UA tell, engine-wide not Akamai-specific.

---

## 6. Cookie & header lifecycle / state machine

`_abck` wire format (verified bestbuy/homedepot/macys, 02_AKAMAI.md
§1.1 [doc]):
`<hex-id>~<stop-signal>~<420B-base64-blob>~<inv-signal>~<r>~<r>~<r>~<r>`.

- **slot 1 stop-signal:** `-1` = no threshold yet (must POST / Hyper
  default = 3 sensors); positive `N` = favorable once
  `sensor_counter ≥ N`.
- **slot 3 inv-signal:** `-1` valid; `≥0` invalidated (post a protected
  action) ⇒ 1 more sensor.

Our parser `ParsedAbck::parse/evaluate` (`session.rs:78-128`)
implements Hyper `IsCookieValid` semantics correctly: `Favorable` iff
`well_formed && inv==-1 && stop!=-1 && sensor_counter≥stop`;
first-response `~-1~…~-1~` ⇒ `NeedsSensor` (the historic
"silently-favorable" bug is fixed; unit tests
`first_response_needs_sensor`, `stop_signal_*` pass) [CODE].
`learn_abck` (`net/lib.rs:362-383`) ingests `_abck`/`bm_sz` from
Set-Cookie; `send_akamai_sensor_data` (:392-447) increments
`sensor_counter`, POSTs `{"sensor_data":"…"}` with
`content-type: text/plain;charset=UTF-8` + `origin`/`referer`,
re-learns `_abck`. **`NeedsSecCpt`/`NeedsPixel` are never returned by
the cookie parser** — they are surfaced from the response path
(session.rs:54-57 [CODE]).

"Solved" on the wire = slot-1 positive **and** `sensor_counter ≥ slot1`
**and** slot-3 == −1, *or* (2026 Phase-1-dominant path) the site simply
serving real content because TLS/H2 scored trusted.

---

## 7. How OSS / commercial tools defeat it (cited)

| Project / source | Technique | Reproducible? | Patched/dead? | Cite (accessed 2026-05-16) |
|---|---|---|---|---|
| **Hyper-Solutions/hyper-sdk-{go,py,js}** | `GenerateSensorData`/`GenerateSecCptPayload`/`GeneratePixelData` — **sensor generation is server-side behind a paid API key**; only parsing helpers (`IsCookieValid`, `ParseSecCptChallenge`, script_path) are local | **No** (sensor is proprietary remote) | maintained | github.com/Hyper-Solutions/hyper-sdk-go |
| **hyper_sdk/akamai/sec_cpt.py** | the sec-cpt crypto PoW algorithm IS open in the SDK (challenge parse via `challenge="(.*?)"` b64+JSON, `0.<hex>` answer, rolling sha256 %difficulty, `time.sleep(duration)`) | **Yes — and our `sec_cpt.rs` matches it byte-for-byte** | live | github.com/Hyper-Solutions/hyper-sdk-py/blob/master/hyper_sdk/akamai/sec_cpt.py |
| **docs.hypersolutions.co /handling-428-sec-cpt** | confirms flow: `chlg_duration` server-enforced, POST `/_sec/verify?provider=crypto`, success ⇔ `sec_cpt` ends `~3~` | spec only | live | docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt |
| **jesterfoidchopped/akamai-v3-sensor** (Go) | **pure TLS+H2+H3 fingerprint, NO sensor generation** — bets that good Phase-1 yields `~0~` `_abck` with no sensor | partial; 3 commits, dormant, unverified | likely-stale | github.com/jesterfoidchopped/akamai-v3-sensor |
| **glizzykingdreko/akamai-v3-sensor-data-helper** + Medium deep-dive | the v3 envelope crypto (elementSwapping/characterSubstitution, cookieHash/fileHash) — **this is the basis our `crypto.rs` byte-matches** | **Yes (crypto only)**; cleartext content still must be live | maintained tools site | medium.com/@glizzykingdreko/akamai-v3-…-da0adad2a784 (Medium article 404'd 2026-05-16; tooling at akamai-v3-tools.vercel.app) |
| **xkiian "deobfuscator is a trap" (DEV, 2026)** | argues static sensor RE is wrong layer; fix TLS/JA4/H2/PQ so Phase-2 is ceremonial; VM dispatch + akid + script-hash are runtime-bound | conceptual | URL 404'd 2026-05-16; corroborated by asadfix scraping-guide + scrapfly | dev.to/xkiian/…-5cjh (dead); asadfix.github.io/scraping-guide |
| **scrapfly post-quantum-tls** | 2026-01-31 Akamai PQ default; missing X25519MLKEM768 = "outside normal browser baseline", pre-application-layer kill | mechanism fact | live | scrapfly.io/blog/posts/post-quantum-tls-bot-detection |
| DalphanDev/akamai-sensor, xiaoweigege/akamai2.0, Edioff/akamai-analysis | v2 LCG/shuffle/substitute + signal taxonomy (basis of our v2 `crypto.rs`/`payload.rs`) | yes for **v2** (v2 ≈ dead deployment in 2026) | v2-era | (02_AKAMAI.md §15) |

**Skeptical synthesis [MECH].** Every *reproducible* OSS path is either
(a) the **crypto envelope only** (which we already byte-match and which
is *not* the blocker) or (b) **pure TLS/H2 fingerprint betting**
(no sensor at all — exactly the strategy that already works for 10/11
of our sites). The *content* generation that would matter for a
v3-scored block is universally **paid-API / proprietary** or
**run-the-real-VM**. No public tool statically forges a v3 body that
ML-passes a strict site in 2026; commercial solvers use VM emulation,
not static analysis (corroborated independently by our own adidas
probe). This is a first-class **negative result**.

---

## 8. What browser_oxide does today (file:line evidence)

**Live path from navigate() to success signal:**

1. `net::HttpClient` GET → `learn_abck` (`net/lib.rs:362`) parses
   `_abck`/`bm_sz` from Set-Cookie into `AkamaiSessionStore`. **WIRED
   & exercised.**
2. `Page::navigate` builds the page, runs vendor JS in V8.
   `started_as_seccpt_challenge = html.contains("sec-if-cpt-container")
   || "sec-cpt-if"` is captured **from the original response HTML**
   (`page.rs:1432`). **WIRED & exercised** (the doc-20 fix).
3. `page.rs:1812-1818`: if `started_as_seccpt_challenge` ⇒
   `akamai_state = NeedsSecCpt` (skip BMP entirely); else
   `handle_akamai_flow(&client)`. **WIRED & exercised.**
4. `handle_akamai_flow` (`page.rs:450-601`): a *second* sec-cpt guard
   on `self.content()` (mutable DOM, :470-476) → tenant via
   `get_tenant_settings` (static bestbuy/homedepot only, lib.rs:445-494)
   **else** `parse_tenant_from_html` (lib.rs:325, dynamic discovery) →
   if `abck_state == NeedsSensor`, drain `__akamai_events`
   (`DRAIN_JS`), call `send_akamai_sensor_data` once (`MAX_POSTS=1`).
   **WIRED but reached only for genuine `NeedsSensor` BMP sites** (none
   of the 11 currently — bestbuy renders, homedepot is sec-cpt-gated).
5. `send_akamai_sensor_data` → `build_sensor_data` (lib.rs:502) →
   `build_cleartext_v3_json` + `build_v3_for_host` →
   `crypto::build_v3_envelope`. **WIRED; effectively unreached at
   runtime for the 11.**

**Component status (the §4/§8 dead-code mandate):**

| Component | File:line | Status |
|---|---|---|
| `ParsedAbck`/`AbckState`/`AkamaiSessionStore` | session.rs | **WIRED & exercised** (learn_abck, abck_state) |
| `learn_abck`/`send_akamai_sensor_data` | net/lib.rs:362,392 | **WIRED & exercised** |
| `parse_tenant_from_html` + helpers | lib.rs:325-443 | **WIRED**, reached only on dynamic-tenant BMP path |
| `started_as_seccpt_challenge` gate | page.rs:1432,1812 | **WIRED & exercised** (homedepot flip) |
| sec-cpt guard on `self.content()` | page.rs:470-476 | **WIRED but redundant/hazardous** (see §10b) |
| `build_v3_envelope`/`shuffle_tokens_v3`/`substitute_chars_v3` | crypto.rs:271-326 | wired-but-effectively-unreached (byte-verified) |
| `build_cleartext_v3_json`/`V3Payload` | v3_payload.rs | wired-but-effectively-unreached; **content static** |
| `sec_cpt::solve_crypto` + `SecCptChallenge` + `find_answer` | sec_cpt.rs:80-120 | **DEAD CODE — zero non-test callers** (grep confirmed) |
| `build_v2_bestbuy` / `build_v2_dalphan` | crypto.rs:212,339 | **DEAD CODE** — only tests + `pub use` re-export; live path is v3 |
| `payload::build_cleartext` (v2 58-elem tAD) | payload.rs:64 | **DEAD CODE** — only tests + `pub use`; superseded by v3 JSON |
| `reverse_substitute` / `reverse_shuffle` | crypto.rs:139,173 | **DEAD CODE** — zero callers, not even tests |
| `BotScoreVector::parse` (ak_p) | lib.rs:103 | **DEAD CODE** — zero callers anywhere; never parsed from a live `Server-Timing` |
| `tea_cbc_*` / `derive_tea_key_candidate_a` | tea_cbc.rs | **DEAD CODE** + **misfiled** (Kasada code in the akamai crate; zero callers) |
| `datadome_crypto::DdEncryptor` | datadome_crypto.rs | **DEAD CODE** (DataDome, not Akamai; zero callers — noted, not my engine) |

---

## 9. GAP ANALYSIS — what we are missing (ranked, concrete)

> Framing correction (verify-don't-assume): the master plan §Phase-3
> proves G3/G4 **flip zero of the 11 sites** (10 render without the
> sensor; homedepot is sec-cpt). They are real divergences but **low
> blast radius**. Ranked by *honest* leverage:

**GAP-1 (highest leverage, but not our crate): homedepot sec-cpt
bundle does not self-solve in our V8.** Evidence: master plan §Phase-5
"homedepot looping on 2.6 KB sec-cpt … `/Wjv3…` bundle does not
self-solve" (pre-`b623d5d`); post-fix it *does* once the BMP POST stops
fighting it (`b623d5d` → `L3-RENDERED`). Blast radius: **1/11**
(homedepot). Difficulty: **L** (Phase-5 in-engine bundle execution; the
network-free §4 gate structurally cannot verify a daily-rotating
oracle). Risk: high (nav-loop changes). Concrete fix: keep the
`started_as_seccpt_challenge` BMP-suppression (done); the residual is
the strategic Phase-5 "execute vendor bundle to solution in V8"
capability + a nav-continuation so the post-sec-cpt page proceeds to
the full homepage (currently `len=2507` intermediate). **Do not** wire
`solve_crypto` — homedepot serves no parseable 428 (GAP-FP-1 §10b).

**GAP-2: v3 cleartext content is ~90% static placeholder** (`wsl`,
`din`, `mst`, `fpt`, `ajr`, `ver`, `kev=""`, `dme=""`). Evidence:
`v3_payload.rs:98-174` [CODE]; master plan G3. Blast radius:
**0/11 today** (no site is blocked *by this*; would matter only if a
target escalated to v3 BMP scoring AND we had its fileHash). Difficulty:
M. Risk: med (regression to the 10 passing). Concrete fix: a `WSL_JS`
drain sibling reading `performance.memory.*`,
`navigator.plugins[0][0].enabledPlugin`, `plugins.refresh`,
`plugins.item(4294967296)`, `SharedArrayBuffer`,
`speechSynthesis.getVoices().length`, plus `din`/`fpt`/`ajr` from
`StealthProfile` (screen/tz/cores/UA) — **but gate behind the same
narrow `NeedsSensor` path so it cannot touch the 10 renderers.**

**GAP-3: fileHash registry covers 3 hosts, 8 fall back to wrong
value.** Evidence: `lib.rs:229-252` (bestbuy/macys/homedepot only;
none overlaps the 8 unmanned target sites) [CODE]; master plan G4.
Blast radius: **0/11 today** (the 8 unmanned sites render anyway).
Difficulty: L (Rust port of glizzy Babel-AST walker over rotating
obfuscated JS — fragile). Risk: med. Concrete fix: only worth it
*paired with* GAP-2 and only if a target ever becomes v3-BMP-blocked;
otherwise pure cost.

**GAP-4 (engine-wide, real): JA4-vs-UA incoherence — TLS pinned
Chrome 147, preset UAs Chrome 148.** Evidence: `tls.rs:1`/`h2_client.rs:1`
say "Chrome 147"; `net/headers.rs` UA = Chrome 148 [CODE]; master plan
G7; asadfix 2026 ("JA4 vs UA cross-check"). Blast radius: ALL edge
gates incl. all 11 Akamai (Phase-1 dominant). Difficulty: S/M. Risk:
med. Concrete fix: re-pin TLS+H2 to a Chrome 148 capture
(`capture_chrome_148_hello.rs` exists) OR roll preset UA back to 147 —
and add a self-asserting JA4-vs-UA test so it cannot silently drift.
**This is the only gap that plausibly improves the Phase-1 score that
actually gates all 11.**

**GAP-5: behavioral telemetry is the weak `humanize.js`, not the
Plamondon/Sigma-Lognormal `stealth::behavior` model; `kev`/`dme`
empty.** Evidence: master plan G8 (DEFERRED with rationale —
`humanize.js` already has an inline sigma-lognormal model; behavioral
is not the blocker for any hard site); `v3_payload.rs:132,261-263`
[CODE]. Blast radius: 0/11 today. Difficulty: M. Risk: med (feeds
`__akamai_events` → regression risk to the green gate). Defer per the
master plan's documented reasoning.

**GAP-6 (PQ-TLS): NON-GAP — explicitly verified.** 2026 intel flagged
missing X25519MLKEM768 as a 2026-01-31 kill. **We send it first with
`set_key_shares_limit(2)`** (`tls.rs:94-100,299-300`). Recorded here so
no future agent re-chases it. (Safari-iOS preset intentionally has no
PQ — correct until iOS 26.)

---

## 10. FALSE-POSITIVE ANALYSIS of our code

### 10a. Detection FPs (classification mislabels)

**FP-DET-1 (the load-bearing one, CONFIRMED then FIXED).** Old
`is_anti_bot_challenge()` body-substring matcher counted bare `bm_sz`
on multi-MB *rendered* pages as "blocked" ⇒ **10 of 11 Akamai sites
were false-positive "blocked"** (master plan §Phase-0.2). Fix: typed
`ChallengeVerdict` + `body_has_challenge_marker` (`page.rs:120-194`)
gates weak markers (`bm_sz`, `human security`, `dd_engagement`) behind
`body.len() < 50 KB` [CODE]. Verified: the 10 now classify `pass`
(0.4–3 MB real renders). **Residual over-match risk:** strong markers
`_abck && sensor_data` and `sec-if-cpt-container`/`sec-cpt-if` are
NOT size-gated — a fully-rendered page that legitimately inlines
`sensor_data` in JS *and* the string `_abck` would still be flagged
EdgeBlock/SensorFail. Low probability (both substrings together on a
benign large page is rare) but unbounded. Test that would catch:
a fixture of a *rendered* Akamai homepage (>1 MB, the
`ab_harness/.../homedepot_.html` capture) asserting
`challenge_verdict()==Pass`.

**FP-DET-2 (under-match / thin-shell, the *other* direction).**
`challenge_verdict()` (`page.rs:274-292`): no marker + `len ≥ 5 KB` ⇒
`Pass`. bestbuy `pass` at **7.8 KB** is almost certainly a thin shell,
not the real homepage (master plan §Phase-0.2 explicitly caveats
bestbuy/spotify/duolingo). Also `len ≥ 50 KB` with a marker ⇒
`SensorFail` not `EdgeBlock` — a 50 KB+ challenge would be mislabelled.
Claimed-false: "bestbuy passes Akamai". Why false: 7.8 KB ≠ a rendered
retail homepage (real renders here are 0.4–3 MB). Test: assert a
content-depth heuristic (DOM node count / presence of product-grid
selectors) for any sub-50 KB `pass`, not just byte length.

**FP-DET-3 (classifier divergence).** `holistic_sweep.rs::classify`
and `audit_failing_sites.rs` use *different* thresholds than
`page.rs::challenge_verdict` (master plan §reconciliation; the
homedepot flip is asserted via `holistic_sweep::classify`
"no challenge substring + len ≥ 1000 ⇒ L3-RENDERED"). Claimed-false
risk: homedepot `len=2507` "RENDERED" — true by `classify` but it is a
*post-sec-cpt intermediate page*, not the full homepage (master plan
explicitly: "rigor caveat … not yet a full content render"). The
"homedepot flipped" claim is honest **only** by the sanctioned metric;
calling it "homedepot fully won" would be an FP. Test: a second
verdict tier that requires content-depth, applied identically by both
classifiers.

### 10b. Solver / logic FPs

**FP-CODE-1 (the canonical "exists ≠ exercised"): `sec_cpt::solve_crypto`
is DEAD CODE.** `crates/akamai/src/sec_cpt.rs:80`. Grep: only callers
are `sec_cpt.rs:141,175` (its own tests). The false impression: 02/12
docs + the module header imply a working sec-cpt solver; three unit
tests pass (`solves_low_difficulty_within_seconds` etc.). Why false:
zero production callers; `handle_akamai_flow` returns `NeedsSecCpt`
and **never** constructs a `SecCptChallenge` or calls `solve_crypto`.
Worse — it is **un-feedable for our actual target**: homedepot serves
no parseable 428 JSON (no `challenge="<b64>"`, no
`nonce`/`difficulty`/`verify_url`), so even if wired it has no input
(master plan §Phase-3/5, confirmed against the `SecCptChallenge`
schema). Test that would have caught it: a `#[test]` asserting at least
one non-`#[cfg(test)]` caller exists (a "no orphan solver" lint), or an
integration test feeding a captured homedepot body to the sec-cpt path
and asserting it produces *something* (it cannot).

**FP-CODE-2: `build_v2_bestbuy` / `build_v2_dalphan` /
`payload::build_cleartext` are DEAD CODE.** crypto.rs:212,339;
payload.rs:64. Only callers are tests + the `pub use` re-export
(lib.rs:64,66). The live path is exclusively v3
(`build_sensor_data → build_cleartext_v3_json → build_v3_for_host`,
lib.rs:513-544). False impression: the crate header + 02_AKAMAI.md §0
describe v2 as the implemented format; ~600 LOC + ~20 passing tests
of v2 crypto/payload give a false sense of "we have a sensor". Why
false: v2 is a near-dead deployment in 2026 (02_AKAMAI.md §3) and our
runtime never builds a v2 body. Test: same orphan-solver lint;
or delete v2 modules and confirm the workspace still builds (it will).

**FP-CODE-3: redundant + hazardous sec-cpt guard on mutable
`self.content()`.** `page.rs:470-476` inside `handle_akamai_flow`
re-checks `body.contains("sec-if-cpt-container")` on the **post-mutation
DOM**. This is the *exact doc-20 anti-pattern* the `b623d5d` fix
diagnosed: after the bundle mutates the DOM the marker is gone, the
guard misses, and (pre-fix) the wrong BMP POST fired. The persistent
`started_as_seccpt_challenge` (page.rs:1432, from the *original* HTML)
now short-circuits *before* `handle_akamai_flow` is even called
(page.rs:1812), so :470-476 is **dead in practice for the homedepot
path** but remains a live mutable-state landmine for any future caller
that invokes `handle_akamai_flow` directly without the outer gate.
False claim it embodies: "this guard prevents the wrong sec-cpt POST".
Why false: it reads post-mutation DOM; it is the bug, not the fix.
Test: a unit test that mutates the DOM to remove
`sec-if-cpt-container` *then* calls the flow and asserts no BMP POST is
issued (would fail today if the outer gate were removed). Recommended
fix: delete :469-477 and rely solely on the persistent signal, or pass
`started_as_seccpt_challenge` into `handle_akamai_flow`.

**FP-CODE-4: `BotScoreVector` advertised as a regression detector,
never invoked.** lib.rs:83-146 + 4 passing tests; 02_AKAMAI.md §8.5/§12.8
call it "observable! use as a regression detector". Grep: **zero
callers** — no code path ever calls `BotScoreVector::parse` on a live
`Server-Timing: ak_p` header. The `ak_p` 6-sub-score signal (the single
best passive regression oracle Akamai hands us for free) is **collected
by Akamai and discarded by us**. False claim: "we monitor ak_p for
fingerprint regressions". Why false: dead code. Test: an integration
assertion that a sweep run records a non-empty `BotScoreVector` for at
least one Akamai response (would fail — nothing parses the header).

**FP-CODE-5: `tea_cbc` + `derive_tea_key_candidate_a` are dead AND
misfiled.** tea_cbc.rs (218 LOC, Kasada TEA-CBC) lives in
`crates/akamai/` with zero callers anywhere. Not an Akamai mechanism at
all (Kasada §2.1). Pure dead weight inflating the akamai crate's
apparent capability. Test/fix: move or delete; orphan-module lint.

**FP-CODE-6 (offline-passes / live-differs): `build_v3_envelope`
byte-parity tests pass, but the *cleartext content* is static, so the
asserted "v3 pipeline correct end-to-end" is true only for the
*encryption*, not for passing a site.** crypto.rs:413 asserts our
envelope == glizzy's reference *for a fixed 3-key test input*. The
test is correct; the **inference** "therefore our sensor works" is the
FP — the live VM assembles a different body (akid, script-hash,
30–48 sections, runtime fields) that no static cleartext reproduces
(§4, adidas probe, xkiian). False claim: green crypto tests ⇒ Akamai
solvable. Why false: crypto ≠ content; content is unreproducible
statically. Test: a documented assertion in the module that
byte-parity covers encoding ONLY, plus the §11 verification regime
note that this can never be network-free-verified to "pass a site".

---

## 11. The concrete pass-guarantee plan (all 11 Akamai sites)

**Reality check first.** 10/11 already render. "Pass-guarantee" for
those = **do not regress them** + verify the thin-shell passes
(bestbuy 7.8 KB) are real content. homedepot is the only one that was
genuinely blocked and is now flipped (intermediate render).

Ordered steps:

1. **Lock the 10 renderers (P0, network-free-ish).** Add a
   content-depth verdict tier (DOM node count / known retail-grid
   selector) so a sub-50 KB `pass` is not counted a win (closes
   FP-DET-2). Add the rendered-homedepot capture as a
   `challenge_verdict()==Pass` fixture (closes FP-DET-1 residual).
   *Verifies: classifier integrity.*

2. **GAP-4 JA4-vs-UA coherence (P0, the only Phase-1 lever that touches
   all 11).** Re-pin TLS+H2 to Chrome 148 (capture exists) or roll
   preset UA to 147; add a self-asserting JA4↔UA drift test
   (`tls.rs` already has `tls_fingerprint_vectors_no_silent_drift` —
   extend it to cross-check the UA major). *Verifies: a JA4-bucket
   assertion test; Phase-1 score is the dominant gate per 2026 intel.*

3. **homedepot nav-continuation (P1, L, Phase-5-adjacent).** Keep
   `started_as_seccpt_challenge` BMP-suppression. After the bundle
   self-solves and `sec_cpt` cookie appears, ensure the nav loop
   *continues to the real homepage* (current result is the 2.5 KB
   post-challenge intermediate). This is a routing/continuation change,
   **not** wiring `solve_crypto`. *Verifies: live `h_store_homedepot`
   re-measure to a full multi-MB render — explicitly a LIVE-oracle
   check; the network-free §4 gate structurally cannot verify it.*

4. **Delete the dead weight (P1, pure hygiene, gate-safe).** Remove or
   `#[cfg(test)]`-quarantine `solve_crypto`/v2 crypto/v2
   payload/`reverse_*`/`tea_cbc`/`BotScoreVector` **or** wire the two
   that have real value: (a) `BotScoreVector::parse` into the sweep as
   a passive `ak_p` regression oracle (cheap, high diagnostic ROI,
   zero nav risk); (b) delete FP-CODE-3's mutable-state guard.
   *Verifies: workspace builds; orphan-solver lint added; full §4 gate
   stays green.*

5. **GAP-2/3 (P2, conditional — only if a target ever escalates to
   v3-BMP-block).** Populate `wsl`/`din`/`fpt` from the live surface
   and port the fileHash AST extractor — **strictly gated behind the
   `NeedsSensor` BMP path** so it cannot touch the 10 renderers.
   *Verifies: only by live navigation against a v3-BMP-blocked target;
   none exists in the 11 today ⇒ this is parked, not pending.*

**Verification regime & where it structurally fails.** Steps 1, 2, 4
are network-free-verifiable (classifier fixtures, JA4 vector test,
build/lint, §4 gate). Step 3 (homedepot full render) and Step 5
(v3 content scoring) are **only** verifiable against a live,
daily-rotating Akamai oracle — the mandated network-free §4 gate
**cannot** prove a sec-cpt/v3 site flip (no daily key, no live edge).
This is the same structural limit the master plan reaches: the residual
Akamai work is a **live-oracle dev loop**, not a gate-checkable commit.
Any agent claiming "homedepot fully passes" from a green offline gate
is committing FP-DET-3.

---

## 12. Sources & experiments

### External sources (URL + what it claims + accessed 2026-05-16)

- docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt —
  sec-cpt 428 flow; `chlg_duration` server-enforced; success ⇔
  `sec_cpt` ends `~3~`; POST `/_sec/verify?provider=crypto`.
- github.com/Hyper-Solutions/hyper-sdk-py/blob/master/hyper_sdk/akamai/sec_cpt.py
  — exact PoW: `challenge="(.*?)"` b64+JSON parse, `0.<hex>` answer,
  `sec+timestamp+nonce+difficulty+answer`, rolling sha256 %difficulty,
  `difficulty+=1`, `time.sleep(duration)`. **Our `sec_cpt.rs` matches
  byte-for-byte.**
- github.com/Hyper-Solutions/hyper-sdk-go — sensor generation is
  **server-side behind a paid API key**; only parsing helpers local.
- github.com/jesterfoidchopped/akamai-v3-sensor — pure TLS+H2+H3
  fingerprint, no sensor gen; 3 commits, dormant; bets Phase-1 yields
  `~0~` `_abck`.
- scrapfly.io/blog/posts/post-quantum-tls-bot-detection — PQ key
  exchange default for all Akamai client connections **2026-01-31**
  (full rollout March 2026); X25519MLKEM768 adds 1088 B to ClientHello;
  absence = "outside normal browser baseline", pre-application kill.
- asadfix.github.io/scraping-guide — JA4+ replaces JA3; HTTP/2 SETTINGS
  leak; "fix Layers 1–3 and you never reach Layer 4"; Akamai 60
  chrome-extension `fetch()` probes; static sensor RE is wrong layer.
- scrapfly.io/bypass/akamai, zenrows.com/blog/bypass-akamai — Akamai
  bypass guides (marketing-heavy; technique notes only).
- medium.com/@glizzykingdreko/akamai-v3-sensor-data-…-da0adad2a784 —
  v3 two-stage crypto (elementSwapping + characterSubstitution),
  cookieHash(bm_sz)+fileHash(AST). **Article 404'd this session**;
  algorithm preserved in our `crypto.rs` byte-parity tests + tooling
  akamai-v3-tools.vercel.app.
- dev.to/xkiian/…-5cjh "deobfuscator is a trap" — **URL 404'd this
  session**; thesis corroborated via asadfix + scrapfly + our adidas
  probe (VM dispatch/akid/script-hash are runtime-bound).

### Local sources read (file:line)

- `crates/akamai/src/{lib,crypto,session,payload,sec_cpt,v3_payload,
  drain,tea_cbc,datadome_crypto}.rs` (full).
- `crates/browser/src/page.rs` :120-194, :263-292, :447-601,
  :1420-1432, :1790-1920, :2222-2304.
- `crates/net/src/lib.rs` :355-447; `crates/net/src/tls.rs`
  :235-331, :93-159, :399-406; `crates/net/src/h2_client.rs` :1-130.
- `crates/browser/src/js/humanize.js` (akamai event taps).
- `docs/research_2026_05_14/02_AKAMAI.md`,
  `…/10_AKAMAI_V3_ENVELOPE_DEEP_2026_05_14.md`,
  `docs/research_2026_05_16/{00_MASTER_PLAN,05_NON_KASADA_VENDORS}…md`,
  `docs/universal_engine/site_debugging/{homedepot,adidas}_akamai_bmp_v3.md`.

### Local experiments (command + result)

- `grep -rn "solve_crypto|build_v2_*|build_cleartext|reverse_*|
  BotScoreVector|tea_cbc|datadome_crypto" --include=*.rs .` →
  **confirmed zero non-test callers** for `solve_crypto`,
  `build_v2_bestbuy/dalphan`, v2 `build_cleartext`, `reverse_*`,
  `BotScoreVector::parse`, `tea_cbc_*`, `DdEncryptor` (FP-CODE-1/2/4/5,
  §8/§10b).
- `grep -E "<script[^>]*>" ab_harness/shots/https_www_homedepot_com_.html`
  + marker grep → 1 030 650 B = a **rendered** homedepot (no
  `sec-if-cpt-container`; `/Wjv3muMJul/…` bmak.js + deferred `?v=<uuid>`
  telemetry tag present) — corroborates the `b623d5d` flip and that
  `parse_sensor_post_path` would correctly extract the deep path.
- `grep MLKEM/X25519/key_shares crates/net/src/tls.rs` →
  `CURVES_DESKTOP = [X25519_MLKEM768, X25519, SECP256R1, SECP384R1]`
  + `set_key_shares_limit(2)` → **PQ-TLS is a verified NON-gap**
  (GAP-6).

No `cargo` builds/sweeps run (build-lock discipline; network tests are
`#[ignore]`). All "pass/block" statements are sourced to the
Phase-0.2 typed re-baseline + code reads, never re-measured here.

— End —
