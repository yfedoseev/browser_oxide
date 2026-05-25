# 23 — TLS + HTTP fingerprint reference

**Audience:** anyone editing `crates/net/src/tls.rs`, `crates/net/src/h2_client.rs`, or `crates/net/src/headers.rs`; anyone bumping a Chrome / Safari / Firefox major version; anyone proposing a new TLS impersonate codename for a new stealth profile; anyone debugging a "the WAF rejected us and the JS hadn't even run" failure mode.

**One-paragraph thesis:** BO's byte-perfect Chrome TLS ClientHello + HTTP/2 SETTINGS frame via `boring2 4.15` (Cloudflare's BoringSSL fork) is one of the project's core differentiators (per `12_COMPETITIVE_LANDSCAPE.md` §2.2). This doc is the single reference for what wire bytes BO emits per profile, how those bytes were verified against a real Chrome capture, how to refresh them when Chrome bumps a major, and what to look for when a network-layer failure is suspected (TLS or H2 reject before the JS runtime even runs). Read this before touching anything in `crates/net/`; cross-link to `19_PROFILE_EXPANSION_PLAN.md` when adding a new profile that needs a new TLS branch and `11_PER_PROFILE_STRATEGY.md` §7 for the per-profile TLS deltas.

---

## 1. The TLS stack — boring2 + Cloudflare BoringSSL fork

### 1.1 Why boring2 not rustls

`rustls 0.23` is the modern, memory-safe TLS library every Rust shop reaches for first. We do NOT use it for the outbound stealth-client path because:

1. **rustls does not expose ClientHello extension ordering byte-for-byte.** rustls emits extensions in a fixed internal order that does not match Chrome. There is no public API for per-handshake extension shuffling. Anti-bot vendors (Cloudflare, Akamai, Kasada) fingerprint the JA3/JA4 hash, which is computed from extension order; using rustls produces a JA4 that's "the Rust rustls JA4" — a signal far more anomalous than even Linux Chrome.
2. **rustls does not support the Chrome 131+ MLKEM768 post-quantum key share** in the order Chrome emits it (X25519MLKEM768 first, then X25519). It can negotiate MLKEM but not as the lead curve.
3. **rustls does not implement BoringSSL's per-extension cert compression (Brotli)** that Chrome desktop/Android use, and does not implement Zlib cert compression that Safari iOS uses.

We use rustls only inside the `quic` module (`crates/net/src/quic.rs:18-22`) for HTTP/3, where we set the ALPN to `h3` — and even there, HTTP/3 is `allow_http3: false` by default on every preset (`crates/stealth/src/presets.rs:887+`, `crates/stealth/src/profile.rs:134-146`) because vanilla `quinn-proto 0.11` randomizes transport_parameters per handshake (worse fingerprint than not speaking h3 at all).

### 1.2 boring2 4.15.15 (pinned)

Per `crates/net/Cargo.toml:28-30`:

```toml
boring2 = { version = "4.15", features = ["pq-experimental", "cert-compression"] }
boring-sys2 = "4.15"
tokio-boring2 = "4.15"
```

Resolved to `4.15.15` per `Cargo.lock:280-293`.

The pin is load-bearing: `boring2 5.0-alpha` removed the Chrome-impersonation APIs we rely on. Specifically (per the Cargo.toml comment at lines 23-30):
- `CertCompressionAlgorithm` (we need this for Brotli desktop + Zlib Safari)
- `SslCurve` (we set the curve list explicitly per profile)
- `SSL_CTX_set_extension_permutation` (the BoringSSL syscall we use to install our per-handshake Fisher-Yates shuffle — `tls.rs:362-367`)
- The `set_curves` builder method
- The `cert-compression` / `pq-experimental` cargo features (gone in 5.0; feature list is now fips, legacy-compat-deprecated, prefix-symbols, underscore-wildcards only)

Cloudflare's 4.x line is what carries those impersonation-specific patches. We stay on stable 4.x until upstream re-adds them or we vendor our own boring fork.

### 1.3 License compatibility

BoringSSL (and therefore boring2) is OpenSSL-style licensed — fits our MIT/Apache-2.0 + no-GPL/LGPL/AGPL policy mechanically enforced by `deny.toml` per `CLAUDE.md`. Neither `boring2`, `boring-sys2`, nor `tokio-boring2` appears in `deny.toml`'s MPL/copyleft exception list — they pass the standard license whitelist without per-crate carve-out.

### 1.4 The `tls_impersonate` codename → BoringSSL config mapping

The `tls_impersonate` field on `StealthProfile` (a `String`, per `crates/stealth/src/profile.rs:108`) is the *informational* label of the wire bytes we send. The actual branch in code is on `profile.device_class` (`DeviceClass::Desktop` / `MobileAndroid` / `MobileIOS`), not on the string. Current mapping per `crates/net/src/tls.rs:233-369`:

| `tls_impersonate` string | `device_class` | Branch in `chrome_connector()` | Verified-real reference |
|---|---|---|---|
| `chrome_147` (desktop Chrome) | `Desktop` | shared Chrome path: `CIPHER_LIST` + `SIGALGS_LIST` + `CURVES_DESKTOP` + Fisher-Yates extension shuffle + Brotli cert compression + ECH GREASE + ALPS | `lexiforest/curl-impersonate chrome_147.*_macos` |
| `chrome_147_android` (Pixel Chrome) | `MobileAndroid` | same Chrome path with `CURVES_ANDROID` (currently == `CURVES_DESKTOP`; see `tls.rs:99-104` for the verify-against-fresh-Pixel-capture caveat) | `lexiforest chrome_131.0.6778.81_android` (Android lagging — verify) |
| `safari_18_ios` (iPhone Safari) | `MobileIOS` | `CIPHER_LIST_SAFARI_IOS` (20 ciphers incl. 3DES) + `SIGALGS_LIST_SAFARI_IOS` (10 incl. duplicated Apple bug) + `CURVES_SAFARI_IOS` (no PQ, adds P-521) + FIXED extension order (no Fisher-Yates) + Zlib cert compression + NO_TICKET + skip ECH GREASE + skip ALPS | `lexiforest safari_18.0_iOS.yaml` |
| `firefox_135` (Firefox desktop) | `Desktop` | currently same as `chrome_147` desktop (real Gecko/NSS TLS is deferred — `presets.rs:457-463` documents the gap) | none yet — Phase B.3 future work |

