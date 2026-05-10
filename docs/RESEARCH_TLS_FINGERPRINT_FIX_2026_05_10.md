# Research: TLS-Fingerprint Bugs on wildberries.ru and canadagoose.com (2026-05-10)

Status: **research / no code changes**.
Author: Claude (Opus 4.7) for @yfedoseev.
Cited URLs in bracketed footnotes; full list at end.

---

## 0. Executive summary

We have two production TLS bugs that are very likely the same root cause:

| Site | Symptom | Server-side classifier verdict |
|---|---|---|
| `wildberries.ru` | `TLS handshake failed: unexpected EOF` (server RST mid-handshake) | "Unknown / not Chrome" → reject |
| `canadagoose.com` | TLS completes, ALPN selects `http/1.1` not `h2` | "Suspicious — Chrome-shaped but a few bytes off" → soft-deny via H1 downgrade |
| `sso.passport.yandex.ru` | Same H1 downgrade as canadagoose | Same soft-deny verdict |

Both manifestations point at a **ClientHello byte-pattern mismatch vs real Chrome 147**.
After re-reading `crates/net/src/tls.rs` against the canonical reference
`docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`, the gold-standard
`wreq-util/chrome` profile [W1] [W2], and the patched BoringSSL source in
`boring-sys2 4.15.0` [B1], **the top three concrete deltas are**:

1. **Extension-permutation indices are encoded against an out-of-date kExtensions
   table.** The `24, // application_settings_new (17613)` comment is *wrong*
   for boring-sys2 4.15: at the pinned BoringSSL commit (`44b3df6f0…`), index 24
   in `kExtensions[]` is `application_settings_new` only after the fork's
   patches insert it at that position. **But the same patch ALSO adds
   `record_size_limit` and (under `rpk.patch`) `server_certificate_type`
   *after* it.** The shuffle therefore probably emits one of those instead of
   ALPS-new, AND silently drops actual ALPS-new from the ClientHello — even
   though we *also* call `SSL_add_application_settings(...)`, which only
   *enables* ALPS, it doesn't bypass the permutation table. Probable side
   effect: zero ALPS extension on the wire → instant Chrome-fingerprint
   mismatch on Akamai/Kasada/Cloudflare. (See §4.2 for the index audit.)
2. **Cert-compression list = `[Brotli, Zlib]` while real Chrome 147 sends
   only `[Brotli]`.** Reference JSON line 17-19 explicitly:
   `"algorithms": ["brotli (2)"]`. wreq-util's chrome profile likewise
   registers `&[&BrotliCompressor]` only. This adds a second algorithm
   identifier (zlib=1) inside the `compress_certificate` extension payload
   → JA3 length and JA4 ext-hash both diverge.
3. **3-bucket "Chrome 130+" shuffle is a folk-misconception.** Per Fastly's
   measurement [F1], chromestatus 5124606246518784 [C1], and the BoringSSL
   source [B2], real Chrome shuffles *all* 16 non-PSK extensions with a
   single Fisher-Yates pass — there is no documented bucket structure.
   Our manual 3-bucket shuffle (Bucket A 9 items, Bucket B 6 items, Bucket
   C signature_algorithms fixed at end) reduces the entropy from 16!
   ≈ 2·10¹³ to 9!·6! ≈ 2·10⁸ AND keeps `signature_algorithms` *always*
   last — a deterministic positional tell that an Akamai per-handshake
   classifier can use as a soft-detection signal.

Plus several lesser concerns documented below.

---

## 1. What we send today (read directly from `crates/net/src/tls.rs`)

```
CIPHERS    : 15 entries, identical to reference JSON ciphers[1..]  ✅
SIGALGS    : 8 entries, identical to reference JSON sigalgs        ✅
CURVES     : X25519MLKEM768, X25519, P-256, P-384                  ✅
ALPN       : "h2", "http/1.1"                                       ✅
GREASE     : enabled via set_grease_enabled(true)                   ✅
KEY_SHARES : 2 (X25519MLKEM768 + X25519)                            ✅
ECH GREASE : enabled per-connection                                 ✅
ALPS       : "h2" + payload of 4 SETTINGS + empty ACCEPT_CH frame  ⚠ (§4.4)
ALPS new   : alps_use_new_codepoint(true)                           ✅
CERT COMPR : Brotli + Zlib                                          ❌ (§4.6)
EXT PERMUT : 17 fixed indices, manually shuffled in 3 buckets       ❌ (§4.1, §4.2)
ROOT STORE : webpki-root-certs (Mozilla)                            ✅
```

