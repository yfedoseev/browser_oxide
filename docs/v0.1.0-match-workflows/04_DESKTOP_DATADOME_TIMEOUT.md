# 04 — Desktop DataDome + uber SPA timeout (per-profile consistency)

**Status:** root-cause + fix-plan research (no live navs — competitor benchmark is
on the single IP; reasoned from captured per-profile data + code + external research)
**Scope (this doc):** the two desktop-side consistency clusters from
`00_DATA_per_profile_matrix.md`:

1. **Desktop DataDome** — spotify / tripadvisor / yelp PASS on the mobile
   profiles (pixel/iphone) but FAIL on the desktop profiles (chrome+firefox)
   with `DataDome-CHL`. Plus the Firefox-only DataDome failures
   (reuters / wsj — also DataDome-CHL).
2. **uber SPA timeout** — `TIMEOUT 0` on chrome / pixel / firefox, but
   `L3-RENDERED 700635` on iphone.

**Companion docs (read alongside):**
`docs/v0.1.0-parity-workflows/external/VENDOR_datadome.md`,
`docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md`,
`docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md`,
`docs/releases/v0.1.0-parity/10_TIMING_OPTIMIZATION.md`.

---

## 0. TL;DR

There are **two independent root causes**, not one:

| Cluster | Profiles failing | Root cause | Class |
|---|---|---|---|
| **Firefox DataDome** (reuters, wsj, tripadvisor) | firefox | **JA4-vs-UA cross-layer mismatch** — the Firefox preset advertises a Firefox UA + Firefox headers but emits Chrome's TLS ClientHello *and* Chrome's HTTP/2 SETTINGS. DataDome scores the ClientHello before reading any byte; "JA4 says Chrome 147, UA says Firefox 135" is the single highest-weighted bot signal. | engine bug (network layer) |
| **Desktop DataDome** (spotify, yelp, tripadvisor on chrome) | chrome (+firefox) | **Mostly legitimate content + IP-trust asymmetry, NOT a chrome fingerprint bug.** DataDome serves mobile UAs a lighter, higher-trust path; the same datacenter IP that gets *challenged* on a desktop UA gets *allowed* on a mobile UA. The chrome JA4 is byte-perfect, so the desktop CHL is the ML/IP-trust tier, not a wire tell. | partly content/IP (limited engine lever) |
| **uber** | chrome, pixel, firefox | **Per-profile nav-budget bug.** uber is a heavy React SPA (multi-MB bundle, 60-90s V8 hydration). It is **not in the host-budget table** (`page.rs:1939`), so it gets the 15s default. The desktop/pixel SPA never reaches `body>50KB` in time → the adaptive `EXTEND` branch (`page.rs:2137`) never fires → `TIMEOUT 0`. iphone is served a **lighter mobile page** that crosses 50KB and renders inside the budget. | engine bug (timeout budget) + content |

**Highest-ROI, highest-confidence desktop wins, in order:**
1. **uber → 90s SPA-shell budget tier** (`page.rs:1955-1965`): one-line host
   add. Flips chrome + firefox (+ likely pixel). **+2 to +3 desktop**, high
   confidence.
2. **Firefox real H2 SETTINGS** (`h2_client.rs`): add a Firefox arm. Removes
   half the cross-layer mismatch. Necessary-but-not-sufficient for the
   Firefox-DataDome sites; medium confidence alone.
3. **Firefox real TLS ClientHello** (`tls.rs`): a genuine NSS-class JA4. The
   load-bearing half of the mismatch. Larger effort; this is what actually
   flips reuters / wsj / tripadvisor on firefox. **+2 to +3 firefox** with #2.
4. **Desktop DataDome self-solve fidelity** (the `07_DATADOME_PRIMITIVES`
   iframe-materialize + cookie-retry path): only helps if the chrome CHL is the
   *solvable silent* variant; spotify/yelp are likely IP-trust/interactive and
   may not flip. Low-medium confidence.

---

## 1. The data signature (evidence)

From `00_DATA_per_profile_matrix.md` rows:

