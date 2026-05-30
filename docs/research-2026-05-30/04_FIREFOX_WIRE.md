# 04 — Firefox Wire Class: the firefox-profile weakness, and why the documented blocker is stale

**Cluster:** firefox-profile weakness — `reuters / zillow / wsj / airbnb / spotify / tripadvisor`
fail **only** the firefox profile; firefox is the worst BO profile with **14 v150-gaps**.

**Root cause (one sentence):** BO's `firefox_135_*` presets advertise a Firefox UA + Firefox
request headers, but the network layer branches **only on `device_class`** (Desktop / MobileAndroid /
MobileIOS) — never on `browser_name` / `tls_impersonate` — so a Firefox profile puts a **Chrome-147
BoringSSL ClientHello + Chrome H2 SETTINGS** on the wire. The result is a JA4 that is Chrome-class
(`t13d1516h2…`) under a `Firefox/135` UA: an internally contradictory identity that any vendor doing
a JA4↔UA cross-check (DataDome, Cloudflare, Akamai, AWS WAF) buckets as high-risk on sight.

**The decisive new finding (this doc):** the historically "documented blocker" — *"boring2 can't emit
an NSS-class ClientHello"* — is **STALE/WRONG for boring2 4.15.15**. Every Firefox primitive the wire
needs is already a first-class API in the pinned crate. This is a **build, not a research project**;
it was mis-scoped as 1–2 days of TLS-internals RE when it is closer to a half-day of wiring + one
`tls.peet.ws` validation loop.

---

## 1. Where the wire forks today (code-grounded)

### 1.1 The connector is Chrome-only and selected by `device_class`

`crates/net/src/tls.rs:239` — the one and only connector builder is `chrome_connector(profile)`. It
matches on `profile.device_class` for three arms only:

```
crates/net/src/tls.rs:248   let curves = match profile.device_class {
                              MobileAndroid => CURVES_ANDROID,
                              MobileIOS     => CURVES_SAFARI_IOS,
                              Desktop       => CURVES_DESKTOP,   // ← Firefox lands here
                            };
```

There is **no `browser_name` arm and no Firefox arm** anywhere in the file. A `firefox_135_macos`
profile has `device_class: DeviceClass::Desktop` (`crates/stealth/src/presets.rs:473`) so it falls
straight into the Chrome-147 desktop config:

- `CIPHER_LIST` — 15 Chrome ciphers in Chrome order (`tls.rs:60`)
- `SIGALGS_LIST` — Chrome 8-entry sigalgs (`tls.rs:79`)
- `CURVES_DESKTOP` — `X25519_MLKEM768, X25519, SECP256R1, SECP384R1` (`tls.rs:91`)
- `set_grease_enabled(true)` (`tls.rs:304`) — GREASE on
- `CHROME_EXTENSION_PERMUTATION` (16 exts) Fisher-Yates **shuffled per handshake** (`tls.rs:209,356`)
- Brotli cert compression (`tls.rs:318`)
- ECH-**GREASE** + Chrome ALPS H2 SETTINGS payload (`tls.rs:394,400`)

Every desktop caller routes through this: `lib.rs:280,399,1076,1198` all call
`tls::chrome_connector(profile)` unconditionally.

### 1.2 H2 is also `device_class`-only

`crates/net/src/h2_client.rs:85` — `handshake` branches `is_safari_ios = device_class == MobileIOS`;
the `else` arm is the Chrome arm and Firefox falls into it:

- SETTINGS order = Chrome 8-entry `1,2,3,4,5,6,8,9` (`h2_client.rs:120`)
- Values: `HEADER_TABLE_SIZE=65536`, `ENABLE_PUSH=false`, `INITIAL_STREAM_WINDOW=6291456`,
  `MAX_HEADER_LIST=262144` (`h2_client.rs:39-42`)
- pseudo-order **masp** (`:method,:authority,:scheme,:path`) (`h2_client.rs:98`)
- a HEADERS-frame stream-priority dependency: weight 255 (wire 256), exclusive=true, depends_on=0
  (`h2_client.rs:167`)

All four are Chrome tells; none match Firefox.

### 1.3 Only the headers layer is Firefox-correct

