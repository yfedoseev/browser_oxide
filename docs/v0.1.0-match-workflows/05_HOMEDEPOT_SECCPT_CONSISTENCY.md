# 05 — HOMEDEPOT sec-cpt PER-PROFILE CONSISTENCY

**Goal of this doc:** explain why, *in the SAME 2026-05-29 gate run*,
homedepot passes `chrome_148_macos` (994 KB L3-RENDERED) but
`pixel_9_pro_chrome_148`, `iphone_15_pro_safari_18`, and
`firefox_135_macos` ALL fail with `Akamai-CHL` (2701 / 2734 / 2734 bytes
= the unsolved sec-cpt stub). Then propose making the sec-cpt self-solve
consistent across all 4 profiles.

**This is a different question from the prior docs.** The existing tickets
(`01_R-AKAMAI-SECCPT-FLAKE.md`, `VENDOR_akamai.md`) frame the homedepot
flake as **cross-day** (daily-rotated provider crypto vs behavioral vs
adaptive) and as a **single-profile** (chrome) reliability story. The
new gate data exposes a **same-run, per-profile** split that those docs
do NOT explain: one bundle, one provider, one day — yet chrome wins and
the other three lose deterministically. That can only be a **per-profile
differentiator**, not provider rotation. This doc closes that gap.

---

## 0. The data signature (and what it rules out)

From `docs/v0.1.0-match-workflows/00_DATA_per_profile_matrix.md:24`:

| profile | result | len |
|---|---|--:|
| chrome_148_macos | ✅ L3-RENDERED | 994281 |
| pixel_9_pro_chrome_148 | · Akamai-CHL | 2701 |
| iphone_15_pro_safari_18 | · Akamai-CHL | 2734 |
| firefox_135_macos | · Akamai-CHL | 2734 |

Two observations that drive the whole analysis:

1. **All three failures are the sec-cpt challenge stub, not a render
   error.** `Akamai-CHL` at ~2.7 KB = the `<div id="sec-if-cpt-container">`
   428 interstitial body, unsolved. (Contrast: pixel's *other* gap-fails
   are `THIN-BODY 0` / `ERROR 0` — nav-reliability bugs. homedepot is NOT
   that class; the page loaded, the bundle ran, the solve didn't land.)

2. **The three failing lengths are 2701 / 2734 / 2734.** The 33-byte
   spread is consistent with the **same challenge stub serialized against
   different UA strings / reflected headers** (the interstitial echoes a
   handful of request-derived values). Same bundle, same provider, same
   day — only the *client* differs. **This is the smoking gun that the
   differentiator is the client profile, not the challenge.**

**What this rules out** (relative to the prior docs):
- NOT provider rotation (crypto vs behavioral/adaptive) — provider is
  chosen per-challenge by Akamai's risk engine; in one same-IP same-minute
  gate run, all 4 navs hit the same tenant config. If it were a
  behavioral-provider day, chrome would fail too.
- NOT the `chlg_duration` daily-variance flake described in
  `01_R-AKAMAI-SECCPT-FLAKE.md §3.1` — that explains chrome being ~60%
  *across days*, not chrome winning while three siblings lose *the same
  day*.
- NOT the nav budget (host-keyed 45 s, `page.rs:1975`) — it is identical
  for all 4 profiles (the budget switch is on `host_str()`, never on
  `device_class` / `browser_name`).
- NOT the `is_seccpt_solved` detector (`page.rs:242-247`) — it is
  profile-neutral and unit-tested (`page.rs:3941-3968`).

So the differentiator is something that **varies per profile** and feeds
*either* (a) Akamai's Phase-1 risk score (which selects how hard the
challenge is / whether the crypto bundle is even willing to self-solve),
*or* (b) the bundle's own per-profile execution path. The candidates,
ranked by evidence below, are: **TLS/H2 ↔ UA coherence (firefox)**,
**mobile device-class challenge variant + Phase-1 score (pixel)**, and
**Safari TLS path + `hardwareConcurrency=2` worker path (iphone)**.

