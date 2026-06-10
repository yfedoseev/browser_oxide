# Keystroke bigram timing — data provenance

The bigram-flight ratios in `behavior.rs::bigram_ratio` are derived from
published aggregates of two open keystroke-dynamics datasets:

## Sources

1. **CMU Keystroke Dynamics Benchmark** — Killourhy & Maxion (2009), "Comparing
   Anomaly-Detection Algorithms for Keystroke Dynamics," DSN 2009.
   <https://www.cs.cmu.edu/~keystroke/>
   - Public dataset of 51 typists × 400 sessions × 8 trials of a fixed phrase.
   - We use derived statistics: mean dwell, mean flight per (key, prev-key)
     pair, summary distributions.

2. **Buffalo Keystroke Dataset** — Sun, Y., Ceker, H., Upadhyaya, S. (2016),
   "Shared keystroke dataset for continuous authentication," IEEE WIFS 2016.
   - Larger free-typing corpus; we use aggregated bigram-flight ratios.

## What we ship

Only **derived numerical aggregates** — facts which are not copyrightable
under U.S. or EU law. Specifically:

- A 26×26 ratio table (top-20 English bigrams as ratios relative to the
  median flight time) embedded as a const in `behavior.rs`.
- No raw user records. No timestamps. No user identifiers.

The ratios were computed offline from the published per-bigram means in the
above papers. Each ratio = mean_flight(bigram) / median_flight(all_bigrams).

## What we do NOT ship

- The raw datasets. Both have research-use-only redistribution restrictions
  for the raw data; we do not redistribute.
- Per-user statistics that could re-identify individuals.
- Timestamps or any temporal information from the original studies.

## License posture

Aggregating numerical statistics from published research is settled fact-use
under U.S. copyright law (Feist v. Rural Telephone, 1991) and the EU
Database Directive's "sweat of the brow" exclusion. The ratios are facts
about the English language and human motor patterns, not creative content.
