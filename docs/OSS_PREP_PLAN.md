# Open-source readiness — audit synthesis + prep plan (2026-06-03)

Repo is currently **PRIVATE** on GitHub (`origin/main = fa7bc03`). This plan covers
what to do before flipping it public. Based on a 5-dimension audit (secrets/internal
refs, vendor-bypass leakage, meta-files, docs/benchmark artifacts, code-comment
hygiene).

## ✅ Good news (no hard blockers in the engine code)
- **No vendor-bypass CODE in public crates.** No `vendor_solvers` crate in-tree; the
  `ChallengeSolver` trait (`crates/browser/src/challenge.rs`) is abstract/no-op; the
  hook delegates to embedders. Verified.
- **No secrets/keys/credentials.** No API keys, passwords, private keys, proxy creds.
  ("token" grep hits = anti-bot domain names + a font tokenizer.)
- **OSS scaffolding mostly present + high quality:** dual MIT/Apache, SCOPE, SECURITY,
  CONTRIBUTING, CODE_OF_CONDUCT, deny.toml, .github (CI SHA-pinned, dependabot,
  templates). README is honest/credible.

## ✅ Already fixed (commit 06595c0 on `chore/oss-prep`)
- Removed nonexistent `akamai` crate row (CONTRIBUTING.md + llms.txt) — was advertising
  in-tree Akamai solver code that doesn't exist (scope leak).
- deny.toml self-contradictory "NO MPL" comment.
- SCOPE.md `benches/` path.
- Added top-level `LICENSE`.

---

## 🔴 BLOCKER 1 — the internal docs are an anti-bot vendor cookbook (must remove)
`docs/` = 6.0 MB / 179 tracked files; **~149 are internal**. `docs/releases/v0.1.0-parity/`
(56 files) is explicitly a per-vendor bypass cookbook — directly contradicting SCOPE.md
("site-specific recipes + reverse-engineering kept in a private companion repo"):
- `06_AWS_WAF_SOLVER.md` — step-by-step AWS-WAF bypass playbook (deobfuscation recipe,
  fingerprint-signal table, Rust `detect()`/`solve()` skeletons, WASM harness, regex
  bail-line rewriting). **Highest concern.**
- `08_KASADA_FRONTIER.md`, `07_DATADOME_PRIMITIVES.md`, `25_CLOUDFLARE_DEEP.md`,
  `26_AKAMAI_BMP_DEEP.md`, `18_ANTI_BOT_VENDOR_COOKBOOK.md`, `29_F5_SHAPE…`, `30_ARKOSE…`,
  `35_IMPERVA…`, etc. — vendor RE deep-dives.
- `12_COMPETITIVE_LANDSCAPE.md`, `27_VENDOR_COMPETITIVE_MATRIX.md` — competitor analysis.
- Session narrative: `docs/HANDOFF_2026_05_27/28/28b.md`, `docs/research-2026-05-30/`
  (40 files), `docs/research/2026-06-02/`, `docs/v0.1.0-*-workflows/`, my
  `docs/HANDOFF_2026_06_03_cap0_duolingo_issues.md`, `docs/OSS_PREP_PLAN.md` (this file).
**Action:** keep only the ~14–20 public engineering refs the README links (ARCHITECTURE,
STEALTH, NETWORKING, PROTOCOL, the per-crate CSS/DOM/JS/CANVAS/LAYOUT/EVENT_LOOP/WORKERS
set, methodology). Move everything else to the private `browser_oxide_internal` repo.

## 🔴 BLOCKER 2 — git HISTORY also contains the cookbook
The 155-commit history (now on `origin/main`, private) includes all of the above. Removing
files from HEAD does NOT remove them from history. Before public:
- **Option A (recommended): fresh public repo with squashed/clean history** — publish a
  single "Initial public release" commit (or a curated history) from the cleaned tree;
  keep the full-history repo private.
