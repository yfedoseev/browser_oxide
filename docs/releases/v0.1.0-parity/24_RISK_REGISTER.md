# 24 — Risk register + dependency map

**Status:** living document — update as risks change state, are
resolved, or new ones surface.
**Last verified:** 2026-05-24 (upstream versions checked against
crates.io API; see §4)

**Purpose:** what could block v0.1.0. Make every visible risk
addressable with an owner, a mitigation, and an SLA. A risk is not
"managed" until it appears here.

---

## 1. Risk taxonomy

| Severity | Definition | SLA to address | Default review cadence |
|---|---|---|---|
| **P0** | Blocks v0.1.0 release. Cannot ship the tag with this open. | immediate (this sprint) | every standup until closed |
| **P1** | High risk to release timeline. Two weeks of slack at most. | within 1 week | weekly |
| **P2** | Manageable; mitigation planned, monitoring in place. | within 1 month | monthly |
| **P3** | Known unknown; track but don't act now. May graduate later. | quarterly review | quarterly |

State transitions: a risk moves between severities as evidence
changes. Note each transition in the risk's entry (`[2026-05-24:
P2 → P1, reason …]`) so we have an audit trail.

---

## 2. Risk register

For each risk: **ID** / severity / description / probability / impact /
mitigation / owner / state-history.

### P0 — Blocking

#### R-001 — wellsfargo pool panic

**Severity:** P0
**Probability:** certain (deterministically reproduces on the 2026-05-24
sweep, site 98 of 126)
**Impact:** blocks the pool-mode v0.1.0 release. The pool path is the
core throughput story (14 pages/min vs cold 2.5 pages/min — per
`10_TIMING_OPTIMIZATION.md §1`); shipping without pool means we ship
~5× slower than the competition.

**Description:** `crates/dom/src/arena.rs:678` cycle detector panics
(`DOM walk cycle in collect_elements from NodeId(0) — visited 100001
unique nodes`) when the pool path reaches `wellsfargo.com`. Pool sweep
aborts entirely; the abort is a `thread caused non-unwinding panic.
aborting.` so the process dies. Cold mode renders the same URL fine.

Hypothesis from `10 §4.3`: `replace_dom` (`crates/js_runtime/src/lib.rs:169-179`)
swaps `DomState` but doesn't invalidate stale JS-side `NodeId` handles
that previous-page scripts may have captured. When the warm isolate
re-runs `op_dom_set_inner_html` (`crates/js_runtime/src/extensions/
dom_ext.rs:452-479`) with a stale id, append_child can splice a node
back into its own ancestor → cycle.

**Mitigation:**

1. **First — reproduce on cold path.** Per `10 §4.2`: if cold-path
   also panics on wellsfargo, it's a wellsfargo-specific arena bug
   and the pool path is innocent.
2. **If cold-clean, ship the defensive fix.** `replace_dom`
   bumps a generation counter (`DomState.gen: u32`); every `op_dom_*`
   op checks the generation tag on the incoming `NodeId`. Mismatch →
   return -1 (treated as "node not found" by JS).
3. **Cold-fallback pattern in production** — per
   `22_PRODUCTION_DEPLOYMENT.md §2.4`, every customer wraps pool nav
   in a try/catch that re-creates the pool and falls back to cold on
   error. This means R-001 is not customer-fatal even before the fix
   lands; it just costs throughput on the wellsfargo class of sites.

**Owner:** TBD (Phase 4 — see `10_TIMING_OPTIMIZATION.md §8`)
**State history:** [2026-05-24: opened from BENCHMARK_2026_05_24.md §7]

---

#### R-002 — Multi-run baseline not yet measured against HEAD

**Severity:** P0
**Probability:** certain — `14_TESTING_VALIDATION.md §L5` defines the
3-run aggregation that hasn't yet been executed against HEAD with
fixes B and C committed
**Impact:** without it, every "is this fix real?" decision is a single-
run measurement subject to ±5 sites of WAF variance (per
`docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`). We could declare v0.1.0
with a number that's 5 sites optimistic and customers see worse.

**Description:** Per `14 §L5`, the source-of-truth pass rate is the
median of 3 full 126-site sweeps. We have one 2026-05-24 sweep on
`90a7ed5` (the commit before fixes B+C). We need fresh runs on:
- HEAD with both fixes
- 3× per profile (4 profiles × 3 runs × 50 min = 10 hours total)
- Then the median per (profile, site) cell + the routed best-of-4
  computation per `14 §L5`

**Mitigation:**

1. Run the 3-run aggregate immediately after fixes B and C are
   committed (the 2026-05-24 working tree has both pending commit
   per `00_README.md "Memory-mode notes"`)
2. Nightly cron picks up the schedule once the CI workflow from
   `14 §"CI integration"` lands
3. Block v0.1.0 tag on the 3-run median meeting the
   `00_README.md` scorecard

**Owner:** TBD (Phase 0 — tooling + methodology, before any further
gap-site work)
**State history:** [2026-05-24: opened from 14 §L5 acceptance gate]

---

### P1 — High

#### R-003 — `deno_core 0.311` version pin diverges from upstream

**Severity:** P1
**Probability:** high — verified 2026-05-24 via crates.io API. Latest
released `deno_core` is **0.401.0** (published 2026-05-22). We pin
**0.311** (per `crates/browser/Cargo.toml:27`,
`crates/js_runtime/Cargo.toml:27`, `crates/event_loop/Cargo.toml:13`,
`crates/workers/Cargo.toml:13`). We are 90 minor versions behind.
**Impact:** high if we need a 0.4xx feature (op2 macro improvements,
V8 prebuilt binary refresh, security fixes). Each upgrade traverses
breaking API changes; the op2 macro has shifted state-parameter
semantics in past versions (per `09 §4.3` rationale for the
`RefCell<Vec<u32>>` workaround).

**Description:** `deno_core` ships frequently and breaking-changes the
extension API on a regular cadence. Pinning to 0.311 means:
- No security fixes from 0.312-0.401 reach us automatically
- V8 prebuilt binaries are frozen at whatever 0.311 shipped with —
  which has its own security surface
- If we discover we need an op2 fix from a later version, the upgrade
  is a multi-week effort (every `#[op2]` site re-validates against the
  new macro semantics)

