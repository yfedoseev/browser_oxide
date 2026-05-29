# NETWORK_fingerprint — BO network-layer fingerprint vs detection (external audit)

**Status:** external research + code-level verification, 2026-05-28
**Scope:** JA3/JA4/JA4+, boring2 ClientHello byte-match to Chrome 147/148, HTTP/2 SETTINGS + frame order + pseudo-header order (Akamai h2), HTTP/3/QUIC, ALPN, GREASE, TLS extension order/permutation, the UA-148 / TLS-147 class split, ECH horizon.
**Verified against:** `crates/net/src/tls.rs`, `crates/net/src/h2_client.rs`, `crates/net/src/quic.rs`, `crates/net/Cargo.toml`, `crates/stealth/src/presets.rs`.
**Companion repo docs read:** `docs/releases/v0.1.0-parity/39_NETWORK_LAYER_FINGERPRINTING.md` (read in full), `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` (referenced by 39).

---

## 0. TL;DR / headline

BO's **Chrome-class** wire fingerprint (TLS + HTTP/2) is byte-perfect and is the project's strongest moat — there is essentially nothing left to engineer for Chrome desktop / Chrome Android / iOS Safari at the wire layer; the residual work is *measurement infrastructure* (capture JA4/akamai_fingerprint baselines into tree) and *one real engineering gap* (Firefox H2 + TLS class). The single highest-confidence **leak** today is the **Firefox profile**: it advertises a Firefox UA + Firefox headers but emits Chrome's TLS ClientHello AND Chrome's HTTP/2 SETTINGS — a textbook cross-layer mismatch that JA4-vs-UA cross-checks (now universal at Cloudflare/AWS/Akamai per 2026 sources) are *specifically built to catch*. Camoufox, by contrast, runs the real Firefox NSS stack, so its Firefox JA4 is genuine — this is a place where Camoufox is strictly ahead of BO. The 2027 horizon is QUIC transport-parameter fingerprinting and ECH; BO's QUIC path (`crates/net/src/quic.rs`) is `rustls`-based (NOT boring2) and HTTP/3 is correctly defaulted off, so QUIC is a latent liability only if BO ever turns h3 on.

---

## 1. What the existing repo docs already concluded (cited)

Doc `39_NETWORK_LAYER_FINGERPRINTING.md` is the canonical cross-vendor view; `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` is the implementation reference. Their load-bearing conclusions:

1. **Detection happens before the page renders** (39 §1.1). TCP→ClientHello→H2 PREFACE+SETTINGS→headers→edge decision; everything left of "edge decision" is wire-byte-deterministic. If the ClientHello is wrong, the JS never runs. This is why the TLS layer is so heavily engineered — it is the entire game for the WAF/edge tier.
2. **Cross-layer mismatch is what vendors weight heaviest** (39 §1.4): TLS-JA4 / akamai-H2 / header-order / JS-`navigator.userAgent` / WebGL / canvas / WebRTC must all tell the *same* story. One disagreement = bot. This is the structural reason a UA-spoofing `requests` client is caught instantly.
3. **TLS is already byte-perfect** for Chrome 147 desktop, Chrome Android, and iOS Safari 18.x against `lexiforest/curl-impersonate` YAML signatures + `tls.peet.ws/api/all` as live oracle (39 §2.5). The whole per-feature catalogue (record version 0x0301, supported_versions, cipher list, MLKEM768 lead key share, Fisher-Yates extension shuffle, Brotli/Zlib cert compression, ALPS, ECH grease, NO_TICKET for Safari) is mapped to file:line in 39 §2.3.
4. **HTTP/2 is byte-perfect** for Chrome/iOS (39 §3.6). akamai_fingerprint `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p` for Chrome; `2:0;3:100;4:2097152;9:1|10420225|0|m,s,a,p` for Safari iOS. **Firefox H2 is explicitly NOT implemented** — the Firefox profile emits Chrome's H2 (39 §3.6 line "❌ NOT IMPLEMENTED").
5. **JA3 is not separately gated** (39 §2.1) — by construction it is Chrome 147's JA3 because the cipher/extension/curve lists are the verified-real Chrome lists. JA4 capture covers the same fields plus more, so a separate JA3 gate is intentionally omitted (and JA3 vendors are pre-2023, no longer market-moving).
6. **Known v0.1.0 gaps** (39 §2.6, §5.2, §8.3–8.5): (a) no JA4 ground-truth baseline file in `crates/net/tests/captures/`; the network test only asserts the `t13d` prefix; (b) no `akamai_fingerprint` baseline in tree; (c) `peetprint` / raw ClientHello hex not captured; (d) Firefox H2 + TLS not differentiated; (e) Safari-on-macOS branch not present.
7. **ECH posture** (39 §2.7): BO emits ECH *grease* for Chrome but not real ECH. When CF ECH-on-Free reaches critical mass, "Chrome that has ECH disabled" becomes a small signal. Real ECH (`SSL_set1_ech_config_list` + DNS HTTPS-record fetch) is deferred to v0.2.0.
8. **HTTP/3 / QUIC and JA4T are flagged as the post-2027 frontier** (39 §5.4): HTTP/3 is default-off because quinn randomizes transport_parameters per handshake (a *worse* fingerprint than not speaking h3); JA4T (TCP options) is unaddressable without kernel tuning / userspace TCP and is v0.3.0+.
9. **The x-com THIN-BODY case** (39 §6.2): a SharedSession `accept_ch` state-bleed hypothesis — even with byte-perfect JA4 the cumulative process-wide state may drift the observable fingerprint. The A/B that resolves it (`HttpClient::shared` vs `HttpClient::new` full sweep) is still open (Q3).

