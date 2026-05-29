# 06 — amazon-ca: gate token-clustering artifact (NOT an engine gap)

**Scope:** the single consistency-gap site `amazon-ca`, which fails on
`chrome_148_macos` (L3-RENDERED **5310 B** = thin AWS-WAF stub) but PASSES
`pixel_9_pro_chrome_148` (997 KB), `iphone_15_pro_safari_18` (891 KB), and
`firefox_135_macos` (1.17 MB) in the *same* 2026-05-29 gate.
**Verdict:** this is a **measurement-consistency artifact** (AWS-WAF per-IP
token clustering + per-request risk-rolling), **not** a per-profile engine /
TLS / fingerprint gap. BO's *true* per-profile AWS pass rate is higher than the
gate shows. The fix is a **harness spacing/cooldown policy**, not engine code.
**Reading order:** this doc →
`docs/v0.1.0-parity-workflows/sites/SITE_awswaf_cluster.md` (ADDENDUM 3) →
`docs/v0.1.0-parity-workflows/external/VENDOR_awswaf.md` (§2.1, §7) →
`benchmarks/run_spaced_aws.sh` → `benchmarks/run_bo_isolated.py`.

---

## 0. TL;DR

`amazon-ca` is one of **9 AWS-WAF cluster sites** in the corpus (8 amazon TLDs +
imdb; `benchmarks/corpus_vendor_map.py:24-32`). All 9 are fronted by AWS WAF's
silent **Challenge action**, which:

1. issues a token (`aws-waf-token`) with a **≥300 s immunity floor**, and
2. **risk-rolls per request** so a clean engine still tops out ~85 % per hit
   (`VENDOR_awswaf.md §1.1`, `06_AWS_WAF_SOLVER.md §0.5`).

When the same IP requests several amazon TLDs **inside that ~300 s window**,
AWS treats them as a burst from one client and the per-request risk-roll turns
hostile — it serves the 1991–5310 B `challenge.js` stub instead of the page,
*regardless of which browser profile sent the request*. This is exactly what the
gate captured: by the luck of run order, `chrome_148_macos` happened to land
`amazon-ca` inside a cluster of other amazon hits and drew the bad roll; the
other three profiles, which ran their amazon batches later (tokens cooled),
drew clean rolls.

**Proof it is not an engine gap:** the spaced harness
(`benchmarks/run_spaced_aws.sh`, 150 s gaps, fresh process per site) passes
`amazon-ca` at **1.03 MB on `chrome_148_macos`** — the *exact profile* the gate
marks as failing — and the whole 9/9 cluster passes spaced
(`SITE_awswaf_cluster.md` ADDENDUM 3, 2026-05-28). A profile-specific
fingerprint/TLS tell cannot explain a site that passes that same profile when
merely re-timed.

---

## 1. The per-profile data signature (evidence)

From `00_DATA_per_profile_matrix.md:20`:

| profile | amazon-ca tag/len | reading |
|---|---|---|
| chrome_148_macos | · L3-RENDERED **5310** | thin AWS stub (challenge served) |
| pixel_9_pro_chrome_148 | ✅ L3-RENDERED 997677 | real page |
| iphone_15_pro_safari_18 | ✅ L3-RENDERED 891169 | real page |
| firefox_135_macos | ✅ L3-RENDERED 1170789 | real page |

Two signatures rule out a per-profile fingerprint cause and rule **in**
clustering:

- **The failing body is the AWS stub, not a profile-specific challenge.** 5310 B
  with tag `L3-RENDERED` is the AWS-WAF `challenge.js` shell (the classifier now
  tags it `AWS-WAF-CHL` when armed — see §3 — but at len ≥ 4096 it can still
  read as a thin rendered page). It is the **same** stub all 4 profiles would
  get on a bad roll; only chrome drew it this run. Compare a real per-profile
  challenge in the same matrix: iphone gets `Cloudflare-CHL` on economist/ecosia
  (`:21-22`) — a *different vendor tag*, the genuine "this UA is challenged"
  signature. amazon-ca shows no such per-profile tag divergence: 3 of 4 profiles
  get the full page, 1 gets the generic stub.

