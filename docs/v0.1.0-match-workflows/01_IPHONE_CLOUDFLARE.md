# 01 — iPhone profile × Cloudflare: per-profile root cause + fix plan

**Cluster:** `iphone_15_pro_safari_18` loses **6 sites to Cloudflare**
that `chrome_148_macos`, `pixel_9_pro_chrome_148`, and
`firefox_135_macos` all PASS:

| site | iphone | chrome | pixel | firefox |
|---|---|---|---|---|
| economist | `Cloudflare-CHL 5891` | ✅ 529205 | ✅ 528716 | ✅ 510197 |
| ecosia | `Cloudflare-CHL 5444` | ✅ 69630 | ✅ 69273 | ✅ 69515 |
| ft | `Cloudflare-CHL 271064` | ✅ 328537 | ✅ 328494 | ✅ 333147 |
| openai | `Cloudflare-CHL 10807` | ✅ 423760 | ✅ 423727 | ✅ 423715 |
| quora | `Cloudflare-CHL 5843` | ✅ 78196 | ✅ 78713 | ✅ 78206 |
| udemy | `Cloudflare-CHL 5929` | ✅ 476498 | ✅ 476498 | ✅ 476507 |

Data: `docs/v0.1.0-match-workflows/00_DATA_per_profile_matrix.md`
rows 21-33. This is the single largest per-profile cluster — closing it
takes iphone from 108 → 114 and turns 6 routed-fallback sites into
"all-four-pass" consistency wins.

**This doc supersedes the planning-stage hypothesis tree in
`docs/releases/v0.1.0-parity/25_CLOUDFLARE_DEEP.md §3` (H1-H4).** Since
that doc was written, the engine-side Cloudflare *recognition + render*
plumbing has SHIPPED (the `cf-mitigated` log, the
`started_as_cf_challenge` poll, the cookie-delta retry). So the residual
is NOT "we can't render the challenge" — it is "the iphone profile gets
*served* the challenge in the first place where the other three don't."
That is a **fingerprint trigger problem**, and this doc identifies the
load-bearing trigger at code level.

---

## 1. The signature in the data tells us this is a TRIGGER, not a RENDER, gap

Two facts from the matrix pin the diagnosis:

1. **The other three profiles pass the *same six origins* from the *same
   IP*.** So the origins are not IP-banning us, and they are not serving
   a challenge to "everyone from this datacenter." The challenge fires
   on a *per-profile discriminator*, not on IP/behaviour.

2. **The bodies are CHALLENGE shells, not hard blocks.** `economist
   5891`, `ecosia 5444`, `quora 5843`, `udemy 5929`, `openai 10807` are
   all 5-11 KB — the Cloudflare Managed-Challenge orchestrator shell
   (`_cf_chl_opt` + `/cdn-cgi/challenge-platform/...` script ref, per
   `25_CLOUDFLARE_DEEP.md §2.1`). `ft 271064` is the larger
   JS-Detections variant. None is a 403 + "Sorry you have been blocked"
   1020 page. So Cloudflare's verdict for iphone is **"challenge"**, not
   **"block"** — the WAF risk score landed in the mid-band, above the
   challenge threshold but below the hard-block threshold, *only for the
   iphone profile*.

A mid-band score that flips on profile = the profile's
**JA4 (TLS) + JA4H (HTTP) + UA-class** combination scores worse than
the Chrome/Firefox profiles' combinations. The DNA of that score is
documented by Cloudflare itself (§2). The concrete code-level cause is
in §3.

---

## 2. External research — why Cloudflare scores iOS-Safari traffic on the JA4 corpus, not on byte-correctness alone

### 2.1 Cloudflare's JA4 is a *corpus-relative* signal, not a binary

The decisive external finding is Cloudflare's own
[JA4 Signals blog][cf-ja4-signals]: Cloudflare does not just check "is
this JA4 a known browser." It computes **inter-request signals over the
last hour of all traffic per JA4** — including a **"browser ratio"**
(what fraction of requests with this exact JA4 look like real browser
sessions), cache ratio, and request-volume quantiles. Fingerprints
"are used on their own for simple rules, and they underpin complex
machine learning models as well." Per the
[bot detection engines doc][cf-engines], the ML engine "accounts for
the majority of all detections" and scores 1-99 from
"headers, session characteristics, and browser signals."

The operational consequence for us:

- A JA4 that **exactly matches** a high-volume real-browser fingerprint
  inherits that fingerprint's low-risk, high-browser-ratio history.