My audit below **confirms** each of these against current source, **corrects/sharpens** a few, and **adds** external 2026 findings + a re-ranked fix list.

---

## 2. Code-level verification (what BO actually emits today)

### 2.1 TLS — `crates/net/src/tls.rs` (verified line-by-line)

The doc-39 catalogue matches the source. Confirmed:

- `TLS_CHROME_MAJOR = 147` (`tls.rs:52`) and `UA_CHROME_MAJOR = 148` (`tls.rs:57`). The long doc-comment at `tls.rs:21-51` correctly argues the split is wire-coherent: Chrome's ClientHello did not change 147→148, and JA4 cannot encode the Chrome version (JA4 = TLS-ver + sorted cipher/ext counts + ALPN + sigalgs; none differ 147↔148). **This reasoning is externally correct** — see §3.1.
- `CIPHER_LIST` 15 entries (`tls.rs:60-76`), `SIGALGS_LIST` 8 entries (`tls.rs:79-88`), `CURVES_DESKTOP` with `X25519_MLKEM768` leading (`tls.rs:91-96`).
- Per-`device_class` branch at `chrome_connector()` (`tls.rs:233-369`): Safari iOS gets distinct ciphers (`CIPHER_LIST_SAFARI_IOS`, 20 entries incl. 3DES tail, `tls.rs:111-132`), the **duplicated `rsa_pss_rsae_sha384` Apple bug** reproduced verbatim (`tls.rs:142-143`), no-PQ curves + P-521 (`tls.rs:152-157`), `min_proto_version = TLS1` so 4 versions advertised (`tls.rs:285-289`).
- Chrome path: `set_grease_enabled(true)` (`tls.rs:298`), `set_key_shares_limit(2)` for the two MLKEM768+X25519 key shares (`tls.rs:306`), Brotli vs Zlib cert compression (`tls.rs:312-319`), `NO_TICKET` only for Safari (`tls.rs:324-326`).
- **Fisher-Yates extension shuffle** is genuine and per-handshake: `shuffled_chrome_extension_permutation()` (`tls.rs:222-228`) shuffles the full 16-element `CHROME_EXTENSION_PERMUTATION` and is applied via the raw `SSL_CTX_set_extension_permutation` FFI in an `unsafe` block with a proper `// SAFETY:` comment (`tls.rs:361-367`). The doc-comment at `tls.rs:194-202` correctly notes the *old 3-bucket scheme was folklore* that left `signature_algorithms` deterministically last — a real positional tell — and the current full-shuffle fixes it. This is a genuinely sophisticated detail most impersonation stacks get wrong.
- Note `tls.rs:300` sets `set_permute_extensions(false)` — correct, because BO drives the permutation *manually* via the FFI at `:361`; letting BoringSSL also permute would double-shuffle.
- ALPS payload (`tls.rs:393-403`) hand-encodes the inner SETTINGS frame `1:65536;2:0;4:6291456;6:262144` + empty ACCEPT_CH — this is consistent with the over-the-wire H2 SETTINGS (good coherence), Safari skips it entirely (`tls.rs:387`). ECH grease only for non-Safari (`tls.rs:389`).

