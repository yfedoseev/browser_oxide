# 33 — Quarterly probe-rotation log

**Status:** operational artifact (template + cadence + live log)
**Audience:** any contributor running the quarterly capture-and-diff cycle, or triaging a pass-rate drop in the nightly sweep.
**Companion docs:** `14_TESTING_VALIDATION.md §L5` (nightly drift signals — the early warning), `18_ANTI_BOT_VENDOR_COOKBOOK.md §5` (the existing cadence summary table — this chapter is its operational expansion), `04_TOOLING_SPEC.md` (capture tooling), `06_AWS_WAF_SOLVER.md §1` (the per-vendor capture recipe for AWS WAF), `28_AWS_WAF_EXTENDED.md §9` (forward-looking AWS WAF changes — what to watch for).

**One-paragraph thesis:** Anti-bot vendors rotate their probes on cadences from daily (DataDome key rotation) to quarterly (Kasada bytecode VM). Without a tracking artifact, drift surprises us — we discover the rotation only when nightly sweep numbers drop and we don't know which vendor to blame. This chapter is the artifact: a per-vendor expected-cadence table, a quarterly review checklist, a log skeleton for entries when rotations are observed, an automation opportunity for CI-driven drift detection, and the link to `18_ANTI_BOT_VENDOR_COOKBOOK.md` updates that must follow.

---

## 1. Why this matters

Three failure modes the rotation-tracking artifact prevents:

### 1.1 Silent solver decay

A working `vendor_solvers::AwsWafSolver` (per chapter 06 Alt B) succeeds at 7/10 amazon-de strict-pass on day 0. On day 30, AWS rotates the `challenge.js` minifier seed; our regex-based bail detection (chapter 06 §3.C) silently no-ops. Pass rate drops to 1/10. The nightly sweep flags the drop via `14_TESTING_VALIDATION.md §L5` Yellow signal ("4-8 sites concentrated in one vendor's cluster") — but without a rotation log, the contributor doesn't know whether to blame:

- A rotation (re-capture + diff)
- A regression in our own engine (bisect against last green build)
- AWS server-side risk-model update (no client-side rotation visible; just wait it out)

The rotation log lets the contributor answer "rotation vs regression" in five minutes instead of an hour of bisection.

### 1.2 Browser-bump cascade

When Chrome 149 ships in 2026-Q4, our existing `chrome_148_macos` profile is one Chrome release behind. Major-version skew is usually fine (UA strings ages gracefully, sec-ch-ua brand list rolls); minor compliance drift starts:

- The reported `chromium` brand version (`148.0.0.0`) is now obviously a major-version-old browser.
- New JS surface that 149 ships (e.g. a new `navigator.*` property) won't be present in our stealth bootstrap.
- Vendors that fingerprint exact `sec-ch-ua` brand-version will start flagging us as "outdated Chrome".

The rotation log's "Browser bumps" section tracks Chrome / Firefox / Safari release dates and our planned profile-update windows. Per `19_PROFILE_EXPANSION_PLAN.md` we target a quarterly profile refresh cycle; this log is where we record when we did the refresh and what changed.

### 1.3 Public-solver breakage

