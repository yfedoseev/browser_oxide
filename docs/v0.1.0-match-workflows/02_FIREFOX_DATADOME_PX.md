# 02 — firefox_135_macos: the DataDome / PerimeterX / nav-error cluster

**Profile under analysis:** `firefox_135_macos` — the weakest of the 4 BO
profiles (**106 / 125**, vs chrome 110 / pixel 108 / iphone 108).
**Workflow goal:** bring firefox to ~112-116 (v150 single-engine parity) and
make the routed-115 sites pass on firefox too.
**Method:** root-cause from the captured per-profile tag/len matrix + BO source
(no live navs — competitor benchmark is running on the single IP).
**Date:** 2026-05-29.

---

## 0. TL;DR / headline

Every firefox-only gap-fail in the matrix has **one shared root cause**: the
firefox profile puts a **coherent Firefox UA + Firefox HTTP headers on the wire
over a genuine Chrome 147 TLS ClientHello AND a genuine Chrome 147 HTTP/2
SETTINGS frame.** That is the single worst shape for a 2026 anti-bot edge,
because every major vendor (Cloudflare, Akamai, DataDome, PerimeterX/HUMAN)
now computes **JA4 (and akamai-H2) BEFORE looking at the User-Agent** and
cross-checks the two. A request whose JA4 says "Chrome 147" but whose UA says
"Firefox 135" is the textbook fake-browser signature these systems were *built*
to catch.

The firefox failures split into two surface forms of the **same** mismatch:

| Form | Sites | What the vendor does with the mismatch |
|---|---|---|
| **Challenge served** | reuters / wsj / tripadvisor (DataDome-CHL), zillow (PerimeterX-PaH), spotify (thin 9875) | edge scores the JA4≠UA request as bot → serves the interstitial/PoW path instead of content |
| **Hard drop / empty** | macys (THIN-BODY 0) | Akamai edge RST/empties the JA4≠UA Firefox request → 0-byte body, no challenge HTML at all |

**The fix is one piece of engineering** — give the firefox profile a *real
Firefox 135 wire class* (TLS ClientHello + HTTP/2 SETTINGS), which BO does not
have today. This is exactly leak **L1** in
`docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md` (already flagged
as "the cleanest single network-layer leak in BO today" and FIX-N3). This doc
is the per-profile, per-site evidence that L1 *is* the firefox-106 ceiling, plus
the concrete reference values to build it.

---