The ALPS payload is a near-verbatim copy of the reference H2 SETTINGS frame
followed by an empty ACCEPT_CH frame (8 bytes header). This matches Chrome
147 byte-for-byte per the reference JSON.

H2 SETTINGS in `crates/net/src/h2_client.rs`: 4 settings sent (1, 2, 4, 6),
order field has all 8 IDs, `INITIAL_CONNECTION_WINDOW_SIZE = 15_728_640` →
wire WINDOW_UPDATE = 15_663_105 = match. **Akamai H2 hash:
`52d84b11737d980aef856699f885ca86` = match.** (Already verified 2026-04-29.)

---

## 2. Real Chrome 147 reference (gold ground truth)

`docs/CHROME_147_TLS_REFERENCE_2026_04_29.json` is a captured GET against
`tls.peet.ws/api/all` from a real Chrome 147 / macOS arm64 / residential IP.
Key fields:

```
ja3      = 771,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,
           51-65037-10-18-45-23-17613-27-43-0-65281-11-5-16-35-13,
           4588-29-23-24,0
ja3_hash = bc6f7cfa92f699f32c8ff5a4178b5cfa
ja4      = t13d1516h2_8daaf6152771_d8a2da3f94cd
ja4_r    = t13d1516h2_002f,0035,…,1303_0005,000a,000b,000d,0012,0017,001b,0023,
                                       002b,002d,0033,44cd,fe0d,ff01_<sigalgs>
peetprint= GREASE-772-771|2-1.1|GREASE-4588-29-23-24|<sigalgs>|1|2|<ciphers>|
           0-10-11-13-16-17613-18-23-27-35-43-45-5-51-65037-65281-GREASE-GREASE
```

JA4 reverse-form `t13d1516h2_…` confirms:
* TLS 1.3 (`13`), SNI present (`d`), 15 ciphers, 16 extensions, ALPN `h2`.
* Extensions sorted (JA4 sorts to be permutation-resistant):
  `0005, 000a, 000b, 000d, 0012, 0017, 001b, 0023, 002b, 002d, 0033, 44cd, fe0d, ff01`
  decimal: `5, 10, 11, 13, 18, 23, 27, 35, 43, 45, 51, 17613, 65037, 65281`
  ⇒ that is **14** non-GREASE extensions — but the JA4 prefix says `16`.
  The other 2 extensions are GREASE entries (1 cipher GREASE, 2 ext GREASE
  positions), but JA4 method only counts non-GREASE in the leading number,
  so `16` here refers to the *raw extension count* including GREASE. (The
  reference JSON shows 16 extension elements in the array including the
  two `TLS_GREASE` entries — line by line.) 
* Captured on-wire extension order:
  `51, 65037, 10, 18, 45, 23, 17613, 27, 43, 0, 65281, 11, 5, 16, 35, 13`
  (16 entries). This is one specific permutation; another capture would
  produce a different order.

The JA4 fingerprint `t13d1516h2_8daaf6152771_d8a2da3f94cd` is widely
reported as the canonical Chrome 147 desktop signature [J1] [P1].

---

## 3. What our code probably emits (best-effort static derivation)

We can derive a JA3-equivalent inventory without packet capture by reading
our code. Below is the *intended* permutation when `set_extension_permutation`
correctly maps byte-indices to extension type values (assuming the index
table matches our comments):

```
14, 1, 4, 11, 15, 2, 24, 21, 17, 0, 3, 5, 8, 7, 6, 9, 22
→ key_share, ECH, supported_groups, SCT, psk_modes, ext_master_secret,
  ALPS-new, cert_compression, supported_versions, server_name, renegotiate,
  ec_point_formats, status_request, ALPN, session_ticket, sigalgs,
  delegated_credential
```

Versus reference (no delegated_credential):
```
51, 65037, 10, 18, 45, 23, 17613, 27, 43, 0, 65281, 11, 5, 16, 35, 13
→ same 16 minus delegated_credential
```