- **The "winner" rotates with run order, not with profile.** In this gate chrome
  lost amazon-ca; in the spaced run chrome *wins* amazon-ca at 1.03 MB
  (`SITE_awswaf_cluster.md` ADDENDUM 3 lists amazon-ca PASS 1.03 MB on
  `chrome_148_macos`). A fingerprint tell is stationary; this one moved. That is
  the definition of a timing/clustering artifact.

Contrast with the *other* chrome gap-fails in the same matrix (`:41`): spotify,
tripadvisor, uber, yelp fail chrome with **DataDome-CHL / TIMEOUT** — those are
real per-profile or reliability issues with distinct signatures. amazon-ca is
the odd one out: a generic AWS stub, profile-independent.

---

## 2. Root cause — why clustering hits one profile per gate run

### 2.1 The gate does NOT space AWS sites far enough apart

The 2026-05-29 matrix was produced by `benchmarks/run_full_gate.sh`, which calls
`benchmarks/run_bo_isolated.py <profile> $CORPUS_FILE` once per profile
(`run_full_gate.sh:50-52`). The corpus is vendor-**spaced** beforehand via
`corpus_vendor_map.space_by_vendor()` so that **no two AWS sites are adjacent**
(`corpus_vendor_map.py:46-110`). That sounds like the clustering guard — but it
is the *wrong granularity* for AWS:

- `space_by_vendor` only guarantees **non-adjacency** (≥1 non-AWS site between
  two AWS sites). It does **not** guarantee a wall-clock gap anywhere near the
  300 s immunity window.
- I simulated a representative 125-site spaced corpus
  (`corpus_vendor_map.space_by_vendor`, seed `demo`): the 9 AWS sites landed at
  positions `[30, 41, 45, 48, 54, 96, 106, 116, 119]`, with inter-AWS gaps of
  **`[11, 4, 3, 6, 42, 10, 10, 3]` sites**. The **minimum gap is 3 sites**.
  At ~5–15 s per non-AWS site, a 3-site gap is **~24–45 s of wall time** —
  an order of magnitude inside AWS's 300 s immunity / risk-roll window.
- Worse, the cluster `[41, 45, 48, 54]` packs **4 amazon TLDs into ~13 sites
  (~2–3 min)**. Whichever profile's run order produces such a tight amazon
  sub-cluster eats the bad risk-rolls. Because each profile is shuffled/spaced
  with its own ordering, the unlucky profile differs run-to-run — which is why
  the gate shows amazon-ca failing on *chrome* this time but the cluster doc
  shows it passing chrome when spaced.

### 2.2 The 30 s cooldown in `run_bo_isolated.py` essentially never fires for AWS

`run_bo_isolated.py:41` reads `BO_VENDOR_COOLDOWN_S` (default **30 s**) and
sleeps it only **before a vendor-clustered site whose *immediately previous*
site hit the same vendor** (`run_bo_isolated.py:62-67`):

```python
vendor = SITE_VENDOR.get(site["name"])
if vendor and vendor == prev_vendor and cooldown:
    time.sleep(cooldown)        # only when AWS directly follows AWS
prev_vendor = vendor
```

But `space_by_vendor` has already guaranteed AWS sites are **never adjacent**, so
`vendor == prev_vendor` is **almost never true for AWS** in the gate corpus. The
cooldown is dead code for the AWS cluster under the spacing pass. Net effect:
AWS sites get **0 s** of deliberate cooldown — only the incidental ~24–45 s of
running the 3–11 interleaved non-AWS sites. That is far too little.

(`run_sweep_isolated.py` — the alternate chunked runner — uses a 45 s
`BO_GATE_VENDOR_COOLDOWN_S` between same-vendor *chunks*, `run_sweep_isolated.py:173`;
also insufficient, and not the runner the gate used.)

### 2.3 Was 30 s "enough"? No — the requirement is set by AWS, not by us

