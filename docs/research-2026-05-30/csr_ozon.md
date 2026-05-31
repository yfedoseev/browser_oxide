# OZON (https://www.ozon.ru/) — THIN-BODY/156 root cause

**Date:** 2026-05-30
**Profile:** chrome_148_macos
**Symptom:** BO returns `THIN-BODY len=156` (`html_len=164` in nav log). v150 passes.
**Verdict:** This is **NOT a render/CSR gap**. It is a **data-fetch-gate** bug in
`crates/net` cookie handling. BO never gets past Ozon's edge antibot redirect loop
because it **silently drops the gate cookie** due to a 2-digit-year `Expires` date
mis-parse. **High confidence. Flippable with a 1-line public-engine fix.**

---

## 1. What Ozon actually serves (live probe)

Ozon front-doors every request with a **custom "fab" antibot** ("Antibot Challenge
Page", served from `cdn2.ozone.ru/s3/abt-challenge/...`). The gate works in two stages:

1. `GET /` → **HTTP 307** → `Location: https://www.ozon.ru/?__rr=1`,
   `Set-Cookie: __Secure-ETC=<hex>; expires=Mon, 31-May-27 ...; domain=.ozon.ru; SameSite=None; HttpOnly; Secure`.
   (`__rr` = "redirect round"; the server bumps it 1→9 then wraps 9→1.)

2. `GET /?__rr=N` — the response depends on the request:
   * **No `__Secure-ETC` cookie echoed → another 307** (bump `__rr`). Infinite loop.
   * **`__Secure-ETC` echoed back + a recent-real-browser `User-Agent` → HTTP 403
     with a ~49 KB gzipped JS challenge page** (`ozon-antibot: 1`, sets `abt_data`).
     The challenge JS (obfuscated generator-VM, `runChallenge` bytecode, `_doCryptBlock`,
     navigator/performance fingerprint dump) computes a token, submits it, and reloads
     past the gate.

### The gate trigger (curl bisection, all reproducible & stable across 3+ iters)

| Request | Result |
|---|---|
| valid `__Secure-ETC` + `Chrome/148 macOS` UA | **403 challenge** |
| **no** cookie (any UA) | 307 loop |
| valid cookie, **no** UA | 307 loop |
| valid cookie, `curl/8.0` \| `Mozilla/5.0` \| `Googlebot` \| Chrome/120 \| Win-Chrome/148 \| Firefox UA | 307 loop |
| valid cookie + macOS-Chrome-148 UA over **HTTP/1.1** | 403 |
| valid cookie + macOS-Chrome-148 UA over **HTTP/2** | 403 |
| valid cookie + macOS-Chrome-148 UA over **HTTP/3** | 403 |

**Conclusions from the bisection:**
* The gate is **protocol- and TLS/JA4-agnostic** (h1/h2/h3 all 403). **Not** a TLS/JA4
  or HTTP/2-fingerprint blocker.
* The gate is satisfied by exactly two things: **(a) the `__Secure-ETC` cookie echoed
  back**, and **(b) a current real-browser UA** (the macOS-Chrome-148 string the profile
  already uses).
* Replaying **BO's exact `chrome_148_macos` nav header set** (sec-ch-ua, sec-fetch-*,
  accept, priority, accept-encoding `gzip,deflate,br,zstd`, etc.) via curl **+ the cookie
  → 403**. So **BO's headers are correct**; nothing in the header set suppresses the gate.

The only remaining variable: **does BO actually send `__Secure-ETC` on the wire?** It
does not.

## 2. BO's live behavior

```
[redirect] hop=0 GET https://www.ozon.ru/        <- 307  loc=?__rr=1  set-cookies=[__Secure-ETC=c38f1c...]
[redirect] hop=1 GET https://www.ozon.ru/?__rr=1 <- 307  loc=?__rr=2  set-cookies=[__Secure-ETC=c38f1c...]
...
[redirect] hop=9 GET https://www.ozon.ru/?__rr=9 <- 307  loc=?__rr=1
[redirect] hit max_redirects=10, final GET https://www.ozon.ru/?__rr=1
[navigate] iter=0 url=https://www.ozon.ru/?__rr=1 html_len=164
```