`crates/net/src/headers.rs` branches on `browser_name` and *does* emit a correct Firefox header set
(no `sec-ch-ua*`, no `priority`, Firefox `accept`, `accept-language` with `q=0.5`). So the
profile presents **Firefox headers + Firefox UA over a Chrome ClientHello + Chrome H2** — the worst
possible incoherence, because the cheap, JS-invisible TLS/H2 layer is exactly what these vendors
score first.

### 1.4 The presets themselves admit the gap

`crates/stealth/src/presets.rs:466-474` (firefox_135_macos):
> `tls_impersonate` "is set to `firefox_135` here as a forward-compatible string … the actual
> TLS-class swap is gated by Phase B.3 … Until B.3 lands, the network layer falls back to the
> chrome_147 cipher suite."

`tls_impersonate` is a **dead string** — grep shows it is read **nowhere** in `crates/net`. The only
consumer of the firefox-ness on the wire is `headers.rs` via `browser_name`.

---

## 2. What an authentic Firefox-135 wire class needs (exact spec)

Target JA4 (internal canonical, `docs/GAP_DEEP_ANALYSIS_2026_04_28.md:206`,
`RESEARCH_REQUIRED_2026_04_28.md:9`):

```
t13d1715h2_5b57614c22b0_3d5424432f57
 │ │ │ ││ │  │            └ sha256-trunc of sorted sigalgs
 │ │ │ ││ │  └ sha256-trunc of sorted ciphers (Firefox NSS cipher set)
 │ │ │ ││ └ ALPN = h2
 │ │ │ │└ 17 ciphers
 │ │ │ └ 15 extensions   ← Chrome (BO today) is "16h2 t13d1516…": DIFFERENT count
 │ │ └ d = no SNI-domain-in-JA4 marker (domain present)
 │ └ TLS 1.3
 └ t = TCP
```

Note the structural tell: **Firefox JA4 has 15 extensions / 17 ciphers; BO's Chrome JA4 has 16/15.**
A classifier need not decode anything — the count digits alone separate the classes.

### 2.1 TLS ClientHello — the Firefox-vs-Chrome deltas

| Layer | Chrome-147 (BO emits today) | Firefox-135 (Camoufox = authentic; target) |
|---|---|---|
| **GREASE** | yes (`set_grease_enabled(true)`) | **Firefox sends NO GREASE** in cipher/group lists (it does send a GREASE-style entry only in supported_versions — see note) |
| **Cipher list** | 15 BoringSSL ciphers, Chrome order | NSS 17-cipher set, NSS order (AES-128-GCM, CHACHA20, AES-256-GCM, then ECDHE-ECDSA/RSA pairs, then CBC + 3DES tail) |
| **supported_groups** | `MLKEM768, X25519, P-256, P-384` | `X25519MLKEM768?, X25519, P-256, P-384, P-521, **FFDHE2048, FFDHE3072**` (Firefox appends the two FFDHE groups — a hard Firefox signature) |
| **sigalgs** | Chrome 8-entry | NSS order incl. `ecdsa_secp521r1_sha512`, `rsa_pss_pss_*`, `rsa_pkcs1_sha1` tail |
| **delegated_credentials (0x22)** | **absent** in Chrome | **present** (sigalgs ext) — Firefox-only |
| **record_size_limit (0x1c)** | absent | **present**, value `0x4001` (16385) |
| **ECH (0xfe0d)** | ECH-**GREASE** | **real ECH** offering (grease only when no config) |
| **session_ticket (0x23)** | present | present |
| **extension order** | per-handshake Fisher-Yates shuffle | **FIXED order** every handshake (NSS does not shuffle) — like the Safari arm |
| **cert compression** | Brotli (2) | **Brotli + Zlib?** Firefox advertises `zlib, brotli, zstd` historically; FF 135 sends the `compress_certificate` ext with NSS's list — verify on capture |
| **ALPN** | `h2, http/1.1` | `h2, http/1.1` (same) |

> NSS GREASE note: NSS-class Firefox does **not** sprinkle GREASE across cipher/group/extension lists
> the way BoringSSL does. The visible "no-GREASE" shape is itself a Firefox tell. In boring2 this
> means calling `set_grease_enabled(false)` for the Firefox arm.

### 2.2 H2 SETTINGS — Firefox frame

Real Firefox H2 SETTINGS (per FoxIO/akamai H2-fingerprint references, the `mn…` Firefox class):