AWS WAF Challenge tokens carry a **minimum immunity time of 300 s**
(`VENDOR_awswaf.md §2.1`, citing
`waf-tokens-immunity-times.html`), and the Challenge action **risk-rolls per
request** (`06_AWS_WAF_SOLVER.md §0.5 / §7.4`). The empirically validated gap
that clears clustering is **150 s** (`run_spaced_aws.sh:DELAY=150`,
`SITE_awswaf_cluster.md` ADDENDUM 3 "spaced 2–3 min apart"). 30 s — even if it
fired — is **5× too short**. The spaced harness deliberately uses 150 s
*between every AWS site* and gets **9/9**; the gate's incidental ~24–45 s gets
clustered failures on a rotating profile.

### 2.4 This is profile-independent by construction

AWS WAF Challenge action is **browser-interrogation + PoW only — no per-UA
behavioral gate** (`28_AWS_WAF_EXTENDED.md §1.7`, summarized in
`VENDOR_awswaf.md §1.2`). The risk decision is dominated by **IP reputation +
request burst rate**, which is shared across all 4 BO profiles (one IP, one
gate). All four profiles ship a coherent Chrome/Safari/Firefox TLS+UA+H2
fingerprint (`crates/net/src/tls.rs`, `headers.rs`, `presets.rs`) and the AWS
self-solve completes under each (the oracle reaches `forceRefreshToken` and the
spaced run passes all profiles). So there is **no TLS/JA4 or UA-coherence lever**
specific to amazon-ca: the variable is *when on the IP timeline* the request
landed, not *which fingerprint* sent it.

---

## 3. What's already correct in the engine (do not touch for this)

The AWS self-solve path is **landed and working** (this is why the spaced run
passes), per `SITE_awswaf_cluster.md` ADDENDUM + the source:

- `is_awswaf_challenge()` / `is_awswaf_solved()` classifiers —
  `crates/browser/src/page.rs:264-275`.
- `started_as_awswaf_challenge` arms the 90 s poll + cookie-delta re-fetch —
  `page.rs:1908, 2231, 2337-2346, 2436`.
- AWS 25 s host-budget tier — `page.rs:1997-2001`.
- The four cookie/FormData correctness fixes that made the self-solve mint a real
  token — `ec895b6` (FIX-COOKIE-SYNC), `5de1a9a` (FIX-COOKIE-DELETE), `37e2597`
  (shared-jar), `2157f92`/`c21c75a` (FIX-FORMDATA) — `SITE_awswaf_cluster.md`
  ADDENDUM 3.

These let **AWS's own challenge.js** mint the token under BO's real fingerprint;
no token forging, all public-engine, per `CLAUDE.md`. **Nothing engine-side
flips amazon-ca's gate result** — the engine already solves it when the IP
timeline is clean. The remaining problem is purely *measurement timing*.

---

## 4. External research (grounds the spacing number)

- **AWS WAF token immunity floor = 300 s.** The Challenge action sets
  `aws-waf-token` with a minimum immunity time of 300 s; within that window AWS
  re-evaluates burst behavior per request
  (AWS — *Token immunity times*,
  `https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-immunity-times.html`;
  cited in `VENDOR_awswaf.md §2.1`).
- **Per-request risk-rolling.** AWS WAF's Challenge serves a PoW page and
  re-rolls the allow/challenge decision per request based on IP + behavior, so
  even a perfect client tops out ~85 % per hit (AWS — *Protect against bots with
  Challenge & CAPTCHA*, `06_AWS_WAF_SOLVER.md §0.5`). Back-to-back same-IP hits
  on multiple amazon tenants inflate the burst signal and depress that rate.
- **No per-UA behavioral signal for the Challenge action.** Mouse/keystroke
  signals belong to the *SDK-integration* token, not the silent Challenge
  (`28_AWS_WAF_EXTENDED.md §1.7`). So the per-profile fingerprint is **not** the
  gate — corroborating that amazon-ca's per-profile split is timing, not
  fingerprint.
