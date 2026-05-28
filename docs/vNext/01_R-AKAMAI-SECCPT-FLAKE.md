# 01 — R-AKAMAI-SECCPT-FLAKE: homedepot sec-cpt bundle self-solve regression

**Status:** ⏸️ deferred from v0.2.0 audit cycle. Public engine; needs an oracle harness + targeted fix.
**Sites in scope:** homedepot (1). Patchright passes; BO + Camoufox v150 both fail.
**Effort:** 1-2 days oracle build + 1-3 days fix.
**Scope:** public engine.

## TL;DR

homedepot serves the Akamai `sec-cpt` bundle variant — a rotating
obfuscated `/Wjv3…` script the page loads inline. In b623d5d the engine
had a built-in `handle_akamai_flow` that DEFENSIVELY suppressed itself
on sec-cpt pages so the bundle could self-solve. After the refactor to
the `ChallengeSolver` trait + private `vendor_solvers` companion crate,
that defensive suppression only fires when a vendor solver is loaded;
the public-engine sweep (empty `default_solvers()`) doesn't trigger it.
The bundle itself runs in V8 but doesn't complete its self-solve — the
gap is some primitive it expects that BO no longer provides.

## Why this matters

- Patchright (real Chromium) **passes** homedepot (1246k L3-RENDERED per
  HANDOFF §1.8). Chromium-class engines CAN do it.
- BO + Camoufox v150 both fail. v150's hardware-spoofing additions
  didn't move homedepot — confirms it's NOT a fingerprint-class fix.
- The b623d5d-era engine WAS passing homedepot (per
  `memory/state_2026_05_16_phase5_datadome.md`). The regression is
  real; the refactor lineage is what's owing.
- Single-site reward (+1) but useful as a "Stratum B → Stratum A"
  proof — every flip here narrows the "we and v150 can't" frontier.

## Current state

What we know from this session (audit/16 §R-AKAMAI-SECCPT-FLAKE):

- `b623d5d` added a sec-cpt guard at the inline `handle_akamai_flow`
  call: when the response HTML contained `sec-if-cpt-container`, the
  Akamai BMP POST path was suppressed for the whole nav. Bundle = sole
  actor.
- HEAD has the equivalent code at `page.rs:2162`:
  ```rust
  let sub = if s.name() == "akamai-bmp" && started_as_seccpt_challenge {
      "sec-cpt"
  } else { "" };
  ```
  But this only fires if an `akamai-bmp` solver is registered.
  `default_solvers()` returns empty, so the loop is a no-op.
- The bundle still runs in V8 (it's a `<script>` in the response HTML).
  But it doesn't complete its self-solve flow — outcome is the BMP
  challenge body, not the rendered content.

What we DON'T know:

- Which specific primitive the bundle needs that BO no longer provides.
  Probable shapes:
  1. A pre-refactor cookie observation hook that watched for
     `sec_cpt` → `~3~` flip and triggered a reload.
  2. A fetch-interception hook that let the bundle's verify POST land
     a real response.
  3. A V8 / DOM surface change that broke the bundle's own JS.
- Whether the bundle's bailout is similar to FIX-J (FileReader.readAsDataURL
  empty result → "malformed data URL") or something different.

## Next steps

Mirror the **R-AWSWAF-OFFLINE-PROBE** methodology that found FIX-J:

### Step 1 — Capture the sec-cpt response (~30 min)

```bash
mkdir -p /tmp/seccpt_probe
curl -sS -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36" \
  -H "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8" \
  -H "Accept-Language: en-US,en;q=0.9" \
  --max-time 30 -o /tmp/seccpt_probe/homedepot.html https://www.homedepot.com/

# Extract the bundle URL
grep -oE 'src="/Wjv3[^"]+"' /tmp/seccpt_probe/homedepot.html

# Download the bundle (URL has a daily-rotated path)
curl -sS -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ..." \
  -H "Referer: https://www.homedepot.com/" \
  --max-time 30 \
  -o /tmp/seccpt_probe/wjv3_bundle.js \
  "https://www.homedepot.com/Wjv3<…>"
```

Note: the bundle path rotates daily; capture must be SAME-DAY for the
oracle replay to work (similar limitation as AWS WAF gokuProps).

### Step 2 — Run `awswaf_probe`-class oracle (~1 day)

Modify `crates/browser/examples/awswaf_probe.rs` (or fork it as
`seccpt_probe.rs`) to:
1. Load `homedepot.html` via `Page::from_html_with_url`
2. Pre-inject the same instrumentation Proxy snippet from
   `/tmp/awswaf_probe/probe_inject.js` (wraps `navigator` / `screen` /
   `document` / `chrome` / `Function.prototype.toString` / `console.*`)
3. Run the event loop until idle
4. Dump the access trace + `document.cookie` state + any errors

Expected outputs to look for:
- A trace showing what surfaces the bundle reads BEFORE it bails
- Any `Error: …` rejection bubbled out of a Promise
- Final `sec_cpt` cookie value (should flip to `~3~` on success)

### Step 3 — Identify the bailout (~few hours)

The trace will point at one of:
- **A specific DOM/Window API the bundle uses that BO doesn't fully
  implement** (e.g., the `MutationObserver` registration shape, a
  specific `Element.attachShadow` slot, an `IntersectionObserver`
  callback timing).
- **A cookie-handling hook** the bundle expects (probably a
  `document.cookie = …` setter side-effect that the v0.1.0-era engine
  observed but the new one doesn't).
- **A fetch-interception detail** (the bundle POSTs to a verify URL;
  BO's net layer may reject the URL or strip a header).

### Step 4 — Land the fix in the public engine (~1-2 days)

Whatever the bailout is, the fix is a public-engine primitive (same
class as FIX-DD's 3 DataDome primitives). Per CLAUDE.md, the WASM /
solver-side computation stays in `vendor_solvers`; the engine just
needs to NOT interfere with the bundle's self-solve. Likely shapes:

- A `is_seccpt_solved(cookies, body)` helper that recognizes the
  `sec_cpt=~3~` cookie + non-challenge body as a solve marker, wired
  at the navigate-loop break (mirror FIX-DD's pattern).
- Restoration of a cookie-observation hook that wakes the nav iter
  on `sec_cpt` cookie changes.

### Step 5 — Validate

- Add a `homedepot_seccpt_self_solve` test in chrome_compat (gated as
  `#[ignore]` if it requires network).
- Run the 8-site Akamai-cluster sweep:
  ```bash
  target/release/examples/sweep_metrics chrome_148_macos <(echo '[{"cat":"stores","name":"homedepot","url":"https://www.homedepot.com/"}]') /tmp/seccpt_validation.json
  ```
- Confirm the routed-median gate doesn't regress other Akamai sites.

## Dependencies

- The `awswaf_probe` oracle pattern (already shipped, commit `ef4f561`).
- A captured sec-cpt response from a working day (the bundle's
  fixed/rotated path).
- No `vendor_solvers` dependency — this is public-engine work.

## Sources / references

- `crates/browser/src/page.rs:1691-1703` — `started_as_seccpt_challenge` detection
- `crates/browser/src/page.rs:2162-2173` — the sec-cpt sub_kind dispatch (currently solver-only)
- `crates/browser/examples/awswaf_probe.rs` — the oracle template
- `~/.claude/projects/-home-yfedoseev-projects-browser-oxide/memory/state_2026_05_16_phase5_datadome.md` — Inc-7 / b623d5d historical context
- audit `16_DECISION_LOG.md` §R-AKAMAI-SECCPT-FLAKE — this session's analysis
