# 34 — Per-vendor test harness specs

**Status:** spec / planning. Three of the harnesses described here ship in v0.1.0; the rest are post-v0.1.0 follow-on work.
**Audience:** any contributor making a vendor-facing change (a new bootstrap surface, a TLS knob, an op signature, a CSP relax line). Run the relevant per-vendor harness before merge — like the prior `kasada_error_blob_capture` workflow, scaled across the vendor matrix.
**Companion docs:** `08_KASADA_FRONTIER.md` (the template — §Lever 1), `14_TESTING_VALIDATION.md` (the L1-L5 validation pyramid this slots into), `04_TOOLING_SPEC.md` (the `--capture` mode + the per-site directory layout), `06_AWS_WAF_SOLVER.md` §4 (the AWS WAF capture flow this generalises), `07_DATADOME_PRIMITIVES.md` (DataDome capture references), `18_ANTI_BOT_VENDOR_COOKBOOK.md` §2 (the vendor-by-vendor marker tables this builds on), `13_FILE_LOCATIONS_INDEX.md` (file:line index for the bootstrap targets).

**One-paragraph thesis:** vendors rotate their challenges quarterly. Camoufox and BO both drift down their pass rates between rotations because nothing tells us "the canadagoose `/tl` body shape changed shape at 02:00 UTC, the `bot1225` field is now using a different obfuscated property name." The Kasada-specific `kasada_error_blob_capture` test pattern caught the CSS calc precision gap weeks before the live sweep would have. Generalising that pattern — one capture harness per vendor, each running against a known protected site, decrypting and asserting against a captured shape — is the lowest-cost insurance against silent regression we can buy. The spec below is six harnesses (AWS WAF, DataDome, Akamai BMP, Cloudflare JSC, Kasada, PerimeterX) that all share the same shape: `cargo test … --ignored --test-threads=1`, write decoded artifacts to a known directory, assert a minimum field count, log a diff against the prior snapshot.

---

## 1. The Kasada pattern as template

The `kasada_error_blob_capture` test referenced in `08_KASADA_FRONTIER.md` is the prototype for everything below. It demonstrated that a single per-vendor diagnostic harness, run from `cargo test`, is enough to (a) prove "the vendor is still solvable" before the sweep catches it, (b) leave an artifact a human can read post-hoc, (c) regress-gate any change to the bootstrap surface that vendor probes.

### 1.1 What the test did

The prototype (referenced at `08_KASADA_FRONTIER.md §Lever 2` and `§Adjacent files`) ran this loop, conceptually:

1. **Navigate to canadagoose.com** via `Page::navigate_with_solvers` using the `chrome_148_macos` profile. canadagoose is the cleanest Kasada-protected target — small body, single `/tl` POST, no captcha-delivery iframe nest.
2. **Intercept the Kasada VM's error-report POST** to the `/tl` endpoint (the VM POSTs error reports when its passive probes throw — see `08 §Phase 1` for the discovery arc).
3. **Decrypt via the XOR / `omgtopkek` wrapper**: the body is `base64(json({"data": base64(xor(plaintext, b"omgtopkek"))}))` per `kasada_wrapper_cracked_and_remaining_leaks.md`. Decrypted using `docs/kasada_ips_analysis/scratch/decrypt_report.py`.
4. **Write blobs** to `docs/kasada_ips_analysis/scratch/kasada_error_*.b64`. One file per error-report POST observed during the navigate.
5. **Validate against expected error fields**: assert ≥ N blobs were produced (the inventory shrinks over time as fixes land), and that none of them carry a regression marker (a previously-fixed field name reappearing).
6. **Single command:**
   ```bash
   cargo test -p browser --test chrome_compat kasada_error_blob_capture \
       -- --ignored --test-threads=1 --nocapture
   ```

### 1.2 Why this beat the sweep

The `holistic_sweep` 126-corpus run takes 50-90 minutes (`14 §L4`) and only reports pass/fail per site — a `Kasada-CHL 740` result tells you nothing about *which* probe in the bytecode VM rejected your fingerprint. The `kasada_error_blob_capture` harness:

- **runs in ~ 90 seconds** (single site, single nav, ~ 60 s of Kasada bytecode execution + 30 s of post-processing),
- **emits a human-readable artifact** (the decrypted error report names the failing field — `bot1225`, `dpv`, `nppm` — pointing directly at the broken Web API),
- **regression-gates the fix loop**: if you ship a CSS calc fix and the calc-precision blob disappears from the inventory, you have evidence the fix landed beyond "the pass rate didn't move."

This is the L2 of `14 §L2` instantiated as a `cargo test` invocation rather than a manual capture-and-diff workflow.

### 1.3 The pattern in one diagram

```
  ┌──────────────────────────────────────────────────┐
  │ #[ignore]  cargo test <harness>_capture          │
  └─────────────────┬────────────────────────────────┘
                    │
                    ▼
  ┌──────────────────────────────────────────────────┐
  │ 1. Page::navigate(target_url, profile)           │
  │                                                  │
  │    JS-level: install_op_capture_sink(sink)       │
  │    Net-level: install_net_intercept(POST_RE)     │
  └─────────────────┬────────────────────────────────┘
                    │
                    ▼
  ┌──────────────────────────────────────────────────┐
  │ 2. Intercepted POST bodies tee'd to sink         │
  │    (vendor-specific endpoint regex)              │
  └─────────────────┬────────────────────────────────┘
                    │
                    ▼
  ┌──────────────────────────────────────────────────┐
  │ 3. Decrypt (if vendor uses encryption)           │
  │    XOR / TEA-CBC / AES-GCM / JWE / WASM-derived  │
  └─────────────────┬────────────────────────────────┘
                    │
                    ▼
  ┌──────────────────────────────────────────────────┐
  │ 4. Write artifacts                               │
  │    ~/projects/browser_oxide_internal/captures/   │
  │      <vendor>/<timestamp>_<site>/                │
  └─────────────────┬────────────────────────────────┘
                    │
                    ▼
  ┌──────────────────────────────────────────────────┐
  │ 5. Assert minimum-field invariant + diff prior   │
  │    DIFF.md per vendor populated on every run     │
  └──────────────────────────────────────────────────┘
```

