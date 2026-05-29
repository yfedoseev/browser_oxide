# 03 — Pixel (Android Chrome) nav-error / empty-body cluster

**Profile:** `pixel_9_pro_chrome_148` (`crates/stealth/src/presets.rs:850`)
**Gate:** full gate 2026-05-29, isolated cold mode (`/tmp/full_gate_2026_05_28/bo_pixel_9_pro_chrome_148_cold.json`)
**Scope:** the 4 pixel-only consistency-gap sites that are NOT vendor challenges:
adidas, airbnb, yandex-ru, prime-video.
**Public-engine only** (per `CLAUDE.md` — no vendor bypass).

---

## 0. TL;DR

These 4 sites are **not** challenge blocks. None carries a vendor tag
(`SITE_VENDOR` map returns `None` for all four), none of the failing rows
contains a `*-CHL` / `PaH` / `BLOCKED` marker, and the **wall-clock time is
nearly identical to the chrome_148_macos run that succeeds**. The pixel
engine fetches and runs the same pages for the same duration as desktop
Chrome — it just hands back an **empty / stub DOM**. So this is a
**mobile-request-shape render/hydration reliability** problem, not TLS, not
a handshake failure, not an IP issue, and not a vendor challenge.

The single highest-leverage root cause is a **within-session Client-Hints
contradiction** that the pixel profile (and only mobile profiles) emits:
every JS-initiated `fetch()` / XHR sub-resource is built by
`chrome_headers_fetch`, which **hardcodes `sec-ch-ua-mobile: ?0`** and
**drops `sec-ch-ua-model` / `sec-ch-ua-form-factors`** — while the *same
session's* navigation request correctly sent `sec-ch-ua-mobile: ?1`,
`sec-ch-ua-platform: "Android"`, `sec-ch-ua-model: "Pixel 9 Pro"`. The
result is a request stream that says "I am an Android phone" on the
document and "I am a non-mobile device" (`"Android"` + `?0`) on every
hydration call. Modern bot/edge stacks treat `Android` + `?0` and a
nav-vs-fetch mobile-flag flip as a direct automation tell — and the SPAs on
these 4 sites stall their hydration (empty body) instead of being
explicitly challenged.

---

## 1. Evidence — the per-profile data signature

From the gate JSONs (`err` field, omitted in the matrix doc, is the decisive
clue):

| site | pixel tag/len/ms/err | iphone (mobile, PASS) | chrome (PASS) |
|---|---|---|---|
| airbnb | THIN-BODY **0** / 40976ms / err=None | L3 528559 / 41754ms | L3 589017 / 40916ms |
| yandex-ru | THIN-BODY **0** / 93154ms / err=None | L3 2712795 / 96521ms | L3 3243596 / 93108ms |
| prime-video | **ERROR 0** / 49891ms / err=`Navigation loop terminated without returning a page` | L3 508277 / 17848ms | L3 691747 / 50031ms |
| adidas | L3-RENDERED **2478** / 116590ms / err=None | L3 1309818 / **5707ms** | L3 1308551 / 25362ms |

Three signatures, three sub-causes:

1. **Empty-body twins — airbnb & yandex-ru (THIN-BODY 0).** `content()` =
   `document.documentElement.outerHTML` (`page.rs:855`) comes back empty.
   `engine_classify` tags `len < THIN_BODY_MAX_BYTES` (1000) as `THIN-BODY`
   (`classify.rs:236`). The wall time is **within 60 ms of the chrome run
   that returns 0.5–3.2 MB**. The engine spent the same compute; the page
   simply never populated. yandex.ru gets the 90 s heavy-SPA budget
   (`page.rs:1959`) and used all 93 s → hydration never completed under the
   mobile request shape. airbnb has no host entry → 15 s default
   (`page.rs:1986`) but extended to ~41 s via pending-nav budget-extend
   (`page.rs:2152`) and still ended empty.

2. **prime-video — ERROR "Navigation loop terminated without returning a
   page".** This string is returned only at `page.rs:3015` when the nav
   loop (`for iter in start_iter..iterations`, `page.rs:2017`) runs out of
   iterations/budget while a **JS-driven pending navigation keeps
   re-arming** (`location.href` / reload / meta-refresh re-fetch path,
   `page.rs:2946–3010`). The mobile Prime Video web client keeps bouncing
   the document (device/region/app-redirect SPA logic) and never settles,
   so the loop exhausts at ~50 s with no renderable page. Desktop Chrome
   settles into a real page in the same ~50 s; iPhone Safari settles in
   17.8 s.