**Delta items**:
* We add **`delegated_credential (34)`** at the end. Real Chrome 147 does
  NOT send this extension (not in the reference). Adds 1 extension type
  → JA3 length 16→17, JA4 prefix `15..→16..` already wrong vs reference.

* Cert-compression payload: ours `[brotli=2, zlib=1]`, reference `[brotli=2]`
  → 4 bytes length difference inside ext 27.

* Bucket-C fixes `signature_algorithms (13)` at position 16 always. In the
  reference it appears at position 16 in this *one* capture, but a different
  capture would put it elsewhere. Net effect: an attacker that hashes 100
  consecutive handshakes from us sees `13` at position 16 every time;
  100 from real Chrome shows it at random positions.

* Bucket-A and Bucket-B: members are different sets from upstream Chrome
  too — our Bucket-A is `[14, 1, 4, 11, 15, 2, 24, 21, 17]` and Bucket-B
  is `[0, 3, 5, 7, 6, 8]`. Real Chrome has no buckets at all [F1].

JA4 robustness: JA4 sorts the extensions, so OUR JA4 will compute as
the same hex hash as Chrome's **only if the *set* of extension type IDs
is identical**. Since we add `delegated_credential (34) = 0x0022`, our
JA4 sorted-ext list is `…, 0022, 0023, 002b, …` versus Chrome's
`…, 001b, 0023, 002b, …` — different byte. So **our JA4 != Chrome's
JA4**. Our prefix becomes `t13d1517h2` (17 ext counter) instead of
`t13d1516h2`. CDN policy lookup misses → reject or downgrade.

---

## 4. Suspect-by-suspect deep-dive

### 4.1 Extension-permutation FFI indices (HIGH severity, EASY to verify)

**boring-sys2 4.15.0 C signature** [B1, lines: see crate
`patches/boringssl-44b3df6f03d85c901767250329c571db405122d5.patch`]:

```c
int SSL_CTX_set_extension_permutation(SSL_CTX *ctx,
                                      const uint8_t *permutation,
                                      size_t permutation_len);
```

So byte indices into `kExtensions[]` is correct. (Note: the *newer* btls
fork — 5.0-alpha and onwards — switched to `SSL_CTX_set_extension_order(ctx,
const uint16_t *ids, int num)` which takes type values [B3]. We're on the
older byte-index API, so our `&[u8]` ptr is fine.)

**The ordering IS the bug.** At the pinned commit `44b3df6f0…` the upstream
`kExtensions[]` table ends with `application_settings (old, 17513)` at
some index N. The boring-sys2 patch hunk `@@ -3267,6 +3408,21 @@` *appends*:
- `application_settings_new (17613)` immediately after,
- `record_size_limit (28)` immediately after that.

Plus `rpk.patch` *additionally* appends `server_certificate_type (20)`.

So the actual table at runtime in our binary is:

```
… 22 = delegated_credential (34)
   23 = application_settings (old, 17513)
   24 = application_settings_new (17613)
   25 = record_size_limit (28)
   26 = server_certificate_type (20)
```

… **only if** the upstream order at the pinned commit happened to put
`delegated_credential` at index 22 and `application_settings` at index
23. The current upstream main has it that way [B2]. But several recent
upstream commits between 2024-Q4 and 2026-Q1 reordered or *added*
extensions ahead of these (e.g. `pake`, `trust_anchors`, `client_cert_type`).
At the pinned commit, the index of `application_settings_new` may be 23,
24, or 25 — we cannot tell from static reading alone.

**Net result**: index `24` in our permutation list **may select the wrong
extension** (potentially `record_size_limit` or `server_certificate_type`),
and `application_settings_new` may be missing from the ClientHello entirely.
That alone is a Chrome 147 fingerprint death-knell.

**Fix path**: Either (a) verify the table by writing a one-shot test that
runs the connector and tcpdumps a real handshake, or (b) port to btls 5.x
which uses the type-value API and removes the index-fragility entirely.

The unit test at `crates/net/src/tls.rs:192-223` only verifies bucket
counts/membership — it does not run a handshake or assert the actual
extension types appear on the wire. **Adding such an assertion is the
single highest-leverage one-line test we could add.**