```
spotify     | chrome ·L3 9881   | pixel ✅147739 | iphone ✅147724 | firefox ·L3 9875
tripadvisor | chrome ·DD-CHL1412| pixel ✅383111 | iphone ✅290654 | firefox ·DD-CHL1464
yelp        | chrome ·DD-CHL1424| pixel ·DD1424  | iphone ✅610288 | firefox ·DD-CHL1458
reuters     | chrome ✅1138793  | pixel ✅1161144| iphone ✅1126171| firefox ·DD-CHL1456
wsj         | chrome ✅691418   | pixel ✅285500 | iphone ✅287970 | firefox ·DD-CHL1461
uber        | chrome ·TIMEOUT 0 | pixel ·TIMEOUT0| iphone ✅700635 | firefox ·TIMEOUT0
```

Three distinct signatures hide in here — and conflating them is the trap:

- **`DataDome-CHL ~1.4 KB`** = the small interstitial body (`is_datadome_challenge`
  fires: `<50KB` + `captcha-delivery.com`). This is a *real* DataDome block, not
  a render miss.
- **Firefox is the ONLY profile that gets DD-CHL on reuters + wsj** while
  chrome/pixel/iphone all render 280KB–1.1MB. reuters/wsj are clearly
  *passable* (3 of 4 profiles pass). So the firefox failure is a
  **firefox-specific tell**, not site difficulty.
- **mobile (pixel+iphone) passes spotify/tripadvisor where desktop fails**, but
  **yelp fails on chrome+pixel+firefox and only passes iphone** — so even
  "mobile" isn't monolithic. iphone (iOS Safari, real-ish JA4) is the strongest
  profile against DataDome; pixel (Android-Chrome) sits in the middle.
- **uber is `TIMEOUT 0` (NOT DD-CHL)** on 3 profiles. Different failure mode
  entirely: no challenge marker, no body — the nav budget expired before any
  body landed. iphone's `700635` is a fully rendered page. This is a
  **timeout/budget + page-weight** story, not an anti-bot story.

---

## 2. Root cause A — Firefox DataDome: the JA4-vs-UA cross-layer mismatch

### 2.1 The code

The Firefox preset (`crates/stealth/src/presets.rs:421-489`, `firefox_135_macos`,
mirrored in `firefox_135_windows/linux`) sets:

```rust
user_agent: "Mozilla/5.0 (Macintosh; …; rv:135.0) Gecko/20100101 Firefox/135.0",
browser_name: "Firefox",
tls_impersonate: "firefox_135",     // ← aspirational STRING ONLY
device_class: DeviceClass::Desktop, // ← this is what actually drives TLS
```

The doc-comment at `presets.rs:464-474` is explicit that `tls_impersonate` is
**informational only**:

> "The actual TLS bytes are emitted by `crates/net` via boring2/BoringSSL with a
> Chrome-tuned ClientHello. A real Firefox JA4 swap requires reconfiguring
> boring2's cipher list / extension order to match NSS — substantial work
> tracked as a future item."

Confirmed in source:

- **TLS** — `crates/net/src/tls.rs::chrome_connector()` branches **only on
  `profile.device_class`** (`tls.rs:241-242`: `is_safari_ios = device_class ==
  MobileIOS`; curves at `tls.rs:242`). It never reads `tls_impersonate`. A
  `Desktop` Firefox profile therefore takes the **Chrome 147 ClientHello path**:
  15-entry `CIPHER_LIST`, `X25519_MLKEM768` lead key share, Brotli cert
  compression, ALPS, ECH-grease, Fisher-Yates extension shuffle. That is a
  *Chrome* JA4 (`t13d1516h2_…`).
- **HTTP/2** — `crates/net/src/h2_client.rs::handshake()` branches **only on
  `is_safari_ios`** (`h2_client.rs:90,110,154`). Every non-iOS profile,
  including all three firefox presets, emits **Chrome SETTINGS**
  (`1:65536;2:0;4:6291456;6:262144`), Chrome connection-window delta
  `15663105`, Chrome pseudo-header order `m,a,s,p`, and the Chrome
  `weight=255 exclusive=true` HEADERS priority hint. There is **no Firefox arm.**

