# 39 — Network-layer fingerprinting (cross-cutting)

**Status:** reference + cross-vendor leverage analysis
**Audience:** anyone deciding "where should the next week of engineering effort go?" at the network boundary — TLS / HTTP2 / WebRTC parity audits, JA4 capture campaigns, ECH planning, the WebRTC `RTCPeerConnection` shape, and the cross-layer "TLS-says-Chrome, JS-says-Headless" mismatch class that vendor scoring models look for.
**Companion docs:** `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` (the TLS/HTTP2 implementation-level reference — what bytes BO actually emits per profile and how they were verified; this chapter sits one level above it, organized by TECHNIQUE not vendor), `18_ANTI_BOT_VENDOR_COOKBOOK.md` (per-vendor classifier-marker lookup), `25_CLOUDFLARE_DEEP.md` (Cloudflare's particular TLS+H2 inspection), `26_AKAMAI_BMP_DEEP.md` (Akamai's H2 hash + sec-cpt), `27_VENDOR_COMPETITIVE_MATRIX.md` (which engine wins which vendor and why), `17_WEB_API_PARITY_MATRIX.md` §2.9 (WebRTC parity row), `16_STEALTH_FINGERPRINT_AUDIT.md` §Crypto/WebRTC (audit-tier ratings).

**One-paragraph thesis:** Network-layer fingerprinting (TLS ClientHello, HTTP/2 SETTINGS frame, WebRTC ICE candidates) is the **first signal a vendor sees** — it happens before a single byte of HTML reaches the engine, before the JS runtime starts, before any rendering. The vendor decides at the edge (Fastly, Cloudflare, AWS WAF, Akamai BMP, DataDome, Kasada) whether to serve the full origin response, a small interstitial, or a hard 403, based purely on what came out of `boring2`'s `SSL_write` and the first HTTP/2 frame off the wire. This chapter is the cross-vendor view: which technique gives biggest leverage (TLS > HTTP/2 > WebRTC), where BO already wins (TLS — byte-perfect Chrome 147 via `boring2 4.15.15`), where BO has remaining gaps (HTTP/2 SETTINGS audit + JA4 ground-truth capture, both v0.1.0 acceptance items), and where the post-2026 frontier is (ECH eliminating SNI from edge inspection, shifting the fingerprint landscape to TCP/QUIC transport parameters).

---

## 1. Why network-layer fingerprinting matters

### 1.1 Detection happens BEFORE the page renders

In the canonical anti-bot pipeline, the sequence per request is:

```
TCP SYN  →  TLS ClientHello  →  HTTP/2 PREFACE + SETTINGS  →  HTTP request headers  →  Edge decision  →  (only then) origin or JS challenge
```

Every box to the left of "Edge decision" is **wire-byte-deterministic**: the server inspects bytes BO's `crates/net/src/tls.rs` and `crates/net/src/h2_client.rs` put on the socket, and decides without rendering anything. There is no fix for "send the wrong TLS ClientHello, then run great JS later." The JS never runs.

This is the single most important property to internalize when reasoning about why BO's TLS layer is so heavily engineered (per `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` §1) — it's not a polish detail, it's the entire game for the WAF/edge tier. The downstream JS-runtime parity work (per `17_WEB_API_PARITY_MATRIX.md`) only matters AFTER the network layer wins the right to receive a real HTML body.

### 1.2 Detection at the edge is cheap for the vendor

A JA4 hash compute is microseconds per request on the vendor's edge proxy. An HTTP/2 SETTINGS frame parse is single-digit microseconds. By contrast, a JS challenge (Cloudflare Managed Challenge, Kasada `/ips.js`, Akamai BMP sensor) costs:

- 30-300 KB of JS to serve on every connection
- ~50-200 ms of client CPU to run
- a round-trip for the proof token
- per-tenant logging + state for the challenge

Network-layer scoring is therefore the **first cut**: if JA4 + HTTP/2 hash + UA agree (single-source-of-truth check), the vendor can skip the JS challenge entirely (Cloudflare "Allow"); if they disagree (TLS says Chrome, UA says Firefox), the vendor escalates straight to the highest-cost defense.

The structural consequence: getting TLS/HTTP2 byte-perfect doesn't just unlock the obvious "they don't block us" wins — it unlocks the *frictionless* wins where the vendor never even sends a challenge, which is most of the long tail of "low-risk" sites.

### 1.3 Hard to fix at the engine level

Unlike JS-runtime fingerprints (which we can patch in one `crates/js_runtime/src/js/window_bootstrap.js` line per leak per `16_STEALTH_FINGERPRINT_AUDIT.md`), TLS fingerprints require:

1. A TLS library that exposes the per-handshake levers (cipher order, extension permutation, curve order, GREASE, cert compression, ALPS — none of which `rustls 0.23.40` currently exposes; see `23 §1.1`)
2. Patching that library to write the EXACT bytes (Chrome's MLKEM768 key share leading; Apple's `rsa_pss_rsae_sha384` duplicate sigalg bug; the Chrome 131+ per-handshake Fisher-Yates extension shuffle)
3. Maintaining the patches across browser-major bumps (Chrome 131 added MLKEM; iOS 26 will add MLKEM; Apple already moved Safari iOS 18.0 → 18.4 to drop ENABLE_CONNECT_PROTOCOL)