| Setting | Chrome (BO today) | Firefox-135 target |
|---|---|---|
| `HEADER_TABLE_SIZE (1)` | 65536 | **65536** |
| `ENABLE_PUSH (2)` | 0 | **(omitted — Firefox does not send ID2=0 in the same slot; sends fewer settings)** |
| `INITIAL_WINDOW_SIZE (4)` | 6291456 | **131072** (Firefox = 128 KiB) |
| `MAX_FRAME_SIZE (5)` | (declared, no value) | **16384** |
| `MAX_HEADER_LIST_SIZE (6)` | 262144 | **(Firefox omits)** |
| **`MAX_CONCURRENT_STREAMS (3)`** | (declared, no value) | not the same set |
| SETTINGS subset/order | `1,2,3,4,5,6,8,9` | Firefox's distinct subset/order — **keeps ID3 where Chrome's wire omission differs** |
| pseudo-header order | **m,a,s,p** (masp) | **m,p,a,s** (Firefox = `:method,:path,:authority,:scheme`) |
| WINDOW_UPDATE | Chrome connection-window math (Δ to 15663105) | Firefox connection-window increment (12517377 class) |
| stream priority | single HEADERS dependency weight 255 excl | **RFC 7540 priority tree** (Firefox builds a 5-node dependency tree of idle streams, weights 201/101/1/etc.) — NOT a single weight |

The pseudo-order (m,p,a,s) and the priority-tree are the two highest-signal H2 Firefox tells. The
`http2` wreq fork already exposes `headers_pseudo_order` + `settings_order` (used in `h2_client.rs`);
the priority **tree** is the one item that may exceed the current builder's `StreamDependency`
single-hint API and need verification.

---

## 3. CAN boring2 4.15.15 emit it? — YES (this reverses the documented blocker)

I read the pinned crate source directly. boring2 4.15.15
(`~/.cargo/registry/.../boring2-4.15.15/src/ssl/mod.rs`) **already exposes every primitive**:

| Firefox need | boring2 4.15.15 API | Source line |
|---|---|---|
| `delegated_credentials (0x22)` | `SslContextBuilder::set_delegated_credentials(&str sigalgs)` | `mod.rs:1885-1889` (`SSL_CTX_set_delegated_credentials`) |
| `record_size_limit (0x1c) = 0x4001` | `SslContextBuilder::set_record_size_limit(u16)` | `mod.rs:1879-1881` (`SSL_CTX_set_record_size_limit`) |
| FFDHE groups | `SslCurve::FFDHE2048` / `SslCurve::FFDHE3072` | `mod.rs:764,766` (`SSL_CURVE_DHE2048/3072`); README.md:13 "kDHE, ffdhe2048, ffdhe3072" |
| Real ECH offer | `SslRef::set_ech_config_list(&[u8])` (or grease-off + config) | `mod.rs:3635` |
| Disable GREASE | `set_grease_enabled(false)` | `mod.rs:1874` |
| Fixed (no-shuffle) ext order | `set_permute_extensions(false)` + `SSL_CTX_set_extension_permutation` with a Firefox order | `mod.rs:1967`; perm call already used at `tls.rs:367` |
| Firefox ext in perm table | `DELEGATED_CREDENTIAL`, `RECORD_SIZE_LIMIT` are in BoringSSL's permutation table | `mod.rs:603,606` (`BORING_SSLEXTENSION_PERMUTATION` includes both, indices 22 & 26) |
| Custom cipher/sigalg/curve lists | `set_cipher_list`, `set_sigalgs_list`, `set_curves` | `mod.rs:1393,2024` |
| compliance / version range | `set_min_proto_version`, `set_max_proto_version` | already used |