- **Option B:** `git filter-repo` to purge `docs/releases/`, `docs/research*`, handoffs,
  benchmark JSONs from history, then force-push. Riskier, rewrites all hashes.

## 🟠 BLOCKER 3 — code-comment hygiene (large, case-by-case)
Engine comments leak vendor-bypass methodology + competitor names (maintainer's own rule:
no competitor/methodology names in engine comments):
- **~550 named-vendor comments** in a bypass-instructional style (Kasada `sbi/sdt/fsc`
  probes, "defeats Kasada's addContentWindowProxy detector", `_abck`, sec-cpt,
  x_wbaas_token). Top files: `page.rs` (186), `window_bootstrap.js` (51), `classify.rs`
  (43), `dom_bootstrap.js` (38), `chrome_compat.rs` (37).
- **~28 competitor names** (camoufox/playwright/patchright/curl-impersonate/puppeteer/
  brave) in comments — `presets.rs`, `headers.rs`, `tls.rs`, `gpu.rs`, `module_loader.rs`, …
- **~20 detection-evasion "headless tell" comments** — reframe to "differs from real Chrome".
- **~38 dated refs (2026-05-xx)** + **~40 internal-doc refs** ("doc 26/27", "Fix 6",
  "research 05_PERIMETERX.md") — genericize/remove.
**Action:** mechanical genericize for competitor names + dates + internal refs; case-by-case
reframe for the vendor-bypass + evasion-framing comments (keep the *web-API-correctness*
rationale, drop the *how-to-defeat-vendor-X* framing).

## 🟠 ITEM 4 — 70 `unsafe` blocks missing `// SAFETY:` (repo's own rule)
55 in `crates/canvas/src/webgl_render.rs` (OSMesa/GL FFI), rest in net/tls, html_parser,
page.rs (~9 in net/csp.rs are false positives = CSP keywords). Add SAFETY comments.

## 🟠 ITEM 5 — competitive benchmarks (decision needed)
README has a head-to-head table (camoufox/playwright/patchright pass-rates + "12× less
memory than Playwright"); `benchmarks/runs/` has competitor result JSONs (528 KB, 10 files).
Auditors split: one judged the README fair/honest; another flagged legal/reputational risk
of naming competitors with superiority claims. **Decision:** keep (honest, self-critical) /
trim to BO-solo numbers / remove competitor JSONs.

## 🟡 ITEM 6 — contact + small meta gaps (need a value from you)
- SECURITY.md + CONTRIBUTING.md point to "the maintainer's email" that **exists nowhere**;
  CODE_OF_CONDUCT.md enforcement contact is **blank**. Need a real channel (GHSA-only? a
  role alias? you said earlier not to use the personal Gmail publicly).
- `.github/ISSUE_TEMPLATE/bug_report.yml` references a "Site failing to render" template
  that doesn't exist — add or remove the reference.
- Missing `CHANGELOG.md` (add a `0.1.0` initial-release entry).
- 36 `/home/yfedoseev` absolute paths — 10 in `benchmarks/*.sh|*.py` are real (scripts
  won't run as cloned → `$REPO`/`$HOME`); the rest are in the internal docs being removed.
- crates.io: all 15 crates `publish=false` (generic names collide); add
  `rust-version="1.83"`, `repository.workspace=true`, keywords/categories IF crates.io
  publish is a goal (GitHub-only launch doesn't need it).
- `CLAUDE.md` is tracked (AI working notes) — keep (it's mostly conventions) or remove.

---

## Recommended order
1. Decide history strategy (fresh repo vs filter-repo) — gates everything.
2. Remove internal docs + competitor JSONs from HEAD; fix script abs-paths.
3. Fix contact info + CHANGELOG + issue-template ref (need your contact value).
4. Code-comment genericization pass (the big one) + add SAFETY comments.
5. Decide competitive-benchmark stance (README + JSONs).
6. Final `cargo build/test/clippy/fmt/doc` + `cargo deny` green, then publish.