3. **adidas — slow + stub (L3-RENDERED 2478, 116 s).** Different signature:
   not empty, not a crash — a *tiny rendered shell* after the budget-extend
   stack ran to ~116 s (adidas base budget is the 25 s Akamai-BMP tier,
   `page.rs:1980`; it stacked `nav_budget_extend` 25 s blocks,
   `page.rs:2008/2152`). iPhone renders the full 1.3 MB in **5.7 s**;
   desktop Chrome in 25 s. So the pixel/Android path is paying the full
   Akamai-BMP sensor round-trips AND still only yields a stub — the Android
   Akamai sensor branch is the laggard (see §3.4). This is the lowest-
   confidence item and may partly be Akamai serving a mobile interstitial.

The common thread for #1/#2: **same engine, same wall time as the passing
desktop run, but the mobile request shape yields an unhydrated/looping
SPA.** That isolates the cause to *what the pixel profile sends*, not to the
render engine itself (which produces 0.5–3.2 MB for the identical URLs under
chrome and iphone).

---

## 2. What the pixel profile sends differently — ruling out the wire layer

### 2.1 TLS and HTTP/2 are byte-identical to desktop Chrome (NOT the cause)

`tls.rs::chrome_connector` branches only on `profile.device_class`
(`tls.rs:241–246`). For `DeviceClass::MobileAndroid` it selects
`CURVES_ANDROID`, which is **defined as `= CURVES_DESKTOP`** (`tls.rs:104`).
Cipher list, sigalgs, extension Fisher-Yates shuffle, GREASE, ALPS, ECH
grease, Brotli cert compression, `key_shares_limit(2)` — all the desktop
Chrome path. `h2_client.rs::handshake` treats Android as `is_safari_ios ==
false` → the full Chrome SETTINGS (1,2,4,6), masp pseudo-order, 6 MB stream
window, weight-255 priority (`h2_client.rs:158–175`). So the pixel TLS/H2
fingerprint *is* the verified-real Chrome fingerprint.

`docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md` confirms
this is **correct and verified**: "TLS is already byte-perfect for Chrome
147 desktop, **Chrome Android**, and iOS Safari" (§ summary item 3), and the
only Android caveat is **L7 — LOW risk**: "`CURVES_ANDROID = CURVES_DESKTOP`
… unverified that Chrome Android 147 emits identical PQ curve order"
(NETWORK_fingerprint.md L127). A curve-order mismatch would manifest as a
*handshake delay/retry or a vendor challenge*, not a 40 s success-timed
empty body, and these 4 sites carry no vendor tag — so L7 is not the cause
here.

**Decisive timing argument:** a TLS/H2 handshake failure fails fast
(milliseconds–seconds) and surfaces as a `NetError` propagated to `err`
(`sweep_metrics.rs:208`). airbnb/yandex have `err=None` and burned the full
SPA budget — they completed the handshake and ran the page. The wire layer
is exonerated.

### 2.2 The Client-Hints fetch bug (PRIMARY root cause)

`crates/net/src/headers.rs::chrome_headers_fetch` (the builder for *every*
JS `fetch()`/XHR, reached via `nav_headers_fetch` → `fetch_get` /
`fetch_post_bytes`, `lib.rs:617/630`, and the op at
`fetch_ext.rs:687`):

```rust
// headers.rs:250  — HARDCODED, ignores device_class
headers.push(("sec-ch-ua-mobile".to_string(), "?0".to_string()));
// headers.rs:251  — platform from os_name = "Android" for pixel
headers.push(("sec-ch-ua-platform".to_string(), format!("\"{}\"", profile.os_name)));
// NO sec-ch-ua-model, NO sec-ch-ua-form-factors emitted on fetch at all
```

Contrast the **navigation** builder `chrome_headers_impl`, which is
device-aware and correct:

```rust
// headers.rs:339  is_mobile = MobileAndroid | MobileIOS
// headers.rs:345  sec-ch-ua-mobile = "?1" on mobile
// headers.rs:409  sec-ch-ua-form-factors = "Mobile" on mobile
// headers.rs:391  sec-ch-ua-model = profile.ua_model ("Pixel 9 Pro")  [accept-CH variant]
```

So within a single pixel session the server sees:

| request | sec-ch-ua-platform | sec-ch-ua-mobile | model | form-factors |
|---|---|---|---|---|
| top-level nav | `"Android"` | `?1` | `"Pixel 9 Pro"`* | `"Mobile"`* |
| every fetch/XHR | `"Android"` | **`?0`** | **(absent)** | **(absent)** |

(*on the Accept-CH-upgraded request)

Two independent tells, both controllable and both well-documented:

- **`"Android"` + `?0` is impossible on real Chrome.** Android Chrome
  always reports `sec-ch-ua-mobile: ?1`. Industry detection guides name this
  exact pair: *"An inconsistent combination like `macOS` + `?1` or
  `Android` + `Win64` is one of the easiest ways to spot automation"*
  ([wilico fingerprint-consistency guide][wilico]); *"Swapping only the
  User-Agent header will often break consistency with UA-CH … and that kind
  of mismatch can directly trigger bot detection"* ([wilico]). On a real
  Pixel, fetch carries `?1`.
- **Nav(`?1`) → fetch(`?0`) flip within one session** is a second tell:
  per-session UA-CH is supposed to be stable. SicuraNext's anti-automation
  write-up specifically uses Sec-Fetch + Client-Hints *inconsistency across
  request types* as the detector ([sicuranext][sec]). Akamai differentiates
  navigation vs fetch by `Sec-Fetch-Mode` and processes them through the
  same identity stack ([Akamai Early Hints][akamai]), so a mobile-flag flip
  between the two is visible.

Why empty body rather than a challenge: airbnb/yandex are React/Vue SPAs
whose first paint depends on JSON hydration fetches. When those fetches
present a self-contradicting UA-CH, the origin/edge soft-fails them
(serves an empty/200-stub payload, a personalization gate, or a
geo/app-redirect JSON) rather than a hard challenge — the SPA has nothing
to render, so `outerHTML` stays empty → THIN-BODY 0. This is consistent
with the "no vendor tag, full wall time, empty DOM" signature.

This bug affects **only mobile profiles** (pixel + would also hurt any
Android variant). Desktop Chrome is unaffected because `"macOS"/"Windows"`
+ `?0` is *correct*. That is exactly why chrome/firefox pass these sites and
pixel does not.

### 2.3 Secondary cross-API incoherence — pixel GPU profile is Apple M3

`pixel_9_pro_chrome_148()` sets `webgl_renderer: "ANGLE (Google, Mali-G715
MP7, OpenGL ES 3.2)"` (`presets.rs:879`) but
`gpu_profile: crate::gpu::apple_m3_macos()` with an explicit
`// TODO: add android_mali_g715 GPU profile` (`presets.rs:931`). Any
detector that reads the structured GpuProfile (WEBGL_DEBUG_RENDERER_INFO
masked/unmasked, supported extensions, max texture size) gets **Apple M3 GPU
capabilities under an Android Mali UA** — a hardware-vs-UA contradiction.
This is not the proximate cause of the empty body (it would gate
canvas/WebGL fingerprint challenges, not SPA hydration), but it is a real
mobile-coherence defect that will block any Android-routed site that
fingerprints WebGL, and it should be fixed in the same pass. iPhone has the
identical `apple_m3_macos()` TODO (`presets.rs:1035`) but iOS reports the
constant string `"Apple GPU"` so the M3 GPU profile is *coincidentally less
wrong* there — which partly explains why iPhone passes where pixel fails.

### 2.4 Compression / accept-encoding (NOT the cause)

`accept-encoding` is identical across chrome and pixel
(`gzip, deflate, br, zstd`, `headers.rs:451`) and `compression.rs::decompress`
is profile-agnostic (handles all four, `compression.rs:8–16`). A decode
failure would populate `err` and fail fast; it doesn't, so decompression is
not pixel-specific here.

---

## 3. Per-site mechanism

### 3.1 airbnb (THIN-BODY 0, 41 s) — HIGH confidence: §2.2 hydration

No vendor tag, no host budget entry (15 s default + pending-nav extend to
41 s). Airbnb's mobile web is a heavy SPA whose listings hydrate via XHR.
The `Android`+`?0` fetch contradiction (§2.2) is the most likely reason the
hydration payloads come back empty/redirected, leaving `outerHTML` empty.
Desktop Chrome (correct `macOS`+`?0`) hydrates to 589 KB at the same wall
time; iPhone Safari (no UA-CH at all — Safari sends none) hydrates to 528 KB.
**Note the asymmetry: Safari sending *zero* UA-CH headers is consistent;
pixel sending *contradictory* UA-CH is the worst of both worlds.**