`NETWORK_fingerprint.md` §2.3 already names this "the load-bearing leak" and
§0/§2.2 confirm "Firefox H2 is explicitly NOT implemented … takes the Chrome
branch." This doc ties it to the *specific failing sites*.

### 2.2 Why DataDome flags exactly this

External 2026 sources are unambiguous that DataDome scores the TLS handshake
**before the first HTML byte** and cross-references JA4 against the claimed UA:

- *startertutorials, "Bypassing DataDome in 2026":* "DataDome's edge nodes
  analyze the TLS handshake — cipher suites, extensions, key exchange — and
  compare them to your declared User-Agent. If you are using a standard Node.js
  TLS stack while claiming to be Chrome 124, you are blocked before the first
  byte of HTML is even sent."
- *olostep / proxies.sx:* "anti-bot systems cross-reference the cryptographic
  JA4 fingerprint against the claimed user agent … a non-Chrome JA4 hash alone
  is enough to escalate to slider CAPTCHA."
- `VENDOR_datadome.md` §2.2 — DataDome's ML model is fed by "TLS fingerprint,
  browser fingerprint, behavioral signals, IP reputation."

BO's Firefox profile presents the **inverse** of the caught case: a *Chrome*
JA4 + *Chrome* H2 with a *Firefox* UA. To DataDome's coherence check this is
just as incoherent as "Chrome JA4 + curl UA" — the layers disagree. A real
Firefox 135 emits a wholly different JA4 (different cipher set/count, no
MLKEM768 desktop key share, NSS extension order, no Fisher-Yates, no ALPS, no
Brotli cert compression) and a different akamai H2 hash (Firefox: `HEADER_TABLE
65536, PUSH 0, INITIAL_WINDOW 131072, MAX_FRAME 16384`, pseudo-order `m,p,a,s`).

**This is why firefox is the ONLY profile that fails reuters/wsj while
chrome/pixel/iphone render them:** chrome's UA *matches* its Chrome JA4 (coherent),
iphone's UA matches its real iOS-Safari JA4 (coherent, and iOS gets the highest
trust). Only firefox is internally incoherent → DataDome escalates it to the
1.4 KB interstitial. Same mechanism drives the firefox-only failures in the
sibling doc (zillow PerimeterX-PaH, tripadvisor DataDome) — PerimeterX/HUMAN
also does JA3/JA4-vs-UA per the Scrapfly/ZenRows PerimeterX guides.

### 2.3 This is a genuine engine bug (not content)

reuters/wsj pass on 3 of 4 profiles → the sites are passable from this IP. The
firefox failure is purely the cross-layer tell. Fixing the firefox wire
fingerprint is the lever; it is the same gap Camoufox does NOT have (Camoufox
runs the real Firefox NSS stack, so its Firefox JA4 is genuine — `NETWORK_
fingerprint.md` §0 calls this "a place where Camoufox is strictly ahead of BO").

---

## 3. Root cause B — Desktop (chrome) DataDome: mostly content/IP-trust, limited engine lever

### 3.1 Why this is NOT a chrome fingerprint bug

The chrome profile's TLS + H2 are **byte-perfect** (`NETWORK_fingerprint.md` §2.1
"Chrome + iOS Safari TLS is byte-perfect … no engineering gap"; §2.2 H2
byte-perfect). The chrome UA *matches* the chrome JA4 — fully coherent. So the
chrome `DataDome-CHL` on spotify/yelp/tripadvisor is **not** a wire mismatch like
firefox's. It is the **ML / IP-reputation tier**.

### 3.2 The mobile-vs-desktop asymmetry is real and IP-driven

External research (proxies.sx, proxyhat, roundproxies) is consistent:

- "Mobile IPs from mobile ASNs receive the highest trust scores — mobile traffic
  is inherently harder to automate at scale … only mobile carrier IPs maintain
  consistently high trust because of CGNAT."