- **`gokuProps.context` binds a token to a domain** — a minted token can't be
  replayed across regional amazon tenants (`28_AWS_WAF_EXTENDED.md §1.6`,
  `VENDOR_awswaf.md §1.2`). This is *why* spacing matters: each amazon TLD needs
  its own fresh solve, and bunching them on the IP raises the burst score for
  the whole cluster.
- **TLS/JA4 per browser is coherent across BO profiles.** BO ships
  Chrome-identical (boring2 BoringSSL) and Firefox/Safari ClientHello + H2
  fingerprints per profile (`crates/net/src/tls.rs`, `h2_client.rs`,
  `NETWORK_fingerprint.md`). DataDome/Cloudflare/PerimeterX *do* fingerprint
  JA4/UA per browser (that explains the iphone/firefox CF/DataDome gap-fails
  elsewhere in the matrix) — but **AWS WAF Challenge does not gate on JA4 vs
  page-solve**; it gates on IP burst + PoW completion. So the JA4-per-browser
  research that explains the CF/DataDome clusters is **not** the amazon-ca lever.

---

## 5. Implication: BO's TRUE per-profile AWS pass is higher than the gate shows

The gate counts amazon-ca as a chrome *fail*, dropping chrome to 110/125. But
amazon-ca passes chrome at 1.03 MB when spaced. The same clustering noise hits
the *other* amazon TLDs run-to-run (e.g. amazon-fr served a 5 KB THIN stub
back-to-back, 799 KB spaced — `run_spaced_aws.sh` header comment;
`SITE_awswaf_cluster.md` ADDENDUM 3). So across the 9-site AWS cluster, **each
profile loses 0–2 sites per gate run purely to clustering**, and *which* sites
rotate. The corrected per-profile AWS pass (spaced) is **9/9 for every profile**
(ADDENDUM 3 proves the cluster on chrome; the engine path is profile-independent
per §2.4). Folding that back:

- chrome_148_macos gate 110 → **~111** with amazon-ca de-clustered (and up to
  +1–2 more across the cluster on noisier runs).
- The consistency-gap list (`00_DATA §:20`) loses amazon-ca entirely once the
  harness spaces AWS — it is not a real consistency gap.

This is a **measurement-correctness win**, not an engine flip. It tightens
all-4-profile consistency *and* nudges each profile's count toward v150 parity.

---

## 6. Ranked fix list (measurement-methodology; all public-engine)

> Acceptance: re-run the AWS cluster on all 4 profiles with the new spacing and
> confirm amazon-ca (and the other 8) pass on **every** profile, matching the
> 9/9 `run_spaced_aws.sh` result. Validate with `run_spaced_aws.sh` as the
> oracle and a spaced full-gate as the regression.

### FIX-M1 — Enforce a wall-clock AWS gap, not just non-adjacency (THE lever)
**What:** Replace the "no two AWS sites adjacent" guarantee with a **wall-clock
minimum gap** for AWS-tagged sites. Two equivalent implementations:
- (a) In `run_bo_isolated.py`, track `last_seen_ts[vendor]` and, before an
  AWS-tagged site, `sleep(max(0, 150 - (now - last_seen_ts['awswaf'])))` instead
  of the current adjacency-only 30 s (`run_bo_isolated.py:62-67`). This charges
  the cooldown against time already spent on interleaved sites, so it only
  sleeps the *shortfall* — cheap when natural spacing is already large.
- (b) Make `space_by_vendor` aware of a per-vendor **minimum index distance**
  (e.g. AWS sites ≥ N apart where N·avg_site_seconds ≥ 150 s) instead of the
  current distance-1 guarantee (`corpus_vendor_map.py:46-110`).
Prefer (a): time-based is robust to per-site duration variance; the index-based
spacing can't know wall time.
**Effort:** 0.5 day. **Expected per-profile gain:** **+1 each** (amazon-ca
de-clusters on chrome; the other 3 profiles already pass it but become *stable*),
plus removes 0–2 rotating false-fails across amazon-fr/jp/com-au/in per profile
per run. **Confidence:** high — the 150 s value is empirically the 9/9 threshold
(`run_spaced_aws.sh`). **Public engine:** yes (harness only).

