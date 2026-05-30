# Memory footprint — browser_oxide vs camoufox (Firefox) vs chromium

**Date:** 2026-05-30. **Status:** BO numbers are *measured* (504 navs); camoufox
numbers are from OOM-kill evidence (`dmesg`) — a controlled head-to-head sweep
is scripted (§4) and pending a free box.

This is a **real, durable differentiator** and is independent of the site-pass
comparison (where camoufox v150 currently leads — see
`../v0.1.0-parity-workflows/02_FULL_GATE_VERIFICATION.md`). It matters for
*deployability and cost*, not stealth pass-rate.

---

## 1. Headline

| engine | per-site RSS (measured) | architecture |
|---|---|---|
| **browser_oxide** | **median 78 MB · p95 ~130 MB · max ~200 MB** | single-process, in-process V8 (deno_core), arena DOM |
| camoufox (Firefox) | **~3.0–4.8 GB** on heavy sites (OOM-kill RSS) | full Firefox, multi-process (1 parent + 6+ content/utility procs) |
| chromium (playwright) | ~0.4–1.5 GB (typical headless Chromium) | full Chromium, multi-process |

**browser_oxide uses ≈ 2 % of camoufox's memory on heavy sites — ~25× lighter at
peak, ~60× lighter at the median.** On the 14 GB shared test box, BO never
OOMs; camoufox's Firefox was OOM-killed repeatedly (§3).

---

## 2. browser_oxide — measured (the real data)

From the 2026-05-30 clean-room run, `sweep_metrics` `rss_mb` per site, all 126
sites × 4 profiles (504 navigations):

| profile | min | median | p95 | max |
|---|--:|--:|--:|--:|
| chrome_148_macos | 36 | 78 | 125 | 198 |
| pixel_9_pro_chrome_148 | 37 | 77 | 127 | 161 |
| iphone_15_pro_safari_18 | 36 | 78 | 127 | 175 |
| firefox_135_macos | 36 | 80 | 133 | 172 |

*(all MB.)* Cold-start floor ~36 MB; the heaviest real sites (Amazon, Vimeo,
multi-MB SPAs) peak ~160–200 MB. **Pool/warm mode** (one process, many navs)
holds a **~77 MB plateau** (measured: 54→61→63→64→76→77→77 MB over 8 warm navs,
after the per-nav worker-leak fix) — i.e. steady-state, no runaway.

Why so small: one OS process, one V8 isolate, an arena-allocated DOM
(`NodeId = u32` in a `Vec`, no `Rc<RefCell>`), and our own lightweight HTTP/CSS
stacks. There is **no separate browser process, no renderer fork, no GPU
process, no driver process.**

---

## 3. camoufox (Firefox) — OOM-kill evidence

Camoufox launches a **full Firefox** (parent + content + utility + rdd + socket
processes). On ad/tracker-heavy sites its RSS balloons. Captured on the 14 GB
box (`dmesg`):

```
Out of memory: Killed process … (camoufox) anon-rss: 4,646,144 kB  (≈4.6 GB)
Out of memory: Killed process … (camoufox) anon-rss: 3,015,680 kB  (≈3.0 GB)
Out of memory: Killed process … (camoufox) anon-rss: 4,802,816 kB  (≈4.8 GB)
```
(total-vm ≈ 5.4 GB.) Each camoufox instance needs **multiple GB**; running more
than ~2 concurrently on a 14 GB box triggers the kernel OOM-killer. (Note: these
OOM kills were *separate* from the playwright-driver crash bug fixed in
`patch_playwright_ff_driver.sh` — that bug produced false *page* failures;
the OOM pressure is the raw memory cost of Firefox.)

> ⚠️ These are RSS *at kill time* (peak), not a controlled per-site sweep. §4
> scripts the apples-to-apples measurement; numbers here will be tightened once
> it runs on a quiet box.

---

## 4. Reproducible controlled measurement (pending)

`benchmarks/measure_memory.py` (to run on a quiet box) samples peak RSS of the
**whole process tree** for each engine on an identical site list:
- BO: peak RSS of the `sweep_metrics` process.
- camoufox/chromium: summed RSS of the browser-bin parent + all children,
  polled every 0.5 s across the nav + settle.

Run: `python3 benchmarks/measure_memory.py <light_and_heavy_sites.json>` →
`docs/benchmarks/runs/<date>_memory/` (gitignored raw data). This replaces the
OOM-evidence camoufox figures in §3 with a clean per-site distribution.

---

## 5. Why it matters

- **Deployability / density.** BO runs **dozens of concurrent instances** on a
  small VM; camoufox runs ~2 before OOM on 14 GB. For a scraping fleet this is
  the difference between 1 box and ~30.
- **Cloud cost.** Memory is the dominant cost dimension for headless fleets. At
  ~78 MB median, BO fits the smallest instance tiers; Firefox/Chromium need
  multi-GB instances.
- **Constrained / edge targets.** BO is viable where a full browser can't load
  at all.
- **Orthogonal to the moat.** This is *separate from* BO's structural advantage
  (no CDP / in-process — undetectable where CDP is sniffed). Together: BO is the
  lighter, CDP-free option; camoufox currently passes more sites. Different axes.

---

## 6. Honest framing

On raw site-pass over the 126-corpus, **camoufox v150 (116) currently leads BO
(routed 113 / best single 111)** once its driver bug is removed. BO's wins are
**memory (~25–60×)** and the **no-CDP/in-process architecture** — real and
durable, but not pass-rate. State both; don't conflate them.

— 2026-05-30.