---

## 2. Generalising the pattern

For every vendor that has either:

- **An error-report or telemetry endpoint** — vendor JS POSTs when its probes throw (Kasada `/tl` error report, Cloudflare orchestrator throw, Akamai sec-cpt failure), OR
- **A fingerprint-collection POST** — vendor JS sends an envelope of device signals before deciding pass/fail (DataDome iframe `captcha/check`, AWS WAF `inputs?client=browser`, PerimeterX `xhr/api/v2/collector`), OR
- **A challenge-result POST** — vendor JS submits a computed solution (Cloudflare JSC `jschl-answer`, Imperva `_Incapsula_Resource?CWUDNSAI=…`, AWS WAF `verify`),

write a `<vendor>_*_capture` test that:

1. **Navigates to a known protected site** that is in the 126-corpus (or accessible from CI). Pick the cleanest target for the vendor — minimum body, minimum dependency chain, no captcha gate. The "cleanest target" choices are listed per-vendor in §3.
2. **Intercepts the relevant POST(s)**. Two viable interception levels:
   - **Net-level** — hook the `op_fetch` path (`crates/js_runtime/src/extensions/fetch_ext.rs:231-381` per `04 §Files referenced`). Adds zero JS-visible side effect. Captures cipher-text bodies. Right answer when the body shape doesn't need pre-encryption inspection (AWS WAF, Cloudflare JSC, PerimeterX where the encryption is opaque).
   - **JS-level** — install a `__captureSink` shim in `stealth_bootstrap.js` that wraps `XMLHttpRequest.send` + `fetch` and captures the body *pre-encryption*. Right answer when the vendor encrypts client-side and the plaintext is the diagnostic gold (Kasada `/tl`, DataDome iframe payload, Akamai sensor_data, Imperva reese84).
3. **Decrypts if applicable** — vendor-specific wrapper. The decryption keys we know:
   - **Kasada `/tl`** — XOR with `b"omgtopkek"` (9-byte rotating, deployment-constant). See `kasada_wrapper_cracked_and_remaining_leaks.md`.
   - **Akamai BMP v2** — TEA-CBC, per-tenant integrity field. Logic was in the stripped private `akamai` crate before `aecdf19` — re-implement against the deobfuscated `akam/13` script.
   - **DataDome iframe** — daily-rotating WASM-derived key (see `07_DATADOME_PRIMITIVES.md §Primitive 2` — the iframe runs WASM that produces the key). The harness ALWAYS does JS-level interception to side-step needing the key.
   - **AWS WAF** — `gokuProps.key` + `gokuProps.iv` are visible in the challenge stub HTML. Decoder is per-tenant but the format is consistent. See `06_AWS_WAF_SOLVER.md §1`.
   - **Cloudflare JSC** — none; the POST body is the cleartext computed JSC answer `(jschl_vc, jschl_answer, pass)`.
   - **PerimeterX** — per-version envelope; v3 is documented at `niespodd/browser-fingerprinting`.
   - **Imperva reese84** — heavily obfuscated client-side encoder, ASTs documented at `Thoosje/Reese84-Deobfuscator`. Always JS-level intercept.
4. **Writes captured artifacts** to a known path under `~/projects/browser_oxide_internal/captures/<vendor>/`. The internal-repo path is **load-bearing**: the public engine repo MUST NOT carry decrypted vendor payloads (license drift, vendor takedown risk, accidental publication). The internal-repo arrangement matches the existing pattern at `~/projects/browser_oxide_internal/benchmarks/baselines/` (per `14 §L3 pass criteria`).
5. **Asserts the capture happened**: non-empty body, expected field count ≥ N for the given vendor. The N values per-vendor are below in §3.
6. **Logs a `DIFF.md` line** comparing this run's capture to the prior snapshot. The diff tool is shared across vendors — see §5.

---

## 3. Per-vendor harness specs

Each subsection follows the same skeleton: target site, POST endpoint pattern, capture method, decryption, expected fields, assertion criteria, output path, run frequency. The "Implemented in v0.1.0" badge is set per-subsection — only AWS WAF, DataDome, and Cloudflare are MUST-ship-by-v0.1.0; the rest are post-v0.1.0 follow-on.

---

### 3.1 AWS WAF — `aws_waf_challenge_capture` *(MUST ship by v0.1.0)*

**Target site:** `https://www.amazon.de/` — the canonical AWS WAF Challenge tenant in our 126-corpus (per `02 §Cluster 5-8`, `18 §2.1`). Stub size 2011 B. amazon-in / amazon-com-au / imdb are alternates if amazon-de starts serving a non-WAF response on the CI IP.

**POST endpoint pattern:** the per-tenant `*.token.awswaf.com` host. The decisive POSTs are:
- `POST https://<tenant>.token.awswaf.com/<region>/<tenant_id>/inputs?client=browser` — the fingerprint envelope (this is the one we want to diff).
- `POST https://<tenant>.token.awswaf.com/<region>/<tenant_id>/verify` — the PoW solution submission (issues `Set-Cookie: aws-waf-token=…`).
- The regex to intercept: `(?i)\.token\.awswaf\.com/.*/(inputs\?client=browser|verify|report)$`.

**Capture method:** **net-level**. The AWS WAF SDK encrypts client-side using `gokuProps.key`/`iv` (per `06 §1`), and we'd need to evaluate `challenge.js` to recover the keys — JS-level intercept doesn't materially help because `gokuProps` is fed to the encryptor before any wrappable entry point. Capture the cipher-text body and the `Content-Type`, save both. The decoder is offline (run separately, not in-test) once `gokuProps` is recovered from the captured stub HTML.

