# 04 — R-WBAAS-WILDBERRIES: custom in-house antibot + geo-gated

**Status:** ⏸️ likely out of scope. Captured + classified; no clean fix path identified.
**Sites in scope:** wildberries (1).
**Effort:** unknown; mostly research.
**Scope:** likely `vendor_solvers` if pursued at all; arguably out-of-scope per [`../../SCOPE.md`](../../SCOPE.md).

## TL;DR

`wildberries.ru` is the Russian Amazon-equivalent. From a US/CA
datacenter IP it returns HTTP 498 (non-standard "Network
authentication required") with a 1447-byte body that loads
`/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js` — a 70 KB
obfuscated ES module. The bundle's plaintext surface is minimal
(`toString` 11×, `userAgent` 2×, `fetch` 1×) so most of the work is in
the obfuscated chunks. v150 + Patchright + BO all fail; likely geo +
custom antibot.

## Why this matters

Not much for v0.2.0 / v0.3.0 timeframe:

- Single site, geo-bound (Russian IP probably required).
- Custom in-house antibot — not a vendor cluster we'd benefit from
  understanding for other sites.
- Even v150 + Patchright (Chromium) fail — no Chromium-class engine
  passes today, so the bar is high.
- No engine-side fingerprint surface improvements have moved the
  needle (FIX-A/C/D/F/J all shipped this session; none would help
  here).

The reasons to document anyway: stop treating it as a "mystery failure
worth investigating" and either accept it as out-of-scope or scope it
to `vendor_solvers`.

## Current state

Captured this session (audit/16 §R-WBAAS-WILDBERRIES):

```
HTTP/2 498
content-type: text/html; charset=UTF-8
title: "Почти готово..." ("Almost ready")
body: 1447 bytes minimal HTML
script: <script type="module" src="/__wbaas/challenges/antibot/__static/v1/index-DQJ0L4Mq.js" data-site-key="7400bd5df8b843b28254659f10915f31">
```

Bundle download (`/tmp/wbaas_probe/wb_challenge.js`):
- 70 KB obfuscated ES module
- Custom encoder (no AWS WAF / DataDome / Kasada / Akamai branding)
- Plaintext markers: `toString` (11×), `navigator.userAgent` (2×), `fetch(` (1×)
- Site-key `7400bd5df8b843b28254659f10915f31` is wildberries-specific

## Next steps

### Option A — Accept as out-of-scope (RECOMMENDED)

Mark `wildberries` in the corpus as a `diagnostic: true` site (the
same mechanism FIX-CORPUS-DIAGNOSTIC-FLAG added for `areyouheadless`).
Wildberries' antibot is custom + geo-bound + single-site; counting it
as a "real-browsable" failure drags the production pass-rate without
representing a fixable gap.

Touch points:
- `benchmarks/build_corpus_json.py` — add `wildberries` to
  `DIAGNOSTIC_SITES`.
- `docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` — add a note
  reclassifying.

### Option B — Verify the geo-gate (~1 hour)

If we have access to a Russian-IP proxy (or a Russian VPS), retry
wildberries from there. If it serves a real page, the engine-addressable
component is zero (just IP routing). If it ALSO serves HTTP 498 from
a Russian IP, then the antibot fires on every visit and the
obfuscated bundle is the gate.

### Option C — Reverse-engineer the bundle (~1-2 weeks, `vendor_solvers`)

The 70 KB bundle is much smaller than AWS WAF's 1.37 MB — tractable
for static analysis with patience. The output would be a custom
site-specific solver living in `vendor_solvers` per CLAUDE.md.
Decision: **don't pursue unless wildberries becomes a customer-
mandatory site**.

## Dependencies

For Option A: none.
For Option B: a Russian-IP proxy.
For Option C: AST-walking tooling (e.g., `joern`, `astxplorer`) +
willing investigator.

## Sources / references

- Captured: `/tmp/wbaas_probe/wb.html`, `/tmp/wbaas_probe/wb_challenge.js`
- audit `16_DECISION_LOG.md` §R-WBAAS-WILDBERRIES — this session's classification
- `CLAUDE.md` / `SCOPE.md` — per-vendor solving boundary