**Silent-drift gate** `tls_fingerprint_vectors_no_silent_drift` (`tls.rs:476-553`) pins cipher string, sigalg string, curve order, extension count (16), and the UA=148/TLS=147 coherence. Plus `safari_ios_emits_tls_1_0_record_version` / `desktop_chrome_emits_tls_1_0_record_version` (`tls.rs:562-666`) empirically capture the first 5 ClientHello bytes and assert record version 0x0301 — this is a *real wire capture against a loopback listener*, not an assertion-from-memory, which is unusually rigorous. `test_shuffle_is_full_fisher_yates` (`tls.rs:668-686`) checks the shuffle preserves the set and is non-deterministic.

**Verdict:** Chrome + iOS Safari TLS is byte-perfect and well-defended against drift. No engineering gap.

### 2.2 HTTP/2 — `crates/net/src/h2_client.rs` (verified)

- Chrome SETTINGS constants `1:65536 / 2:0 / 4:6291456 / 6:262144` (`h2_client.rs:39-42`), connection window target `15_728_640` → wire delta `15_663_105` (`h2_client.rs:50`). The doc-comment at `h2_client.rs:34-38` records the historical bug (extra `3` + `5` settings → wrong Akamai hash `d23e6399…` vs Chrome's `52d84b11…`) and the 2026-04-29 reference capture that corrected it. Good provenance.
- Pseudo-header order branch (`h2_client.rs:90-104`): Chrome `m,a,s,p`, Safari `m,s,a,p`. Confirmed.
- SETTINGS wire order branch (`h2_client.rs:110-130`): Chrome declares the 8-entry canonical order (`1,2,3,4,5,6,8,9`) but only 4 carry values; Safari declares `2,3,4,9`. Confirmed.
- HEADERS-frame priority hint `weight=255, dep=0, exclusive=true` for Chrome (`h2_client.rs:167-174`); Safari skips it (`h2_client.rs:154-157` per NO_RFC7540_PRIORITIES). Confirmed.
- Safari workaround: `max_header_list_size` is set on the builder even though Safari does NOT advertise it on the wire, to stop h-m.com (Akamai) RST_STREAM INTERNAL_ERROR (`h2_client.rs:140-150`). This is an internal-validation-only value, not on the wire — coherent.

**The Firefox gap is real and confirmed.** `handshake()` (`h2_client.rs:85`) branches ONLY on `is_safari_ios`. Every non-iOS profile — including all three `firefox_135_*` presets — takes the Chrome branch and emits **Chrome SETTINGS + Chrome `m,a,s,p` pseudo-header order**. There is no Firefox arm.

### 2.3 The Firefox class split — the load-bearing leak

`crates/stealth/src/presets.rs:421-638` defines `firefox_135_macos/windows/linux`. Each sets `browser_name: "Firefox"`, a Gecko UA (`Mozilla/5.0 (…; rv:135.0) Gecko/20100101 Firefox/135.0`), AND `tls_impersonate: "firefox_135"` (`presets.rs:474, 550, …`). But the comment at `presets.rs:464-474` is explicit:

> "A real Firefox JA4 swap requires … NSS — substantial work tracked as a future item. Many sites that flip on Firefox UA do so based on the UA + headers …"

So the `tls_impersonate: "firefox_135"` string is **aspirational** — `tls.rs::chrome_connector()` never reads it to select a Firefox path; it only branches on `profile.device_class` (Desktop/Android/iOS). A Firefox-135-desktop profile has `device_class == Desktop` and therefore gets the **Chrome 147 ClientHello + Chrome H2**.

**Result observable to a vendor for any BO Firefox profile:**
- TLS JA4 → `t13d1516h2_…` (Chrome 147: 16 extensions, MLKEM768, Brotli, ALPS, ECH-grease)
- akamai H2 → `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p` (Chrome)
- UA / `navigator.userAgent` → `Firefox/135.0`
- Accept / sec-fetch headers → Firefox-style (no UA-CH, different Accept)

A real Firefox 135 would emit a *completely different* JA4 (different cipher set/count, no MLKEM768 desktop ordering, NSS-style extension order with no Fisher-Yates, no ALPS, no ECH-grease, no Brotli cert compression — Firefox uses a different extension set) and a different akamai hash (Firefox H2: `HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=131072, MAX_FRAME_SIZE=16384`, pseudo-order `m,p,a,s`). The mismatch between "JA4 says Chrome 147" and "UA says Firefox 135" is **exactly** the cross-layer signal doc-39 §1.4 says vendors weight heaviest, and which 2026 sources (§3.1) say is now universally deployed.

**This is the cleanest single network-layer leak in BO today.** It is masked only because (a) routed-mode currently uses the Firefox profile opportunistically on a few sites where the UA+headers alone flip the decision (39 §5.2 mentions adidas), and (b) most of the 126-corpus is routed to Chrome-class profiles where TLS *is* perfect. But any vendor that does JA4-vs-UA on the Firefox-routed sites will score BO as a mismatched bot.

### 2.4 HTTP/3 / QUIC — `crates/net/src/quic.rs` (verified)

Confirmed default-off: every preset sets `allow_http3: false` (`presets.rs:93, 185, 260, 314, 362, 489, 565, 638, 914`); the field is `profile.rs:155`.

Two notable code facts beyond what doc-39 says:

1. **The QUIC path uses `rustls`, not boring2.** `QuicClient::new()` builds `rustls::ClientConfig` (`quic.rs:19-22`) with ALPN `h3`. boring2 is the stealth path for TCP/TLS; `rustls 0.23` is pulled only for QUIC (`Cargo.toml:69-72`). **rustls's QUIC ClientHello is NOT Chrome's** — different cipher/extension defaults, no MLKEM, no GREASE-by-default, no extension shuffle. So if h3 were ever enabled, BO's QUIC fingerprint would be a generic-rustls fingerprint, instantly distinguishable from Chrome's QUIC. This reinforces doc-39's "h3 off is correct" but pins the *reason* more concretely than the doc (which cites quinn transport-param randomization; the rustls-ClientHello mismatch is an even larger tell).
2. **Transport parameters are only partially set** (`quic.rs:25-34`): `receive_window=15MB`, `stream_receive_window=6MB`, `max_concurrent_bidi/uni=1000`, `max_idle_timeout=30s`, `keep_alive=10s`. The §3.2 high-detectability params (`max_udp_payload_size`, `active_connection_id_limit`, `initial_max_streams_bidi` exact value, QPACK settings) are left at quinn defaults. So even the value-level QUIC fingerprint is non-Chrome.

The net is: **QUIC is a latent liability, correctly neutralized by `allow_http3: false`.** No action needed unless/until a target requires h3.

---

## 3. New external findings (2026), with sources

### 3.1 JA4-vs-UA cross-checking is now universal (confirms the Firefox leak is dangerous)

2026 industry sources converge: JA4+ is "the industry standard at Cloudflare, AWS, and VirusTotal, with universal adoption in 2026," and JA4 was *engineered specifically to survive Chrome's extension randomization* by sorting ciphers/extensions and stripping GREASE before hashing ([proxies.sx TLS guide](https://www.proxies.sx/use-cases/privacy/tls-fingerprint), [Auth0 JA4 signals](https://auth0.com/blog/strengthening-bot-detection-ja4-signals/), [WebDecoy JA4 guide](https://webdecoy.com/blog/ja4-fingerprinting-ai-scrapers-practical-guide/)). The key operational point for BO: because JA4 strips GREASE and sorts before hashing, **BO's per-handshake Fisher-Yates shuffle and BoringSSL GREASE injection are correctly invisible to JA4** — they do not destabilize BO's JA4 (good), but they also do *not* help against the Firefox-UA mismatch (the JA4 still reads "Chrome 147"). JA4+ explicitly differentiates "real browsers, automation tools, and fake browsers even when they spoof headers" — which is precisely the Firefox-UA-over-Chrome-TLS shape BO emits on Firefox-routed sites.

### 3.2 QUIC/HTTP-3 transport-parameter fingerprinting is moving into JA4+ in 2026

[proxies.sx HTTP/3 guide](https://www.proxies.sx/use-cases/privacy/http3-quic) and [Scrapfly H2/H3 guide](https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide) name the concrete high-detectability QUIC vectors: `max_idle_timeout`, `max_udp_payload_size`, `initial_max_streams_bidi`, `active_connection_id_limit`, plus HTTP/3 SETTINGS (`QPACK_MAX_TABLE_CAPACITY`, `QPACK_BLOCKED_STREAMS`, `MAX_FIELD_SECTION_SIZE`), and lower-level signals: **connection-ID length (0–20 bytes), CID generation algorithm, and rotation pattern**. HTTP/3 is now 30%+ of web traffic. "Major CDNs now include QUIC transport parameters in JA4+ fingerprinting" (timeline: Multipath QUIC 2025–2026, HTTP/3 extensions 2026–2027). This validates doc-39 §5.4's 2027 estimate and confirms BO's "h3 off" is the right posture — but it also means **if any 126-corpus site forces h3-only at the edge, BO will fall back to h2/h1 and look like "a Chrome that refuses QUIC,"** itself a faint signal as h3 adoption deepens.

### 3.3 Camoufox runs the *real* Firefox NSS stack — BO's Firefox class is strictly behind here

DeepWiki query on `daijro/camoufox`: Camoufox intercepts HTTP *headers* at `nsHttpHandler` (User-Agent / Accept-Language / Accept-Encoding via `MaskConfig`) and spoofs WebRTC ICE IPs via a `WebRTCIPManager`, but it does **not** modify TLS ClientHello or HTTP/2 framing — it uses the native Firefox NSS network stack. Therefore **Camoufox's JA4 and akamai-H2 are genuine Firefox**, perfectly coherent with its Firefox UA. BO's Firefox profile is the inverse: spoofed Firefox UA/headers over a genuine Chrome wire fingerprint. On any JA4-vs-UA-checking vendor, **Camoufox-Firefox wins and BO-Firefox loses.** This is a concrete competitive gap: BO's multi-profile routing is a moat *for Chrome/iOS* but a liability for the Firefox class until BO emits real Firefox wire bytes. (Strategically: BO should either (a) only route Firefox-UA to sites that demonstrably don't JA4-cross-check, or (b) build the Firefox TLS+H2 class. See fixes.)

### 3.4 Open-source reference for a Firefox-correct wire stack exists

[`sardanioss/httpcloak`](https://github.com/sardanioss/httpcloak) (Go) advertises "browser-identical TLS/HTTP2 fingerprinting … Chrome, Firefox, and Safari … JA3/JA4, Akamai fingerprint, header order … HTTP/1.1, HTTP/2, HTTP/3." Together with `lexiforest/curl-impersonate`'s `firefox_*` YAML signatures and `0x676e67/wreq-util`'s `src/emulate/profile/firefox/` modules (already the cited gold standard for BO's Chrome H2), there are at least three concrete, MIT/BSD-class references for the exact Firefox 135 cipher list, extension set/order, sigalgs, and H2 SETTINGS — i.e., the Firefox class is a *known, documented* target, not research. This materially lowers the effort estimate for fixing the §2.3 leak (the cipher/sigalg/curve/extension constants are transcribable from those references; the only real work is wiring a Firefox branch in `tls.rs::chrome_connector` + `h2_client.rs::handshake` and confirming boring2 can express Firefox's extension order).

### 3.5 GREASE / extension-shuffle posture is current and correct

The 2026 sources confirm Chrome shuffles extension order per-session and emits GREASE, and that JA4 strips both — meaning BO's current posture (`set_grease_enabled(true)` + per-handshake full Fisher-Yates) is *exactly* what real Chrome 2026 does and is JA4-stable. One subtlety worth a regression note: real Chrome's GREASE values appear in **multiple positions** (cipher list, extensions, supported_versions, supported_groups). BO relies on BoringSSL's `set_grease_enabled(true)` to inject all of them; this is the right primitive (BoringSSL is Chrome's own TLS library), so GREASE placement is correct by construction. No gap.

---

## 4. Where BO leaks at the network layer today (consolidated)

| # | Leak | Severity | Who catches it | Code locus |
|---|------|----------|----------------|------------|
| L1 | **Firefox-UA over Chrome TLS + Chrome H2** (cross-layer mismatch) | HIGH (on Firefox-routed sites) | Any JA4-vs-UA vendor: CF, AWS WAF, Akamai, DataDome, Kasada | `h2_client.rs:85` (no FF branch), `tls.rs:241` (branch only on device_class), `presets.rs:474` (aspirational `tls_impersonate`) |
| L2 | **No real ECH** (emit grease only) | LOW, rising | CF (post ECH-on-Free) — "Chrome with ECH disabled" signal | `tls.rs:389` |
| L3 | **QUIC uses rustls, non-Chrome transport params** | LATENT (masked by `allow_http3:false`) | Any QUIC-JA4 vendor — only if h3 enabled | `quic.rs:19-34` |
| L4 | **No JA4 / akamai_fingerprint baseline in tree** | NOT a runtime leak — a *verification* gap | n/a (risk: silent drift on boring2 bump) | `crates/net/tests/tls_fingerprint.rs` asserts only `t13d` prefix |
| L5 | **JA4T (TCP options) = Linux kernel defaults, not Chrome** | LOW today, rising | JA4T-adopting vendors (few in 2026) | `tokio::net::TcpStream` — no per-profile control |
| L6 | **SharedSession `accept_ch` state-bleed** (x-com) — *may* surface as a state-dependent observable wire shape | MEDIUM, unconfirmed | Twitter/X WAF | hypothesis per 39 §6.2; A/B unresolved |
| L7 | **Android curves == Desktop** (`CURVES_ANDROID = CURVES_DESKTOP`, `tls.rs:104`) — unverified that Chrome Android 147 emits identical PQ curve order | LOW | curve-order-sensitive Akamai/Kasada | `tls.rs:98-104` (comment flags "verify against fresh Pixel capture") |

Everything *not* in this table (Chrome/iOS TLS, Chrome/iOS H2, MLKEM768, ALPS, ECH-grease, cert compression, Fisher-Yates, WebRTC mDNS, GREASE) is verified correct and needs no work.

---

## 5. ECH / 2027 horizon

- **ECH at scale (L2):** CF flipped ECH-on for Free zones in late 2024; adoption is climbing. As >30% of HTTPS uses ECH, the outer SNI becomes useless for edge routing and vendors pivot to inner-handshake JA4 (decrypted inside their infra) + transport signals. BO emits ECH-*grease* (correct, matches real Chrome 124+), but real ECH (encrypted inner ClientHello from a DNS HTTPS record) is unimplemented. The boring2 primitive exists (`SSL_set1_ech_config_list` in recent boring2); the missing piece is a DNS HTTPS-record fetcher (`hickory-dns`). Until then BO looks like "Chrome with ECH off" to ECH-aware origins — a *small* signal, low priority for v0.1.0, in-scope for v0.2.0.
- **QUIC transport-param JA4+ (L3, §3.2):** confirmed moving into CDN JA4+ in 2026; BO's posture (h3 off) is correct. The forward plan when h3 becomes load-bearing: a deterministic Chrome transport-parameter set (NOT quinn defaults / randomization) AND a Chrome-byte QUIC ClientHello (which rustls cannot produce — would need boring2's QUIC API or a quinn-crypto-boring shim). Large; v0.3.0.
- **JA4T (L5):** TCP MSS/window/options-order. BO uses `tokio::net::TcpStream` = kernel defaults, no per-profile control. Few vendors consume JA4T in 2026; matching it needs kernel tuning or userspace TCP. v0.3.0+, watch-and-wait.

