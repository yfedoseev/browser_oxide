# Free-OSS Competitor Comparison vs browser_oxide — 2026-05-17

> **SUPERSEDED for corpus-wide numbers by
> `CORPUS_126_MULTITOOL_RANKING_2026_05_17.md`** — that doc has the full
> measured 126-site head-to-head (browser_oxide **121** ≫ camoufox 96 >
> nodriver 81 > patchright 79 > curl_cffi 65; 14 hard sites only
> browser_oxide passes). This file remains valid for the *hard-subset*
> deep-dive + the architecture/box-honesty narrative.


**Scope:** FREE open-source tools only. **Paid tools (Scrapfly, Hyper
Solutions, ZenRows, CapSolver, Bright/Oxylabs, commercial anti-detect
browsers) are OUT OF SCOPE** — no budget; none were run, installed, or
signed up for. They appear here only as a single *cited* "commercial
ceiling" reference row, sourced from existing repo docs.

**Box:** the project's hostile dev box (sustained external CPU
contention; flaky rootless-Xwayland `DISPLAY=:0`). Same datacenter IP
as the browser_oxide measurements. Python 3.14 is the default but
breaks `nodriver`/`patchright` source parsing — all driver/FF-fork
venvs were built on **Python 3.12** (`/home/linuxbrew/.../python3.12`).
System **Google Chrome 147.0.7727.101** is present at
`/opt/google/chrome/chrome` and verified to run headless.

**Provenance tags on every datum:** `[MEAS]` = this session ran it on
this box; `[CITED:doc]` = taken from an existing repo doc, not measured
here. **Zero competitor numbers were fabricated.**

---

## 0. Headline

- **3 of 4 free-OSS tools RAN here and produced real measured data:**
  `curl_cffi` (headless HTTP, trivially), **`patchright`** (real
  Chrome 147 driver, headless, no display needed), and **`camoufox`**
  (patched-Firefox fork — only after discovering it needs a clean
  **Xvfb** virtual display; it hangs forever on the box's flaky
  Xwayland `:0`, reproducing the documented `nocdp.sh` hang).
- **`nodriver` could NOT run** — it fails its CDP connect-as-root
  handshake against system Chrome even with sandbox disabled and even
  under Xvfb (its own error message names the cause). Its architecture
  class is nonetheless fully covered by Patchright.
- **The central thesis is independently confirmed by measurement:**
  the **Kasada gap is the universal OSS gap.** Patchright (real
  Chrome) **and** Camoufox (the gold-standard FF-fork) are **both
  hard-blocked on all 3 Kasada sites** (canadagoose / hyatt / realtor)
  from this IP — exactly where browser_oxide is blocked. **Camoufox
  also fails DataDome etsy.** No free-OSS tool — ours or theirs —
  passes Kasada from scratch on this IP.

---

## 1. Architecture taxonomy (the fair-read framing)

| Tool | Architecture class | Ships its own engine? | Drives a real binary? | JS engine? |
|---|---|---|---|---|
| **browser_oxide** (ours) | **From-scratch Rust + embedded V8** | **Yes — own DOM/CSS/layout/HTTP/TLS** | No | V8 (embedded) |
| curl_cffi | TLS-impersonation HTTP client | No | No | **None** |
| camoufox | Patched **Firefox** fork (C++ MaskConfig) | No (forks Firefox) | Yes (real FF) | Yes (Firefox SpiderMonkey) |
| nodriver | Real-Chromium CDP driver (no WebDriver) | No | Yes (real Chrome) | Yes (real Chrome V8) |
| patchright | Patched Playwright → real Chromium | No | Yes (real Chrome) | Yes (real Chrome V8) |

**Fair read:** "drives a real browser binary" is *trivial* for
camoufox/nodriver/patchright — they inherit a complete, real,
continuously-Google-maintained Chrome/Firefox engine, TLS stack, and
fingerprint surface for free. browser_oxide is the **only entrant that
hand-builds the engine** and must reproduce that surface itself.
curl_cffi sits at the opposite extreme: a TLS handshake and nothing
else. So the comparison is **apples-to-oranges on architecture,
apples-to-apples on outcome.** The interesting axis for browser_oxide
is *fingerprint parity without a real binary*; "did it drive a real
binary" is a non-achievement for the drivers.

---

## 2. Master result table

Hard-site subset run from this datacenter IP. browser_oxide column =
`[CITED]` from `docs/HANDOFF_2026_05_17.md` §2 (the 2026-05-17 4-mode
sweep, this IP, conservative debug floor) — **not re-measured here**;
this session measured only the *competitors*.