- "Datacenter IPs get flagged immediately by DataDome's reputation scoring …
  datacenter ASNs get aggressive rate limits (3 requests before challenge)."

But note: BO uses **one datacenter IP for all four profiles**. The IP's ASN
trust is identical across profiles. So the mobile-vs-desktop split BO observes
is **not** "mobile IP vs datacenter IP" — both are the same datacenter IP. The
asymmetry that remains is **UA-class trust weighting**: DataDome's per-customer
ML model assigns a *mobile UA from a datacenter IP* a different (often higher,
or differently-thresholded) trust band than a *desktop UA from a datacenter IP*,
because mobile automation at scale from datacenter ranges is rarer / the false-
positive cost of challenging mobile is higher. The iphone profile additionally
carries a **real-class iOS-Safari JA4** (`tls.rs` MobileIOS branch — distinct
ciphers, P-521, 3DES tail, the duplicated `rsa_pss_rsae_sha384` Apple bug), which
is the *most* coherent and highest-trust wire signature BO ships. That is why
**iphone passes yelp where even pixel (Android-Chrome) fails it.**

### 3.3 The likely page-content overlay (spotify specifically)

`SITE_diagnostic_and_tail.md:213` records spotify as a **borderline thin-shell**
(invisible reCAPTCHA-v3 shell, ~9.6 KB, "a hydration change either way flips
it"). The matrix matches: chrome/firefox = `L3-RENDERED 9881/9875` (the thin
shell — NOT a DD-CHL), while pixel/iphone = `147739` (full content). So
**spotify desktop is not even a DataDome interstitial** — it's the thin SPA
shell scored as L3 but under the 15 KB pass bar. The mobile path is served a
**heavier server-rendered page**. This is a *content + render-fidelity* problem
on desktop, distinct from yelp/tripadvisor's true 1.4 KB DD-CHL.

### 3.4 The engine lever that exists (and its ceiling)

For the true DD-CHL desktop sites (tripadvisor on chrome, yelp on chrome), the
public-engine path is the `07_DATADOME_PRIMITIVES` self-solve:

- **It is already wired** (`page.rs:1872`): `started_as_dd_challenge =
  solvers.any(relax_response_csp) || is_datadome_challenge(&html)` — fires on the
  raw DD interstitial shape even with the empty solver set. `rematerialize_
  iframes` (`page.rs:705`) materializes the `geo.captcha-delivery.com` iframe;
  the child gets a 10s `run_until_idle` drain (`iframe.rs:232`); WASM runs
  natively in V8 (`window_bootstrap.js:17`); `is_datadome_solved`
  (`page.rs:221`) gates on the body-transition (FP-D3 guard).
- **Ceiling:** per `VENDOR_datadome.md` §4, this only flips the **silent
  `rt:'i'`** variant where the bundle self-solves with no human input. **yelp
  serves the interactive `rt:'c'` captcha** — even Camoufox gets `DataDome-CHL
  1487` (`07_DATADOME_PRIMITIVES.md:110`). So yelp desktop is **not** flippable
  in the public engine; it's a `vendor_solvers` / out-of-reach target. The
  realistic desktop DataDome flip is **tripadvisor-on-chrome** (if it's the
  silent variant) — and even that has the v135→v150 regression risk
  (`VENDOR_datadome.md` §2.5: DataDome ML tightened so even Camoufox v150
  regressed on etsy/tripadvisor).

**Bottom line for cluster B:** the chrome DataDome failures are largely IP-trust
+ content, not a chrome wire bug. The only clean engine lever (DD self-solve)
caps at the silent variant. Expected desktop gain here is **+0 to +1**, low
confidence — distinct from the firefox cross-layer fix (real lever) and the uber
budget fix (clean win).

---

## 4. Root cause C — uber: per-profile nav-budget bug

### 4.1 The bug

The host-budget table (`crates/browser/src/page.rs:1939-1986`) has tiers:

```rust
// SPA shells — heavy React/Vue hydration.
Some(h) if h.ends_with("twitter.com") || h.ends_with("x.com")
    || h.ends_with("hulu.com") || h.ends_with("yandex.ru")
    || h.ends_with("hm.com") || h.ends_with("khanacademy.org")
    || h.ends_with("spotify.com") => 90_000,
…
_ => 15_000,    // ← uber falls here
```

**uber.com is not listed** → it gets the **15s default**. The adaptive `EXTEND`
branch (`page.rs:2129-2160`) only grants the extra +25s **if `body_len > 50*1024`
on iter 0** (`page.rs:2137`). The comment at `page.rs:1935-1938` describes the
exact failure for this class:

> "SPA shells (twitter, x.com, hulu, yandex.ru, h&m, khanacademy): main bundle
> is 1-5MB; React/Vue hydration in our V8 takes 60-90s vs ~5s on headed Chrome.
> Without the bump, body=0/69 bytes after deadline."

uber's desktop site is exactly this class — a heavy React SPA whose body stays
empty (the `<div id="root">` shell) until multi-MB JS hydrates. At 15s the body
is still `<50KB`, so the EXTEND never fires, the budget expires, and the matrix
records `TIMEOUT 0`. Same on pixel and firefox (both also get the 15s default;
pixel is Android-Chrome desktop-class budget, firefox is Desktop-class).

### 4.2 Why iphone passes (content, not budget)

iphone's `L3-RENDERED 700635` is **not** the budget being more generous — iphone
gets the same 15s default. iphone passes because **uber serves the mobile
Safari UA a lighter, more server-rendered page** (m.uber-style / SSR landing)
that crosses the 50KB body bar *within* 15s and renders. This is a legitimate
content difference (mobile-optimized page), the same pattern as spotify §3.3.
So uber is **half engine-bug (desktop budget too tight for the heavy SPA) and
half content (mobile served lighter)**.

### 4.3 The fix is trivial and proven for this exact class

Add `uber.com` to the **90s SPA-shell tier** — identical treatment to
twitter/x.com/spotify which are the same heavy-hydration class. With 90s, the
desktop SPA gets time to hydrate past 50KB; the EXTEND branch then keeps it
alive to `readyState=complete`. This is the single highest-confidence desktop
win in this doc — it's a one-line host add to a tier that already exists and is
proven on the structurally identical x.com/twitter/yandex cases.

Risk: near zero. The 90s budget only fires for `uber.com`, only extends on
benign hydration (no CHL marker), and `humanize.js` timers are unref'd
(`10_TIMING_OPTIMIZATION.md` §5) so a benign fast page still exits at
`readyState=complete` long before 90s. No regression to other sites.

Caveat: if BO's V8 cannot hydrate uber's specific bundle even in 90s (some SPAs
need APIs/WebGL/fonts BO stubs), it may still TIMEOUT — but the x.com precedent
(flipped 69B→274KB with the 90s tier) makes this the right first move at high
confidence.

---

## 5. Ranked fix list

Effort / confidence / expected per-profile site gain / public-engine flag.