---

## 6. Ranked fix list (ROI order)

> Public-engine note: all wire-layer impersonation lives in public `crates/net` per `CLAUDE.md` ("HTTP/TLS: own stack in `crates/net/`"). None of these are per-vendor solver code, so all are **public-engine** fixes. Per-vendor *challenge solving* stays in `vendor_solvers`; nothing below is that.

### FIX-N1 — Capture JA4 + akamai_fingerprint baselines into tree (verification, not engineering)
**What:** Hit `tls.peet.ws/api/all` per profile, store `tls.ja4` and `http2.akamai_fingerprint` into `crates/net/tests/captures/<profile>/`, and upgrade `test_tls_fingerprint_peet` from `t13d`-prefix to full-string `assert_eq!`. Closes 39 §8.3/§8.4.
**Effort:** 0.5–1 day. **Impact:** flips 0 sites directly; converts "we believe TLS is correct" into machine-proof and arms the boring2-bump silent-drift guard. **Confidence:** high. **Public engine:** yes.

### FIX-N2 — Decide Firefox-routing policy NOW (cheap risk reduction for L1)
**What:** Until a real Firefox wire class exists (FIX-N3), gate Firefox-UA routing to ONLY sites empirically shown not to JA4-vs-UA cross-check; never route Firefox-UA to CF/AWS/Akamai/DataDome/Kasada-fronted sites (where §3.1 says the mismatch is caught). Implementable as a routing-table flag, no wire changes.
**Effort:** 0.5–1 day. **Impact:** prevents silent losses on any Firefox-routed site behind a JA4-checking vendor; protects the few current Firefox wins (adidas-class) from being a liability elsewhere. **Confidence:** medium-high (depends on which sites currently route Firefox). **Public engine:** yes.