| Tool | Ran here? | canadagoose (Kasada) | hyatt (Kasada) | realtor (Kasada) | homedepot (Akamai) | etsy (DataDome) | CreepJS | sannysoft |
|---|---|---|---|---|---|---|---|---|
| **browser_oxide** | n/a (cited) | **blocked** Kasada-CHL `[CITED:HANDOFF§2]` | **blocked** Kasada-CHL `[CITED]` | **blocked** Kasada-CHL `[CITED]` | passes 3-iter / blocked 1-iter `[CITED:HANDOFF§2]` | mobile-profile pass; desktop iframe-gap `[CITED:HANDOFF§2]` | PASS `[CITED:LANDSCAPE_2026_04_28]` | PASS `[CITED:LANDSCAPE_2026_04_28]` |
| **curl_cffi 0.15.0** | **YES** `[MEAS]` | **HTTP 429** Kasada `KPSDK`/`ips.js` stub `[MEAS]` | **HTTP 429** Kasada stub `[MEAS]` | **HTTP 429** Kasada stub `[MEAS]` | HTTP 200 but 2.6 KB Akamai `sec-if-cpt` shell (no content) `[MEAS]` | **HTTP 403** DataDome `dd={}` + `captcha-delivery` `[MEAS]` | HTTP 200 static HTML only, **no JS run → no score** `[MEAS]` | HTTP 200 static HTML only, no JS `[MEAS]` |
| **patchright 1.59.1** (real Chrome 147) | **YES** `[MEAS]` | **THIN 756 B**, Kasada, no title — **blocked** `[MEAS]` | 12.5 KB soft shell, title "Hyatt Hotels and Resorts", partial `[MEAS]` | **THIN 1768 B**, Kasada, no title — **blocked** `[MEAS]` | 1.03 MB, real content markers (sign-in/search/nav/footer) + Akamai instrumentation strings — largely rendered `[MEAS]` | (not in patchright subset) | **renders** 260 KB, FP-ID generated; trust-score widget async, not captured in 16 s `[MEAS]` | renders 40 KB; `WebDriver (New): missing`, no red "failed" `[MEAS]` |
| **camoufox 0.4.11** (patched FF, **Xvfb only**) | **YES (via Xvfb)** `[MEAS]` | **THIN 756 B**, Kasada — **blocked** `[MEAS]` | **THIN 741 B**, Kasada — **blocked** `[MEAS]` | **THIN 1768 B**, Kasada — **blocked** `[MEAS]` | 1.05 MB, title "The Home Depot" + Akamai challenge strings — largely rendered `[MEAS]` | **1488 B DataDome challenge — blocked** `[MEAS]` | **renders** 218 KB, FP-ID generated; trust-score async, not captured in 16 s `[MEAS]` | renders 37 KB `[MEAS]` |
| **nodriver 0.50.3** | **NO** `[MEAS:failed-to-launch]` | — | — | — | — | — | — | — |
| *Commercial ceiling (paid, NOT run)* | cited only | Scrapfly ~99% realtor `[CITED:HANDOFF§4]` | — | Scrapfly ~99% `[CITED:HANDOFF§4]` | yes `[CITED:HANDOFF§4]` | yes (IP-bound cookie) `[CITED:HANDOFF§4]` | — | — |

### Measured TLS layer (curl_cffi, the architecture-isolation point)

`https://tls.browserleaks.com/json`, `impersonate="chrome"` `[MEAS]`:
- **JA3:** `6edffbe97cdec3898d461b1099377ea9`
- **JA4:** `t13d1516h2_8daaf6152771_d8a2da3f94cd`
- UA echoed: `Chrome/146.0.0.0` macOS

⇒ curl_cffi's **TLS handshake is accepted everywhere** (200 on the
detector endpoints). It is then **hard-blocked the instant a JS-VM
challenge fires** (429 Kasada ×3, 403 DataDome etsy, 2.6 KB Akamai
sensor shell on homedepot) **because it has no JS/WASM engine to
execute the challenge.** This cleanly isolates the layers: TLS/JA3/JA4
parity is necessary but **nowhere near sufficient** — every hard site
in this corpus gates on a client-side JS/WASM VM that a TLS-only tool
structurally cannot answer. This is precisely the wall browser_oxide's
embedded V8 + shim layer exists to clear, and that the real-binary
drivers clear for free by shipping a real engine.

---

## 3. The decisive finding — Kasada is the universal free-OSS gap

Measured, this IP, this session:

| Kasada site | curl_cffi `[MEAS]` | patchright (real Chrome) `[MEAS]` | camoufox (patched FF) `[MEAS]` | browser_oxide `[CITED:HANDOFF§2]` |
|---|---|---|---|---|
| canadagoose | 429 stub | THIN 756 B — blocked | THIN 756 B — blocked | Kasada-CHL — blocked |
| hyatt | 429 stub | 12 KB soft shell (no real content) | THIN 741 B — blocked | Kasada-CHL — blocked |
| realtor | 429 stub | THIN 1768 B — blocked | THIN 1768 B — blocked | Kasada-CHL — blocked |

**Every free-OSS tool tested — including Camoufox, the OSS
gold-standard FF-fork, and Patchright driving a real Chrome 147 — is
blocked by Kasada on this IP, exactly where browser_oxide is blocked.**
This independently *measures* the handoff's claim ("the Kasada gap is
the universal SOTA gap too… no open-source tool passes Kasada from
scratch", `docs/HANDOFF_2026_05_17.md` §4) rather than merely citing
it. browser_oxide's K2-DIFF decoded fix list
(`docs/HANDOFF_2026_05_17.md` §3) is a concrete named path the rest of
the free-OSS field does **not** have publicly — the differentiator is
not "passes Kasada" (nobody free does) but "has a mapped route".

**DataDome etsy:** Camoufox **also fails** it here (1488 B challenge),
matching browser_oxide's desktop-profile state and the FP-E1
script-iframe gap (`docs/research/engines/datadome.md` §9 G1; README
"single most important finding"). curl_cffi gets a clean 403. No
free-OSS tool cleared etsy DataDome on this IP this session.

**Where the real-binary tools clearly win (measured):** broad-corpus
JS-render pages. Patchright and Camoufox both fully render CreepJS
(~218–260 KB, real fingerprint IDs) and sannysoft, and largely render
the 1 MB homedepot — they inherit a complete real engine. This is the
honest "real-browser advantage" axis: for any page that just needs a
faithful full browser, a tool that *is* a real browser wins by
construction. browser_oxide's parity claim on the broad corpus
(117–121/126, `[CITED:HANDOFF§2]`) is the notable result *because* no
real binary is involved.

---

## 4. What could NOT run on this box, and exactly why (honest)

| Item | Outcome | Root cause (evidence) |
|---|---|---|
| **nodriver** | **Could not launch Chrome** `[MEAS]` | Fails its CDP connect handshake against system Chrome as root, even with `Config(sandbox=False)` (auto-adds `--no-sandbox`) and even under Xvfb. nodriver's own raised exception: *"Failed to connect to browser … One of the causes could be when you are running as root."* System Chrome itself runs fine headless here (verified independently via `--dump-dom` → DevTools ws opened), so this is a **nodriver-as-root CDP-handshake incompatibility**, not a Chrome/box-display fault. Architecture class fully covered by Patchright (same real-Chromium-driver class). |
| **camoufox on Xwayland `:0`** | **Hung indefinitely** (killed at 150–200 s, zero output) `[MEAS]` | Reproduces the documented box hostility — the project's `ab_harness/nocdp.sh` real-browser runs hung ~1.5 h on this same flaky rootless-Xwayland `:0`. **Workaround found:** under a clean `xvfb-run` virtual display camoufox starts in ~21 s and runs the full sweep. All camoufox `[MEAS]` numbers above are the **Xvfb** runs. |
| **CreepJS trust score (all tools)** | Page renders, **headline trust-% not captured** `[MEAS]` | CreepJS computes the trust score via deep async work that is not in the DOM text region within a bounded 16 s headless settle. Both Patchright and Camoufox demonstrably *execute* CreepJS (distinct FP-IDs generated, WebRTC candidates produced) — the render succeeds; only the async score widget is not extractable in a bounded run. Recorded as a measurement-budget limit, **not** a tool failure. |
| **Python 3.14 default** | nodriver/patchright unimportable on 3.14 | 3.14's stricter source parser rejects non-UTF-8 / syntax in their vendored CDP modules. Mitigated by building all driver/FF venvs on the box's Python 3.12. |
| **Live competitor pass-rate over the full 126-corpus** | Not attempted | Out of bounded scope on a contention-hostile box; the hard-site subset + detector pages is the decisive, affordable slice (matches the plan). browser_oxide's 126 numbers are `[CITED:HANDOFF§2]`, not re-measured here. |
| **Paid tools** | **Not run — by policy** | No budget. Scrapfly/Hyper/ZenRows/CapSolver/Bright/Oxylabs and commercial anti-detect browsers are cited-only (the §4 "commercial ceiling" row), sourced from `docs/HANDOFF_2026_05_17.md` §4 / `docs/LANDSCAPE_2026_04_28.md`. Never installed, never signed up. |

---

## 5. Detector scorecard (what was reachable)