| # | Fix | File:line | Effort | Confidence | Expected gain | Public engine? |
|---|---|---|---|---|---|---|
| **1** | **uber → 90s SPA-shell budget tier.** Add `\|\| h.ends_with("uber.com")` to the SPA-shell arm at `page.rs:1955-1965`. Heavy-React-SPA, structurally identical to x.com/twitter which already flip with this tier. | `crates/browser/src/page.rs:1962` | **trivial (1 line)** | **high** | **+2 to +3** (chrome, firefox, likely pixel; iphone already passes) | yes |
| **2** | **Firefox real HTTP/2 SETTINGS.** Add a Firefox arm to `handshake()` (currently branches only on `is_safari_ios`): SETTINGS `HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=131072, MAX_FRAME_SIZE=16384`, pseudo-order `m,p,a,s`, no priority frame. Removes the H2 half of the cross-layer mismatch. | `crates/net/src/h2_client.rs:85-180` | medium (2-3 d) | medium (necessary, not sufficient alone) | 0 alone; load-bearing with #3 | yes |
| **3** | **Firefox real TLS ClientHello (NSS-class JA4).** Add a Firefox branch to `tls.rs::chrome_connector()` keyed off a new `device_class`/profile flag: Firefox cipher list + curve order (no MLKEM768 desktop lead), NSS extension order (no Fisher-Yates, no ALPS, no ECH-grease, no Brotli cert compression). This is THE fix that makes firefox's UA coherent with its JA4 and flips the DataDome/PerimeterX cross-check. Pair with #2. | `crates/net/src/tls.rs:230-369` | **large (1-2 wk)** | medium-high (with #2) | **+2 to +3 firefox** (reuters, wsj, tripadvisor, zillow-PX) | yes |
| **4** | **Desktop DataDome self-solve verification.** Confirm the already-wired DD primitive (`page.rs:1872`, `rematerialize_iframes`) actually completes the silent `rt:'i'` self-solve on the LIVE chrome path for tripadvisor (same live-nav-drain question as AWS/etsy in `VENDOR_datadome.md` §5 #1-2). Only flips the silent variant; yelp (`rt:'c'`) is out of reach (Camoufox fails it too). | `crates/browser/src/page.rs:705,221,2236` | medium (2-4 d) | low-medium | **+0 to +1 chrome** (tripadvisor maybe; yelp no) | yes |
| **5** | **Quantify spotify/uber mobile-vs-desktop content delta.** Confirm spotify desktop = thin SPA shell (`SITE_diagnostic_and_tail.md:213`) and uber/iphone = SSR mobile page, so these are scored as content differences, not anti-bot — avoids mis-attributing them to a fingerprint regression. Pure diagnostic; informs whether desktop spotify is even flippable (needs SPA hydration fidelity, not DataDome work). | n/a (capture) | low (0.5 d) | high | 0 (clarifies #1/#4 scope) | yes |

### Expected per-profile outcome

- **Fix #1 (uber)** alone: chrome 110→~111, firefox 106→~107, pixel 108→~109
  (iphone unchanged). The single best consistency win in this doc — flips one
  site on 3 profiles toward the all-4-pass column.
- **Fixes #2+#3 (firefox real wire)** together: firefox 106→~109 (reuters, wsj,
  tripadvisor; zillow PerimeterX is the same mechanism so likely +1 more →
  ~110). This is the structural firefox-parity fix — it closes the cleanest
  network leak BO has and is the area where Camoufox is currently ahead.
- **Fix #4 (chrome DD)**: at most tripadvisor-on-chrome; yelp is unwinnable in
  the public engine. **+0 to +1 chrome.**

### Public-engine / `vendor_solvers` split

All five fixes are **public engine** — nav-budget tiers, wire-fingerprint
fidelity (TLS/H2 are already in `crates/net`, this just adds a Firefox class),
the generic challenge-doc self-solve primitive, and a capture. **None** names a
vendor in flow code or encodes a token. The only DataDome work that would be
`vendor_solvers` is the daily-key `ddCaptchaEncodedPayload` encoder for the
interactive `rt:'c'` path (yelp) — explicitly out of scope per `CLAUDE.md` and
`07_DATADOME_PRIMITIVES.md` §"Out of scope", and not proposed here.

---

## 6. Open questions

1. **Is tripadvisor's chrome CHL the silent `rt:'i'` or interactive `rt:'c'`?**
   A `var dd={…}` capture answers it (live, deferred — IP contention). Decides
   whether fix #4 can flip tripadvisor-on-chrome or whether it's yelp-class.
2. **Does the real Firefox JA4 actually clear DataDome from a datacenter IP?**
   The cross-layer fix removes the mismatch, but DataDome still weights IP
   reputation. If firefox-with-real-JA4 still gets CHL on reuters/wsj, the
   residual is IP-trust (same datacenter-IP ceiling as chrome DataDome §3) and
   the gain estimate for #2+#3 drops. The chrome/iphone passes on reuters/wsj
   from the *same IP* argue the coherent-fingerprint fix should suffice (IP is
   demonstrably clean enough when the fingerprint is coherent).
3. **Can BO's V8 hydrate uber's specific bundle within 90s?** x.com precedent
   says yes for this class, but uber may pull APIs/WebGL/fonts BO stubs. Fix #5
   capture confirms before committing #1's expected gain.
4. **Does pixel (Android-Chrome) need its own H2/TLS arm?** pixel uses
   `chrome_147_android` (`presets.rs:893`) which today maps to the Chrome
   desktop wire path. Android Chrome H2/TLS differs subtly from desktop; pixel's
   yelp failure (vs iphone pass) may partly be a pixel wire-coherence gap, not
   only UA-class trust. Out of this doc's desktop scope but flagged.

---

## 7. Sources

External (2026):
- [startertutorials — Bypassing DataDome in 2026: The Ultimate Engine-Level Guide](https://www.startertutorials.com/blog/bypassing-datadome-in-2026-the-ultimate-engine-level-guide.html) — "TLS handshake compared to declared User-Agent … blocked before the first byte of HTML"
- [proxies.sx — DataDome & Akamai Bypass Guide 2026](https://www.proxies.sx/blog/datadome-akamai-bypass-mobile-proxies) — mobile-ASN trust, JA4-vs-UA cross-reference
- [olostep — Best User Agent for Scraping](https://www.olostep.com/blog/best-user-agent-for-scraping) — "non-Chrome JA4 hash alone is enough to escalate to slider CAPTCHA"; "use mobile only when IP + fingerprint look mobile"
- [proxyhat — DataDome Detection & How Legitimate Automation Passes](https://proxyhat.com/blog/datadome-detection-residential-proxies) — datacenter vs residential/mobile trust scoring
- [roundproxies — Bypass DataDome 2026](https://roundproxies.com/blog/bypass-datadome/) — mobile/CGNAT trust, datacenter challenge thresholds
- [Scrapfly — Bypass PerimeterX 2026](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping) / [ZenRows — PerimeterX bypass](https://www.zenrows.com/blog/perimeterx-bypass) — PerimeterX/HUMAN also TLS/JA-vs-UA + behavioral ML (zillow firefox PerimeterX-PaH)
- [ZenRows — DataDome bypass 2026](https://www.zenrows.com/blog/datadome-bypass), [Scrapfly — DataDome 96%](https://scrapfly.io/bypass/datadome) — ML trust-score model (TLS + browser FP + behavior + IP)

Repo docs:
- `docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md` §0, §2.1–2.3 — Firefox JA4/H2 mismatch as "the load-bearing leak"; Chrome/iOS byte-perfect
- `docs/v0.1.0-parity-workflows/external/VENDOR_datadome.md` §2.2, §2.5, §4 — DataDome ML tiers, silent `rt:'i'` vs interactive `rt:'c'`, yelp unwinnable, v150 regression
- `docs/releases/v0.1.0-parity/07_DATADOME_PRIMITIVES.md` — the 3 wired self-solve primitives + the `vendor_solvers` boundary
- `docs/releases/v0.1.0-parity/10_TIMING_OPTIMIZATION.md` §2, §5, §6.3 — per-host budget table, adaptive extend, unref'd humanize timers
- `docs/v0.1.0-parity-workflows/sites/SITE_diagnostic_and_tail.md:213` — spotify thin-shell classification

BO source:
- `crates/stealth/src/presets.rs:421-489` (firefox_135_macos, `tls_impersonate` aspirational), `:853-914` (pixel chrome_147_android), `:956-1003` (iphone safari_18_ios)
- `crates/net/src/tls.rs:230-369` (chrome_connector branches on device_class only), `:111-157` (Safari iOS class)
- `crates/net/src/h2_client.rs:85-180` (handshake branches on is_safari_ios only — Firefox falls to Chrome)
- `crates/browser/src/page.rs:1939-1986` (host-budget table — uber absent → 15s), `:2129-2160` (adaptive EXTEND gated on body>50KB), `:1872` (DD self-solve flag), `:221,705` (is_datadome_solved, rematerialize_iframes)
- `crates/browser/src/iframe.rs:232` (child iframe 10s drain)