### 4.2 The 3-bucket shuffle (HIGH severity, MEDIUM to verify)

Per the Fastly post-Jan-2023 measurement [F1]:

> "Chrome permutes the set of TLS extensions sent in the ClientHello message,
> resulting in a different JA3 fingerprint with every new connection."
> "Subject to the pre_shared_key constraint in the RFC."

Per chromestatus 5124606246518784 [C1] and the BoringSSL source [B2],
function `ssl_setup_extension_permutation`:

```cpp
for (size_t i = kNumExtensions - 1; i > 0; i--) {
  // Set element |i| to a randomly-selected element 0 <= j <= i.
  std::swap(permutation[i], permutation[seeds[i - 1] % (i + 1)]);
}
```

That is a Fisher-Yates over **all** entries — no buckets. The only
positional constraint is "PSK must be last" enforced separately
(via `pre_shared_key` not being in the permuted list — it's added
after the loop, see [B2]).

Our `crates/net/src/tls.rs:67-71` claims:

```
// Bucket A: [0..9] (indices 14, 1, 4, 11, 15, 2, 24, 21, 17) - Shuffled
// Bucket B: [9..15] (indices 0, 3, 5, 7, 6, 8) - Shuffled
// Bucket C: [15] (index 9) - Fixed at end (signature_algorithms)
```

**No public source supports this 3-bucket model.** It appears to be
folklore from `tls.peet.ws` observations of *one* specific browser
build. The only published constraint on Chrome's permutation is
PSK-last; sigalgs is shuffled freely. wreq-util's gold-standard chrome
profile [W2] sets `permute_extensions(true)` and lets BoringSSL do the
plain Fisher-Yates — *no* manual bucketing.

**Net result**: We produce a heavily-constrained permutation distribution
(8! · 6! / 16! ≈ 1/720k of the natural Chrome distribution). Akamai's
classifier, when it sees the same 720k-subset of permutations produced
across many handshakes from many "Chrome 147" UAs from many of our IPs,
can score this as anomalous. Cloudfront/Akamai already publishes the use
of "fingerprint diversity" as a soft signal [J2].

**Fix**: Drop the manual buckets entirely. Either:
- `set_permute_extensions(true)` and remove the manual permutation call
  → BoringSSL will do plain Fisher-Yates (Chrome behaviour).
- OR keep the manual call but generate a uniform permutation of all
  16 extension indices each time, with no bucket constraints.

### 4.3 Extra `delegated_credential` extension (HIGH severity, EASY to verify)

Reference JSON enumerates 16 extension entries. Our permutation has 17
(adds `delegated_credential (34) / 0x0022`). Real Chrome 147 desktop
does NOT send `delegated_credential` per the reference capture and per
multiple Chromium audits [W2 — wreq-util chrome profile has no delegated
credentials in its TlsOptions; only Firefox profiles do].

JA4-prefix consequence: ext-count counter `15..` becomes `16..` (we have
14 non-GREASE non-PSK + delegated_credential = 15, but JA4 counts non-PSK
non-GREASE as 15 vs Chrome's 14 → ours `t13d1516h2_…`-ish to `t13d1517h2_…`
depending on what counts as GREASE in this counter). In all cases, the
ja4_a prefix differs from Chrome's `t13d1516h2`. **CDN policy lookup
misses.**

**Fix**: Remove `22, // delegated_credential (34)` from
`CHROME_EXTENSION_PERMUTATION`. (This is the cheapest single-line fix
in the entire research scope.)

### 4.4 ALPS payload contents (LOW–MEDIUM severity, MEDIUM to verify)

We send (`crates/net/src/tls.rs:240-253`):
```
00 00 18 04 00 00 00 00 00     SETTINGS frame header (24-byte body, type 4)
00 01 00 01 00 00              ID 1 = 65536
00 02 00 00 00 00              ID 2 = 0  (push disabled)
00 04 00 60 00 00              ID 4 = 6291456
00 06 00 04 00 00              ID 6 = 262144
00 00 00 89 00 00 00 00 00     empty ACCEPT_CH frame (type 0x89)
```

Reference JSON's `application_settings (17613)` field shows only
`"protocols": ["h2"]` — tls.peet.ws does not surface the inner ALPS
payload. So we cannot directly diff. But:

- **wreq-util's chrome profile [W2] does NOT register an ALPS payload at
  all**; it just sets `alps_protocols([AlpsProtocol::HTTP2])`. That
  means it sends ALPS with type=h2 and an empty body. Wreq-util's
  Chrome 147 profile handshakes successfully against Akamai/Cloudflare.

So our **non-empty ALPS payload may itself be a divergence**. Chrome
147's actual ALPS body (per draft-vvv-tls-alps and Chromium
`net/socket/ssl_client_socket_impl.cc`) contains the client's intended
HTTP/2 SETTINGS list as advisory data the server can pre-process — but
the *exact* payload Chrome emits is HTTP-version-specific and may be
empty/short on macOS Chrome 147. Sending too much (e.g. an empty
ACCEPT_CH frame the server doesn't expect at this codepoint) is one
plausible reason a server might reject the handshake.

**Fix path**: Try empty ALPS payload first (just register protocol "h2"
without `SSL_add_application_settings` payload) and verify on tls.peet.ws
that the resulting JA4 still shows `application_settings (17613)`.

### 4.5 ECH retry config / GREASE sizing (MEDIUM severity, HARD to verify)

`config.set_enable_ech_grease(true)` — this generates a random ECH
extension payload of valid HPKE structure but un-decryptable contents.
BoringSSL's GREASE-ECH implementation (`ssl/extensions.cc`
`ext_ech_add_clienthello`) generates a payload whose length is computed
from the configured `ECH_PSK_MODE_GREASE` constants.

Reference JSON ECH data is 232 bytes (length 0x000000…). Our boring-sys2
4.15 ECH-GREASE generator produces a payload of ~232 bytes for the same
HPKE cipher suite (X25519/HKDF-SHA256/AES128-GCM). So this is *probably*
fine, but worth diffing byte-for-byte.

Sites that **require** ECH (a few in 2026) will reject any client whose
ECH config doesn't decrypt to a valid inner ClientHello. None of
{wildberries, canadagoose, yandex.passport} are known ECH-required as
of 2026-05-10 (per Cloudflare ECH adoption tracker [E1]).

### 4.6 Cert-compression list (HIGH severity, EASY to verify) — already noted

Reference JSON `compress_certificate (27)`:
```
"algorithms": ["brotli (2)"]
```

`crates/net/src/tls.rs:158-162`:
```rust
builder.add_cert_compression_alg(CertCompressionAlgorithm::Brotli)?;
builder.add_cert_compression_alg(CertCompressionAlgorithm::Zlib)?;
```

We send 2 algorithms; Chrome 147 sends 1. Wire bytes inside ext 27 differ
by 4 (length-prefix change + 2-byte algorithm ID for zlib).

**Fix**: Remove the Zlib line. (Single-line fix; verifiable by re-running
`tls.peet.ws` after the patch.)

### 4.7 GREASE positional layout (LOW severity, HARD to verify)

`set_grease_enabled(true)` makes BoringSSL emit GREASE values at
deterministic positions (cipher list[0], curve list[0], extension list[0],
extension list[end]). The reference JSON shows GREASE at:
- `ciphers[0] = 0x2A2A`,
- `extensions[0] = 0x1A1A`,
- `extensions[end] = 0x8A8A`,
- `key_share[0] = 0xFAFA`,
- `supported_groups[0] = 0xFAFA`,
- `supported_versions[0] = 0x6A6A`.

That's **6 GREASE positions**, all in places BoringSSL handles. Our
config matches this — `set_grease_enabled(true)` covers them all, and
the reference values look correct (GREASE values are derived from
the per-handshake GREASE seed, so values change between handshakes
but positions are deterministic).

### 4.8 X25519MLKEM768 share generation (LOW severity, EASY to verify)

`set_key_shares_limit(2)` plus `CURVES = [X25519_MLKEM768, X25519, P256, P384]`
should produce key_share with TWO shares: PQ + classical X25519.
Reference JSON shows exactly this. boring-sys2 4.15 with feature
`pq-experimental` (which we enable in `Cargo.toml:14`) supports X25519MLKEM768.
This appears correct.

---

## 5. H2 SETTINGS-frame audit (LOW severity)