**Conclusion:** the boring2 layer needs **no fork, no alternative TLS lib, no raw ClientHello
injection** for the cipher/group/sigalg/extension surface. The "Phase B.3 blocker"
(`RESEARCH_REQUIRED_2026_04_28.md:9`, "requires reconfiguring boring2 to emit Firefox's
NSS-coherent ClientHello … 1-2 days … access to NSS source") was written against an earlier boring2
and is now **stale**: 4.15.15 ships `set_delegated_credentials` + `set_record_size_limit` + FFDHE
curves + ECH as plain builder methods.

### 3.1 The two genuine residual risks (verify on capture, don't assume)

1. **Exact byte-ordering of `record_size_limit` / `delegated_credentials` within the ext block.**
   boring2 places them via its fixed `BORING_SSLEXTENSION_PERMUTATION`. We supply a Firefox-ordered
   index list to `SSL_CTX_set_extension_permutation` (same mechanism as the Safari arm at
   `tls.rs:353-373`). If NSS's exact interleave can't be reproduced with the permutation indices
   alone (e.g. a Firefox ext boring2's table doesn't carry, or PADDING positioning), that single ext
   may need raw injection — but the *presence* and the *values* are already controllable.
2. **H2 priority tree.** The wreq `http2` fork's `StreamDependency` is a single HEADERS-frame hint
   (`h2_client.rs:167`). Firefox emits a 5-node idle-stream priority **tree** in separate PRIORITY
   frames before the request. If the fork can't send standalone PRIORITY frames, this is the one
   place that needs an `http2`-fork patch — but it is a *second-order* tell; the JA4 + pseudo-order +
   SETTINGS subset carry most of the discriminative weight and are all reachable today.

---

## 4. Proposed path (ROI-ranked)

### Path A (RECOMMENDED) — native boring2 Firefox arm, in-tree

Add a `is_firefox = profile.browser_name == "Firefox"` (or a new `WireClass` field) branch alongside
the existing `is_safari_ios` branch in `tls.rs::chrome_connector` and `h2_client.rs::handshake`. This
mirrors exactly the Safari-iOS arm that already exists and is the proven pattern in the codebase.

Concrete edits:

1. **`crates/stealth`** — stop overloading `device_class` for wire selection. Either:
   - add a `wire_class: WireClass { ChromeDesktop, ChromeAndroid, SafariIOS, FirefoxDesktop }` enum to
     `StealthProfile`, derived from `browser_name`+`device_class`; **or**
   - cheapest: branch on `profile.browser_name == "Firefox"` directly in the two net functions
     (the string is already populated, `presets.rs:428`).
2. **`crates/net/src/tls.rs`** (mirror the iOS arm):
   - `CIPHER_LIST_FIREFOX` (NSS 17-cipher order)
   - `SIGALGS_LIST_FIREFOX` (NSS order)
   - `CURVES_FIREFOX = [X25519, SECP256R1, SECP384R1, SECP521R1, FFDHE2048, FFDHE3072]`
     (+ MLKEM768 lead if FF135 PQ is on — verify)
   - `FIREFOX_EXTENSION_PERMUTATION` (fixed NSS order, no shuffle → `permutation = FIREFOX_…` not
     `shuffled_…`)
   - `builder.set_grease_enabled(false)` for the FF arm
   - `builder.set_record_size_limit(0x4001)`
   - `builder.set_delegated_credentials("ecdsa_secp256r1_sha256:ecdsa_secp384r1_sha384:…")`
   - `add_cert_compression_alg(Zlib)` + Brotli as FF advertises (verify)
   - in `configure_connection`: real ECH (or ECH-grease as FF does when no config) + **no Chrome ALPS
     payload** (Firefox does not send Chrome's ALPS H2 SETTINGS blob).
3. **`crates/net/src/h2_client.rs`** (mirror the iOS arm): Firefox SETTINGS subset/order,
   `INITIAL_WINDOW_SIZE=131072`, pseudo-order **m,p,a,s**, and either the priority tree (if the fork
   allows standalone PRIORITY frames) or — pragmatic fallback — omit the single Chrome priority hint
   (closer to Firefox than a Chrome-weighted one).
4. **Rename** `chrome_connector` → `tls_connector` (it is no longer Chrome-only) or add a thin
   `tls_connector` dispatcher. Update the 4 callers in `lib.rs`.
5. **Test/validate** with the existing pattern: a `firefox_emits_no_grease_record_version` unit test +
   a manual `tls.peet.ws/api/all` capture from a `firefox_135_macos` profile; assert JA4 ==
   `t13d1715h2_5b57614c22b0_3d5424432f57`.

**Effort:** ~0.5–1 day for the cipher/group/sigalg/ext/H2-SETTINGS surface (boring2 has the APIs);
+0.5 day iff the H2 priority tree needs an `http2`-fork patch. **Confidence: HIGH** that boring2 can
emit the ClientHello; **MEDIUM** on byte-perfect ext interleave (needs one capture loop).

### Path B (NOT recommended) — alternative TLS lib (rustls/NSS) for the Firefox arm only

Running a second TLS stack (e.g. rustls with a Firefox preset, or linking NSS) only for Firefox
profiles is far more invasive: a parallel handshake path, a second ALPN/H2 bridge, a second cert
store, and a maintenance fork. Given §3 shows boring2 already has the primitives, this is
**unjustified**. Keep one stack.

### Path C (NOT recommended for now) — raw ClientHello injection

boring2 has no public "emit these exact bytes" hook; doing this would require a boring fork to splice
a pre-serialized ClientHello, defeating the point of using boring2. Only fall back here if §3.1(1)
proves NSS's exact ext interleave is unreachable via the permutation API — and even then, prefer a
narrow boring patch over full injection.

---

## 5. Scope guard — what the Firefox wire does and does NOT fix

**Fixes (wire is the dominant signal):** the 6-site firefox-only cluster (reuters, zillow, wsj,
airbnb, spotify, tripadvisor) and the DataDome class (etsy/tripadvisor/leboncoin/wsj), where the
internal docs are explicit that **Firefox-NSS TLS is the *only* documented bypass** — DataDome scores
`Chromium-TLS=bot, Firefox-TLS=human` by default tenant weighting
(`GAP_DEEP_ANALYSIS_2026_04_28.md:147-149`). Estimated 4–8 sites flip
(`RESEARCH_REQUIRED_2026_04_28.md`, "Probably 4-8 sites flip").

**Does NOT fix:** the 5-of-7 ground-truth fails that are **JS-hydration/SPA-execution** gaps
(douyin/duolingo/ozon/adidas/wildberries render a 1.8–13 KB shell vs v150's 100 KB–1.5 MB). Those are
the ES-module/event-loop-budget gaps covered in the render-path docs, **independent of the wire** —
the wire already gets BO past the edge with a 200 + shell. Do not expect the Firefox arm to move those.
homedepot is Akamai sec-cpt (challenge-drain), also not a wire fix.

---

## Appendix — exact file:line ledger

- `crates/net/src/tls.rs:239,248-252` — connector branches only on `device_class`; Firefox→Desktop→Chrome
- `crates/net/src/tls.rs:304` (grease on), `:209,356` (Chrome 16-ext shuffle), `:318` (Brotli), `:394,400` (ECH-grease + Chrome ALPS)
- `crates/net/src/tls.rs:353-373` — Safari fixed-permutation pattern to copy for Firefox
- `crates/net/src/h2_client.rs:85,98,110-130,159-174` — H2 device_class-only; Chrome SETTINGS/masp/priority
- `crates/net/src/headers.rs` — the ONLY Firefox-correct layer (branches on `browser_name`)
- `crates/stealth/src/presets.rs:421-474` — firefox_135_macos; `:466-474` the "informational only" admission; `device_class: Desktop`, `tls_impersonate: "firefox_135"` (dead string)
- boring2 4.15.15 `src/ssl/mod.rs`: `:1879` set_record_size_limit, `:1885` set_delegated_credentials, `:764/766` FFDHE2048/3072, `:1874` set_grease_enabled, `:1967` set_permute_extensions, `:603/606` ext perm table carries DELEGATED_CREDENTIAL + RECORD_SIZE_LIMIT, `:3635` set_ech_config_list
- `crates/net/Cargo.toml:22-32` — boring2 pinned 4.15 (5.0-alpha removed the impersonation APIs — stay on 4.15, which is exactly the line that *has* the FF primitives)
- Internal: `docs/GAP_DEEP_ANALYSIS_2026_04_28.md:147-149,204-221` (Firefox-NSS = only DataDome bypass; +Fix4 → 110-115 PASS); `docs/RESEARCH_REQUIRED_2026_04_28.md` B.3-ext (the now-stale blocker writeup + entry points + JA4 target)

## Sources (external)
- [Making curl impersonate Firefox — lwthiker (NSS cipher/extension reference)](https://lwthiker.com/reversing/2022/02/17/curl-impersonate-firefox.html)
- [curl_cffi impersonate (delegated_credentials/record_size_limit are Firefox-only extra_fp)](https://curl-cffi.readthedocs.io/en/v0.11.2/impersonate.html)
- [Scrapfly JA3/JA4 fingerprint reference](https://scrapfly.io/web-scraping-tools/ja3-fingerprint?algo=ja4)
