#!/usr/bin/env python3
"""Generate the 126-site corpus.json for sweep_metrics + bench_corpus_v2.

Source of truth is `crates/browser/tests/holistic_sweep.rs::sites_list()`.
This script mirrors that list and adds the `diagnostic` flag for sites
that are INTENDED to fail (e.g. `areyouheadless`).

Usage:
    python3 benchmarks/build_corpus_json.py > /tmp/corpus.json

See `docs/releases/v0.1.0-parity/audit/16_DECISION_LOG.md` §R-CORPUS-DIAGNOSTIC-FLAG.
"""
import json
import re
import sys
from pathlib import Path

# Sites that exist in the corpus as DIAGNOSTIC probes — by design they
# report "headless detected" / similar to every browser. Counting them as
# failures drags every engine's raw pass-rate down equally; reporting
# production (raw minus diagnostic) gives the honest "real-browsable"
# number.
DIAGNOSTIC_SITES = {"areyouheadless"}


def main() -> None:
    repo_root = Path(__file__).resolve().parent.parent
    holistic = (repo_root / "crates/browser/tests/holistic_sweep.rs").read_text()

    # Find the `sites_list()` block (the vec! between `fn sites_list()` and
    # its closing `]`). Each entry is `("cat", "name", "url"),`.
    fn_start = holistic.index("fn sites_list()")
    vec_start = holistic.index("vec![", fn_start)
    # Match to the matching `]` — count brackets.
    depth = 0
    end = vec_start
    for i, ch in enumerate(holistic[vec_start:], start=vec_start):
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                end = i
                break

    block = holistic[vec_start:end + 1]
    # Match `("cat", "name", "url"[,])` across multiple lines.
    # Tolerates whitespace + the trailing comma after url that some
    # multi-line entries carry.
    pattern = re.compile(
        r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,\s*"([^"]+)"\s*,?\s*\)',
        re.DOTALL,
    )
    corpus = []
    seen = set()
    for cat, name, url in pattern.findall(block):
        # Dedup by name (a few sites appear in two categories).
        if name in seen:
            continue
        seen.add(name)
        entry = {"cat": cat, "name": name, "url": url}
        if name in DIAGNOSTIC_SITES:
            entry["diagnostic"] = True
        corpus.append(entry)

    json.dump(corpus, sys.stdout, indent=2)
    sys.stdout.write("\n")
    diag = sum(1 for e in corpus if e.get("diagnostic"))
    print(
        f"# {len(corpus)} sites total; {diag} diagnostic; "
        f"{len(corpus) - diag} production",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
