# Research — Kasada bypass landscape (2026-04-29)

> Public information, code, and reverse-engineering writeups for getting through
> Kasada bot defense. Compiled to seed an eventual `crates/kasada/` solver crate
> for browser_oxide. Built by web-searching GitHub, Substack, Medium,
> independent research blogs, and pricing pages — no paid services subscribed.
>
> **Context:** browser_oxide already matches Chrome 147 byte-for-byte at TLS
> + HTTP/2 + JS fingerprint (99% probe parity). Kasada still serves us its
> JS-VM challenge on canadagoose / hyatt / realtor; our V8 runs the bootstrap
> but bails to error reports (`reporting.cdndex.io/error`) instead of
> completing the `/tl` POW handshake to get a `x-kpsdk-ct` token. Real Chrome
> 147 from the same machine via Playwright MCP completes the handshake.

---

## 1 — Open-source projects (ranked by usefulness to us)

### 1.1 nullpt.rs — Devirtualizing Nike's Bot Protection (★★★★★)

**URL**: <https://nullpt.rs/devirtualizing-nike-vm-1>
**Type**: Long-form RE writeup with reproducible code
**Targets**: Nike's `accounts.nike.com/[UUID]/ips.js` (= Kasada)

This is the deepest public Kasada VM analysis. Documents:

- Bytecode decoding algorithm (full code excerpted below).
- ~60 atomic opcodes (arithmetic, logical, register read/write).
- Register-machine model (no stack); state is `t.g[]` with instruction
  pointer at `t.g[0]`.
- String decryption: `String.fromCharCode((4294967232 & l) | ((39 * l) & 63))`.

Quoted bytecode decoder verbatim:

```javascript
function decodeBytecode(n) {
  const {V, W} = {V: "abcdefg...0123456789", W: 50};
  const o = V.length - W;
  let u = [];
  for (let e = 0; e < n.length; ) {
    for (let f = 0, c = 1; ; ) {
      const a = V.indexOf(n[e++]);
      if ((f += c * (a % W)), a < W) {
        u.push(0 | f);
        break;
      }
      f += W * c;
      c *= o;
    }
  }
  return u;
}
```

XOR step on a chunk of decoded bytecode:
`c = r[i + v.indexOf(".")] ^ i` — XOR using array length / index.

**Status**: Part 1 covers architecture + decoder; Part 2 promised
opcode semantics + full decompilation but not located in search results
as of 2026-04-29.

**License**: blog post, no formal license — informational use.

### 1.2 opcodes.fr — Reverse engineering Kasada javascript VM obfuscation (★★★★)

**URL**: <https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1>
**Date**: 2021-08
**Status**: Domain returned ECONNREFUSED on 2026-04-29 fetch — try again
or use Wayback Machine: <https://web.archive.org/web/*/https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1>

Covers VM opcode set, regex match in babel-helpers package that lets the
authors compare a "compiler" input/output for ground truth, and an
analysis methodology. Description from search results: *"clearly not
the end" — there was "still some work to do, especially if you want to
reverse the code that actually runs inside the VM."*

### 1.3 Humphryyy/Kasada-Deobfuscated (★★★)

**URL**: <https://github.com/Humphryyy/Kasada-Deobfuscated>
**License**: not specified
**Last activity**: 4 commits total — abandoned
**Language**: JavaScript
**What it produces**: partially-deobfuscated `p_deobed.js` alongside
the original `p.js`.

Quotes:
- *"The script was obfuscated by replacing most strings with a function to grab the string from an array and decode it."*
- *"The next step of deobfuscating this script is coming up with a strategy to make the VM logic starting at line 1508 more readable."*
- *"To deobfuscate the script I used AST manipulation."*

Useful as a starting point + technique reference, NOT a working solver.

### 1.4 0x6a69616e/kpsdk-solver (★★★ — Playwright-based, not for us)

**URL**: <https://github.com/0x6a69616e/kpsdk-solver>
**License**: MIT
**Stars**: 78 · **Status**: archived 2025-06-10
**Language**: JavaScript