### FIX-N3 — Build the real Firefox 135 TLS + HTTP/2 class (closes L1 properly)
**What:** Add a Firefox branch in `tls.rs::chrome_connector` (Firefox cipher list, extension set/order — Firefox does NOT Fisher-Yates, no ALPS, no ECH-grease, different sigalgs, different curve set; transcribe from `lexiforest/curl-impersonate` `firefox_*` YAML + `wreq-util` firefox profile + cross-check `httpcloak`), and a Firefox branch in `h2_client.rs::handshake` (`HEADER_TABLE_SIZE=65536, ENABLE_PUSH=0, INITIAL_WINDOW_SIZE=131072, MAX_FRAME_SIZE=16384`, pseudo-order `m,p,a,s`, no priority hint). Add a `firefox_h2_settings_match_firefox_reference` byte test parallel to the Chrome one, and a Firefox JA4 baseline. **Risk to verify:** boring2 must be able to express Firefox's extension order (it has `SSL_CTX_set_extension_permutation` — likely yes); if Firefox emits an extension boring2 can't, that's the hard part.
**Effort:** 1–2 weeks (most of it is reference transcription + boring2 expressivity validation + capture). **Impact:** makes BO-Firefox genuinely coherent → reaches Camoufox parity on the Firefox class and removes BO's single biggest wire leak; could flip Firefox-class-favorable sites (adidas, some amazon-*/disney-class WAFs that treat Firefox leniently) from "accidental win that's fragile" to "robust win," and unlock sites currently lost because the Firefox mismatch trips JA4-vs-UA. **Confidence:** medium (impact is real but exact site count needs the L4 sweep). **Public engine:** yes.

