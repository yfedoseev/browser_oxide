# Holistic Site Verification — Camoufox baseline — 126 sites — 2026-04-28

**Engine**: Camoufox v135.0.1-beta.24 (patched Firefox), Python `camoufox` via `AsyncCamoufox`
**Harness**: `/tmp/camoufox_sweep.py` — same 126-URL list and same classifier as `crates/browser/tests/holistic_sweep.rs`
**Profile**: default Camoufox (`headless=True`, `humanize=False`)
**Per-site timeout**: 90 s (Playwright `page.goto` capped at 60 s + 10 s `networkidle` extension)
**Total runtime**: **7.3 min** (vs browser_oxide's 96 min — 13× faster)
**Engine errors**: **0** crashes, all 126 sites navigated cleanly
**Raw log**: `/tmp/camoufox_full.log`

---

## Headline numbers

| Outcome | Count | % |
|---|---:|---:|
| ✅ **L3-RENDERED** | **51** | **40%** |
| ⚠ **CHL** (anti-bot challenge) | **57** | **45%** |
| ❌ **BLOCKED** | **18** | **14%** |
| THIN-BODY | 0 | 0% |
| TIMEOUT | 0 | 0% |
| ERROR | 0 | 0% |

### Anti-bot vendor breakdown for the 57 CHL responses

| Vendor / marker | Count |
|---|---:|
| `captcha-CHL` | 36 |
| `Akamai-CHL` | 12 |
| `Kasada-CHL` | 3 |
| `DataDome-CHL` | 3 |
| `PerimeterX-PaH` | 1 |
| `PerimeterX-CHL` | 1 |
| `Cloudflare-CHL` | 1 |

---

## Per-category PASS rate

| Category | PASS | Tested | PASS rate |
|---|---:|---:|---:|
| tech | 7 | 9 | **78%** |
| amazon | 6 | 8 | **75%** |
| antibot | 6 | 10 | **60%** |
| reference | 3 | 5 | **60%** |
| gov-bank | 3 | 6 | **50%** |
| chl-known | 2 | 5 | **40%** |
| travel | 3 | 8 | **38%** |
| search | 3 | 8 | **38%** |
| ru | 2 | 6 | **33%** |
| misc | 4 | 12 | **33%** |
| stores | 5 | 17 | **29%** |
| streaming | 2 | 8 | **25%** |
| realestate | 1 | 4 | **25%** |
| social | 2 | 10 | **20%** |
| news | 2 | 10 | **20%** |

## Per-site appendix (authoritative — all numbers from the log)

### amazon
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| amazon-ca | L3-RENDERED | 1384 | 2008 |
| amazon-co-uk | L3-RENDERED | 3727 | 914858 |
| amazon-com-au | L3-RENDERED | 2489 | 928019 |
| amazon-com | **captcha-CHL** | 1638 | 5539 |
| amazon-de | L3-RENDERED | 3990 | 1095979 |
| amazon-fr | L3-RENDERED | 3817 | 1029964 |
| amazon-in | L3-RENDERED | 3646 | 1022815 |
| amazon-jp | **captcha-CHL** | 2374 | 5502 |

### antibot
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| amiunique | L3-RENDERED | 2968 | 724352 |
| areyouheadless | L3-RENDERED | 2811 | 3668 |
| botd | **captcha-CHL** | 923 | 2549888 |
| browserleaks-canvas | L3-RENDERED | 2055 | 23545 |
| creepjs | **BLOCKED** | 2109 | 231195 |
| fingerprintscan | **captcha-CHL** | 813 | 2729953 |
| iphey | L3-RENDERED | 3971 | 51025 |
| nowsecure | L3-RENDERED | 1991 | 179774 |
| pixelscan | **captcha-CHL** | 2122 | 288827 |
| sannysoft | L3-RENDERED | 2067 | 37450 |

### chl-known
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| adidas | L3-RENDERED | 1998 | 2372 |
| canadagoose | **Kasada-CHL** | 2939 | 799 |
| douyin | **captcha-CHL** | 10962 | 1595886 |
| hyatt | **Kasada-CHL** | 2486 | 804 |
| leboncoin | L3-RENDERED | 3622 | 473528 |

### gov-bank
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| bofa | **BLOCKED** | 4061 | 1021750 |
| chase | L3-RENDERED | 2639 | 395233 |
| irs | L3-RENDERED | 1700 | 128898 |
| paypal | **captcha-CHL** | 3275 | 432654 |
| usa-gov | L3-RENDERED | 1750 | 47480 |
| wellsfargo | **BLOCKED** | 3596 | 314091 |

### misc
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| coursera | **BLOCKED** | 2487 | 899547 |
| discord-com | L3-RENDERED | 3123 | 212764 |
| duolingo | **captcha-CHL** | 2731 | 498276 |
| imdb | L3-RENDERED | 2015 | 2001 |
| khanacademy | **BLOCKED** | 2169 | 483912 |
| medium | **captcha-CHL** | 2368 | 49234 |
| slack-com | L3-RENDERED | 3439 | 366555 |
| substack | **captcha-CHL** | 2399 | 196228 |
| udemy | **Cloudflare-CHL** | 5321 | 747695 |
| weather | **Akamai-CHL** | 3481 | 2127043 |
| yelp | **DataDome-CHL** | 11104 | 1488 |
| zoom | L3-RENDERED | 2260 | 360734 |

### news
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| bbc | L3-RENDERED | 3176 | 469731 |
| bloomberg | **captcha-CHL** | 4273 | 7829248 |
| cnn | **BLOCKED** | 3532 | 4968075 |
| economist | **BLOCKED** | 2562 | 865022 |
| ft | L3-RENDERED | 11311 | 357616 |
| guardian | **captcha-CHL** | 2229 | 1332513 |
| nytimes | **captcha-CHL** | 3350 | 1298017 |
| reuters | **BLOCKED** | 2780 | 1705484 |
| washingtonpost | **Akamai-CHL** | 2219 | 2939608 |
| wsj | **DataDome-CHL** | 1647 | 1491 |

### realestate
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| realtor | **Kasada-CHL** | 1999 | 1831 |
| redfin | L3-RENDERED | 2014 | 451905 |
| trulia | **BLOCKED** | 2078 | 229166 |
| zillow | **PerimeterX-PaH** | 2177 | 9851 |

### reference
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| github | **captcha-CHL** | 1572 | 590144 |
| mdn | L3-RENDERED | 1223 | 70924 |
| stackoverflow | L3-RENDERED | 1682 | 387449 |
| wikipedia-en | **captcha-CHL** | 1290 | 425224 |
| wiktionary | L3-RENDERED | 1324 | 399942 |

### ru
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| mail-ru | **BLOCKED** | 12611 | 1057576 |
| ozon | L3-RENDERED | 7338 | 583662 |
| ria | **captcha-CHL** | 4343 | 761067 |
| vk | **captcha-CHL** | 5095 | 190147 |
| wildberries | L3-RENDERED | 2687 | 1619 |
| yandex-ru | **captcha-CHL** | 8530 | 3392980 |

### search
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| bing | **BLOCKED** | 3263 | 402463 |
| brave | **BLOCKED** | 1208 | 95713 |
| duckduckgo | **BLOCKED** | 1813 | 445993 |
| ecosia | L3-RENDERED | 1168 | 97271 |
| google | L3-RENDERED | 1279 | 222846 |
| startpage | L3-RENDERED | 1275 | 130326 |
| yahoo | **BLOCKED** | 2286 | 758569 |
| yandex | **captcha-CHL** | 3551 | 463436 |

### social
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| facebook | **captcha-CHL** | 1794 | 445029 |
| instagram | **captcha-CHL** | 1929 | 642302 |
| linkedin | **captcha-CHL** | 1568 | 155704 |
| pinterest | **captcha-CHL** | 2474 | 876346 |
| quora | **captcha-CHL** | 2709 | 116775 |
| reddit | **captcha-CHL** | 2868 | 1137735 |
| threads | **captcha-CHL** | 4213 | 895701 |
| tumblr | **captcha-CHL** | 2747 | 962021 |
| twitter | L3-RENDERED | 5512 | 282158 |
| x-com | L3-RENDERED | 4746 | 282164 |

### stores
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| alibaba | L3-RENDERED | 4944 | 584397 |
| aliexpress | L3-RENDERED | 4259 | 959855 |
| asos | L3-RENDERED | 3854 | 315711 |
| bestbuy | **Akamai-CHL** | 3020 | 7453 |
| costco | **Akamai-CHL** | 3494 | 4329962 |
| ebay | **captcha-CHL** | 3075 | 1482707 |
| etsy | **DataDome-CHL** | 1284 | 1488 |
| h-m | **Akamai-CHL** | 1479 | 46826 |
| homedepot | **Akamai-CHL** | 6650 | 998186 |
| ikea | L3-RENDERED | 1476 | 726409 |
| macys | **Akamai-CHL** | 4547 | 2102070 |
| shopify | L3-RENDERED | 1764 | 504775 |
| target | **captcha-CHL** | 3205 | 817464 |
| uniqlo | **Akamai-CHL** | 3033 | 648259 |
| walmart | **Akamai-CHL** | 3629 | 1491461 |
| wayfair | **PerimeterX-CHL** | 11498 | 1227462 |
| zara | **Akamai-CHL** | 3884 | 717565 |

### streaming
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| disneyplus | L3-RENDERED | 845 | 178903 |
| hulu | **Akamai-CHL** | 4067 | 1650444 |
| netflix | **captcha-CHL** | 3841 | 1013620 |
| prime-video | **BLOCKED** | 3436 | 501020 |
| spotify | **captcha-CHL** | 4733 | 467168 |
| twitch | **captcha-CHL** | 11600 | 312370 |
| vimeo | L3-RENDERED | 5213 | 1976343 |
| youtube | **captcha-CHL** | 3503 | 958395 |

### tech
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| anthropic | L3-RENDERED | 5737 | 274858 |
| apple | L3-RENDERED | 2173 | 347295 |
| aws | L3-RENDERED | 6600 | 1961951 |
| azure | L3-RENDERED | 4396 | 734712 |
| cloudflare | **BLOCKED** | 3401 | 2184242 |
| google-cloud | **captcha-CHL** | 2669 | 2152814 |
| microsoft | L3-RENDERED | 3077 | 242157 |
| openai | L3-RENDERED | 3209 | 424031 |
| stripe | L3-RENDERED | 2603 | 721132 |

### travel
| Site | Outcome | nav_ms | Body len (B) |
|---|---|---:|---:|
| airbnb | **captcha-CHL** | 3576 | 1214545 |
| booking | L3-RENDERED | 4560 | 900181 |
| expedia | **Akamai-CHL** | 4242 | 854326 |
| hotels | **captcha-CHL** | 2963 | 120275 |
| kayak | L3-RENDERED | 5408 | 1614734 |
| skyscanner | **BLOCKED** | 3646 | 446880 |
| tripadvisor | L3-RENDERED | 2464 | 398194 |
| uber | **BLOCKED** | 3229 | 703249 |

---

## Reproducibility

```bash
# 1. Install Camoufox into a Python 3.12 venv
uv venv camoufox-test --python 3.12
source camoufox-test/bin/activate
uv pip install camoufox playwright
python -m camoufox fetch  # ~300 MB download

# 2. Run the harness
python /tmp/camoufox_sweep.py 2>&1 | tee /tmp/camoufox_full.log

# 3. Parse outcomes
grep "^holistic-end:" /tmp/camoufox_full.log | wc -l       # should be 126
grep "^holistic-end:" /tmp/camoufox_full.log | awk '{print $5}' | sort | uniq -c | sort -rn
```

Run takes **~7 min** on Apple M-series. 300 MB Camoufox/Firefox binary download required first time only.