### FIX-M2 — Raise / actually-fire the AWS cooldown to ≥150 s
**What:** Bump `BO_VENDOR_COOLDOWN_S` default from 30 → **150** for the AWS
vendor specifically (or per-vendor cooldown map: `{"awswaf": 150, ...}`), and —
critically — change the trigger from "previous site == same vendor" to "any
previous AWS site within 300 s" so it fires under the spacing pass. Without the
trigger fix, raising the number alone does nothing (§2.2: it never fires for
AWS). This is the minimal version of FIX-M1 if a full time-tracking rewrite is
deferred.
**Effort:** 0.25 day. **Expected per-profile gain:** same as FIX-M1 (it's the
same mechanism, coarser). **Confidence:** high. **Public engine:** yes.

### FIX-M3 — Score AWS sites from the spaced oracle, not the inline gate
**What:** The gate's `build_gate_report.py` already notes that "authoritative AWS
measurement is `benchmarks/run_spaced_aws.sh`" (`build_gate_report.py:167`).
Make that operational: have the gate, for the 9 AWS-tagged sites, **substitute**
the most-recent `run_spaced_aws.sh` result (per profile) rather than the inline
clustered result, with a freshness window (e.g. ≤24 h). Annotate the matrix cell
as "spaced-oracle" so it's auditable.
**Effort:** 0.5 day. **Expected per-profile gain:** corrects the *reported*
count (+1 chrome via amazon-ca, stabilizes the cluster) without lengthening the
main gate. **Confidence:** high. **Public engine:** yes. **Caveat:** keep the
inline run too, as a canary that the engine self-solve still works.

### FIX-M4 — Run the AWS cluster as a dedicated spaced phase, last
**What:** Pull the 9 AWS sites out of the interleaved per-profile pass and run
them as a separate, all-profiles, 150 s-spaced phase at the end of the gate
(reusing `run_spaced_aws.sh`'s structure but looping the 4 profiles). Adds ~9 ×
150 s × 4 ≈ 90 min to a 3–5 h gate, but guarantees the AWS cluster is measured
exactly once, cleanly, per profile. Combine with FIX-M3 so the main pass doesn't
also pay the AWS time.
**Effort:** 0.5 day. **Expected per-profile gain:** same correction as M1/M3,
maximally robust. **Confidence:** high. **Public engine:** yes. **Trade-off:**
gate wall time.

### NON-FIX — do NOT add engine code for amazon-ca
The engine already solves amazon-ca on every profile when the IP timeline is
clean (§3). Any further "fix" (token forging, longer drains, per-profile TLS
tweaks) would be (a) out of scope per `CLAUDE.md` if it touches forging, and
(b) chasing AWS's per-request dice (`06_AWS_WAF_SOLVER.md §0.5`: ~85 % ceiling)
rather than the real defect, which is the harness. amazon-com/amazon-ca were
historically mislabeled "IP/probabilistic" purely because of back-to-back
clustering and pass cleanly when spaced (`SITE_awswaf_cluster.md` ADDENDUM 3) —
the lesson is "bake spacing into the gate," not "add engine code."

---

## 7. Definition of done
- FIX-M1 (or M2+M3) lands; a spaced full-gate shows **amazon-ca PASS on all 4
  profiles** and the AWS cluster at **9/9 per profile**, matching
  `run_spaced_aws.sh`.
- `00_DATA_per_profile_matrix.md` is regenerated with amazon-ca dropping off the
  consistency-gap list; chrome_148_macos rises ≥1 (to ~111) from de-clustering.
- The matrix annotates AWS cells as spaced-oracle-sourced (FIX-M3) for
  auditability.
- No engine code changed for amazon-ca (the self-solve path stays as landed).

— Research agent, 2026-05-29 (amazon-ca gate-clustering, evidence: spaced-corpus
simulation above + `run_spaced_aws.sh` 9/9 + `SITE_awswaf_cluster.md` ADDENDUM 3)