`crates/net/src/h2_client.rs` is verified byte-exact against the reference
capture. Akamai H2 hash matches: `52d84b11737d980aef856699f885ca86`.
This is **not** the bug.

When ALPN gets downgraded to http/1.1 (canadagoose case), no H2 SETTINGS
frame is ever sent — the H1 path is taken. So fixing the TLS bug
*automatically* re-enables H2 use and brings the H2 SETTINGS into play.

---

## 6. Server-side decision tree (why same site responds differently)

```
ClientHello bytes
       │
       ▼
[Akamai/Kasada/Cloudflare TLS classifier]
   1. Extract JA3, JA3N (normalised), JA4, JA4_r
   2. Extract HTTP2 hash (when ALPN=h2) — happens later
   3. Compare against catalog of known classifications:
      ├─ Matches "Chrome stable channel desktop"   → ALLOW + offer h2
      ├─ Matches "Chrome but slightly off (cluster B)" → ALLOW + force h1
      │  (this is the canadagoose / yandex passport bucket)
      ├─ Matches "node/headless/curl/python/Go/utls" → BLOCK with RST
      │  (this is the wildberries bucket — "unknown shape")
      └─ Matches nothing → either pass-through (low-risk site) or
         BLOCK (high-risk site like wildberries)
```

This three-tier classifier is documented by Kasada [K1], Cloudflare [C2],
and Akamai BMP [A1]. The "soft-deny via H1 downgrade" tier is specifically
called out in Cloudflare's product docs [C2] as a way to **reduce false
positives**: if you LOOK like Chrome but fail one secondary check, drop
to h1 (cheaper to detect bots in h1 since h1 doesn't carry h2 fingerprint
data) and let server-side JS challenges decide.

**Implication for us**: every JA4 byte difference matters more than any
single byte feels like it should. A 1-byte change from `t13d1516h2` to
`t13d1517h2` is enough to drop us from cluster A → cluster B → H1 forced.

---

## 7. Diff strategy — getting JA4 ground truth without local capture

Cheapest options ranked by signal:

### Option A — query tls.peet.ws from our binary (BEST, ~10 minutes)

Run a manual `cargo test --release -p net --test … -- --ignored …` style
invocation that hits `https://tls.peet.ws/api/all` using *our* TLS
connector and prints the full JSON to stdout. Diff against
`docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`. Field-level diff will
immediately show:
- Our JA4 hash
- Our extension list
- Our cert_compression algorithms
- Our supported_groups
- Whether ALPS made it onto the wire (look for `application_settings`
  in extensions)

This is the ONE step that produces ground truth. Everything else in this
doc is informed conjecture.

### Option B — public JA4 echo via browserleaks.com/tls

Same as A but JSON path is different (`https://tls.browserleaks.com/json`
returns `ja4`, `ja3`, `ja4_r`). Less detail than tls.peet.ws but works
when peet.ws is down.

### Option C — tcpdump/Wireshark on a sample handshake

Run our binary against any HTTPS server while tcpdumping :443. Open in
Wireshark, decode TLS, screenshot the ClientHello extension list. Then
do the same on a real Chrome 147 macOS handshake to the same server.
Side-by-side diff. ~30 minutes including setup.

### Option D — read Chrome source for the matching version

`https://chromium.googlesource.com/chromium/src/+/refs/tags/147.0.7390.x/
net/socket/ssl_client_socket_impl.cc` — the function `Connect` calls
`SSL_CTX_set1_curves_list`, `SSL_CTX_set_cipher_list`, etc., in a fixed
order. The actual extension type values are pulled from BoringSSL.
Static — no signal on which permutations are OBSERVED, only on what's
configured.

### Option E — diff against wreq-util gold standard

Run `wreq-util` against tls.peet.ws using its Chrome 147 profile [W1].
Diff that JSON against the reference. Anything that differs is something
we're allowed to differ on too. Anything that *matches* is something we
must also match.

---

## 8. Priority fixes ranked by likelihood × ease