---

## 1. How Akamai sec-cpt scoring keys on the client (external)

Akamai Bot Manager scores in **two phases** (confirmed across multiple
2026 public write-ups):

- **Phase 1** — everything scoreable from **TCP/IP (p0f) + TLS
  (JA3/JA4) + the HTTP/2 fingerprint (akamai-h2 hash) + the first
  request headers**, *before any JS runs*. This phase decides whether to
  serve content, a soft 428 crypto challenge, or a harder
  behavioral/adaptive challenge.
- **Phase 2** — the sensor_data POST + the PoW / pixel round-trip.

Sources:
- [How to Bypass Akamai when Web Scraping in 2026 — Scrapfly](https://scrapfly.io/blog/posts/how-to-bypass-akamai-anti-scraping)
  ("five layers: IP reputation, TCP/IP fingerprinting (p0f), TLS
  fingerprinting (JA3/JA4), sensor payload verification, device
  fingerprint **consistency**").
- [Bypassing Akamai v3 sensor_data with TLS in 2026 — DEV (xkiian)](https://dev.to/xkiian/bypassing-akamai-v3-sensordata-with-tls-in-2026-why-the-deobfuscator-is-a-trap-5cjh)
  ("Phase one is everything they can score from your TCP/TLS/HTTP frames
  and your first request, before any JS has run").
- [Akamai Blog — Bots Tampering with TLS to Avoid Detection](https://www.akamai.com/blog/security/bots-tampering-with-tls-to-avoid-detection)
  (Akamai explicitly correlates the negotiated TLS fingerprint with the
  advertised UA; a **mismatch** between the two is a first-class signal).
- [How to Bypass Akamai Bot Detection 2026 — VoidMob](https://voidmob.com/blog/how-to-bypass-akamai-bot-detection-2026)
  ("A request claiming Chrome 131+ but lacking a post-quantum key share
  is flagged before any HTTP traffic occurs" — i.e. TLS↔UA coherence is
  enforced per claimed browser).

**The load-bearing external fact:** Akamai's Phase-1 device-fingerprint
**consistency** check correlates the **TLS/H2 cryptographic fingerprint**
with the **claimed User-Agent**. A high Phase-1 risk score (e.g. from a
TLS↔UA mismatch, or from a less-trusted client class) pushes the
challenge from the self-solvable `crypto` provider toward
`behavioral`/`adaptive`, which the public engine *cannot* self-solve
(`VENDOR_akamai.md §2.1`). It can also make the tenant simply refuse to
re-serve content even after a correct PoW.

This is the mechanism that turns a per-profile *fingerprint* delta into a
per-profile *pass/fail* on homedepot.

---

## 2. Per-profile root cause (code-level)

The four BO profiles diverge on exactly the inputs Akamai Phase-1 reads.
BO branches its TLS and HTTP/2 fingerprint **only on `device_class`**
(`Desktop` / `MobileAndroid` / `MobileIOS`) — NOT on `browser_name`:

- TLS: `crates/net/src/tls.rs:241-256` — `is_safari_ios =
  device_class == MobileIOS`; curves/cipher/sigalgs pick `*_SAFARI_IOS`
  only for iOS, else the shared `CURVES_DESKTOP` / `CIPHER_LIST` /
  `SIGALGS_LIST` (= **Chrome 147**). Android only swaps the curves list
  (`CURVES_ANDROID`, tls.rs:243).
- HTTP/2: `crates/net/src/h2_client.rs:73-90` — identical structure:
  `Desktop / Android → Chrome 147 SETTINGS (1,2,4,6) + masp pseudo-header
  order`; `MobileIOS → Safari 18.4 SETTINGS (2,3,4,9) + msap`.

HTTP **headers** ARE per-browser (`crates/net/src/headers.rs:17-19`
dispatches `firefox_headers` / `safari_headers` / chrome). So the
incoherence is specifically: **HTTP-layer headers say browser X, but the
TLS+H2 cryptographic layer says Chrome (or Safari).**

### 2.1 firefox_135_macos — TLS/H2 ↔ UA MISMATCH (highest confidence)

`firefox_135_macos` is `device_class: DeviceClass::Desktop`
(`presets.rs:473`) with `tls_impersonate: "firefox_135"`
(`presets.rs:474`). But `tls_impersonate` is **informational only** — the
actual bytes are emitted by the `Desktop` branch = **Chrome 147
ClientHello + Chrome 147 akamai-h2 fingerprint**. The preset's own
comment admits this (`presets.rs:466-472`):

> "String token — currently informational only. The actual TLS bytes are
> emitted by `crates/net` via boring2/BoringSSL with a Chrome-tuned
> ClientHello. A real Firefox JA4 swap requires reconfiguring boring2's
> cipher list / extension order to match NSS — substantial work tracked
> as a future item."

So on the wire Akamai sees:
- `User-Agent: …rv:135.0) Gecko/20100101 Firefox/135.0` + Firefox-shaped
  headers (no `sec-ch-ua`, `Accept` q=0.5, Gecko `priority`) — coherent
  Firefox at L7.
- **JA3/JA4 = Chrome 147**, **akamai-h2 hash = Chrome 147** — at the
  crypto layer this is *Chrome, not Firefox*.

Firefox's real TLS fingerprint is **profoundly** different from Chrome:
distinct cipher ordering, NSS extension order, no GREASE the Chrome way,
different supported_groups, no `X25519MLKEM768` arranged the Chrome way,
and a totally different HTTP/2 SETTINGS profile (Firefox sends
`1,4,5` with different values and uses RFC7540 priorities differently).
Akamai's per-browser TLS↔UA consistency check therefore fires
immediately: **a Firefox UA presenting a Chrome JA4 is one of the
cleanest "tampered client" signals Akamai documents**
([Akamai TLS-tampering blog](https://www.akamai.com/blog/security/bots-tampering-with-tls-to-avoid-detection)).

**Root cause (firefox):** the Phase-1 TLS/H2↔UA mismatch pushes
homedepot's risk engine to serve the *non-crypto* (behavioral/adaptive)
sec-cpt provider — which the public engine structurally cannot self-solve
(`VENDOR_akamai.md §2.1`) — or to refuse the post-PoW content re-serve.
Either way firefox sticks on the 2734-byte stub. chrome_148_macos has a
**coherent** Chrome JA4 ↔ Chrome UA, scores low in Phase 1, gets the
self-solvable crypto provider, and the bundle completes.

This is the same field-cluster the repo flagged for the **adidas
firefox-only** anomaly, just inverted by tenant: `26_AKAMAI_BMP_DEEP.md
§4.1` ranks "(a) TLS class" as the top hypothesis for why adidas behaves
differently on firefox. homedepot is the same lever — firefox's
crypto-layer/UA incoherence — producing the opposite outcome (adidas's
tenant tolerates it; homedepot's does not).

### 2.2 pixel_9_pro_chrome_148 — mobile device-class variant + Phase-1 (medium-high)

`pixel_9_pro_chrome_148` (`presets.rs:850-933`) is coherent at the UA
layer (mobile Chrome 148 UA + `sec-ch-ua-mobile: ?1` + empty plugins) and
its TLS is the Android branch (`CURVES_ANDROID`, tls.rs:243) — *closer*
to coherent than firefox. So why does it fail?

Two compounding mobile-specific causes:

1. **Akamai serves a mobile sec-cpt variant to mobile UAs, and Phase-1
   scores mobile/Android differently.** homedepot's tenant treats a
   mobile-Chrome client on a *datacenter IP* as higher-risk than desktop
   Chrome (mobile traffic from a server IP is itself anomalous — a real
   Pixel does not originate from an AWS range). The challenge it serves
   the mobile profile is the harder (behavioral) variant, or the
   crypto-PoW completes but the content re-serve is refused on the mobile
   risk band. This is the same observation `00_DATA` shows in the
   *inverse* on other tenants — DataDome sites (spotify/tripadvisor/yelp)
   *fail desktop and pass mobile*; Akamai-homedepot does the opposite.
   **The challenge-hardness is tenant×device-class specific.**

2. **Android TLS is only a partial Firefox-class fix.** The Android
   branch swaps **only the curves** (tls.rs:243); the cipher list,
   sigalgs, ALPS, GREASE, and the **HTTP/2 SETTINGS are still the desktop
   Chrome 147 values** (h2_client.rs:74 lumps `Desktop / Android`
   together). Real Android Chrome's akamai-h2 fingerprint is *not* byte-
   identical to desktop Chrome (different INITIAL_WINDOW / header-table
   tuning on mobile builds). A mobile-Chrome UA with a desktop-Chrome H2
   fingerprint is a softer version of the firefox mismatch — enough to
   nudge Phase-1 on a strict tenant.

**Root cause (pixel):** mobile-on-datacenter-IP Phase-1 risk band +
residual desktop-vs-mobile H2 fingerprint imprecision → harder challenge
variant → public engine cannot self-solve. (Note: pixel's homedepot
failure is sec-cpt — distinct from pixel's `THIN-BODY 0` nav-reliability
failures on airbnb/yandex/prime-video, which are a separate bug.)

### 2.3 iphone_15_pro_safari_18 — Safari TLS path + hardwareConcurrency=2 worker path (medium)

`iphone_15_pro_safari_18` (`presets.rs:956+`) is the most-coherent of the
three failing profiles at the fingerprint layer: it has a **real Safari
18 ClientHello** (`CIPHER_LIST_SAFARI_IOS`, `CURVES_SAFARI_IOS`, 4 TLS
versions, zlib cert compression, `NO_TICKET`, skipped ALPS/ECH —
tls.rs:237-295) and a **real Safari 18.4 H2 fingerprint** (4 SETTINGS
2,3,4,9 + `msap` + no priority frame — h2_client.rs:76, 133-156). So
iphone's TLS↔UA coherence is the *best* of the failing three. Its failure
has a different shape:

1. **`hardwareConcurrency: 2` + `deviceMemory: undefined`**
   (`presets.rs:979, 983`). The sec-cpt crypto bundle (like AWS WAF's
   challenge.js) offloads SHA-256 PoW to a **blob-URL Web Worker** (BO
   handles exactly this path — `page.rs:256`, `worker_ext.rs:9-10`
   "Akamai's BMP v3 spawns workers via blob: URLs"). The bundle reads
   `navigator.hardwareConcurrency` to decide **how many PoW workers to
   spawn / whether to use the worker path at all**. With `2` it may take
   a different (e.g. single-worker or main-thread) code path than the
   `8`-core chrome profile took — and that path is the one BO's drain
   model is *least* exercised on. (The PoW itself is cheap and runs on a
   real OS thread regardless of the reported count — `worker_ext.rs`
   uses real mpsc threads — so this is about which **code branch** the
   bundle takes, not raw compute.)

2. **The Safari TLS/H2 path is the newest and least-validated in BO**
   (presets.rs:938-940: "Use only after Phase 3 lands"; tls.rs comments
   throughout flag the iOS branch as Phase-3). Any residual byte-delta in
   the Safari ClientHello vs a real iPhone (e.g. the supported_versions
   length, the zlib cert-compression advertisement) is a fresh Phase-1
   tell on a Safari-claiming client.

3. **The worker secure-context fix (commit `5216336`) is a *prerequisite*
   not yet validated for sec-cpt.** That commit fixed `crypto.subtle`
   being stripped in workers (which broke AWS/recaptcha worker PoW). If
   the iphone sec-cpt bundle's worker path was the one previously hitting
   the stripped-`crypto.subtle` bug, the fix is necessary but, per the
   commit message itself, "necessary-but-not-sufficient" — there is ≥1
   further worker-round-trip gap. **`VENDOR_akamai.md §7 Q2` lists "does
   the sec-cpt crypto bundle offload PoW to a blob-URL Web Worker?" as an
   explicit open question — if yes, the worker drain/round-trip is the
   iphone-specific failure point.**

**Root cause (iphone):** the bundle takes a low-core / Safari-specific
worker PoW branch whose async round-trip (worker → token → main-thread
continuation → verify POST → reload) is truncated by BO's drain model
(`page.rs:3661` 50 ms inter-script drain; `01_R-AKAMAI-SECCPT-FLAKE §3.1`
unref'd `chlg_duration` timer). chrome's 8-core path happens to keep the
loop busy enough to land; iphone's 2-core path goes quiet during the
worker round-trip and the loop idles out. This is the per-profile
expression of the **same drain root cause** the prior docs found — the
2-core profile is simply the one that exposes it deterministically.

### 2.4 Why chrome_148_macos wins

chrome_148_macos is the **fully-validated, maximally-coherent** profile:
- coherent Chrome 147 JA3/JA4 ↔ Chrome 148 UA ↔ `sec-ch-ua` headers,
- canonical desktop Chrome H2 fingerprint,
- 8 cores → the bundle's mainstream multi-worker PoW path (the one BO's
  drain model has been tuned against, e.g. for AWS WAF),
- lowest Phase-1 risk → homedepot serves the self-solvable **crypto**
  provider → bundle completes → 994 KB.

It is not that chrome is "lucky"; it is that chrome is the only profile
whose entire stack (TLS + H2 + UA + cores) is internally consistent AND
matches BO's best-exercised execution path.

---

## 3. The FIX-1 timer flag is already landed — but does NOT close the per-profile gap

`01_R-AKAMAI-SECCPT-FLAKE §4 FIX-1` (refed challenge-nav timers) **has
shipped**: `page.rs:3594-3601` sets `globalThis.__keepLongTimersRefed =
true` when the initial doc is a challenge (incl. `sec-if-cpt-container` /
`sec-cpt-if`), and `timer_bootstrap.js:69` honors it (`if (ms >=
UNREF_THRESHOLD_MS && !globalThis.__keepLongTimersRefed) _unrefRaw(p)`).
This pins the `chlg_duration` wait timer — a genuine fix for the
*cross-day* flake.

**But it is profile-neutral and therefore cannot explain or fix the
per-profile split.** It helps all 4 profiles equally; chrome already
passes, the other 3 still fail. The per-profile gap is upstream of the
drain (Phase-1 provider selection / fingerprint coherence) for
firefox+pixel, and is a *different drain branch* (worker round-trip on
the 2-core path) for iphone.

Also note **FIX-2's core is NOT landed**: the inter-script drain at
`page.rs:3661` is still an unconditional `Duration::from_millis(50)`. The
challenge-aware burst that `VENDOR_akamai.md §4 Fix 1` and
`01_R-AKAMAI-SECCPT-FLAKE §4 FIX-2` prescribe is still open — and it is
the iphone-relevant one (§2.3).

---

## 4. Ranked fix list (per-profile consistency)

Goal: make sec-cpt solve on all 4 profiles, or — where structurally
impossible in the public engine — route homedepot to the profile that
solves so the *routed* gate keeps the +1 while the consistency story is
honest. All public-engine unless flagged.

### FIX-A — Real Firefox TLS + HTTP/2 fingerprint (fixes firefox; highest per-profile leverage)
**Profile:** firefox_135_macos. **What:** reconfigure boring2 to emit an
actual Firefox/NSS ClientHello (Firefox cipher order, supported_groups,
extension order, no Chrome-style GREASE, Firefox `X25519MLKEM768`
placement) and add a Firefox HTTP/2 SETTINGS branch (Firefox sends a
distinct SETTINGS set + RFC7540 priority tree) gated on
`browser_name == "Firefox"` (today TLS/H2 branch only on `device_class`,
tls.rs:241 / h2_client.rs:85). This removes the TLS/H2↔UA mismatch that
is Akamai's Phase-1 trigger.
**Effort:** large — 5-10 days (this is the "substantial work" the preset
comment defers, presets.rs:469-471; needs a captured real-Firefox JA4 +
akamai-h2 reference to diff against).
**Expected per-profile gain:** firefox +1 on homedepot, and **plausibly
firefox +1 on reuters/wsj/tripadvisor (DataDome)** — DataDome also keys
on TLS↔UA coherence, and those are firefox's other gap-fails
(`00_DATA:29,31,34`). Potentially the single biggest firefox-profile
lever in the whole gap set.
**Confidence:** high that it removes the mismatch; medium that the
mismatch is the *sole* homedepot blocker (provider selection may still
land behavioral on a strict day — but it stops being a *deterministic*
fail).
**Engine:** public (net/TLS).

### FIX-B — Challenge-aware inter-script + warm-rebuild drain (fixes iphone; shared AWS/booking win)
**Profile:** iphone_15_pro_safari_18 (and reinforces all). **What:**
replace the unconditional 50 ms inter-script drain (`page.rs:3661`) and
the warm-rebuild 50 ms/500 ms drain (`VENDOR_akamai.md §3.2`) with a
challenge-aware burst: when `doc_is_challenge` (the flag already computed
at `page.rs:3594`), drain ≥1-2 s between scripts and ≥ `chlg_duration` +
verify RTT on the warm rebuild, capped by `V8DeadlineWatcher`. This lets
the 2-core worker-PoW round-trip + verify POST + `location.reload()`
complete on the iphone branch.
**Effort:** 1-2 days (the predicate exists; `01_R-AKAMAI-SECCPT-FLAKE
§4 FIX-2` scoped it).
**Expected per-profile gain:** iphone +1 on homedepot; cross-cluster
co-benefit on the **AWS-WAF cluster** and **booking** (same drain class,
`HANDOFF_2026_05_28b §4`); marginally hardens chrome too.
**Confidence:** high (line-level, gated to challenge bodies = zero
fast-site regression).
**Engine:** public.

### FIX-C — Verify the worker PoW path under the iphone profile via the sec-cpt oracle (diagnostic for iphone)
**Profile:** iphone (confirms §2.3). **What:** execute the deferred
oracle (`01_R-AKAMAI-SECCPT-FLAKE` Step 1-2 / `VENDOR_akamai.md §4 Fix
2`): fork `awswaf_probe.rs` → `seccpt_probe.rs`, replay a captured 428 +
bundle through `from_html_with_url` **once per profile** (chrome AND
iphone), trace whether the bundle spawns a worker, reads
`hardwareConcurrency`, takes a different branch at `2` vs `8`, and
whether `crypto.subtle` is present in that worker (validates commit
`5216336` for sec-cpt). Decode the base64 `challenge` attr to read the
**provider** per profile — directly answers whether firefox/pixel get a
*different provider* than chrome (confirms §2.1/§2.2).
**Effort:** 1-2 days. **Gain:** 0 direct flips; it is the experiment that
*proves* which of FIX-A/B/D is the real lever per profile. **Confidence:**
high. **Engine:** public (dev tool).

### FIX-D — Per-device-class Phase-1 hardening for pixel (Android H2 + risk-band)
**Profile:** pixel_9_pro_chrome_148. **What:** (a) split the H2 fingerprint
so Android Chrome gets its own SETTINGS/window tuning instead of sharing
the desktop branch (h2_client.rs:74); (b) accept that mobile-on-datacenter
-IP is an inherent Phase-1 penalty homedepot's tenant applies and that
the public engine may not beat it from a server IP. Pair with FIX-B
(drain) since pixel also runs the worker path.
**Effort:** 2-3 days (H2 split) + diagnosis. **Gain:** pixel +1 on
homedepot *if* the residual is the H2 delta; **possibly 0** if it is the
mobile-IP risk band (in which case route, see FIX-F). **Confidence:**
medium-low (mobile-IP penalty may dominate). **Engine:** public (net/H2).

### FIX-E — Use the `~3~` cookie signal to force the post-solve re-fetch (residual hardening, all profiles)
**What:** `01_R-AKAMAI-SECCPT-FLAKE §4 FIX-3` — at the `MIN_RETRY_BUDGET`
guard (`page.rs:2420`-era), if `sec_cpt=…~3~` is present, do NOT bail;
fire the cookie-delta re-fetch immediately rather than waiting for the
body transition. Removes the worst-day budget cliff after a *successful*
solve. Helps whichever profiles get to `~3~` but lose the re-fetch race.
**Effort:** 1 day. **Gain:** converts residual flake to 0 on profiles
that already reach `~3~`. **Confidence:** medium-high. **Engine:** public.

### FIX-F — Route homedepot to chrome in the best-of-4 (consistency stopgap, not a true fix)
**What:** until FIX-A/B land, accept homedepot as a chrome-only pass and
ensure the routed gate selects chrome for homedepot. **This does NOT
satisfy goal (1) consistency** — it keeps the routed +1 but leaves the
per-profile numbers (pixel/iphone/firefox) down by 1 each. Document it as
a known per-profile gap, not a flip. **Effort:** ~0 (already the routed
behavior). **Gain:** 0 per-profile; preserves routed 115. **Confidence:**
high. **Engine:** N/A (harness).

### FIX-G — vendor_solvers sec-cpt behavioral/adaptive encoder (structural ceiling, private)
**What:** `VENDOR_akamai.md §4 Fix 6` — if FIX-A/B/C prove that
firefox/pixel get the *behavioral/adaptive* provider (not crypto), the
public engine **cannot** self-solve those regardless of fingerprint, and
the only path is the private sensor encoder in `vendor_solvers`.
**Effort:** 3-5 days. **Gain:** firefox/pixel +1 *if* provider is the
blocker. **Confidence:** medium. **Engine:** **vendor_solvers ONLY**
(CLAUDE.md forbids per-vendor PoW/sensor in public crates).

---

## 5. Recommended sequence

1. **FIX-C (oracle, per-profile)** first — cheap, and it *decides* the
   rest: it tells you whether firefox/pixel are blocked by **provider
   selection** (→ FIX-A reduces Phase-1 risk; residual → FIX-G private)
   or whether iphone is purely **worker-drain** (→ FIX-B).
2. **FIX-B (challenge-aware drain)** — cheap, high-confidence, fixes
   iphone *and* pays the AWS/booking dividend. Do this regardless.
3. **FIX-A (real Firefox TLS/H2)** — the big firefox lever; also unlocks
   firefox's DataDome gap-fails. Schedule as a dedicated net-stack task.
4. **FIX-E** to mop up any "solved but didn't re-fetch" residual.
5. **FIX-D** for pixel only if FIX-C shows an H2-delta cause (not a
   mobile-IP risk band).
6. **FIX-F** is the honest interim state for the consistency report.
7. **FIX-G** only if FIX-C proves a non-crypto provider is served to the
   failing profiles — and only in `vendor_solvers`.

**Definition of done for consistency:** homedepot L3-RENDERED on chrome +
iphone + (firefox after FIX-A) across ≥2 days; pixel either flips (FIX-D)
or is documented as a mobile-IP structural gap; `webgl_parity` /
`chrome147` parity stay green; the new per-profile `seccpt_probe` oracle
is checked in.

---

## 6. Sources

External:
- [Scrapfly — How to Bypass Akamai when Web Scraping in 2026](https://scrapfly.io/blog/posts/how-to-bypass-akamai-anti-scraping) — five-layer detection, device-fingerprint **consistency**
- [DEV (xkiian) — Bypassing Akamai v3 sensor_data with TLS in 2026](https://dev.to/xkiian/bypassing-akamai-v3-sensordata-with-tls-in-2026-why-the-deobfuscator-is-a-trap-5cjh) — the two-phase scoring (Phase 1 = TCP/TLS/HTTP frames before JS)
- [Akamai Blog — Bots Tampering with TLS to Avoid Detection](https://www.akamai.com/blog/security/bots-tampering-with-tls-to-avoid-detection) — TLS↔UA correlation as a first-class signal
- [VoidMob — How to Bypass Akamai Bot Detection 2026](https://voidmob.com/blog/how-to-bypass-akamai-bot-detection-2026) — per-claimed-browser PQ key-share / TLS coherence enforced pre-HTTP
- [Hyper Solutions — Handling 428 SEC-CPT](https://docs.hypersolutions.co/akamai-web/handling-428-status-code-sec-cpt) — crypto/behavioral/adaptive providers, `~3~` success, `chlg_duration` server-enforced wait

Internal:
- `docs/v0.1.0-match-workflows/00_DATA_per_profile_matrix.md:24` — the per-profile homedepot row (994281 / 2701 / 2734 / 2734)
- `docs/v0.1.0-parity-workflows/external/VENDOR_akamai.md` — §2.1 three-provider taxonomy, §3.2 drain truncation, §4 fix list, §7 Q2 (worker PoW open question)
- `docs/vNext/01_R-AKAMAI-SECCPT-FLAKE.md` — §3.1 unref'd `chlg_duration` timer, §4 FIX-1/2/3 (cross-day flake; FIX-1 landed, FIX-2 core open)
- `docs/v0.1.0-parity-workflows/sites/SITE_homedepot.md` — repo's prior homedepot conclusion (drain/timing)
- `docs/releases/v0.1.0-parity/26_AKAMAI_BMP_DEEP.md` §2.4, §4.1 — sec-cpt Flow B; adidas firefox-only "TLS class" top hypothesis (same lever as §2.1 here)
- `crates/stealth/src/presets.rs:466-474` (firefox `tls_impersonate` informational-only, `device_class: Desktop`), `:850-933` (pixel: MobileAndroid, cores 8), `:956+` (iphone: MobileIOS, `cpu_cores: 2`, `device_memory: 0`, "Phase 3" Safari TLS), `:938-940` (Safari profile maturity caveat)
- `crates/net/src/tls.rs:241-256` (TLS branch on `device_class` only; Desktop=Chrome147, Android=curves-only, iOS=Safari), `:280-295` (Safari 4-version range)
- `crates/net/src/h2_client.rs:73-90, 133-156` (H2 branch on `device_class` only; Desktop/Android share Chrome147 SETTINGS; iOS=Safari)
- `crates/net/src/headers.rs:17-19` (HTTP headers ARE per-browser — so the incoherence is TLS/H2 vs UA, not headers)
- `crates/browser/src/page.rs:242-247` (`is_seccpt_solved`), `:1884-1885` (`started_as_seccpt_challenge`), `:1975` (homedepot 45 s budget, host-keyed not profile-keyed), `:3594-3601` (`__keepLongTimersRefed` set on challenge docs — FIX-1 landed), `:3661` (50 ms inter-script drain — FIX-2 core NOT landed)
- `crates/js_runtime/src/js/timer_bootstrap.js:57-69` (`UNREF_THRESHOLD_MS=2000`, `__keepLongTimersRefed` gate)
- `crates/js_runtime/src/js/worker_bootstrap.js:147` (`hardwareConcurrency` exposed to workers from the profile), `crates/js_runtime/src/extensions/worker_ext.rs:9-10` ("Akamai's BMP v3 spawns workers via blob: URLs")
- commit `5216336` (worker secure-context fix — crypto.subtle in workers; "necessary-but-not-sufficient", a prerequisite for any worker-PoW sec-cpt path)
