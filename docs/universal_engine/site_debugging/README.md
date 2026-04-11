# Per-site debugging writeups

These files document the deep state of each currently-blocked site as
of the 2026-04-10 session. Each writeup includes:

- The engine and exact response shape we observe
- What we've tried (with task numbers)
- What's almost certainly still wrong (ranked by likelihood)
- What to try next (ordered by ROI)
- How to reproduce the current state

| File | Site | Engine | Tractability |
|---|---|---|---|
| `adidas_akamai_bmp_v3.md` | adidas.com | Akamai BMP v3 | Hard — at the open-source frontier |
| `homedepot_akamai_bmp_v3.md` | homedepot.com | Akamai BMP v3 | Same engine; one stochastic pass observed (close but unstable) |
| `canadagoose_kasada.md` | canadagoose.com | Kasada KPSDK v3 | Solver runs cleanly, retry blocked — likely fixed by the generic refactor |
| `hyatt_kasada.md` | hyatt.com | Kasada KPSDK v3 | Same as canadagoose |
| `wildberries_wbaas.md` | wildberries.ru | WBAAS in-house | Closest of all 8 — solver POST returns 200; only retry token forwarding missing |
| `dns_shop_qrator.md` | dns-shop.ru | QRATOR | Empty PoW nonce/qsessid — single missing capability, ~4-8 hours to fix |
| `ozon_yandex_simple.md` | ozon.ru, ya.ru | Mostly redirect handling | 5 minutes (ozon) + 30-90 minutes (yandex) — quickest wins |

## Cheapest wins first

If you want to flip sites from FAIL to PASS quickly, work in this
order:

1. **ozon.ru** — one-line fix (`get_follow` instead of `get`).
2. **ya.ru** — fix probe markers + maybe one header tweak.
3. **dns-shop.ru** — capture the QRATOR script, find the missing
   capability, implement it generically.
4. **wildberries.ru** — verify the cookie-jar propagation works for
   `x_wbaas_token`; possibly that's all the site needs.

After those four, you've doubled the L3 PASS count and cleared the
"medium-effort" tier. The remaining four (adidas, homedepot,
canadagoose, hyatt) are all at the open-source frontier and require
either the clean-IP Chrome reference (task #72) or substantial
capability investment (T1.1, T1.2, T1.4 from `05_capability_gaps.md`).
