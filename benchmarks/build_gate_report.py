#!/usr/bin/env python3
"""Build the detailed verification report from a full-gate run.

Reads /tmp/full_gate_2026_05_28/*.json (BO 4 profiles + pool + competitors
incl camoufox v150 & v135), computes per-engine production pass rates and
BO routed-best-of-4, and emits a detailed markdown report.

Usage: build_gate_report.py [GATE_DIR] [OUT_MD]
"""
import json
import sys
from pathlib import Path

GATE = Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/full_gate_2026_05_28")
OUT = Path(sys.argv[2] if len(sys.argv) > 2 else
           "docs/v0.1.0-parity-workflows/02_FULL_GATE_VERIFICATION.md")

DIAGNOSTIC = {"areyouheadless"}


def is_pass(r):
    return r.get("tag") == "L3-RENDERED" and r.get("len", 0) >= 15000


def load(name):
    p = GATE / f"{name}.json"
    if not p.exists():
        return None
    try:
        d = json.loads(p.read_text())
    except Exception:
        return None
    # name -> result row
    return {r["name"]: r for r in d.get("results", [])}


# Engine files to look for.
BO_PROFILES = [
    ("bo_chrome_148_macos_cold", "BO chrome"),
    ("bo_pixel_9_pro_chrome_148_cold", "BO pixel"),
    ("bo_iphone_15_pro_safari_18_cold", "BO iphone"),
    ("bo_firefox_135_macos_cold", "BO firefox"),
]
COMPETITORS = [
    ("comp_playwright", "Playwright"),
    ("comp_playwright_stealth", "PW-stealth"),
    ("comp_patchright", "Patchright"),
    ("comp_camoufox_v150", "Camoufox v150"),
    ("comp_camoufox_v135", "Camoufox v135"),
]

engines = {}
for fn, label in BO_PROFILES + [("bo_chrome_148_macos_pool", "BO pool")] + COMPETITORS:
    m = load(fn)
    if m is not None:
        engines[label] = m

# Universe of site names (union across engines).
all_names = set()
for m in engines.values():
    all_names.update(m.keys())
prod_names = sorted(n for n in all_names if n not in DIAGNOSTIC)

# BO routed-best-of-4: pass if any BO profile passes.
bo_labels = [lbl for _, lbl in BO_PROFILES if lbl in engines]


def routed_pass(name):
    return any(
        name in engines[lbl] and is_pass(engines[lbl][name]) for lbl in bo_labels
    )


def eng_pass(label, name):
    m = engines.get(label)
    return bool(m and name in m and is_pass(m[name]))


def count(label):
    m = engines.get(label)
    if not m:
        return None
    return sum(1 for n in prod_names if n in m and is_pass(m[n]))


routed_count = sum(1 for n in prod_names if routed_pass(n))
denom = len(prod_names)

lines = []
lines.append("# 02 — FULL GATE VERIFICATION")
lines.append("")
lines.append(f"> Source: `{GATE}` · production denominator = **{denom}** "
             f"(126 corpus − {len(DIAGNOSTIC)} diagnostic).")
lines.append("> Pass = `L3-RENDERED` AND body ≥ 15 000 B. Corpus vendor-spaced.")
lines.append("")
lines.append("## Scorecard (production pass / %)")
lines.append("")
lines.append("| Engine | Pass | % |")
lines.append("|---|--:|--:|")
lines.append(f"| **browser_oxide (routed best-of-4)** | **{routed_count}/{denom}** | "
             f"**{100*routed_count/denom:.1f}%** |")
for _, lbl in BO_PROFILES:
    c = count(lbl)
    if c is not None:
        lines.append(f"| · {lbl} | {c}/{denom} | {100*c/denom:.1f}% |")
pc = count("BO pool")
if pc is not None:
    lines.append(f"| · BO pool (chrome) | {pc}/{denom} | {100*pc/denom:.1f}% |")
for _, lbl in COMPETITORS:
    c = count(lbl)
    if c is not None:
        lines.append(f"| {lbl} | {c}/{denom} | {100*c/denom:.1f}% |")
    else:
        lines.append(f"| {lbl} | _NODATA_ | — |")
lines.append("")

# Head-to-head vs each competitor.
lines.append("## browser_oxide (routed) vs each competitor")
lines.append("")
lines.append("| Competitor | BO-only wins | Competitor-only wins | Both | BO routed Δ |")
lines.append("|---|--:|--:|--:|--:|")
for _, lbl in COMPETITORS:
    m = engines.get(lbl)
    if not m:
        lines.append(f"| {lbl} | _NODATA_ | | | |")
        continue
    bo_only = comp_only = both = 0
    for n in prod_names:
        b = routed_pass(n)
        c = eng_pass(lbl, n)
        if b and c:
            both += 1
        elif b and not c:
            bo_only += 1
        elif c and not b:
            comp_only += 1
    delta = (routed_count) - (count(lbl) or 0)
    lines.append(f"| {lbl} | {bo_only} | {comp_only} | {both} | {delta:+d} |")
lines.append("")

# Contested sites: any site not passed by every engine present.
lines.append("## Contested-site matrix (sites some engine fails)")
lines.append("")
hdr = ["site", "BO routed"] + [lbl for _, lbl in BO_PROFILES] + [lbl for _, lbl in COMPETITORS]
lines.append("| " + " | ".join(hdr) + " |")
lines.append("|" + "|".join(["---"] * len(hdr)) + "|")
present_labels = [lbl for _, lbl in BO_PROFILES + COMPETITORS if lbl in engines]
for n in prod_names:
    verdicts = {lbl: eng_pass(lbl, n) for lbl in present_labels}
    rb = routed_pass(n)
    # contested if not everyone passes
    if rb and all(verdicts.get(lbl, False) for lbl in present_labels):
        continue
    row = [n, "✅" if rb else "❌"]
    for _, lbl in BO_PROFILES:
        row.append("✅" if eng_pass(lbl, n) else ("·" if lbl in engines else " "))
    for _, lbl in COMPETITORS:
        row.append("✅" if eng_pass(lbl, n) else ("·" if lbl in engines else "?"))
    lines.append("| " + " | ".join(row) + " |")
lines.append("")
lines.append("> ✅ pass · `·` fail/thin · `?` engine NODATA")
lines.append("")
lines.append("## Caveats")
lines.append("")
lines.append("- **AWS-WAF spacing:** the gate runs the corpus serially; even "
             "vendor-spaced, AWS sites can token-cluster on one IP. The "
             "authoritative AWS measurement is `benchmarks/run_spaced_aws.sh` "
             "(9/9 PASS, 150 s gaps). Treat any AWS fail here as a possible "
             "clustering artifact, not an engine gap.")
lines.append("- Camoufox v135/v150 run from the same cache dir via swap; the "
             "active version at report time is v150.")

OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text("\n".join(lines) + "\n")
print(f"wrote {OUT}")
print(f"\nBO routed: {routed_count}/{denom} ({100*routed_count/denom:.1f}%)")
for _, lbl in COMPETITORS:
    c = count(lbl)
    print(f"  {lbl}: {c}/{denom}" if c is not None else f"  {lbl}: NODATA")