**The intentional UA-vs-TLS-label split.** Real Chrome's TLS ClientHello is version-stable across majors (last change was Chrome 131 MLKEM768 rollout); the bytes Chrome 148 puts on the wire are identical to a Chrome 147 capture. `crates/net/src/tls.rs:22-57` documents this in full and `tls_fingerprint_vectors_no_silent_drift` (`tls.rs:476-553`) machine-checks the coherence:
- `pub const TLS_CHROME_MAJOR: u32 = 147;` — the version whose verified-real ClientHello these constants reproduce byte-exact
- `pub const UA_CHROME_MAJOR: u32 = 148;` — the version every desktop Chrome preset's UA advertises

JA4 cannot encode the Chrome version (JA4 = TLS version + sorted cipher/extension counts + ALPN + sorted sigalgs — none of those differ 147↔148). A vendor's "JA4-vs-UA cross-check" verifies the JA4 corresponds to *a Chrome consistent with the UA family* — it cannot, even in principle, distinguish a 147-vs-148 label difference.

---

## 2. JA3 vs JA4 — the two TLS fingerprints

### 2.1 JA3 (legacy, MD5-based)

Spec: `MD5("TLS_VERSION,CIPHER_LIST,EXTENSION_LIST,SUPPORTED_GROUPS,EC_POINT_FORMATS")`, each list comma-joined, each value a decimal IANA codepoint. Output: 32 hex characters.

JA3 is the older fingerprint that powered the first generation of TLS-based bot detection (~2017-2022). It's deterministic: a fixed cipher list + extension list produces a fixed JA3. That's also its weakness — vendors quickly built signature databases, and every modern stealth client copy-pasted the same Chrome JA3, making the JA3 itself a useless discriminator.

JA3's failure modes that JA4 fixes:
- Hash collisions on minor list reorderings (MD5 hides the structure)
- No version separation between JA3-of-TLS-1.2 and JA3-of-TLS-1.3
- No support for QUIC, no support for HTTP-layer signals

Real Chrome 147 JA3 (from a 2026-04-29 Playwright capture against `tls.peet.ws/api/all`):
- The exact MD5 hash changes per handshake because Chrome's Fisher-Yates extension shuffle randomizes extension order per connection — `771,4865-4866-...,17513-65281-...,29-23-24,0` would be the deterministic JA3 string; the MD5 of THAT is stable.

**Status in BO:** we don't compute or publish JA3 — we only target the JA4 family. The JA3 of our handshakes is whatever falls out of the JA4-correct configuration. There is no JA3 test gate.

### 2.2 JA4 (FoxIO 2023+)

Spec (from `https://github.com/FoxIO-LLC/ja4`): JA4 is a *family* of fingerprints (`JA4`, `JA4S`, `JA4H`, `JA4X`, `JA4T`, `JA4L`, `JA4SSH`). The TLS ClientHello one is just "JA4":

```
JA4 = "t" + tls_version_2 + sni_d/i + cipher_count_2 + ext_count_2 + alpn_2 + "_" + cipher_hash_12 + "_" + ext_sigalg_hash_12
```

Concrete decode for the format string `t13d1516h2_<cipher_hash>_<sigalg_hash>`:
- `t` — TCP (`q` for QUIC)
- `13` — TLS 1.3
- `d` — SNI present, domain (vs `i` IP-literal)
- `15` — 15 cipher suites in the offered list (Chrome desktop emits 15; iOS Safari emits 20)
- `16` — 16 extensions in the offered list (Chrome desktop emits 16; iOS Safari emits 13)
- `h2` — first ALPN protocol is `h2`
- `cipher_hash` — first 12 hex of SHA256 of the sorted-ascending IANA cipher codepoints
- `ext_sigalg_hash` — first 12 hex of SHA256 of `<sorted_ext_codepoints>_<sigalg_codepoints_in_order>`

**Key property:** JA4 is intentionally collision-resistant on the metadata (TLS-version + counts + ALPN are plain-text) and hash-based on the bag-of-codepoints. Extension ORDER does NOT affect JA4 (because the codepoints are sorted before hashing), but extension COUNT does. That's why our Fisher-Yates shuffle (`tls.rs:222-228`) doesn't break JA4 stability — the count stays 16, the codepoints stay the same, only their order on the wire varies. Real Chrome shuffles for the same reason; the shuffle is itself a positive signal (vendors look for the per-handshake variability and flag clients whose extensions are always in the same order).

### 2.3 What real Chrome 148 emits (target)

From a 2026-04-29 Playwright capture against `tls.peet.ws/api/all` (referenced at `crates/net/src/h2_client.rs:7-13`):

- Cipher count: 15 (per `CIPHER_LIST` at `tls.rs:60-76`)
- Extension count: 16 (per `CHROME_EXTENSION_PERMUTATION.len() == 16`, asserted at `tls.rs:516-520`)
- ALPN: `h2,http/1.1` (`tls.rs:186`) — first ALPN → `h2`
- TLS version negotiated: 1.3
- SNI: present, domain → `d`

Expected JA4 prefix: `t13d1516h2_*_*` — the first 13 characters are stable; the two 12-hex hashes are stable as long as the cipher / sigalg / extension *set* doesn't change.

### 2.4 What BO emits today

**We do NOT have a current published JA4 in the repo or in `~/projects/browser_oxide_internal/benchmarks/baselines/2026-05-24/`.** The closest in-tree evidence:
- The Chrome cipher list constant (`tls.rs:60-76`) — 15 entries, in the order matching the verified-real Chrome 147 reference
- `CHROME_EXTENSION_PERMUTATION.len() == 16` asserted at `tls.rs:516-520`
- ALPN `\x02h2\x08http/1.1` at `tls.rs:186` (length-prefixed list: `h2` then `http/1.1`)