**Decryption needed?** Indirect. The captured body is `gokuProps`-encrypted; the harness writes both the cipher-text + the stub HTML (which contains `gokuProps.key`/`iv` cleartext). A separate offline decoder script (`tools/aws_waf_decode.py`, post-v0.1.0) is the path to plaintext. For v0.1.0 the harness asserts only "POST happened, body is non-empty, `aws-waf-token` cookie was OR was NOT issued" — the latter being the regression signal.

**Expected fields (in the encrypted body once decoded):**
- `gokuProps.key` — present in stub, captured separately.
- `gokuProps.iv` — present in stub, captured separately.
- Fingerprint envelope: ~ 50+ navigator / screen / WebGL / AudioContext properties (the AWS WAF SDK collector — list in `06 §2`).
- PoW solution variant: one of `HashcashScrypt | SHA-256 | NetworkBandwidth` per [`neiii/aws-waf-solver`](https://github.com/neiii/aws-waf-solver).

**Assertion criteria (v0.1.0 ship):**
- ≥ 1 POST to `/inputs?client=browser` observed (the fingerprint envelope went out).
- The encrypted body length is ≥ 200 B (not a degenerate empty envelope).
- Stub HTML contains `AwsWafIntegration` + `gokuProps` literal strings.
- If `Set-Cookie: aws-waf-token=…` was observed → assertion: `getToken()` resolved → fail loud (because the v0.1.0 expectation is amazon-de DOES NOT pass; if it suddenly does, we want to know — possibly amazon rolled the WAF rule and the test target needs re-pinning to imdb).
- Conversely if no `/verify` POST + no cookie → expected baseline; pass.

**Assertion criteria (post-v0.1.0 once decoder ships):**
- Decoded envelope contains ≥ 30 fields matching the published collector list at [`xKiian/awswaf`](https://github.com/xKiian/awswaf).
- Field set diff vs the prior snapshot < 5 fields renamed/added (vendor rotation tolerance).

**Output path:** `~/projects/browser_oxide_internal/captures/aws_waf/<YYYY-MM-DD>_amazon_de/`.

```
~/projects/browser_oxide_internal/captures/aws_waf/2026-05-24_amazon_de/
├── stub.html               # the 2011-B challenge stub (cleartext)
├── challenge.js.sha256     # hash of challenge.js for rotation tracking
├── inputs.body.bin         # cipher-text of POST /inputs (captured)
├── inputs.headers.json     # request + response headers
├── verify.body.bin         # cipher-text of POST /verify (if observed)
├── verify.headers.json
├── set_cookies.json        # all Set-Cookie observations
├── fetches.json            # the full chain (re-uses `04 §fetches.json` shape)
└── meta.json               # {profile, target_url, ms, bo_commit, capture_at}
```

**Run frequency:**
- **per-fix:** any contributor changing any WAF-relevant surface (TLS, fetch headers, AWS WAF detection markers in `classify.rs:81-156`, the `_maskAsNative` audit per Lever 3 of `08 §`).
- **weekly:** in the nightly-then-aggregated sweep workflow (`14 §L5`).
- **quarterly:** review captured rotations via `DIFF.md`. If amazon-de changes endpoint shape, update the harness.

---

### 3.2 DataDome — `datadome_challenge_capture` *(MUST ship by v0.1.0)*

**Target site:** `https://www.etsy.com/` — the canonical DataDome `rt:'i'` (silent interstitial) tenant in our 126-corpus (per `02 §3`, `18 §2.2`). Stub size 1424 B across all 4 profiles. tripadvisor (1430 B) is the second-line alternate if etsy starts serving a captcha-mode (`rt:'c'`) response.

**POST endpoint pattern:** the i.js telemetry endpoint, served from `captcha-delivery.com`. The decisive POSTs:
- `POST https://geo.captcha-delivery.com/captcha/check?…` — the iframe fingerprint envelope. This is the one we want.
- `POST https://js.datadome.co/i.js?…` — telemetry on outer-document load (some tenants).
- Regex: `(?i)(geo\.captcha-delivery\.com/captcha/check|js\.datadome\.co/i\.js)`.

**Capture method:** **JS-level**. DataDome's iframe WASM derives the encryption key client-side from a daily-rotating seed (`07_DATADOME_PRIMITIVES.md §Primitive 2`). Net-level capture only gives us cipher-text we cannot decrypt offline. JS-level intercept hooks `XMLHttpRequest.prototype.send` + `fetch` *inside the iframe scope* (the iframe runs in its own realm per `07 §Primitive 2`), captures the body parameter pre-encryption, and tees to a per-frame sink.

**Decryption needed?** No (because we intercept pre-encryption). The plaintext payload structure is documented at [`glizzykingdreko/Datadome-Captcha-Deobfuscator`](https://github.com/glizzykingdreko/Datadome-Captcha-Deobfuscator) — JSON with ~ 200+ fingerprint signals.

**Expected fields:**
- `dd_engagement` / `dd_session_id` literal cookies, observed via `cookie_writes.json`.
- Iframe payload: `cid`, `referer`, `r` (random nonce), `s` (sequence number), `pp` (page-position), and a fingerprint blob containing canvas hash, audio hash, navigator, WebGL, font-list, behavioural-timing.
- Response from `/captcha/check`: `{"status": "ok", "cookie": "datadome=<base64>"}` on solve, or `{"status": "ko", "url": "…"}` on rejection.

**Assertion criteria (v0.1.0 ship):**
- ≥ 1 `Set-Cookie: datadome=…` observed (per Primitive 3 of `07`).
- ≥ 1 POST to `/captcha/check` if `started_as_dd_challenge` was set.
- The iframe was rematerialised (per Primitive 2 — `rematerialize_iframes` at `page.rs:649` ran ≥ once).
- Body of the post-challenge document ≥ 30 KB (per `14 §Phase 3` acceptance — etsy passes when body > 30 KB).

**Assertion criteria (post-v0.1.0):**
- Plaintext payload has ≥ 50 fingerprint-signal fields populated (no nulls in the canvas/audio/WebGL trio).
- The `pp` (page-position) field is non-zero (proves the iframe ran in a real layout, not a degenerate 0×0 box).

**Output path:** `~/projects/browser_oxide_internal/captures/datadome/<YYYY-MM-DD>_etsy/`.

```
~/projects/browser_oxide_internal/captures/datadome/2026-05-24_etsy/
├── stub.html
├── dd_script.js.sha256     # daily-rotating bundle hash
├── iframe.url              # the captcha-delivery iframe src
├── iframe.payload.json     # plaintext (JS-intercepted) fingerprint
├── check.response.json     # /captcha/check response
├── set_cookies.json
├── fetches.json
└── meta.json
```

**Run frequency:** per-fix on any DataDome-touching change (CSP relax tweaks, iframe rematerialise logic, the clearance-cookie predicate at `cookies_carry_anti_bot_clearance` per `07 §Primitive 3`). Weekly in the aggregated sweep. Quarterly for DataDome's daily-rotating bundle audit.

---

### 3.3 Akamai BMP — `akamai_sensor_data_capture` *(post-v0.1.0)*

**Target site:** `https://www.adidas.com/` with the `firefox_135_macos` profile. adidas is the cleanest Akamai BMP target that BO consistently *passes* (per `12 §1.1`, `26 §4.1`) — capturing on a passing site gives us the gold-shape sensor_data; running with a *different* profile that fails (e.g. `chrome_148_macos`) on the same site gives us the diff. homedepot is an alternate but uses the sec-cpt sub-product which has a different POST flow.

**POST endpoint pattern:** the sensor POST URL is per-tenant — varies between `/akam/13/<random>`, `/<random_8>/<random_8>`, and `/<vendor_specific>/akam-sw.js?_=…`. The regex is: `(?i)(/akam/\d+/|/akam-sw\.js|/_bm/|/sensor)` paired with `Content-Type: text/plain` (Akamai's signature combo). Detection logic is in `classify.rs:128` (`sensor_data` co-signal).

**Capture method:** **JS-level**. Akamai BMP v2 uses TEA-CBC client-side (per `aecdf19` stripped logic); v3 uses PRNG-shuffled JSON keyed off the `_abck` value. Both encryptors run in the BMP script before the POST is issued. Hook the POST body pre-encryption by wrapping `XMLHttpRequest.prototype.send` in the same shim used for DataDome (one shim, vendor-dispatched by URL regex).

**Decryption needed?** Not at capture-time. Post-v0.1.0 the decoder reads the captured plaintext and validates structure.

**Expected fields:** the sensor_data v2 / v3 envelope contains ~ 200 signals per `18 §2.3`:
- `bb` — navigator basic fields (UA, platform, language).
- `bcd` — battery + connection + device-pixel-ratio.
- `bda` / `bdb` — behavioural timing arrays (mouse/touch event arrival times).
- `bdf` — font list.
- `bdg` — WebGL renderer / vendor strings.
- `bz` — entropy seed (per-page-load).
- `pkg_data` (v3 only) — packed obfuscated wrapper of the above.

**Assertion criteria:**
- ≥ 1 POST observed matching the regex.
- The captured body decrypts (TEA-CBC for v2, JSON-parse for v3) without error.
- `_abck` cookie's score-bearing infix after the POST is *not* `~0~-1~-1~` (per `18 §1.4` — `-1~-1` means uncleared).
- Field count in the decrypted envelope ≥ 100 (sentinel value — anything less means the BMP script bailed early, which is itself the regression).

**Output path:** `~/projects/browser_oxide_internal/captures/akamai_bmp/<YYYY-MM-DD>_adidas_firefox/`.

**Run frequency:** post-v0.1.0 monthly initially (Akamai v3 rotates less frequently than DataDome). Per-fix on `26_AKAMAI_BMP_DEEP.md`-related changes.

---

### 3.4 Cloudflare — `cloudflare_jschl_capture` *(MUST ship by v0.1.0)*

**Target site:** a Cloudflare-JSC ("Just a moment...") site — pick from a public list of `cf-browser-verification`-serving hosts. Candidates (from §2.5 of `18` and the cookbook): a smaller-tier publisher / forum / community site that consistently issues the classic JSC interstitial. **The harness MUST NOT use the 126-corpus's CF-Managed-Challenge sites (udemy, weather, economist, ft, ecosia, quora, openai)** — Managed Challenge is a *different* POST flow (orchestrator-based, no `jschl-answer` endpoint).

Suggested target site: pick at v0.1.0 commit-time via the cookbook §3 onboarding playbook. Document the choice in the harness header comment + pin to a specific commit-time.

**POST endpoint pattern:** `https://<target>.com/cdn-cgi/challenge-platform/h/g/jschl-answer` — the classic JS Challenge answer submission. Regex: `(?i)/cdn-cgi/challenge-platform/h/[a-z]/jschl-answer`.

**Capture method:** **net-level**. The body is the *cleartext* computed JSC answer (`jschl_vc`, `jschl_answer`, `pass`) — no encryption to undo. Hook `op_fetch` and tee.

**Decryption needed?** No.

**Expected fields:**
- `jschl_vc` — the challenge-validation token (echoed back from the stub).
- `jschl_answer` — the computed numeric answer.
- `pass` — the per-challenge pass token (also echoed).
- Request `Referer` matching the original URL.

**Assertion criteria:**
- ≥ 1 POST observed matching the regex.
- The POST body contains all three field names.
- Response status 200 + `Set-Cookie: cf_clearance=…`.
- Subsequent GET of the original URL returns ≥ 50 KB body (the JSC cleared, the page loaded).

**Output path:** `~/projects/browser_oxide_internal/captures/cloudflare/<YYYY-MM-DD>_<target>/`.

**Run frequency:** per-fix on any CF-touching change. Weekly. Cloudflare JSC is the most stable of the harnesses — the body shape has been steady for years — so the capture is essentially a regression-only assertion.

---

### 3.5 Kasada — `kasada_tl_sensor_capture` *(post-v0.1.0)*

This is the **K2-DIFF** harness from `08 §Lever 1`. It's the named-and-spec'd post-v0.1.0 entry; the existing `kasada_error_blob_capture` test (also referenced in `08`) is a sibling — both produce per-Kasada-deployment diagnostic artifacts; the error-blob one is reactive (capture what the VM throws), the `/tl` sensor one is proactive (capture the full passive collection).

**Target site:** `https://www.hyatt.com/` — chosen because:
- Smallest stub (745 B), shortest navigate.
- Real-Chrome ground-truth already captured at `ab_harness/tl/hyatt.tl_body.bin` (36 KB decrypted plaintext per `08 §Phase 5`).
- canadagoose is the alternate target; per `08 §Phase 5`, `ab_harness/tl/canadagoose.pcap` (15 MB + `.keys`) is the alternate ground-truth.

**POST endpoint pattern:** `https://<target>/.well-known/.../tl` or `https://<target>/tl` (path varies per-tenant). Regex: `(?i)/tl$` paired with `Content-Type: application/json` + JS-derived caller (the `ips.js` loader).

**Capture method:** **JS-level**. The `omgtopkek` XOR (per `08 §Phase 1`) is applied client-side just before the POST. JS-level intercept captures the pre-XOR plaintext sensor body. Shim is the same one used for DataDome / Akamai (vendor-dispatched by URL regex).

**Decryption needed?** No (because we intercept pre-XOR). If a future run accidentally captures the post-XOR cipher-text (e.g. shim misses a path), the offline decoder at `docs/kasada_ips_analysis/scratch/decrypt_report.py` recovers plaintext.

**Expected fields:** the `ips.js` signal envelope per `08 §Phase 3` — the 16-field list (`bot1225`, `csc`, `kl`, `dpv`, `smc`, `sfc`, `sdt`, `nppm`, `fsc`, `npc`, `esd`, `wse`, `bfe`, `ao`, `cbf`, …). Plus the broader passive collection (~ 100+ navigator/screen/WebGL/audio fields).

**Assertion criteria:**
- ≥ 1 POST to `/tl` observed.
- The captured plaintext decodes (UTF-8 JSON parse succeeds).
- Field count ≥ 80 (sentinel — anything less means `ips.js` bailed before completing passive collection).
- **Field-diff vs `ab_harness/tl/hyatt.tl_body.bin`:** ≤ 20 fields differ in shape/value class. This is the K2-DIFF assertion that turns the harness into the Kasada fix-driver.

**Output path:** `~/projects/browser_oxide_internal/captures/kasada/<YYYY-MM-DD>_hyatt/`.

```
~/projects/browser_oxide_internal/captures/kasada/2026-05-24_hyatt/
├── stub.html
├── ips_js.sha256
├── tl.body.plaintext.json   # our engine's plaintext (JS-intercepted)
├── tl.body.cipher.bin       # cipher-text (for offline decode if needed)
├── tl.response.json
├── diff_vs_real_chrome.md   # auto-generated K2-DIFF output
├── set_cookies.json
├── fetches.json
└── meta.json
```

**Run frequency:** post-v0.1.0 weekly + per-fix on any Kasada-relevant change (CSS calc evaluator at `crates/css_values/src/types/length.rs`, `_maskAsNative` per Lever 3, any new bootstrap surface). Quarterly review of `diff_vs_real_chrome.md` accumulated history.

---

### 3.6 PerimeterX — `px_sensor_capture` *(post-v0.1.0)*

**Target site:** zillow is **NOT** a valid target — BO passes zillow (the documented BO-vs-Camoufox advantage per `12 §1.1`, `27 §2`), and the harness needs a site that EXERCISES the PerimeterX sensor on a non-trivial path. Pick from the cookbook §2.6 list of PerimeterX-protected sites; candidates documented in [`thewebscrapingclub Bypassing PerimeterX 3`](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3) include sites that issue the PaH (Press-and-Hold) widget reliably.

Suggested target: pick at harness implementation time from a PerimeterX-tenant publisher / e-commerce site that triggers the sensor on every navigate (PerimeterX scoring sensitivity varies enormously per-tenant — pick one that does sensor-on-every-page).

**POST endpoint pattern:** the `/xhr/api/v<n>/collector` or `/init.js` host POST. Regex: `(?i)(perimeterx\.net|/xhr/api/v\d+/collector|/init\.js)`.

**Capture method:** **JS-level**. PerimeterX encrypts via a per-version envelope (v3 is most common). Pre-encryption hook captures the cleartext sensor body. Shim is the same multi-vendor URL-dispatched one used for Kasada / DataDome / Akamai.

**Decryption needed?** No at capture-time; offline decoder per [`MiddleSchoolStudent/PerimeterX-solver`](https://github.com/MiddleSchoolStudent/PerimeterX-solver) for any cipher-text-only captures.

**Expected fields:** per [`thewebscrapingclub PerimeterX 3`](https://substack.thewebscraping.club/p/the-lab-56-bypassing-perimeterx-3) — fingerprint envelope similar in shape to Kasada (~ 200 signals), behavioural-timing trace, plus the PerimeterX-specific identifiers (`_pxhd`, `_pxvid`).

**Assertion criteria:**
- ≥ 1 POST observed matching the regex.
- Captured payload decodes.
- Field count ≥ 80.
- Cookie lifecycle: `_pxhd` set on first nav (persistent), `_pxvid` set on first nav (persistent), `_px3` set within ≤ 5 s of nav-start (short-lived clearance).

**Output path:** `~/projects/browser_oxide_internal/captures/perimeterx/<YYYY-MM-DD>_<target>/`.

**Run frequency:** post-v0.1.0 weekly + per-fix.

---

### 3.7 Summary matrix

| Harness | v0.1.0 | Target | POST | Method | Decrypt | Min fields | Cadence |
|---|---|---|---|---|---|--:|---|
| `aws_waf_challenge_capture` | ✅ | amazon-de | `/inputs`, `/verify` | net-level | offline | (cipher-text only at ship) | per-fix + weekly |
| `datadome_challenge_capture` | ✅ | etsy | `/captcha/check` | JS-level | none (intercept pre-enc) | 50 | per-fix + weekly |
| `akamai_sensor_data_capture` | ⏳ | adidas firefox | `/akam/13/*` | JS-level | TEA-CBC (offline) | 100 | per-fix + monthly |
| `cloudflare_jschl_capture` | ✅ | TBD JSC site | `/cdn-cgi/.../jschl-answer` | net-level | none | 3 | per-fix + weekly |
| `kasada_tl_sensor_capture` | ⏳ | hyatt | `/tl` | JS-level | XOR-omgtopkek (none with intercept) | 80 | per-fix + weekly |
| `px_sensor_capture` | ⏳ | TBD PX site | `/xhr/api/v3/collector` | JS-level | offline | 80 | per-fix + weekly |

---

## 4. Run cadence integration with chapter 14

The harnesses slot into the existing L1-L5 validation pyramid (`14 §Three layers of validation`):

### 4.1 L1 — unit / integration

Every per-vendor harness is an `#[ignore]`-by-default integration test in `crates/browser/tests/per_vendor_*.rs` (one file per vendor — `per_vendor_aws_waf.rs`, `per_vendor_datadome.rs`, etc.). Per `CLAUDE.md` they stay `#[ignore]` because they need network + a live target — CI does not run them in the default workflow.

Local-developer invocation pattern (matches the `kasada_error_blob_capture` template):

```bash
cargo test -p browser --test per_vendor_aws_waf aws_waf_challenge_capture \
    -- --ignored --test-threads=1 --nocapture
```

The standard regression gate before any PR (per `14 §Regression gate before any PR`) does NOT block on per-vendor harnesses — they're the L2 follow-up.

### 4.2 L2 — single-site capture per fix

This is where the per-vendor harnesses replace the manual `sweep_metrics --capture` workflow. Per `14 §L2`, the loop becomes:

1. Capture pre-fix state: `cargo test --ignored <vendor>_capture`. Artifacts land in `~/projects/browser_oxide_internal/captures/<vendor>/<YYYY-MM-DD>_<site>/`.
2. Apply fix.
3. Capture post-fix state. Compare artifacts.
4. The `DIFF.md` auto-update mechanism (§5 below) writes the per-fix delta.

The "per-fix" interpretation is per-relevant-vendor — a CSS calc evaluator change exercises Kasada; a CSP relax change exercises DataDome; an `op_fetch` header-ordering change exercises AWS WAF + Cloudflare.

### 4.3 L4 — nightly

The nightly sweep workflow (`14 §L4`, `.github/workflows/sweep-nightly.yml`) gains a new job step:

```yaml
- name: Per-vendor capture sweep
  timeout-minutes: 30
  env:
    CAPTURES_DIR: ${{ runner.temp }}/captures
  run: |
    # Each harness is a separate cargo test invocation
    for harness in aws_waf_challenge_capture datadome_challenge_capture cloudflare_jschl_capture; do
      cargo test -p browser --test per_vendor_${harness%%_*} $harness \
          -- --ignored --test-threads=1 --nocapture \
          || echo "::warning::Per-vendor harness $harness failed (may be vendor-side)"
    done
- name: Upload captures
  uses: actions/upload-artifact@v4
  with:
    name: per-vendor-captures-${{ github.run_id }}
    path: ${{ runner.temp }}/captures/
```

The captures are uploaded as an artifact + (separately, asynchronously) committed to the internal repo on the `captures` branch. The commit step uses a deploy key scoped to that branch only — no main-line write permission.

### 4.4 L5 — quarterly

Review captured diffs across the rolling 90-day window. Each vendor's `DIFF.md` accumulates one entry per nightly run:

```
~/projects/browser_oxide_internal/captures/aws_waf/DIFF.md
  ├── 2026-05-24 → 2026-05-25: no field-set change, body size +12 B
  ├── 2026-05-25 → 2026-05-26: 1 new field `cb_extra`, body size +84 B
  ├── 2026-05-26 → 2026-06-01: no change
  ├── ...
  └── 2026-08-22 → 2026-08-23: ⚠ field `gokuProps.iv` rotated key length 12→16
```

Quarterly review pulls `DIFF.md` and identifies rotations that warrant a fix (e.g. AWS rolled their PoW from SHA-256 to NetworkBandwidth — engine needs to add a variant). Aligns with `14 §L5 weekly + before merge of any significant fix` cadence.

---

## 5. Output organisation

The internal-repo directory tree:

```
~/projects/browser_oxide_internal/captures/
├── aws_waf/
│   ├── 2026-05-24_amazon_de/
│   │   ├── stub.html
│   │   ├── challenge.js.sha256
│   │   ├── inputs.body.bin
│   │   ├── inputs.headers.json
│   │   ├── verify.body.bin           # if observed
│   │   ├── verify.headers.json
│   │   ├── set_cookies.json
│   │   ├── fetches.json
│   │   └── meta.json
│   ├── 2026-06-01_amazon_de/         # weekly snapshot
│   ├── 2026-06-08_amazon_de/
│   ├── ...
│   └── DIFF.md                       # running log, append-only
├── datadome/
│   ├── 2026-05-24_etsy/
│   ├── ...
│   └── DIFF.md
├── cloudflare/
│   ├── 2026-05-24_<target>/
│   ├── ...
│   └── DIFF.md
├── akamai_bmp/
│   └── DIFF.md
├── kasada/
│   └── DIFF.md
└── perimeterx/
    └── DIFF.md
```

Rules:

1. **One subdir per (date × site)** — never overwrite. The date prefix in the directory name guarantees ordering.
2. **`DIFF.md` is the only file at the vendor-root level** — append-only, ~ 1 line per nightly run, formatted as `(prev_date → this_date): <human-readable delta summary>`.
3. **All artifacts are committed to the internal repo via the `captures` branch** — deploy key scoped to that branch only, never merged to main. The branch grows unbounded (~ 100 KB per nightly per vendor × 6 vendors × 365 days ≈ 220 MB/yr — acceptable).
4. **`meta.json` is the join key** — schema:
   ```json
   {
     "harness": "aws_waf_challenge_capture",
     "vendor": "aws_waf",
     "target_url": "https://www.amazon.de/",
     "profile": "chrome_148_macos",
     "bo_commit": "8b12977",
     "capture_at": "2026-05-24T03:00:00Z",
     "harness_version": "v0.1.0",
     "ms_total_nav": 9742,
     "ms_first_post": 1820,
     "n_posts_intercepted": 2,
     "outcome": "blocked"     // {"passed", "blocked", "vendor-error", "engine-error"}
   }
   ```
5. **The `DIFF.md` generator** (`tools/per_vendor_diff.py` — post-v0.1.0) walks two adjacent timestamped subdirs and emits a 1-line summary. The summary covers: field-set additions/removals, body-size delta, outcome change, status-code change.

---

## 6. Implementation skeleton (Rust)

Reference implementation shape for one harness — `aws_waf_challenge_capture`. The other five follow the same pattern with vendor-specific regex + decoder hooks.

```rust
// crates/browser/tests/per_vendor_aws_waf.rs
//! AWS WAF Challenge capture harness. See docs/releases/v0.1.0-parity/
//! 34_PER_VENDOR_TEST_HARNESS.md §3.1 for the spec.

use std::path::PathBuf;
use std::time::Instant;

const TARGET_URL: &str = "https://www.amazon.de/";
const TARGET_NAME: &str = "amazon_de";
const VENDOR_DIR: &str = "aws_waf";
const POST_REGEX: &str = r"(?i)\.token\.awswaf\.com/.*/(inputs\?client=browser|verify|report)$";

#[tokio::test(flavor = "current_thread")]
#[ignore]
async fn aws_waf_challenge_capture() {
    // 1. Per-test output dir.
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = capture_root()
        .join(VENDOR_DIR)
        .join(format!("{date}_{TARGET_NAME}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir");

    // 2. Install a net-level capture sink + URL-regex filter.
    let sink = browser::capture::install_per_vendor_sink(
        out_dir.clone(),
        POST_REGEX,
        browser::capture::CaptureLevel::Net,
    );

    // 3. Run the production navigate path.
    let profile = stealth::presets::chrome_148_macos();
    let t0 = Instant::now();
    let res = browser::Page::navigate(TARGET_URL, profile, 3).await;
    let ms = t0.elapsed().as_millis() as u64;

    // 4. Write stub.html (the first response body, before any retry).
    if let Ok(page) = &res {
        std::fs::write(out_dir.join("stub.html"), sink.first_response_body())
            .expect("write stub");
    }

    // 5. Flush the sink — writes inputs.body.bin, verify.body.bin, set_cookies.json,
    //    fetches.json, meta.json.
    sink.flush(ms, &res);

    // 6. Update the vendor-level DIFF.md.
    browser::capture::append_diff_entry(
        capture_root().join(VENDOR_DIR).join("DIFF.md"),
        &out_dir,
    );

    // 7. Assertions (the regression gate).
    let inputs_body = std::fs::read(out_dir.join("inputs.body.bin"))
        .expect("inputs.body.bin must exist (POST /inputs was observed)");
    assert!(inputs_body.len() >= 200, "inputs.body shorter than 200 B");

    let stub_html = std::fs::read_to_string(out_dir.join("stub.html"))
        .expect("stub.html");
    assert!(stub_html.contains("AwsWafIntegration"));
    assert!(stub_html.contains("gokuProps"));

    // 8. Sentinel: if a verify.body.bin AND an aws-waf-token cookie were both
    //    observed, amazon-de unexpectedly passed; surface to maintainers.
    let cookies = std::fs::read_to_string(out_dir.join("set_cookies.json"))
        .unwrap_or_default();
    if std::path::Path::new(&out_dir.join("verify.body.bin")).exists()
        && cookies.contains("aws-waf-token=")
    {
        eprintln!(
            "::warning::aws_waf_challenge_capture: amazon-de PASSED unexpectedly. \
             Re-pin to imdb if Amazon flipped the WAF rule."
        );
    }
}

fn capture_root() -> PathBuf {
    std::env::var("BROWSER_OXIDE_CAPTURES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap()
                .join("projects/browser_oxide_internal/captures")
        })
}
```

Per-vendor variations:

- **DataDome / Kasada / Akamai / PerimeterX**: swap `CaptureLevel::Net` for `CaptureLevel::Js` — the sink wires a `stealth_bootstrap.js`-installed `XMLHttpRequest.send` + `fetch` shim instead of a `fetch_ext.rs` hook.
- **Cloudflare**: keep `CaptureLevel::Net`, but the assertion set is the 3-field-name check (`jschl_vc`, `jschl_answer`, `pass`) parsed from the captured form-encoded body.

The `browser::capture` module gains a new entry point `install_per_vendor_sink` that wraps the existing sink machinery from `04 §Rust changes — new browser::capture module` with URL filtering + the per-vendor output layout.

---

## 7. Acceptance for v0.1.0

- [ ] At least 3 per-vendor harnesses implemented and runnable:
  - [ ] `aws_waf_challenge_capture` — single-command runnable, writes the spec'd artifacts.
  - [ ] `datadome_challenge_capture` — JS-level intercept working, plaintext payload captured.
  - [ ] `cloudflare_jschl_capture` — target site pinned + documented in the harness header.
- [ ] `browser::capture::install_per_vendor_sink` + `append_diff_entry` exist in `crates/browser/src/capture.rs`.
- [ ] All 3 harnesses integrated into the nightly CI workflow per chapter 14 (sweep-nightly.yml addition).
- [ ] Capture artifacts checked into `~/projects/browser_oxide_internal/captures/` via the `captures` deploy-key branch.
- [ ] Per-vendor `DIFF.md` populated by automation — at least 7 nightly entries before v0.1.0 tag.
- [ ] CONTRIBUTING.md mentions the per-vendor capture pattern as the L2 of `14`.
- [ ] Test file paths exist:
  - `crates/browser/tests/per_vendor_aws_waf.rs`
  - `crates/browser/tests/per_vendor_datadome.rs`
  - `crates/browser/tests/per_vendor_cloudflare.rs`

## 8. Acceptance post-v0.1.0

- [ ] `akamai_sensor_data_capture` — TEA-CBC decoder ships, asserts ≥ 100 fields.
- [ ] `kasada_tl_sensor_capture` — the K2-DIFF harness from `08 §Lever 1` is the canonical Kasada one (supersedes the legacy `kasada_error_blob_capture`).
- [ ] `px_sensor_capture` — PerimeterX target site pinned + documented.
- [ ] `tools/per_vendor_diff.py` generator ships (replaces manual `DIFF.md` append).
- [ ] Quarterly review process documented in `00_README.md` operational runbook section.

---

## 9. Why this matters — the regression model

The cost model for vendor-side rotation:

| Scenario | Without per-vendor harness | With per-vendor harness |
|---|---|---|
| Vendor rotates an encryption key or PoW algorithm | Sweep pass rate silently drops by N sites over a week; takes 1-3 days of debugging to identify which vendor's challenge changed shape, then 1-3 days more to recover. | `DIFF.md` flags the rotation within 24 h; field-set diff names the changed component; fix scoped to that one component. |
| Bootstrap-surface regression (`_maskAsNative` audit miss) | Some sites fail; sweep is the only signal; no per-site explanation. | Kasada/PX harness asserts ≥ 80 fields → if a new bootstrap masking gap drops the field count, the harness fails loud. |
| TLS-class regression (boring2 cipher list churn) | AWS WAF + Cloudflare both flip on the next sweep; root-cause requires `tls_fingerprint_probe.rs` instrumentation. | AWS WAF + CF harnesses fail; the encrypted body length delta in `DIFF.md` is the signal. |
| New CDP-or-similar sniff added by a vendor | Affects only Patchright / Playwright family; BO unaffected; signal lost. | Cookie-lifecycle assertion in the harness reveals if BO is on the wrong side of the new sniff. |

The harnesses cost ~ 1 dev-day per vendor to write + ~ 30 min/day of CI wall-clock + ~ 220 MB/yr internal-repo storage. The recovery savings on a single rotation event recover that cost.

---

## 10. Files referenced

- `crates/browser/tests/chrome_compat.rs` — the historical home of `kasada_error_blob_capture` (referenced in `08_KASADA_FRONTIER.md §Adjacent files`).
- `crates/browser/tests/per_vendor_aws_waf.rs` — NEW (v0.1.0).
- `crates/browser/tests/per_vendor_datadome.rs` — NEW (v0.1.0).
- `crates/browser/tests/per_vendor_cloudflare.rs` — NEW (v0.1.0).
- `crates/browser/tests/per_vendor_akamai.rs` — NEW (post-v0.1.0).
- `crates/browser/tests/per_vendor_kasada.rs` — NEW (post-v0.1.0, supersedes `kasada_error_blob_capture`).
- `crates/browser/tests/per_vendor_perimeterx.rs` — NEW (post-v0.1.0).
- `crates/browser/src/capture.rs` — gains `install_per_vendor_sink`, `append_diff_entry`, `CaptureLevel`. See `04 §Rust changes — new browser::capture module` for the base module spec.
- `crates/js_runtime/src/extensions/fetch_ext.rs:231-381` — net-level intercept hook target (per `04 §Files referenced`).
- `crates/js_runtime/src/js/stealth_bootstrap.js` — JS-level intercept shim install location.
- `crates/browser/src/page.rs:1054-1069` — existing vendor-detect log lines (AWS WAF / DataDome / wbaas markers — guides regex choices for the harnesses).
- `crates/browser/src/classify.rs:81-156` — vendor-marker tables (guides target-site picks).
- `docs/kasada_ips_analysis/scratch/decrypt_report.py` — XOR-omgtopkek decoder (Kasada harness offline path).
- `ab_harness/tl/hyatt.tl_body.bin` — real-Chrome `/tl` ground-truth (Kasada K2-DIFF reference).
- `~/projects/browser_oxide_internal/captures/` — output root.
- `~/projects/browser_oxide_internal/captures/<vendor>/DIFF.md` — per-vendor rotation log.
- `.github/workflows/sweep-nightly.yml` — gains per-vendor capture job (per `14 §CI integration`).
- `tools/per_vendor_diff.py` — post-v0.1.0 DIFF.md generator.
- `tools/aws_waf_decode.py` — post-v0.1.0 offline decoder for AWS WAF cipher-text.

---

## 11. Cross-references

- `08_KASADA_FRONTIER.md` — the prototype harness pattern + K2-DIFF specification.
- `14_TESTING_VALIDATION.md` — the L1-L5 validation pyramid this slots into.
- `04_TOOLING_SPEC.md` — `--capture` mode + `browser::capture` module foundation.
- `06_AWS_WAF_SOLVER.md` — the AWS WAF deep dive (capture context for §3.1).
- `07_DATADOME_PRIMITIVES.md` — the DataDome primitives + iframe-rematerialise context for §3.2.
- `18_ANTI_BOT_VENDOR_COOKBOOK.md` — vendor marker tables driving the URL regex choices.
- `25_CLOUDFLARE_DEEP.md` — Cloudflare context for §3.4.
- `26_AKAMAI_BMP_DEEP.md` — Akamai BMP context for §3.3.
- `35_IMPERVA_ABP.md` — sibling vendor doc; once written, gains its own `imperva_reese84_capture` entry in §3.7.
