# 00 — DATA: per-profile pass matrix (full gate, 2026-05-29)

Production denominator = 125. Pass = L3-RENDERED & len>=15000. Per-site isolated run.

| profile | pass/125 |
|---|--:|
| chrome_148_macos | 110 |
| pixel_9_pro_chrome_148 | 108 |
| iphone_15_pro_safari_18 | 108 |
| firefox_135_macos | 106 |

Consistent-pass (all 4): **95**  ·  Consistency gaps: **20**  ·  Fail-all: **10**

## Consistency-gap sites (pass on some profiles, fail others) — tag/len per profile

| site | chrome | pixel | iphone | firefox | fails-on |
|---|---|---|---|---|---|
| adidas | ✅ L3-RENDERED 1308551 | · L3-RENDERED 2478 | ✅ L3-RENDERED 1309818 | ✅ L3-RENDERED 1308529 | pixel |
| airbnb | ✅ L3-RENDERED 589017 | · THIN-BODY 0 | ✅ L3-RENDERED 528559 | ✅ L3-RENDERED 588983 | pixel |
| amazon-ca | · L3-RENDERED 5310 | ✅ L3-RENDERED 997677 | ✅ L3-RENDERED 891169 | ✅ L3-RENDERED 1170789 | chrome |
| economist | ✅ L3-RENDERED 529205 | ✅ L3-RENDERED 528716 | · Cloudflare-CHL 5891 | ✅ L3-RENDERED 510197 | iphone |
| ecosia | ✅ L3-RENDERED 69630 | ✅ L3-RENDERED 69273 | · Cloudflare-CHL 5444 | ✅ L3-RENDERED 69515 | iphone |
| ft | ✅ L3-RENDERED 328537 | ✅ L3-RENDERED 328494 | · Cloudflare-CHL 271064 | ✅ L3-RENDERED 333147 | iphone |
| homedepot | ✅ L3-RENDERED 994281 | · Akamai-CHL 2701 | · Akamai-CHL 2734 | · Akamai-CHL 2734 | pixel,iphone,firefox |
| macys | ✅ L3-RENDERED 1537880 | ✅ L3-RENDERED 1269917 | ✅ L3-RENDERED 1269833 | · THIN-BODY 0 | firefox |
| openai | ✅ L3-RENDERED 423760 | ✅ L3-RENDERED 423727 | · Cloudflare-CHL 10807 | ✅ L3-RENDERED 423715 | iphone |
| prime-video | ✅ L3-RENDERED 691747 | · ERROR 0 | ✅ L3-RENDERED 508277 | ✅ L3-RENDERED 643479 | pixel |
| quora | ✅ L3-RENDERED 78196 | ✅ L3-RENDERED 78713 | · Cloudflare-CHL 5843 | ✅ L3-RENDERED 78206 | iphone |
| reuters | ✅ L3-RENDERED 1138793 | ✅ L3-RENDERED 1161144 | ✅ L3-RENDERED 1126171 | · DataDome-CHL 1456 | firefox |
| spotify | · L3-RENDERED 9881 | ✅ L3-RENDERED 147739 | ✅ L3-RENDERED 147724 | · L3-RENDERED 9875 | chrome,firefox |
| tripadvisor | · DataDome-CHL 1412 | ✅ L3-RENDERED 383111 | ✅ L3-RENDERED 290654 | · DataDome-CHL 1464 | chrome,firefox |
| uber | · TIMEOUT 0 | · TIMEOUT 0 | ✅ L3-RENDERED 700635 | · TIMEOUT 0 | chrome,pixel,firefox |
| udemy | ✅ L3-RENDERED 476498 | ✅ L3-RENDERED 476498 | · Cloudflare-CHL 5929 | ✅ L3-RENDERED 476507 | iphone |
| wsj | ✅ L3-RENDERED 691418 | ✅ L3-RENDERED 285500 | ✅ L3-RENDERED 287970 | · DataDome-CHL 1461 | firefox |
| yandex-ru | ✅ L3-RENDERED 3243596 | · THIN-BODY 0 | ✅ L3-RENDERED 2712795 | ✅ L3-RENDERED 3269378 | pixel |
| yelp | · DataDome-CHL 1424 | · DataDome-CHL 1424 | ✅ L3-RENDERED 610288 | · DataDome-CHL 1458 | chrome,pixel,firefox |
| zillow | ✅ L3-RENDERED 441828 | ✅ L3-RENDERED 402231 | ✅ L3-RENDERED 402267 | · PerimeterX-PaH 14558 | firefox |

## Per-profile failure clusters

- **chrome_148_macos** (chrome) gap-fails (5): amazon-ca, spotify, tripadvisor, uber, yelp
- **pixel_9_pro_chrome_148** (pixel) gap-fails (7): adidas, airbnb, homedepot, prime-video, uber, yandex-ru, yelp
- **iphone_15_pro_safari_18** (iphone) gap-fails (7): economist, ecosia, ft, homedepot, openai, quora, udemy
- **firefox_135_macos** (firefox) gap-fails (9): homedepot, macys, reuters, spotify, tripadvisor, uber, wsj, yelp, zillow

## Fail-all-4 (frontier, separate from consistency): bestbuy, canadagoose, douyin, duolingo, etsy, hyatt, ozon, realtor, redfin, wildberries