- A JA4 that is **one cipher off** from real iOS Safari is a *novel*
  fingerprint with near-zero corpus history. Cloudflare's ML has no
  "this JA4 is 99% real Safari sessions" prior to lean on, so it falls
  back to challenge. This is exactly the
  [JA4-in-the-wild "rarity ⇒ false-positive challenge"][cf-ja4-wild]
  failure mode — benign-but-rare fingerprints get challenged.

So for the iphone profile, **near-correct is worse than the desktop
Chrome JA4**, because the desktop Chrome JA4 is the single most common
fingerprint on the internet (huge corpus, overwhelming browser ratio)
while a slightly-wrong iOS-Safari JA4 is a corpus orphan.

### 2.2 iOS Safari is intrinsically a smaller, tighter corpus

Two compounding factors, both from public sources:

- **Mobile Safari is a much smaller share of bot traffic than Chrome
  desktop**, so vendors have *fewer* confident-real samples to anchor a
  rare iOS-Safari JA4 to (the syndrome named in
  `11_PER_PROFILE_STRATEGY.md §5.3`, quoted in `25_CLOUDFLARE_DEEP.md
  §3`). The challenge-vs-pass threshold for the iOS-Safari class sits
  lower.
- **iOS Safari does NOT permute its TLS extensions.** Per the lexiforest
  [`safari_18.0_iOS.yaml`][lexi-safari] signature, Safari emits a
  **fixed** extension order every handshake (unlike Chrome, which has
  shuffled since v110 — [JA4 background][cf-ja4-wild]). That means the
  iOS-Safari JA4 is *deterministic*: there is exactly ONE correct
  iOS-Safari-18 JA4 string. Chrome's per-handshake shuffle changes
  extension *order* but JA4 sorts the codepoints, so the JA4 stays
  stable — the point is that for Safari there is no shuffle to hide
  behind: if our cipher *set* is wrong, our single deterministic JA4 is
  *visibly* wrong and *consistently* wrong on every request.

### 2.3 What real iOS 18 Safari actually puts on the wire (reference)

From the lexiforest [`safari_18.0_iOS.yaml`][lexi-safari] (cross-checked
against the [curl_cffi safari18_ios JA4 discrepancy report][cf-460] which
confirms `safari18_ios` JA4/JA4_o are a known moving target):

- **Ciphers (20 real + GREASE):** AES-128/256-GCM, CHACHA20, the ECDHE
  ECDSA/RSA GCM + CHACHA suites, **then the ECDHE-AES-CBC-SHA(256/384)
  block, then RSA-AES-GCM, then RSA-AES-CBC-SHA, then
  `ECDHE-ECDSA-AES256-SHA` + `ECDHE-RSA-AES256-SHA`, then a SINGLE
  3DES** (`DES-CBC3-SHA` = `TLS_RSA_WITH_3DES_EDE_CBC_SHA`, 0x000A).
- **Extensions (13 real + 2 GREASE + padding), FIXED order:**
  server_name, extended_master_secret, renegotiation_info,
  supported_groups, ec_point_formats, ALPN, status_request,
  signature_algorithms, signed_certificate_timestamp, key_share,
  psk_key_exchange_modes, supported_versions, compress_certificate.