**Mitigation:**

1. **Stay pinned through v0.1.0** — the engine works on 0.311, the
   risk is upgrade-not-stay
2. **Vendor-fork deno_core if it diverges** — if we need a 0.4xx
   feature and the migration is too costly, fork at 0.311 and
   cherry-pick the required commit
3. **Monitor monthly** — `WebFetch
   https://crates.io/api/v1/crates/deno_core` returns
   `max_stable_version`; check it's not catastrophically ahead (e.g.
   0.500+ would mean V8 has had multiple major bumps)
4. **MSRV constraint** — per CI `.github/workflows/ci.yml:97-108`, MSRV
   is 1.83 which is the floor of what 0.311 + V8 prebuilts compile
   against. Bumping deno_core bumps MSRV — coordinate with
   `R-013`/license policy.

**Owner:** TBD
**State history:** [2026-05-24: opened. Upstream 0.401 verified via
crates.io API]

---

#### R-004 — `boring2` maintenance (Cloudflare BoringSSL fork)

**Severity:** P1
**Probability:** moderate. Verified 2026-05-24: latest stable
**4.15.15** (we pin **4.15** per `crates/net/Cargo.toml:30`, so cargo
resolves to 4.15.15); latest alpha **5.0.0-alpha.13** with the
Chrome-impersonation APIs **removed** per `crates/net/Cargo.toml:22-30`
comment. We are intentionally on the last-known-good major.
**Impact:** high if Cloudflare deprioritizes the fork. Our entire TLS
ClientHello + HTTP/2 fingerprint stack (per
`crates/net/src/tls.rs:22-228`) sits on top of boring2's Chrome-
impersonation API. If 5.x stays alpha-with-removed-APIs and 4.x
stagnates, we're stuck on 4.x security forever (no upstream patches).

**Description:** From `crates/net/Cargo.toml:22-30`:
> "boring2 5.0-alpha removed the Chrome-impersonation APIs we rely on:
> [list of APIs]. Staying on the 4.15.x stable line until 5.x reaches
> feature parity (or until we adapt to the new API surface)."

This is a deliberate pin to a stable line that may stop receiving
upstream patches if all attention shifts to 5.x.

**Mitigation:**

1. **Monitor monthly** — `WebFetch
   https://crates.io/api/v1/crates/boring2` for new 4.x.y patch
   releases. If 4.x stops getting BoringSSL upstream merges for > 6
   months, escalate to P0.
2. **Track 5.x API surface** — subscribe to the
   https://github.com/cloudflare/boring repo's "Chrome impersonation"
   issue threads; when 5.x re-adds parity, plan the migration
3. **Fallback plan** — if Cloudflare ever drops the fork entirely,
   our options are: (a) vendor-fork boring2 ourselves with BoringSSL
   upstream merges, (b) migrate to rustls + a custom ClientHello
   builder, (c) migrate to native-tls (lose fingerprinting, big pass-
   rate hit). Option (a) is the v0.2.x answer.
4. **Silent-drift gate** —
   `crates/net/src/tls.rs:506+` `tls_fingerprint_vectors_no_silent_drift`
   catches the case where a BoringSSL update changes the ClientHello
   bytes without us noticing. Already in place; CI runs it per
   `.github/workflows/ci.yml`.

**Owner:** TBD
**State history:** [2026-05-24: opened. 4.15.15 stable / 5.0.0-alpha.13
verified via crates.io API]

---

#### R-005 — Chrome 149 release timing drift

**Severity:** P1
**Probability:** high — Chrome ships a major every ~4 weeks; Chrome
149 stable is expected mid-2026
**Impact:** high on the 4-week window after a stable release. Our
`chrome_148_*` profiles immediately become "one major behind real
Chrome". Some WAFs flag the inconsistency: e.g., "UA claims 148 but
the Client Hints brand list doesn't include the new 149 brand entry."

