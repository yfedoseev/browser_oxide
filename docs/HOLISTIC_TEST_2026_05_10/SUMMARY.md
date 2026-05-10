# Holistic sweep — 2026-05-10

Run with: `cargo test --release -p browser --test holistic_sweep holistic_sweep_parallel -- --ignored --test-threads=1 --nocapture`

Engine state: HEAD = `22d379d` (post CSS calc fix + MediaRecorder stub +
Function.toString mask sweep + WebGL static _g + attachShadow _wrap fix).

**Result: 106/126 (84%) L3-RENDERED.** Wall clock 48.2 min.

## Outcome breakdown
| Outcome           | Count |
|-------------------|------:|
| L3-RENDERED       | 106   |
| THIN-BODY         |   6   |
| DataDome-CHL      |   4   |
| Kasada-CHL        |   3   |
| captcha-CHL       |   3   |
| ERROR:            |   2   |
| Cloudflare-CHL    |   1   |
| Akamai-CHL        |   1   |

## Sites that didn't pass (20)

### Engine-side issues to investigate
| Category   | Site         | Outcome         |
|------------|--------------|-----------------|
| antibot    | iphey        | ERROR:          |
| ru         | wildberries  | ERROR:          |
| ru         | yandex-ru    | THIN-BODY       |
| social     | twitter      | THIN-BODY       |
| social     | x-com        | THIN-BODY       |
| stores     | h-m          | THIN-BODY       |
| streaming  | hulu         | THIN-BODY       |
| misc       | khanacademy  | THIN-BODY       |

### Antibot-engine challenge pages (the actual stealth-fight surface)
| Category   | Site         | Outcome         |
|------------|--------------|-----------------|
| chl-known  | canadagoose  | Kasada-CHL      |
| chl-known  | hyatt        | Kasada-CHL      |
| realestate | realtor      | Kasada-CHL      |
| chl-known  | leboncoin    | DataDome-CHL    |
| misc       | yelp         | DataDome-CHL    |
| news       | wsj          | DataDome-CHL    |
| stores     | etsy         | DataDome-CHL    |
| chl-known  | douyin       | captcha-CHL     |
| search     | yandex       | captcha-CHL     |
| streaming  | spotify      | captcha-CHL     |
| misc       | udemy        | Cloudflare-CHL  |
| stores     | homedepot    | Akamai-CHL      |

## Notable passes
- **All 8 amazon properties** rendered.
- **All antibot diagnostic pages** (creepjs, pixelscan, sannysoft, browserleaks-canvas, fingerprintscan, amiunique, areyouheadless, nowsecure, botd) — 9/10 (only iphey errored).
- **adidas** — Akamai-protected, was previously a hard target. Now passing.
- **All 6 gov-bank** sites (bofa, chase, irs, paypal, usa-gov, wellsfargo).

## Comparison vs prior baseline
HANDOFF_2026_05_09 claimed Wildberries / Nike / Adidas / Hyatt OPEN.
This sweep shows: adidas ✓ (matches), nike absent from list (would
need re-check), hyatt = Kasada-CHL ✗ (regression vs claim), wildberries
= ERROR (regression vs claim). Some claims didn't survive a fresh test.

## Next steps (informed by Kasada-decryption findings, this session)
The Kasada-CHL set (canadagoose, hyatt, realtor) and the Akamai-CHL
(homedepot) all share the same root-cause inventory documented in
`docs/CANADA_GOOSE_DIAGNOSIS_2026_05_10_PART2.md`:
1. The `unjzomuybtbyyhwwkdpkxomylnab` undefined-receiver probes
   (5 fields share one underlying cause).
2. Error-message-text parity for structuredClone, class-extends, etc.

Fix those and re-sweep to validate the hypothesis that Kasada+Akamai
sites share the same engine-leak set.