Wraps Playwright. Doesn't reimplement the algorithm — it runs Kasada's
legitimate SDK inside a real browser and exposes hooks. Key README quote:
*"Available as a replacement to Browser.newPage() and BrowserContext.newPage()."*

NOT useful as a direct port (we don't run a real browser), but its
hook surface (custom script import, "same-page client token regeneration",
"Interact with Kasada's Fetch API") tells us where Kasada's surface
intersects browser intrinsics.

Notable limitation: *"Fails to bypass detection on… Chrom(e/ium) browsers; Firefox preferred."* That's a fingerprint tell of theirs we can study.

### 1.5 youdie323323/enigma — Kasada VM recreation (★★★★)

**URL**: <https://github.com/youdie323323/enigma> (referenced in WebSearch results as "recreation of Kasada's virtual machine")
**Status**: TBD — needs separate fetch.
**Why it matters**: A clean re-implementation of the Kasada VM for
analysis is the closest a public project gets to a usable foundation.

### 1.6 lktop/kpsdk (★★ — analysis only, paywalled)

**URL**: <https://github.com/lktop/kpsdk>
**Language**: JavaScript · **6 commits**

Author analyses `ips.js` and notes:
*"register-like program behavior, breakpoint analysis at fixed locations can reveal the execution flow"*
*"the generation algorithm is very simple [once deconstructed]"*

But the actual algorithm is **withheld and sold via QQ/email**, not in the public repo. Not useful directly.

### 1.7 unicorn-aio/kpsdk (★★ — Nike SNKRS targeted, paywalled)

**URL**: <https://github.com/unicorn-aio/kpsdk>
**Language**: Python (69%) + JS (31%) · **5 commits**

Toolkit targeting Nike SNKRS endpoints (Kasada-protected). Headers
mentioned: `x-kpsdk-ct`, `x-kpsdk-cd`, `ak_bmsc_nke`, `_abck`. Docs
behind contact / `us.unicorn-bot.com/docs`. Commercial.

### 1.8 nixbro/Kasada-Solver, ChrisYP/ChrisYP.github.io (★★)

- <https://github.com/nixbro/Kasada-Solver> — billed as "Kasada Bot Protection: A High-Level Overview" — overview-level, no working code.
- <https://github.com/ChrisYP/ChrisYP.github.io/blob/main/en-US/kasada.md> — personal blog page on Kasada.

### 1.9 Mrclintons/kpsdk-1 (★)

**URL**: <https://github.com/Mrclintons/kpsdk-1>
Appears to be a clone of unicorn-aio/kpsdk. Skip.

---

## 2 — Commercial bypass services (the ceiling we're trying to undercut)

### 2.1 Hyper-Solutions / hypersolutions.co (★★★★)

**Repos**: <https://github.com/Hyper-Solutions/hyper-sdk-py> (Python) and <https://github.com/Hyper-Solutions/hyper-sdk-js> (TS/JS)
**License**: MIT (the **SDK** is MIT — it's a thin client to a paid backend)
**Last commit**: April 2024 (v0.1.2)
**Pricing**: Pay-as-you-go + subscription tiers (rates not public).

**Architecture**: SDK is just a client. All `Session("api-key")` calls
proxy to Hyper Solutions' servers — they generate the actual sensor /
ct / cd tokens server-side. So this is **a paid API**, not a local
solver, even though the SDK code is open-source. Useful only as
reference for the **header / cookie shapes** their API claims to produce:

- Akamai: sensor_data, sec-cpt challenges, pixel challenges, cookie validation
- Incapsula: Reese84 sensors, UTMVC cookies
- Kasada: payload/CT tokens, POW/CD tokens
- DataDome: interstitial challenges, slider captchas, tags payload
- Vercel BotID: x-is-human header

That's the most precise public taxonomy of "what a Kasada bypass actually
ships" you can get without paying — which is more useful than it sounds
when designing our own crate's API surface.

### 2.2 NoCaptcha.io (shrotam.com Kasada page)

**URL**: <https://shrotam.com/en/kasada>
Captcha-solver-style service. Mentioned only for completeness.

### 2.3 ZenRows — How to Bypass Kasada in 2026

**URL**: <https://www.zenrows.com/blog/kasada-bypass>
Marketing page for ZenRows' scraping API; describes the obstacle but
not the solution at a code level.

### 2.4 Scrapfly — Bypass Kasada

(Mentioned in adjacent searches; same shape — paid scraping API.)

---

## 3 — Public protocol facts (what we can pin down without paying)

### 3.1 Request flow

1. Initial GET to a protected page.
2. Server returns the page with a `<script>` that points at
   `<tenant>/<region>/ips.js` and an inline KPSDK init: `window.KPSDK={};KPSDK.now=...;KPSDK.start=KPSDK.now();`.
3. The KPSDK loader fetches `ips.js`, evaluates it (this IS the VM bootstrap).
4. The VM runs ~60 opcodes, executes a fingerprint-collection program
   over `globalThis`, `navigator`, `document`, prototypes, intrinsics.
5. The VM POSTs to `<tenant>/<region>/tl` with a body that contains the
   collected fingerprint data, encoded + (probably) XOR'd with a
   VM-derived key.
6. `/tl` responds with `x-kpsdk-cr: true`, `x-kpsdk-st: <server-ms>`, and
   (on success) `x-kpsdk-ct: <session-token>`.
7. Subsequent requests carry the `x-kpsdk-ct` token + a freshly-computed
   `x-kpsdk-cd: {workTime, id, answers, duration, st, rst}` PoW solution.

Browser_oxide is good through step 4 (we evaluate ips.js) but **bails
at step 5** — VM detects something it doesn't like and POSTs to
`reporting.cdndex.io/error` instead.

### 3.2 ct vs cd token semantics

- `x-kpsdk-ct` (Computed Token): expensive, **reusable** for the session.
- `x-kpsdk-cd` (Computed Data): cheap, **single-use**, recomputed per request via PoW.

We already have a working PoW solver (`crates/stealth/src/kasada.rs`)
that produces a valid `x-kpsdk-cd`. It's just useless without ct.

### 3.3 Error blob (`reporting.cdndex.io/error`)

- Wire format: JSON `{"data":"<base64-of-encoded-bytes>"}`
- Inner bytes are XOR-encoded with a **runtime-derived key** (not a fixed string — confirmed: simple XOR with `KPSDK`, `kasada`, `ips.js`, hostnames, tenant UUIDs, header names, `j-1.2.386` all fail; common prefix `14 4F` shared across blobs but differ at byte 2, ruling out periodic XOR).
- Likely scheme: VM-register-state-derived rolling key (consistent with public deobfuscation work above).
- Decoding the blob ≈ devirtualising the VM (sections 1.1, 1.2, 1.5 above).

### 3.4 Production version on canadagoose (2026-04-29)

`x-kpsdk-v: j-1.2.386` (captured from the Playwright MCP run).

---

## 4 — Adjacent prior art (worth mining for techniques)

- **Camoufox** — uses Patchright underneath, a Playwright fork that re-exposes Function/eval after Kasada's prototype-pin checks. The patches in `Patchright/patches/` are a high-signal source for individual Kasada/Cloudflare/Akamai probe interventions.
- **Veritas' TikTok VM analysis** — same VM-virtualisation shape, similar techniques. Cited by nullpt.rs as a methodology blueprint.
- **JavaScript Deobfuscation** — Kasada themselves have a writeup on *why* they use VM obfuscation (`https://www.kasada.io/javascript-deobsfusction-bot-defenses/`). Useful for understanding the design intent (anti-deobfuscator countermeasures, prototype pinning, etc.) which is what trips us up.

---

## 5 — Realistic implementation paths for browser_oxide

Ranked by ROI:

### 5.1 Path A: full local re-implementation (1–2 months)

1. Devirtualize the current `j-1.2.386` ips.js using the techniques in §1.1, §1.2, §1.5.
2. Identify the specific environment probe(s) our V8 fails. Likely candidates:
   - `Function.prototype.toString.call(...)` on patched intrinsics.
   - `Object.getOwnPropertyDescriptor(globalThis, "X")` shape mismatch.
   - Performance-timer counters / typed-array tricks the VM uses for
     anti-debug.
   - Specific CSS/layout queries that depend on real rendering.
3. Patch our V8 environment until the VM runs to completion.
4. Re-test canadagoose / hyatt / realtor.

**Risk**: Kasada rotates the VM. The surface we patch this month may
not match next month's `j-1.2.387`. Need ongoing maintenance.

### 5.2 Path B: ride a deobfuscator (3–7 days)

1. Stand up `youdie323323/enigma` (§1.5) or Humphryyy's deobfuscator (§1.3) locally.
2. Run our captured 67 KB error blob through whatever decoder it can
   produce.
3. Read the human-readable VM trace to identify the failing probe.
4. Fix forward.

**Risk**: Deobfuscators may be stale relative to the current `j-1.2.386`.
Either we update them ourselves (time-equiv to Path A) or we get a
partial trace and must guess.

### 5.3 Path C: pay Hyper-Solutions for Kasada CT/CD generation (operational, immediate)

`hypersolutions.co` provides an API that produces ct/cd tokens. We POST
our session info to their server, they return a valid token, we attach
it to outbound requests. **Engineering cost: a thin Rust client + an
API key.** Per-request cost depends on their pricing tier.

**Trade-off**: it's a paid third-party dependency, not the
"best stealth engine ever" goal. But it's instantly the same as
buying a residential proxy — operational, not architectural.

### 5.4 Path D: accept and move on (free)

The 3 Kasada-CHL sites are 2.4% of the 126-site sweep. Phase 7 closed
the wide-spectrum fingerprint surface; Kasada specifically went deeper
than that. Recover them later when there's budget for §5.1 or §5.3.

---

## 6 — Open follow-ups for whoever picks this up

- **Fetch youdie323323/enigma directly** and confirm whether the README documents an end-to-end `j-1.2` VM.
- **Try opcodes.fr Part 1 + Part 2 again** (the domain was offline on 2026-04-29; try Wayback Machine or a different network egress).
- **Run the captured 67 KB error blob through Humphryyy's deobfuscator** — even with VM logic still partially obfuscated, the JSON-tree of what the blob encodes may be readable.
- **Capture a Kasada `/tl` POST from a SUCCESSFUL Playwright-MCP run** against canadagoose — i.e., add a Playwright network listener that snapshots the body of the POST that Kasada's VM sent. That gives a known-good ct-issuance payload to diff against ours.

---

## 7 — Sources (verbatim URLs)

### Open-source / RE writeups
- <https://github.com/Humphryyy/Kasada-Deobfuscated>
- <https://github.com/0x6a69616e/kpsdk-solver>
- <https://github.com/lktop/kpsdk>
- <https://github.com/unicorn-aio/kpsdk>
- <https://github.com/nixbro/Kasada-Solver>
- <https://github.com/Mrclintons/kpsdk-1>
- <https://github.com/youdie323323/enigma>
- <https://github.com/ChrisYP/ChrisYP.github.io/blob/main/en-US/kasada.md>
- <https://nullpt.rs/devirtualizing-nike-vm-1>
- <https://opcodes.fr/publications/2021-08/kasada-javascript-vm-obfuscation-reverse-part1>
- <https://www.kasada.io/javascript-deobsfusction-bot-defenses/>
- <https://www.kasada.io/bot-detection-do-you-see-what-i-see/>
- <https://www.scribd.com/document/899949744/Understanding-Kasada-Bot-Defense-and-Bypass-Mechanisms> (mirrored writeup)

### Commercial
- <https://github.com/Hyper-Solutions/hyper-sdk-py>
- <https://github.com/Hyper-Solutions/hyper-sdk-js>
- <https://hypersolutions.co>
- <https://www.zenrows.com/blog/kasada-bypass>
- <https://shrotam.com/en/kasada>

### Adjacent VM-RE techniques
- <https://synthesis.to/2021/10/21/vm_based_obfuscation.html> (Tim Blazytko — VM disassemblers)
- <https://jwillbold.com/posts/obfuscation/2019-06-16-the-secret-guide-to-virtualization-obfuscation-in-javascript/>
- <https://github.com/aesthetic0001/js-virtualizer>