BO receives the cookie on every hop but **never echoes it** → permanent 307 loop →
`get_follow` exhausts `max_redirects=10` and returns the last 164-byte 307 body →
`THIN-BODY len=156`. (`get_follow` is at `crates/net/src/lib.rs:911`; cookie attach at
`get_with_headers` `crates/net/src/lib.rs:788-793`.)

## 3. ROOT CAUSE — `__Secure-ETC` is dropped by a 2-digit-year `Expires` mis-parse

Direct in-crate probe (temporary test against the real `CookieJar`):

```
cookies_for(www.ozon.ru/?__rr=1) = None       <-- cookie NOT echoed
jar buckets = []                               <-- jar EMPTY, cookie never stored
parse_http_date("Mon, 31-May-27 00:05:44 GMT") = Some(0)   <-- BUG
parse_set_cookie(...) -> name="__Secure-ETC" expires=Some(0) domain="ozon.ru"
```

Ozon's `Set-Cookie` uses the **Netscape 2-digit-year date** form:
`expires=Mon, 31-May-27 00:05:44 GMT` (abbreviated weekday `Mon`, dashes, 2-digit year `27` = 2027).

`parse_http_date` (**`crates/net/src/cookies.rs:203-219`**) tries these formats in order:

```rust
const FORMATS: &[&str] = &[
    "%a, %d %b %Y %H:%M:%S GMT",   // #1 spaces, 4-digit  -> Err (Ozon uses dashes)
    "%a, %d-%b-%Y %H:%M:%S GMT",   // #2 dashes, 4-digit %Y
    "%A, %d-%b-%y %H:%M:%S GMT",   // #3 dashes, 2-digit %y -- BUT %A = FULL weekday
    "%a %b %e %H:%M:%S %Y",        // #4 asctime -> Err
];
```

* Format **#3** is the one meant for this case, but it uses **`%A` (full weekday
  "Monday")** while Ozon/nginx send the **abbreviated "Mon" (`%a`)** → no match.
* Format **#2** (`%Y`, 4-digit) then **greedily matches** the 2-digit `27` and parses it
  as **year 27 AD** → timestamp `-61302182056`. `parse_http_date` does `.max(0)` →
  returns **`Some(0)`** (Unix epoch / 1970).

Then in `set_cookies` (**`crates/net/src/cookies.rs:73`**), the **FIX-COOKIE-DELETE**
guard (added for the AWS-WAF imdb fix) treats an expiry `<= now` as a *deletion*:

```rust
if cookie.expires.is_some_and(|e| e <= now) {   // 0 <= now(2026) == true
    ... remove the cookie ...                    // __Secure-ETC deleted, never stored
    continue;
}
```

So the cookie is parsed, mis-dated to 1970, and **deleted instead of stored**. The jar
stays empty, Ozon never sees `__Secure-ETC`, and the 307 loop never breaks.

This is a **regression interaction**: FIX-COOKIE-DELETE (correct for real `expires=1970`
deletions) now also nukes any valid cookie whose 2-digit-year `Expires` the date parser
clamps to 0. Ozon's far-future `-27` cookie is the trigger.

## 4. FIX (public-engine, 1 line)

**`crates/net/src/cookies.rs:205-210`** — add the abbreviated-weekday 2-digit-year format
and place it **before** the 4-digit `%Y` dashed format so the 2-digit case matches first:

```rust
const FORMATS: &[&str] = &[
    "%a, %d %b %Y %H:%M:%S GMT",
    "%a, %d-%b-%y %H:%M:%S GMT",   // ADD: Netscape 2-digit year, abbrev weekday (Ozon/nginx)
    "%a, %d-%b-%Y %H:%M:%S GMT",
    "%A, %d-%b-%y %H:%M:%S GMT",   // (existing; %A full-weekday variant kept for safety)
    "%a %b %e %H:%M:%S %Y",
];
```

