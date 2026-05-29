#!/usr/bin/env python3
"""Delta head-to-head: browser_oxide (4 profiles) vs Camoufox v150, SAME IP.

Why this exists
---------------
The full 126-site gate (`run_fix12_gate.sh`) spends ~99% of its 10h
wall-clock re-confirming sites whose verdict we already know. The entire
contested surface vs Camoufox v150 is exactly 12 sites (FAILED_SITES_ANALYSIS.md:
11 Stratum-A + 1 Stratum-B homedepot). This harness runs ONLY those 12,
back-to-back BO-vs-v150 from the *same datacenter IP in the same session*,
N trials, so a per-site difference isolates ENGINE quality from IP reputation
and AWS-WAF probabilistic token-rolling (the dominant confound — see
02_GAP_ANALYSIS.md §5-8). It runs in minutes, not 10 hours, so we can take
many trials and beat the noise.

Pass rule is the shared Rust classifier (`classify_stdin`), identical to the
gate: PASS == tag=='L3-RENDERED' AND len>=15000. Apples-to-apples by
construction — both engines' HTML goes through the same binary.

Triage the output table:
  * v150 ALSO fails from our IP  -> IP / probabilistic, NOT an engine gap.
    Stop spending engine effort on it.
  * v150 passes where BO fails (same IP, same window) -> real engine delta,
    worth a targeted fix.
  * BO passes where v150 fails (e.g. homedepot) -> a site where WE beat v150.

Usage
-----
    /tmp/bo-venv/bin/python benchmarks/run_delta_headtohead.py [N_TRIALS] [OUT.json]

Env knobs:
    BO_PROFILES   space-separated profile list (default: the 4 gate profiles)
    INCLUDE_CAMOUFOX=0   skip the Camoufox arm (BO-only baseline)
    NAV_TIMEOUT_MS / SETTLE_MS  passed through to the Camoufox harness
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BO_BIN = REPO / "target" / "release" / "examples" / "sweep_metrics"
COMP_HARNESS = REPO / "benchmarks" / "bench_corpus_v2.py"
BUILD_CORPUS = REPO / "benchmarks" / "build_corpus_json.py"
DELTA_CORPUS = Path("/tmp/delta_corpus.json")

# The 12 contested sites. 11 Stratum-A (Camoufox v150 passes, BO doesn't) +
# 1 Stratum-B (homedepot: only Chromium/Patchright passes — even v150 fails;
# our sec-cpt fix flipped it, so it's a site where we can BEAT v150).
DELTA_NAMES = [
    "amazon-ca", "amazon-com", "amazon-com-au", "amazon-fr",
    "amazon-in", "amazon-jp", "imdb",          # AWS WAF cluster (7)
    "booking", "douyin",                        # SPA hydration (2)
    "duolingo",                                 # reCAPTCHA Worker (1)
    "x-com",                                    # TLS / SharedSession (1)
    "homedepot",                                # Stratum-B (BO can beat v150)
]

BO_PROFILES = os.environ.get(
    "BO_PROFILES",
    "chrome_148_macos pixel_9_pro_chrome_148 iphone_15_pro_safari_18 firefox_135_macos",
).split()

PY = sys.executable  # run the comp harness with the same interpreter (this venv)


def is_pass(tag, length):
    """The shared gate rule — same as bench_corpus_v2.aggregate()."""
    return tag == "L3-RENDERED" and length >= 15000


def build_delta_corpus():
    """Filter the canonical 126-corpus down to the 12 delta sites, preserving
    each site's exact url + cat so the classifier/vendor map stays consistent."""
    full = json.loads(subprocess.check_output([sys.executable, str(BUILD_CORPUS)]))
    by = {s["name"]: s for s in full}
    missing = [n for n in DELTA_NAMES if n not in by]
    if missing:
        sys.exit(f"delta sites missing from canonical corpus: {missing}")
    delta = [by[n] for n in DELTA_NAMES]
    DELTA_CORPUS.write_text(json.dumps(delta, indent=1))
    return delta