> **✅ LIVE-CONFIRMED + ⚠️ IMPLEMENTATION BLOCKER (2026-05-29).** A live
> `tls.peet.ws` capture (`crates/net/tests/tls_fingerprint.rs::capture_profiles_ja4`)
> **confirms the diagnosis exactly**: `firefox_135_macos` emits
> `ja4 = t13d1516h2_8daaf6152771_d8a2da3f94cd` — **byte-identical to
> `chrome_148_macos`** — and `ja4h = 1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`,
> also identical to Chrome. So the firefox profile is 100% Chrome on the wire at
> BOTH the TLS and HTTP/2 layers. `tls.rs::chrome_connector` branches only on
> `device_class` (firefox = `Desktop` → Chrome); the discriminator for a Firefox
> branch would be `profile.tls_impersonate.starts_with("firefox")`.
>
> **Authoritative Firefox 135 reference** (lexiforest `firefox_135.0.1_linux.yaml`,
> TLS is OS-independent): ciphers `4865,4867,4866,49195,49199,52393,52392,49196,49200,49162,49161,49171,49172,156,157,47,53`
> (17, CHACHA before AES256, **no 3DES**); extensions incl. **delegated_credentials
> (0x22), record_size_limit (0x1c), real ECH (0xfe0d), session_ticket**, **no
> GREASE, no padding**; curves `4588(MLKEM768),29,23,24,25,256(ffdhe2048),257(ffdhe3072)`;
> H2 SETTINGS `1:65536;2:0;4:131072;5:16384` (**no ID3**), WINDOW_UPDATE 12517377,
> pseudo-order **m,p,a,s**, + `priority`/`te` headers.
>
> **BLOCKER:** a faithful Firefox ClientHello needs `delegated_credentials`,
> `record_size_limit`, real ECH, FFDHE groups, and **GREASE/padding suppression**.
> BO's Chrome/Safari `*_EXTENSION_PERMUTATION` tables don't reference those
> extensions, and boring2 4.15.15's safe builder API (as used in `tls.rs`) doesn't
> expose them — so their kExtensions indices are unknown and may be absent. A
> *partial* Firefox JA4 (right ciphers, Chrome-ish extensions) is a **novel,
> near-zero-corpus fingerprint — WORSE than the current high-corpus Chrome JA4**
> (the §2.1 rarity principle that drove the iphone #26 fix). Therefore the TLS
> layer is **all-or-nothing** and must NOT be shipped partially.
>
> **Turnkey next step (when unblocked):** locate boring2 4.15.15's kExtensions
> table indices for `delegated_credentials`/`record_size_limit`/`ECH`/`FFDHE`
> (or use raw ClientHello injection like the deferred padding work), build the
> Firefox branch, then iterate JA4 against `tls.peet.ws` (IP-safe) until it equals
> the real Firefox 135 JA4 BEFORE any anti-bot site test. The H2 layer
> (`h2_client.rs`) is independently achievable (BO owns its H2 stack) and safe to
> ship: Firefox SETTINGS (no ID3, window 12517377) + pseudo-order m,p,a,s.

## 1. The data signature (why this is the wire layer, not JS)

From `docs/v0.1.0-match-workflows/00_DATA_per_profile_matrix.md`:

| site | chrome | pixel | iphone | firefox | vendor |
|---|---|---|---|---|---|
| reuters | ✅ 1138793 | ✅ 1161144 | ✅ 1126171 | **DataDome-CHL 1456** | DataDome |
| wsj | ✅ 691418 | ✅ 285500 | ✅ 287970 | **DataDome-CHL 1461** | DataDome |
| tripadvisor | ✗ DataDome-CHL 1412 | ✅ 383111 | ✅ 290654 | **DataDome-CHL 1464** | DataDome |
| zillow | ✅ 441828 | ✅ 402231 | ✅ 402267 | **PerimeterX-PaH 14558** | PerimeterX/HUMAN |
| macys | ✅ 1537880 | ✅ 1269917 | ✅ 1269833 | **THIN-BODY 0** | Akamai |
| spotify | ✗ 9881 | ✅ 147739 | ✅ 147724 | **9875 (thin)** | (PX/edge) |

Two observations pin this to the **network layer**, not the JS/DOM/fingerprint
layer:

1. **The failures are mobile-or-Chrome-pass / Firefox-fail, and the passing
   profiles are exactly the ones with a coherent wire class.** chrome / pixel
   (Chrome desktop / Chrome Android) emit a *genuine Chrome* ClientHello+H2;
   iphone emits a *genuine Safari* ClientHello+H2 (BO built both — see §2).
   Only firefox emits the wrong-class wire bytes. The JS surface BO presents for
   firefox (vendor="", productSub 20100101, masked WebGL, Firefox accept/q=0.5
   headers) is *correct Firefox* — so if the JS layer were the problem, chrome
   and iphone (which have *different* JS surfaces) wouldn't uniformly pass. The
   only variable that isolates the firefox failures is the **wire class**.

2. **reuters/wsj pass on chrome AND both mobiles but fail ONLY firefox.** If
   reuters/wsj were genuinely hard DataDome targets, they'd fail more broadly
   (compare tripadvisor, which also fails chrome). They fail *only* the one
   profile whose TLS/H2 disagrees with its UA. That is a per-profile wire tell,
   not a site-difficulty tell.

3. **macys = THIN-BODY 0** (not a challenge page). A 0-byte body is the
   signature of an **edge-layer drop** — the request never reaches the
   content/challenge tier. Akamai (which fronts macys) is the most aggressive
   JA4-vs-UA enforcer in the corpus; a Firefox-UA-over-Chrome-JA4 request is
   dropped at the edge before any HTML (challenge or content) is generated. The
   *same boring2 ClientHello* connects fine for the chrome profile (macys chrome
   = 1.54 MB), so this is **not** a TLS handshake/connect bug in BO's stack — it
   is the vendor rejecting the *coherence*, not the *connection*.

---

## 2. Code root cause (file:line)

### 2.1 BO has NO Firefox wire class — TLS and H2 both branch only on `device_class`

**TLS** — `crates/net/src/tls.rs::chrome_connector` (`tls.rs:233-369`) branches
**only** on `profile.device_class`:

```rust
let is_safari_ios = profile.device_class == DeviceClass::MobileIOS;
let curves: &[SslCurve] = match profile.device_class {
    DeviceClass::MobileAndroid => CURVES_ANDROID,   // = CURVES_DESKTOP
    DeviceClass::MobileIOS     => CURVES_SAFARI_IOS,
    DeviceClass::Desktop       => CURVES_DESKTOP,    // ← firefox lands HERE
};
```

`firefox_135_macos` sets `device_class: DeviceClass::Desktop`
(`presets.rs:473`). So the firefox profile gets the **full Chrome 147 desktop
ClientHello**:
- `CIPHER_LIST` (15 Chrome ciphers, `tls.rs:60-76`)
- `CURVES_DESKTOP` with **`X25519_MLKEM768` leading** (`tls.rs:91-96`) — Firefox
  135 does **not** ship desktop MLKEM in this position
- `SIGALGS_LIST` (Chrome's 8, `tls.rs:79-88`)
- **Fisher-Yates extension shuffle** of the 16-element Chrome set
  (`tls.rs:222-228`, `tls.rs:347-367`) — Firefox does **not** Fisher-Yates
- **Brotli** cert compression (`tls.rs:312-319`) — Firefox negotiates differently
- **ALPS** + **ECH grease** (`tls.rs:387-426`) — Firefox emits neither

**HTTP/2** — `crates/net/src/h2_client.rs::handshake` (`h2_client.rs:78-181`)
branches **only** on `is_safari_ios`. Every non-iOS profile (including all three
`firefox_135_*`) takes the Chrome arm:
- SETTINGS `1:65536; 2:0; 4:6291456; 6:262144` (`h2_client.rs:39-42`)
- pseudo-header order **`m,a,s,p`** (`h2_client.rs:98-104`)
- INITIAL_WINDOW_SIZE **6 291 456** (`h2_client.rs:41`), wire connection-window
  **15 663 105** (`h2_client.rs:50`)
- HEADERS priority hint **weight=255, exclusive=true, dep=0**
  (`h2_client.rs:167-174`)

### 2.2 `tls_impersonate: "firefox_135"` is aspirational — it is never read

`firefox_135_macos` sets `tls_impersonate: "firefox_135"` (`presets.rs:474`) and
the preset's own comment is explicit (`presets.rs:464-474`):

> "String token — currently informational only. The actual TLS bytes are emitted
> by `crates/net` via boring2/BoringSSL with a Chrome-tuned ClientHello. A real
> Firefox JA4 swap requires reconfiguring boring2's cipher list / extension
> order to match NSS — substantial work tracked as a future item."

Grep confirms `tls.rs` and `h2_client.rs` never read `tls_impersonate`; both
branch on `device_class` alone. So the firefox profile is **Firefox at the UA
+ header layer, Chrome at the wire layer** — a guaranteed JA4≠UA mismatch.

### 2.3 The HTTP header layer IS correct Firefox (this is what makes the leak so sharp)

`crates/net/src/headers.rs` *does* have a real Firefox branch:
`nav_headers` dispatches `browser_name == "Firefox"` → `firefox_headers`
(`headers.rs:17-18`), and `firefox_headers_impl` (`headers.rs:660-706`) emits a
faithful Firefox request: short `accept`
(`text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8`), `q=0.5`
accept-language (`build_firefox_accept_language`), no `sec-ch-ua*`, no
`priority` header, `upgrade-insecure-requests: 1`. This is *good* — but it
**sharpens** the leak: the request now tells a perfectly coherent Firefox story
in the UA + headers, then contradicts it completely in the JA4 + akamai-H2. A
vendor that sees "Firefox headers + Firefox UA + Chrome JA4 + Chrome H2" has an
unambiguous fake-browser fingerprint. (A *less* careful header layer would
actually be less suspicious, because it wouldn't assert "Firefox" as loudly.)

---

## 3. Why each firefox site fails — mapped to the mismatch

### 3.1 DataDome (reuters, wsj, tripadvisor)

DataDome scores every request with an ML model fed by **TLS fingerprint +
browser fingerprint + behavior + IP** (VENDOR_datadome.md §2.2; ZenRows/Scrapfly
2026). Per the 2026 sources, **"Cloudflare, Akamai, and DataDome all check JA4
before they even look at the user-agent,"** and DataDome maintains a database of
JA4 hashes mapped to client types. A Firefox-135 UA arriving with a *Chrome-147
JA4* is an immediate trust-score collapse → DataDome serves the interstitial/PoW
path (`DataDome-CHL`, 1456-1464 bytes) instead of content.

Crucially, this is a **different failure mode** from the tripadvisor/etsy
*content* DataDome gap analyzed in `VENDOR_datadome.md` (the daily-key WASM
self-solve). reuters and wsj **pass on chrome + both mobiles** — they are *not*
hard DataDome targets. They fail firefox **purely on the JA4≠UA pre-filter**:
the request is bounced to CHL before the page (or any self-solving bundle) even
runs. So reuters/wsj firefox is fixed by the wire class alone (§4 FIX-FF1), with
no need for the harder DataDome self-solve work. tripadvisor firefox is the same
pre-filter bounce (it also fails chrome for the separate content reason, but its
firefox-fail is the wire mismatch).

### 3.2 PerimeterX / HUMAN (zillow) — `PerimeterX-PaH 14558`

zillow passes chrome + both mobiles (genuine wire classes) and fails **only**
firefox with a PerimeterX "Press & Hold" challenge (`PaH`, 14 558 bytes — note
this is a *real challenge page*, not a 0-byte drop, so the request reached the
PX tier but was scored bot). PerimeterX/HUMAN's `_px3`/sensor pipeline ingests
the TLS fingerprint as a first-class signal (Scrapfly/ZenRows PerimeterX 2026
guides). Same root cause: the Chrome-147 JA4 contradicts the Firefox UA → PX
escalates to the interactive PaH gate. The 14 558-byte body is the PaH
interstitial, consistent with a *fingerprint*-driven escalation (not an
IP/behavior block, since the other three profiles from the same IP pass clean).

### 3.3 Akamai hard drop (macys) — `THIN-BODY 0`

macys is Akamai-fronted and passes chrome (1.54 MB), pixel/iphone (~1.27 MB).
firefox gets **THIN-BODY 0** = a 0-byte body, i.e. the request was
**dropped/RST at the edge** before any HTML. Akamai is the corpus's most
aggressive JA4-vs-UA enforcer; a Firefox-UA / Chrome-JA4 / Chrome-akamai-H2
request is rejected at the wire tier rather than challenged. This is **not** a BO
connection bug: the identical boring2 Chrome ClientHello connects fine for the
chrome profile against the same host. The variable is the *coherence*, not the
*connection*. (BO's net stack does have a stale-conn retry and H1 fallback —
`lib.rs:796-859` — but neither helps when the server deliberately returns an
empty body to a coherence-failing fingerprint; a retry re-sends the same
mismatched bytes.) Fixed by the wire class (§4 FIX-FF1).

### 3.4 spotify (thin 9875)

spotify fails **both desktop profiles** (chrome 9881, firefox 9875) and passes
**both mobiles** (147 KB). The chrome-desktop thin body means spotify's edge is
serving a degraded/gate page to *desktop* class generally — so spotify is only
**partly** a firefox-wire issue (the firefox-vs-mobile delta is consistent with
the wire mismatch, but the chrome-desktop thin shows a second, desktop-class
factor independent of firefox). Treat spotify as **lower-confidence** for the
firefox wire fix: FIX-FF1 may close the firefox↔mobile gap, but the
chrome-desktop thin needs separate investigation (likely a desktop content gate
/ geolocation / Accept-CH issue, out of scope for this firefox doc). Do **not**
count spotify as a guaranteed firefox flip.

---

## 4. The fix: build a real Firefox 135 wire class

This is `NETWORK_fingerprint.md` **FIX-N3** ("Build the real Firefox 135 TLS +
HTTP/2 class"), scoped here with concrete reference values and per-site expected
gain. All of it lives in public `crates/net` (per `CLAUDE.md` "HTTP/TLS: own
stack in `crates/net/`") — none of it is per-vendor solver code, so it is
**public-engine**.

### Reference values to transcribe (confirmed from external sources)

**Firefox 135 HTTP/2** (from `0x676e67/wreq-util` `firefox/http2.rs` +
Scrapfly/Trickster 2026 + Akamai H2 whitepaper):
- SETTINGS: `HEADER_TABLE_SIZE=65536`, `ENABLE_PUSH=0`,
  **`INITIAL_WINDOW_SIZE=131072`** (Chrome=6 291 456), **`MAX_FRAME_SIZE=16384`**
  (Chrome omits it). Firefox does NOT send `MAX_CONCURRENT_STREAMS` /
  `MAX_HEADER_LIST_SIZE` on the client SETTINGS.
- pseudo-header order **`m,p,a,s`** (`:method,:path,:authority,:scheme`) — Chrome
  is `m,a,s,p`. This is "implementation-specific and hard to spoof" and is one of
  the strongest H2 tells.
- connection window: WINDOW_UPDATE to **12 517 377** delta (target 12 582 912 =
  12 MB) — distinct from Chrome's 15 663 105.
- **Firefox sends an explicit PRIORITY tree** on dedicated streams (3, 5, 7, 9,
  11, 13) with fixed weights/dependencies (the classic Firefox "idle priority
  group" frames) — Chrome sends a single HEADERS-frame priority hint instead.
  The `http2` crate (wreq's fork) supports emitting these.

**Firefox 135 TLS** (transcribe from `lexiforest/curl-impersonate`
`firefox_*` YAML + `wreq-util` firefox profile + cross-check
`sardanioss/httpcloak`):
- Firefox cipher list/order (NSS order, distinct from Chrome's BoringSSL list)
- **NO MLKEM768 desktop lead** in supported_groups (Firefox 135 desktop curve
  set differs; verify against the YAML)
- Firefox sigalgs set/order
- **No Fisher-Yates extension shuffle** — Firefox uses a stable extension order
  (so set `set_permute_extensions(false)` and apply Firefox's fixed permutation,
  exactly the mechanism the iOS Safari branch already uses, `tls.rs:347-351`)
- **No ALPS, no ECH grease** (skip both, like the Safari branch does,
  `tls.rs:387`)
- cert compression: Firefox's negotiated algorithm (per YAML), not Brotli-by-default

### Wiring

The codebase already has the exact pattern to copy: the **iOS Safari branch**
proves boring2 can express a non-Chrome class (distinct ciphers, sigalgs,
curves, a fixed extension permutation, skipped ALPS/ECH, alternate cert
compression — `tls.rs:241-326, 347-351`) and the H2 layer already supports a
second SETTINGS/pseudo-order/priority profile (`h2_client.rs:85-175`). FIX-FF1
adds a third branch keyed off `browser_name == "Firefox"` (or a new
`DeviceClass`/`tls_impersonate` read) in both files.

**Verify-risk (the one unknown):** boring2 must be able to express Firefox's
exact extension *set* and order. It has `SSL_CTX_set_extension_permutation`
(already used at `tls.rs:361`), so order is expressible; if Firefox emits an
extension boring2 cannot, that's the hard part — validate early against a
captured Firefox 135 JA4 from `tls.peet.ws/api/all`.

---

## 5. Ranked fix list

> Public-engine note: all of these are wire-layer (`crates/net`) or
> routing-policy changes. None is per-vendor bypass code; all stay in public
> crates per `CLAUDE.md`.

### FIX-FF1 — Build the real Firefox 135 TLS + HTTP/2 wire class (THE fix)
**What:** Add a `browser_name == "Firefox"` branch in
`tls.rs::chrome_connector` (Firefox ciphers/sigalgs/curves, no-MLKEM-lead,
fixed extension order via the existing `SSL_CTX_set_extension_permutation`
path, no ALPS, no ECH grease, Firefox cert compression) and in
`h2_client.rs::handshake` (SETTINGS `1:65536;2:0;4:131072;5:16384`, pseudo-order
`m,p,a,s`, 12 MB connection window, Firefox PRIORITY-tree frames). Mirror the
existing iOS Safari branch structure. Add a `firefox_h2_settings_match_reference`
byte test + a Firefox JA4 baseline capture, parallel to the Chrome/Safari tests.
**Effort:** 1-2 weeks (mostly reference transcription + boring2 extension-set
validation + capture).
**Expected firefox gain:** **+4 high-confidence** (reuters, wsj, zillow, macys —
all four are *pure* JA4≠UA pre-filter/drop failures that pass on every coherent
profile) **+0-2 lower-confidence** (tripadvisor firefox-arm; spotify if the
desktop factor in §3.4 is secondary). Net firefox **106 → ~110-112**.
**Confidence:** high for the +4 (the mismatch is the only variable isolating
those four); medium for tripadvisor/spotify.
**Public engine:** yes.

### FIX-FF2 — Firefox-routing policy guard (cheap interim risk reduction)
**What:** Until FIX-FF1 ships, do NOT route the Firefox UA to JA4-cross-checking
vendors (CF / Akamai / DataDome / PerimeterX-fronted sites). The routed
best-of-4 already masks this (a coherent profile wins those sites), but an
explicit guard prevents the firefox profile from being *selected* for a site
where its mismatch is a guaranteed loss, and protects the few genuine firefox
wins (adidas-class, where UA+headers alone flip the decision). Implementable as a
routing-table flag; no wire changes.
**Effort:** 0.5-1 day.
**Expected gain:** 0 *new* passes, but stops firefox from dragging the routed
union and clarifies which sites firefox can legitimately help on.
**Confidence:** high. **Public engine:** yes. (= NETWORK_fingerprint FIX-N2.)

### FIX-FF3 — Capture a Firefox JA4 + akamai-H2 baseline into tree (verification)
**What:** Once FIX-FF1 lands, hit `tls.peet.ws/api/all` with the firefox profile,
store `tls.ja4` + `http2.akamai_fingerprint` under
`crates/net/tests/captures/firefox_135_macos/`, and assert the full strings
(not just a prefix). Confirms the new class is byte-correct and guards against
silent drift on a boring2 bump. (= NETWORK_fingerprint FIX-N1, firefox arm.)
**Effort:** 0.5 day (after FIX-FF1).
**Expected gain:** 0 sites; converts "we think firefox JA4 is right" into
machine-proof.
**Confidence:** high. **Public engine:** yes.

### FIX-FF4 — Investigate the spotify chrome-DESKTOP thin (separate from firefox)
**What:** spotify is thin on chrome *and* firefox but full on both mobiles
(§3.4) — a desktop-class content gate independent of the firefox wire mismatch.
Diagnose offline (geolocation/Accept-CH/desktop-vs-mobile content split) — out of
scope for the firefox wire fix but tracked here so spotify isn't double-counted.
**Effort:** 0.5-1 day (diagnostic).
**Expected gain:** uncertain; possibly +1 firefox *and* +1 chrome if it's a
shared desktop-gate issue.
**Confidence:** low. **Public engine:** yes.

---

## 6. Relationship to the other workflow clusters

- **macys THIN-BODY 0 is NOT a firefox-specific connection bug** — it is the
  *hard-drop* expression of the same JA4≠UA mismatch that produces *challenges*
  on reuters/wsj/zillow. One fix (FIX-FF1) addresses both forms.
- This is **orthogonal** to the iphone Cloudflare cluster (a different profile's
  different tell) and to the pixel nav-error cluster (request reliability /
  empty bodies on Chrome-Android). The firefox cluster is uniquely a *wire-class
  coherence* problem because firefox is the only profile BO never built a
  matching wire class for.
- **Competitive note:** Camoufox v150 runs the **native Firefox NSS stack**
  (confirmed via deepwiki on `daijro/camoufox` — it patches only headers in
  `nsHttpHandler.cpp` and never touches the ClientHello / H2 SETTINGS), so its
  Firefox JA4 + akamai-H2 are *genuine Firefox*, perfectly coherent with its
  Firefox UA. This is the one place v150 is structurally ahead of BO. FIX-FF1
  closes exactly that gap and is the highest-leverage single change for the
  firefox profile.

---

## 7. Sources

- BO source: `crates/net/src/tls.rs` (`:52,57,60-96,222-228,233-369,347-367,387-426`),
  `crates/net/src/h2_client.rs` (`:39-50,78-181`),
  `crates/net/src/headers.rs` (`:17-18,595-706`),
  `crates/net/src/lib.rs` (`:796-859`),
  `crates/stealth/src/presets.rs` (`:421-505` firefox_135_macos, `:464-474`
  aspirational tls_impersonate comment, `:473` device_class=Desktop).
- Repo docs:
  `docs/v0.1.0-match-workflows/00_DATA_per_profile_matrix.md`,
  `docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md` (L1 / FIX-N1-N3
  — the canonical statement of this leak),
  `docs/v0.1.0-parity-workflows/external/VENDOR_datadome.md` (DataDome mechanism;
  reuters/wsj firefox-fail is the JA4 pre-filter, distinct from the etsy daily-key
  self-solve gap),
  `docs/releases/v0.1.0-parity/39_NETWORK_LAYER_FINGERPRINTING.md` §3.6
  ("Firefox H2 ❌ NOT IMPLEMENTED").
- External (2026):
  [proxies.sx TLS/JA4+ guide](https://www.proxies.sx/use-cases/privacy/tls-fingerprint),
  [Cloudflare JA4 signals](https://blog.cloudflare.com/ja4-signals/),
  [Auth0 JA4 signals](https://auth0.com/blog/strengthening-bot-detection-ja4-signals/),
  [Scrapfly HTTP/2+HTTP/3 fingerprinting](https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide),
  [Scrapfly HTTP/2 fingerprint tool](https://scrapfly.io/web-scraping-tools/http2-fingerprint),
  [Trickster Dev HTTP/2 fingerprinting](https://www.trickster.dev/post/understanding-http2-fingerprinting/),
  [Akamai Passive HTTP/2 Fingerprinting whitepaper](https://blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf),
  [ZenRows DataDome bypass 2026](https://www.zenrows.com/blog/datadome-bypass),
  [ZenRows PerimeterX bypass 2026](https://www.zenrows.com/blog/perimeterx-bypass),
  [Scrapfly PerimeterX bypass 2026](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping).
- Reference impls for the Firefox wire class:
  [0x676e67/wreq-util firefox profile](https://github.com/0x676e67/wreq-util),
  [lexiforest/curl-impersonate firefox_* signatures](https://github.com/lexiforest/curl-impersonate),
  [sardanioss/httpcloak](https://github.com/sardanioss/httpcloak).
- Competitor: deepwiki `daijro/camoufox` — confirms Camoufox uses native Firefox
  NSS for TLS/H2 (genuine Firefox JA4), patches only headers + WebRTC IPs.
</content>
</invoke>