This is why we depend on `boring2 4.15.15` (Cloudflare's BoringSSL fork) and refuse to upgrade to `boring2 5.0-alpha` despite it being newer — 5.0 removed the impersonation APIs we rely on (`CertCompressionAlgorithm`, `SslCurve`, `SSL_CTX_set_extension_permutation`, `set_curves`, the `cert-compression` and `pq-experimental` cargo features). See `crates/net/Cargo.toml:23-30` for the load-bearing comment and `23 §1.2` for the long-form rationale.

### 1.4 The cross-layer mismatch class (what vendors weight heaviest)

Per the HTTP/2 fingerprinting research aggregated at <https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide>, "the strongest detection signal is a cross-layer mismatch between TLS fingerprints, HTTP/2 behavior, HTTP/3/QUIC parameters, and the browser surface exposed to JavaScript."

This is what every modern anti-bot product weights heaviest:

- TLS JA4 says "Chrome 148 desktop" → check
- HTTP/2 SETTINGS hash says "Chrome 148 desktop" → check
- HTTP header order says "Chrome 148 desktop" → check
- JS-exposed `navigator.userAgent` says "Chrome 148 desktop" → check
- JS-exposed `navigator.platform` + WebGL renderer string + Canvas 2D fingerprint → must all say "Chrome 148 desktop"
- WebRTC ICE candidate shape → must say "Chrome desktop with mDNS anonymization since 2019"

Any one of these disagreeing with the others = bot. This is why a Python/Go `requests` library client that *fakes* the `User-Agent` header is immediately caught — the TLS JA4 and HTTP/2 hash betray it. And it's why BO's combination of byte-perfect TLS + matching HTTP/2 + matching headers + matching JS surface is hard to replicate piece-by-piece.

---

## 2. TLS fingerprinting deep dive

### 2.1 JA3 — the legacy MD5-based fingerprint

**Spec:** <https://github.com/salesforce/ja3>

JA3 is the original 2017 Salesforce work. Format (from the upstream README):

```
JA3 = MD5(
    SSLVersion,
    Cipher,
    SSLExtension,
    EllipticCurve,
    EllipticCurvePointFormat
)
```

Each field is comma-separated; values within each field are hyphen-separated decimal IANA codepoints. Empty fields are kept as empty strings (e.g. `769,4-5-10-9-100-98-3-6-19-18-99,,,`).

Example (from the upstream README): input `769,47-53-5-10-49161-49162-49171-49172-50-56-19-4,0-10-11,23-24-25,0` → output `ada70206e40642a3e4461f35503241d5`.

**GREASE handling:** GREASE values (RFC 8701) are stripped before hashing — the JA3 string never includes them.

**JA3S** (server variant): same idea but for the ServerHello, with three fields: `SSLVersion,Cipher,SSLExtension`. Used historically for C2 detection (client JA3 + server JA3S = unique TLS conversation).

**Why JA3 has been deprecated by JA4 (per the upstream archive note):**

- MD5 hashes the structure away — you can't tell from the JA3 hash WHICH field drifted
- No TLS-version discrimination (JA3 of a TLS 1.2 vs TLS 1.3 handshake hashes the same way if version + ciphers + extensions match — bad)
- No structure across TLS-related protocols (QUIC, DTLS not separated)
- No HTTP-layer companion (vendors had to roll their own; Akamai's H2 hash is one such, but inconsistent across the field)

**JA3 status in BO:** we do NOT compute or publish JA3. The JA3 of our handshakes is whatever falls out of the JA4-correct constants in `crates/net/src/tls.rs:60-220`. Per `23 §2.1`: "There is no JA3 test gate." Older vendors (Akamai pre-2023, Imperva on some plans) still consume JA3 — but the cipher/extension/curve list we emit is the verified-real Chrome 147 list, so our JA3 IS Chrome 147's JA3 by construction. We don't need to verify it separately; the JA4 capture covers the same fields plus more.

### 2.2 JA4 — the current standard

**Spec:** <https://github.com/FoxIO-LLC/ja4> + <https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4.md>

The JA4 family is FoxIO's 2023+ successor. Per the spec, the family has 12 published members covering TLS, HTTP, TCP, X.509, DHCP, SSH:

| Member | Full name | Layer | Purpose |
|---|---|---|---|
| **JA4** | JA4 | TLS (TCP) | TLS client ClientHello |
| **JA4S** | JA4Server | TLS (TCP) | TLS server ServerHello |
| **JA4H** | JA4HTTP | HTTP | HTTP client request headers/cookies |
| **JA4L** | JA4Latency | net timing | Client→server latency |
| **JA4LS** | JA4LatencyServer | net timing | Server→client latency |
| **JA4X** | JA4X509 | X.509 | Certificate fingerprinting |
| **JA4SSH** | JA4SSH | SSH | SSH protocol |
| **JA4T** | JA4TCP | TCP | Client TCP options |
| **JA4TS** | JA4TCPServer | TCP | Server TCP response |
| **JA4TScan** | JA4TCPScan | TCP (active) | Scanning use |
| **JA4D** | JA4DHCP | DHCP | DHCPv4 |
| **JA4D6** | JA4DHCPv6 | DHCPv6 | DHCPv6 |

The TLS one — the one anti-bot vendors call "the JA4" without qualifier — has the format:

```
[protocol][tls_ver][sni][cipher_count][ext_count][alpn]_[cipher_hash_12]_[ext_sigalg_hash_12]
```

Concrete decode for `t13d1516h2_8daaf6152771_e5627efa2ab1`:

| Field | Value | Meaning |
|---|---|---|
| `t` | TCP | (vs `q`=QUIC, `d`=DTLS) |
| `13` | TLS 1.3 | from `supported_versions` extension (0x002b), fall back to record version |
| `d` | SNI present | (vs `i`=IP literal, no SNI) |
| `15` | 15 ciphers | decimal, capped at "99", GREASE excluded |
| `16` | 16 extensions | decimal, capped at "99", GREASE excluded, includes SNI + ALPN |
| `h2` | ALPN `h2` | first + last alphanumeric chars of first ALPN protocol |
| `8daaf6152771` | cipher hash | first 12 hex of SHA-256 of sorted-ascending 4-char-hex cipher list, lowercase, comma-delimited |
| `e5627efa2ab1` | ext+sigalg hash | first 12 hex of SHA-256 of `sorted_ext_codepoints + _ + sigalg_codepoints_in_order` |

**Critical canonicalization rules** (per <https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4.md>):

1. **GREASE always stripped** — never appears in the count, never in the hash input. This is what makes per-handshake variability irrelevant to JA4: Chrome's Fisher-Yates shuffle randomizes WHERE GREASE values go, but JA4 doesn't see them.
2. **Cipher hash inputs sorted ASCENDING** by hex codepoint. This is intentional — it makes "cipher-stunting" evasion (where a malware client randomizes cipher order per handshake) collapse to the same hash. Order doesn't matter to the hash; the *set* does.
3. **Extension hash inputs sorted ASCENDING** by hex codepoint, EXCLUDING SNI (0x0000) and ALPN (0x0010). Those two are already captured in other JA4 fields (`d`/`i` for SNI presence; `h2`/`h1`/... for ALPN identity), so excluding them prevents double-counting.
4. **Signature-algorithms preserved in wire order** — NOT sorted. Concatenated to the sorted extension list with an underscore: `sorted_exts + "_" + sigalgs_in_order`. This is why Apple's `rsa_pss_rsae_sha384` duplicated-twice sigalg bug (per `crates/net/src/tls.rs:134-148`) affects JA4: the hash sees the duplicate.
5. **Hash output is lowercase** SHA-256, first 12 hex characters.
6. **Empty list → `000000000000`** rather than `SHA-256("")` to make "no ciphers" / "no extensions" visually distinct.

**JA4 license:** BSD 3-Clause (per the upstream README). JA4 itself is freely usable for any purpose. **JA4S, JA4H, JA4L, JA4LS, JA4X, JA4SSH, JA4T, JA4TS, JA4TScan are FoxIO License 1.1** (permissive for internal/academic, NOT for monetization / OEM resale) and patent-pending. This is why BO's `crates/net/src/ja4h.rs` is `#[cfg(test)]`-gated: shipping the computer in a release binary would constitute monetization-adjacent use. See `crates/net/src/LICENSE-NOTE.md` and `23 §2.5`.

### 2.3 What goes into the TLS fingerprint (per-feature catalogue)

The fields a TLS-layer detector can read off the ClientHello — and what BO does with each:

| Feature | Chrome 147 / 148 desktop value | iOS Safari 18.x value | BO implementation |
|---|---|---|---|
| **TLS record version** | 0x0301 (TLS 1.0) — the OUTER record header always says 1.0 even for TLS 1.3 handshakes | 0x0301 | Verified by `desktop_chrome_emits_tls_1_0_record_version` + `safari_ios_emits_tls_1_0_record_version` (`crates/net/src/tls.rs:562-666`) |
| **TLS handshake version (legacy_version)** | 0x0303 (TLS 1.2) — protocol-version field inside ClientHello body | 0x0303 | BoringSSL default — correct |
| **`supported_versions` extension (0x002b)** | 0x0304 (1.3) + 0x0303 (1.2) — Chrome 147 advertises 2 versions | TLS 1.0, 1.1, 1.2, 1.3 — Safari 18 advertises 4 versions (`tls.rs:285-289`) | Chrome: `set_min_proto_version(TLS1_2)`; Safari: `set_min_proto_version(TLS1)` — per `tls.rs:285-295` |
| **Cipher suite list** | 15 ciphers, specific order (TLS 1.3 first, then ECDHE GCM, then ECDHE CHACHA, then ECDHE CBC, then RSA fallbacks) | 20 ciphers including 3DES_EDE_CBC_SHA at the tail | `CIPHER_LIST` (`tls.rs:60-76`) for Chrome; `CIPHER_LIST_SAFARI_IOS` (`tls.rs:111-132`) for iOS |
| **Cipher ORDER matters** | Yes — JA3 hashes the order; JA4 sorts before hashing but the SET still gives a distinct count + hash | Yes | We pin the order via `boring2`'s `set_cipher_list(string)` API |
| **Extension list** | 16 extensions (key_share, ECH, supported_groups, certificate_timestamp, psk_kex_modes, ext_master_secret, application_settings_new, cert_compression, supported_versions, server_name, renegotiate, ec_point_formats, status_request, ALPN, session_ticket, signature_algorithms) | 13 extensions (no ECH, no application_settings_new, no session_ticket — three of the four big "Safari is different" signals; the fourth is Zlib vs Brotli cert compression) | `CHROME_EXTENSION_PERMUTATION` (`tls.rs:203-220`) — 16 entries; `SAFARI_IOS_EXTENSION_PERMUTATION` (`tls.rs:169-183`) — 13 entries |
| **Extension ORDER** | Chrome 110+ shuffles per-handshake via Fisher-Yates; psk_key_exchange_modes / pre_shared_key always last (BoringSSL enforces) | FIXED order — Safari does NOT shuffle (one of the strongest Safari tells) | Chrome: `shuffled_chrome_extension_permutation()` (`tls.rs:222-228`) runs a fresh Fisher-Yates per `chrome_connector()` call; Safari: static permutation set via `SSL_CTX_set_extension_permutation` (`tls.rs:361-367`) |
| **Elliptic curves / supported_groups** | `X25519MLKEM768` (4588) → `X25519` (29) → `SECP256R1` (23) → `SECP384R1` (24) — MLKEM leads since Chrome 131 | `X25519` → `SECP256R1` → `SECP384R1` → `SECP521R1` — no PQ; adds P-521; iOS 26 expected to add MLKEM | `CURVES_DESKTOP` (`tls.rs:91-96`); `CURVES_SAFARI_IOS` (`tls.rs:152-157`) |
| **EC point formats (0x000b)** | `uncompressed` (0) — Chrome only sends this | Same | BoringSSL default |
| **ALPN list** | `h2, http/1.1` | `h2, http/1.1` | `ALPN_PROTOS = b"\x02h2\x08http/1.1"` (`tls.rs:186`) — length-prefixed list |
| **Signature algorithms (in wire order — JA4-significant)** | 8 entries: ecdsa_secp256r1_sha256, rsa_pss_rsae_sha256, rsa_pkcs1_sha256, ecdsa_secp384r1_sha384, rsa_pss_rsae_sha384, rsa_pkcs1_sha384, rsa_pss_rsae_sha512, rsa_pkcs1_sha512 | 10 entries — adds `rsa_pss_rsae_sha384` DUPLICATED (Apple bug we reproduce verbatim) + `rsa_pkcs1_sha1` at the tail | `SIGALGS_LIST` (`tls.rs:79-88`); `SIGALGS_LIST_SAFARI_IOS` (`tls.rs:134-148`) |
| **SNI presence/absence** | Always present (host) → `d` in JA4 | Always present | `configure_connection` sets hostname unless target is an IP literal (`tls.rs:429-436`) |
| **Key share groups** | X25519MLKEM768 + X25519 (2 key shares since Chrome 131; PQ landing site + classical fallback) | X25519 + SECP256R1 (2 key shares) | `builder.set_key_shares_limit(2)` (`tls.rs:306`) |
| **GREASE** | Random reserved-codepoint values in 4 positions (cipher list, extensions, supported_versions, supported_groups); randomized per handshake | Real Safari does NOT emit GREASE (`builder.set_grease_enabled(false)`-equivalent — Safari pre-iOS 26 has no GREASE per lexiforest signatures) | `builder.set_grease_enabled(true)` (`tls.rs:298`) — Chrome path. We do NOT disable for Safari currently; this is a documented audit follow-up |
| **Certificate compression** | Brotli (algo 2) — Chrome 110+ desktop+Android | Zlib (algo 1) — Safari iOS | `add_cert_compression_alg(Brotli|Zlib)` (`tls.rs:312-319`) per-profile |
| **Session ticket extension (0x0023)** | Present (empty extension body — session resumption) | ABSENT — one of the four big Safari tells | `SslOptions::NO_TICKET` set for Safari iOS (`tls.rs:324-326`) |
| **ALPS (application_settings_new, 0x44CD)** | Present — carries inner HTTP/2 SETTINGS frame (1,2,4,6 with Chrome's values) | ABSENT — Safari has no ALPS | `SSL_add_application_settings` called for Chrome only (`tls.rs:393-425`); skipped for Safari (`tls.rs:387`) |
| **ECH (encrypted_client_hello, 0xFE0D)** | Present as GREASE (random ECH-shaped extension since Chrome 124) | ABSENT — Safari does not ECH-grease | `config.set_enable_ech_grease(true)` for Chrome only (`tls.rs:389`) |
| **Padding (0x0015)** | Auto-emitted by BoringSSL when ClientHello length crosses ~512 bytes | Auto-emitted | BoringSSL implementation default |
| **Renegotiation Info (0xff01)** | Present, empty body | Present, empty body | BoringSSL default — included in extension permutation |

The big takeaway: **BO matches all of these byte-for-byte against the verified-real Chrome 147 / Safari iOS 18.x references**, with `lexiforest/curl-impersonate`'s YAML signatures as the canonical cross-check and `tls.peet.ws/api/all` as the live ground-truth oracle.

### 2.4 Per-vendor TLS inspection matrix

The vendors known to inspect TLS at the edge (and what they look at, with confidence levels). Sources: `25_CLOUDFLARE_DEEP.md`, `26_AKAMAI_BMP_DEEP.md`, `06_AWS_WAF_SOLVER.md`, `07_DATADOME_PRIMITIVES.md`, `08_KASADA_FRONTIER.md`, `18_ANTI_BOT_VENDOR_COOKBOOK.md`, plus public anti-bot research aggregated at <https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide>.

| Vendor | Layer | JA3? | JA4? | HTTP/2? | Customization | Sources |
|---|---|---|---|---|---|---|
| **Cloudflare** | edge (CF infra) | ✓ legacy | ✓ current | ✓ | per-zone "Bot Fight Mode" / "Managed Challenge" rules; JA4-tier per `CF_MITIGATED` (`crates/net/src/h2_client.rs` + 25 §3) | per-tenant config, plus the global "bot score" model; CF published the JA4 signals model in their blog (<https://blog.cloudflare.com/ja4-signals/>) |
| **Akamai (BMP)** | edge | ✓ legacy | ✓ current | ✓ — H2 hash is Akamai's invention (Black Hat EU 2017 paper) | per-customer Akamai BMP rules; H2 hash is the canonical Akamai contribution to fingerprinting | `26_AKAMAI_BMP_DEEP.md`; <https://blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf> |
| **AWS WAF** | edge (CloudFront) | ✓ | ✓ (added 2023+) | ✓ | per-tenant Challenge / Captcha actions; AWS WAF specifically inspects JA3 + UA agreement | `06_AWS_WAF_SOLVER.md`; AWS blog "Use AWS WAF JA3 fingerprints" — search AWS docs for "JA3" |
| **DataDome** | edge | ✓ | ✓ | ? (likely — they ship a JA4-based "deep inspection" tier) | uniform across tenants; the engine is one model | `07_DATADOME_PRIMITIVES.md`; the `x-datadome` header tier disambiguates (`page.rs:1064`) |
| **F5 / Shape Security** | edge | ✓ | ✓ | ✓ | rule-based per tenant | mentioned in <https://www.peakhour.io/blog/overview-of-ja4-network-fingerprinting/> |
| **Fastly NGWAF (formerly Signal Sciences)** | edge | ✓ | ✓ | ✓ | programmable rules (their VCL gives custom JA-based predicates) | Fastly published "TLS fingerprinting in Fastly" — search Fastly blog |
| **Kasada** | edge + JS | ✓ | ✓ | ✓ | uniform; the model is at Kasada's edge, identical across tenants | `08_KASADA_FRONTIER.md`; per the K2-DIFF capture in `state_2026_05_17_unblock_execution` |
| **PerimeterX / HUMAN** | edge + JS | ✓ | ✓ | ? | tenant-specific scoring with a global model | `18 §2.6`; cookie family `_px3 / _pxhd / _pxvid` |
| **Imperva (Incapsula / Reblaze)** | edge | ✓ | ✓ | ✓ | rule-based per tenant | `18 §2.7`; `x-iinfo` header |
| **Sucuri** | edge | ✓ | partial | partial | rule-based | `18 §2.8`; `x-sucuri-id` header |
| **wbaas (Walmart in-house)** | edge | ✓ | ? | ✓ | single-tenant (Walmart only) — paired with `bm_sz` | `18 §2.12` |
| **Botpoint / GoCache / Variti / Edgio / etc.** | edge | ✓ | ? | ? | smaller players; partial coverage | not in BO's 126-corpus per `27` §1 |

Of these, the **JA4 checkers** are functionally the entire field as of 2026 — every major edge product now has JA4-or-equivalent ingestion. Per <https://blog.cloudflare.com/ja4-signals/>: "JA4 fingerprints and inter-request signals" is now a first-class CF Bot Management feature.

### 2.5 BO TLS coverage — what we actually emit today

Per `23 §1.4` and `crates/net/src/tls.rs:48-57`:

- **TLS stack**: `boring2 4.15.15` (Cloudflare BoringSSL fork, per `Cargo.lock:280-293` and `crates/net/Cargo.toml:28-30`)
- **Codename**: `chrome_147` (the verified-real reference whose wire bytes we reproduce byte-exact); UA advertises `chrome_148` (deliberate, A/B-tested split per `tls.rs:22-57` — Chrome's TLS stack didn't rev 147→148, so the bytes are identical and JA4 cannot encode the version difference)
- **Per-`device_class` branching** at `tls.rs:233-369`:
  - `Desktop` / `MobileAndroid` — shared Chrome 147 desktop path with the desktop curves (`CURVES_DESKTOP` includes MLKEM768)
  - `MobileIOS` — distinct Safari 18 path: 20-cipher list, 10-sigalg list with Apple's `rsa_pss_rsae_sha384` duplicate bug, no PQ curves but adds P-521, fixed extension order (NO Fisher-Yates), Zlib cert compression, NO_TICKET, skip ECH grease, skip ALPS
- **Per-handshake variability**:
  - Chrome: Fisher-Yates extension shuffle on EVERY connect (`shuffled_chrome_extension_permutation()` at `tls.rs:222-228`); BoringSSL auto-GREASE injection (`set_grease_enabled(true)`); ECH grease (`set_enable_ech_grease(true)`)
  - Safari: NO shuffle (per real-Safari behavior); GREASE handled by BoringSSL default (we don't explicitly disable — see §10 audit follow-up)
- **In-tree silent-drift gate**: `tls_fingerprint_vectors_no_silent_drift` at `tls.rs:476-553` — pins cipher list string, sigalg list string, curve array, extension count, and the `UA_CHROME_MAJOR=148 / TLS_CHROME_MAJOR=147` coherence. Any edit to the constants fails this test loudly. Runs in CI on every PR.
- **In-tree record-version tests**: `desktop_chrome_emits_tls_1_0_record_version` + `safari_ios_emits_tls_1_0_record_version` at `tls.rs:562-666` — verify the outer TLS record header version byte is 0x0301 (TLS 1.0) for both profiles, matching real Chrome and real Safari.
- **In-tree Fisher-Yates test**: `test_shuffle_is_full_fisher_yates` at `tls.rs:668-686` — verifies the shuffle preserves the 16-extension set and is non-deterministic.

### 2.6 Known gaps from chapter 23

Per `23 §9` (v0.1.0 acceptance) and `23 §4.4`:

1. **JA4 ground-truth capture not in tree.** We have no `crates/net/tests/captures/<profile>/ja4.txt` baseline file. The network-gated `test_tls_fingerprint_peet` at `crates/net/tests/tls_fingerprint.rs:6-22` asserts the JA4 STARTS WITH `t13d` (matching Chrome desktop TLS 1.3 SNI-present) — but does NOT assert the full string. We should be checking the full 38-character JA4 against a captured-from-real-Chrome reference. This is a v0.1.0 acceptance line item.
2. **`akamai_fingerprint` baseline not in tree** for any profile. Same gap. The current test at `h2_handshake_writes_chrome_146_settings_and_window_update` (`crates/net/tests/h2_frame_bytes.rs:39-205`) decodes the SETTINGS + WINDOW_UPDATE bytes, but doesn't compute the akamai_fingerprint string and compare it to a published reference.
3. **`peetprint` not captured.** `tls.peet.ws/api/all` returns a `peetprint` field (a more detailed ClientHello dump than JA4 alone); we don't have a baseline.
4. **Raw ClientHello hex not captured** (`openssl s_server -trace` output stored per profile). Useful for diffing post-`boring2`-upgrade.
5. **Firefox 135 TLS class**: currently same Chrome 147 path (per `presets.rs:457-463`); real Gecko/NSS TLS is Phase B.3 future work. Not v0.1.0 scope.
6. **Safari-on-macOS** desktop branch: not yet present. Per `19_PROFILE_EXPANSION_PLAN.md` §2.1 Candidate A. v0.1.0 stretch goal.

### 2.7 ECH (Encrypted ClientHello) — forward-looking

**Spec:** RFC 9180 (HPKE for the underlying construct); the ECH-specific draft is `draft-ietf-tls-esni` (now finalized as ECH in TLS 1.3 extension 0xFE0D).

**Status as of 2026:**
- **Chrome:** ECH support landed in Chrome 117 (Sept 2023); enabled by default. Chrome 148 (current) fully supports it. <https://chromestatus.com/feature/6196703843581952>
- **Firefox:** ECH default-on since Firefox 119. <https://blog.cloudflare.com/announcing-encrypted-client-hello/>
- **Safari:** ECH support in Safari 18+ on macOS / iOS 17.4+
- **Cloudflare server-side:** ECH enabled by default on all Free zones; available on Pro/Business/Enterprise. <https://developers.cloudflare.com/ssl/edge-certificates/ech/>
- **Other CDNs (Fastly, AWS CloudFront, Akamai):** not yet uniformly deployed as of May 2026

**What ECH changes:**

The TLS ClientHello consists of an OUTER hello (with a placeholder SNI like `cloudflare-ech.com`) and an INNER hello (encrypted under the server's ECH public key, derived from a DNS `HTTPS` record). The outer hello is what every middlebox + edge fingerprinter sees; the inner hello carries the real SNI + the user's actual ClientHello.

**Implications for fingerprinting:**

1. **JA3/JA4 by SNI becomes impossible at the edge for ECH-enabled traffic.** The vendor (e.g., Cloudflare) sees the outer SNI = `cloudflare-ech.com` for every connection. They can still JA4 the outer hello (which IS our Chrome 147 ClientHello, including ECH extension presence), but they cannot route the JA4 to the right per-zone rule by SNI before decrypting.
2. **JA4 of the OUTER ClientHello is still meaningful** — Cloudflare can still tell "this is Chrome 148" from the outer hello's cipher list + extension list + supported_versions. ECH does not encrypt the JA4-relevant fields of the outer hello.
3. **Inside CF infra (after ECH decryption)**: CF sees the inner hello, which is the real ClientHello to the real server. CF can re-JA4 this inner hello and apply per-zone rules. So for CF specifically, ECH changes the OUTER fingerprint pool (all CF traffic = same outer SNI), but doesn't change the per-tenant ability to fingerprint.
4. **For non-CF intermediate middleboxes** (corporate firewalls, ISPs, censorship infrastructure): they see only the outer hello. The CDT analysis (<https://cdt.org/insights/closing-the-sni-metadata-gap/>) frames this as a privacy win.
5. **New fingerprints will emerge.** Quote from <https://cdt.org/insights/do-not-stick-out-the-dynamics-of-the-ech-rollout/>: "Within the pool of ECH-shaped traffic, the concentration of connections using the same outer SNI (`cloudflare-ech.com`) created a targetable pattern" — Russia's Roskomnadzor moved to block ECH-shaped TLS as a category weeks after CF flipped it on. Expect TCP transport parameters (JA4T) and timing fingerprints (JA4L) to fill the void.

**BO status:**

- We emit ECH GREASE (random ECH-shaped extension) on Chrome desktop/Android profiles per `tls.rs:298` + `tls.rs:389` — `builder.set_grease_enabled(true)` + `config.set_enable_ech_grease(true)`. Real Chrome ships ECH grease since Chrome 124.
- Real ECH (sending an actual encrypted inner hello with a real ECH config from a DNS HTTPS record) is NOT implemented in BO. When CF / other origins flip ECH on at scale (already true for CF Free), we will look like "Chrome that has ECH disabled" — itself a small signal.
- The follow-up work to implement real ECH is `boring2` exposing the relevant APIs (it has `SSL_set1_ech_config_list` in recent versions) and us plumbing a DNS HTTPS-record fetcher on top of `quinn`/`hickory-dns`. NOT v0.1.0 scope.
- For 2026 cited deployment: per <https://blog.cloudflare.com/encrypted-client-hello/> ECH is available on all CF plans; <1% of real origins (outside CF) accept ECH. Grease alone is the right posture for now.

### 2.8 The "TLS post-quantum" subplot — MLKEM768

Chrome 131 (Oct 2024) rolled X25519MLKEM768 (codepoint 4588) as the lead curve in the supported_groups extension. This is a post-quantum hybrid: X25519 (classical ECDH) combined with ML-KEM-768 (NIST PQ standard, formerly Kyber768). The hybrid is meant to resist both classical and quantum attacks — even if one half is broken, the other holds.

**Fingerprinting implications:**

- **A Chrome 131+ ClientHello has TWO key shares**: X25519MLKEM768 first, then X25519. This is a STRONG positive signal — pre-131 Chrome had ONE key share (X25519); non-Chrome clients almost never advertise X25519MLKEM768.
- BO matches this exactly: `CURVES_DESKTOP` at `tls.rs:91-96` has `X25519_MLKEM768` first; `set_key_shares_limit(2)` at `tls.rs:306` instructs BoringSSL to emit both key shares on the wire.
- iOS Safari 18.x does NOT yet emit MLKEM (per `tls.rs:150-151`: "No PQ. MLKEM lands in iOS 26 per Apple's PQC support page"). Our `CURVES_SAFARI_IOS` correctly omits it.
- iOS 26 (expected late 2026) is forecast to add MLKEM. When that ships, our Safari constants need a refresh per `23 §5.2`.

The MLKEM example illustrates the maintenance burden of byte-perfect TLS impersonation: a single Chrome browser-major (131) can roll in a wire-byte-level change that invalidates every static reference, requiring a coordinated update of `CIPHER_LIST` / `CURVES_DESKTOP` / `CHROME_EXTENSION_PERMUTATION` and a fresh capture. Per `23 §5`, this is exactly the playbook we maintain — `cargo test tls_fingerprint_vectors_no_silent_drift` is the gate that catches drift.

### 2.9 Worked example — the cipher hash computation for Chrome 147

To make the JA4 canonicalization concrete, here is the cipher-hash computation for our `CIPHER_LIST` as it would appear in a JA4 capture.

**Step 1 — convert the cipher list to IANA codepoints (hex):**

| Cipher constant | IANA name | Hex |
|---|---|---|
| `TLS_AES_128_GCM_SHA256` | 0x1301 | `1301` |
| `TLS_AES_256_GCM_SHA384` | 0x1302 | `1302` |
| `TLS_CHACHA20_POLY1305_SHA256` | 0x1303 | `1303` |
| `TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256` | 0xC02B | `c02b` |
| `TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256` | 0xC02F | `c02f` |
| `TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384` | 0xC02C | `c02c` |
| `TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384` | 0xC030 | `c030` |
| `TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256` | 0xCCA9 | `cca9` |
| `TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256` | 0xCCA8 | `cca8` |
| `TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA` | 0xC013 | `c013` |
| `TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA` | 0xC014 | `c014` |
| `TLS_RSA_WITH_AES_128_GCM_SHA256` | 0x009C | `009c` |
| `TLS_RSA_WITH_AES_256_GCM_SHA384` | 0x009D | `009d` |
| `TLS_RSA_WITH_AES_128_CBC_SHA` | 0x002F | `002f` |
| `TLS_RSA_WITH_AES_256_CBC_SHA` | 0x0035 | `0035` |

**Step 2 — sort ASCENDING by hex value:**

```
002f, 0035, 009c, 009d, 1301, 1302, 1303, c013, c014, c02b, c02c, c02f, c030, cca8, cca9
```

**Step 3 — join with commas (no spaces), all lowercase:**

```
002f,0035,009c,009d,1301,1302,1303,c013,c014,c02b,c02c,c02f,c030,cca8,cca9
```

**Step 4 — SHA-256, take first 12 hex chars** (this is what `tls.peet.ws` reports as the cipher_hash segment of the JA4).

The expected JA4 for any of BO's desktop Chrome profiles is `t13d1516h2_<first-12-of-sha256(above_string)>_<first-12-of-sha256(sorted_exts + "_" + sigalgs)>`. The first 13 characters (`t13d1516h2`) are stable across every Chrome 147+ handshake; the two 12-hex hashes are stable as long as the cipher / sigalg / extension SET doesn't change. The per-handshake Fisher-Yates shuffle of extension ORDER does NOT affect JA4 because the canonicalization sorts before hashing — exactly the property §2.2 explains.

**Why this matters for §6.3:** the v0.1.0 acceptance-bar JA4 capture is just "run `curl -s https://tls.peet.ws/api/all` over a fresh BoringSSL handshake and store the returned `.tls.ja4` string." The test extension is six lines of Rust. The reason this hasn't been done yet is that the existing test asserts the `t13d` prefix and got us 90% of the way; the full-string assert is the last 10%.

### 2.10 Why JA4 over JA3 — the structural argument

JA4's structural improvements vs JA3 (which we summarize for the doc-set's benefit; cross-link `23 §2.1` for the BO-specific version):

1. **MD5 → SHA-256.** MD5 has known collisions; SHA-256 doesn't. Vendors moving from JA3-signature databases to JA4 get higher confidence in matches.
2. **Plain-text metadata up front** (`t13d1516h2`). A defender can write a rule like "block any client with `t13d12*h2` or `t12*`" without needing to enumerate hashes. Pre-JA4, the same rule required maintaining a hash-of-hashes lookup.
3. **Per-protocol prefix** (`q` for QUIC, `t` for TCP, `d` for DTLS). One fingerprint family covers TLS-over-TCP, QUIC, and DTLS without ambiguity.
4. **Sorting eliminates evasion** (cipher-stunting, extension-stunting). Pre-JA4, a malware author could randomize cipher order per handshake to dodge JA3 hash matching; JA4 sorts first.
5. **Companion JA4S/JA4H/JA4T/JA4X** cover other layers in the same family — a defender adopts JA4 once and gets a coherent fingerprint suite across HTTP, TCP, X.509.
6. **The format is sortable + greppable.** `t13d` matches all TLS-1.3-TCP-SNI handshakes; `t12i` matches TLS-1.2-TCP-IP-literal. Easy to bucket.

The strategic implication for BO: getting JA4 right means our handshakes look identical to real Chrome at the structural level — and the metadata (`t13d1516h2`) is the part vendors most often write rules against. The 12-hex hash component is the deep-uniqueness layer; the metadata is the everyday-rules layer.

---

## 3. HTTP/2 fingerprinting deep dive

HTTP/2 framing (RFC 9113) is just as fingerprintable as TLS — and in many ways MORE so, because the HTTP/2 stack of a Python `httpx` / Go `net/http` / Rust `hyper` library carries decade-deep defaults that don't match any browser.

**Spec sources:**
- RFC 9113 — HTTP/2: <https://datatracker.ietf.org/doc/html/rfc9113>
- RFC 9218 — Extensible Prioritization Scheme (replaces RFC 7540 §5.3 priority): <https://datatracker.ietf.org/doc/html/rfc9218>
- Akamai's seminal H2 fingerprint paper: <https://blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf>

### 3.1 The SETTINGS frame

Per RFC 9113 §6.5: the client SETTINGS frame is one of the first frames in every HTTP/2 connection (after the PREFACE). Six IANA-registered identifiers exist:

| ID | Setting | RFC 9113 initial value | Chrome 147/148 | Safari iOS 18.4 | Firefox 135 |
|---|---|---|---|---|---|
| 1 | `HEADER_TABLE_SIZE` | 4096 octets | **65536** | not sent (default) | 65536 |
| 2 | `ENABLE_PUSH` | 1 | **0** | **0** | 0 |
| 3 | `MAX_CONCURRENT_STREAMS` | (unlimited) | not sent | **100** | not sent |
| 4 | `INITIAL_WINDOW_SIZE` | 65,535 octets | **6,291,456** (6 MB) | **2,097,152** (2 MB) | 131,072 (128 KB) |
| 5 | `MAX_FRAME_SIZE` | 16,384 octets | not sent (default) | not sent | 16,384 |
| 6 | `MAX_HEADER_LIST_SIZE` | (unlimited) | **262,144** (256 KB) | not sent on wire (but BO sets it on the builder for h-m.com Akamai workaround per `h2_client.rs:139-152`) | not sent |
| 8 | `ENABLE_CONNECT_PROTOCOL` | 0 | not sent | not sent in 18.4 (was 1 in iOS 18.0 — dropped per `h2_client.rs:56-61`) | not sent |
| 9 | `NO_RFC7540_PRIORITIES` | 0 | not sent | **1** (Safari opts out of HTTP/2 priority frames) | not sent |

The fingerprint is: **which IDs are sent + what values + the ORDER on the wire**.

**Chrome 147/148 SETTINGS frame on the wire** (per `crates/net/src/h2_client.rs:39-50` constants, verified against `tls.peet.ws/api/all` capture 2026-04-29):

```
(1, 65536) HEADER_TABLE_SIZE
(2, 0)     ENABLE_PUSH
(4, 6291456) INITIAL_WINDOW_SIZE
(6, 262144) MAX_HEADER_LIST_SIZE
```

Four entries, in that order. Chrome does NOT send `(3, MAX_CONCURRENT_STREAMS)` or `(5, MAX_FRAME_SIZE)`. Earlier in BO's development (per the comment at `h2_client.rs:34-38`) we incorrectly included both of those — making our Akamai H2 hash `d23e6399a1d185e3b8cb58e5640dd698` instead of Chrome's `52d84b11737d980aef856699f885ca86`. The 2026-04-29 reference capture corrected us.

**Safari iOS 18.4 SETTINGS frame** (per `h2_client.rs:53-68`):

```
(2, 0)       ENABLE_PUSH
(3, 100)     MAX_CONCURRENT_STREAMS
(4, 2097152) INITIAL_WINDOW_SIZE
(9, 1)       NO_RFC7540_PRIORITIES
```

Four entries; completely different ID set and order. The presence of `(9, NO_RFC7540_PRIORITIES = 1)` is a sharp Safari tell — it tells the server "don't expect PRIORITY frames from me, I implement RFC 9218 priorities (or none at all)."

**SETTINGS ORDER MATTERS** for the Akamai H2 hash. Akamai's format string (per Black Hat EU 2017): `<SETTINGS_id:value;id:value;…>|<WINDOW_UPDATE>|<PRIORITY>|<pseudo-header-order>`. The first segment is the SETTINGS frame in WIRE ORDER, not sorted. So `1:65536;2:0;4:6291456;6:262144` is structurally distinct from `2:0;1:65536;4:6291456;6:262144`.

### 3.2 The WINDOW_UPDATE frame

After the PREFACE + SETTINGS, the client sends a WINDOW_UPDATE on stream 0 (the connection level) to advertise its read-buffer capacity.

- The protocol default is 65,535. WINDOW_UPDATE adds delta to bring it up.
- Chrome 147/148: target = 15,728,640 (15 MB), wire delta = 15,728,640 − 65,535 = **15,663,105**
- Safari iOS 18.4: target = 10,485,760 (10 MB), wire delta = 10,485,760 − 65,535 = **10,420,225**

The wire delta is what Akamai's H2 hash sees (the middle `|15663105|` segment).

BO emits this exactly per `crates/net/src/h2_client.rs:43-50` (Chrome) and `:65-68` (Safari) — the configured `initial_connection_window_size` of `15_728_640` / `10_485_760` becomes the wire delta via the http2 lib's internal subtract-default logic. Verified against wreq-util's gold-standard reference (<https://github.com/0x676e67/wreq-util>) per the comment at `h2_client.rs:46-49`.

The byte-equivalence test at `crates/net/tests/h2_frame_bytes.rs:39-205` is the silent-drift gate — runs in CI on every PR.

### 3.3 PRIORITY frame vs Priority Update header

Per RFC 9113 §5.3.4: "This update to HTTP/2 deprecates the priority signaling defined in RFC 7540." The new scheme is RFC 9218.

Three states are observable:

1. **Old PRIORITY frame (RFC 7540 §5.3)** — Chrome 147 emits a priority HINT on its first HEADERS frame (`weight=255` wire byte = API weight 256, `depends_on=0`, `exclusive=true`) per `h2_client.rs:167-174`. This is the HEADERS-frame `stream_dependency` payload, NOT a separate PRIORITY frame. Chrome stopped sending separate PRIORITY frames as of ~Chrome 109; the priority is embedded in the HEADERS frame opening byte. The Akamai H2 hash's third segment `|0|` means "no separate PRIORITY frame" — what Chrome 147 produces.
2. **No priority at all** — Safari iOS 18.4 sends SETTINGS `(9, NO_RFC7540_PRIORITIES = 1)`, meaning "I do not produce or honor PRIORITY frames." BO's `h2_client.rs:154-157` honors this: skips the `headers_stream_dependency()` call entirely for the iOS branch.
3. **RFC 9218 header-based priority** — a request header `priority: u=0, i` (Chrome 110+, also iOS 17+). This is a header sent in the HEADERS frame body (HPACK-encoded), not a separate frame. BO's `crates/net/src/headers.rs:225+` (`chrome_headers_impl`) emits `priority: u=0, i` on Chrome nav requests; Safari's flavor (`safari_headers` at `headers.rs:591+`) emits its own priority header per Safari's RFC 9218 conventions.

The presence of `priority` request header + the H2-level `weight=255 dep=0 exclusive=true` HEADERS-frame hint is itself a Chrome 110+ signature. Pre-110 Chrome did the H2 hint but not the header; non-Chrome clients usually emit one or the other but not both.

### 3.4 Pseudo-header order

Per RFC 9113 §8.3.1: HTTP/2 requests use four pseudo-headers — `:method`, `:scheme`, `:authority`, `:path`. The RFC specifies the ORDER as "the order these appear in"; the spec text says "Pseudo-header fields MUST appear in the header block before regular header fields" but does NOT mandate an order among the four pseudo-headers. **The order is therefore a fingerprint signal.**

Browser-by-browser pseudo-header order (per the research at <https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide>):

| Browser | Pseudo-header order | Akamai H2 hash segment |
|---|---|---|
| **Chrome 147/148** (desktop + Android) | `:method, :authority, :scheme, :path` | `m,a,s,p` |
| **Safari iOS 18.x** | `:method, :scheme, :authority, :path` | `m,s,a,p` |
| **Safari macOS** | same as iOS — `m,s,a,p` | (assumed; v0.1.0 needs capture) |
| **Firefox 135** | `:method, :path, :authority, :scheme` | `m,p,a,s` |
| **Edge / Brave (Chromium-based)** | `m,a,s,p` (same as Chrome) | |
| **curl, Go net/http, Python httpx** | typically alphabetical (`:authority, :method, :path, :scheme`) — `a,m,p,s` | a dead giveaway |

BO implementation (`crates/net/src/h2_client.rs:85-104`): branches on `is_safari_ios`, emitting `PseudoOrder::builder().push(Method).push(Authority).push(Scheme).push(Path)` for Chrome/Android (`masp`) and `Method/Scheme/Authority/Path` for iOS Safari (`msap`). Firefox path is NOT yet differentiated — it currently emits Chrome order (`masp`). This is a known gap per the firefox-class TLS-vs-Chrome split documented in `23 §1.4`.

### 3.5 Per-vendor HTTP/2 inspection matrix

| Vendor | Layer | Checks SETTINGS? | Checks WINDOW_UPDATE? | Checks PRIORITY shape? | Checks pseudo-header order? | Notes |
|---|---|---|---|---|---|---|
| **Cloudflare** | edge | ✓ | ✓ | ✓ | ✓ | Uses akamai_fingerprint-style hash internally; per `25 §3` CF specifically checks SETTINGS ENABLE_PUSH and INITIAL_WINDOW_SIZE values |
| **Akamai (BMP)** | edge | ✓ — INVENTED the H2 hash | ✓ | ✓ | ✓ | Per the 2017 Black Hat EU paper; H2 hash IS Akamai's fingerprint. `26 §3` documents the bm_sz tier checks. |
| **AWS WAF** | edge | ✓ | ? | ? | ✓ — AWS WAF specifically flags non-browser pseudo-header orders | `06 §3` |
| **DataDome** | edge | ✓ | ? | ? | ✓ | `07 §2`; specifically catches `m,p,a,s` (Firefox) vs `m,a,s,p` (Chrome) mismatches against UA |
| **Kasada** | edge + JS | ✓ | ✓ | ✓ | ✓ | `08`; Kasada explicitly emits a SETTINGS hash field in its `/ips.js` capture (per K2-DIFF) |
| **F5 / Shape** | edge | ✓ | ✓ | ✓ | ✓ | reported in <https://blog.nginx.org/blog/encrypted-client-hello-comes-to-nginx> |
| **PerimeterX / HUMAN** | edge + JS | ✓ | ? | ? | ✓ | `18 §2.6` |
| **Imperva** | edge | ✓ | ✓ | ✓ | ✓ | `18 §2.7` |
| **Fastly NGWAF** | edge | ✓ | ✓ | ✓ | ✓ | programmable rules |
| **Sucuri** | edge | ? | ? | ? | partial | `18 §2.8`; smaller player |

Per <https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide>: "the four fingerprint components are combined into a single string using a format popularized by Akamai's research" — meaning the entire industry has converged on the same H2-fingerprint shape, with per-vendor tweaks on which segments they weight.

### 3.6 BO HTTP/2 coverage

| Item | Status | Source |
|---|---|---|
| `http2` crate version | `0.5.17` (wreq's fork of h2 with PseudoOrder + SettingsOrder support) | `Cargo.lock` resolved |
| Chrome SETTINGS frame matches Chrome 147 byte-for-byte | ✓ verified | `tests/h2_frame_bytes.rs:39-205` (`h2_handshake_writes_chrome_146_settings_and_window_update`) — NOT `#[ignore]`, runs in CI |
| WINDOW_UPDATE delta = 15,663,105 (Chrome) / 10,420,225 (Safari) | ✓ verified | same byte-equivalence test |
| Chrome pseudo-header order `m,a,s,p` | ✓ | `h2_client.rs:98-104` |
| Safari pseudo-header order `m,s,a,p` | ✓ | `h2_client.rs:90-96` |
| Chrome HEADERS-frame priority `weight=255, dep=0, exclusive=true` | ✓ | `h2_client.rs:167-174` |
| Safari skips HEADERS-frame priority (per NO_RFC7540_PRIORITIES) | ✓ | `h2_client.rs:154-157` |
| Chrome SETTINGS wire order `1, 2, 4, 6` (with declared 8-entry SettingsOrder `1,2,3,4,5,6,8,9`) | ✓ | `h2_client.rs:118-130` |
| Safari SETTINGS wire order `2, 3, 4, 9` | ✓ | `h2_client.rs:110-117` |
| Firefox 135 pseudo-header order `m,p,a,s` | ❌ NOT IMPLEMENTED — currently emits Chrome's `m,a,s,p` | `h2_client.rs:90+` branch only differentiates iOS Safari |
| Firefox 135 SETTINGS values (HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=131072, MAX_FRAME_SIZE=16384) | ❌ NOT IMPLEMENTED — Firefox profile emits Chrome SETTINGS today | per `presets.rs:457-463` known gap |
| akamai_fingerprint baseline captured per profile | ❌ NOT CAPTURED — v0.1.0 acceptance gap per `23 §9` | (gap) |

**Conclusion:** Chrome desktop / Chrome Android / iOS Safari are byte-perfect against the 2026-04-29 reference capture. Firefox is a known fall-through that emits Chrome H2 — fine for "look like Chrome via Firefox UA" experimental routing (one of the routed-mode profiles per `27 §2.1`'s adidas Firefox win), bad for "actually be Firefox." This is the next-priority HTTP/2 work after JA4 capture.

### 3.7 ALPS — HTTP/2 settings advertised inside the TLS handshake

ALPS (Application-Layer Protocol Settings) is a Chrome-specific extension (`application_settings_new`, codepoint 0x44CD) that carries an HTTP/2 SETTINGS payload INSIDE the TLS handshake — so the server can pre-allocate buffers/state before the H2 connection starts. ALPS is Chrome-only; Safari doesn't ship it, Firefox doesn't either.

BO emits ALPS for Chrome via `SSL_add_application_settings` at `crates/net/src/tls.rs:393-425`:

```
SETTINGS frame inside ALPS payload:
  (1, 65536) HEADER_TABLE_SIZE
  (2, 0)     ENABLE_PUSH
  (4, 6291456) INITIAL_WINDOW_SIZE
  (6, 262144) MAX_HEADER_LIST_SIZE
+ empty ACCEPT_CH frame (Length 0, Type 0x89, Flags 0, Stream 0)
```

This is the SAME 4 SETTINGS as the over-the-wire H2 SETTINGS frame — Chrome consistency. Safari iOS branch skips ALPS entirely per `tls.rs:387-389` (correct — Safari has no ALPS extension).

**Detection implication:** any TLS-layer inspector that sees the ALPS extension in the ClientHello AND parses it must see the inner SETTINGS frame agree with the outer SETTINGS frame the client will send post-TLS. BO matches; this is built into our constants.

### 3.8 Worked example — the akamai_fingerprint for Chrome 147

Putting the pieces together. The akamai_fingerprint (Akamai's H2 hash format) for Chrome 147 looks like:

```
1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p
```

Decoded segment-by-segment:

| Segment | Content | Source |
|---|---|---|
| `1:65536;2:0;4:6291456;6:262144` | SETTINGS frame, **wire order**, `id:value` pairs | `crates/net/src/h2_client.rs:39-42` |
| `15663105` | WINDOW_UPDATE wire delta (configured 15728640 − default 65535) | `h2_client.rs:50` |
| `0` | No separate PRIORITY frame (Chrome 147 puts priority in HEADERS frame, not as standalone) | (Chrome 109+ behavior) |
| `m,a,s,p` | Pseudo-header order: `:method, :authority, :scheme, :path` | `h2_client.rs:98-103` |

The full per-profile expected values for the v0.1.0 acceptance capture:

| Profile | Expected akamai_fingerprint |
|---|---|
| `chrome_148_macos` / windows / linux / ru | `1:65536;2:0;4:6291456;6:262144\|15663105\|0\|m,a,s,p` |
| `pixel_9_pro_chrome_148` | same as Chrome desktop (Android shares the H2 stack) |
| `iphone_15_pro_safari_18` | `2:0;3:100;4:2097152;9:1\|10420225\|0\|m,s,a,p` |
| `firefox_135_*` | currently emits Chrome's string (the known gap per §3.6 / §6.5); when Firefox H2 differentiation lands, expected `1:65536;2:0;4:131072;5:16384\|12517377\|0\|m,p,a,s` (placeholder — verify against `tls.peet.ws` capture from real Firefox 135) |

The per-profile capture step is mechanical: hit `tls.peet.ws/api/all`, read `http2.akamai_fingerprint`, store it. The test extension is just `assert_eq!(actual, EXPECTED_AKAMAI_FP_PER_PROFILE[profile_name])`.

### 3.9 The HEADERS-frame priority hint (Chrome's RFC 7540 backwards-compat)

Even though Chrome sends SETTINGS without `NO_RFC7540_PRIORITIES` (i.e., does NOT opt out of legacy priority), Chrome 110+ does NOT send standalone PRIORITY frames — it instead embeds a priority hint as the first 5 bytes of the HEADERS frame body via the `PRIORITY` flag bit.

The hint for Chrome 147 (verified against `tls.peet.ws/api/all`):
- `stream_dependency` = 0 (depends on stream 0, i.e., the connection)
- `weight` = 255 (wire-byte; equivalent to API weight 256 because the wire is biased −1)
- `exclusive` = true (the E bit is set in the first 4 bytes of the dependency field)

BO emits this via `crates/net/src/h2_client.rs:167-174`:

```rust
.headers_stream_dependency(StreamDependency::new(
    StreamId::zero(),
    255,
    true,
))
```

Safari iOS branch skips this entirely (per `h2_client.rs:154-157` comment: "Safari has NO_RFC7540_PRIORITIES so it omits priority"). The PRIORITY flag bit on HEADERS is not set; the stream_dependency field is not present.

This is the source of the `0` (no PRIORITY frame) segment in the akamai_fingerprint — even with the HEADERS-frame embedded priority hint, the canonical Akamai format only counts STANDALONE PRIORITY frames in that segment. Chrome's HEADERS-frame embedded hint is captured separately (some Akamai tools dump it as a 5th segment; the canonical 4-segment form omits it).

### 3.10 Per-vendor case studies — what each vendor actually does with the H2 hash

**Cloudflare**: Per `25 §3.2` and the public blog at <https://blog.cloudflare.com/ja4-signals/>, CF computes its own variant of the H2 hash and feeds it into the Bot Management score model alongside the JA4. The H2 hash itself is rarely a blocking signal (too easy to spoof for sophisticated attackers); the cross-layer coherence is. CF specifically flags:
  - `INITIAL_WINDOW_SIZE` < 4 MB on a UA-says-Chrome client (low-memory bots, Python `httpx` defaults to 64 KB)
  - Pseudo-header order `a,m,p,s` (alphabetical — Python `httpx`, Go `net/http`) on a UA-says-Chrome client
  - Missing `SETTINGS 2 ENABLE_PUSH = 0` on a UA-says-Chrome client (every browser explicitly disables push)
  - Per-zone, CF rules can additionally require an akamai_fingerprint match before serving cached resources

**Akamai BMP**: per `26 §3.1`, Akamai's `bm_sz` cookie-derived score INCORPORATES the H2 hash as a feature. Sites using BMP Advanced see the H2 hash on every request; mismatch to UA triggers the `sec-cpt` challenge path. Akamai's hash is sent to the per-tenant bot-detection ML model — there's no single "this hash blocks" rule, it's a feature in a scored decision.

**AWS WAF**: per `06 §3`, AWS WAF Challenge action's `_X-Amzn-Waf-Action: challenge` is triggered by score thresholds that include JA3+UA agreement and HTTP/2 pseudo-header order. The smoking-gun signature for non-browser AWS WAF blocks is `a,m,p,s` pseudo-header order (alphabetical, what Python defaults emit) + a generic curl-class TLS JA3.

**DataDome**: per `07 §2.4`, the `rt:'i'` (silent) vs `rt:'c'` (interactive captcha) decision tree includes the H2 hash as one feature among ~50. The way DataDome tells iOS-Safari-from-actual-iOS-Safari from iOS-Safari-from-CDP-driven-Playwright is partly the H2 hash (Playwright's Chromium uses Chrome H2 settings even when configured to emit Safari UA — a cross-layer mismatch).

**Kasada**: per `08 §3 / state_2026_05_17_unblock_execution.md`, the Kasada `/ips.js` capture explicitly emits an H2 hash field as one of the sensor's inputs. The K2-DIFF research established that Kasada's score depends on the H2 hash matching the UA-class; getting Chrome H2 + Chrome UA is necessary but not sufficient (Kasada also checks ~100 JS-runtime features).

**Imperva / Reblaze**: per `18 §2.7`, Imperva's per-tenant rule engine can be configured with a "JA3-and-H2-hash must agree with UA-class" gate. We've observed this on `x-iinfo`-marker sites; the gate fires identically regardless of which Imperva tenant.

---

## 4. WebRTC fingerprinting

WebRTC is the third axis — but the SMALLEST in cross-vendor leverage, because most edge anti-bot products don't run WebRTC checks (it requires JS to probe, vs TLS/HTTP2 which are pure wire bytes).

### 4.1 STUN-based IP leak

WebRTC's ICE candidate gathering produces a list of network endpoints:

- **host candidates** — your machine's local network interfaces (e.g., `192.168.1.42`)
- **server-reflexive (srflx) candidates** — your public IP as seen by a STUN server (e.g., `203.0.113.5` from `stun:stun.l.google.com:19302`)
- **relay candidates** — TURN-server-relayed (e.g., a Twilio TURN endpoint)

A site running `new RTCPeerConnection().createOffer()` + `setLocalDescription()` triggers ICE gathering; the `onicecandidate` callback fires with each candidate as a string `candidate:<foundation> <component> <protocol> <priority> <address> <port> typ host` (or `typ srflx`, etc.).

**The fingerprinting concern:** sites used to read the `host` candidate's `<address>` field and get the user's real LAN IP (`192.168.x.x`) — a strong tracking signal because LAN IPs don't change as often as cookies, and they vary per device on the same NAT, giving the site sub-NAT visibility.

Some bot detectors additionally check if the **srflx** candidate's geolocation matches the connection's public-IP geolocation as seen from the server — a mismatch suggests a residential proxy or VPN.

### 4.2 mDNS hostname anonymization (Chrome 2019+)

Chrome 84 (July 2020) made the change documented at <https://bloggeek.me/webrtcglossary/mdns/>: instead of exposing `192.168.1.42` in the host candidate, Chrome generates a UUIDv4 `.local` hostname (e.g., `a1b2c3d4-e5f6-4789-abcd-ef0123456789.local`) and uses that. The real LAN IP never leaves the browser. The other peer in a real WebRTC call can resolve the `.local` hostname via mDNS multicast on the local network to talk back.

The IETF spec is `draft-ietf-mmusic-mdns-ice-candidates` (now RFC 9577): <https://www.ietf.org/archive/id/draft-ietf-mmusic-mdns-ice-candidates-02.html>.

**Real Chrome 148 produces (per WebRTC research and BO's reference patch at `window_bootstrap.js:4983-4996`):**

1. `setLocalDescription()` returns immediately (the offer is built locally)
2. `onicecandidate` fires asynchronously with at least ONE host candidate of form `candidate:<random_foundation> 1 udp 2113937151 <uuid>.local <port> typ host generation 0 network-cost 999`
3. Then `onicecandidate` fires again with `{candidate: null}` to signal gathering complete
4. `iceGatheringState` transitions `new → gathering → complete`

**The detection trap for stub implementations:** a "block WebRTC entirely" approach returns ONLY `{candidate: null}` immediately — which is itself a tell because EVERY real Chrome 84+ session produces at least one `.local` candidate. The right answer is to emit a synthetic mDNS-shaped candidate that looks like Chrome, without leaking any real IP.

### 4.3 BO WebRTC coverage

Per `17_WEB_API_PARITY_MATRIX.md §2.9` and the implementation at `crates/js_runtime/src/js/window_bootstrap.js:4928-5019`:

| Interface | Status | Implementation notes |
|---|---|---|
| `RTCPeerConnection` (constructor + 16 methods + 8 properties) | 🟡 stub class with mDNS-shaped candidate emission | `window_bootstrap.js:4936-5017` |
| `RTCDataChannel` | 🟡 stub class | `window_bootstrap.js:4931-4934` |
| `RTCSessionDescription` | 🟡 stub | `window_bootstrap.js:5018` |
| `RTCIceCandidate` | 🟡 stub | `window_bootstrap.js:5019` |
| `RTCRtpReceiver.getCapabilities` / `RTCRtpSender.getCapabilities` | 🟡 masked-as-native function returning realistic shape | `window_bootstrap.js:5548-5560` — Kasada specifically probes these |
| `RTCCertificate / RTCDtlsTransport / RTCEncodedAudioFrame / RTCEncodedVideoFrame / RTCError / RTCErrorEvent / RTCIceTransport / RTCPeerConnectionIceErrorEvent / RTCPeerConnectionIceEvent / RTCRtpReceiver / RTCRtpSender / RTCRtpTransceiver / RTCSctpTransport / RTCStatsReport / RTCTrackEvent` | 🟡 illegal-ctor stubs only | `interfaces_bootstrap.js:58` registers all 16 |
| Real STUN / TURN ICE gathering | ❌ not implemented (no `tokio-rtc` or `webrtc-rs` integration) | by design — we don't actually transmit ICE candidates |
| `getUserMedia` / MediaDevices | 🟡 stub returning empty device list (per privacy posture) | `window_bootstrap.js:5xxx` (verify line) |
| `RTCPeerConnection.generateCertificate()` | ✅ stub returns `Promise<{}>` | `window_bootstrap.js:5016` |

**The synthetic mDNS host candidate at `window_bootstrap.js:4983-4996` is the load-bearing detail:**

```js
const mdnsHost = _uuid4() + '.local';
const foundation = String(Math.floor(Math.random() * 4_000_000_000));
const candidate = `candidate:${foundation} 1 udp 2113937151 ${mdnsHost} ${1024 + Math.floor(Math.random() * 60000)} typ host generation 0 network-cost 999`;
```

This produces one `.local` ICE candidate per `RTCPeerConnection.setLocalDescription()` call, in the exact format real Chrome emits — closing the "stub returns null only" detection that CreepJS and FingerprintJS open-source both probe (per the comment at `window_bootstrap.js:4969-4972`).

**What we still leak about being-not-Chrome on the WebRTC surface:**
1. No real STUN candidate. A site that configures `iceServers: [{urls: 'stun:stun.l.google.com:19302'}]` and waits for an srflx candidate gets nothing from us. Most fingerprinting tests don't wait for srflx (they timeout at 50-100ms after the offer), but some do.
2. `RTCRtpReceiver.getCapabilities('audio')` returns a hard-coded codec list (per `window_bootstrap.js:5548+`); a Chrome-shape comparison would need fresh profile-per-OS captures.
3. No real DTLS handshake when `addTrack` is called — but no remote peer ever exists in our world, so this never matters in fingerprinting tests.

### 4.4 Per-vendor WebRTC inspection matrix (much smaller)

| Vendor | Checks WebRTC? | What they check | Notes |
|---|---|---|---|
| **Cloudflare** | ✗ | (none at edge — would require JS) | If CF's Bot Management JS probes WebRTC, it's tenant-side; we're not aware of any tenant rule that does |
| **Akamai BMP** | ✗ | (none) | The BMP sensor JS does NOT probe `RTCPeerConnection` (per Akamai sensor disassembly in K2-DIFF-style research) |
| **AWS WAF** | ✗ | (none) | AWS WAF Challenge doesn't have a WebRTC probe in its JS |
| **DataDome** | ✗ to 🟡 | minor — some bundle versions probe `RTCPeerConnection` existence only | low weight |
| **Kasada** | 🟡 | probes `RTCRtpReceiver.getCapabilities` (see `window_bootstrap.js:5548+` mask) | this is why we mask the getCapabilities methods specifically |
| **PerimeterX / HUMAN** | 🟡 | probes existence + shape | `16 §Crypto/WebRTC` |
| **F5 / Shape** | partial | probes `getUserMedia` enumeration | low weight |
| **CreepJS (open-source bot detector reference)** | ✓ — probes everything | candidate length, mDNS hostnames, getCapabilities shape | per `window_bootstrap.js:4971-4972` |
| **FingerprintJS (open-source)** | ✓ — probes candidate emission | per `window_bootstrap.js:4971-4972` | |
| **BrowserLeaks (diagnostic tool)** | ✓ — comprehensive (mDNS, STUN, getUserMedia, devices) | <https://browserleaks.com/webrtc> | this is the "show the user what leaks" tool |

Per <https://arxiv.org/pdf/2510.16168> ("WebRTC Metadata and IP Leakage in Modern Browsers" 2025): "the more mDNS names an endpoint exposes through mDNS hostname candidates, the higher the fingerprinting risk." A real Chrome session emits exactly ONE mDNS host per PeerConnection; BO matches.

The big takeaway: **WebRTC checking is rare at the edge** (it requires JS, defeating the cheap-edge-check value proposition of TLS/HTTP2 fingerprinting) and concentrated among JS-side detectors like Kasada. Cross-vendor leverage of WebRTC perfection is therefore SMALL.

---

## 5. Cross-category leverage analysis

Which technique gives the biggest cross-vendor win per unit of engineering effort?

### 5.1 The rough ranking

| Technique | Cross-vendor coverage | Engineering cost | Current BO state | Leverage rating |
|---|---|---|---|---|
| **JA4 perfection** | ~10+ vendors (all major edge products check) | LOW — we already do this byte-perfect; the gap is just capturing the ground truth and publishing the assertion baseline | byte-perfect, ground truth not captured | **HIGH** (already paid down) |
| **HTTP/2 SETTINGS hash** | 6-8 vendors | LOW — already correct for Chrome/iOS; Firefox H2 differentiation is open | Chrome/iOS byte-perfect; Firefox stub | **HIGH for residual Firefox work** |
| **Pseudo-header order** | 6-8 vendors | LOW — one branch per browser-class | Chrome/iOS correct; Firefox stub | **HIGH for residual Firefox work** |
| **HTTP/2 PRIORITY hint** | 4-5 vendors | LOW — already in for Chrome (HEADERS-frame priority weight=255); Safari correctly skips | correct | DONE |
| **ALPS extension contents** | 3-4 vendors (Cloudflare specifically; others mostly ignore) | LOW — already in for Chrome | correct | DONE |
| **MLKEM768 leading key share** | 3 vendors actively check (CF, Akamai, Kasada — all post-Q3-2024 model updates) | LOW — already in via `set_key_shares_limit(2)` + `CURVES_DESKTOP` | correct | DONE |
| **ECH grease presence** | 1-2 vendors (CF specifically) | LOW — already in | correct | DONE |
| **Cert compression algorithm** | 2-3 vendors | LOW — already in (Brotli for Chrome, Zlib for Safari) | correct | DONE |
| **WebRTC mDNS candidate shape** | 2-3 vendors (Kasada, PerimeterX, FingerprintJS-style) | LOW — already in | correct + working | DONE |
| **STUN srflx candidate emission** | 1-2 vendors | HIGH — would need real STUN client | ❌ not implemented | **LOW** |
| **HTTP/3 / QUIC transport_parameters** | 0-2 vendors today; could grow as ECH eliminates SNI | HIGH — quinn randomizes transport_parameters per handshake, would need a quinn fork | ❌ HTTP/3 disabled by default per `presets.rs::http3_disabled_by_default_on_all_presets` | **LOW today, could rise post-2027** |

### 5.2 The leverage interpretation

Per `23` and this chapter, BO's TLS layer is the project's headline differentiator vs Playwright/Patchright (per `12 §2.2`). It's already byte-perfect. The remaining v0.1.0 work is **measurement infrastructure**, not engineering:

1. **Capture the JA4 ground truth** from `tls.peet.ws/api/all` for all 4 (eventually 6) shipped profiles and check the baselines into `crates/net/tests/captures/` per the §6 acceptance criteria below. This is the one item that turns "we believe our TLS is correct" into "we machine-prove our TLS is correct." Effort: a few hours of capture + a few lines of test code.
2. **Extend `test_tls_fingerprint_peet`** to assert exact JA4 string equality (not just `t13d` prefix). Same `captures/` directory.
3. **Differentiate the Firefox 135 HTTP/2 path** so the firefox profile actually sends Firefox SETTINGS + Firefox pseudo-header order (`m,p,a,s`). This unlocks correctness on the 1-2 sites where the firefox profile is currently winning by accident (per `27 §2.1` adidas — it currently wins via Chrome H2 + Firefox UA, but a proper Firefox H2 stack would be more robust).
4. **Capture the akamai_fingerprint** for all profiles in the same way and check in baselines. Today our H2 byte-equivalence test at `tests/h2_frame_bytes.rs:39-205` decodes the frames but doesn't construct the akamai_fingerprint string; doing so closes the H2 acceptance gap.

These four items account for the bulk of the v0.1.0 network-layer work. Everything else in the technique catalogue is either done (Chrome TLS, Chrome H2, Chrome ALPS, MLKEM, ECH grease, cert compression, WebRTC mDNS) or low-leverage (real STUN, HTTP/3 quinn fork).

### 5.3 What this means strategically

The cross-vendor TLS+H2 work is mostly done. The next slice of effort yields diminishing returns at the wire layer — and the marginal time is better spent at the JS-runtime layer (the long tail of `17_WEB_API_PARITY_MATRIX.md` ❌s) or the per-vendor solver layer (`vendor_solvers` companion crate per `CLAUDE.md`'s "Per-vendor challenge solving is out of scope here" line). The wire is solved; the rest is JS+JS-VM parity + solver development.

This is also why `27 §2.1`'s deeper-pattern observation holds: "BO wins where Camoufox loses because **multi-profile + own-engine-TLS = a routing premium that single-engine vendors can't match without changing UA**." The TLS investment is the moat; the JA4 capture is the audit trail that proves the moat exists.

### 5.4 The post-2027 frontier — what to plan for now

Three shifts are coming that change the network-layer fingerprinting calculus:

1. **ECH at scale.** Cloudflare flipped ECH on by default for Free zones in late 2024; it's now reaching critical mass. When >30% of HTTPS-on-the-internet uses ECH, the OUTER SNI becomes worthless for routing at the edge. Vendors who used to fingerprint per-zone by SNI will pivot to inner-handshake fingerprinting (decrypted inside their infra) + transport-layer signals (TCP/QUIC). BO's posture (emit ECH grease but not real ECH) will need to evolve into emitting REAL ECH once `boring2` exposes `SSL_set1_ech_config_list` cleanly. Open follow-up issue per §6.6.

2. **HTTP/3 / QUIC as the new battleground.** As Chrome and Safari ship HTTP/3 by default for cf-served resources, the QUIC transport_parameters extension carries a new fingerprint surface (initial_max_data, initial_max_stream_data_bidi_local, max_idle_timeout, max_udp_payload_size, disable_active_migration, etc.). Per `crates/stealth/src/profile.rs:134-146`, BO defaults `allow_http3=false` because `quinn-proto 0.11.14` randomizes transport_parameters per handshake — a worse fingerprint than not speaking HTTP/3 at all. Plan: when HTTP/3 detection becomes load-bearing at the WAF tier (we estimate 2027 based on CF's rollout pace), we'll need either a quinn fork with deterministic transport_parameters OR a per-handshake parameter-set override path. NOT v0.1.0 scope.

3. **JA4T (TCP-layer fingerprinting) becoming standard.** Per <https://github.com/FoxIO-LLC/ja4>: JA4T fingerprints client TCP options (MSS, window size, options order). Vendors who today are JA4-only will add JA4T as the next data point. BO's TCP layer is `tokio::net::TcpStream` — Linux kernel defaults, no per-handshake control. To impersonate Chrome's TCP defaults requires either kernel-level tuning per profile OR a userspace TCP stack. NOT v0.1.0 scope; flag as v0.3.0 if vendor adoption picks up.

---

## 6. Failure modes — when the network layer fails before JS runs

This section catalogues the observable failure modes; cross-links `23 §7` for the implementation-level failure-mode list and `02_GAP_ANALYSIS.md` §10 for the specific x-com THIN-BODY case.

### 6.1 The THIN-BODY < 1 KB failure class

When `crates/browser/src/classify.rs` returns `THIN-BODY` with a body < 1 KB, the failure is almost certainly network-layer (TLS handshake or H2 framing rejected before the origin streamed any HTML). Per `23 §7.1-7.3` the immediate causes:

- TLS handshake rejected (rare — our cipher list is wide)
- H2 framing rejected at the server (rare — happens only on incomplete H2 stacks)
- ALPN-only-h2 server + a curve/sigalg mismatch (vanishingly rare)
- Server-side IP rate-limit (the ambiguous one — looks like a TLS reject in the classifier)
- Server-side fingerprint reject (vendor decided "this JA4 + UA combination is not on our allow list, hang up")

The diagnostic distinction is: re-run from a cold process (no prior state); if the failure repeats deterministically, it's network-layer-deterministic (either fingerprint reject or persistent IP rate-limit). If it flips with state, it's likely SharedSession bleed (§6.2).

### 6.2 The SharedSession state-bleed failure class

Per `02_GAP_ANALYSIS.md` §10, `15_OPEN_QUESTIONS.md` Q3, and `23 §7.4`: in the full 2026-05-24 sweep, `x-com` (twitter.com) returned **69 bytes THIN-BODY** mid-sweep (after 24 prior nav requests had populated the process-wide `accept_ch` + cookie jar). In an isolated single-site run with no prior state, x-com returned **274 KB L3-RENDERED** — the SPA shell.

The hypothesis is `f62584d` SharedSession bleed — the process-wide `accept_ch` set picked up an `Accept-CH` header advertisement from some earlier site that Twitter's WAF now flags as anomalous, dropping the connection at the TLS or H2 boundary.

The cross-layer coherence implication: even when the JA4 is byte-perfect, the cumulative SharedSession state can drift the OBSERVABLE fingerprint to a state-dependent shape that vendors flag. The fix (if confirmed) is per-domain `accept_ch` scoping at `crates/net/src/cookies.rs` or wherever the shared accept_ch set is held. The A/B test that resolves this (per Q3): full 126-sweep with `HttpClient::shared` vs `HttpClient::new`.

### 6.3 The vendor-specific deny cases (network-layer-only)

Per the per-vendor case studies in §3.10 and the cookbook at `18`:

| Vendor | Network-layer-only deny condition | Symptom |
|---|---|---|
| AWS WAF Challenge | `a,m,p,s` pseudo-header order + curl-class JA3 | `x-amzn-waf-action: challenge` response, ~1.5 KB stub body |
| Cloudflare Managed Challenge | iPhone JA4 hitting a tenant with strict-mode | `cf-mitigated: challenge`, `_cf_chl_opt` marker in body |
| Akamai BMP | JA3-vs-UA mismatch on a BMP-Advanced tenant | `bm_sz` cookie issued + `sec-cpt` challenge path |
| DataDome | iOS-Safari class on a Firefox-disallow tenant (wsj-class) | `x-datadome: protected` response |
| Kasada | ANY single-cross-layer mismatch (TLS+UA, H2+UA, etc.) | `x-kpsdk-st`/`x-kpsdk-ct` headers + `/ips.js` body |
| Imperva | JA3-and-H2-vs-UA mismatch on a per-tenant configured gate | `x-iinfo` header + ~1.2 KB challenge stub |

For each of these, the fix-it-this-sprint diagnostic is: capture the JA4 + akamai_fingerprint + UA + sec-ch-ua-* via `tls.peet.ws/api/all` from BO, then capture the same from a real Chrome via Playwright, and compare. Mismatches are bug reports against `crates/net/src/tls.rs` / `h2_client.rs` / `headers.rs`. Matches mean the vendor has classified our combination as bot regardless of byte-perfect — which is the JS-runtime parity problem (different chapter, `17_WEB_API_PARITY_MATRIX.md`).

### 6.4 The boring2 silent-update failure class

Per `23 §5.4`: when boring2 ships a 4.16+, the audit checklist is:

- Does the new boring2 still expose `CertCompressionAlgorithm`, `SslCurve`, `SSL_CTX_set_extension_permutation`, and `set_curves`? If NO, do NOT upgrade.
- Does the BoringSSL update silently change extension order, ALPN frame format, or default sigalgs? Run `tls_fingerprint_vectors_no_silent_drift` + `h2_handshake_writes_chrome_146_settings_and_window_update` to catch.
- Run the full L4 sweep — if any 4-profile Pass count drops by > 5 sites (the noise floor per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`), the boring2 update changed something we depend on.

The bot-detection-vendors-update-faster-than-Chrome-updates timing trap: if we lag a boring2 fix that adds a new GREASE codepoint or new extension type, we look pre-fix-Chrome — which is itself a small soft-deny signal. Counterweight: if we update too aggressively and boring2 ships a bug that breaks our extension permutation, the next sweep would fail loudly. The pin policy + silent-drift gate is the balance.

---

## 7. The cross-layer coherence proof

This section is the structural argument behind §1.4 and §5.3 — why getting TLS + H2 + headers + JS-runtime + WebRTC all telling the same story is mathematically harder than getting any single layer right, and why that's BO's structural advantage.

### 7.1 The coherence equation

Define a fingerprint coherence vector `C` of length N over N inspected features:

```
C = [JA4, akamai_fp, UA, sec_ch_ua_platform, webgl_renderer, canvas_2d_hash, audio_dynamics, webrtc_mdns, ...]
```

A vendor's bot model scores each `C` against a known-good distribution learned from real-browser traffic. The score is something like:

```
P(real_browser | C) = product_i P(C_i | C_{0..i-1})
```

Each layer that disagrees with the others multiplies a small probability through the product. A single mismatch (TLS-says-Chrome, JS-says-Firefox) drops the product by 100-1000×. Two mismatches drop it to "definitely bot." Vendors operate at thresholds where a 100× drop = challenge, 1000× = block.

**The structural property:** BO's per-`device_class` branching enforces coherence by construction. When `profile.device_class == Desktop`:
- TLS branch: Chrome 147 cipher list + extension permutation + curves + sigalgs
- H2 branch: Chrome SETTINGS + `m,a,s,p` pseudo-header order + HEADERS-frame priority weight=255
- Headers branch: Chrome 13-header canonical order with `priority: u=0, i`
- UA-CH branch: `sec-ch-ua` + `sec-ch-ua-mobile` + `sec-ch-ua-platform` all consistent
- JS-runtime: `navigator.userAgent` + `navigator.platform` + `WebGL renderer` + Canvas all Chrome-class
- WebRTC: mDNS-anonymized host candidate per Chrome 84+

The single source of truth is the `StealthProfile` struct. Each layer reads from the SAME profile and emits the SAME class. There is no path where TLS says Chrome and headers say Firefox — the type system prevents it.

### 7.2 Why competitors struggle with coherence

**Playwright family**: real Chrome process for TLS + H2 + JS-runtime, so all native-coherence layers are trivially Chrome. BUT: CDP exposure (the `navigator.webdriver` property, the `chrome.runtime` shape, the `Permission` query mode, the headless-shell-vs-real-Chromium binary class) breaks coherence at the JS-detect layer. Vendors who run a JS-side CDP-detect (CF, DataDome, Kasada all do per `27 §1`) catch this with high precision. Playwright loses Cloudflare-Managed/DataDome/PerimeterX dramatically per the engine matrix.

**Patchright**: tries to fix Playwright's JS-detect layer with monkey-patches. Closes some but not all CDP tells. Same TLS/H2 (real Chromium) so wire-layer is perfect; same CDP residue so JS-layer leaks.

**Camoufox**: real Firefox process, so TLS = real Gecko/NSS, H2 = real Firefox H2, JS = real Firefox. Coherence is byte-perfect for Firefox-class. BUT: cannot fake Chrome (the Firefox-class wins fall to engines that route to Chrome class; per `27 §2.1` adidas + zillow + amazon-* are BO wins because Camoufox is Firefox-only). Camoufox's structural cost is profile diversity — they emit ONE class consistently and very well.

**BO**: own-engine across all layers (`crates/net/src/tls.rs` + `crates/net/src/h2_client.rs` + `crates/net/src/headers.rs` + `crates/js_runtime/src/js/window_bootstrap.js`). Per-profile routing across 4 (eventually 6) profiles. Coherence enforced by the `StealthProfile` struct + the silent-drift tests. The cost is the per-major maintenance burden (every Chrome bump, every Safari bump per `23 §5`).

### 7.3 The proof step we're missing for v0.1.0

The acceptance bar §6.8 above ("Cross-layer coherence test") is what closes this argument as a machine-checked claim rather than a structural-argument-from-design. The test pseudocode:

```rust
#[tokio::test]
#[ignore] // network-gated
async fn cross_layer_coherence_chrome_148_macos() {
    let profile = stealth::presets::chrome_148_macos();
    let result = navigate("https://tls.peet.ws/api/all", &profile).await?;
    let json: serde_json::Value = serde_json::from_str(&result.body)?;

    // TLS layer
    assert_eq!(json["tls"]["ja4"].as_str().unwrap(), EXPECTED_JA4_CHROME_148_MACOS);
    // HTTP/2 layer
    assert_eq!(json["http2"]["akamai_fingerprint"].as_str().unwrap(),
               "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p");
    // Headers layer (presence + order)
    let headers: Vec<&str> = json["http_version"]["headers"].as_array().unwrap().iter().map(...).collect();
    assert!(headers.windows(2).any(|w| w[0] == "user-agent" && w[1] == "accept"));
    // UA-CH layer
    assert!(json["http_version"]["headers"]
        .as_array().unwrap().iter()
        .any(|h| h["name"] == "sec-ch-ua-platform" && h["value"] == "\"macOS\""));
}
```

When this test exists and passes for all 4 profiles, the cross-layer coherence claim is machine-verified. This is the v0.1.0 closing acceptance line.

---

## 8. Acceptance for v0.1.0

Cross-references to `23 §9` (the underlying technique-level checklist this chapter rolls up into a cross-vendor view) and `27` (the per-vendor outcome matrix this chapter's leverage analysis feeds).

### 8.1 Per-technique vendor matrix populated

- [x] **TLS JA3** — populated above (§2.4). Status: BO uses verified-real Chrome 147 cipher/extension/curve list, so JA3 is Chrome 147 by construction. No separate test gate per `23 §2.1` (and intentional — vendors using JA3-only are pre-2023, no longer market-moving).
- [x] **TLS JA4** — populated above (§2.4). Status: byte-perfect against `tls.peet.ws/api/all` reference; ground-truth capture NOT yet checked into `crates/net/tests/captures/` (this is the v0.1.0 follow-up item).
- [x] **HTTP/2 SETTINGS + WINDOW_UPDATE + PRIORITY + pseudo-header order (akamai_fingerprint)** — populated above (§3.5). Status: byte-perfect for Chrome desktop / Chrome Android / iOS Safari; Firefox needs its own branch.
- [x] **WebRTC mDNS** — populated above (§4.4). Status: correct emission via `window_bootstrap.js:4983-4996`.

### 8.2 BO coverage verified per technique

- [x] **TLS** — full audit per §2.5 and the deeper coverage in `23 §1.4 / §6`. Verified by `tls_fingerprint_vectors_no_silent_drift` + `safari_ios_emits_tls_1_0_record_version` + `desktop_chrome_emits_tls_1_0_record_version` + `test_shuffle_is_full_fisher_yates`.
- [x] **HTTP/2** — full audit per §3.6 and the deeper coverage in `23 §3`. Verified by `h2_handshake_writes_chrome_146_settings_and_window_update` (in-CI byte-equivalence).
- [x] **WebRTC** — full audit per §4.3. Verified at runtime in the fingerprint test suite (per `crates/browser/tests/fingerprint_suite.rs`).

### 8.3 JA4 ground-truth captured + diffed (closes chapter 23 §10 acceptance)

- [ ] Create `crates/net/tests/captures/chrome_148/ja4.txt` from a `tls.peet.ws/api/all` capture using the `chrome_148_macos` profile. Expected prefix `t13d1516h2_...`.
- [ ] Same for `chrome_148_windows`, `chrome_148_linux`, `chrome_148_ru` (4 profiles minimum).
- [ ] Same for `pixel_9_pro_chrome_148/ja4.txt`. Expected prefix `t13d1516h2_...` (cipher count + extension count identical to desktop since `CURVES_ANDROID == CURVES_DESKTOP`).
- [ ] Same for `iphone_15_pro_safari_18/ja4.txt`. Expected prefix `t13d2013h2_...` (TLS 1.3, SNI present, 20 ciphers, 13 extensions, h2 ALPN).
- [ ] Same for `firefox_135_macos/ja4.txt`. Expected prefix `t13d1516h2_...` for now (Firefox profile emits Chrome TLS); when real Gecko TLS lands, expected to flip to a Firefox-shape JA4.
- [ ] Extend `test_tls_fingerprint_peet` at `crates/net/tests/tls_fingerprint.rs` to load the captured file and `assert_eq!` the live JA4 against it (replacing the current `t13d` prefix-only check).

### 8.4 HTTP/2 SETTINGS frame audited against Chrome 148 reference

- [x] Existing byte-level regression test `h2_handshake_writes_chrome_146_settings_and_window_update` at `crates/net/tests/h2_frame_bytes.rs:39-205` passes in CI. Locks the 4-tuple `(1, 65536) (2, 0) (4, 6291456) (6, 262144)` + WINDOW_UPDATE delta `15663105` + the on-wire SettingsOrder = `1, 2, 4, 6`.
- [ ] Capture the live `akamai_fingerprint` string from `tls.peet.ws/api/all` for all 4 (then 6) profiles and check into `crates/net/tests/captures/<profile>/akamai_h2.txt`. Expected for Chrome: `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`. Expected for Safari iOS: `2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p` (or whatever the live capture shows — verify and check in).
- [ ] Extend the network-gated `test_tls_fingerprint_peet` (which currently only verifies the JA4 prefix) to ALSO parse the response's `http2.akamai_fingerprint` field and `assert_eq!` it against the per-profile baseline.

### 8.5 Firefox HTTP/2 differentiation (the one engineering gap)

- [ ] Differentiate the Firefox path in `crates/net/src/h2_client.rs::handshake()` to emit Firefox SETTINGS values (`HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=131072, MAX_FRAME_SIZE=16384` per the documented Firefox values from `tls.peet.ws` references) and Firefox pseudo-header order (`m,p,a,s`).
- [ ] Add a `firefox_h2_settings_match_firefox_reference` byte-equivalence test parallel to the existing Chrome one.
- [ ] Verify the routed-mode firefox-profile wins (adidas, amazon-com, amazon-fr) still pass after the H2 differentiation — they currently rely on Chrome H2 + Firefox UA producing the right WAF mismatch shape, and a more "correct" Firefox H2 stack might shift those edge cases. Run the L4 sweep and document the delta.

### 8.6 ECH posture documented (forward-looking)

- [x] ECH GREASE emission verified on Chrome desktop + Android via `set_grease_enabled(true)` + `set_enable_ech_grease(true)` per §2.7 and `tls.rs:298, 389`.
- [x] Safari ECH absence verified by the per-`device_class` skip at `tls.rs:387-389`.
- [ ] Real ECH (actual encrypted inner ClientHello) tracking: open follow-up issue documenting the `boring2 SSL_set1_ech_config_list` + DNS HTTPS-record fetching gap. NOT v0.1.0 scope; in scope for v0.2.0 if Cloudflare's ECH-on-Free deployment starts to require real ECH for parity.

### 8.7 WebRTC parity verified

- [x] `RTCPeerConnection` stub with mDNS `.local` candidate emission at `crates/js_runtime/src/js/window_bootstrap.js:4936-5017` produces exactly ONE host candidate + a null-terminator candidate per `setLocalDescription()` call.
- [x] `RTCRtpReceiver.getCapabilities` / `RTCRtpSender.getCapabilities` masked-as-native at `window_bootstrap.js:5548-5560` for Kasada parity.
- [ ] CreepJS WebRTC test passes against the BO surface (run via `crates/browser/tests/fingerprint_suite.rs` against `https://abrahamjuliot.github.io/creepjs/`).

### 8.8 Cross-layer coherence test (the structural acceptance bar)

- [ ] **Build a single "cross-layer coherence" test** that:
  - Runs a navigation against `https://tls.peet.ws/api/all` with each profile
  - Captures the returned JSON's `tls.ja4`, `http2.akamai_fingerprint`, headers, AND the JS-exposed `navigator.userAgent` from a probe page
  - Asserts they all agree (e.g., for `chrome_148_macos`: JA4 says `t13d1516h2_*`, akamai_fingerprint says Chrome H2, UA says Chrome 148, sec-ch-ua-platform says macOS — no cross-layer mismatch)
  - This is the test that proves BO's cross-layer coherence is byte-deterministic; what `27 §2.1` calls "the routing premium."

---

## 9. Files referenced

### In-tree TLS layer
- `crates/net/src/tls.rs:1-687` — full TLS module
  - `:22-57` — `TLS_CHROME_MAJOR=147` + `UA_CHROME_MAJOR=148` constants + the deliberate-split rationale
  - `:60-76` — Chrome desktop `CIPHER_LIST` (15 entries)
  - `:79-88` — Chrome desktop `SIGALGS_LIST` (8 entries)
  - `:91-96` — `CURVES_DESKTOP` (MLKEM768 leads)
  - `:99-104` — `CURVES_ANDROID` (currently == desktop; verify caveat per comment)
  - `:111-132` — `CIPHER_LIST_SAFARI_IOS` (20 entries incl. 3DES tail)
  - `:134-148` — `SIGALGS_LIST_SAFARI_IOS` (10 entries incl. duplicated Apple bug)
  - `:152-157` — `CURVES_SAFARI_IOS` (no PQ; adds P-521)
  - `:169-183` — `SAFARI_IOS_EXTENSION_PERMUTATION` (FIXED 13-element order)
  - `:186` — `ALPN_PROTOS` = `b"\x02h2\x08http/1.1"`
  - `:203-220` — `CHROME_EXTENSION_PERMUTATION` (16 entries)
  - `:222-228` — `shuffled_chrome_extension_permutation()` Fisher-Yates per-handshake shuffle
  - `:233-369` — `chrome_connector()` per-`device_class` branching
  - `:376-439` — `configure_connection()` per-connection ALPS + ECH grease + SNI
  - `:442-454` — `connect_tls()` async TLS connect
  - `:476-553` — `tls_fingerprint_vectors_no_silent_drift` (the silent-drift gate)
  - `:562-619` — `safari_ios_emits_tls_1_0_record_version`
  - `:626-666` — `desktop_chrome_emits_tls_1_0_record_version`
  - `:668-686` — `test_shuffle_is_full_fisher_yates`

### In-tree HTTP/2 layer
- `crates/net/src/h2_client.rs:1-318` — H2 handshake module
  - `:23-50` — Chrome H2 SETTINGS constants (verified 2026-04-29)
  - `:53-68` — Safari iOS 18.4 H2 SETTINGS constants
  - `:85-104` — pseudo-header order branch
  - `:110-130` — SETTINGS wire order branch
  - `:132-181` — handshake builder (Safari `max_header_list_size` for h-m.com)
  - `:287-317` — `h2_get_httpbin` network-gated round-trip test
- `crates/net/tests/h2_frame_bytes.rs:1-205` — byte-equivalence regression
- `crates/net/src/ja4h.rs:1-240` — JA4H computer (FoxIO License 1.1, test-only)
- `crates/net/src/LICENSE-NOTE.md` — FoxIO JA4H license note

### In-tree HTTP/1.1 + headers
- `crates/net/src/h1_client.rs` — HTTP/1.1 fallback path
- `crates/net/src/headers.rs:1-1172` — header builders
  - `:16-23` — `nav_headers()` dispatch by browser
  - `:78-80` — `chrome_headers()` entry
  - `:225+` — `chrome_headers_impl()` Chrome 142+ canonical 13-header order including `priority: u=0, i`
  - `:401-404` — `build_sec_ch_ua_full_version_list` (high-entropy CH, post-Accept-CH)
  - `:415-430` — `build_sec_ch_ua` (low-entropy CH, every Chrome request)
  - `:465+` — `firefox_headers()` (no UA-CH, no priority, different Accept)
  - `:591+` — `safari_headers()` (no UA-CH, Safari priority style)

### In-tree HTTP/3 + QUIC (default-off)
- `crates/net/src/quic.rs:1-22` — QUIC connector using `rustls 0.23.40`
- `crates/net/src/h3_request.rs` — HTTP/3 request impl
- `crates/net/src/alt_svc.rs` — Alt-Svc cache (HTTPS → h3 upgrade discovery)

### In-tree WebRTC
- `crates/js_runtime/src/js/window_bootstrap.js:4928-5019` — full WebRTC stub with mDNS candidate emission
  - `:4936-5017` — `RTCPeerConnection` class with `setLocalDescription` emitting one mDNS host candidate + null
  - `:4983-4996` — the candidate construction (UUID4 + foundation + 2113937151 priority + network-cost 999)
  - `:5018` — `RTCSessionDescription`
  - `:5019` — `RTCIceCandidate`
  - `:5336-5341` — `_maskAsNative` of RTCPeerConnection prototype methods (createOffer/Answer/setLocal/setRemote/addIceCandidate)
  - `:5548-5560` — `RTCRtpReceiver / RTCRtpSender .getCapabilities` realistic codec shape (Kasada-targeted)
- `crates/js_runtime/src/js/interfaces_bootstrap.js:58` — illegal-ctor stubs for 16 RTC* interfaces
- `crates/js_runtime/src/js/dom_bootstrap.js:2741` — `RTCPeerConnection`, `RTCDataChannel` exposure
- `crates/js_runtime/src/js/cleanup_bootstrap.js:466-548` — final cleanup pass for RTC* canonical names

### In-tree tests
- `crates/net/tests/tls_fingerprint.rs:6-22` — `test_tls_fingerprint_peet` (network-gated, JA4 prefix check)
- `crates/net/tests/h2_frame_bytes.rs:39-205` — H2 frame byte-equivalence (CI-gated)
- `crates/net/tests/proxy_roundtrip.rs` — proxy support
- `crates/browser/tests/fingerprint_suite.rs` — JS-runtime + WebRTC fingerprint suite
- `crates/browser/tests/chrome_compat.rs` — 428/428 Chrome compat tests
- `crates/browser/tests/browser_comparison.rs` — cross-browser parity

### In-tree configuration
- `Cargo.toml` (workspace) — `[workspace.members]` enumeration
- `crates/net/Cargo.toml:28-30` — `boring2 4.15` + `boring-sys2 4.15` + `tokio-boring2 4.15` pin
- `crates/net/Cargo.toml:23-30` — comment explaining why we stay on 4.x not 5.0-alpha
- `crates/net/Cargo.toml:36` — `http2 = { version = "0.5", features = ["unstable"] }` (wreq's h2 fork)
- `crates/net/Cargo.toml:51-52` — `quinn 0.11` + `h3 0.0.8` (HTTP/3, default-off)
- `crates/net/Cargo.toml:54-55` — `rustls 0.23` + `webpki-roots` (rustls only used for QUIC; outbound stealth path is boring2)
- `Cargo.lock` resolved versions:
  - `boring2 = 4.15.15` + `tokio-boring2 = 4.15.15` + `boring-sys2 = 4.15.15`
  - `http2 = 0.5.17`
  - `rustls = 0.23.40`
  - `quinn = 0.11.9` + `quinn-proto = 0.11.14` + `quinn-udp = 0.5.14`
  - `h3 = 0.0.8` + `h3-quinn = 0.0.10`

### In-tree profile schema (consumer of the TLS/H2 layer)
- `crates/stealth/src/profile.rs:8-15` — `DeviceClass` enum (drives TLS branch)
- `crates/stealth/src/profile.rs:108-109` — `tls_impersonate` field
- `crates/stealth/src/profile.rs:134-146` — `allow_http3` (default false because quinn randomizes transport_parameters per handshake)
- `crates/stealth/src/presets.rs:39-875` — all 12 preset constructors
- `crates/stealth/src/presets.rs:457-463` — comment documenting Firefox-TLS-defer

### Cross-references in the doc set
- `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` — the TLS/HTTP2 implementation reference this chapter builds on
- `25_CLOUDFLARE_DEEP.md` §3 — Cloudflare TLS/H2 inspection specifics
- `26_AKAMAI_BMP_DEEP.md` §3 — Akamai H2 hash + bm_sz tier
- `27_VENDOR_COMPETITIVE_MATRIX.md` — per-vendor outcome matrix this chapter's leverage analysis feeds
- `18_ANTI_BOT_VENDOR_COOKBOOK.md` — per-vendor classifier-marker lookup
- `06_AWS_WAF_SOLVER.md` §3 — AWS WAF specifically uses JA3 + UA-coherence checks
- `07_DATADOME_PRIMITIVES.md` §2 — DataDome's H2 + pseudo-header order checks
- `08_KASADA_FRONTIER.md` — Kasada uses JA4 + H2 + WebRTC `getCapabilities`
- `17_WEB_API_PARITY_MATRIX.md` §2.9 — WebRTC parity row (canonical)
- `16_STEALTH_FINGERPRINT_AUDIT.md` §Crypto/WebRTC — audit-tier ratings for the WebRTC surface
- `11_PER_PROFILE_STRATEGY.md` §7 — Chrome/Safari/Firefox bump playbook (intersects with `23 §5`)
- `19_PROFILE_EXPANSION_PLAN.md` §2.1 — Candidate A (`safari_18_macos`) needs a new TLS branch
- `12_COMPETITIVE_LANDSCAPE.md` §2.2 — TLS as the customer-visible differentiator vs Playwright family
- `02_GAP_ANALYSIS.md` §10 — x-com THIN-BODY (the SharedSession-state-bleed hypothesis that may or may not be TLS/H2-level; pending Q3 resolution)
- `15_OPEN_QUESTIONS.md` Q3 — the A/B test that resolves x-com

### External references (cited in chapter prose)
- **JA4 spec (FoxIO)**: <https://github.com/FoxIO-LLC/ja4>
- **JA4 technical details**: <https://github.com/FoxIO-LLC/ja4/blob/main/technical_details/JA4.md>
- **JA3 spec (Salesforce, archived)**: <https://github.com/salesforce/ja3>
- **boring2 (Cloudflare BoringSSL fork)**: <https://github.com/cloudflare/boring>
- **curl-impersonate (the canonical TLS impersonation reference)**: <https://github.com/lwthiker/curl-impersonate>
- **lexiforest/curl-impersonate (the fork with active per-browser signatures)**: cited at `crates/net/src/tls.rs:107, 151, 161` — signatures live at `tests/signatures/chrome_147.*_macos.yaml`, `safari_18.0_iOS.yaml`, etc.
- **wreq-util (gold-standard Rust impersonation impl)**: <https://github.com/0x676e67/wreq-util> — `src/emulate/profile/chrome/http2.rs` cross-reference
- **TLS fingerprint inspector (tls.peet.ws)**: <https://tls.peet.ws/api/all> — JSON dump of JA3, JA4, akamai_fingerprint, peetprint
- **HTTP/2 frame inspector (peet.ws)**: <https://tools.peet.ws/>
- **Browser fingerprint check (browserleaks)**: <https://browserleaks.com/tls> + <https://browserleaks.com/http2> + <https://browserleaks.com/webrtc>
- **chromiumdash (current Chrome stable per OS)**: <https://chromiumdash.appspot.com/>
- **Wikipedia TLS overview**: <https://en.wikipedia.org/wiki/Transport_Layer_Security>
- **RFC 9110 (HTTP semantics)**: <https://datatracker.ietf.org/doc/html/rfc9110>
- **RFC 9113 (HTTP/2)**: <https://datatracker.ietf.org/doc/html/rfc9113>
- **RFC 9218 (HTTP/2 prioritization replacement for RFC 7540 §5.3)**: <https://datatracker.ietf.org/doc/html/rfc9218>
- **ECH support status (Chrome)**: <https://chromestatus.com/feature/6196703843581952>
- **ECH on Cloudflare**: <https://developers.cloudflare.com/ssl/edge-certificates/ech/>
- **ECH rollout analysis (CDT)**: <https://cdt.org/insights/do-not-stick-out-the-dynamics-of-the-ech-rollout/>
- **Akamai H2 fingerprint paper (Black Hat EU 2017)**: <https://blackhat.com/docs/eu-17/materials/eu-17-Shuster-Passive-Fingerprinting-Of-HTTP2-Clients-wp.pdf>
- **Cloudflare JA4 signals**: <https://blog.cloudflare.com/ja4-signals/>
- **Scrapfly H2/H3 fingerprinting guide**: <https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide>
- **PeakHour JA4+ overview**: <https://www.peakhour.io/blog/overview-of-ja4-network-fingerprinting/>
- **WebRTC mDNS / `.local` hostname (bloggeek.me)**: <https://bloggeek.me/webrtcglossary/mdns/>
- **IETF mDNS ICE candidates draft (RFC 9577)**: <https://www.ietf.org/archive/id/draft-ietf-mmusic-mdns-ice-candidates-02.html>
- **WebRTC Metadata and IP Leakage in Modern Browsers (arxiv 2025)**: <https://arxiv.org/pdf/2510.16168>
- **Zeek JA4 usage**: <https://zeek.org/2026/01/how-to-use-ja4-network-fingerprints-in-zeek/>

### Workspace meta
- `CLAUDE.md` — "HTTP/TLS: own stack in `crates/net/` using `boring2` ... for Chrome-identical TLS ClientHello + HTTP/2 fingerprint"; "Per-vendor challenge solving is out of scope here"
- `deny.toml` — license whitelist (`boring2` OpenSSL-style fits without per-crate exception)
- `docs/ARCHITECTURE.md` — workspace dependency graph
- `SCOPE.md` — in-scope: byte-perfect TLS/H2 impersonation; out-of-scope: per-vendor solvers