def run_bo(profile, trial):
    """One BO sweep over the delta corpus -> {name: (tag, len)}."""
    out = Path(f"/tmp/delta_bo_{profile}_t{trial}.json")
    log = Path(f"/tmp/delta_bo_{profile}_t{trial}.log")
    t0 = time.perf_counter()
    with open(log, "w") as lf:
        subprocess.run([str(BO_BIN), profile, str(DELTA_CORPUS), str(out)],
                       stdout=lf, stderr=subprocess.STDOUT, check=False)
    dur = time.perf_counter() - t0
    res = {}
    if out.exists():
        for r in json.load(open(out)).get("results", []):
            res[r["name"]] = (r["tag"], r["len"])
    return res, dur


def run_camoufox(trial, delta_sites):
    """Camoufox v150 over the delta sites, ONE FRESH BROWSER PER SITE.

    Per-site isolation is mandatory, not an optimization: a single hostile
    site (douyin) crashes the Firefox/Camoufox driver, and bench_corpus_v2
    only flushes its JSON at the very end — so a mid-sweep crash loses ALL
    results for the run. Per-site means a crash costs exactly one site, and
    a missing/empty file is recorded as NODATA (never as a silent fail).
    Returns {name: (tag, len)} where (tag,len) may be ('NODATA', 0)."""
    res = {}
    t0 = time.perf_counter()
    for site in delta_sites:
        name = site["name"]
        one = Path(f"/tmp/delta_cf_one_{name}.json")
        out = Path(f"/tmp/delta_camoufox_{name}_t{trial}.json")
        log = Path(f"/tmp/delta_camoufox_{name}_t{trial}.log")
        one.write_text(json.dumps([site], indent=1))
        env = dict(os.environ, CORPUS_FILE=str(one))
        with open(log, "w") as lf:
            subprocess.run([PY, str(COMP_HARNESS), "camoufox", str(out)],
                           stdout=lf, stderr=subprocess.STDOUT, check=False, env=env)
        recs = json.load(open(out)).get("results", []) if out.exists() else []
        res[name] = (recs[0]["tag"], recs[0]["len"]) if recs else ("NODATA", 0)
    return res, time.perf_counter() - t0


