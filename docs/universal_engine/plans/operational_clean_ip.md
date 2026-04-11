# Operational: get a clean-IP Chrome reference

**Task**: #72
**Priority**: P0 (the single biggest unblocker for adidas investigation)
**Effort**: 30 minutes of execution after ~1-2 hours of operational setup
**Dependencies**: access to any IP that isn't currently on Akamai's graylist

## Goal

Capture a real Chrome's `sensor_data` POST body for adidas.com (or
homedepot.com) using Playwright, then diff it section-by-section against
our POST body to identify the specific field(s) that Akamai flags. Without
this reference, every adidas investigation is guesswork.

## Why this is the single biggest unblocker

Per `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md`, we've
ruled out:

- Workers (verified by probe; VM doesn't touch them)
- Canvas pixel extraction (verified by probe; VM doesn't extract)
- Audio sum (calibrated to 60 ppm from Chrome reference)
- Navigator class prototypes (matched Chrome)
- Behavioral event counts (humanize script makes them plausible)
- TLS fingerprint (our rquest gets the interstitial, not the WAF block)
- Basic POST format (server returns 201 Created, accepting wire format)

The remaining hypotheses are:

1. Audio per-sample bit-accuracy (our calibrated sine vs Blink wavetable)
2. Canvas paint state via an unusual extraction path we haven't found
3. Font metrics (measureText)
4. `performance.memory` or other navigator value drift
5. `akid` round-trip hash mismatch
6. Event timing jitter in the per-event substream

A byte-level diff against a known-good Chrome POST body identifies the
specific field in 30 minutes. Without the reference, each hypothesis takes
4-8 hours to investigate individually, and we might still be wrong.

## The blocker

Our machine's egress IP is currently on Akamai's graylist for **headless
Playwright traffic**. Confirmed 2026-04-10: a fresh headless Chromium on
this IP returns the WAF hard-block page (`"Reference Error: 18.65951eb8.
1775861888.161f9a02"` / `"403 ERROR"`) instead of the sensor challenge.

Interestingly, the same IP via our `rquest` (Chrome 131 TLS impersonation)
still gets the sensor challenge — Akamai treats rquest as Chrome-shaped
and headless Playwright as suspect. That's a meaningful architectural win
for our TLS stack, but it doesn't help us capture the reference.

## Options for clean egress

### Option 1 — Cellular tether (cheapest, 15 min setup)

Connect a laptop to a phone's cellular hotspot. The phone's IP is a
cellular carrier IP that Akamai has never seen from headless Playwright.

**Pros**: free if you already have an unlimited data plan, no new
services to sign up for.
**Cons**: cellular is slow (~20-50 Mbps, fine for a single capture),
and the IP is shared with other cellular customers so it may also be
graylisted for high-abuse traffic.

### Option 2 — Commercial residential proxy ($10-100/month)

Services: Bright Data, Smartproxy, Oxylabs, IPRoyal, Soax, SmartProxy.
They rent IPs from real residential ISPs.

**Pros**: reliable, many IPs to try, explicit "clean" marketing.
**Cons**: costs money, signup friction, some services require KYC.

**Recommendation**: IPRoyal pay-as-you-go is the cheapest entry (~$4
per GB, plenty for a single capture).

### Option 3 — VPN ($5-15/month)

Services: Mullvad, ProtonVPN, NordVPN, ExpressVPN, Surfshark.

**Pros**: cheap, simple setup, recurring monthly.
**Cons**: most VPN IPs are already flagged by anti-bot engines. You'd
need to try a few exit nodes.

**Recommendation**: Mullvad. Pay with Monero or cash, get a random
European exit. Free trial available.

### Option 4 — Friend's network / co-working space / coffee shop

**Pros**: free.
**Cons**: you need to physically be there and your laptop needs to
be able to run the capture script.

### Option 5 — Hyper Solutions free trial ($0 / SaaS)

Hyper Solutions offers a free trial of their Akamai solver SDK. You
can use their API to GET a real Chrome POST body for any Akamai
protected site. This is the cheapest and fastest if they'll give you
API access.

**Signup**: https://hypersolutions.co — look for "Get started" or
"Trial".

**Pros**: no egress setup, they give you raw sensor_data payloads.
**Cons**: may not give you the EXACT same rotated VM we see (their
solver caches payloads), and you need to sign up.

### Option 6 — Cloud VM in a different region ($1-5)

Spin up a DigitalOcean / Hetzner / Linode / AWS droplet for an hour
in a region we haven't used. Install Playwright + Chromium, run the
capture script.

**Pros**: reproducible, scriptable, cheap (< $1 for a few hours).
**Cons**: cloud IPs are often flagged. Try residential-adjacent
providers first (Contabo, OVH sometimes work where AWS/DO fail).

## The capture script

Once you have clean egress, the actual capture is this Node.js script.
It's already written at `/tmp/capture_adidas_sensor.js` (from the
2026-04-10 session).

```javascript
// /tmp/capture_adidas_sensor.js
const fs = require('fs');
const path = require('path');
const { chromium } = require('/tmp/pw-capture/node_modules/playwright');

const OUT_DIR = '/tmp/chrome-sensor';
fs.mkdirSync(OUT_DIR, { recursive: true });

function isAdidasSensor(req) {
    if (req.method() !== 'POST') return false;
    const url = req.url();
    if (!url.includes('adidas.com')) return false;
    const ct = (req.headers()['content-type'] || '').toLowerCase();
    if (ct.startsWith('text/plain')) return true;
    try {
        const p = new URL(url).pathname;
        if (/^\/[A-Za-z0-9_-]{4,}\/[A-Za-z0-9_-]{4,}\/[A-Za-z0-9_-]{4,}\//.test(p)) {
            return true;
        }
    } catch {}
    return false;
}

(async () => {
    const browser = await chromium.launch({
        headless: true,
        args: ['--disable-blink-features=AutomationControlled'],
    });
    const ctx = await browser.newContext({
        viewport: { width: 1280, height: 900 },
        locale: 'en-US',
        timezoneId: 'America/Los_Angeles',
    });
    const page = await ctx.newPage();

    let n = 0;
    page.on('request', (req) => {
        if (!isAdidasSensor(req)) return;
        const i = ++n;
        const record = {
            index: i,
            method: req.method(),
            url: req.url(),
            headers: req.headers(),
            postData: req.postData(),
            postDataBase64: null,
        };
        try {
            const buf = req.postDataBuffer();
            if (buf) record.postDataBase64 = buf.toString('base64');
        } catch {}
        fs.writeFileSync(
            path.join(OUT_DIR, `${String(i).padStart(3, '0')}.json`),
            JSON.stringify(record, null, 2)
        );
        console.log(`[capture] #${i} ${req.url().substring(0, 80)} body=${(record.postData || '').length}`);
    });

    console.log('[nav] https://www.adidas.com/us');
    await page.goto('https://www.adidas.com/us', { waitUntil: 'commit', timeout: 60000 });
    await page.waitForTimeout(15000);
    const title = await page.title();
    console.log(`[title] ${title}`);
    fs.writeFileSync(path.join(OUT_DIR, 'final.html'), await page.content());
    console.log(`[done] ${n} sensor POSTs captured.`);
    await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