**The verification harness that would publish this number** is the network-gated test `test_tls_fingerprint_peet` at `crates/net/tests/tls_fingerprint.rs:1-22`. It connects to `https://tls.peet.ws/api/all` with the `chrome_148_ru` profile and asserts the JA4 starts with `t13d`. The test is `#[ignore]` because it requires internet; per `CLAUDE.md` "Network tests are `#[ignore]`. They require internet and live target sites. Run with `--ignored` locally only."

**The v0.1.0 acceptance bar (§9) requires** that we publish the actual JA4 strings from a 4-profile (eventually 6-profile) sweep of `tls.peet.ws/api/all` and check them into this doc.

### 2.5 JA4H — the HTTP-layer fingerprint

JA4H is the HTTP-layer member of the JA4 family — it hashes header order, cookie names, language, method, and version. The spec is documented at `crates/net/src/ja4h.rs:14-26`:

```
JA4H = {method2}{ver2}{c|n}{r|n}{hdr_count2}{lang4}_{hdr_hash12}_{ck_hash12}_{ck_val_hash12}
```

We compute JA4H for unit-test regression (`crates/net/src/ja4h.rs:128-240`) — locked-in baselines per profile that fail loudly if the header order or cookie shape drifts. The computer is `#[cfg(test)]`-gated because JA4H is patent-pending under FoxIO License 1.1 (non-commercial); shipping the function in a release binary would violate the license. See `crates/net/src/LICENSE-NOTE.md`.

---

## 3. HTTP/2 fingerprint (Akamai H2 hash + pseudo-header order)

The HTTP/2 fingerprint is `<SETTINGS_in_order>|<WINDOW_UPDATE>|<PRIORITY>|<PSEUDO_HEADER_ORDER>`. Akamai publishes a hash form (md5) for telemetry; the open-source equivalent is the format used by `tls.peet.ws` (the "akamai_fingerprint" field).

### 3.1 Real Chrome 147 (verified target)

From the 2026-04-29 Playwright capture at `tls.peet.ws/api/all`, documented at `crates/net/src/h2_client.rs:10-13`:

```text
akamai_fingerprint: "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p"
priority: { weight: 256, depends_on: 0, exclusive: 1 }
```

Decoded:
- `1:65536` — SETTINGS_HEADER_TABLE_SIZE = 65,536
- `2:0` — SETTINGS_ENABLE_PUSH = 0
- `4:6291456` — SETTINGS_INITIAL_WINDOW_SIZE = 6,291,456 (6 MB)
- `6:262144` — SETTINGS_MAX_HEADER_LIST_SIZE = 262,144 (256 KB)
- `15663105` — wire delta for WINDOW_UPDATE = 15,728,640 (target) − 65,535 (default) = 15,663,105
- `0` — no PRIORITY frame (Chrome 147 sends priority on HEADERS only, not a separate PRIORITY frame)
- `m,a,s,p` — pseudo-header order: `:method`, `:authority`, `:scheme`, `:path`

Plus, on the first HEADERS frame, Chrome sends a priority hint: `weight=255` (wire byte; equivalent to API weight 256), `depends_on=0`, `exclusive=true`. Verified at `crates/net/src/h2_client.rs:167-174`.

### 3.2 What BO desktop / Android emits (verified against `h2_handshake_writes_chrome_146_settings_and_window_update`)

The byte-level regression test `crates/net/tests/h2_frame_bytes.rs:39-205` decodes the actual wire bytes and asserts:

- PREFACE = `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n` (24 bytes)
- SETTINGS frame: 4 settings, in order:
  - `(1, 65536)` HEADER_TABLE_SIZE
  - `(2, 0)` ENABLE_PUSH
  - `(4, 6291456)` INITIAL_WINDOW_SIZE
  - `(6, 262144)` MAX_HEADER_LIST_SIZE
- WINDOW_UPDATE frame: stream 0, delta 15,663,105

This test is NOT `#[ignore]` — it runs in CI and gates every PR. If a boring2 update or http2-crate update silently changes the wire bytes, this test fails.

The constants live at `crates/net/src/h2_client.rs:39-50`.

### 3.3 What BO iOS Safari emits

Per `lexiforest safari_18.4_iOS.yaml`, documented at `crates/net/src/h2_client.rs:52-68`. iOS Safari 18.4 sends 4 SETTINGS, but a different subset in a different order:

- `(2, 0)` ENABLE_PUSH
- `(3, 100)` MAX_CONCURRENT_STREAMS
- `(4, 2097152)` INITIAL_WINDOW_SIZE (2 MB, vs Chrome's 6 MB)
- `(9, 1)` NO_RFC7540_PRIORITIES (Safari opts out of HTTP/2 prioritization)

Plus a different pseudo-header order: `:method`, `:scheme`, `:authority`, `:path` (`m,s,a,p`) per `crates/net/src/h2_client.rs:88-104`.

Plus no separate HEADERS-frame priority hint (Safari emitting `NO_RFC7540_PRIORITIES = 1` means PRIORITY frames are disabled; we skip `headers_stream_dependency()` for the iOS branch).

WINDOW_UPDATE delta: configured target = 10,485,760 → wire = 10,420,225.

Note from `h2_client.rs:139-152`: Safari 18.4 ALSO needs `max_header_list_size` set on the connection (we use Chrome's 256 KB default) because some Akamai-fronted servers (e.g., `h-m.com`) return `RST_STREAM INTERNAL_ERROR` if the connection has no header-list-size limit. The setting is configured but does NOT appear on the wire (the http2 builder uses it internally for validation only). Discovered via Phase B sweep regression 2026-05-12.

### 3.4 ALPN negotiation

BO offers `h2,http/1.1` (length-prefixed: `\x02h2\x08http/1.1` per `tls.rs:186`). Both ALPN entries are advertised regardless of profile; the server picks. ECH grease is set on desktop+Android only (per `tls.rs:388-389`) — Safari does not send ECH grease.

If the server only supports `http/1.1`, we fall back to `h1_client.rs`. The HTTP/1.1 path has its own header order construction in `headers.rs` (the same builders work for both — HTTP/2 turns the textual headers into pseudo-headers + HPACK; HTTP/1.1 keeps them as-is).

If the server only supports `h2` and there's a TLS-level rejection, we get a connect failure and surface it as `NetError::Tls`.

---

## 4. Verification methodology

### 4.1 The byte-level harness in-tree

Two existing tests anchor the wire-byte verification:

1. **`crates/net/src/tls.rs:476-553` — `tls_fingerprint_vectors_no_silent_drift`** (in-process, no network). Pins every JA4 input (cipher list as a string, sigalgs list as a string, curves array, extension count, the deliberate `UA_CHROME_MAJOR=148` / `TLS_CHROME_MAJOR=147` split). Any drift fails loudly.

2. **`crates/net/tests/h2_frame_bytes.rs:39-205` — `h2_handshake_writes_chrome_146_settings_and_window_update`** (in-process, no network). Spawns a TCP listener on `127.0.0.1`, runs an HTTP/2 handshake with the `chrome_148_macos` profile, and parses the captured bytes to verify PREFACE + SETTINGS frame (in order) + WINDOW_UPDATE frame match Chrome 146/147.

Two existing tests anchor TLS-record-level behavior:

3. **`crates/net/src/tls.rs:562-619` — `safari_ios_emits_tls_1_0_record_version`**. Spawns a TCP server, attempts a TLS handshake with the iPhone profile, captures the first 5 bytes (TLS record header), and asserts `record_version == 0x0301` (TLS 1.0). Real Safari sends 0x0301 record version for the initial ClientHello (TLS-version negotiation happens in the inner `supported_versions` extension, not the outer record header).

4. **`crates/net/src/tls.rs:626-666` — `desktop_chrome_emits_tls_1_0_record_version`**. Same shape for desktop Chrome — also 0x0301.

One existing test anchors the Fisher-Yates shuffle behaviour:

5. **`crates/net/src/tls.rs:668-686` — `test_shuffle_is_full_fisher_yates`**. Verifies `shuffled_chrome_extension_permutation()` returns all 16 codepoints (preserves the set) and is non-deterministic run-to-run.

### 4.2 Network-gated tests (run with `--ignored`)

Per `CLAUDE.md`: "Network tests are `#[ignore]`. They require internet and live target sites. Run with `--ignored` locally only."

- `crates/net/tests/tls_fingerprint.rs:6-22` — `test_tls_fingerprint_peet` connects to `https://tls.peet.ws/api/all` (a known TLS fingerprint inspector — see §8) and asserts JA4 starts with `t13d`. Currently asserts ONLY the prefix; the v0.1.0 acceptance bar (§9) requires checking in the full expected JA4 string per profile and asserting equality.
- `crates/net/src/h2_client.rs:287-317` — `h2_get_httpbin` (network-gated) verifies the HTTP/2 path round-trips against `httpbin.org/get`.

### 4.3 The reference capture process

When you need a fresh capture of real Chrome's wire bytes (e.g., for adding a new TLS branch for `safari_18_macos`, or after Chrome ships a major TLS-stack change):

1. **Get a real Chrome stable on the target OS** (or use Playwright's bundled Chromium, which is verified to track Chrome stable).
2. **Capture via `tls.peet.ws/api/all`** for a structured JSON dump (JA3, JA4, akamai_fingerprint, peetprint), OR
3. **Capture via `openssl s_server -trace`** for raw ClientHello bytes:
   ```bash
   openssl s_server -trace -key key.pem -cert cert.pem -accept 127.0.0.1:4433 > capture.txt 2>&1
   ```
   Then point Chrome at `https://127.0.0.1:4433` and read the captured ClientHello hex from `capture.txt`.
4. **Cross-reference against `lexiforest/curl-impersonate`** — they ship verified signature YAML per browser per version (e.g., `tests/signatures/chrome_147.*_macos.yaml`, `safari_18.0_iOS.yaml`). Per `tls.rs:107` comment, the iOS Safari constants in BO are sourced from lexiforest's `safari_18.0_iOS.yaml`.
5. **Confirm via 3-source cross-check** (recommended by Chrome bot-detection research): the wire capture should match (a) the lexiforest reference, (b) a fresh `tls.peet.ws` JSON dump, AND (c) `wreq-util`'s Rust profile (their `src/emulate/profile/chrome/http2.rs` is the gold-standard Rust impl; we cross-reference it at `h2_client.rs:46-49`).

### 4.4 Where reference captures live

Currently there is **no dedicated directory for captured Chrome reference ClientHello bytes** in the repo. Reference data is embedded in:
- `crates/net/src/tls.rs:60-220` — Chrome constants (with inline comments citing the lexiforest / Playwright source)
- `crates/net/src/tls.rs:107-183` — iOS Safari constants (with `safari_18.0_iOS.yaml` cite)
- `crates/net/src/h2_client.rs:23-50` — Chrome H2 constants (with `tls.peet.ws/api/all` 2026-04-29 cite)
- `crates/net/src/h2_client.rs:52-68` — Safari H2 constants (with `lexiforest safari_18.4_iOS.yaml` cite)

**v0.1.0 deliverable per §9:** create `crates/net/tests/captures/` with one subdirectory per profile (`chrome_148/`, `safari_18_ios/`, etc.) containing (1) raw ClientHello hex, (2) the JA4 string, (3) the akamai_fingerprint string, (4) the source URL and capture timestamp. Then extend `tls_fingerprint_vectors_no_silent_drift` to load these files and diff against the live constants.

---

## 5. Quarterly refresh checklist

When Chrome / Safari / Firefox ships a new stable major, the checklist below is the playbook. Most majors do NOT change the TLS stack; the typical refresh is UA-only.

### 5.1 Chrome major bump (e.g., 148 → 149)

The expected delta (assuming no TLS-stack change, which is the modal case — see `11_PER_PROFILE_STRATEGY.md` §7.2):

- [ ] Confirm Chrome 149 stable shipped via `chromiumdash.appspot.com` (Mac/Windows row)
- [ ] **Capture a real Chrome 149 ClientHello** via `tls.peet.ws/api/all` from a Playwright-driven Chrome 149 on macOS arm64, Windows, AND Linux
- [ ] Compare captured cipher list / sigalg list / curves / extension count vs the existing `CIPHER_LIST` / `SIGALGS_LIST` / `CURVES_DESKTOP` / `CHROME_EXTENSION_PERMUTATION` constants
- [ ] If NO change (typical): bump `UA_CHROME_MAJOR = 149` (`tls.rs:57`); leave `TLS_CHROME_MAJOR = 147` (`tls.rs:52`)
- [ ] Bump UA + `browser_version` in every Chrome-class preset:
  - [ ] `crates/stealth/src/presets.rs:39-108` (`chrome_148_windows` → `chrome_149_windows`)
  - [ ] `crates/stealth/src/presets.rs:120-196` (`chrome_148_macos` → `chrome_149_macos`)
  - [ ] `crates/stealth/src/presets.rs:199-269` (`chrome_148_linux`)
  - [ ] `crates/stealth/src/presets.rs:272-385` (`chrome_148_ru`, `_cn`, `_de`, `_jp`)
  - [ ] `crates/stealth/src/presets.rs:690-792` (`pixel_9_pro_chrome_148`)
  - [ ] Rename function names + update doc comments
  - [ ] `crates/stealth/profiles/chrome_148_macos.yaml` → `chrome_149_macos.yaml` (or update in place)
- [ ] Update `crates/net/src/tls.rs:535-552` test loop to reference the new preset names
- [ ] `crates/browser/examples/sweep_metrics.rs:88-99` match arms updated
- [ ] `cargo test --workspace -- --test-threads=1` — `tls_fingerprint_vectors_no_silent_drift` should fail until `UA_CHROME_MAJOR` is bumped (this IS the silent-drift gate)
- [ ] Run regression sweep (per `14_TESTING_VALIDATION.md` §L4)
- [ ] If Pass rate drops > noise floor (5 sites per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`), bisect by reverting one field at a time

If Chrome 149 DOES roll a TLS-stack change (rare — last one was Chrome 131 MLKEM768), follow the existing 131 MLKEM playbook:
1. Capture a real Chrome 149 ClientHello in PCAP, run it through Wireshark + JA3/JA4 tools
2. Update `CIPHER_LIST`, `SIGALGS_LIST`, `CURVES_DESKTOP`, and the extension permutation in `tls.rs:60-220`
3. Bump `TLS_CHROME_MAJOR = 149`
4. Re-run the byte-level fingerprint test
5. Capture a fresh `tls.peet.ws/api/all` JSON dump for the new profile, check into `crates/net/tests/captures/chrome_149/` (per §4.4)

### 5.2 iOS Safari major bump (e.g., 18 → 19)

iOS Safari changes its TLS list MORE frequently than Chrome (Apple rolls TLS each major: 17→18 added new ciphers, 18.0→18.4 changed the SETTINGS frame).

- [ ] Confirm iOS 19 (or whatever Safari major) shipped via Apple's support page
- [ ] Refresh source: `lexiforest/curl-impersonate/tests/signatures/safari_19.X_iOS.yaml` (the canonical reference per `tls.rs:107` comment)
- [ ] Update the iOS Safari TLS constants in `tls.rs:107-183`:
  - [ ] `CIPHER_LIST_SAFARI_IOS` — Apple may add/remove ciphers (iOS 26 expected to add MLKEM per `tls.rs:150-151` comment)
  - [ ] `SIGALGS_LIST_SAFARI_IOS` — Apple has been known to fix the `rsa_pss_rsae_sha384` duplication bug; verify whether it's still present
  - [ ] `CURVES_SAFARI_IOS` — iOS 26 will add MLKEM768 to the front per Apple's PQC support page
  - [ ] `SAFARI_IOS_EXTENSION_PERMUTATION` — Apple may add extensions (signed_cert_timestamp behaviour, ECH if Apple ships it)
- [ ] Update the iOS Safari H2 constants in `h2_client.rs:53-68` if Apple changes the SETTINGS subset (18.0 → 18.4 dropped `8 ENABLE_CONNECT_PROTOCOL = 1`)
- [ ] Update the preset in `presets.rs:795-875` — UA + `browser_version` + `os_version`
- [ ] Re-run `safari_ios_emits_tls_1_0_record_version` to confirm BoringSSL still emits 0x0301 record version
- [ ] Re-run network-gated `test_tls_fingerprint_peet` with the new profile + check the JA4 against a fresh real-Safari capture from `tls.peet.ws`

### 5.3 Firefox major bump (e.g., 135 → 136)

UA-only at minimum — there is no TLS-class swap to maintain (BO's Firefox profiles still send Chrome-class TLS; the `firefox_135` `tls_impersonate` string is informational, see `presets.rs:457-463` comment).

- [ ] Bump `presets.rs:413-642` (`firefox_135_macos`, `firefox_135_windows`, `firefox_135_linux`) — UA + `browser_version`
- [ ] Real Gecko TLS is tracked as Phase B.3 future work (reconfigure boring2's cipher list and extension order to match NSS). NOT v0.1.0 scope.

### 5.4 BoringSSL / boring2 update

When boring2 ships a 4.16+, audit:
- [ ] Does the new boring2 still expose `CertCompressionAlgorithm`, `SslCurve`, `SSL_CTX_set_extension_permutation`, and `set_curves`? If NO, do NOT upgrade (per `Cargo.toml:23-30` comment)
- [ ] Does the BoringSSL update silently change extension order, ALPN frame format, or default sigalgs? Run `tls_fingerprint_vectors_no_silent_drift` + `h2_handshake_writes_chrome_146_settings_and_window_update` to catch
- [ ] Run the full L4 sweep — if any 4-profile Pass count drops by > 5 sites, the boring2 update changed something we depend on

---

## 6. ALPN, cipher pinning, ECH

### 6.1 ALPN

Per `tls.rs:186`: `const ALPN_PROTOS: &[u8] = b"\x02h2\x08http/1.1";`. We offer both protocols on every profile.

Behavioural note: real Chrome does the same. Real Safari on iOS does too. Firefox NSS also offers both. ALPN itself is not a differentiator across browsers.

If a server only supports `h2` and TLS-level negotiation fails for any reason (cipher mismatch, curve mismatch, sigalg mismatch), the connection is dropped at the TLS layer — the HTTP/2 path never starts and the failure surfaces as `NetError::Tls("TLS handshake failed: ...")`. The classifier in `crates/browser/src/classify.rs` will likely tag the resulting empty body as `THIN-BODY` (per the classification rules in `03_BENCHMARK_METHODOLOGY.md`).

### 6.2 Cipher / sigalg / curve pinning

Some sites pin TLS 1.3 only; some require specific cipher orderings; some require AES-GCM (no CHACHA20).

Our Chrome desktop cipher list (`CIPHER_LIST` at `tls.rs:60-76`) offers:
- 3 TLS 1.3 ciphers (`TLS_AES_128_GCM_SHA256`, `TLS_AES_256_GCM_SHA384`, `TLS_CHACHA20_POLY1305_SHA256`)
- 6 TLS 1.2 ECDHE_ECDSA/RSA combos with AES-GCM/CHACHA20-POLY1305
- 6 TLS 1.2 legacy fallbacks (CBC modes + RSA key exchange)

This matches Chrome 147 exactly. Any server that picks a TLS 1.3 cipher (which is most modern servers) will negotiate fine; servers that require a specific TLS 1.2 cipher will also be served from our list.

Our iOS Safari cipher list (`CIPHER_LIST_SAFARI_IOS` at `tls.rs:111-132`) is 20 entries — longer because Apple still includes 3DES variants at the tail. JA4 will encode the count `20` instead of `15`, which is one of the strongest browser-class signals available.

### 6.3 TLS 1.3 ECH (Encrypted Client Hello)

Per `tls.rs:298,388-389`: we set `builder.set_grease_enabled(true)` (Chrome desktop+Android), then `config.set_enable_ech_grease(true)` per-connection. The "grease" variant of ECH sends a random-looking ECH extension that LOOKS like Encrypted Client Hello to a server but doesn't actually encrypt — its purpose is to prevent middleboxes from being surprised when real ECH rolls out, and to be a stable Chrome-class signal.

Real Chrome ships ECH grease since ~Chrome 124. Safari does NOT — we skip ECH grease for the iOS branch per `tls.rs:387-389`. This is one of the four "Safari is missing" / "Safari is different" signals per `tls.rs:310-311` (the others: ALPS absence, session_ticket absence, Zlib cert compression vs Brotli).

Real ECH (not grease) is not implemented in BO. When Cloudflare flips ECH on for all servers, we'll need a follow-up — but currently <1% of real servers accept ECH (most just see the grease extension and proceed normally).

### 6.4 GREASE values (per RFC 8701)

`builder.set_grease_enabled(true)` at `tls.rs:298` enables BoringSSL's automatic GREASE injection. GREASE values (random-looking reserved cipher / extension / version codepoints) are injected at random positions to prevent ossification. Chrome does this on every handshake; we match.

---

## 7. Failure modes (when the network layer fails before JS runs)

When the classifier (`crates/browser/src/classify.rs`) returns `THIN-BODY` with < 1 KB, the failure is almost certainly network-layer (not JS-rendered). Common causes:

### 7.1 TLS handshake rejected → connection drop → THIN-BODY < 1 KB

Causes:
- Server pinned a cipher not in our list (rare for Chrome desktop; possible for Safari iOS profiles on servers that reject 3DES variants outright)
- Server rejected the curve list (rare — MLKEM768 is now widely supported)
- Server rejected the sigalg list (rare — Chrome's set is wide)
- ECH grease misinterpreted (vanishingly rare; we've not observed this)
- BoringSSL bug introduced by a `boring2` minor update (see §5.4 audit checklist)

Symptom: `NetError::Tls("TLS handshake failed: ...")` logged, then THIN-BODY 0b at the page level.

### 7.2 HTTP/2 framing rejected → http/1.1 fallback

Some servers (rare) reject Chrome's H2 SETTINGS frame because they implement an incomplete H2 stack. We don't automatically fall back to http/1.1 at the H2 frame layer — if the H2 handshake fails after a successful ALPN negotiation of `h2`, the connection closes. Recovery would require a TCP-level retry with ALPN-restricted to `http/1.1`. Not currently implemented.

### 7.3 ALPN mismatch

If the server only supports `h2` but we somehow negotiate `http/1.1` (impossible given our ALPN list offers both), the H1 client would send H1 framing into an H2-only server — which would close the connection. We do NOT see this in the wild because boring2's ALPN negotiator is correct.

### 7.4 The x-com THIN-BODY anomaly (open question)

Per `02_GAP_ANALYSIS.md:218-230` and `15_OPEN_QUESTIONS.md` §Q3:

In the full 2026-05-24 sweep, `x-com` (twitter.com) returned **69 bytes THIN-BODY** mid-sweep (after 24 prior nav requests had populated the process-wide `accept_ch` + cookie jar). In an isolated single-site run with no prior state, x-com returned **274 KB L3-RENDERED** — the SPA shell.

**Root cause hypothesis**: `f62584d` SharedSession bleed. The process-wide `accept_ch` set picked up an `Accept-CH` header advertisement from some earlier site that Twitter's WAF now flags as anomalous, dropping the connection at the TLS or H2 boundary. Alternative: IP rate-limit. Either way, the symptom is THIN-BODY 0b-ish at the page level.

**Why it's in scope for THIS doc:** if the bleed is TLS/H2-level (a state-dependent header bleed that twiddles a fingerprint feature), the fix lives in `crates/net/src/cookies.rs` / wherever the SharedSession accept_ch set is held. If the bleed is purely IP-rate-limit, no engine fix.

**v0.1.0 acceptance bar (§9):** identify the root cause via the A/B test in `15_OPEN_QUESTIONS.md` §Q3 (full 126-sweep with `HttpClient::shared` vs `HttpClient::new`).

---

## 8. References

### Project-external references
- **boring2 GitHub** (Cloudflare BoringSSL fork): https://github.com/cloudflare/boring — pinned at 4.15.15 per `Cargo.lock:280-293`
- **JA4 spec** (FoxIO): https://github.com/FoxIO-LLC/ja4 — the family of fingerprints (JA4, JA4S, JA4H, JA4X, ...)
- **HTTP/2 frame inspector** (peet.ws): https://tools.peet.ws/ — interactive H2 dump
- **TLS fingerprint inspector** (tls.peet.ws): https://tls.peet.ws/api/all — JSON dump of JA3, JA4, akamai_fingerprint, peetprint
- **TLS fingerprint check** (browserleaks): https://browserleaks.com/tls — alternative checker
- **lexiforest / curl-impersonate**: https://github.com/lexiforest/curl-impersonate — canonical signatures per browser per version (referenced at `tls.rs:107,151,161`)
- **Chrome's per-handshake Fisher-Yates** (Fastly blog + Chromestatus): see `tls.rs:194-202` for the citation and how Chrome's extension shuffle works
- **chromiumdash**: https://chromiumdash.appspot.com/ — current Chrome stable versions per OS
- **wreq-util** (gold-standard Rust impersonation impl): https://github.com/0x676e67/wreq-util — `src/emulate/profile/chrome/http2.rs` is what we cross-reference

### In-tree TLS-layer source
- `crates/net/src/tls.rs:22-57` — `TLS_CHROME_MAJOR=147` / `UA_CHROME_MAJOR=148` constants + rationale (the deliberate, wire-coherent split)
- `crates/net/src/tls.rs:60-76` — Chrome cipher list (15 entries, JA4-significant order)
- `crates/net/src/tls.rs:79-88` — Chrome sigalg list (8 entries)
- `crates/net/src/tls.rs:91-96` — Chrome desktop curves (MLKEM768 leads since Chrome 131)
- `crates/net/src/tls.rs:99-104` — Chrome Android curves (currently == desktop; verify caveat per comment)
- `crates/net/src/tls.rs:107-132` — iOS Safari cipher list (20 entries incl. 3DES tail)
- `crates/net/src/tls.rs:134-148` — iOS Safari sigalg list (10 entries incl. duplicated `rsa_pss_rsae_sha384` Apple bug)
- `crates/net/src/tls.rs:150-157` — iOS Safari curves (no PQ; adds P-521)
- `crates/net/src/tls.rs:169-183` — iOS Safari extension permutation (FIXED order, 13 extensions, no Fisher-Yates)
- `crates/net/src/tls.rs:186` — ALPN protos
- `crates/net/src/tls.rs:188-228` — Chrome extension permutation (16 entries) + `shuffled_chrome_extension_permutation` Fisher-Yates
- `crates/net/src/tls.rs:233-369` — `chrome_connector()` per-`device_class` branch
- `crates/net/src/tls.rs:376-439` — `configure_connection()` per-connection ALPS + ECH grease
- `crates/net/src/tls.rs:442-454` — `connect_tls()` async TLS connect
- `crates/net/src/tls.rs:457-459` — `negotiated_alpn()` helper
- `crates/net/src/tls.rs:476-553` — `tls_fingerprint_vectors_no_silent_drift` (THE silent-drift gate)
- `crates/net/src/tls.rs:562-619` — `safari_ios_emits_tls_1_0_record_version` (record-version test)
- `crates/net/src/tls.rs:626-666` — `desktop_chrome_emits_tls_1_0_record_version`
- `crates/net/src/tls.rs:668-686` — `test_shuffle_is_full_fisher_yates`

### In-tree HTTP/2 source
- `crates/net/src/h2_client.rs:23-50` — Chrome H2 SETTINGS constants (verified 2026-04-29 against `tls.peet.ws/api/all`)
- `crates/net/src/h2_client.rs:52-68` — Safari iOS 18.4 H2 SETTINGS constants
- `crates/net/src/h2_client.rs:85-130` — `handshake()` per-`device_class` branch (Chrome `masp` vs Safari `msap` pseudo-header order; Chrome 8-entry SettingsOrder vs Safari 4-entry; Chrome HEADERS-frame priority vs Safari skip)
- `crates/net/src/h2_client.rs:132-181` — H2 handshake builder construction (gates Safari `max_header_list_size` for h-m.com / Akamai-fronted servers)
- `crates/net/tests/h2_frame_bytes.rs:39-205` — `h2_handshake_writes_chrome_146_settings_and_window_update` byte-equivalence test
- `crates/net/src/ja4h.rs:1-26` — JA4H spec + test-only computer (FoxIO non-commercial license carve-out)

### In-tree HTTP/1.1 + header source
- `crates/net/src/h1_client.rs` — HTTP/1.1 fallback path (used when ALPN negotiates http/1.1)
- `crates/net/src/headers.rs:16-23` — `nav_headers()` browser-aware dispatch (Chrome / Firefox / Safari)
- `crates/net/src/headers.rs:78-80` — `chrome_headers()` entry
- `crates/net/src/headers.rs:225-...` — `chrome_headers_impl()` (the 13-header Chrome 142+ canonical order)
- `crates/net/src/headers.rs:401-404` — `build_sec_ch_ua_full_version_list` (high-entropy CH, only after Accept-CH)
- `crates/net/src/headers.rs:415-430` — `build_sec_ch_ua` (low-entropy CH, every Chrome request)
- `crates/net/src/headers.rs:465-...` — `firefox_headers()` (no UA-CH, no priority, different accept)
- `crates/net/src/headers.rs:591-...` — `safari_headers()` (no UA-CH on Safari)

### In-tree HTTP/3 + QUIC source (default-off, see `presets.rs::http3_disabled_by_default_on_all_presets`)
- `crates/net/src/quic.rs:1-22` — QUIC connector using rustls (TLS 1.3 only, ALPN `h3`)
- `crates/net/src/h3_request.rs` — HTTP/3 request implementation
- `crates/net/src/alt_svc.rs` — Alt-Svc cache (for HTTPS-to-h3 upgrades)

### Existing test infrastructure
- `crates/net/tests/tls_fingerprint.rs:6-22` — `test_tls_fingerprint_peet` (network-gated, runs against `tls.peet.ws/api/all`)
- `crates/net/tests/h2_frame_bytes.rs` — byte-equivalence regression
- `crates/net/tests/proxy_roundtrip.rs` — proxy support
- `crates/net/src/h2_client.rs:287-317` — `h2_get_httpbin` (network-gated, round-trip)

### In-tree configuration
- `crates/net/Cargo.toml:28-30` — boring2 pin + features
- `crates/net/Cargo.toml:23-30` — comment explaining why we stay on 4.x not 5.0-alpha
- `crates/net/src/lib.rs:1-25` — module entry points + ja4h test-gating

### Cross-references in this doc set
- `11_PER_PROFILE_STRATEGY.md` §1.5 — per-profile TLS deltas table
- `11_PER_PROFILE_STRATEGY.md` §7 — Chrome / Safari / Firefox bump playbook (the higher-level prose version of §5 above)
- `12_COMPETITIVE_LANDSCAPE.md` §2.2 — why our TLS impl is a customer-visible differentiator vs Playwright / Patchright / playwright-stealth
- `19_PROFILE_EXPANSION_PLAN.md` §2.1 — Candidate A (`safari_18_macos`) needs a NEW TLS branch covered by this doc
- `19_PROFILE_EXPANSION_PLAN.md` §5 — per-profile maintenance schedule that triggers §5 of this doc
- `02_GAP_ANALYSIS.md` §10 — x-com THIN-BODY (the open question §7.4 references)
- `15_OPEN_QUESTIONS.md` Q3 — the A/B test that resolves x-com
- `03_BENCHMARK_METHODOLOGY.md` — classifier rules (`THIN-BODY < 1 KB` definition)
- `14_TESTING_VALIDATION.md` §L4 / §L5 — sweep methodology that catches network-layer drift

---

## 9. Acceptance for v0.1.0

- [ ] **Captured JA4 per profile published in-tree**: `crates/net/tests/captures/<profile>/ja4.txt` for each of the 4 (eventually 6) shipped profiles, containing the JA4 string from a `tls.peet.ws/api/all` capture; `test_tls_fingerprint_peet` extended to load + assert exact-match (not just `t13d` prefix)
- [ ] **Captured akamai_fingerprint per profile**: same directory, `akamai_h2.txt` file
- [ ] **Raw ClientHello hex per profile**: same directory, `clienthello.hex` from `openssl s_server -trace`
- [ ] **HTTP/2 SETTINGS frame byte-equivalence test stays green**: `h2_handshake_writes_chrome_146_settings_and_window_update` (`tests/h2_frame_bytes.rs:39-205`) passes
- [ ] **Silent-drift gate stays green**: `tls_fingerprint_vectors_no_silent_drift` (`tls.rs:476-553`) passes
- [ ] **Record-version test stays green for both profile classes**: `safari_ios_emits_tls_1_0_record_version` + `desktop_chrome_emits_tls_1_0_record_version`
- [ ] **Quarterly refresh checklist tested**: dry-run the Chrome 148 → (hypothetical) 149 path per §5.1 against a Chrome Canary capture; document the deltas observed
- [ ] **x-com THIN-BODY root cause identified**: per `15_OPEN_QUESTIONS.md` Q3 — A/B `HttpClient::shared` vs `HttpClient::new` on the full 126-sweep, record whether x-com flips
- [ ] **Per-profile JA4H baseline locked**: `crates/net/src/ja4h.rs` tests pass for all shipped profiles (currently passes for the documented set; extend if new profiles are added per `19_PROFILE_EXPANSION_PLAN.md` §7)
- [ ] **Safari-on-macOS TLS branch added** (if `19_PROFILE_EXPANSION_PLAN.md` Candidate A ships in v0.1.0): new `CIPHER_LIST_SAFARI_MACOS` / `SIGALGS_LIST_SAFARI_MACOS` / `CURVES_SAFARI_MACOS` constants in `tls.rs`, branching on a `DesktopSafari` discriminator (extend `DeviceClass` or add a `browser_name == "Safari"` predicate)
- [ ] **boring2 pin documented**: any proposed bump from 4.15.x must run the full L4 sweep + the 2 byte-equivalence tests + JA4 capture diff; document the result in a changelog entry

---

## 10. Files referenced

### TLS implementation
- `crates/net/src/tls.rs:1-687` — full TLS module (constants, builder, per-connection config, tests)
- `crates/net/Cargo.toml:28-30` — `boring2 4.15` + `boring-sys2 4.15` + `tokio-boring2 4.15` (pinned)
- `Cargo.lock:266-293` — boring-sys2 / boring2 resolved version (4.15.15)

### HTTP/2 implementation
- `crates/net/src/h2_client.rs:1-318` — H2 handshake + send_get / send_post
- `crates/net/tests/h2_frame_bytes.rs:1-205` — byte-equivalence regression test (PREFACE, SETTINGS, WINDOW_UPDATE)

### HTTP/1.1 + header builders
- `crates/net/src/h1_client.rs` — HTTP/1.1 path (fallback when ALPN negotiates http/1.1)
- `crates/net/src/headers.rs:1-1172` — header builders for Chrome / Firefox / Safari (nav / reload / fetch flavours)
- `crates/net/src/ja4h.rs:1-240` — JA4H computer (test-only per FoxIO License 1.1; see `LICENSE-NOTE.md`)

### HTTP/3 + QUIC (default-off)
- `crates/net/src/quic.rs:1-22` — QUIC connector using rustls
- `crates/net/src/h3_request.rs` — HTTP/3 request impl
- `crates/net/src/alt_svc.rs` — Alt-Svc cache (HTTPS → h3 upgrade discovery)

### Profile schema (consumer of the TLS layer)
- `crates/stealth/src/profile.rs:8-15` — `DeviceClass` enum (drives TLS branch)
- `crates/stealth/src/profile.rs:108-109` — `tls_impersonate` field
- `crates/stealth/src/profile.rs:134-146` — `allow_http3` (default false because quinn randomizes transport_parameters)
- `crates/stealth/src/presets.rs:39-875` — all 12 preset constructors

### Network tests (run with `--ignored`)
- `crates/net/tests/tls_fingerprint.rs:6-22` — `test_tls_fingerprint_peet` (network-gated)
- `crates/net/src/h2_client.rs:287-317` — `h2_get_httpbin` (network-gated)

### Workspace
- `CLAUDE.md` — "HTTP/TLS: own stack in `crates/net/` using `boring2` ... for Chrome-identical TLS ClientHello + HTTP/2 fingerprint"
- `deny.toml` — license whitelist (boring2 OpenSSL-style fits without per-crate exception)
- `docs/ARCHITECTURE.md` — workspace dependency graph
- `crates/net/src/LICENSE-NOTE.md` — FoxIO JA4H license note