- **Supported groups:** GREASE, x25519, secp256r1, secp384r1, secp521r1
  — **no MLKEM / post-quantum.** Confirmed by
  [Apple's TLS security doc][apple-tls]: X25519MLKEM768 ships on
  **iOS 26**, not iOS 18. (So BO's "no PQ for iOS" is correct.)
- **H2 SETTINGS:** keys 2/3/4/(8)/9 — push off, max-concurrent 100,
  2 MB window, NO_RFC7540_PRIORITIES.

---

> **⚠️ CORRECTION (2026-05-29, verified against ground truth — supersedes §3
> below).** §3's load-bearing claim is **FALSE**. I decoded the actual
> `CIPHER_LIST_SAFARI_IOS` (`tls.rs:111-132`) against the authoritative
> `lexiforest/curl-impersonate/tests/signatures/safari_18.0_iOS.yaml`:
> `4865,4866,4867,49196,49195,52393,49200,49199,52392,49162,49161,49172,49171,157,156,53,47,49160,49170,10`.
> **BO matches it byte-for-byte — both the SET (JA4 cipher-hash) and the ORDER
> (JA4_o).** Real iOS-18 Safari *does* carry all THREE 3DES suites
> (`49160=0xC008`, `49170=0xC012`, `10=0x000A`); the doc's "real Safari has only
> one 3DES" was transcribed from a wrong reference. Curves (`29,23,24,25`) and
> sigalgs (incl. the duplicated `rsa_pss_rsae_sha384`) also match. **Do NOT make
> the §3 cipher change — it would break the currently-correct list.** The real
> residual (if TLS at all) is the §4.1 ordered-variant: BoringSSL's auto-emitted
> PADDING / trailing-GREASE *position* (JA4_o), confirmable only with a live
> `tls.peet.ws` capture (added: `crates/net/tests/tls_fingerprint.rs::capture_profiles_ja4`).
> If the live JA4_o matches the reference too, the iphone-Cloudflare trigger is
> **not** the TLS layer and the search moves to JA4H (HTTP/2) / Accept headers.

## 3. ROOT CAUSE — the iphone TLS cipher SET is wrong (JA4 cipher-hash mismatch)

> **(Falsified — see the CORRECTION banner above. Retained for the analysis
> trail only.)**

BO's iOS-Safari TLS branch is *substantially* implemented — this is the
good news that retires `25_CLOUDFLARE_DEEP.md` H4's "maybe it falls back
to Chrome." `crates/net/src/tls.rs:233-369` correctly branches
`MobileIOS` to: distinct cipher list, distinct sigalgs (incl. the
duplicated `rsa_pss_rsae_sha384` Apple bug), `CURVES_SAFARI_IOS` (no PQ,
adds P-521), a FIXED extension permutation, Zlib cert compression,
`NO_TICKET`, and skips ECH-GREASE + ALPS in `configure_connection`. The
H2 branch (`h2_client.rs:85-175`) is equally correct (msap pseudo-order,
2/3/4/9 settings, no priority frame). The JS surface for iOS (no
`navigator.connection`, no `deviceMemory`, `window.orientation` present,
`hardwareConcurrency: 2`) is gated correctly at
`window_bootstrap.js:1016` (`_isMobileIOS()`).

**But the cipher LIST has the wrong set at the tail.** Compare
`crates/net/src/tls.rs:111-132` (`CIPHER_LIST_SAFARI_IOS`) against the
lexiforest reference:

| position | BO `CIPHER_LIST_SAFARI_IOS` (tail) | real iOS 18 Safari (lexiforest) |
|---|---|---|
| 16 | `TLS_RSA_WITH_AES_256_CBC_SHA` (0x0035) | `AES256-SHA` (0x0035) ✅ |
| 17 | `TLS_RSA_WITH_AES_128_CBC_SHA` (0x002F) | `AES128-SHA` (0x002F) ✅ |
| 18 | **`TLS_ECDHE_ECDSA_WITH_3DES_EDE_CBC_SHA`** (0xC008) ❌ | **`ECDHE-ECDSA-AES256-SHA`** (0xC00A) |
| 19 | **`TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA`** (0xC012) ❌ | **`ECDHE-RSA-AES256-SHA`** (0xC014) |
| 20 | `TLS_RSA_WITH_3DES_EDE_CBC_SHA` (0x000A) | `DES-CBC3-SHA` (0x000A) ✅ |

BO emits **THREE 3DES ciphers** (0xC008, 0xC012, 0x000A); real iOS 18
Safari emits **ONE** (0x000A) and instead carries two
**ECDHE-AES256-CBC-SHA** suites (0xC00A, 0xC014) in slots 18-19.

### Why this is the load-bearing trigger

JA4's cipher-hash is `SHA256(sorted-ascending IANA cipher codepoints)`
truncated to 12 hex (per `23_TLS_HTTP_FINGERPRINT_REFERENCE.md §2.2`).
The **set** of codepoints is hashed. BO's set contains `{0xC008, 0xC012}`
which real Safari does NOT have, and is MISSING `{0xC00A, 0xC014}` which
real Safari DOES have. Therefore:

- BO's JA4 cipher-hash ≠ any real iOS-Safari-18 cipher-hash.
- The cipher **count digit** happens to still read `20` (both lists are
  20 real ciphers), so the JA4 *prefix* (`t13d20…`) looks Safari-shaped
  — which is the trap. BO clears the cheap "count" check but fails the
  expensive "hash matches a known-good fingerprint" check.
- Per §2.1, the result is a JA4 with **near-zero corpus history**: not a
  known iOS Safari, not a known anything. Cloudflare's ML has no
  low-risk prior → mid-band score → **Managed Challenge**. The Chrome
  and Firefox profiles route on JA4s with massive corpus history, so
  they clear.

This single set-difference is the highest-probability cause that (a) is
profile-specific (only the iOS branch uses this list), (b) explains a
*challenge* not a *block* (the JA4 is plausible-Safari-shaped, not
garbage), and (c) explains why all six sites flip together (they share
one Cloudflare ML model keyed on the same wrong JA4).

> Provenance note: `tls.rs:107-110` cites `safari_18.0_iOS.yaml` as the
> source, but the 3 vs 1 3DES discrepancy means the constant was either
> transcribed from an older/different reference (e.g. a desktop Safari or
> a curl-impersonate variant that does carry the extra 3DES suites) or
> hand-edited. The
> [curl_cffi safari18_ios JA4-mismatch report][cf-460] documents that
> exactly this profile's JA4 has been wrong/unstable upstream — so the
> in-repo cite being stale is the most likely explanation.

---

## 4. Secondary candidates (lower probability; check after the cipher fix)

These are NOT believed load-bearing on their own, but should be
re-verified after the cipher fix in case the JA4 correction only
partially closes the gap.

### 4.1 The fixed extension permutation may be missing `padding` / 2nd GREASE
`SAFARI_IOS_EXTENSION_PERMUTATION` (`tls.rs:169-183`) lists 13
extensions and the comment (`tls.rs:159-168`) acknowledges PADDING +
the trailing GREASE are auto-emitted by BoringSSL "outside the
permutation table" and that "PADDING positional ordering requires raw
extension injection — deferred." The lexiforest reference ends
`…compress_certificate, GREASE, padding`. JA4's **extension count**
digit excludes GREASE, so 13 is the right count digit either way; but if
BoringSSL emits PADDING in a position real Safari doesn't, or omits the
trailing GREASE, a strict JA4_o (ordered variant — the
[curl_cffi report][cf-460] explicitly flags `ja4_o` as differing) or a
peetprint-style check could still see daylight. **Verify with a
`tls.peet.ws` capture after the cipher fix; only chase if the cipher fix
alone doesn't flip the six sites.**

### 4.2 `tls_impersonate` string is cosmetic — not a bug, just confirm
`profile.tls_impersonate = "safari_18_ios"` (`presets.rs:1003`) is the
informational label only; the real branch is on `device_class ==
MobileIOS` (`tls.rs:241`), which the iphone preset sets
(`presets.rs:1002`). No action — documented here to retire the stale
preset comment at `presets.rs:937-940` ("without it the profile produces
a Chrome-flavored ClientHello") which is **no longer true**: the iOS
branch is fully wired. That comment should be deleted to avoid future
confusion.

### 4.3 Safari accept-language q-step is approximated, not verified
`build_safari_accept_language` (`headers.rs:808-810`) just delegates to
the Chrome builder (`en-US,en;q=0.9`). Real iOS Safari sends
`en-US,en;q=0.9` for an en-US device, so this is *probably* fine — but
it is the JA4H `lang4` input and an unverified approximation. Low
priority; verify in the same `tls.peet.ws` / real-device capture.

### 4.4 H1 null hypothesis (iOS class is just harder) — now unlikely
`25_CLOUDFLARE_DEEP.md §3 H1` posited "real iOS Safari would also be
challenged here." The cipher-set bug in §3 is a concrete, falsifiable
engine defect that fully explains the data without invoking H1. H1
drops to residual: only entertain it if a *byte-correct* JA4 (after the
§3 fix, verified equal to a real iPhone capture) still gets challenged.

---

## 5. Ranked fix list

Effort = approx engineering hours. Gain = expected iphone-profile sites
recovered (all 6 already pass on the other 3 profiles, so each is also a
consistency win and a routed win where iphone was the fallback).
"Public-engine" = lives in public crates per CLAUDE.md (no
`vendor_solvers` needed — this is fingerprint correctness, not a
per-vendor bypass).

| # | Fix | File:line | Effort | Conf | Expected gain | Public-engine |
|---|---|---|---|--:|---|---|
| 1 | **Correct the iOS Safari cipher SET**: replace slots 18-19 of `CIPHER_LIST_SAFARI_IOS` — drop `TLS_ECDHE_ECDSA_WITH_3DES_EDE_CBC_SHA` (0xC008) + `TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA` (0xC012); insert `TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA` (0xC00A) + `TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA` (0xC014) before the single `TLS_RSA_WITH_3DES_EDE_CBC_SHA` (0x000A). | `crates/net/src/tls.rs:111-132` | 1-2 h | **high** | **+4 to +6 iphone** (acceptance bar +4; target all 6) | yes |
| 2 | **Add a JA4 byte-regression vector for the Safari cipher list** mirroring `tls_fingerprint_vectors_no_silent_drift` — pin the corrected `CIPHER_LIST_SAFARI_IOS` string so it can never silently drift again, and assert the cipher count digit + the absence of 0xC008/0xC012. | `crates/net/src/tls.rs` (test mod ~476) | 1 h | high | 0 direct (regression insurance for #1) | yes |
| 3 | **Capture the corrected iphone JA4 from `tls.peet.ws/api/all`** (network-gated, `--ignored`) and diff against a real iOS-18 Safari JA4 (BrowserStack/real iPhone). Confirms #1 closed the gap and surfaces any residual §4.1 extension/padding daylight. | `crates/net/tests/tls_fingerprint.rs` | 2-3 h | high | validation (gates whether §4.1 is needed) | yes |
| 4 | **(Only if #1+#3 leave residual)** Raw-inject the Safari `padding` + trailing GREASE in the real positions so JA4_o / peetprint match byte-exact, per `tls.rs:159-168` deferred note. | `crates/net/src/tls.rs:169-183` + raw ext injection | 4-8 h | med | +0 to +2 (residual sites only) | yes |
| 5 | **Delete the stale `presets.rs:937-940` comment** claiming the iOS profile emits a Chrome ClientHello — it is wired; the comment misleads future debugging. | `crates/stealth/src/presets.rs:937-940` | 5 min | high | 0 (hygiene) | yes |
| 6 | **Verify Safari accept-language q-step** against a real iOS capture; only change if it differs from `en-US,en;q=0.9`. | `headers.rs:808-810` | 1 h | low | 0 to +1 | yes |

### Recommended sequence
Ship **#1 + #2 together** (the fix + its regression lock). Then run
**#3** to confirm the six sites flip (cannot run live now — a competitor
benchmark holds the single IP; queue #3 for when the IP is free). If #3
shows all six pass, stop — **#4/#6 are unnecessary**. If a subset still
challenges, run #4. **#5** any time (pure hygiene).

### Expected outcome
The cipher-set fix moves the iphone JA4 onto a real, high-corpus-history
iOS-Safari-18 fingerprint. Per §2.1 that is the difference between a
corpus-orphan (mid-band → challenge) and a known-good Safari (low-risk →
pass). Acceptance bar **+4 iphone** (108 → 112, matching v150
single-engine); target **+6** (108 → 114, iphone reaches/exceeds the
other three profiles and all six sites become all-four-pass consistency
wins).

---

## 6. Cross-references

- `docs/v0.1.0-match-workflows/00_DATA_per_profile_matrix.md` — the data.
- `docs/releases/v0.1.0-parity/25_CLOUDFLARE_DEEP.md` — the CF product
  taxonomy (§0), recognition markers (§1), mechanism (§2), and the
  now-superseded H1-H4 hypothesis tree (§3). The render/recognise
  primitives (§4) are largely shipped (`page.rs:1207` cf-mitigated log,
  `page.rs:1898` started_as_cf_challenge, `page.rs:2230/2435` retry).
- `docs/releases/v0.1.0-parity/23_TLS_HTTP_FINGERPRINT_REFERENCE.md §1.4`
  — the `device_class`→branch mapping; **its row for `safari_18_ios`
  says "20 ciphers incl. 3DES" — that row should be corrected to note
  the cipher-set bug fixed by §5 #1 of this doc.**
- `docs/releases/v0.1.0-parity/11_PER_PROFILE_STRATEGY.md §5.3` — the
  "iphone is the specialist" framing (now refined: the specialism was
  partly a fixable cipher-set defect, not purely environmental).
- `crates/net/src/tls.rs:111-183` — the iOS Safari TLS constants.
- `crates/net/src/h2_client.rs:52-175` — the iOS Safari H2 constants
  (verified correct; no change).
- `crates/js_runtime/src/js/window_bootstrap.js:1016` — iOS JS-surface
  gating (verified correct; no change).

[cf-ja4-signals]: https://blog.cloudflare.com/ja4-signals/
[cf-engines]: https://developers.cloudflare.com/bots/concepts/bot-detection-engines/
[cf-ja4-wild]: https://deveshshetty.com/blog/ja4-client-fingerprinting/
[lexi-safari]: https://github.com/lexiforest/curl-impersonate/blob/main/tests/signatures/safari_18.0_iOS.yaml
[cf-460]: https://github.com/lexiforest/curl_cffi/issues/460
[apple-tls]: https://support.apple.com/guide/security/tls-security-sec100a75d12/web