```

**Setup**:

```bash
mkdir -p /tmp/pw-capture
cd /tmp/pw-capture
npm init -y
npm install playwright
npx playwright install chromium
```

**Run** (from the clean egress):

```bash
node /tmp/capture_adidas_sensor.js
```

**Expected successful output**:

```
[nav] https://www.adidas.com/us
[capture] #1 https://www.adidas.com/9qizx734Mu_fe/... body=277
[capture] #2 https://www.adidas.com/9qizx734Mu_fe/... body=3762
[capture] #3 https://www.adidas.com/9qizx734Mu_fe/... body=4892
[title] Sneakers and Activewear | adidas US
[done] 3 sensor POSTs captured.
```

If the title is "adidas" (not "Sneakers and Activewear | adidas US"),
the page loaded the WAF-block version and you need a cleaner IP.

## The diff

Once you have `/tmp/chrome-sensor/001.json`, `002.json`, etc., run a
synchronized capture from browser_oxide at the same time:

```bash
cd /home/yfedoseev/projects/browser_oxide
BOXIDE_DUMP_POST_DIR=/tmp/oxide-sensor-reference \
    cargo test -p browser --test adidas_sensor_capture -- \
    --ignored --test-threads=1 --nocapture
```

**Important**: run both captures within ~30 seconds of each other
because Akamai rotates the sensor VM URL per request. If the
timestamps are far apart, the two captures may be against different
VM variants and the diff is meaningless.

**The diff script**:

```python
#!/usr/bin/env python3
# /tmp/diff_adidas_sensors.py
import json
import base64