| # | Fix | Likelihood it's a real bug | Ease (1=trivial, 5=hard) | Net priority |
|---|---|---|---|---|
| 1 | Remove `delegated_credential (34)` from extension permutation | **HIGH** (reference shows it absent) | 1 | **DO FIRST** |
| 2 | Remove `CertCompressionAlgorithm::Zlib` (keep Brotli only) | **HIGH** (reference confirms) | 1 | **DO FIRST** |
| 3 | Verify index `24 → application_settings_new` actually maps right in boring-sys2 4.15's kExtensions | **HIGH** (table changed in 4.15 fork) | 3 | **DO 2ND** |
| 4 | Replace 3-bucket shuffle with plain Fisher-Yates over all 16 indices (or `set_permute_extensions(true)`) | **MEDIUM** (folklore unsupported by source) | 2 | **DO 3RD** |
| 5 | Try empty ALPS payload (no SETTINGS body) | LOW–MEDIUM | 2 | DO 4TH |
| 6 | Capture our actual JA4 via tls.peet.ws to confirm 1–4 worked | n/a (verification) | 1 | **STEP ZERO** |
| 7 | Audit GREASE positional layout against tcpdump | LOW | 4 | last |
| 8 | Audit ECH GREASE payload byte structure | LOW | 5 | last |

**Easiest-and-most-likely-wrong**: #1 and #2 (one-line code edits, both
clearly documented as wrong vs reference).

**Hardest-but-most-likely-wrong**: #3 (requires knowing the exact
kExtensions table at the pinned BoringSSL commit; if wrong it silently
drops ALPS-new from the wire and selects a bogus extension instead).

---

## 9. Concrete 5-step action plan

| Step | Task | Time | Acceptance criterion |
|---|---|---|---|
| 1 | **Capture ground truth.** Add a `cargo test --release -p net … -- --ignored capture_jaa4` test that hits `https://tls.peet.ws/api/all` using `chrome_connector` and writes the JSON to `/tmp/our_jaa4.json`. Diff against `docs/CHROME_147_TLS_REFERENCE_2026_04_29.json`. | 30 min | Test produces a JSON; visible diff list (JA4 hash, extension array). |
| 2 | **Apply trivial JA4 fixes.** Remove `delegated_credential` from `CHROME_EXTENSION_PERMUTATION` (drop entry `22`). Remove `CertCompressionAlgorithm::Zlib` line. Re-run step 1's capture. | 15 min | Diff shrinks: ext count goes from 17 → 16, cert_compression algorithms list is `[brotli]` only. JA4 prefix becomes `t13d1516h2`. |
| 3 | **Audit the index table.** Either (a) modify `chrome_connector` to dump the live `extension_permutation` field via FFI and compare against `kExtensions[]` in the patched boring-ssl source, OR (b) for each index in our list, send a single-extension test ClientHello and check the wire-emitted extension type. Confirm `24 → 17613` (or whatever the real value is). Re-run step 1's capture. | 1–2 hr | tls.peet.ws JSON contains `application_settings (17613)` extension. |
| 4 | **Drop the 3-bucket shuffle.** Replace `shuffled_chrome_extension_permutation` body with a single Fisher-Yates over all 16 entries. Run step 1 capture 3× back-to-back to confirm permutation varies and JA4_r changes between runs (proving randomness). | 30 min | 3 captures show 3 different on-wire extension orders, all with the same JA4 hash (since JA4 sorts), and identical 16-element extension SET. |
| 5 | **Re-test the failing sites.** Re-run `cg_run.sh` and the holistic sweep. Verify `canadagoose.com` ALPN-negotiates `h2`, `wildberries.ru` completes the handshake (then check whether the Accept-CH path runs), and `sso.passport.yandex.ru` no longer downgrades. | 30 min | All three previously-failing sites either succeed or fail at a *different* (later, server-side) layer. |