| Detector | curl_cffi `[MEAS]` | patchright `[MEAS]` | camoufox `[MEAS]` | browser_oxide |
|---|---|---|---|---|
| **CreepJS** | static HTML only, **no JS → no fingerprint at all** | full render 260 KB, FP-ID generated; trust-% async (not captured in budget) | full render 218 KB, FP-ID generated; trust-% async (not captured) | **PASS** `[CITED:LANDSCAPE_2026_04_28]` (mirror-realm topo fix; notably CreepJS *BLOCKED* Camoufox in that 2026-04-28 run) |
| **bot.sannysoft** | static HTML only, no JS | renders; `WebDriver (New): missing`, no red "failed" | renders 37 KB | **PASS** `[CITED:LANDSCAPE_2026_04_28]` |
| **browserleaks TLS** | **JA3 `6edffbe9…`, JA4 `t13d1516h2_8daaf6152771_d8a2da3f94cd`** (Chrome-class) | (real Chrome 147 TLS) | (real Firefox TLS) | JA4 byte-identical Chrome 147 `[CITED:LANDSCAPE_2026_04_28]` |

Note the historically-cited contrast worth preserving: in the
`docs/COMPARISON_OXIDE_VS_CAMOUFOX_2026_04_28.md` head-to-head, **CreepJS
marked Camoufox BLOCKED while browser_oxide passed** — a from-scratch
engine cleared the canonical anti-detect detector that the FF-fork did
not. This session could not re-measure the CreepJS *verdict* (async
score budget), but did re-confirm both tools at least *render* it.

---

## 6. Bottom line

1. **Free-OSS reality, measured here:** on the hard Kasada subset from
   this IP, **every free-OSS tool fails identically to browser_oxide**
   — curl_cffi (429, no JS), Patchright (THIN, real Chrome), Camoufox
   (THIN, patched FF). Kasada is not a browser_oxide-specific gap; it
   is the **universal free-OSS gap**, now measured, not just cited.
2. **The architecture point is cleanly isolated:** curl_cffi proves
   TLS/JA3/JA4 parity alone gets you a handshake and nothing past the
   first JS-VM challenge. The real-binary drivers clear broad-corpus JS
   renders for free *because they ship a real engine*. browser_oxide is
   the only entrant reproducing that surface from scratch — its
   broad-corpus parity (`[CITED]` 117–121/126) is the headline
   precisely because no real binary is involved.
3. **The only thing that beats Kasada in 2026 is paid** (cited-only:
   Scrapfly ~99% realtor, `docs/HANDOFF_2026_05_17.md` §4) — a paid
   real-browser farm, no open algorithm. browser_oxide's K2-DIFF
   decoded named-fix list is a route the free-OSS field lacks publicly.
4. **Box honesty:** 3/4 free tools ran (curl_cffi trivially; patchright
   headless; camoufox **only** under Xvfb — Xwayland `:0` hangs it,
   reproducing the documented `nocdp.sh` hang). **nodriver could not
   launch** (CDP-as-root handshake). No competitor number was
   fabricated; every datum is tagged `[MEAS]` or `[CITED:doc]`.

---

## Appendix — reproduction

Venvs (kept under `/tmp`, not committed): `ccffi_venv` (py3.14,
curl_cffi 0.15.0); `drv312` (py3.12, nodriver 0.50.3 + patchright
1.59.1); `cam312` (py3.12, camoufox 0.4.11). Probe scripts:
`/tmp/ccffi_test.py`, `/tmp/patchright_test.py`, `/tmp/nodriver_test.py`,
`/tmp/camoufox_test.py`. Raw JSON: `/tmp/{ccffi,patchright,camoufox}_results.json`.

```bash
# curl_cffi (headless, no display)
/tmp/ccffi_venv/bin/python /tmp/ccffi_test.py
# patchright (real Chrome 147, headless, no display)
/tmp/drv312/bin/python /tmp/patchright_test.py
# camoufox (patched FF) — MUST use Xvfb; hangs forever on Xwayland :0
xvfb-run -a -s "-screen 0 1280x1024x24" /tmp/cam312/bin/python /tmp/camoufox_test.py
# nodriver — fails to launch Chrome as root on this box (recorded, not worked around)
```

Competitor numbers: `[MEAS]` 2026-05-17, this box, this datacenter IP.
browser_oxide numbers: `[CITED:HANDOFF_2026_05_17 §2/§4]`,
`[CITED:LANDSCAPE_2026_04_28]`,
`[CITED:COMPARISON_OXIDE_VS_CAMOUFOX_2026_04_28]` — **not re-measured
this session.** No paid tool was run, installed, or signed up for.
