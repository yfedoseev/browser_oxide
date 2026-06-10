# Memory footprint

BrowserOxide renders pages in **tens of MB**. A headless Chrome instance
rendering the same page runs a multi-process tree that climbs into the
**gigabytes**. On the same box, same pages, BrowserOxide is **roughly an order
of magnitude (~15×) lighter**.

This is not a tuning trick — it falls out of the architecture. BrowserOxide is
a **single Rust process** with its own HTML/CSS/DOM/JS engine, V8 for scripting,
and a from-scratch BoringSSL network stack. There is **no Chromium, no Chrome
DevTools Protocol driver, and no renderer/GPU/utility subprocess fan-out**. A
headless Chrome "instance" is really a browser process plus one renderer process
per site (often more), a GPU process, and several utility processes — each with
its own heap, V8 isolate, and graphics buffers.

## Measured: BrowserOxide vs headless Chrome

Peak resident set size (RSS), **5-run median**, same machine, warm binary cache.
BrowserOxide is measured as single-process peak (`VmHWM`); Chrome is measured as
the **summed RSS of its entire process tree** (browser + renderers + GPU +
utility), which is the fair "total memory to render one page" number.

| Page | BrowserOxide | headless Chrome 147 | Advantage |
|---|--:|--:|--:|
| `example.com` (528 B) | **49 MB** | 1,028 MB | **21.0× lighter** |
| `en.wikipedia.org` (~230 KB) | **63 MB** | 1,111 MB | **17.6× lighter** |
| `x.com` (heavy SPA) | **87 MB** | 1,302 MB | **15.0× lighter** |
| `nytimes.com` | **133 MB** | 1,743 MB | **13.1× lighter** |

BrowserOxide's RSS is strikingly stable run-to-run (e.g. wikipedia across 5 runs:
62–63 MB); Chrome varies more (renderer/GC timing), so the median is the honest
central number.

Across the full 126-site anti-bot corpus, BrowserOxide's **median per-page RSS
is ~79 MB** (p90 ~126 MB; the tail is heavy SPAs and multi-MB DOMs).

## Why it matters: fleet density

Memory is usually the binding constraint when you scale headless browsing, not
CPU. The footprint difference translates directly into how many concurrent pages
fit on a host:

- One headless Chrome page: **~1–2 GB**.
- One BrowserOxide page: **~60–135 MB**.

On a 16 GB worker that's a handful of Chrome tabs versus **dozens of
BrowserOxide pages**. With the warm `PagePool`, isolates are reused across
navigations so steady-state stays in the same band rather than growing per page.

## Methodology

So anyone can reproduce the numbers:

**BrowserOxide** — single-process peak RSS via `/proc/<pid>/status` `VmHWM`:

```bash
cargo build --release -p browser_oxide --example thin_probe
target/release/examples/thin_probe https://en.wikipedia.org/wiki/Web_scraping &
pid=$!; peak=0
while kill -0 $pid 2>/dev/null; do
  hwm=$(awk '/VmHWM/{print $2}' /proc/$pid/status 2>/dev/null)
  [ -n "$hwm" ] && [ "$hwm" -gt "$peak" ] && peak=$hwm
  sleep 0.05
done
echo "BrowserOxide peak RSS: $((peak/1024)) MB"
```

**Headless Chrome** — summed RSS across the whole process tree (the launch uses
a unique `--user-data-dir` token so every child process can be matched):

```bash
tag="ch_$$"; udd="/tmp/$tag"
google-chrome --headless=new --disable-gpu --no-sandbox --no-first-run \
  --user-data-dir="$udd" --screenshot="/tmp/$tag.png" \
  https://en.wikipedia.org/wiki/Web_scraping &
peak=0
while pgrep -f "$tag" >/dev/null; do
  sum=0
  for pid in $(pgrep -f "$tag"); do
    r=$(awk '/VmRSS/{print $2}' /proc/$pid/status 2>/dev/null); [ -n "$r" ] && sum=$((sum+r))
  done
  [ "$sum" -gt "$peak" ] && peak=$sum
  sleep 0.05
done
echo "Chrome full-tree peak RSS: $((peak/1024)) MB"; rm -rf "$udd" "/tmp/$tag.png"
```

## Caveats

- **Point-in-time, one box.** Absolute numbers depend on the host, page weight,
  and Chrome version; re-measure on your own hardware. The *ratio* (~15×) is the
  durable claim and is stable across the pages tested.
- **Fair comparison = full Chrome tree.** Measuring only Chrome's parent process
  undercounts it badly (it misses the renderers, where most page memory lives).
  The numbers here sum the entire tree.
- **Functionality differs.** Chrome paints to a real compositor/GPU; BrowserOxide
  renders to an in-process Canvas 2D / WebGL surface and is built for headless
  scraping and agents, not interactive display. The memory comparison is for the
  "load and extract a page" workload these tools share.

See also: [BENCHMARK.md](BENCHMARK.md) for anti-bot pass-rate, and the README
[Per-page performance](../README.md#per-page-performance) section for wall-clock.