**Description:** Per `11_PER_PROFILE_STRATEGY.md §7.2` ("When Chrome
ships 149"), the expected delta is mechanical:
- Bump UA strings in every `chrome_148_*` preset
- Bump `UA_CHROME_MAJOR` in `crates/net/src/tls.rs:57`
- Leave `TLS_CHROME_MAJOR` at 147 unless there's a fresh BoringSSL
  capture proving the bytes changed (per `tls.rs:22-57` rationale —
  Chrome TLS is stable across majors barring MLKEM-like rollouts)

The risk is the *delay between Chrome stable and our refresh*. If
Chrome 149 ships day-zero and we don't bump for 2 weeks, every
chrome_148-profiled scrape sees more risk-class friction from WAFs
than necessary.

**Mitigation:**

1. **Subscribe to Chrome stable releases** —
   https://chromereleases.googleblog.com/ . On stable-channel post,
   open a PR within 48 hours per the `11 §7.2` mechanical checklist.
2. **Per `19` / `23` quarterly refresh** — even without a Chrome bump,
   refresh profiles quarterly to catch drift from UA-CH / Client
   Hints / accept-CH evolution.
3. **Silent-drift test** — `tls_fingerprint_vectors_no_silent_drift`
   (`tls.rs:506+`) fails until `UA_CHROME_MAJOR` is bumped; this is
   the forcing-function for a refresh PR.

**Owner:** TBD
**State history:** [2026-05-24: opened. Chrome 148 is current as of
sweep date]

---

#### R-006 — AWS WAF challenge.js obfuscation rotation

**Severity:** P1
**Probability:** moderate-high. AWS rolls challenge.js at unannounced
intervals (per `06_AWS_WAF_SOLVER.md §7 risks`); historical observation
is ~monthly minor changes, ~quarterly substantive changes
**Impact:** high if our chapter-06 solver work depends on a specific
challenge.js byte-pattern. Per `12 §3.5` and `02 GAP_ANALYSIS.md`,
amazon.* + imdb are 5+ recoverable sites — the entire chapter-06
deliverable is gated by AWS WAF detection mechanics.

**Description:** The AWS WAF `challenge.js` (loaded from
`*.token.awswaf.com`) is obfuscated JavaScript that fingerprints the
JS environment and POSTs a token to `/verify`. Our engine executes it
but the fingerprint check fails silently and `getToken()` short-
circuits (per `15 §Q2`). Any solver we ship needs to either:
- (a) recognize the specific check and patch the engine to pass it
- (b) rewrite challenge.js into known-good form before it executes
- (c) provide enough engine fidelity that the unmodified challenge.js
  passes (the long-term right answer)

Approach (a) is brittle — every challenge.js rotation can re-break
it. Approach (c) is robust but high-effort.

**Mitigation:**

1. **Per `06 §7 risks`** — recommend solver Alternative C (JS-rewrite)
   as the most robust to rotation. The rewrite operates on
   *structural* properties (Worker-vs-no-Worker, particular JS API
   call patterns) which are less likely to change than literal byte
   patterns.
2. **Capture + diff at each amazon retail-season change** — per
   `12 §5.2` Q4 retail traffic surges correlate with AWS WAF
   challenge.js rotations. Re-capture and diff in Oct/Nov/Jan/Feb.
3. **Monitor for regression** — the 33-site spot-check
   (`14 §L3`) includes the amazon cluster; a sudden drop on amazon-de
   / amazon-in / amazon-com-au / imdb is an early signal of an AWS WAF
   change.

**Owner:** TBD (Phase 2 — `06_AWS_WAF_SOLVER.md`)
**State history:** [2026-05-24: opened]

---

### P2 — Manageable

#### R-007 — DataDome WASM-iframe-daily-key rotation

**Severity:** P2
**Probability:** certain — DataDome rotates keys daily (per
`memory/state_2026_05_16_phase5_datadome.md` and
`07_DATADOME_PRIMITIVES.md`)
**Impact:** moderate. Affects etsy / tripadvisor / yelp (the
`07_DATADOME_PRIMITIVES.md` recovery cluster — 2-3 sites). Per the
`02 GAP_ANALYSIS.md` per-site breakdown, these are recoverable but
hard.

**Description:** Per memory note, DataDome's challenge mechanics:
- WASM-iframe challenge with cross-origin DOM
- A daily-rotated key embedded in the WASM blob
- Even if `07_DATADOME_PRIMITIVES.md`'s primitives restoration ships
  (CSP relax + cross-origin iframe materialization + cookie-write-
  as-solve signal), the underlying daily key change means a stored
  solution from yesterday won't work today

**Mitigation:**

1. **Per `07_DATADOME_PRIMITIVES.md`** — restore the post-aecdf19
   engine-internal primitives (no vendor name in code) — these are
   protocol-correct browser behaviour, not vendor bypass
2. **Extract daily, not statically** — any solver must re-derive
   the key per-day from the WASM blob, not cache it
3. **Acceptance gated on a multi-day test** — per `14 §Phase 3`,
   the test must run on 3 different days and pass each time

**Owner:** TBD (Phase 3)
**State history:** [2026-05-24: opened]

---

#### R-008 — Camoufox catches up

**Severity:** P2
**Probability:** moderate. Camoufox is actively maintained on a
quarterly Firefox refresh cadence (per `12 §5.1`)
**Impact:** moderate-high. The v0.1.0 bar is 115 strict-pass (Camoufox
= 113). If Camoufox lands an AWS WAF or DataDome improvement before us,
the bar moves to 115-120 and our 115 target is no longer a "beat"
result.

**Description:** Camoufox's open roadmap (per their repo) suggests:
- Firefox 136+ class refreshes quarterly
- C++ patch coverage extending to more APIs (AudioContext, WebRTC)
- Better Kasada handling under active research

Any of these could shift their headline number up by 2-5 sites in a
single release. Our v0.1.0 plan needs to be robust to a moving bar.

**Mitigation:**

1. **Monthly comparative sweep** — per `12 §5.1`, re-run
   `benchmarks/run_full_sweep.sh` against latest Camoufox each
   month
2. **If Camoufox jumps to 116+** — investigate which sites flipped
   and assess whether the new wins are something BO can match via
   per-profile work or solver primitives (the chapter-05-08 plan
   already covers all major recoverable surfaces)
3. **Per `12 §5.4` catch-up plan** — chapters 05/06/07 in order get
   us to 117-119; chapter 08 is bonus. Even if Camoufox jumps to 116
   we have headroom to reach 118-119

**Owner:** TBD (ongoing competitive intel)
**State history:** [2026-05-24: opened]

---

#### R-009 — Kasada SOTA frontier

**Severity:** P2
**Probability:** certain — per `08_KASADA_FRONTIER.md`, the Kasada
sites (canadagoose / hyatt / realtor) are research-bound
**Impact:** low-moderate. v0.1.0 explicitly does not promise Kasada.
3 sites of 126 = 2.4% — not a deal-breaker for any customer who
isn't specifically targeting those domains.

**Description:** Per `08_KASADA_FRONTIER.md`, even Camoufox loses all
3 of these. The hopes (per memory `state_2026_05_16_*`) are not all
landing for v0.1.0. Kasada is explicitly out of scope.

**Mitigation:**

1. **Clear expectations in `00_README.md`** — already says routed
   best-of-4 bar is 115, not 118; the Kasada residual is documented
2. **Treat the residual as a research backlog** — per `15 §R4`,
   tracked but not blocking
3. **Customer guidance in `22 §2`** — DataDome and Cloudflare get
   profile routing recipes; Kasada gets "try all 4, accept low
   success rate" (per `11 §4.1 rule 6`)

**Owner:** research (open-ended)
**State history:** [2026-05-24: opened, accepted as out-of-scope for
v0.1.0]

---

#### R-010 — V8 `HEAP_INITIAL` reduction regresses creepjs

**Severity:** P2
**Probability:** moderate (only triggered if the §5 of
`09_MEMORY_OPTIMIZATION.md` decision lands as "reduce to 256 MB")
**Impact:** high if it lands — creepjs is a keystone "the engine is
fingerprint-honest" pass site; regressing it would be a public
embarrassment

**Description:** Per `09 §5` and `15 §D1`: the V8 heap is currently
sized at 1 GB initial / 4 GB max (`crates/js_runtime/src/runtime.rs:98-111`).
The 1 GB initial was chosen because creepjs allocates well past
256 MB during fingerprint collection; with 256 MB initial, V8 spends
time in `Builtins_ArrayPrototypePush` compactions and OOM'd around
1.8 GB on macOS arm64 pre-bump.

If we reduce to 256 MB to save -30 to -50 MB per-isolate baseline, we
risk:
- creepjs body-len regression (compaction time delays fingerprint
  resolution past the nav budget)
- General pass-rate regression on heap-heavy challenge VMs (Kasada
  ips.js, Akamai sensor_data, DataDome i.js)

**Mitigation:**

1. **A/B test before commit** — per `09 §5 acceptance gate`:
   3-run 126-site sweep with delta-pass ≥ -2 sites
2. **If A/B fails any gate, do not commit** — documented in
   `15_OPEN_QUESTIONS.md §D1`
3. **Worker reap fix is the larger memory win anyway** — per
   `09 §4` hypothesis 13 sites × 15 MB = 195 MB; if that fix
   delivers, the §5 question may not need to be answered for v0.1.0

**Owner:** TBD (memory work, Phase 4)
**State history:** [2026-05-24: opened, gated behind D1 decision]

---

#### R-011 — Worker reap fix C may break sites that rely on long-lived workers

**Severity:** P2
**Probability:** low. The 33-site spotcheck (per `09 §4` validation)
showed no functional regression
**Impact:** moderate if it manifests — a site that explicitly spawns a
Worker on load and keeps it alive across navigations (rare but real
pattern for analytics SDKs, real-time chat widgets, …) would lose its
worker mid-flight on `Page::drop`

**Description:** Per `09 §4` (the worker-reap fix already in working
tree): `Page::drop` calls `drain_owned_workers` which terminates every
worker spawned by the page's isolate. Sites that:
- Spawn a worker for a long-running background task
- Don't explicitly tear it down via `worker.terminate()` from JS
- Rely on the worker continuing after navigation

... would lose the worker. The current sweep-corpus shows no such
sites in the 33-site spotcheck, but full validation needs the 126-site
sweep + 3-run.

**Mitigation:**

1. **Pre-merge gate: full 126 sweep + 3-run** — per `R-002`. If any
   site flips Pass → CHL/THIN after the fix, investigate before
   committing
2. **Net positive memory wins** — the spotcheck showed +80 MB peak
   RSS drop expected; if validation confirms, the fix is net-good
   even with 1-2 false-flip sites
3. **Roll-forward fallback** — if a v0.1.x customer reports a
   long-lived-worker breakage, gate the reap behind an opt-out env
   var (`BROWSER_OXIDE_WORKER_REAP=0`) for v0.1.1

**Owner:** TBD (the working-tree fix needs validation + commit)
**State history:** [2026-05-24: opened, fix applied uncommitted]

---

#### R-012 — `SharedSession` bleed (`15 §Q3`)

**Severity:** P2
**Probability:** suspected from one sweep data point — x-com returned
69 bytes (THIN-BODY) mid-sweep but 274 KB (L3-RENDERED) in isolated
single-site run (per `15 §Q3`)
**Impact:** moderate. If real, it explains a handful of mid-sweep
flakes; potential -1 to -3 sites in WAF-strict origins

**Description:** Per `15 §Q3`: commit `f62584d` introduced process-
wide `SharedSession` that pools cookies + `accept_ch` across origins.
Hypothesis: Twitter's WAF flags requests that carry `Accept-CH`
hints derived from another origin's prior advertisement.

This is unverified — no A/B has been run.

**Mitigation:**

1. **A/B test** — per `15 §Q3 next step`: full 126-sweep with
   `HttpClient::shared` vs `HttpClient::new`, observe x-com
2. **If confirmed, gate behind env var** — `BROWSER_OXIDE_SHARED_SESSION`
   default off for benchmark (clean isolation), on for production
   (customer can opt in if they understand the tradeoff)
3. **A/B harness exists** — per `14 §"A/B harness — for evaluating a
   specific fix"`, `tools/ab_sweep.sh` runs the comparison

**Owner:** TBD (Phase 0 tooling + Phase 1 validation)
**State history:** [2026-05-24: opened]

---

### P3 — Tracked, not actioned

#### R-013 — License creep

**Severity:** P3
**Probability:** low — `cargo deny check` runs in CI per
`.github/workflows/ci.yml:110-119`
**Impact:** high if it lands — any GPL/LGPL/AGPL transitive dep would
break our "permissive only" policy in `deny.toml`

**Description:** Per `CLAUDE.md`: "License: MIT OR Apache-2.0; no
GPL/LGPL/AGPL." Per `deny.toml` allow-list (read 2026-05-24): MIT,
Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause,
BSD-3-Clause, ISC, Zlib, Unicode-DFS-2016, Unicode-3.0,
CDLA-Permissive-2.0, 0BSD, BSL-1.0, MIT-0, CC0-1.0.

Exceptions (per `deny.toml [licenses.exceptions]`):
- `cooked-waker` — MPL-2.0, transitive via `deno_core 0.311 → v8`
- `adblock` — MPL-2.0, optional behind `blocker` feature in `net`
  (default OFF)

**Mitigation:**

1. **Mechanical CI gate** — `cargo deny check all` in
   `.github/workflows/ci.yml:110-119` fails the PR if a new
   non-allowed license appears
2. **Per-PR review** — license-relevant changes (new dependency,
   feature gate that pulls in a new dep) must be explicitly checked
   by the reviewer
3. **Re-audit each `deno_core` bump** — per `R-003`, the upgrade
   may pull in new transitive deps; verify `cargo deny check` stays
   green before merging

**Owner:** ongoing (everyone)
**State history:** [2026-05-24: opened]

---

#### R-014 — Vendor solver code leaking from private to public

**Severity:** P3
**Probability:** low — `CLAUDE.md` explicitly forbids it and reviewers
catch it
**Impact:** high if it lands — leaks the project's "engine + primitives
only" scope; pulls vendor-bypass code into the MIT-Apache public
engine

**Description:** Per `CLAUDE.md`:
> "Per-vendor challenge solving is out of scope here. The engine
> exposes a `browser::ChallengeSolver` trait + `Page::
> navigate_with_solvers` hook; the concrete Akamai/Kasada/DataDome/
> Cloudflare implementations live in the private `vendor_solvers`
> companion crate."

The risk: a contributor (especially an AI assistant) re-implements an
Akamai sensor_data encoder or a Kasada `x-kpsdk-ct` solver in
`crates/browser/` or `crates/net/`. This is the kind of code that
looks like normal engine work but actually crosses the scope line.

**Mitigation:**

1. **Code review** — every PR reviewed by Yury (project lead) before
   merge
2. **`grep` for vendor names in PRs** — pre-merge automated check:
   `grep -rE 'akamai|kasada|datadome|imperva|perimeterx|cloudflare-managed' crates/`
   against the diff. Hits in `crates/` outside of comment-only and
   ChallengeSolver dispatch are flagged
3. **`SCOPE.md`** — clarifies the boundary; cite in PR reviews when
   needed
4. **Private crate stays private** — `vendor_solvers` is in a
   sibling repo with its own access control

**Owner:** Yury (project lead, code review)
**State history:** [2026-05-24: opened. CLAUDE.md memo a11044f-era
makes the rule explicit]

---

#### R-015 — V8 isolate-per-thread constraint

**Severity:** P3
**Probability:** known constraint, well-documented
**Impact:** low — design constraint, not a regression risk

**Description:** Per `CLAUDE.md`: "V8 isolates are per-thread. Running
multi-threaded crashes the test process. CI enforces
`--test-threads=1`." Per `.github/workflows/ci.yml:84-90`: the test
step runs `cargo test --workspace --no-fail-fast --
--test-threads=1`.

The parallel cold sweep (per `15 §R2` and `10 §6.4`) and the
production worker (per `22 §4`) both need careful thread modeling.
Future contributors who don't read this may break it.

**Mitigation:**

1. **CI enforcement** — `--test-threads=1` flag in the test step
2. **`CLAUDE.md`** — convention documented at the top
3. **Skeleton in `22 §4`** — the production worker pattern
   demonstrates the correct topology
4. **`Page` / `PagePool` are `!Send`** — the type system enforces
   it; any `tokio::spawn` of a future holding a `Page` won't compile

**Owner:** documented; no further action needed unless violated
**State history:** [2026-05-24: opened, accepted as design constraint]

---

#### R-016 — Chrome moves to ECH (Encrypted ClientHello)

**Severity:** P3
**Probability:** low for v0.1.0 timeframe; high over 2-3 years
**Impact:** very high if it happens — would invalidate our entire
JA4/ClientHello impersonation work in `crates/net/src/tls.rs`

**Description:** Encrypted ClientHello (ECH) is an IETF draft that
encrypts the SNI and other fields in the TLS ClientHello. If Chrome
makes ECH mandatory:
- Our byte-perfect ClientHello reproduction becomes moot — the bytes
  on the wire are different per session
- JA4 fingerprinting (vendor-side) becomes useless — vendors will
  shift to other signals
- Our `tls_fingerprint_vectors_no_silent_drift` test would need a
  complete redesign

**Mitigation:**

1. **Monitor TLS spec evolution** — watch IETF tls-wg mailing list +
   Chromestatus for ECH announcements (per `R-005` quarterly cadence)
2. **Track Chrome canary ECH defaults** — if ECH flips to default-on
   in canary, escalate to P1 immediately
3. **Plan a v0.2.x rewrite** — when ECH lands, the fingerprinting
   game changes. Our value-add moves from "byte-perfect TLS" to
   "byte-perfect ECH-encapsulated inner ClientHello + outer
   ClientHello shape parity"
4. **Per `23 §6`** (when written) — TLS-future-proofing section

**Owner:** TBD (research)
**State history:** [2026-05-24: opened, monitored]

---

#### R-017 — Test flakiness from WAF variance

**Severity:** P3
**Probability:** certain — per `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`,
±5 sites of variance per sweep
**Impact:** low — `14 §L5` 3-run aggregation specifically designed for
this

**Description:** WAFs roll their risk model unpredictably; the same
URL + same profile may CHL on one sweep and Pass on the next, with no
engine change in between. The regression-check CI (per `14 §"CI
integration"`) may false-positive on this.

**Mitigation:**

1. **3-run aggregation** — per `14 §L5`, the source-of-truth metric
   is median across 3 sweeps. False positives drop from "high" to
   "negligible".
2. **Pass criterion: ≥ baseline - 3** — per `14 §L4 Pass criteria`,
   single-run regressions of up to -3 are within noise floor and not
   flagged. > -3 fires alert.
3. **±5 noise floor documented in
   `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md`** — reviewers cite
   it when investigating an apparent regression
4. **Retry budget** — per `22 §2.5`: max 2 retries per URL; don't
   retry-until-pass

**Owner:** ongoing (test infrastructure)
**State history:** [2026-05-24: opened, mitigations in place via
14 §L5]

---

## 3. Dependency map

Verified 2026-05-24 against `Cargo.toml` files in the workspace.

```
browser_oxide (workspace, 15 crates, MIT OR Apache-2.0)
├── deno_core 0.311  (PIN — see R-003; latest released: 0.401)
│   └── v8 (prebuilt binaries, ~130 MB on first cargo fetch)
│       └── cooked-waker (TRANSITIVE — MPL-2.0; per-crate exception in deny.toml)
├── boring2 4.15  (PIN — see R-004; latest stable: 4.15.15; latest alpha: 5.0.0-alpha.13)
├── tokio 1.*  (workspace dep, features = ["full"])
├── tokio-boring2 4.15
├── async-trait 0.1
├── futures-util 0.3
├── url 2.*
├── serde 1 (derive) + serde_json 1
├── thiserror 2
├── tracing 0.1
├── adblock 0.12  (OPTIONAL — MPL-2.0; behind `blocker` feature; default OFF)
└── (transitive: paste, bincode, adler, miniz_oxide, ring, …
   advisories tracked in deny.toml with explicit ignores)
```

### Critical dependencies (would block v0.1.0 if broken)

| Dep | Pin | Why critical | If it breaks |
|---|---|---|---|
| `deno_core` | 0.311 | JS runtime + V8 binding | engine cannot run JS |
| `v8` (via deno_core) | (whatever 0.311 ships) | JS execution + heap | engine cannot run JS |
| `boring2` | 4.15 | Chrome TLS ClientHello | every stealth profile broken |
| `tokio` | 1.* | async runtime | nothing runs |
| `adblock` | 0.12 | only if `blocker` feature enabled | `blocker` feature disabled |

### Build-time dependencies (not runtime)

- Rust toolchain ≥ 1.83 (MSRV per `.github/workflows/ci.yml:97-108`,
  floor of deno_core 0.311 + V8 prebuilts)
- ~130 MB V8 prebuilt binary fetch on first build (per `CLAUDE.md`)

### Workspace internal crates (per `Cargo.toml [workspace.members]`)

15 crates, all version 0.1.0, all `publish = false` (per
`crates/*/Cargo.toml:8`): css_parser, css_selectors, css_values,
css_cascade, dom, html_parser, js_runtime, event_loop, browser,
canvas, layout, net, workers, stealth, protocol.

---

## 4. Upstream watch list

Verified 2026-05-24 via crates.io API + gh release lists.

| Dep | Source | Latest stable | We pin | Action cadence |
|---|---|---|---|---|
| `deno_core` | https://crates.io/crates/deno_core | **0.401.0** (2026-05-22) | **0.311** | monthly — major version watch |
| `boring2` | https://crates.io/crates/boring2 | **4.15.15** (current) | **4.15** | monthly — alpha vs stable monitoring |
| `tokio` | https://crates.io/crates/tokio | 1.x | 1.* | follows tokio LTS cadence |
| `adblock` | https://crates.io/crates/adblock | 0.12 | 0.12 | only matters when `blocker` feature in use |
| Chrome stable | https://chromereleases.googleblog.com/ | 148 (current) | (UA reflects 148) | on every Chrome stable post — 48 h refresh per R-005 |

WebFetch URLs to verify versions monthly:

```
https://crates.io/api/v1/crates/deno_core   (read max_stable_version, newest_version)
https://crates.io/api/v1/crates/boring2     (read max_stable_version)
https://crates.io/api/v1/crates/tokio       (read max_stable_version)
https://crates.io/api/v1/crates/adblock     (read max_stable_version)
```

---

## 5. Decision log

For deferred decisions: who made the call, when, on what evidence.

| Date | ID | Decision | Decided by | Source |
|---|---|---|---|---|
| 2026-05-21 | aecdf19 | Vendor strip — public engine ships no solvers | Yury | commit `aecdf19`; `CLAUDE.md` "Per-vendor challenge solving is out of scope here" |
| 2026-05-24 | D2 | Solver implementations stay in `vendor_solvers` private crate | Yury | `15_OPEN_QUESTIONS.md §D2` (resolved: NO) |
| 2026-05-24 | D1 | V8 `HEAP_INITIAL` stays at 1 GB until A/B test on creepjs | Yury | `15_OPEN_QUESTIONS.md §D1` (deferred); `09_MEMORY_OPTIMIZATION.md §5` |
| 2026-05-24 | D3 | `chrome_148_windows` profile deferred to v0.2.0 unless v0.1.0 lands under target | Yury | `15_OPEN_QUESTIONS.md §D3` |
| 2026-05-24 | Q4 | Engine-internal primitives restored to public engine (per `07_DATADOME_PRIMITIVES.md`) — they are protocol-correct, not vendor bypass | Recommended in `15_OPEN_QUESTIONS.md §Q4` | `15_OPEN_QUESTIONS.md §Q4` |

When a deferred decision is acted on: move it from "deferred" rows to
"resolved" rows with the date and link to the implementing commit.

---

## 6. Pre-flight checklist before tagging v0.1.0

A v0.1.0 tag must satisfy every box. If any box is unchecked, the tag
is premature.

### Risk closure

- [ ] All P0 risks **resolved** or **explicitly accepted** (with
      decision-log entry):
  - [ ] R-001 wellsfargo pool panic — either fixed or pool path
        marked experimental with cold-fallback as the supported path
  - [ ] R-002 multi-run baseline — 3-run aggregated full sweep
        committed to `/tmp/full_sweep_v0.1.0_RC/` or equivalent
- [ ] All P1 risks have a **documented mitigation** that's actually
      in place (not just planned):
  - [ ] R-003 deno_core pin — monitoring cadence set up
  - [ ] R-004 boring2 maintenance — monthly check in calendar
  - [ ] R-005 Chrome 149 timing — subscriber on
        chromereleases.googleblog.com
  - [ ] R-006 AWS WAF rotation — solver shipped per `06_AWS_WAF_SOLVER.md`
        if intended for v0.1.0; or deferred per decision log

### Measurement

- [ ] 3-run baseline aggregated (per `14 §L5`)
- [ ] Routed best-of-4 median Pass ≥ 115 (per `00_README.md` scorecard)
- [ ] Per-profile median Pass ≥ baseline (chrome ≥ 99, pixel ≥ 102,
      iphone ≥ 98, firefox ≥ 101)
- [ ] Pool path completes 126/126 with no panic (R-001 closed)
- [ ] Cold path RSS peak ≤ 350 MB on all 4 profiles (per `09 §7`)
- [ ] Pool path RSS peak ≤ 800 MB on full 126 sweep (per `09 §7`)
- [ ] Pool throughput ≥ 13.5 pages/min (= Patchright; per `10 §7`)

### Documentation

- [ ] All chapters in `docs/releases/v0.1.0-parity/` finalized + reviewed
- [ ] `CHANGELOG.md` updated with per-fix attribution
- [ ] `README.md` updated with v0.1.0 numbers (replacing 2026-05-24
      numbers)
- [ ] Decision log (`§5` above) up to date through tag-day

### Build + license

- [ ] Dependency map (`§3` above) verified against current `cargo tree`
- [ ] License audit clean (`cargo deny check all`)
- [ ] CI green on main:
  - [ ] fmt
  - [ ] clippy -D warnings
  - [ ] test (single-threaded, ubuntu + macos, stable + beta)
  - [ ] msrv (1.83)
  - [ ] deny check all
- [ ] `cargo doc --no-deps --workspace` clean with `RUSTDOCFLAGS=-D warnings`

### Tag mechanics

- [ ] Branch `release/v0.1.0-parity` created from a clean main
- [ ] `git tag v0.1.0-parity` after a clean 3-run from the release
      branch
- [ ] Tag is annotated (`git tag -a`) with the headline pass-rate
      numbers + commit summary
- [ ] Release notes drafted (link from CHANGELOG to this doc)

---

## 7. Files referenced

### Engine source

- `crates/browser/src/page.rs:200-214` — `Page` struct
- `crates/browser/src/page.rs:823-848` — `with_solvers` (R-014 surface)
- `crates/browser/src/page.rs:955-960` — `navigate_with_solvers`
- `crates/browser/src/pool.rs:1-87` — `PagePool` (R-001, R-010 surface)
- `crates/browser/src/challenge.rs:1-43` — solver trait doc
  (R-014 boundary)
- `crates/dom/src/arena.rs:5-19` — `WALK_LIMIT = 100_000` (R-001)
- `crates/dom/src/arena.rs:655-699` — `collect_elements` panic site
  (R-001)
- `crates/js_runtime/src/lib.rs:169-179` — `replace_dom` (R-001)
- `crates/js_runtime/src/runtime.rs:98-111` — `HEAP_INITIAL = 1 GB` (R-010)
- `crates/js_runtime/src/extensions/worker_ext.rs:407-434` — worker
  reap + ownership (R-011)
- `crates/net/src/tls.rs:22-57` — TLS_CHROME_MAJOR / UA_CHROME_MAJOR
  (R-004, R-005)
- `crates/net/src/tls.rs:506+` — silent-drift gate (R-004, R-005)
- `crates/net/src/lib.rs:123-145` — `SharedSession` (R-012)
- `crates/net/src/blocker.rs:1-115` — adblock feature gate (R-013)

### Build / policy

- `Cargo.toml:5-25` — workspace members + version pinning
- `crates/browser/Cargo.toml:27` — `deno_core = "0.311"`
- `crates/js_runtime/Cargo.toml:27` — `deno_core = "0.311"`
- `crates/event_loop/Cargo.toml:13` — `deno_core = "0.311"`
- `crates/workers/Cargo.toml:13` — `deno_core = "0.311"`
- `crates/net/Cargo.toml:22-30` — `boring2 = "4.15"` + the rationale
  for not moving to 5.x alpha
- `crates/net/Cargo.toml:49` — `adblock = "0.12"` optional
- `deny.toml` — license + advisory + source policy (R-013 enforcement)
- `.github/workflows/ci.yml` — fmt / clippy / test / msrv / deny jobs
- `.github/workflows/ci.yml:97-108` — MSRV 1.83 pin (R-003 ripple)

### Tests + harness

- `crates/browser/tests/holistic_sweep.rs` — 126-site corpus (R-002,
  R-017)
- `crates/browser/examples/sweep_metrics.rs` — sweep harness (R-002,
  R-011 validation)
- `benchmarks/run_full_sweep.sh` — 4-profile orchestrator (R-002)
- `benchmarks/build_report.py` — sweep aggregator

### Sibling chapters

- `00_README.md` — release plan overview + scorecard
- `09_MEMORY_OPTIMIZATION.md` — R-010, R-011 detail
- `10_TIMING_OPTIMIZATION.md` — R-001 detail
- `11_PER_PROFILE_STRATEGY.md` — R-005 Chrome bump playbook
- `12_COMPETITIVE_LANDSCAPE.md` — R-008 Camoufox catch-up monitoring
- `14_TESTING_VALIDATION.md` — R-002, R-017 mitigation harness
- `15_OPEN_QUESTIONS.md` — Q-mappings to risks (Q1→R-?,Q2→R-006,
  Q3→R-012, Q5→R-001)
- `22_PRODUCTION_DEPLOYMENT.md` — R-001 cold-fallback pattern (§2.4),
  R-010 watchdog (§3)

### External references (verified 2026-05-24)

- https://crates.io/api/v1/crates/deno_core — newest 0.401.0
  (2026-05-22)
- https://crates.io/api/v1/crates/boring2 — max_stable 4.15.15,
  newest 5.0.0-alpha.13
- https://github.com/cloudflare/boring — upstream repo (R-004 watch
  list)
- https://github.com/denoland/deno_core — upstream repo (R-003 watch
  list)
- https://chromereleases.googleblog.com/ — Chrome stable release
  feed (R-005 watch list)
- `CLAUDE.md` — vendor solver scope (R-014), license rules (R-013),
  per-thread V8 constraint (R-015)
- `docs/NOISE_FLOOR_ANALYSIS_2026_05_23.md` — ±5 site noise floor
  (R-017)
- `docs/BENCHMARK_2026_05_24.md §7` — wellsfargo panic capture (R-001)