# Load Chrome reference POST #2 (the full sensor, not the ping).
with open('/tmp/chrome-sensor/002.json') as f:
    chrome_data = json.loads(f.read())
chrome_body = chrome_data.get('postData') or \
    base64.b64decode(chrome_data['postDataBase64']).decode('utf-8')
chrome_sd = json.loads(chrome_body)['sensor_data']
chrome_parts = chrome_sd.split(';')

# Load our POST #2.
with open('/tmp/oxide-sensor-reference/002.body') as f:
    our_body = f.read()
our_sd = json.loads(our_body)['sensor_data']
our_parts = our_sd.split(';')

print(f"Chrome: {len(chrome_sd)} chars, {len(chrome_parts)} sections")
print(f"Ours:   {len(our_sd)} chars, {len(our_parts)} sections")
print()

# Diff section-by-section
for i in range(max(len(chrome_parts), len(our_parts))):
    c = chrome_parts[i] if i < len(chrome_parts) else '(missing)'
    o = our_parts[i] if i < len(our_parts) else '(missing)'
    status = '=' if c == o else 'X'
    print(f"[{i:3}] {status} chrome({len(c)}) our({len(o)})")
    if c != o and (len(c) < 100 and len(o) < 100):
        print(f"       chrome: {c[:80]!r}")
        print(f"       our:    {o[:80]!r}")
```

Run it:

```bash
python3 /tmp/diff_adidas_sensors.py
```

**Expected output** if the diff is diagnostic:

```
Chrome: 6234 chars, 48 sections
Ours:   3762 chars, 30 sections

[  0] = chrome(1) our(1)
[  1] = chrome(1) our(1)
[  2] = chrome(1) our(1)
[  3] X chrome(1) our(1)
       chrome: '2'
       our:    '0'
[  4] X chrome(7) our(7)
       chrome: '4823951'
       our:    '3485761'
[  5] X chrome(44) our(44)
       chrome: 'jThJkIT...'
       our:    'wh+qncZ...'  (akid tokens differ per session)
[  6] X chrome(35) our(16)
       chrome: '41,3,0,0,2,1823'
       our:    '30,39,0,0,5,2075'
[  7] ...
```

The important sections:

- **Section 6**: event counts. Ours `30,39,0,0,5,2075`, Chrome's
  `41,3,0,0,2,1823`. That would tell us Chrome has ~10x the mousemoves
  vs mduce ratio (correct human ratio) while we have ~1:1 (implausible).
  The `1823` vs `2075` elapsed ms tells us about timing — Chrome posted
  earlier.
- **Section 4**: the counter that differs between runs. If consistently
  different, our VM is taking a different code path.
- **Sections after 6**: encrypted. A byte-length mismatch tells us our
  sensor is omitting or truncating specific fields even if we can't
  read them.

## What you'll likely find

Based on the existing evidence, the highest-likelihood diffs are:

1. **Section 6 event counts**: Chrome has realistic mouse/scroll event
   counts. Ours has very few and with bad ratios even after humanize.
2. **Payload size**: Chrome is 5-10 KB, ours is 3-4 KB. Some section
   is shorter or missing.
3. **Post count**: Chrome typically posts 3-5 sensor POSTs per page
   load. If we only post 1-2, an entire sub-fingerprint section is
   never assembled.
4. **Section 4 or 5** (version / counter / akid): if the VM is
   computing a different internal state from the start, everything
   downstream differs.

## After you find the diff

1. Update `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.
   md` with the specific section that differs.
2. Map the section to a fingerprint capability (canvas? audio?
   navigator? timing?).
3. Pick the capability that owns the diff and work on it (T1.2 fonts,
   T1.1 canvas, behavioral event injection, etc.).
4. Re-capture and verify the diff closes.

## Privacy / ethics note

Capturing adidas's sensor_data from a real Chrome is **not** a
breach of anti-bot systems — it's literally what your browser sends
when you visit the page. The `sensor_data` POST body is already
public in the sense that any browser tab can see it via DevTools.

Using this to understand how browser_oxide's output differs is
research, not attack. Don't replay captured payloads from one
session to another (that IS a breach and also doesn't work because
tokens are bound to session state).

## Related

- `docs/universal_engine/site_debugging/adidas_akamai_bmp_v3.md` —
  everything we've tried.
- Task #61 (done) — proved cookies are not portable, so replay doesn't
  help.
- Task #67 (done) — the session-level diff investigation, cut short
  by the IP graylist.
- Task #72 (this task) — pending, the unblocker.