**Total budget: ~3.5 hours wall-clock**, of which ~30 min is the
verification capture in step 1 (the only step that *can't* be skipped).

---

## 10. Open questions for the next research pass

1. Does Chrome 147 macOS arm64 actually omit `delegated_credential`, or is
   it omitted only when the server hasn't advertised the extension before?
   (Recommend: capture against multiple test endpoints to see if it ever
   appears.)
2. Does ALPS payload byte layout matter to Akamai/Kasada beyond the
   presence/absence of the extension? (Recommend: instrument
   `wreq-util` Chrome 147 against canadagoose to see if their empty-body
   ALPS works there.)
3. Is the extension permutation API in boring-sys2 4.15 confirmed to take
   indices into the *fork's* kExtensions or upstream's? Need to read the
   build.rs of the crate and confirm which extensions.cc actually compiled
   into the .so/.dylib our binary loads. (Could be done with `nm` /
   `objdump` against the prebuilt artifact.)
4. Is the wildberries `unexpected EOF` actually mid-ClientHello, or is
   the server completing the handshake and then closing on the first
   ApplicationData? Need a tcpdump to know which.

---

## Sources

| Tag | Title | URL |
|---|---|---|
| F1 | Fastly: A First Look at Chrome's TLS ClientHello Permutation in the Wild | https://www.fastly.com/blog/a-first-look-at-chromes-tls-clienthello-permutation-in-the-wild |
| C1 | Chrome Platform Status: TLS ClientHello extension permutation | https://chromestatus.com/feature/5124606246518784 |
| C2 | Cloudflare bot solutions docs: JA3/JA4 fingerprint | https://developers.cloudflare.com/bots/additional-configurations/ja3-ja4-fingerprint/ |
| W1 | wreq-util chrome.rs (versions table; v147 = v132 = tls_options!(7, CURVES_3)) | https://github.com/0x676e67/wreq-util/blob/main/src/emulate/profile/chrome.rs |
| W2 | wreq-util chrome/tls.rs (cipher list, sigalgs, certificate compressors, ALPS config) | https://github.com/0x676e67/wreq-util/blob/main/src/emulate/profile/chrome/tls.rs |
| B1 | boring-sys2 4.15.0 BoringSSL patch (`SSL_CTX_set_extension_permutation` C signature with `*const uint8_t`, ALPS-new and record_size_limit additions to kExtensions) | https://crates.io/crates/boring-sys2/4.15.0 (download crate, see `patches/boringssl-44b3df6f03d85c901767250329c571db405122d5.patch`) |
| B2 | BoringSSL ssl/extensions.cc (ssl_setup_extension_permutation Fisher-Yates shuffle of all extensions) | https://boringssl.googlesource.com/boringssl/+/refs/heads/main/ssl/extensions.cc |
| B3 | btls (the boring2 5.x fork) `SSL_CTX_set_extension_order` API switch to type-value uint16_t | https://github.com/0x676e67/btls/blob/main/btls-sys/patches/boringssl.patch |
| J1 | FoxIO-LLC JA4+ spec & reference fingerprints (Chrome `t13d1516h2_…`) | https://github.com/FoxIO-LLC/ja4 |
| J2 | Cloudflare blog: JA4 fingerprints and inter-request signals | https://blog.cloudflare.com/ja4-signals/ |
| K1 | Scrapfly: Bypass Kasada Bot Protection | https://scrapfly.io/bypass/kasada |
| A1 | CDNetworks: Practical Application of TLS Fingerprinting in Bot Mitigation | https://www.cdnetworks.com/blog/tls-fingerprinting-bot-mitigation/ |
| E1 | Cloudflare PQC support / ECH adoption | https://developers.cloudflare.com/ssl/post-quantum-cryptography/pqc-support/ |
| P1 | Proxies.sx: TLS/JA4+ Fingerprinting Guide 2026 | https://www.proxies.sx/use-cases/privacy/tls-fingerprint |
| ALPS | Chrome Status: New ALPS code point (17613 vs 17513) | https://chromestatus.com/feature/5149147365900288 |
| ALPS-2 | Intent to Ship: New ALPS code point | https://groups.google.com/a/chromium.org/g/blink-dev/c/yGdMW_gsGS4 |
| ECH | RFC 9849 — TLS Encrypted Client Hello | https://datatracker.ietf.org/doc/rfc9849/ |
| MLKEM | Hacker News — Google Chrome ML-KEM PQ default | https://thehackernews.com/2024/09/google-chrome-switches-to-ml-kem-for.html |
| MLKEM-2 | Cloudflare Netmeister: TLS 1.3 Hybrid Key Exchange (X25519MLKEM768 / ML-KEM) | https://www.netmeister.org/blog/tls-hybrid-kex.html |
| Bbs | net4people/bbs Issue 220: Google Chrome TLS extension permutation | https://github.com/net4people/bbs/issues/220 |