### FIX-N4 — Verify Chrome Android curve/PQ order against a fresh Pixel capture (closes L7)
**What:** `CURVES_ANDROID = CURVES_DESKTOP` (`tls.rs:104`) is asserted, not verified. Capture a real Pixel 9 Chrome 147 JA4 from `tls.peet.ws` and confirm MLKEM768 ordering matches; if Android differs, split the constant.
**Effort:** 0.5 day (needs a real Android device or BrowserStack). **Impact:** protects the `pixel_9_pro_chrome_148` profile from a latent curve-order tell on Akamai/Kasada. **Confidence:** medium. **Public engine:** yes.

### FIX-N5 — Resolve the x-com SharedSession `accept_ch` state-bleed A/B (L6)
**What:** Run the full 126-sweep twice — `HttpClient::shared` vs `HttpClient::new` — to confirm/deny whether process-wide `accept_ch` accumulation produces a state-dependent observable wire shape that x.com's WAF flags (39 §6.2 / Q3). If confirmed, scope `accept_ch` per-domain in `crates/net/src/cookies.rs`.
**Effort:** 0.5 day to run the A/B; 1–2 days to fix if confirmed. **Impact:** potentially flips x-com (currently 69B THIN-BODY mid-sweep vs 274KB isolated). **Confidence:** medium (hypothesis unconfirmed). **Public engine:** yes.

