# 06 — The No-CDP Real-Browser Oracle Methodology (THE ENABLER)

**Date:** 2026-05-29
**Author:** frontier research agent (oracle/harness design)
**Status:** methodology spec + audit of the proven tooling that already exists
**Scope:** the capture-and-diff harness that makes *all* frontier vendor work
(Kasada, DataDome, Akamai, AWS WAF, in-house) possible. Reusable across all 5 vendors.

> **Why this doc is the enabler.** Every other frontier doc (`05`/`07`/`08`,
> `VENDOR_kasada.md`, `VENDOR_datadome.md`, `VENDOR_akamai.md`) ends at the same
> wall: *"field-diff BO's payload vs a real passing one."* That diff is only
> trustworthy if the "real passing one" was captured **without any automation
> protocol**. Playwright / Patchright / Puppeteer / Selenium / Camoufox-over-CDP
> / Playwright-MCP all drive the browser over CDP or the Firefox Juggler, and
> **Kasada / DataDome / Akamai detect the protocol layer itself** — producing a
> false *"even a real browser fails"* reading that has burned this project before
> (MEMORY: `proxy_not_the_problem`, `state_2026_05_15_playwright_ab_decisive`).
> This doc specifies the *only* valid oracle: a CDP-free real browser observed
> strictly out-of-band, plus the offline BO replay that closes the diff.

**Reading order / cited prior art:**
- `docs/v0.1.0-parity-workflows/external/DETECT_vectors.md` §5 — "BO is structurally
  immune to the CDP/automation-leak class." This is the *thesis the oracle exploits.*
- `docs/v0.1.0-parity-workflows/external/VENDOR_kasada.md` §1.1 (nocdp anchor), §K-1
  (K2-DIFF — the canonical diff this doc generalizes).
- `crates/browser/examples/awswaf_probe.rs` + `aws_capture.rs` — the working offline
  BO-replay oracle for AWS WAF; the template for the BO side of every vendor diff.
- `docs/HANDOFF_2026_05_28b.md` §5.1 — AWS = self-solve *execution* gap (the offline
  oracle runs the worker, the live navigate path does not), the case study that proves
  offline-oracle ≠ live-nav and why both must be in the harness.
- Internal (private, not in this repo): `~/projects/browser_oxide_internal/ab_harness/`
  — the proven scripts (`nocdp.sh`, `tl_capture.sh`, `probe_title.sh`) and the captured
  real-Chrome artifacts (`tl/hyatt.tl_body.bin` = 36 KB accepted Kasada `/tl` sensor,
  `tl/*.pcap`, `tl/*.keys`). This doc documents + generalizes those.

---

## 1. The exact detection mechanism this oracle addresses

The oracle does not bypass a detector — it **measures** one. But to design it we
must be precise about *what gets detected when you try to capture a real session
the naive way*, because that is exactly the trap the oracle is built to avoid.

### 1.1 The CDP / automation-protocol fingerprint (why naive capture lies)

Modern Kasada (`ips.js` VM), DataDome (`tags.js` / device-check), and Akamai BMP
(`sensor_data`) all probe the *control-channel* residue, not just page values:

| Detected artifact | Driver that leaks it | Mechanism |
|---|---|---|
| `Runtime.enable` execution-context event | Puppeteer/Playwright/Patchright (CDP) | CDP sends `Runtime.enable`; anti-bot scripts register a listener and observe the extra `executionContextCreated` event / the timing side-channel. (rebrowser-patches documents this as the #1 Puppeteer tell.) |
| Isolated-world / utility-world names | Puppeteer | `__puppeteer_utility_world__`, `__playwright_*` world names visible to in-page probes. |
| `//# sourceURL=__puppeteer_evaluation_script__` / `pptr:` | Puppeteer/Playwright | regex on eval'd script names in `error.stack`. |
| `cdc_`, `$cdc_`, `$wdc_` globals | ChromeDriver/Selenium | property probe on `window`/`document`. |
| `navigator.webdriver === true` | all WebDriver-based | direct read (most engines now mask, but the *masking itself* is a CDP-set flag in some stacks). |
| Juggler artifacts / `MOZ_*` automation prefs | Camoufox (Firefox + Juggler) | Camoufox drives Gecko over the Juggler protocol (Playwright's Firefox transport); the automation surface and pref flags are observable. |
| CDP timing signature | all CDP | `Runtime.evaluate` round-trip timing is distinguishable from in-process JS. |

**Consequence:** if you capture a "real Chrome" session via Playwright/Patchright
to use as your passing reference, the vendor may have *challenged that session too*
— so your "reference" is itself a blocked/penalized session. Diffing BO against a
poisoned reference sends you hunting phantom divergences. This is precisely the
2026-05-15 CDP-confound error (MEMORY `state_2026_05_15_playwright_ab_decisive`):
Playwright "real Chrome" returned Kasada 429 from this IP, leading to a wrong
"IP-rep, stop engine work" verdict — falsified the next day by the CDP-free probe.

### 1.2 BO's structural position relative to this mechanism

This is the thesis the whole frontier rests on (`DETECT_vectors.md` §5,
verified against source):

> **BO has NO CDP.** It embeds V8 directly via `deno_core`. There is no
> DevTools endpoint, no `--remote-debugging-port`, no Juggler, no isolated
> world, no `cdc_`/`pptr:`/utility-world surface, no `Runtime.enable` event.
> External page scripts run via `execute_script_with_name(code, real_url)`
> (`crates/browser/src/page.rs:419,467`); bootstrap runs as `<anonymous>`
> (`crates/.../snapshot.rs:97`). There is *nothing in the CDP-leak class to find.*

So BO is on the **same side of the line as the no-CDP real browser** and the
**opposite side from every CDP competitor**. The valid oracle must therefore be a
no-CDP real browser, because that is the only reference in BO's own detection
class. A CDP reference would be comparing BO to a *different and more-detected*
class of client.

---

## 2. Does a no-CDP real browser pass? (evidence → engine-addressable)

**Yes, and that is the load-bearing fact that makes the frontier worth working.**

Captured, repeatable, from *this datacenter IP*, zero interaction
(`~/projects/browser_oxide_internal/ab_harness/nocdp.sh` + `.windows.txt`/`.png`):

| Site (vendor) | No-CDP real Chrome 147, this IP | Verdict |
|---|---|---|
| canadagoose (Kasada) | title "Luxury Performance Outerwear … \| Canada Goose" | **PASSES** |
| hyatt (Kasada) | title "Hotel Reservations … Hyatt Hotels and Resorts" | **PASSES** |
| realtor (Kasada) | title "Realtor.com® \| Homes for Sale …" | **PASSES** |
| yelp (DataDome) | interactive DataDome captcha shown on screen | NOT a ban — human gate (out of stealth scope) |
| homedepot (Akamai) | BO's own engine renders it (no Playwright ref needed) | engine-tractable |

Kasada's 429 challenge page has an **empty `<title>`**; the three real product
titles are impossible if Kasada had blocked. This rules out, with measurement:

1. **IP reputation** — real Chrome passes from the same IP.
2. **Behavioral absence** — zero mouse/scroll/keyboard, still passes.
3. **Paid-farm requirement** — vanilla launch, fresh profile, first visit.
4. **CDP** — BO exposes none (so "BO fails because CDP" is impossible).

⇒ **The residual gap is a passive, static engine-vs-real-Chrome surface
divergence** that the vendor JS measures. That is **ENGINE-ADDRESSABLE by
construction** — the only question per site is whether the divergent fields are
public-engine-stubable or require a `vendor_solvers` VM (the §5 verdict).

The oracle's job: turn "unknown divergence" into a **named, finite field list**
by diffing the real (passing) payload against BO's payload for the *same challenge*.

---

## 3. The concrete oracle architecture (the no-CDP advantage in action)

Three components. (a) and (b) capture the *real passing session* with zero
automation protocol. (c) replays the *same challenge* through BO's V8 offline and
field-diffs. The harness is vendor-agnostic; only the payload extractor (§3.4) is
per-vendor.

```
 ┌────────────────────────── REAL SIDE (no CDP) ──────────────────────────┐
 │ (a) non-CDP driver        (b) passive capture                          │
 │  human / OS-input    ───►  vanilla Chrome  ───►  SSLKEYLOGFILE + tcpdump│
 │  (xdotool/headful)        (no --remote-debugging)   (or mitmproxy CA)   │
 │       observe out-of-band: WM_NAME title + screenshot (undetectable)    │
 └───────────────────────────────────────────────┬───────────────────────┘
                                                  │ decrypt + reassemble (tshark)
                                                  ▼
                              real_payload.bin  +  clearance cookies
                                                  │
 ┌────────────────────────── BO SIDE (offline) ──┼───────────────────────┐
 │ (c) feed the SAME challenge HTML/JS into BO's V8 (awswaf_probe pattern) │
 │     run event loop to idle, intercept the sensor POST PRE-encryption    │
 │     ──► bo_payload.bin                                                   │
 └────────────────────────────────────────────────────────────────────────┘
                                                  │
                                                  ▼
                              (d) DIFF: decrypt both → field-diff → named gap list
```

### 3.1 Component (a) — the non-CDP driver

Three tiers, in order of fidelity. **Tier 0 is already built and proven.**

**Tier 0 — headful + OS-observed (already exists, zero automation surface).**
`ab_harness/nocdp.sh` launches `/opt/google/chrome/chrome` with *only* the flags
this headless box physically requires and **none that are JS-observable**:

```
chrome --user-data-dir=<fresh> --no-sandbox --ozone-platform=x11 \
       --no-first-run --no-default-browser-check \
       --window-size=1366,900 <URL>
```

- No `--remote-debugging-port`, no `--enable-automation`, no `--headless`.
- Fresh `--user-data-dir` per run = first-visit, like a user double-clicking a link.
- `--no-sandbox` and `--ozone-platform=x11` are **not** JS-observable (verified —
  they affect the OS sandbox + the rendering backend, not any `navigator`/`window`
  property the vendor JS can read).
- **Observation is purely out-of-band:** `import -window root` screenshot +
  `xprop WM_NAME` (the X window title mirrors `document.title`). No website can
  read its own window-manager title, so the observation channel is undetectable.

This is the faithful mechanization of the user's manual "double-click the link"
test. **It is the ground-truth verdict instrument** ("did the site render?") and
it already gave the canadagoose/hyatt/realtor PASS evidence in §2.

**Tier 1 — OS-level synthetic input (for sites needing a click/scroll).** When a
site gates content behind one interaction (a cookie banner, a "continue" button),
drive it with `xdotool` (X11) or `pyautogui` (X11/Wayland), which inject input at
the **kernel/X-event layer** — indistinguishable from a physical mouse/keyboard
and *not* via any browser automation API:

```bash
# install once (apt): xdotool
DISPLAY=:0 xdotool mousemove --sync 640 420 click 1     # real X button event
DISPLAY=:0 xdotool key Return
```

`xdotool` is **not currently installed** on this box (`which xdotool` → not found)
— add it (`apt-get install xdotool`) only when a frontier site is proven to need a
gating interaction. For Kasada (zero-interaction pass per §2) Tier 0 suffices.

**Tier 2 — fully human (the gold reference).** For a one-off canonical capture, a
human opens vanilla Chrome on a headful machine and browses normally while §3.2
captures passively. Highest fidelity (real biometrics), lowest throughput. Use to
mint the *reference* payload that Tier 0/1 reproductions are checked against.

> **What is FORBIDDEN as a driver:** Playwright, Patchright, Puppeteer, Selenium,
> ChromeDriver, Playwright-MCP, Camoufox, or *any* `--remote-debugging-port`
> launch. All are CDP/Juggler and poison the reference (§1.1). This is a hard rule.

### 3.2 Component (b) — passive traffic capture (two interchangeable methods)

**Method A — SSLKEYLOGFILE + tcpdump (preferred; fingerprint-intact).** Already
built: `ab_harness/tl_capture.sh`. Chrome writes its TLS session secrets to a
file (`SSLKEYLOGFILE=…`, a client-side debug feature — **not** a proxy, **not**
observable by the server, **changes nothing on the wire**); `tcpdump` captures
packets passively; `tshark` decrypts + reassembles HTTP/2 offline:

```bash
sudo tcpdump -i eth0 -s 0 -w cap.pcap 'tcp port 443' &
SSLKEYLOGFILE=keys.txt chrome --user-data-dir=<fresh> --no-sandbox \
    --ozone-platform=x11 <URL> &     # real Chrome TLS/JA3/JA4 + H2 fully intact
# ... wait, kill, then:
tshark -r cap.pcap -o tls.keylog_file:keys.txt \
    -Y 'http2.headers.method=="POST"' \
    -T fields -e http2.headers.authority -e http2.headers.path \
              -e http2.body.fragment
```

This is the **canonical method** for the frontier because it preserves the real
Chrome ClientHello / JA3 / JA4 / H2 SETTINGS fingerprint — critical when the
vendor cross-checks TLS against the JS payload. tcpdump and tshark are installed
(`/usr/bin/tcpdump`, `/usr/bin/tshark`). It already produced the **36 KB accepted
Kasada `/tl` reference** at `ab_harness/tl/hyatt.tl_body.bin`.

**Method B — mitmproxy TLS-terminating addon (when you need request *editing* /
live replay, not just observation).** mitmproxy terminates TLS with its own CA
(installed into the Chrome profile's NSS DB), so it **changes the TLS fingerprint
to mitmproxy's** — acceptable *only* when the vendor's pass does not depend on TLS
(or when you separately confirm TLS parity via Method A). The advantage is a
programmatic Python addon that dumps decrypted vendor payloads directly:

```python
# nocdp_oracle_addon.py  —  run: mitmdump -s nocdp_oracle_addon.py
#   chrome ... --proxy-server=http://127.0.0.1:8080  (note: this is NOT CDP)
import json, pathlib
VENDOR_PATHS = {
    "kasada":   lambda f: f.endswith("/tl") or "/149e9513" in f,
    "datadome": lambda f: "datadome" in f or "/js/" in f and "captcha-delivery" in f,
    "akamai":   lambda f: "/akam/" in f or f.endswith("/sensor_data"),
    "awswaf":   lambda f: "challenge.js" in f or "/verify" in f or "token" in f,
}
OUT = pathlib.Path("oracle_capture"); OUT.mkdir(exist_ok=True)
def request(flow):
    p = flow.request.path
    for vendor, match in VENDOR_PATHS.items():
        if match(p):
            (OUT / f"{vendor}_{flow.request.host}_req.bin").write_bytes(flow.request.raw_content or b"")
            (OUT / f"{vendor}_{flow.request.host}_req.headers.json").write_text(
                json.dumps(dict(flow.request.headers)))
def response(flow):
    # capture clearance cookies set by the vendor's accept response
    sc = flow.response.headers.get_all("set-cookie")
    if sc:
        (OUT / f"{flow.request.host}_setcookie.txt").write_text("\n".join(sc))
```

mitmproxy/mitmdump is **not currently installed** (`which mitmdump` → not found);
install via `pipx install mitmproxy` when Method B is needed. Note: `--proxy-server`
is a standard browser network flag, **not** an automation protocol — it does not
leak CDP. The only fidelity cost is the TLS fingerprint (mitigate per above).

**Method C — Chrome NetLog (richest request/response metadata, no TLS change).**
`chrome --log-net-log=netlog.json --net-log-capture-mode=Everything <URL>` writes
a structured JSON log of every request/response (headers, timing, body presence)
**without altering the wire**. Bodies are not always fully captured, so NetLog is
best as a *correlation* layer (which requests fired, in what order, with what
headers) alongside Method A's body bytes. `--log-net-log` is a logging flag, not
CDP. Useful to confirm BO's request *ordering/timing* matches real Chrome (the
AWS §5.1 lesson: the divergence was execution *order/drain*, not payload bytes).

### 3.3 Component (c) — BO offline replay (the awswaf_probe pattern, generalized)

The BO side already exists for AWS WAF and is the template for every vendor.
`crates/browser/examples/awswaf_probe.rs`:

1. Reads a captured challenge HTML (with an instrumentation Proxy prepended that
   records every `navigator`/`screen`/`window.chrome`/`Function.prototype.toString`
   access into `window.__awswafProbe`).
2. `Page::from_html_with_url(&html, &url, Some(profile))` (`page.rs:548`) — loads
   it into a real BO V8 isolate with the production stealth profile, believing it
   was served from the real origin (so CSP / `sec-fetch-site` / same-origin
   checks behave correctly).
3. `page.event_loop().run_until_idle(Duration::from_secs(5))` — runs the async
   challenge to completion (promise chains, internal `fetch`es, blob workers).
4. Dumps the per-property access trace + whether the challenge proceeded
   (`getTokenCalled`) or **silently bailed** — the exact signal the Kasada VM
   gives when it detects an env divergence (`state_2026_05_15`: ips.js bails
   *before* the `/tl` POST).

**Generalization to a reusable `nocdp_oracle` example** (`crates/browser/examples/`):
add a thin per-vendor sensor-POST interceptor so the BO replay emits its *own*
payload **pre-encryption** (the BO analogue of the real PRE-XOR capture):

- **Kasada:** intercept the in-VM `XHR.send`/`fetch` to `/tl` and capture the body
  *before* the `xor(plaintext, "omgtopkek")` wrapper (the wrapper is known:
  `VENDOR_kasada.md` §1.2). This is exactly the K2-DIFF in-VM dump tool
  (`VENDOR_kasada.md` §K-1). Net-side, the request is already logged:
  `crates/net/src/lib.rs:680,719,812,854` `eprintln`s on any `/tl` (and `/mfc`,
  `/akam/13`, `/r`) request — so you can confirm whether BO even *reaches* the
  POST (the measured failure was: it never fires → silent pre-POST bail).
- **DataDome:** capture the device-check / `tags.js` POST body.
- **Akamai:** capture the `sensor_data` POST body pre-`bmak` encoding.
- **AWS WAF:** already covered (`awswaf_probe.rs`); the verify/token call.

The net stack also needs the *raw challenge* to feed (c). `aws_capture.rs` does
this with faithful TLS: `net::HttpClient::new(&profile)` + `nav_headers_for_url`
+ `get_follow_with_headers` fetches the live challenge stub *with BO's own Chrome
TLS* — so the captured stub is exactly what the vendor would serve BO. Use this to
keep the offline replay honest (same challenge BO would really get), or use the
Method-A `tl/*.pcap` extraction of the real challenge HTML.

### 3.4 Component (d) — the diff

For each vendor, decrypt both payloads to plaintext, then field-diff:

- **Kasada `/tl`:** `body = base64(json({"data": base64(xor(plaintext,
  "omgtopkek"))}))` (`VENDOR_kasada.md` §1.2). Decode real (`tl/hyatt.tl_body.bin`)
  and BO captures the same way → JSON field-by-field diff. The error-report path
  uses the same XOR; canvas-FP RGBA buffers decode as raw image data (MEMORY:
  blobs #2-5 are canvas, #0 is the field report, #6 carries `bot1225`).
- **DataDome/Akamai/AWS:** decode per each vendor's known encoding (documented in
  the respective `VENDOR_*.md`); diff the structured fields.

Output: a **named, finite list of divergent fields**, each tagged
public-engine-stubable vs `vendor_solvers`-only. This converts the "unknown
holistic ML tail" into the K2-DIFF deliverable (`VENDOR_kasada.md` §K-1).

---

## 4. The validation plan (capture + diff, step by step, per vendor)

Reusable runbook. Steps 1–4 are vendor-agnostic; step 5 is the per-vendor decoder.

1. **Verdict baseline (Tier 0).** `nocdp.sh <slug> <url> 25` → confirm the real
   browser PASSES (non-empty product `<title>`, screenshot shows content). If it
   does NOT pass no-CDP from this IP, STOP — the gap may be IP/geo (§5) and this
   oracle does not apply. (Re-confirm periodically; vendor verdicts drift.)
2. **Real passing payload (Method A).** `tl_capture.sh <slug> <url> 32` →
   `tl/<slug>.pcap` + `.keys`; extract the sensor POST body + the accept-response
   `set-cookie` clearance. Store as the reference (`tl/<slug>.<vendor>_body.bin`).
   Confirm via NetLog (Method C) the request *order* (challenge → sensor → accept
   → real page) — this is the AWS §5.1 guardrail.
3. **Same challenge into BO (offline).** Feed the captured challenge HTML (from
   step 2's pcap, or `aws_capture.rs`-fetched) to the `nocdp_oracle` example
   (`awswaf_probe.rs`-derived). Run `run_until_idle`. Assert BO reaches the
   sensor POST at all (net log `lib.rs:680`); if it silently bails, that bail is
   the first finding (the env-divergence is *before* the network, the Kasada case).
4. **Capture BO's payload pre-encryption** via the §3.4 in-VM interceptor →
   `bo_<slug>.<vendor>_body.bin`.
5. **Decode both + field-diff** (§3.4 decoders) → named gap list. For each field:
   verify the real-Chrome value out-of-band with `probe_title.sh '<js>'`
   (CDP-free single-property probe — already built; reads result via window
   title), **including inside a child iframe/realm** (MEMORY: many Kasada
   divergences live in a child realm the main-window bootstrap never populated —
   the `_getIframeWindow` / per-realm bootstrap lever).
6. **Fix → re-replay → re-diff.** Implement the public-engine stub, rebuild,
   re-run steps 3–5, confirm the field converged, gate on `chrome_compat`/`net`/
   `akamai` green. Loop until the named list is empty or only `vendor_solvers`
   fields remain.

**Two failure modes the oracle distinguishes (the key diagnostic value):**
- **Payload-content divergence** (a field differs) → public-engine stub candidate.
- **Execution divergence** (BO computes the right payload offline but never *emits*
  it live) → the AWS §5.1 / Kasada-pre-POST-bail class: a live-nav drain /
  event-loop / realm-propagation gap, not a fingerprint value. Method C (NetLog
  request ordering) + the net-side POST log (`lib.rs:680`) catch this; the offline
  oracle alone would *miss* it (it runs the worker to idle artificially). **This is
  why the harness needs all three: Tier-0 verdict, offline replay, AND live-nav
  request-ordering capture.**

---

## 5. Honest verdict (engine-addressable / vendor_solvers / IP-geo)

The oracle is **methodology, not a fix** — it is universally **ENGINE-ADDRESSABLE
and fully public** (no bypass code; it captures/observes/replays). Its *output*
classifies each site:

| Cluster | No-CDP real browser passes? | Oracle verdict |
|---|---|---|
| **Kasada** (canadagoose/hyatt/realtor) | **YES**, this IP, zero-interaction (§2) | **ENGINE-ADDRESSABLE.** The oracle (K2-DIFF, end-to-end) bounds the gap to a named field list. Honest caveat (`VENDOR_kasada.md` §4, `DETECT_vectors.md` §6): the residual tail is axis-2 *lies* + child-realm propagation that BO can only **minimize** in pure JS, not zero out, vs Camoufox's C++ spoof. If K2-DIFF shows ≤3 public-stubable fields → pursue; if it shows a deep VM-internal env probe → that field is `vendor_solvers`. The oracle is the instrument that decides which. |
| **DataDome** (etsy) | partial (yelp = interactive captcha = human gate, out of scope) | **MIXED.** 3 public primitives shipped (`VENDOR_datadome.md`); the daily-rotating-key WASM signal solver is **`vendor_solvers`**. The oracle confirms the device-check payload diff but cannot make the WASM key public-engine. |
| **Akamai** (bestbuy/homedepot) | homedepot engine-tractable; bestbuy = no from-scratch engine passes | **ENGINE-ADDRESSABLE for the sensor + sec-cpt** (oracle diffs `sensor_data`); the per-day BMP obfuscation + PoW is **`vendor_solvers`**. The AWS §5.1 lesson (live-nav drain) is an *engine* fix the oracle's Method-C ordering capture pinpoints. |
| **AWS WAF** (amazon-in) | — | **ENGINE-ADDRESSABLE execution gap** (`awswaf_probe.rs` already proves the offline self-solve works; §5.1 = wire it into the live navigate drain). The oracle's offline-vs-live split is exactly this case. |
| **In-house geo** (wildberries/ozon, douyin-sig) | — | Likely **IP-GEO-BOUND** (RU/CN geo) and/or Firefox-signature — the oracle's step-1 Tier-0 verdict is the honest gate: if no-CDP real Chrome *from a clean in-region IP* still fails, escalate to geo, not engine. Do **not** assert IP-geo without a captured hard-403 from `nocdp` (MEMORY rule). |

**Bottom line.** This oracle is the prerequisite that makes every "field-diff vs a
real passing payload" step in the frontier docs *trustworthy*. It exploits BO's one
categorical advantage — **no CDP, same detection class as a real non-CDP browser** —
to obtain a reference no CDP competitor (incl. Camoufox v150) can produce. Most of
it (`nocdp.sh`, `tl_capture.sh`, `probe_title.sh`, `awswaf_probe.rs`, `aws_capture.rs`,
the net-side `/tl` log) **already exists and is proven**; the remaining build is (1)
generalize `awswaf_probe.rs` → `nocdp_oracle` with per-vendor pre-encryption
interceptors, (2) add the optional mitmproxy/NetLog methods + `xdotool` Tier-1 input
when a site needs interaction, (3) run K2-DIFF end-to-end (Kasada) as the first full
exercise of the harness.

---

## 6. Build checklist (what to add to the repo, all public)

- [ ] `crates/browser/examples/nocdp_oracle.rs` — generalize `awswaf_probe.rs`:
      parametrize the instrumentation Proxy + add a per-vendor sensor-POST
      pre-encryption interceptor (Kasada `/tl` PRE-XOR, DataDome device-check,
      Akamai `sensor_data`).
- [ ] `crates/browser/examples/nocdp_capture.rs` — generalize `aws_capture.rs` to
      fetch any vendor's challenge stub with faithful BO TLS for offline replay.
- [ ] Move the proven `ab_harness/{nocdp,tl_capture,probe_title}.sh` into a
      public `tools/oracle/` (they contain no bypass code — pure capture/observe).
      Keep captured `*.pcap`/`*.keys`/`*.bin` artifacts **private** (they carry
      session tokens) — reference them, do not commit.
- [ ] `tools/oracle/mitm_addon.py` — the §3.2 Method-B addon (vendor path map).
- [ ] `tools/oracle/decode_kasada_tl.py` — base64→json→base64→XOR(omgtopkek) +
      JSON field-diff (the §3.4 / §4-step-5 decoder; first of the per-vendor set).
- [ ] `apt-get install xdotool` + `pipx install mitmproxy` — only when a frontier
      site is proven to need Tier-1 input or Method-B editing.