### 3.2 yandex-ru (THIN-BODY 0, 93 s) — HIGH confidence: §2.2 hydration

Explicitly in the 90 s heavy-SPA budget tier (`page.rs:1959`). Used the full
93 s and still empty. Same §2.2 hydration-fetch mechanism; the larger
budget just means the engine tried longer before giving up. Desktop renders
3.2 MB, iPhone 2.7 MB at the same wall time.

### 3.3 prime-video (ERROR, 50 s) — MEDIUM confidence: loop + §2.2

`Navigation loop terminated without returning a page` (`page.rs:3015`). The
mobile Prime Video client runs device/region gating that keeps re-arming a
JS navigation (`location.href` / reload, handled at `page.rs:2946–3010`)
so the nav loop never converges and exhausts at ~50 s. Whether the
re-navigation is *driven by* the UA-CH contradiction (a "we don't recognize
this client, bounce it" path) or is an independent mobile-redirect-loop
parsing gap needs a hop trace (`BROWSER_OXIDE_DEBUG_REDIRECTS=1` +
`[navigate]` iter logging) — both desktop and iPhone settle, so it is
pixel-specific. Fixing §2.2 is the cheapest thing to try first.

### 3.4 adidas (L3-RENDERED 2478 stub, 116 s) — LOW confidence: Akamai mobile sensor

adidas is Akamai BMP (`page.rs:1980`, 25 s base budget; the presets header
comment also calls out "Akamai-protected adidas"). The pixel run stacked
budget-extends to 116 s and produced only a 2478-byte stub, while iPhone got
the full 1.3 MB in 5.7 s. Two non-exclusive possibilities: (a) Akamai serves
the Android Chrome a heavier/slower BMP sensor path that the engine doesn't
finish within budget; (b) the §2.2 contradiction trips a soft-deny that
holds the body at a stub. The 116 s near-timeout points more at (a) — a
*budget/sensor* problem — than at the clean empty-body of airbnb/yandex.
This one is the least certain to flip from the §2.2 fix alone.

---

## 4. Cross-references

- `docs/v0.1.0-parity-workflows/external/NETWORK_fingerprint.md` — Chrome
  Android TLS/H2 verified byte-perfect; L7 (Android==Desktop curves) is the
  only Android wire caveat and is LOW. Confirms the wire layer is not the
  cause here.
- `docs/v0.1.0-parity-workflows/external/ENGINE_camoufox_v150.md` /
  `external/VENDOR_akamai.md` — Akamai BMP sensor model (relevant to adidas
  §3.4).
- `docs/v0.1.0-parity-workflows/api/API_navigator_hardware.md` — the
  hardware/UA coherence surface (relevant to the §2.3 GPU-profile defect).
- `crates/stealth/src/presets.rs:931` — the `apple_m3_macos()` Android GPU
  TODO (already flagged in-tree).
- `crates/net/src/headers.rs:985` — existing test
  `pixel_android_emits_mobile_client_hints` covers the **nav** path but
  there is **no test for the fetch path**, which is why the `?0` regression
  went unnoticed.

---

## 5. Ranked fix list

Effort = engineering size. Confidence = will it flip the site. Gain =
expected pixel passes recovered (out of the 4). All public-engine.

### FIX-PX1 — device-aware `chrome_headers_fetch` (mobile UA-CH coherence)
- **What:** In `headers.rs::chrome_headers_fetch`, replace the hardcoded
  `sec-ch-ua-mobile: ?0` (line 250) with the `is_mobile` computation used by
  `chrome_headers_impl` (`headers.rs:339`), emitting `?1` for
  `MobileAndroid`/`MobileIOS`. Keep `sec-ch-ua-platform` reading
  `profile.os_name` (already correct). Real Chrome's *fetch* request only
  carries the **low-entropy** trio (`sec-ch-ua`, `-mobile`, `-platform`) by
  default — it does NOT add `-model`/`-form-factors`/high-entropy on fetch
  unless Accept-CH-upgraded — so the minimal correct change is just the
  `?1`/`?0` fix. (If a site Accept-CH-upgraded the origin, also splice the
  high-entropy mobile hints to match the nav, mirroring
  `chrome_headers_with_accept_ch`.)
- **Why:** removes the `Android`+`?0` impossibility and the nav→fetch
  mobile-flag flip (§2.2) — the single controllable bot tell shared by
  airbnb + yandex (and any future Android-routed SPA).
- **Effort:** ~10 LOC + 1 regression test (`fetch_emits_mobile_hint_on_pixel`,
  mirror of `pixel_android_emits_mobile_client_hints`).
- **Confidence:** **medium-high** for airbnb + yandex; **low-medium** for
  prime-video; **low** for adidas.
- **Expected pixel gain:** **+2 (airbnb, yandex-ru)**, plausibly +1
  (prime-video) → **+2 to +3**.
- **Risk:** zero for desktop (desktop `is_mobile==false` keeps `?0`). It only
  changes mobile profiles, which currently send a *wrong* value.

### FIX-PX2 — Android GPU profile (`android_mali_g715_chrome`)
- **What:** Add a real Mali-G715 `GpuProfile` (renderer string, extension
  list, limits matching Android Chrome/ANGLE) and assign it in
  `pixel_9_pro_chrome_148()` instead of `apple_m3_macos()` (`presets.rs:931`).
  Mirror onto `webgl_renderer` the same way `chrome_148_macos_sampled` does
  (`presets.rs:815`).
- **Why:** kills the Apple-M3-GPU-under-Android-UA contradiction (§2.3) — a
  hardware-vs-UA tell that gates WebGL-fingerprinting vendors on *all*
  Android-routed sites, not just these 4.
- **Effort:** medium (~1 GPU profile struct, ~40 LOC + validate() coherence,
  following the existing `gpu.rs` apple_* pattern).
- **Confidence:** medium that it improves Android coherence broadly; low that
  it flips *these specific 4* (they aren't WebGL-gated). Do it for
  consistency/durability, not as the airbnb/yandex fix.
- **Expected pixel gain:** 0–1 direct here; protects future Android sites.

### FIX-PX3 — prime-video redirect/reload-loop diagnosis
- **What:** Run pixel prime-video once with `BROWSER_OXIDE_DEBUG_REDIRECTS=1`
  and the `[navigate]` iter trace (already in `page.rs:2934`) to see whether
  the loop is (a) a parseable mobile redirect chain the loop mishandles, or
  (b) JS re-arming `__pendingNavigation` because the client rejects the
  UA-CH (in which case FIX-PX1 resolves it). If (a), add a same-URL
  re-navigation guard / loop-detection in the pending-nav handler
  (`page.rs:2946–3010`).
- **Why:** turns the `ERROR` into either a pass (if FIX-PX1 fixes it) or a
  scoped loop bug.
- **Effort:** low to diagnose; small-medium if a loop guard is needed.
- **Confidence:** medium it flips after FIX-PX1; otherwise a contained fix.
- **Expected pixel gain:** +1 (prime-video).

### FIX-PX4 — adidas Android Akamai-BMP budget/sensor (DEFER)
- **What:** After FIX-PX1+PX2, re-measure adidas. If still a stub at ~116 s,
  it's the Android Akamai-BMP sensor path being slow/incomplete — raise the
  adidas host budget to the 45 s heavy tier (`page.rs:1980`→ heavy-PoW
  block) OR accept it as an Akamai-mobile content difference. Lowest
  priority; don't gate the cluster on it.
- **Effort:** low (budget tier change) to large (Android BMP sensor work).
- **Confidence:** low.
- **Expected pixel gain:** +0 to +1 (adidas).

---

## 6. Bottom line

The pixel cluster is a **mobile request-shape coherence** problem, not a
wire/stealth or challenge problem. **FIX-PX1 (10 LOC) is the dominant
move** — it removes the `Android`+`?0` UA-CH contradiction that the empty-
body SPAs (airbnb, yandex-ru) almost certainly choke on, and likely
prime-video too. Realistic outcome: **+2 to +4 pixel passes**, taking pixel
from 108 toward 110–112. FIX-PX2 (Android GPU) is the durable coherence
fix that hardens *all* Android routing for the v150-per-profile-parity goal,
even though it isn't the proximate cause of these four.

[wilico]: https://wilico.co.jp/en/blog/browser-fingerprint-inconsistency-detection-consistency-check
[sec]: https://blog.sicuranext.com/sec-fetch-and-client-hints-a-powerful-tool-against-automation/
[akamai]: https://techdocs.akamai.com/property-mgr/docs/early-hints
