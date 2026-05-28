"""Vendor tags for corpus sites + vendor-aware spacing pass.

Sprint 1.1 of HANDOFF_2026_05_27 path-to-115. The gate runner shuffles
the corpus per-run to avoid per-IP per-vendor token clustering, but
random shuffle does NOT guarantee that consecutive sites hit different
vendors. This module:

1. Tags each known site with its antibot vendor (conservative — sites
   we don't know stay untagged and never trigger the constraint).
2. Provides `space_by_vendor(sites)` which permutes the list so no
   two consecutive sites share a (tagged) vendor.

Vendor evidence trail: docs/releases/v0.1.0-parity/02_GAP_ANALYSIS.md,
docs/releases/v0.1.0-parity/FAILED_SITES_ANALYSIS.md, and the
ANTI_BOT_VENDOR_COOKBOOK §1.
"""

# Site name -> vendor cluster tag. Only sites with HIGH confidence
# (documented per-site verdict or vendor header capture) are listed;
# unknown sites stay untagged so they never trip the spacing rule.
SITE_VENDOR = {
    # AWS WAF cluster — amazon TLDs + imdb (Amazon-owned)
    "amazon-ca": "awswaf",
    "amazon-co-uk": "awswaf",
    "amazon-com-au": "awswaf",
    "amazon-com": "awswaf",
    "amazon-de": "awswaf",
    "amazon-fr": "awswaf",
    "amazon-in": "awswaf",
    "amazon-jp": "awswaf",
    "imdb": "awswaf",
    # Kasada cluster
    "canadagoose": "kasada",
    "hyatt": "kasada",
    "realtor": "kasada",
    # DataDome cluster
    "etsy": "datadome",
    "yelp": "datadome",
    "leboncoin": "datadome",
    # Akamai BMP cluster
    "homedepot": "akamai",
    "bestbuy": "akamai",
    # Twitter rate-limit cluster (same backend, single IP)
    "x-com": "twitter",
    "twitter": "twitter",
}


def space_by_vendor(sites, max_passes=200):
    """Permute `sites` in place-ish so no two consecutive tagged sites
    share a vendor. Untagged sites act as natural separators.

    Strategy: walk the list; whenever sites[i] and sites[i+1] share a
    vendor tag, swap sites[i+1] with the next site whose vendor differs
    from both sites[i] and sites[i+2] (if it exists). Bounded by
    `max_passes` so we don't loop forever on infeasible inputs.

    Returns the spaced list (a new list — caller's input is not
    mutated). If a feasible spacing can't be found in `max_passes`
    sweeps, returns the best-effort intermediate (still vendor-mixed).
    """
    result = list(sites)
    n = len(result)
    for _ in range(max_passes):
        clashed = False
        for i in range(n - 1):
            v_i = SITE_VENDOR.get(result[i]["name"])
            v_j = SITE_VENDOR.get(result[i + 1]["name"])
            if v_i is not None and v_i == v_j:
                # Find a swap candidate: index k > i+1 whose vendor
                # differs from result[i] AND (if exists) from result[i+2].
                v_after = (
                    SITE_VENDOR.get(result[i + 2]["name"]) if i + 2 < n else None
                )
                swap_k = None
                for k in range(i + 2, n):
                    v_k = SITE_VENDOR.get(result[k]["name"])
                    if v_k != v_i and v_k != v_after:
                        swap_k = k
                        break
                if swap_k is None:
                    # Fallback: any k > i+1 whose vendor != v_i
                    for k in range(i + 2, n):
                        if SITE_VENDOR.get(result[k]["name"]) != v_i:
                            swap_k = k
                            break
                if swap_k is not None:
                    result[i + 1], result[swap_k] = result[swap_k], result[i + 1]
                    clashed = True
        if not clashed:
            break
    return result


def vendor_run_summary(sites):
    """Return a per-vendor count of adjacency clashes (for diagnostics)."""
    n = len(sites)
    clashes = {}
    for i in range(n - 1):
        v_i = SITE_VENDOR.get(sites[i]["name"])
        v_j = SITE_VENDOR.get(sites[i + 1]["name"])
        if v_i is not None and v_i == v_j:
            clashes[v_i] = clashes.get(v_i, 0) + 1
    return clashes


if __name__ == "__main__":
    import json
    import random
    import sys

    corpus_path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/corpus.json"
    seed = sys.argv[2] if len(sys.argv) > 2 else "demo"
    out_path = sys.argv[3] if len(sys.argv) > 3 else "/tmp/corpus_spaced.json"

    c = json.load(open(corpus_path))
    random.seed(seed)
    random.shuffle(c)
    before = vendor_run_summary(c)
    c = space_by_vendor(c)
    after = vendor_run_summary(c)
    json.dump(c, open(out_path, "w"))
    print(f"seed={seed} clashes before={before} after={after} -> {out_path}")