When an open-source vendor solver breaks (because the vendor rotated their protocol), our private `vendor_solvers` implementation is likely to break for the same reason on a similar timeline. Tracking when [xKiian/awswaf](https://github.com/xKiian/awswaf), [glizzykingdreko/Datadome-Captcha-Deobfuscator](https://github.com/glizzykingdreko/Datadome-Captcha-Deobfuscator), or [xvertile/akamai-bmp-generator](https://github.com/xvertile/akamai-bmp-generator) last shipped a fix gives us early warning of upcoming impact on our own solvers. The log's "Public solver status" column captures this.

---

## 2. Per-vendor rotation cadence (the canonical table)

This expands `18_ANTI_BOT_VENDOR_COOKBOOK.md §5` with sources and per-element granularity.

### 2.1 The cadence table

| Vendor | Rotation cadence | What rotates | Last documented major change | Public-solver activity | Source |
|---|---|---|---|---|---|
| **AWS WAF — `challenge.js` obfuscation** | Monthly-ish | Minifier seed, identifier mangling, string-array indirection | 2026-02 — [xKiian/awswaf](https://github.com/xKiian/awswaf) update note (per [roundproxies bypass](https://roundproxies.com/blog/bypass-aws-waf/) "the information found was recently updated in February 2026") | Active (xKiian, neiii, Switch3301, jonathanyly) | [roundproxies](https://roundproxies.com/blog/bypass-aws-waf/), [xKiian commits](https://github.com/xKiian/awswaf/commits/main) |
| **AWS WAF — PoW algorithm** | Stable (12+ months) | HashcashScrypt / SHA256 / NetworkBandwidth split unchanged since 2024 | (none observed) | Implemented in all 3+ solvers | [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) |
| **AWS WAF — Bot Control `TGT_ML_*` rules** | Continuous (server-side) | ML thresholds; invisible from outside | 2026-Q1 — `Version_4.0` added Web Bot Authentication (WBA) | n/a — AWS-managed | [AWS Managed Rules changelog](https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-changelog.html), [Bot Control rule group](https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-bot.html) |
| **DataDome — `dd-script.js` keys** | Daily (every day at a specific UTC time) | Six-character random keys in the signals dictionary | Ongoing — `DataDome implements a daily rotation of the files` | Active (glizzykingdreko maintains deobfuscator) | [glizzykingdreko Medium](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21), [DataDome key rotation docs](https://docs.datadome.co/docs/key-rotation) |
| **DataDome — WASM blob** | Daily (key only); structural changes ~quarterly | Encryption keys (daily); WASM opcodes (rarer) | 2026-02 — DataDome announced VM-based obfuscation: `On a regular schedule, every aspect of their obfuscation changes: variable names, code structure, encryption keys, and now, VM opcodes and interpreter architecture.` | Glizzykingdreko deobfuscator tracks updates | [DataDome VM-based obfuscation changelog](https://datadome.co/changelog/vm-based-obfuscation/), [Security Boulevard](https://securityboulevard.com/2026/02/datadome-releases-vm-based-obfuscation-the-next-evolution-in-client-side-detection-security/) |
| **DataDome — iframe URL** | Daily | Path segment of `captcha-delivery.com/captcha/?...` | Continuous | Active | [glizzykingdreko Medium](https://medium.com/@glizzykingdreko/breaking-down-datadome-captcha-waf-d7b68cef3e21) |
| **Akamai BMP — `sensor_data` PRNG seed** | Per-tenant; weekly-ish | PRNG seed inside the bootstrap script | Rolling | Active (xvertile/akamai-bmp-generator) | [glizzykingdreko Akamai v3 deep-dive](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784), [xvertile/akamai-bmp-generator](https://github.com/xvertile/akamai-bmp-generator) |
| **Akamai BMP — script file hash** | Per-tenant; weekly-ish | The hash that feeds v3 cookie-keyed encryption | Rolling — `v3 encryption relies heavily on real-time JavaScript file hashes` | Active | [glizzykingdreko Akamai v3](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784) |
| **Akamai BMP — major version (v2 → v3)** | Rare; multi-year | Encryption scheme (TEA-CBC → cookie-keyed PRNG) | 2023 — v3 rollout | n/a | [glizzykingdreko Akamai v3](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784) |
| **Akamai sec-cpt bundle** | Per-tenant; rare | The PoW preimage scheme | (none observed since landing) | Self-solves in our V8 today | `memory/state_2026_05_16_phase5_datadome.md` |
| **Cloudflare — Managed Challenge orchestrator** | Weekly | The JS at `/cdn-cgi/challenge-platform/h/b/jsd/...` | 2025-10 — CF deployed an update that incorrectly blocked legitimate Chrome visitors briefly | Closed-source; PW family loses everything; BO chrome wins | [Cloudflare changelogs](https://developers.cloudflare.com/changelog/), [October 2025 incident](https://isdown.app/status/cloudflare/incidents/465013-turnstile-challenges-incorrectly-blocking-human-visitors) |
| **Cloudflare — Turnstile widget** | Continuous; new features in changelog | Public callback / config flags (turnstile.ready, language, retry, timeout-callback, expired-callback) | Ongoing per [Turnstile changelog](https://developers.cloudflare.com/turnstile/changelog/) | n/a | [Turnstile changelog](https://developers.cloudflare.com/turnstile/changelog/) |
| **Kasada — `ips.js` bytecode + opcode table** | Quarterly (major); weekly (minor probes) | Custom bytecode VM contents; new opcodes; new fingerprint probes | Continuous; last major opcode reshuffle tracked in `~/projects/browser_oxide_internal/docs/...` | Active but lossy ([umasii/ips-disassembler](https://github.com/umasii/ips-disassembler), [Humphryyy/Kasada-Deobfuscated](https://github.com/Humphryyy/Kasada-Deobfuscated)) | [ScrapeBadger Kasada](https://scrapebadger.com/kasada-bypass), [Scrapfly Kasada](https://scrapfly.io/blog/posts/how-to-bypass-kasada-anti-scraping-waf), [opcodes.fr part 1](https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1) |
| **Kasada — wrapper envelope** | Stable (cracked, per `memory/kasada_wrapper_cracked_and_remaining_leaks.md`) | XOR(omgtopkek) wrapper unchanged | (none observed) | n/a | `memory/kasada_wrapper_cracked_and_remaining_leaks.md` |
| **PerimeterX / HUMAN — sensor JS obfuscation** | Weekly | Variable mangling, AST shape | Ongoing | Active (community trackers, no canonical) | [ZenRows PerimeterX](https://www.zenrows.com/blog/perimeterx-bypass), [Scrapfly PerimeterX](https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping) |
| **PerimeterX — `_px3` lifetime** | Stable (60 s) | Short token TTL = constant refresh load | (none observed) | n/a | `18 §1.4` |
| **Imperva Incapsula — `/_Incapsula_Resource` script** | Monthly | Obfuscation layer; sensor field set is stable | (none observed) | Closed | [Scrapfly Imperva](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping) |
| **Reblaze / Thales** | Unknown (sample-thin) | (insufficient public data) | (none observed) | n/a | n/a |
| **Sucuri** | Rare (~quarterly+) | Obfuscation layer | (none observed) | n/a | (no major public reverse-eng since 2023) |

### 2.2 How to read the "Source" column

Sources have three tiers:

1. **Vendor-published docs** (DataDome key rotation, AWS Managed Rules changelog, Cloudflare changelog, Turnstile changelog) — authoritative; what the vendor *says* they do.
2. **Public reverse-eng writeups** (glizzykingdreko, scrapfly, scrapebadger, opcodes.fr, roundproxies, zenrows) — empirical; what reverse engineers *observed*. Treat as ground truth unless contradicted by tier 1.
3. **GitHub solver commit history** (xKiian/awswaf, glizzykingdreko/Datadome-Captcha-Deobfuscator, xvertile/akamai-bmp-generator) — temporal; *when* protocol changes happened, inferred from when the solver had to ship a fix. Indirect but useful for cadence verification.

When a row in §2.1 references a tier-2/3 source, cross-check against tier 1 when you do the quarterly review. If they diverge, the vendor docs are wrong (vendors don't always document what they ship) — record the divergence in §4.

### 2.3 The "stable vs rotating" axis

Within each vendor, identify what is **stable** and what **rotates**. Stable = our solver is a one-time investment. Rotating = our solver carries maintenance cost.

| Vendor | Stable | Rotating |
|---|---|---|
| AWS WAF | PoW algorithm split (3 types); endpoint shape (/inputs, /verify, /report); token cookie name | challenge.js obfuscation layer; ML thresholds (invisible) |
| DataDome | Sensor field set; iframe origin (`captcha-delivery.com`); cookie name | Daily keys; iframe path segment; WASM bytecode |
| Akamai BMP | Sensor field set (per major version); endpoint URL pattern | PRNG seed; script file hash (for v3 encryption) |
| Cloudflare | Cookie names (`cf_clearance`, `__cf_bm`); orchestrator URL pattern | Orchestrator JS (weekly); Turnstile features |
| Kasada | Wrapper envelope (XOR omgtopkek); cookie/header names (`x-kpsdk-*`) | Bytecode (quarterly major; weekly minor probes) |
| PerimeterX | Token lifetime; cookie name set (`_px3`, `_pxhd`, `_pxvid`) | Sensor JS obfuscation; collector endpoint path |
| Imperva | `_Incapsula_Resource` URL pattern; cookie name set | Script obfuscation |

This decomposition tells us where to spend solver-engineering time. Targeting the **stable** part is durable; relying on the **rotating** part means perpetual carry cost.

---

## 3. Quarterly review template

The quarterly checklist a contributor runs during the dedicated rotation-review session. Typical duration: 1 contributor-day if no major rotations; 3-5 days if a major rotation hit one vendor.

### 3.1 Pre-flight

```bash
# Set the quarter token (Q1=Jan-Mar, Q2=Apr-Jun, Q3=Jul-Sep, Q4=Oct-Dec)
QUARTER=$(date +%Y_Q$((($(date +%-m)-1)/3+1)))
PREV_QUARTER=$(date -d "3 months ago" +%Y_Q$((($(date -d "3 months ago" +%-m)-1)/3+1)))

# Capture dir (private repo — captures often contain vendor-specific bypass artifacts)
CAP_DIR=~/projects/browser_oxide_internal/docs/rotation_$QUARTER
mkdir -p $CAP_DIR/{captures,diffs,notes}
```

### 3.2 Capture pass — pull current state for every vendor

```bash
# Per-vendor anchor sites (one per vendor; doesn't need to be the whole 126-corpus)
declare -A VENDOR_ANCHORS=(
  [aws-waf]="https://www.amazon.de/"
  [datadome]="https://www.etsy.com/"
  [akamai-bmp]="https://www.adidas.com/us"
  [akamai-sec-cpt]="https://www.homedepot.com/"
  [cloudflare-managed]="https://www.openai.com/"
  [cloudflare-turnstile]="https://www.linkedin.com/"  # uses Turnstile widget
  [kasada]="https://www.canadagoose.com/"
  [perimeterx]="https://www.zillow.com/"
  [imperva]="https://www.example-imperva-tenant.com/"  # replace with current tenant
)

for vendor in "${!VENDOR_ANCHORS[@]}"; do
  url=${VENDOR_ANCHORS[$vendor]}
  echo "=== $vendor — $url ==="
  curl -sS -A 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' \
       -H 'Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8' \
       -H 'Accept-Language: en-US,en;q=0.9' \
       -D $CAP_DIR/captures/$vendor.headers \
       "$url" \
       -o $CAP_DIR/captures/$vendor.html
  
  HASH=$(sha256sum $CAP_DIR/captures/$vendor.html | cut -c1-12)
  SIZE=$(wc -c < $CAP_DIR/captures/$vendor.html)
  echo "  hash=$HASH size=$SIZE"
done
```

### 3.3 Capture pass — pull each vendor's subresource(s)

```bash
# AWS WAF — pull challenge.js per chapter 06 §1.2
CHL_URL=$(grep -oE 'https://[a-f0-9]+\.[a-f0-9]+\.[a-z0-9-]+\.token\.awswaf\.com/[^"]*challenge\.js' \
  $CAP_DIR/captures/aws-waf.html | head -1)
[ -n "$CHL_URL" ] && curl -sS -H "Referer: https://www.amazon.de/" "$CHL_URL" \
  -o $CAP_DIR/captures/aws-waf.challenge.js

# DataDome — pull dd-script.js
DD_URL=$(grep -oE 'https://[a-z0-9.-]+\.captcha-delivery\.com/[^"]*\.js' \
  $CAP_DIR/captures/datadome.html | head -1)
[ -n "$DD_URL" ] && curl -sS "$DD_URL" -o $CAP_DIR/captures/datadome.dd-script.js

# Kasada — pull ips.js
KASADA_URL=$(grep -oE '/ips\.js' $CAP_DIR/captures/kasada.html | head -1)
[ -n "$KASADA_URL" ] && curl -sS "https://www.canadagoose.com$KASADA_URL" \
  -o $CAP_DIR/captures/kasada.ips.js

# Cloudflare — pull the orchestrator JS
CF_URL=$(grep -oE '/cdn-cgi/challenge-platform/[^"]*\.js' \
  $CAP_DIR/captures/cloudflare-managed.html | head -1)
[ -n "$CF_URL" ] && curl -sS "https://www.openai.com$CF_URL" \
  -o $CAP_DIR/captures/cloudflare.platform.js

# Akamai BMP — find the akam/13 script
AK_URL=$(grep -oE '/akam/13/[^"]*' $CAP_DIR/captures/akamai-bmp.html | head -1)
[ -n "$AK_URL" ] && curl -sS "https://www.adidas.com$AK_URL" \
  -o $CAP_DIR/captures/akamai.bootstrap.js
```

### 3.4 Diff pass — diff against the prior quarter

```bash
for vendor in "${!VENDOR_ANCHORS[@]}"; do
  for ext in html headers challenge.js dd-script.js ips.js platform.js bootstrap.js; do
    cur=$CAP_DIR/captures/$vendor.$ext
    prev=$(echo $cur | sed "s/$QUARTER/$PREV_QUARTER/")
    [ -f "$cur" ] && [ -f "$prev" ] || continue
    
    cur_hash=$(sha256sum "$cur" | cut -c1-12)
    prev_hash=$(sha256sum "$prev" | cut -c1-12)
    
    if [ "$cur_hash" != "$prev_hash" ]; then
      echo "=== ROTATED: $vendor.$ext ($prev_hash → $cur_hash) ==="
      diff -u <(prettier --parser babel "$prev" 2>/dev/null || cat "$prev") \
              <(prettier --parser babel "$cur" 2>/dev/null || cat "$cur") \
          > $CAP_DIR/diffs/$vendor.$ext.diff
      wc -l $CAP_DIR/diffs/$vendor.$ext.diff
    fi
  done
done
```

### 3.5 Triage pass — for each rotation, classify

For every `$CAP_DIR/diffs/*.diff` file produced:

```
Open the diff. Classify:
  - COSMETIC: identifier mangling only (string-array renames, variable renames)
    → No solver change required. Note in log; move on.
  - SHALLOW STRUCTURAL: new function added, removed, or reordered
    → Likely-impact-low. Check if any of our regex / pattern matchers reference
      the changed names. If so, update them.
  - DEEP STRUCTURAL: new protocol endpoint, new request body field, new
    response header, new cookie name, new PoW algorithm
    → High impact. File issue; bump solver work into the active backlog.
  - PROBE ADDITION: new fingerprint property read (a `navigator.X` or
    `screen.Y` we don't emit, or emit differently)
    → Check BO emission point. If gap → add to chapter 28 §3 signal table
      with INVESTIGATE flag and log here.
```

The triage rubric maps to the §4 log entry's "Impact" field.

### 3.6 Re-run sweep — measure the rotation's real-world impact

```bash
# Full 3-run sweep per chapter 14 §L5 (this is what tells us if a rotation
# actually matters to pass-rates).
for run in 1 2 3; do
  for profile in chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos; do
    target/release/examples/sweep_metrics $profile /tmp/corpus_126.json \
      /tmp/rotation_check/${profile}_run${run}.json
  done
done

# Aggregate via tools/aggregate_sweep.py (per 14 §L5 §Aggregation logic)
python3 tools/aggregate_sweep.py /tmp/rotation_check/*.json \
  > $CAP_DIR/notes/sweep_post_rotation.json

# Compare against the last quarter's sweep
python3 tools/sweep_delta.py \
  ~/projects/browser_oxide_internal/sweeps/$PREV_QUARTER.json \
  $CAP_DIR/notes/sweep_post_rotation.json \
  > $CAP_DIR/notes/delta.txt
```

If the delta shows > -3 sites concentrated in one vendor → that vendor's rotation hurt us; the §3.5 triage already identified the likely cause; now prioritize the fix.

### 3.7 Update pass — sync cookbook + this log

For each rotation that warrants any solver / engine change:

1. **Update `18_ANTI_BOT_VENDOR_COOKBOOK.md`** with the new protocol details (the cookbook is the SETTLED knowledge; this log is the FORWARD signal — see §6).
2. **Update `28_AWS_WAF_EXTENDED.md §3` (or equivalent vendor deep-dive)** if a new signal was added.
3. **Update this log §4** with the dated entry.
4. **File issues** in the private `vendor_solvers` repo for any solver fix that needs work.
5. **If a major capture or A/B verifies the diagnosis** — push the captures to `~/projects/browser_oxide_internal/docs/rotation_$QUARTER/`.

---

## 4. Per-quarter log entries

This is the live log. Entries are appended at quarterly review time (§5 cadence). Skeleton:

```
## YYYY-QN (Month-Month)

### Summary
- Sweep delta vs prior quarter: <+N / -N strict-pass>
- Major rotations observed: <list of vendor/component>
- Solver-fix tasks generated: <count> issues filed

### <vendor>
- (date) — observed <COSMETIC | SHALLOW STRUCTURAL | DEEP STRUCTURAL | PROBE ADDITION>: <brief>
- Capture: `~/projects/browser_oxide_internal/docs/rotation_YYYY_QN/captures/<file>`
- Diff: `~/projects/browser_oxide_internal/docs/rotation_YYYY_QN/diffs/<file>.diff`
- Source citations: <cite vendor docs or solver-repo commit links>
- Impact on pass rate: <delta number from §3.6>
- Fix: <link to issue or "no action required">
- Cookbook update: <link to 18_… PR>
```

### 4.1 Initial log entry (2026-Q2, baseline)

This is the seed entry. Captures established 2026-05-24 as the "T0" baseline that all future deltas reference.

```
## 2026-Q2 (April-June) — BASELINE
Seeded 2026-05-24.

### Summary
- Sweep state at baseline: 108 strict-pass routed (per `01_CURRENT_STATE.md`)
- Major rotations observed: n/a — this is the initial capture
- Solver-fix tasks generated: n/a

### AWS WAF
- 2026-05-24 — initial capture committed:
  - `aws-waf.html` (anchor: amazon.de; 2011 B stub)
  - `aws-waf.challenge.js` (~50-150 KB; per chapter 06 §1.3)
  - hash recorded; subsequent quarters diff against this.
- Source: chapter 06 §1.
- Impact: baseline (4/8 AWS WAF cluster strict-pass per `27 §1`).
- Fix: chapter 06 Alt A/B/C tracked separately; no rotation work this quarter.

### DataDome
- 2026-05-24 — initial capture committed:
  - `datadome.html` (anchor: etsy.com; 1424 B stub)
  - `datadome.dd-script.js`
  - Note: WASM blob inside dd-script.js — daily key rotation expected; structural
    rotation per [DataDome VM-based obfuscation Feb 2026 changelog](https://datadome.co/changelog/vm-based-obfuscation/)
    has already shipped to most tenants.
- Source: `18 §2.2`, [DataDome key rotation docs](https://docs.datadome.co/docs/key-rotation).
- Impact: baseline (3/4 DataDome cluster strict-pass per `27 §1`).
- Fix: per `07_DATADOME_PRIMITIVES.md` Primitives 1+2+3; out of rotation scope.

### Akamai BMP
- 2026-05-24 — initial capture committed:
  - `akamai-bmp.html` (anchor: adidas.com)
  - `akamai.bootstrap.js` (the akam/13/* script)
  - sec-cpt fragment captured separately from homedepot — `akamai-sec-cpt.html`
- Source: `18 §2.3`, [glizzykingdreko Akamai v3 deep-dive](https://medium.com/@glizzykingdreko/akamai-v3-sensor-data-deep-dive-into-encryption-decryption-and-bypass-tools-da0adad2a784).
- Impact: baseline (1/3 Akamai BMP cluster strict-pass per `27 §1`).
- Fix: per `26_AKAMAI_BMP_DEEP.md`.

### Cloudflare
- 2026-05-24 — initial capture committed:
  - `cloudflare-managed.html` (anchor: openai.com)
  - `cloudflare.platform.js` (orchestrator JS at /cdn-cgi/challenge-platform/h/b/jsd/...)
- Note: October 2025 incident ([IsDown report](https://isdown.app/status/cloudflare/incidents/465013-turnstile-challenges-incorrectly-blocking-human-visitors))
  is in the historical record but predates our baseline.
- Source: `18 §2.5`, [Cloudflare Turnstile changelog](https://developers.cloudflare.com/turnstile/changelog/).
- Impact: baseline (CF Managed iphone-targeted cluster 7/7 BO routed per `27 §1`).
- Fix: per `25_CLOUDFLARE_DEEP.md`.

### Kasada
- 2026-05-24 — initial capture committed:
  - `kasada.html` (anchor: canadagoose.com; 740 B stub per `18 §2.4`)
  - `kasada.ips.js`
  - Live oracle: `ab_harness/tl/hyatt.tl_body.bin` (36 KB decrypted plaintext)
- Source: `08_KASADA_FRONTIER.md`, `memory/kasada_wrapper_cracked_and_remaining_leaks.md`.
- Impact: baseline (0/3 Kasada cluster strict-pass — open-source SOTA frontier).
- Fix: per `08_KASADA_FRONTIER.md §4` engine fix list.

### PerimeterX
- 2026-05-24 — initial capture committed:
  - `perimeterx.html` (anchor: zillow.com)
- Source: `18 §2.6`, [ZenRows PerimeterX bypass](https://www.zenrows.com/blog/perimeterx-bypass).
- Impact: baseline (1/1 zillow strict-pass routed — BO marquee advantage per `27 §2`).
- Fix: not required; we win this vendor.

### Imperva
- 2026-05-24 — n/a (no Imperva site in 126-corpus).
- Source: `18 §2.7`.
- Impact: cannot measure without a corpus site.
- Fix: add an Imperva tenant to the corpus in a future expansion (see `19_PROFILE_EXPANSION_PLAN.md`).

### Browser bumps
- 2026-05-24 — current profiles:
  - chrome_148_macos (Chrome 148, May 2026)
  - pixel_9_pro_chrome_148 (Chrome 148)
  - iphone_15_pro_safari_18 (Safari 18)
  - firefox_135_macos (Firefox 135)
- Next planned bump: Chrome 149 release (estimated Aug 2026); profile refresh
  targeted for 2026-Q3.
```

### 4.2 Future-quarter log slots (skeleton; fill at review time)

```
## 2026-Q3 (July-September)
- (to be filled at end of Q3)

## 2026-Q4 (October-December)
- (to be filled at end of Q4)

## 2027-Q1 (January-March)
- (to be filled at end of Q1)
```

### 4.3 Per-vendor cross-reference

When you add a Q3+ entry, also add a one-liner under the vendor's section in `18_ANTI_BOT_VENDOR_COOKBOOK.md §5`. Keep that table the SETTLED snapshot; this log is the WORKING history.

---

## 5. Cadence — when to run the quarterly review

### 5.1 Calendar cadence

The review runs **once per quarter**, ideally at the start of the next quarter so the just-ended quarter is closed out:

| Review window | Reviews quarter | Captures dated |
|---|---|---|
| Apr 1 - Apr 15 | 2026-Q1 (Jan-Mar) | 2026-04-01 |
| Jul 1 - Jul 15 | 2026-Q2 (Apr-Jun) | 2026-07-01 |
| Oct 1 - Oct 15 | 2026-Q3 (Jul-Sep) | 2026-10-01 |
| Jan 1 - Jan 15 | 2026-Q4 (Oct-Dec) | 2027-01-01 |

Two weeks is enough time for a single contributor to:
- Day 1: Capture pass (§3.2-3.3).
- Day 2: Diff pass (§3.4) + triage (§3.5).
- Day 3-7: Re-run sweep (§3.6) + fix any high-priority structural rotations.
- Day 8-10: Update cookbook + this log (§3.7).
- Day 11-14: Buffer / follow-up A/Bs for any close-call rotations.

If a quarter had no structural rotations (only cosmetic), the work is closer to 2 days.

### 5.2 Event-driven cadence — when to skip ahead

Don't wait for the quarterly window if any of these trigger:

| Trigger | Action |
|---|---|
| Nightly sweep `14 §L5` **Yellow** signal for 3+ consecutive nights, concentrated in one vendor | Run the §3.2-3.3 capture pass for that vendor immediately; don't wait for the quarter |
| Nightly sweep `14 §L5` **Red** signal at all | Pause everything; bisect engine first; then check for cross-vendor rotation |
| Vendor publishes a major changelog entry (DataDome VM-obfuscation, Cloudflare Turnstile feature drop, AWS WAF Bot Control version bump) | Capture + triage on the day of announcement |
| Public solver repo ships a fix commit (xKiian/awswaf, glizzykingdreko/Datadome-Captcha-Deobfuscator, xvertile/akamai-bmp-generator) | Within 7 days: read the diff, classify whether it impacts our solver, log here |
| Chrome / Firefox / Safari ships a major version | Run the **browser bump** sub-checklist (§5.3) — separate from vendor rotation |

### 5.3 The browser-bump sub-checklist

When a new major browser version ships (Chrome 149, Firefox 136, Safari 19, etc.):

1. **Read the browser's release notes** — look for new JS surface, deprecated APIs, new UA-CH brand version, fingerprint-affecting changes.
2. **Capture a real-browser fingerprint** with the new version on a clean profile via the chapter 04 tooling.
3. **Diff against the current BO emission** — same signals as chapter 28 §3.
4. **Update the matching profile YAML** if UA, sec-ch-ua brand version, or any property changed.
5. **Update the bootstrap.js** if new properties were added that we now need to emit.
6. **Re-run the full sweep** with the new profile and confirm pass-rate stayed flat or improved.
7. **Log the bump under "Browser bumps" in the quarter's entry** (§4).

Per `19_PROFILE_EXPANSION_PLAN.md` we target a quarterly profile refresh, so the bump checklist usually piggybacks on the quarterly review. If Chrome ships mid-quarter, run the bump checklist immediately — don't wait.

---

## 6. Integration with chapter 18

The cookbook (`18_ANTI_BOT_VENDOR_COOKBOOK.md`) and this log are two halves of one knowledge cycle:

```
                  Quarterly review (§3-§5)
                          ↓
              ┌─ Capture + diff + triage
              │
              ↓
        Rotation observed?
              │
       ┌──────┴──────┐
       │             │
       NO            YES
       │             │
       ↓             ↓
     log a       Update §4 log
     "no-op"     with date + impact
     entry            ↓
       │       Update 18 §5 table
       │       with new cadence
       │       evidence + fixes
       │       (cookbook = SETTLED)
       │             ↓
       │       File issue in
       │       vendor_solvers for
       │       any solver work
       │             ↓
       │       Update 28 §3 (for AWS)
       │       or vendor-deep-dive
       │       (25/26/etc.) for that vendor
       │             ↓
       └─────────────┘
                  ↓
        Continue to next quarter
```

### 6.1 Cookbook is SETTLED, log is WORKING

The cookbook is the **encyclopedia** — when a contributor encounters a new site mid-development and needs to identify the vendor + recovery path, they read chapter 18. It must always be current.

This log is the **diary** — temporal record of what changed and when. It supports backward-looking analysis ("why did pass-rate dip in Q3?") and forward-looking planning ("which vendor is overdue for a structural rotation?"). It does not replace the cookbook.

### 6.2 What goes where

| Information | Cookbook (18) | This log (33) |
|---|---|---|
| Vendor identification flowchart | YES — `§1` | NO |
| Per-vendor challenge mechanism | YES — `§2.*` | NO |
| Per-vendor public solver state | YES — `§2.* "Public solver state"` | LINK to latest commit dates |
| Per-vendor failure mode | YES — `§2.* "Failure mode"` | NO |
| Current rotation cadence | YES — `§5` table (summary) | YES — `§2.1` table (sourced) |
| Past rotation events (specific dates) | NO (would clutter encyclopedia) | YES — `§4` log entries |
| Per-quarter sweep delta | NO | YES — `§4 entries "Impact"` |
| Cookbook-update PRs triggered by rotation | NO (the result lives in cookbook) | YES — `§4 entries "Cookbook update"` |

If a future contributor asks "where is X documented?" the answer is always:
- Recurring / encyclopedic → chapter 18.
- Dated event / forward signal → chapter 33 (this).

---

## 7. Automation opportunity — CI-driven drift detection

Manual quarterly capture is the floor. Automated daily drift detection is the ceiling.

### 7.1 The minimal CI job

A nightly CI step that:
1. For each anchor site (per §3.2), does a `curl` + hash.
2. Compares the hash against the last-night snapshot stored as a CI artifact.
3. On hash change → opens a GitHub issue tagged `rotation-detected` with a link to the new + old captures.

```yaml
# .github/workflows/vendor-drift.yml (NEW; see 14 §CI integration for the existing nightly-sweep.yml)
name: vendor-drift-detection
on:
  schedule:
    - cron: '0 7 * * *'  # 07:00 UTC — runs after the nightly sweep at 06:00
  workflow_dispatch: {}

jobs:
  drift:
    runs-on: [self-hosted, sweep]
    steps:
      - uses: actions/checkout@v4
      - name: Fetch prior snapshot
        uses: actions/download-artifact@v4
        with:
          name: vendor-snapshot
          path: prior_snapshot/
        continue-on-error: true  # first run has no prior

      - name: Capture current state
        run: |
          mkdir -p current_snapshot/
          declare -A ANCHORS=(
            [aws-waf]="https://www.amazon.de/"
            [datadome]="https://www.etsy.com/"
            [akamai-bmp]="https://www.adidas.com/us"
            [cloudflare]="https://www.openai.com/"
            [kasada]="https://www.canadagoose.com/"
            [perimeterx]="https://www.zillow.com/"
          )
          for v in "${!ANCHORS[@]}"; do
            curl -sS -A 'Mozilla/5.0 (...)Chrome/148.0.0.0 Safari/537.36' \
                 "${ANCHORS[$v]}" -o current_snapshot/$v.html
            sha256sum current_snapshot/$v.html | cut -c1-12 > current_snapshot/$v.hash
          done

      - name: Diff hashes
        run: |
          for f in current_snapshot/*.hash; do
            v=$(basename $f .hash)
            cur=$(cat $f)
            prev=$(cat prior_snapshot/$v.hash 2>/dev/null || echo "none")
            if [ "$cur" != "$prev" ] && [ "$prev" != "none" ]; then
              echo "ROTATION DETECTED: $v ($prev → $cur)"
              # Save the diff into a file the next step uploads
              diff -u prior_snapshot/$v.html current_snapshot/$v.html \
                > drift_$v.diff 2>/dev/null || true
            fi
          done

      - name: Upload current snapshot for next run
        uses: actions/upload-artifact@v4
        with:
          name: vendor-snapshot
          path: current_snapshot/
          retention-days: 30  # keep ~30 nightly snapshots

      - name: Upload drift diffs (if any)
        uses: actions/upload-artifact@v4
        if: hashFiles('drift_*.diff') != ''
        with:
          name: vendor-drift-diffs-${{ github.run_id }}
          path: drift_*.diff

      - name: Open issue on rotation
        if: hashFiles('drift_*.diff') != ''
        run: |
          # Use gh CLI to create an issue listing the rotated vendors;
          # contributor triages per §3.5.
          for f in drift_*.diff; do
            v=$(basename $f .diff | sed 's/^drift_//')
            gh issue create \
              --title "[rotation] $v rotated $(date +%Y-%m-%d)" \
              --body "Drift detected; see workflow run ${{ github.run_id }}. \
                Triage per docs/releases/v0.1.0-parity/33_QUARTERLY_PROBE_ROTATION_LOG.md §3.5." \
              --label rotation-detected,priority-2
          done
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### 7.2 Subresource drift detection (richer signal)

Hash on the HTML body is the cheapest signal but the noisiest. Each anchor's HTML may rotate trivially (CSRF tokens, timestamps, A/B test markers) without any vendor-protocol change. Better signals:

- Hash the **referenced subresource URL** (e.g. the `challenge.js` URL on amazon.de). If the tenant ID + integration ID change, that's a real rotation.
- Hash the **subresource body** (e.g. `challenge.js` content). Vendor-protocol rotation here is what we care about.
- For DataDome: hash the **first 4 KB of the `dd-script.js`**. The header rarely changes within a rotation; if it does, it's a structural change.

A second CI job (`vendor-subresource-drift.yml`) does this richer pass weekly (overhead per anchor is ~3 HTTP requests).

### 7.3 Public-solver drift detection

A third CI job (`solver-repo-drift.yml`) polls weekly:
- `git ls-remote https://github.com/xKiian/awswaf HEAD` — note the latest commit SHA.
- Compare against the SHA stored in the prior artifact.
- On change, open an issue: "Public AWS-WAF solver shipped a fix; review for impact."

Same pattern for `glizzykingdreko/Datadome-Captcha-Deobfuscator`, `xvertile/akamai-bmp-generator`, `umasii/ips-disassembler`.

### 7.4 What NOT to automate

- **Triage**: §3.5 is a contributor judgment call. Machine cannot classify COSMETIC vs STRUCTURAL reliably.
- **Sweep re-runs**: the nightly sweep already exists per `14 §CI integration`; do not double-run on rotation detection (the sweep itself catches the impact).
- **Cookbook updates**: human-curated content; do not let CI commit doc changes.

---

## 8. Operational gotchas

### 8.1 Capture timing matters

DataDome rotates daily at a specific (undocumented) UTC time. If you capture at 09:00 UTC one day and 11:00 UTC the next, you might see a "rotation" that's just the daily roll. Solution: standardize capture time across quarters (e.g. always run §3.2 at exactly 12:00 UTC).

### 8.2 Per-tenant rotation differs from per-vendor rotation

Akamai BMP rotates per-tenant — amazon-de's `akam/13/*` script may rotate on a different cadence than nike.com's. Don't conflate "Akamai rotated" with "this tenant's instance rotated." Capture multiple anchors per vendor when feasible.

### 8.3 IP rotation can masquerade as protocol rotation

If your capture machine's IP changes between quarters (laptop, VPN exit, ISP rebalance), some vendors will serve different stubs because of IP-class differences. Solution: tag every capture with the source IP class (residential / datacenter / mobile) in the `notes/` dir, and prefer the SAME source IP class quarter-over-quarter.

### 8.4 CDP-detection on capture tooling

If you use Playwright / Puppeteer to do the capture (instead of `curl`), Kasada and DataDome will serve different bodies because they detect CDP. Per `memory/state_2026_05_15_playwright_ab_decisive.md` — **never use Playwright/MCP as a real-browser ref for CDP-sniffers.** Use plain `curl` or our own `target/release/examples/sweep_metrics` (which is CDP-free).

### 8.5 Geo-routing

Vendors route to different rule sets by client geo. A capture from a US IP is not necessarily comparable to one from an EU IP. Keep the capture machine in a single geo across quarters; if you must change geo, log it loudly in §4.

### 8.6 The browser-bump trap

When Chrome 149 ships, our captures use the SAME `Mozilla/5.0 ... Chrome/148.0.0.0` UA we always do. Vendors will see "outdated Chrome 148" and may serve a different body than they'd serve to real Chrome 149. This is fine for measuring **vendor rotation** (we want a controlled UA across quarters), but it does NOT measure **how the vendor would treat our updated profile**. Run two captures per quarter when a browser bump is in scope: one with the old UA (rotation tracking), one with the new UA (profile-fitness check).

---

## 9. Acceptance for v0.1.0

This chapter is **done** when ALL of the following hold:

- [ ] §4.1 baseline entry is committed (covers all 7 vendors + browser bumps).
- [ ] The quarterly review checklist (§3) has been run at least once in a dry-run on the baseline captures, end-to-end, by the contributor responsible for the v0.1.0 release.
- [ ] The CI scaffolding (§7.1) is added to `.github/workflows/vendor-drift.yml` and has run once on `main`, producing a (no-rotation) artifact.
- [ ] The chapter is cross-linked from `00_README.md`, `14_TESTING_VALIDATION.md §L5`, and `18_ANTI_BOT_VENDOR_COOKBOOK.md §5`.
- [ ] An entry exists in `15_OPEN_QUESTIONS.md` for the next quarterly review date, so it isn't forgotten.

This chapter is **not** done when:

- The CI job exists but is failing silently (e.g. `gh issue create` token missing).
- The §4 log is empty beyond the baseline.
- The cookbook (18) was updated for a rotation but this log was not (the asymmetry breaks the §6 cycle).

---

## 10. Files / references

### 10.1 Public release docs (cross-linked)

- `00_README.md` — top-level release index; this chapter is listed under operational artifacts.
- `04_TOOLING_SPEC.md` — capture tooling (the `sweep_metrics` example used in §3.6).
- `06_AWS_WAF_SOLVER.md §1` — the per-vendor capture recipe for AWS WAF; reused in §3.3.
- `14_TESTING_VALIDATION.md §L5` — nightly drift signals; the early-warning that triggers §5.2.
- `14_TESTING_VALIDATION.md §CI integration` — existing nightly-sweep.yml; sibling to §7's vendor-drift.yml.
- `15_OPEN_QUESTIONS.md` — backlog for next-quarter review date + any open rotation work.
- `18_ANTI_BOT_VENDOR_COOKBOOK.md §5` — cadence summary table; this chapter is its operational expansion.
- `19_PROFILE_EXPANSION_PLAN.md` — quarterly profile refresh; piggybacks on this review cadence per §5.3.
- `24_RISK_REGISTER.md` — vendor-rotation maintenance is a v0.1.0 carry-cost risk.
- `25_CLOUDFLARE_DEEP.md` — Cloudflare-specific deep dive; cross-referenced for rotation tracking of CF specifically.
- `26_AKAMAI_BMP_DEEP.md` — Akamai-specific deep dive; same.
- `27_VENDOR_COMPETITIVE_MATRIX.md` — per-engine numbers; this chapter's §3.6 deltas feed back here.
- `28_AWS_WAF_EXTENDED.md §3` — signal inventory updated when a new probe is observed (§3.5 PROBE ADDITION).
- `28_AWS_WAF_EXTENDED.md §9` — forward-looking AWS WAF changes; complementary to this chapter's backward-looking log.

### 10.2 Private (where rotation captures live)

- `~/projects/browser_oxide_internal/docs/rotation_YYYY_QN/captures/` — per-quarter raw captures.
- `~/projects/browser_oxide_internal/docs/rotation_YYYY_QN/diffs/` — per-quarter rotation diffs.
- `~/projects/browser_oxide_internal/docs/rotation_YYYY_QN/notes/` — per-quarter sweep deltas + triage notes.
- `~/projects/browser_oxide_internal/sweeps/YYYY_QN.json` — per-quarter sweep aggregate snapshot (the §3.6 `sweep_post_rotation.json`, archived).

### 10.3 CI artifacts (public repo)

- `.github/workflows/sweep-nightly.yml` — existing nightly sweep (per `14 §CI integration`).
- `.github/workflows/sweep-weekly-agg.yml` — existing weekly aggregator.
- `.github/workflows/vendor-drift.yml` — NEW per §7.1 (HTML-hash daily diff).
- `.github/workflows/vendor-subresource-drift.yml` — NEW per §7.2 (weekly subresource diff).
- `.github/workflows/solver-repo-drift.yml` — NEW per §7.3 (weekly public-solver commit-tracking).

### 10.4 External vendor docs (rotation tracking)

| URL | Vendor | What to monitor |
|---|---|---|
| [AWS Managed Rules changelog](https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-changelog.html) | AWS WAF | Bot Control rule-group version bumps, ATP / ACFP version bumps |
| [AWS WAF Bot Control rule group](https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-bot.html) | AWS WAF | Version requirement note + new `TGT_*` rules |
| [DataDome key rotation docs](https://docs.datadome.co/docs/key-rotation) | DataDome | Daily-rotation confirmation |
| [DataDome changelog](https://datadome.co/changelog/) | DataDome | Structural changes (VM obfuscation, etc.) |
| [Cloudflare changelogs](https://developers.cloudflare.com/changelog/) | Cloudflare | All product updates |
| [Cloudflare Turnstile changelog](https://developers.cloudflare.com/turnstile/changelog/) | Cloudflare Turnstile | Widget feature drops |
| [Akamai techdocs case management changelog](https://techdocs.akamai.com/case-mgmt/changelog) | Akamai | Customer-facing changes; sensor / bm-edge updates |
| (Kasada has no public changelog) | Kasada | Monitor [unicorn-aio/kpsdk](https://github.com/unicorn-aio/kpsdk) for structural opcode updates |
| (PerimeterX / HUMAN has no public changelog) | HUMAN | Monitor [PerimeterX GitHub org](https://github.com/perimeterx) for tooling changes |
| (Imperva has no public changelog for Incapsula sensor) | Imperva | Monitor [scrapfly Imperva bypass](https://scrapfly.io/blog/posts/how-to-bypass-imperva-incapsula-anti-scraping) updates |

### 10.5 External public solver repos (rotation tracking)

| Repo | Vendor | Why we watch |
|---|---|---|
| [xKiian/awswaf](https://github.com/xKiian/awswaf) | AWS WAF | Most active public solver; commit history mirrors AWS rotation events |
| [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) | AWS WAF | Independent re-impl; confirms or refutes xKiian's structural claims |
| [Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver) | AWS WAF | Tertiary check |
| [jonathanyly/awswaf-solver-api](https://github.com/jonathanyly/awswaf-solver-api) | AWS WAF | API wrapper; tracks solver-service-level changes |
| [glizzykingdreko/Datadome-Captcha-Deobfuscator](https://github.com/glizzykingdreko/Datadome-Captcha-Deobfuscator) | DataDome | Updated when daily/structural rotations occur |
| [xvertile/akamai-bmp-generator](https://github.com/xvertile/akamai-bmp-generator) | Akamai BMP | Active sensor_data generator; tracks v3 hash-key updates |
| [umasii/ips-disassembler](https://github.com/umasii/ips-disassembler) | Kasada | ips.js disassembler; new opcode tables = bytecode rotation |
| [unicorn-aio/kpsdk](https://github.com/unicorn-aio/kpsdk) | Kasada | Reverse copies of recent ips.js drops |
| [Humphryyy/Kasada-Deobfuscated](https://github.com/Humphryyy/Kasada-Deobfuscated) | Kasada | Partial deobfuscation; tracks VM-logic changes |
| [Hyper-Solutions/hyper-sdk-js](https://github.com/Hyper-Solutions/hyper-sdk-js) | Multi (AWS / DataDome / Akamai / Incapsula / Kasada) | Commercial SDK; commit cadence = aggregate rotation pulse |

### 10.6 Internal memory cross-refs

- `memory/state_2026_05_24_*` — current sweep state; the §4.1 baseline is anchored here.
- `memory/state_2026_05_15_playwright_ab_decisive.md` — CDP-detection warning (§8.4).
- `memory/kasada_wrapper_cracked_and_remaining_leaks.md` — Kasada wrapper stability fact (§2.3 row).
- `memory/state_2026_05_16_phase5_datadome.md` — Akamai sec-cpt self-solve fact (§2.1 Akamai sec-cpt row).
- `~/projects/browser_oxide_internal/docs/HANDOFF_2026_05_17.md` — current handoff; the rotation log cadence is referenced for v0.1.0 carry-cost calc.

---

## 11. The honesty note

This is an operational artifact, not a research paper. Two failure modes for the chapter itself:

1. **Cadence atrophy**: the quarterly review never runs because no one owns it. Mitigation: §9 acceptance requires an `15_OPEN_QUESTIONS.md` entry for the next review date, owned by the v0.1.0 release manager.
2. **Cookbook drift**: rotations are logged here but not propagated to chapter 18 (the asymmetry in §6). Mitigation: every §4 entry **must** include the "Cookbook update" line — either with a PR link or with "no cookbook change required" justified.

Per `CLAUDE.md`: vendor-specific bypass code stays in private `vendor_solvers`. This chapter is about **detecting** rotations; the **fixing** happens in the private repo. The handoff seam: a rotation logged in §4 with "Fix: filed issue in vendor_solvers#NNN" is the correct handshake.