Validated standalone (chrono 0.4):
```
OLD "Mon, 31-May-27 00:05:44 GMT" -> Some(0)            (buggy: deleted)
FIX "Mon, 31-May-27 00:05:44 GMT" -> Some(1811721944)   (2027: stored)
FIX "Tue, 01 Jan 2999 ..." -> Some(32472144001)         (4-digit still OK)
FIX "Thu, 01 Jan 1970 ..." -> Some(1)                   (real deletes still fire)
FIX "Mon, 31 May 2027 ..." -> Some(1811721944)          (spaced 4-digit still OK)
```

Optional hardening (defense-in-depth, not required for Ozon): in `parse_http_date`,
treat a parse that yields a *negative* timestamp as **`None`** rather than clamping to
`Some(0)` — so a future mis-parse degrades to a session cookie (stored) instead of an
accidental deletion. The clamp `.max(0) as u64` is what converted "year 27" into a
spurious deletion signal.

### Cookie sent → does Ozon then pass?

Once `__Secure-ETC` is stored, BO will echo it on `?__rr=1` and Ozon will return the
**403 JS challenge** (confirmed via curl with the same cookie+UA). Clearing the redirect
gate (307→challenge) is the **necessary first step** and the **current sole blocker**.
Whether BO then renders depends on executing the `fab` challenge VM (a downstream,
separate concern). v150 (a real Firefox/Gecko) sends a valid `__Secure-ETC` natively and
runs the challenge, which is why it passes. With this fix BO reaches the same starting
line; if the challenge JS doesn't self-solve in the nav path, that becomes the next
(separate) investigation — but it cannot even begin today because the cookie never
survives.

## 5. Discarded hypotheses (with evidence)

* **TLS/JA4 rejection** — ruled out: curl (non-Chrome TLS) with cookie+UA → 403; gate is
  TLS-agnostic.
* **HTTP/2 fingerprint** — ruled out: gate fires identically over h1/h2/h3.
* **Header/UA gate** — partially true but **not BO's problem**: BO's exact nav header set
  + cookie → 403 via curl. UA must be a current real browser (BO's already is).
* **HTTP/3 cookieless path** (`try_h3_request` early-returns before cookie attach,
  `crates/net/src/lib.rs:768`; `h3_request` never reads the jar,
  `crates/net/src/h3_request.rs:49`) — **real latent bug**, but **not active here**:
  all presets set `allow_http3: false` (`crates/stealth/src/presets.rs`), so the nav path
  stayed on H2. *Flag for a separate fix:* if H3 is ever enabled, the H3 path must attach
  jar cookies, or this class of gate breaks again.
* **Render/CSR gap** — ruled out: BO never receives any app HTML at all; the body is the
  nginx 307 stub.

## 6. Reproduction

```bash
# Live gate behavior (cookie is what flips 307 -> 403):
curl -sS -o/dev/null -w '%{http_code}\n' \
  -H 'user-agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36' \
  -b '__Secure-ETC=<value-from-first-307>' 'https://www.ozon.ru/?__rr=1'   # -> 403

# BO:
echo '[{"name":"ozon","url":"https://www.ozon.ru/","cat":"t"}]' > /tmp/ozon.json
BROWSER_OXIDE_DEBUG_NAV=1 BROWSER_OXIDE_DEBUG_REDIRECTS=1 BO_SITE_TIMEOUT=70 \
  /tmp/warm_verify/sweep_stable chrome_148_macos /tmp/ozon.json /tmp/ozon.out 2>/tmp/ozon.log
# /tmp/ozon.log shows __Secure-ETC received every hop but never broken out of the loop.
```

In-crate proof (temporary test, since reverted): feeding Ozon's exact `Set-Cookie` to the
live `CookieJar` leaves `jar buckets = []` and `cookies_for(...) = None`.