### FIX-N6 — Real ECH (L2) — defer to v0.2.0
**What:** Plumb `SSL_set1_ech_config_list` + a DNS HTTPS-record fetcher (`hickory-dns`); send a real encrypted inner ClientHello to ECH-enabled origins.
**Effort:** 1–2 weeks. **Impact:** removes the faint "Chrome with ECH off" signal; near-zero site flips in 2026, rising. **Confidence:** low (low current impact). **Public engine:** yes. **Defer.**

### FIX-N7 — Chrome-byte QUIC/HTTP-3 class (L3/L5) — defer to v0.3.0
**What:** Replace the rustls QUIC ClientHello with a Chrome-byte one (boring2 QUIC API or quinn-crypto-boring shim), set deterministic Chrome transport params (`max_udp_payload_size`, `active_connection_id_limit`, QPACK settings, CID length/rotation), then consider enabling h3 selectively.
**Effort:** 3+ weeks. **Impact:** none today (h3 off is correct); rises as h3 adoption deepens and JA4-QUIC spreads. **Confidence:** low (no current site needs it). **Public engine:** yes. **Watch-and-wait; do NOT enable h3 with the current rustls path.**

---

## 7. Sources

- `docs/releases/v0.1.0-parity/39_NETWORK_LAYER_FINGERPRINTING.md` (read in full) and its companion `23_TLS_HTTP_FINGERPRINT_REFERENCE.md`
- BO source: `crates/net/src/tls.rs`, `crates/net/src/h2_client.rs`, `crates/net/src/quic.rs`, `crates/net/Cargo.toml`, `crates/stealth/src/presets.rs`, `crates/stealth/src/profile.rs`
- [proxies.sx — TLS Fingerprinting Guide 2026 / JA4+](https://www.proxies.sx/use-cases/privacy/tls-fingerprint)
- [proxies.sx — HTTP/3 QUIC Fingerprinting Guide 2026](https://www.proxies.sx/use-cases/privacy/http3-quic)
- [Auth0 — Strengthening Bot Detection with JA4 Signals](https://auth0.com/blog/strengthening-bot-detection-ja4-signals/)
- [WebDecoy — JA4 Fingerprinting: Detect AI Scrapers by TLS](https://webdecoy.com/blog/ja4-fingerprinting-ai-scrapers-practical-guide/)
- [Scrapfly — HTTP/2 and HTTP/3 Fingerprinting](https://scrapfly.io/blog/posts/http2-http3-fingerprinting-guide)
- [Cloudflare — JA4 signals](https://blog.cloudflare.com/ja4-signals/)
- [FoxIO-LLC/ja4 — JA4 spec + technical_details/JA4.md](https://github.com/FoxIO-LLC/ja4)
- [sardanioss/httpcloak — Go Chrome/Firefox/Safari TLS+H2+H3 impersonation reference](https://github.com/sardanioss/httpcloak)
- [0x676e67/wreq-util — gold-standard Rust impersonation profiles](https://github.com/0x676e67/wreq-util)
- [lexiforest/curl-impersonate — per-browser TLS signatures](https://github.com/lexiforest/curl-impersonate)
- DeepWiki `daijro/camoufox` query (Camoufox uses native Firefox NSS for TLS/H2; spoofs only headers + WebRTC IPs)
- [tls.peet.ws/api/all](https://tls.peet.ws/api/all) — live JA3/JA4/akamai_fingerprint oracle