def main():
    n_trials = int(sys.argv[1]) if len(sys.argv) > 1 else 3
    out_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("/tmp/delta_headtohead.json")
    include_cf = os.environ.get("INCLUDE_CAMOUFOX", "1") != "0"

    if not BO_BIN.exists():
        sys.exit(f"BO binary missing: {BO_BIN} (cargo build --release --example sweep_metrics)")

    delta_sites = build_delta_corpus()
    print(f"=== DELTA HEAD-TO-HEAD: {len(DELTA_NAMES)} sites x {n_trials} trials ===")
    print(f"BO profiles: {', '.join(BO_PROFILES)}")
    print(f"Camoufox v150: {'ON' if include_cf else 'OFF'} (same IP, interleaved)\n")

    # per-site tallies across trials
    bo_pass = {n: 0 for n in DELTA_NAMES}        # BO routed best-of-profiles
    cf_pass = {n: 0 for n in DELTA_NAMES}
    cf_tested = {n: 0 for n in DELTA_NAMES}      # trials where v150 returned real data (not NODATA)
    # keep last-seen detail for the report (tag/len), per engine per site
    bo_detail = {n: [] for n in DELTA_NAMES}
    cf_detail = {n: [] for n in DELTA_NAMES}
    trials_log = []

    for t in range(1, n_trials + 1):
        print(f"--- trial {t}/{n_trials} ---", flush=True)
        trial_rec = {"trial": t, "bo": {}, "camoufox": {}}

        # BO arm: all profiles back-to-back. Routed = pass on ANY profile.
        bo_profile_res = {}
        for prof in BO_PROFILES:
            res, dur = run_bo(prof, t)
            bo_profile_res[prof] = res
            got = sum(1 for n in DELTA_NAMES if n in res and is_pass(*res[n]))
            print(f"  BO {prof:28} {got}/{len(DELTA_NAMES)} pass  ({dur:.0f}s)", flush=True)
            trial_rec["bo"][prof] = {n: list(res.get(n, ("MISSING", 0))) for n in DELTA_NAMES}

        for n in DELTA_NAMES:
            routed_pass = any(n in r and is_pass(*r[n]) for r in bo_profile_res.values())
            if routed_pass:
                bo_pass[n] += 1
            # best (largest) body across profiles for the detail column
            best = max(((r[n] for r in bo_profile_res.values() if n in r)),
                       key=lambda tl: tl[1], default=("MISSING", 0))
            bo_detail[n].append(best)

        # Camoufox arm — per-site fresh browser (crash isolation)
        if include_cf:
            res, dur = run_camoufox(t, delta_sites)
            got = sum(1 for n in DELTA_NAMES if n in res and is_pass(*res[n]))
            nodata = sum(1 for n in DELTA_NAMES if res.get(n, ("NODATA", 0))[0] == "NODATA")
            print(f"  Camoufox v150 {' ' * 18}{got}/{len(DELTA_NAMES)} pass"
                  f"{f'  ({nodata} NODATA)' if nodata else ''}  ({dur:.0f}s)", flush=True)
            trial_rec["camoufox"] = {n: list(res.get(n, ("NODATA", 0))) for n in DELTA_NAMES}
            for n in DELTA_NAMES:
                tag, ln = res.get(n, ("NODATA", 0))
                if tag != "NODATA":
                    cf_tested[n] += 1
                if is_pass(tag, ln):
                    cf_pass[n] += 1
                cf_detail[n].append((tag, ln))

        trials_log.append(trial_rec)
        print(flush=True)

    # ---- report ----
    print("=" * 84)
    print(f"{'site':16} {'BO routed':>10} {'v150':>10}   verdict")
    print("-" * 84)
    engine_gap, ip_noise, we_win, both_pass, nodata_sites = [], [], [], [], []
    for n in DELTA_NAMES:
        b, c, ct = bo_pass[n], cf_pass[n], cf_tested[n]
        v150_col = f"{c}/{ct}" if ct else "NODATA"
        if include_cf:
            if ct == 0:
                verdict = "v150 NODATA — crashed/unreached, re-run"; nodata_sites.append(n)
            elif b == 0 and c == 0:
                verdict = "IP/PROB — both fail from this IP (NOT engine)"; ip_noise.append(n)
            elif c > b:
                verdict = "ENGINE GAP — v150 more reliable than BO"; engine_gap.append(n)
            elif b > 0 and c == 0:
                verdict = "WE WIN — BO passes, v150 doesn't"; we_win.append(n)
            else:
                verdict = "comparable / both pass"; both_pass.append(n)
        else:
            verdict = ""
        print(f"{n:16} {f'{b}/{n_trials}':>10} {v150_col:>10}   {verdict}")
    print("=" * 84)

    if include_cf:
        print("\n(v150 column = passes/trials-with-real-data; NODATA = browser crashed or site unreached)")
        print(f"\nENGINE GAPS (v150 ahead — fix these): {engine_gap or '—'}")
        print(f"IP/PROBABILISTIC (don't chase):       {ip_noise or '—'}")
        print(f"WE BEAT v150:                         {we_win or '—'}")
        print(f"comparable / both pass:               {both_pass or '—'}")
        print(f"v150 NODATA (inconclusive, re-run):   {nodata_sites or '—'}")
        print(f"\n  BO routed total:   {sum(1 for n in DELTA_NAMES if bo_pass[n] > 0)}/{len(DELTA_NAMES)} sites passed in >=1 trial")
        print(f"  v150 total:        {sum(1 for n in DELTA_NAMES if cf_pass[n] > 0)}/{len(DELTA_NAMES)} sites passed in >=1 trial"
              f" ({sum(1 for n in DELTA_NAMES if cf_tested[n] == 0)} sites NODATA)")

    report = {
        "n_trials": n_trials,
        "bo_profiles": BO_PROFILES,
        "include_camoufox": include_cf,
        "per_site": {
            n: {
                "bo_pass": bo_pass[n], "cf_pass": cf_pass[n], "cf_tested": cf_tested[n],
                "bo_detail": bo_detail[n], "cf_detail": cf_detail[n],
            } for n in DELTA_NAMES
        },
        "triage": {
            "engine_gap": engine_gap, "ip_noise": ip_noise,
            "we_win": we_win, "both_pass": both_pass, "nodata": nodata_sites,
        } if include_cf else {},
        "trials": trials_log,
    }
    out_path.write_text(json.dumps(report, indent=1))
    print(f"\nwrote {out_path}")


if __name__ == "__main__":
    main()
