# 28 — AWS WAF Extended (product family, signals, WASM PoW, /verify)

**Status:** reference + research-backlog
**Audience:** any contributor who has already read `06_AWS_WAF_SOLVER.md` and needs (a) the broader AWS WAF product family context, (b) the public-research signal inventory grounded with citations, (c) the WebAssembly proof-of-work analysis, (d) the `/verify` endpoint deep dive, and (e) a forward look toward ECH / HTTP/3 / Bot Control v4 changes that will outdate today's solver design.
**Companion docs:** `06_AWS_WAF_SOLVER.md` (the solver design; tracks A/B/C), `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.1 + §2.10` (vendor identification flowchart), `27_VENDOR_COMPETITIVE_MATRIX.md §1` (per-engine numbers — BO routed 4/8 AWS-WAF cluster vs Camoufox 5/8 vs PW family 7/8), `04_TOOLING_SPEC.md` (capture tooling), `33_QUARTERLY_PROBE_ROTATION_LOG.md` (cadence).

**One-paragraph thesis:** Chapter 06 builds the solver. Chapter 28 widens the lens: AWS WAF is not one product but *four* (Challenge action, CAPTCHA action, Bot Control managed rule group, Fraud Control / ATP + ACFP rule groups), each with a different detection surface and different leverage point. Most of the public reverse-engineering attention has focused on the Challenge action's `challenge.js` because it's what fires on consumer-facing pages (Amazon, IMDb, Binance, HuggingFace) — but the real production deployment uses Challenge **plus** Bot Control plus the SDK telemetry plus TLS-level JA3/JA4 fingerprinting, *all four scoring together*. Knowing this changes the solver carry-cost calculus: a solver that beats `challenge.js` does not automatically beat Bot Control's `TGT_` rule set, and a solver that beats Bot Control does not automatically beat ATP credential-stuffing detection. This chapter inventories the lot.

---

## 0. The AWS WAF product family — four distinct surfaces

Per the [AWS WAF developer guide](https://docs.aws.amazon.com/waf/latest/developerguide/waf-managed-protections.html), "intelligent threat mitigation" decomposes into four orthogonal products. They share a common token (`aws-waf-token`) and a common SDK (`AwsWafIntegration`), but the rule-evaluation paths differ. Every public BO contributor onboarding to AWS-WAF work needs to know which one they're looking at — *the same domain can deploy all four simultaneously*.

### 0.1 Cheat-sheet

| Product | Rule-group / action name | What it does | When it fires | Identifier |
|---|---|---|---|---|
| **AWS WAF Challenge action** | `Action: Challenge` on a rule | Returns 2011-B stub with `challenge.js`; silent PoW; sets `aws-waf-token` cookie on success | Configurable per-rule (URI path, header, rate, etc.) | `x-amzn-waf-action: challenge` |
| **AWS WAF CAPTCHA action** | `Action: CAPTCHA` on a rule | Returns visual CAPTCHA page; user solves; sets `aws-waf-token` with CAPTCHA timestamp | Same trigger pattern as Challenge, but harder verdict | `x-amzn-waf-action: captcha` |
| **AWS WAF Bot Control** | `AWSManagedRulesBotControlRuleSet` | Multi-signal managed rule group; common + targeted protection levels; labels requests with bot category | Always-on once enabled in web ACL; can also issue Challenge on its own | `awswaf:managed:aws:bot-control:*` labels |
| **AWS WAF Fraud Control — ATP** | `AWSManagedRulesATPRuleSet` | Targets login endpoints; inspects credentials against stolen-credential DB; tracks failed-login rate per session | Only on the configured login path | `awswaf:managed:atp:*` labels |
| **AWS WAF Fraud Control — ACFP** | `AWSManagedRulesACFPRuleSet` | Targets signup endpoints; account-creation-fraud variant of ATP | Only on the configured signup path | `awswaf:managed:acfp:*` labels |

A consumer-facing site like Amazon-DE may deploy:
- **Challenge action** on every product-listing URL (the 2011-B stub we see).
- **Bot Control common-level** as the always-on baseline (labels and IP-rate triggers).
- **Bot Control targeted-level** specifically on cart / search / autocomplete endpoints.
- **ATP** only on `/ap/signin` (the login form).
- **ACFP** only on `/ap/register`.

A scraper hits the **first** wall — Challenge action — and never sees the latter three. But because `challenge.js` runs the **same** browser interrogation that Bot Control's `TGT_` rules use (and the resulting `aws-waf-token` is consumed by ATP), a single fingerprint mismatch can flag all four. This is why §3 below enumerates 18+ signals even though chapter 06 only needs to defeat the Challenge action: *the same signal failure that bails `getToken()` will also fail ATP if we ever try to scrape behind a login*.

### 0.2 The "Intelligent Threat Mitigation" umbrella

Per the [Token use in AWS WAF intelligent threat mitigation](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens.html) docs (verbatim):

> AWS WAF tokens are an integral part of the enhanced protections offered by AWS WAF intelligent threat mitigation. A token, sometimes called a fingerprint, is a collection of information about a single client session that the client stores and provides with every web request that it sends. AWS WAF uses tokens to identify and separate malicious client sessions from legitimate sessions, **even when both originate from a single IP address**. (emphasis added)

The "even when both originate from a single IP address" is the architectural point: AWS WAF's token system is engineered to defeat exactly the residential-proxy-rotation strategy that scraper vendors sell as the bypass. The token is server-side state; rotating IPs without rotating the token does nothing because each new IP without a token gets re-challenged from zero.

### 0.3 What this means for BO

Today (per `27_VENDOR_COMPETITIVE_MATRIX.md §1`) BO loses 4/8 of the AWS WAF cluster strict-pass. The losses are *all* on the Challenge action — we never see Bot Control labels on our 126-corpus because we don't get past the first wall. So chapter 06's solver design only has to defeat Challenge. But:

- If a future corpus added **HuggingFace** — which per `~/projects/browser_oxide_internal/docs/VERIFICATION_REPORT_2026_04_26.md:342` does not gate on Challenge but does deploy Bot Control common-level — BO would have to start scoring on the `TGT_` rule labels, not just on body-size.
- If a future corpus added a **logged-in scraping target** — a price comparator that needs an authenticated session — BO would hit ATP, which checks behavioral signals (mouse, keystroke, form-fill timing) we don't currently emit.

Both are out-of-scope for v0.1.0 (per `SCOPE.md`), but the engine architecture must not foreclose them.

---

## 1. Challenge action — the deep dive

This expands chapter 06 §0.1 with material that didn't fit there.

### 1.1 Documented behavior, verbatim

The [AWS WAF CAPTCHA and Challenge page](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge.html) says:

> **Challenge** – Runs a silent challenge that requires the client session to verify that it's a browser, and not a bot. The verification runs in the background without involving the end user. This is a good option for verifying clients that you suspect of being invalid without negatively impacting the end user experience with a CAPTCHA puzzle.

And from [How the AWS WAF CAPTCHA and Challenge rule actions work](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-how-it-works.html):

> CAPTCHA puzzles and silent challenges can only run when browsers are accessing HTTPS endpoints. Browser clients must be running in secure contexts in order to acquire tokens.

This rules out one possible BO failure mode immediately: we ARE on HTTPS, we ARE in a secure context (per `crates/js_runtime/src/js/window_bootstrap.js` `isSecureContext: true` shim). So "secure context" is not the bail.

And per [Best practices for using the CAPTCHA and Challenge actions](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-best-practices.html):

> Configure your Challenge and CAPTCHA use so that AWS WAF only sends CAPTCHA puzzles and silent challenges in response to GET text/html requests. You can't run either the puzzle or the challenge in response to POST requests, Cross-Origin Resource Sharing (CORS) preflight OPTIONS requests, or any other non-GET request types.

The Challenge action only fires on `GET text/html`. Once we have a token, subsequent `POST`/`fetch()` carries it via the `aws-waf-token` cookie or `x-aws-waf-token` header — and those *can* be against any method/content-type, just not the initial challenge gate.

### 1.2 Token immunity — the 300-second floor

From [Setting timestamp expiration and token immunity times in AWS WAF](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-immunity-times.html):

> The default protection pack (web ACL) setting for both immunity times is 300 seconds. […] The minimum value for the challenge immunity time is 300 seconds. The minimum value for the CAPTCHA immunity time is 60 seconds. The maximum value for both immunity times is 259,200 seconds, or three days.

Floor of 300 seconds = 5 minutes. Once we get past Challenge, we have at least 5 minutes of token validity. This is why `06_AWS_WAF_SOLVER.md §1.5` says "BO solver wins are sticky for the rest of the test run" — if we get the token, the corpus run completes inside the immunity window.

### 1.3 getToken() — verbatim spec

From [How to use the integration getToken](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html):

> When you call `getToken`, it does the following:
> * If an unexpired token is already available, the call returns it immediately.
> * Otherwise, the call retrieves a new token from the token provider, **waiting for up to 2 seconds for the token acquisition workflow to complete before timing out**. If the operation times out, **it throws an error**, which your calling code must handle.

The 2-second ceiling matters because:
- Our 8-second drain at `crates/browser/src/page.rs:3400` is wider than the SDK's wait window. Any `getToken()` that progresses should resolve within drain.
- The SDK is documented to *throw* on timeout. We observe (per chapter 06 §0.3) that no error is thrown, no `then` fires, no `catch` fires. **The promise stays pending forever.** This is the chapter 06 anomaly and it remains unexplained — it implies the SDK's bail path either (a) never resolves the promise (a bug in the SDK relative to its own spec) or (b) bails *before* the promise is created, swallowing the call.

### 1.4 API surface — what's documented vs what the live stub uses

The [Intelligent threat API specification](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-specification.html) lists exactly three public methods:

```
AwsWafIntegration.fetch()
AwsWafIntegration.getToken()
AwsWafIntegration.hasToken()
```

But the live 2011-B Amazon-DE stub (per chapter 06 §0.3) also calls `AwsWafIntegration.saveReferrer()`, `AwsWafIntegration.checkForceRefresh()`, and `AwsWafIntegration.forceRefreshToken()`. These are **undocumented** in the public spec page. They appear only in the AWS-published sample integrations on the [aws-samples/aws-waf-bot-control-api-protection-with-captcha](https://github.com/aws-samples/aws-waf-bot-control-api-protection-with-captcha) repo, which the [getToken doc](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html#) cross-references as a working example.

Implication for the solver: the public method surface is 3 functions; the real surface is 6. Any V8-side rewrite (chapter 06 Alt C) needs to honor all six because the stub calls the three private ones before the public ones.

### 1.5 Cross-domain token via header — the second carrier

Per the same getToken doc:

> The fetch wrapper handles these cases automatically, but if you aren't able to use the fetch wrapper, you can handle this by using a custom `x-aws-waf-token` header. **AWS WAF reads tokens from this header, in addition to reading them from the `aws-waf-token` cookie.**

So the token is carried by TWO mechanisms:
- `Set-Cookie: aws-waf-token=...` (same-domain, default for SDK fetch wrapper).
- `x-aws-waf-token: <token>` request header (cross-domain or non-fetch-wrapper calls).

A working BO solver (chapter 06 Alt B) must inject the cookie into the shared cookie jar at `crates/net/src/cookie_jar.rs` AND also be prepared to add the request header for any cross-origin `fetch()` the page makes post-solve. The SharedSession commit (`f62584d`) handles the cookie side; the header side is currently unwired.

### 1.6 The token domain rules

From [Specifying token domains and domain lists](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-domains.html):

> By default, AWS WAF only accepts tokens whose domain setting exactly matches the host domain of the resource that's associated with the protection pack (web ACL). This is the value of the `Host` header in the web request. In a browser, you can find this domain in the JavaScript `window.location.hostname` property and in the address that your user sees in their address bar.

The token has a `domain` field. If we mint a token at `*.token.awswaf.com` and present it at `www.amazon.de`, the WAF rejects it unless `www.amazon.de` is in the token domain list. For most consumer-facing deployments, the token domain list either matches the host exactly or includes a parent (`amazon.de`). Important when designing the solver: the `gokuProps.context` field (per public reverse-engineering) contains the bound domain — we can't just mint a generic token and replay it across Amazon's regional sites; each regional WAF tenant has its own token domain config.

### 1.7 Token characteristics — verbatim

From [AWS WAF token characteristics](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-details.html) (this is the most informative public doc on what's inside a token):

> The token fingerprints the client session with a sticky granular identifier that contains the following information:
> * The timestamp of the client's latest successful response to a silent challenge.
> * The timestamp of the end user's latest successful response to a CAPTCHA. This is only present if you use CAPTCHA in your protections.
> * Additional information about the client and client behavior that can help separate your legitimate clients from unwanted traffic. The information includes various client identifiers and client-side signals that can be used to detect automated activities. The information gathered is non-unique and can't be mapped to an individual human being.
>   - All tokens include data from client browser interrogation, such as indications of automation and browser setting inconsistencies. This information is retrieved by the scripts that are run by the Challenge action and by the client application SDKs. **The scripts actively interrogate the browser and put the results into the token.**
>   - Additionally, when you implement a client application integration SDK, the token includes passively collected information about the end user's interactivity with the application page. **Interactivity includes mouse movements, key presses, and interactions with any HTML form that's present on the page.** This information helps AWS WAF detect the level of human interactivity in the client, to challenge users that do not seem to be human.
>
> For security reasons, AWS doesn't provide a complete description of the contents of AWS WAF tokens or detailed information about the token encryption process.

The two paragraphs distinguish:
- **Challenge action tokens** = browser interrogation only (no mouse/keystroke).
- **SDK integration tokens** = browser interrogation + passive behavioral.

This matters: Amazon's consumer pages deploy the **Challenge action** flavor (no SDK), so behavioral simulation (mouse jiggle, keystroke timing) is NOT what's gating us. The gate is browser-interrogation only. This eliminates one possible chapter 06 failure mode and refocuses the §3 signal inventory on passive fingerprint signals only.

---

## 2. CAPTCHA action — when interactive is in play

### 2.1 What it is

Per the [CAPTCHA and Challenge in AWS WAF](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge.html) page:

> **CAPTCHA** – Requires the end user to solve a CAPTCHA puzzle to prove that a human being is sending the request. CAPTCHA puzzles are intended to be fairly easy and quick for humans to complete successfully and hard for computers to either complete successfully or to randomly complete with any meaningful rate of success.

The CAPTCHA action returns a larger interstitial (not the 2011-B stub) with a visual puzzle. The `x-amzn-waf-action` response header is `captcha` not `challenge`.

### 2.2 Triggers

CAPTCHA is rarer in the wild than Challenge — Challenge is the default for "I'm suspicious of you", CAPTCHA is "I'm very suspicious." Common triggers:
- Repeated Challenge failures from same `aws-waf-token` lineage.
- Aggressive POST-rate from a session.
- Manual operator escalation in the AWS WAF console.

In our 126-corpus, we do not currently see CAPTCHA from any AWS-WAF site — we lose at Challenge. The 1487-B yelp body that Camoufox gets (per `27 §1`) is **DataDome's** interactive captcha (`rt:'c'`), not AWS WAF's CAPTCHA action; do not conflate them.

### 2.3 Why BO doesn't need to handle CAPTCHA today

- Out of scope per `SCOPE.md` — interactive captcha solving requires either a human-in-the-loop or a 3rd-party CAPTCHA-solving service (CapMonster, 2Captcha, CapSolver). The Rust engine should never have native CAPTCHA solving code.
- The chapter 06 solver design pegs the silent-Challenge action as the target, not CAPTCHA.
- If BO ever needed to handle CAPTCHA, the pattern is: detect the CAPTCHA response, expose a solver-hook to the embedder (mirror of `ChallengeSolver` trait but for CAPTCHA), let the embedder forward to a 3rd-party CAPTCHA-solving service, return the solution to inject into the form. This is private `vendor_solvers` territory.

### 2.4 The CAPTCHA JS API

Per the [CAPTCHA JavaScript API specification](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-captcha-api-specification.html), the CAPTCHA integration exposes `AwsWafCaptcha.renderCaptcha()` for in-page rendering. Distinct from `AwsWafIntegration.*`. Important when reading `challenge.js`: if you see references to `AwsWafCaptcha`, you are looking at a CAPTCHA-action integration, not a Challenge-action integration.

---

## 3. Detection signals — the full inventory (with citations)

This is the central artifact of chapter 28. Chapter 06 §2 listed 18 candidate signals "ordered by historical hit-rate against WAFs" but without source citations. Below: every signal that public research has plausibly linked to AWS WAF, with the source, and a note on BO's current coverage.

### 3.1 Notation

| Tag | Meaning |
|---|---|
| **COVERED** | BO emits this signal correctly per real-Chrome capture |
| **GAP** | BO emits incorrectly or doesn't emit at all; known fix needed |
| **INVESTIGATE** | Status uncertain; needs a real-Chrome A/B capture to verify |
| **PROFILE-DEPENDENT** | Some BO profiles match Chrome, others don't |

For each signal: source citations are inline; the file:line reference is where BO produces the value.

### 3.2 The signal table

#### Tier-1: TLS / HTTP layer (pre-JavaScript)

| # | Signal | Source | BO status | BO emission point |
|---|---|---|---|---|
| T1 | **JA3 TLS fingerprint** (32-char MD5 of cipher list + extensions + curves + point-formats) | [doiT JA3/JA4 in AWS WAF](https://www.doit.com/blog/ja3-and-ja4-fingerprints-in-aws-waf-and-beyond), [API_JA3Fingerprint](https://docs.aws.amazon.com/waf/latest/APIReference/API_JA3Fingerprint.html) | **COVERED** (verified byte-perfect Chrome 147 per `memory/session_delta_2026_05_10.md`) | `crates/net/src/tls.rs:1-687` (boring2 + Chrome 147 ClientHello) |
| T2 | **JA4 TLS fingerprint** (newer; includes ALPN, SNI mode, cipher counts) | [doiT JA3/JA4 in AWS WAF](https://www.doit.com/blog/ja3-and-ja4-fingerprints-in-aws-waf-and-beyond) | **INVESTIGATE** (boring2 produces the same ClientHello as Chrome 147; JA4 should match; not independently captured via `tls.peet.ws` yet) | same |
| T3 | **HTTP/2 frame priority + SETTINGS ordering** | Inferred from the same TLS-fingerprint family; not directly cited by AWS but standard fingerprint-vendor practice | **COVERED** (per `23_TLS_HTTP_FINGERPRINT_REFERENCE.md`) | `crates/net/src/http2.rs` |
| T4 | **TLS extension order + lengths** (part of JA3 hash input) | [API_JA3Fingerprint](https://docs.aws.amazon.com/waf/latest/APIReference/API_JA3Fingerprint.html) | **COVERED** | `crates/net/src/tls.rs` |
| T5 | **IP reputation** (AWS-side; datacenter range lookup) | [roundproxies.com bypass guide](https://roundproxies.com/blog/bypass-aws-waf/) — "AWS maintains lists of known datacenter ranges, VPN endpoints, and previously flagged addresses" | **OUT OF SCOPE** for the engine; user-supplied via proxy config | n/a |
| T6 | **User-Agent header coherence with TLS class** | Inferred + common practice | **COVERED** per profile; `chrome_148_macos.yaml:16` UA matches the boring2 ClientHello | `crates/net/src/headers.rs` |

#### Tier-2: JavaScript navigator + window properties

| # | Signal | Source | BO status | BO emission point |
|---|---|---|---|---|
| J1 | **`navigator.webdriver`** — must be `false` for Chrome; getter must report native code | Universal anti-bot signal, confirmed by [ZenRows AWS WAF bypass](https://www.zenrows.com/blog/bypass-aws-waf) ("WebDriver flags and predictable JavaScript fingerprints") | **COVERED** | `window_bootstrap.js:991-995`, child realm `:1657`, worker `:124` |
| J2 | **`navigator.userAgent` ↔ `navigator.userAgentData`** coherence | Standard UA-CH practice; AWS validates via [WAF logging fields](https://docs.aws.amazon.com/waf/latest/developerguide/logging-fields.html) | **COVERED** but **fragile** — sec-ch-ua brand list, UA string, UA-Data brand JS must all describe the same Chrome version | UA at profile YAML:16; UA-Data in `window_bootstrap.js` |
| J3 | **`navigator.userAgentData.getHighEntropyValues()`** result | Same source; this is the async high-entropy fetch on Sec-CH-UA | **INVESTIGATE** — call returns; values must match Chrome shape exactly | `window_bootstrap.js` (search `getHighEntropyValues`) |
| J4 | **`navigator.permissions.query({name:'notifications'})`** state | [scrapfly AWS WAF bypass](https://scrapfly.io/bypass/aws-waf) implies WAF reads navigator surface broadly | **COVERED** per `cleanup_bootstrap.js:272-278` returning `'default'` (NOT `'denied'`) | `cleanup_bootstrap.js:261-279` |
| J5 | **`navigator.hardwareConcurrency`** | Common fingerprint signal; not specifically AWS-cited but standard | **COVERED** per profile | `window_bootstrap.js:974` |
| J6 | **`navigator.deviceMemory`** | Same | **COVERED** | `window_bootstrap.js:1031` |
| J7 | **`navigator.plugins` + `navigator.mimeTypes`** | [roundproxies AWS WAF bypass](https://roundproxies.com/blog/bypass-aws-waf/) — fingerprint includes "installed plugins" | **COVERED** per profile (5 plugins / 2 MIME types match Chrome 148) | `window_bootstrap.js` `_navPlugins` |
| J8 | **`navigator.language` + `navigator.languages`** | Standard fingerprint | **COVERED** | `window_bootstrap.js` |
| J9 | **`navigator.maxTouchPoints`** + `'ontouchstart' in window` coherence | macOS Chrome must have 0 touch / no touchstart; mobile profiles flip both | **COVERED** per profile (`max_touch_points` in profile YAML) | `chrome_148_macos.yaml:37` + `dom_bootstrap.js:2706` |
| J10 | **`navigator.getBattery()`** stub (deprecated in modern Chrome but still queryable) | [xKiian solver](https://github.com/xKiian/awswaf) (battery in fingerprint pool) | **COVERED** — explicitly deleted at `cleanup_bootstrap.js:15`, stub registered at `window_bootstrap.js:1163, 5331` | as cited |
| J11 | **`Notification.permission`** state | Standard fingerprint | **INVESTIGATE** — should be `'default'` on a secure context | search `window_bootstrap.js` for `Notification.permission` |
| J12 | **`document.referrer`** value after `saveReferrer()` call | Inferred from stub's call to `AwsWafIntegration.saveReferrer()` | **INVESTIGATE** — value on second iteration after `location.reload()` must equal the entry URL | `crates/browser/src/page.rs` referrer plumbing |

#### Tier-3: Screen / display / locale

| # | Signal | Source | BO status | BO emission point |
|---|---|---|---|---|
| S1 | **`screen.width` × `screen.height`** + `screen.availWidth` × `availHeight` | [roundproxies bypass](https://roundproxies.com/blog/bypass-aws-waf/) — fingerprint includes "screen dimensions" | **COVERED** per profile | profile YAML viewport/screen fields |
| S2 | **`screen.colorDepth` + `screen.pixelDepth`** | Standard | **COVERED** | profile YAML |
| S3 | **`window.devicePixelRatio`** | Standard | **COVERED** | profile YAML |
| S4 | **Timezone** via `Intl.DateTimeFormat().resolvedOptions().timeZone` | Standard cross-vendor signal | **COVERED** per profile | `crates/stealth/src/profiles.rs` timezone field |
| S5 | **Locale** via `navigator.language` + `Intl.NumberFormat().resolvedOptions().locale` coherence | Standard | **COVERED** | profile YAML |
| S6 | **`screen.orientation`** type+angle (mobile-relevant) | Inferred from mobile fingerprints | **COVERED** per profile | `window_bootstrap.js` |

#### Tier-4: Canvas + WebGL + Audio

| # | Signal | Source | BO status | BO emission point |
|---|---|---|---|---|
| C1 | **Canvas hash** via `<canvas>.toDataURL()` + `getImageData()` | [roundproxies bypass](https://roundproxies.com/blog/bypass-aws-waf/) — "Canvas rendering" | **COVERED** per profile but **PROFILE-DEPENDENT** — `canvas_seed` is profile-static; WAFs may blacklist over-seen seeds | `crates/canvas/` + `canvas_bootstrap.js`; seed in profile YAML:68 |
| C2 | **Canvas text + emoji rendering glyph offsets** | Same | **INVESTIGATE** — emoji rendering varies by OS; macOS-vs-Linux glyph offsets are distinguishable | `crates/canvas/src/text_render.rs` |
| C3 | **WebGL `getParameter(VENDOR)` / `getParameter(RENDERER)`** | Same — "WebGL capabilities" | **COVERED** per profile (`"WebKit"` / `"WebKit WebGL"`) | `canvas_bootstrap.js:443` |
| C4 | **WebGL `getParameter(UNMASKED_VENDOR_WEBGL)` / `UNMASKED_RENDERER_WEBGL`** | Standard | **COVERED** per profile ("Google Inc. (Apple)" / "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)") | `chrome_148_macos.yaml:40-41` |
| C5 | **WebGL `getSupportedExtensions()`** — order matters | Inferred + common-practice | **INVESTIGATE** — real Chrome 148 returns ~30 extensions in a specific order; need a `tls.peet.ws`-style capture to diff | `canvas_bootstrap.js:485` |
| C6 | **WebGL parameter set** (`MAX_TEXTURE_SIZE`, `MAX_VIEWPORT_DIMS`, etc.) | Standard | **COVERED** per profile | profile YAML WebGL section |
| C7 | **WebGL2 + WebGPU support flags** | Standard | **COVERED** | bootstrap |
| C8 | **AudioContext fingerprint** — offline render of `OscillatorNode` → `DynamicsCompressorNode` | [roundproxies bypass](https://roundproxies.com/blog/bypass-aws-waf/) — fingerprint includes audio | **COVERED** per profile (`audio_seed` YAML:69; DynamicsCompressor port per `memory/tier1_priority_for_akamai.md`) | `crates/audio/` |
| C9 | **AudioContext `sampleRate`** + `outputLatency` + `baseLatency` | Standard | **COVERED** per profile | bootstrap |

#### Tier-5: WebAssembly + Worker + advanced runtime

| # | Signal | Source | BO status | BO emission point |
|---|---|---|---|---|
| W1 | **WebAssembly availability** (`typeof WebAssembly === 'object'`) | Mandatory for `challenge.js` to run | **COVERED** | V8 native |
| W2 | **WebAssembly features**: `simd128`, `mutable-globals`, `bulk-memory`, `reference-types`, `threads-and-atomics`, `exception-handling`, `gc` | [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) implies the WASM module uses scrypt/sha256 PoW; certain features may be required | **INVESTIGATE** — `WebAssembly.validate()` should pass for the magic header; feature-detect each | `window_bootstrap.js:17-27` WASM wrappers |
| W3 | **WebAssembly compile + instantiate timing** | Inferred; WASM perf class can leak engine identity | **INVESTIGATE** — V8 via deno_core should match; timing skew may flag | as above |
| W4 | **Worker spawning** + `self.navigator` shape inside Worker | Some `challenge.js` builds spin up Workers to compute PoW off-main-thread (chapter 06 §2 row 13) | **COVERED** but **WORKER-PARITY-FRAGILE** — `worker_bootstrap.js:115-137` matches main-thread `webdriver`, `hardwareConcurrency`, `performance.now`; if `challenge.js` reads further properties inside Worker, gaps possible | `crates/js_runtime/src/extensions/worker_ext.rs` |
| W5 | **`MessageChannel` round-trip latency** | Inferred; some PoW variants use cross-thread timing | **INVESTIGATE** | event_loop crate |
| W6 | **`Function.prototype.toString` of polyfilled natives** | Universal anti-bot signal — if `Navigator.prototype.webdriver` getter `.toString()` doesn't return `"function get webdriver() { [native code] }"`, it's a bot | **COVERED** via `window_bootstrap.js:5325-5360` native-code masking | as cited |
| W7 | **`Symbol.toStringTag` + `[Symbol.iterator]` presence on synthetic objects** | Standard | **COVERED** | various bootstraps |
| W8 | **Proxy / Reflect detector** — wrapped polyfills emit `Proxy` traps; sniffer can inspect | Inferred | **INVESTIGATE** — depends on which APIs are wrapped via `new Proxy(...)` vs `Object.defineProperty` |  `window_bootstrap.js` |
| W9 | **Detached-iframe `contentWindow.navigator`** comparison (chrome_148_macos vs detached iframe must match) | Common engine-detection trick | **COVERED** per `memory/state_2026_05_16_phase0_rebaseline.md` (Kasada realm/sentinel/identity line CLOSED) | `window_bootstrap.js:1648-1660` child realm bootstrap |

#### Tier-6: Timing / performance

| # | Signal | Source | BO status | BO emission point |
|---|---|---|---|---|
| P1 | **`performance.now()` quantization granularity** (Chrome 148 = 5 µs cross-origin-isolated, 100 µs otherwise per Spectre mitigations) | Inferred + common practice | **INVESTIGATE** — `op_perf_now_humanized` may quantize at a different floor; verify matches Chrome's exact step | `timer_bootstrap.js:169-188`, `window_bootstrap.js:2815` |
| P2 | **`performance.now()` monotonicity** | Standard | **COVERED** | as above |
| P3 | **`Date.now()` vs `performance.timeOrigin + performance.now()`** coherence | Standard | **COVERED** | bootstrap |
| P4 | **`requestAnimationFrame` cadence** (~16.67 ms on 60Hz) | Inferred | **COVERED** | `crates/event_loop` |
| P5 | **`IntersectionObserver` callback timing semantics** | Standard | **INVESTIGATE** | `dom_bootstrap.js` |
| P6 | **JIT warm-up curve** of repeated function calls (real Chrome shows characteristic V8 tier-up timing) | Inferred; advanced | **COVERED** — we run real V8 via deno_core | n/a |

### 3.3 Top-5 highest-leverage gaps to investigate

Given the chapter 06 §0.3 observation that `getToken()` silently bails (no error, no continuation), the most likely culprits — to bisect via §4.3 of chapter 06's "negative test" protocol — are:

1. **C5 — WebGL `getSupportedExtensions()` ordering**. Cheap to verify (one DevTools paste). Highest leverage because the list is well-defined per-Chrome-version and per-OS, and a wrong order is a "guaranteed-bot" classifier signal.
2. **W4 — Worker scope `navigator.userAgentData`**. The Worker bootstrap (`worker_bootstrap.js:115-137`) overrides `webdriver`, `hardwareConcurrency`, `performance.now` — but does it cover `userAgentData`? Search needed. If `challenge.js` spawns a Worker and reads `self.navigator.userAgentData.platform`, the value may be `undefined` in our worker.
3. **J2 — UA / UA-CH / sec-ch-ua brand-list triple-coherence** under the live-request scenario. We test this in isolation but the live `challenge.js` may pull all three via different APIs and cross-check.
4. **W2 — WebAssembly feature coverage**. If `challenge.js` calls `WebAssembly.validate(...)` on a SIMD-using module and we report `false`, the WASM module never instantiates and `getToken()` cannot proceed.
5. **P1 — `performance.now()` quantization**. Chrome 148's exact quantization step is observable; a wrong step (or too-fine resolution) is a classic timing-channel bot signal.

The chapter 06 §2 "Plan B" bisect (force one signal incorrect, re-run amazon-de, watch for body-size flip) is the validation path for each of the above. Budget per signal: 30 minutes capture + 1 hour A/B run + 30 minutes write-up.

### 3.4 Per-signal verification recipes

For each signal in §3.2 marked INVESTIGATE: here is the concrete real-Chrome capture command, the BO comparison method, and the "pass / fail" criteria. Paste these into a real Chrome 148 DevTools console (Mac, clean profile, no extensions, normal browsing window — NOT incognito since some properties differ in incognito):

#### C5 — WebGL extensions ordering

Real Chrome 148 capture:

```js
// In DevTools console, run on any HTTPS site
(() => {
  const c = document.createElement('canvas');
  const gl = c.getContext('webgl');
  const exts = gl.getSupportedExtensions();
  return JSON.stringify({ count: exts.length, list: exts });
})();
```

Save the output as `/tmp/real_chrome_148_webgl_exts.json`.

BO comparison:

```bash
target/release/examples/sweep_metrics chrome_148_macos \
  /tmp/just_data_url.json /tmp/bo_webgl_exts.json \
  --init-script 'window.__bo_dump = (() => { const c = document.createElement("canvas"); const gl = c.getContext("webgl"); return JSON.stringify({ count: gl.getSupportedExtensions().length, list: gl.getSupportedExtensions() }); })();' \
  --capture-window-var __bo_dump
```

Compare:
- **count must match exactly** (any difference = guaranteed flag).
- **list order must match exactly** (any reorder = guaranteed flag).
- **list contents must match** with no missing or extra extensions.

If BO is missing extensions: extend `crates/canvas/src/webgl.rs` extension list. If BO has extras: prune. If order differs: fix the iteration order to match Chrome's emission.

#### W4 — Worker scope navigator.userAgentData

Real Chrome 148 capture:

```js
// In DevTools, run a Worker and inspect its self.navigator
(() => {
  const blob = new Blob([`
    self.postMessage(JSON.stringify({
      userAgent: self.navigator.userAgent,
      userAgentData_brands: self.navigator.userAgentData?.brands,
      userAgentData_mobile: self.navigator.userAgentData?.mobile,
      userAgentData_platform: self.navigator.userAgentData?.platform,
      webdriver: self.navigator.webdriver,
      hardwareConcurrency: self.navigator.hardwareConcurrency,
    }));
  `], {type:'application/javascript'});
  return new Promise(res => {
    const w = new Worker(URL.createObjectURL(blob));
    w.onmessage = e => { w.terminate(); res(e.data); };
  });
})();
```

BO: run the same script via `sweep_metrics --init-script`. Compare each field.

The critical field is `userAgentData_brands` — Chrome 148 in a Worker should return `[{brand:"Not(A:Brand", version:"99"}, {brand:"Google Chrome", version:"148"}, {brand:"Chromium", version:"148"}]`. If BO returns `undefined`, W4 is the gap.

#### J2 — UA / UA-CH / sec-ch-ua triple-coherence

Real Chrome 148 capture (run a server that logs request headers + a JS that POSTs back to it):

```js
// Capture all three sources of "what browser am I"
(async () => {
  const uaString = navigator.userAgent;
  const uaData = navigator.userAgentData;
  const uaDataValues = await uaData.getHighEntropyValues(['architecture','bitness','model','platform','platformVersion','uaFullVersion','fullVersionList']);
  const ch = await fetch('/echo-headers').then(r => r.json());
  return { uaString, uaDataValues, requestHeaders: ch };
})();
```

The three sources must describe the same browser:
- `uaString` → "Chrome/148.0.0.0"
- `uaDataValues.fullVersionList` includes `{brand:"Google Chrome", version:"148.0.7222.X"}`
- `requestHeaders["sec-ch-ua-full-version-list"]` includes `"Google Chrome";v="148.0.7222.X"`

The minor-version `X` MUST match between `uaDataValues` and `requestHeaders`. BO's profile YAML pins one version (e.g. `148.0.7222.107`); confirm the same value flows through both the HTTP header builder (`crates/net/src/headers.rs`) and the JS bootstrap (`window_bootstrap.js` `userAgentData`).

#### W2 — WebAssembly feature coverage

Real Chrome 148 (one-liner):

```js
JSON.stringify({
  // Magic bytes validation
  magic: WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0])),
  // SIMD: validate a v128.const i32x4 instruction
  simd: WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,15,1,13,0,253,12,0,0,0,0,1,0,0,0,2,0,0,0,3,0,0,0,11])),
  // Mutable globals
  mutable_globals: typeof WebAssembly.Global !== 'undefined',
  // Bulk memory: validate memory.copy
  bulk_memory: WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,5,3,1,0,1,10,9,1,7,0,65,0,65,0,65,0,252,10,0,0,11])),
  // Reference types
  reference_types: typeof WebAssembly.Function !== 'undefined' || WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,10,5,1,3,0,208,11])),
  // Threads & atomics: validate i32.atomic.load
  threads: WebAssembly.validate(new Uint8Array([0,97,115,109,1,0,0,0,5,4,1,3,1,1,1,4,1,2,1,1,10,9,1,7,0,65,0,254,16,2,0,11])),
});
```

All six should be `true` on Chrome 148. BO must match all six. Any `false` is a gap.

#### P1 — performance.now() quantization

Real Chrome 148 capture:

```js
(() => {
  const samples = [];
  for (let i = 0; i < 10000; i++) samples.push(performance.now());
  // Compute the GCD of differences — that's the quantization step
  const diffs = [];
  for (let i = 1; i < samples.length; i++) {
    const d = samples[i] - samples[i-1];
    if (d > 0) diffs.push(d);
  }
  const min = Math.min(...diffs);
  return { 
    min_step_microseconds: min * 1000,
    sample_count: samples.length,
    unique_diffs: new Set(diffs.map(d => Math.round(d * 1e6))).size
  };
})();
```

Chrome 148 in a non-cross-origin-isolated context: `min_step_microseconds` should be exactly `5` (5 microseconds). In a cross-origin-isolated context: `100` (100 microseconds, for `crossOriginIsolated: false` defaults).

BO emission: `crates/js_runtime/src/js/timer_bootstrap.js:169-188`. If BO emits a finer step (e.g. 1 µs or 0.1 µs), that's a "too-precise-to-be-Chrome" signal. If BO emits a coarser step (e.g. 1 ms), that's a "too-coarse-to-be-Chrome" signal. Both are flagged.

#### J3 — userAgentData.getHighEntropyValues()

```js
navigator.userAgentData.getHighEntropyValues([
  'architecture','bitness','model','platform',
  'platformVersion','uaFullVersion','fullVersionList','wow64'
]).then(r => JSON.stringify(r));
```

Expected on Chrome 148 macOS:
```json
{
  "architecture": "arm",  // or "x86" on Intel Macs
  "bitness": "64",
  "brands": [...],
  "fullVersionList": [...],
  "mobile": false,
  "model": "",
  "platform": "macOS",
  "platformVersion": "15.0.0",  // or whatever the host OS reports
  "uaFullVersion": "148.0.7222.107",
  "wow64": false
}
```

BO must emit identical shape + values. The `wow64` field is often forgotten by stealth profiles — must be `false` on macOS / Linux Chrome.

#### J11 — Notification.permission

```js
Notification.permission
```

Should be `"default"` on a fresh secure-context page. NOT `"denied"` (which would imply the user explicitly blocked notifications — uncommon).

#### J12 — document.referrer on second iteration

This requires running the page TWICE (since `saveReferrer()` is called on iter 1 and consumed on iter 2):

```js
// After location.reload() — `document.referrer` should be the URL of iter 1
console.log(document.referrer);
```

For a first-load to `https://www.amazon.de/`, `document.referrer` should be `""` (empty) — there is no referrer. After `location.reload()`, depending on AWS WAF's `saveReferrer()` semantics, the referrer may be set to the iter-1 URL. Confirm: BO's reload at `crates/browser/src/page.rs` must produce the same `document.referrer` value Chrome produces on a real `location.reload(true)`.

#### C2 — Canvas emoji + text rendering

```js
(() => {
  const c = document.createElement('canvas');
  c.width = 300; c.height = 60;
  const ctx = c.getContext('2d');
  ctx.font = '20px "Arial Unicode MS"';
  ctx.fillText('Hello, World!', 10, 30);
  ctx.fillText('🎉🚀 test', 10, 55);
  return c.toDataURL().slice(-100);  // tail of the base64 hash region
})();
```

The tail-100 of the dataURL is sensitive to:
- Anti-aliasing settings (varies by OS).
- Emoji rendering (macOS vs Linux vs Windows = different glyph data).
- Font fallback chain.

BO must produce the *macOS* dataURL when running `chrome_148_macos` profile, the *Android* dataURL when running `pixel`, etc. If BO emits Linux-host-OS rendering regardless of profile, that's a profile-vs-environment mismatch.

#### W6 — Function.prototype.toString masking

```js
[
  Object.getOwnPropertyDescriptor(Navigator.prototype, 'webdriver')?.get?.toString(),
  Object.getOwnPropertyDescriptor(Navigator.prototype, 'permissions')?.get?.toString(),
  Object.getOwnPropertyDescriptor(window, 'localStorage')?.get?.toString(),
  Navigator.prototype.javaEnabled?.toString(),
  HTMLCanvasElement.prototype.toDataURL?.toString(),
].map(s => s?.slice(0, 80));
```

Each entry must be `"function get NAME() { [native code] }"` or `"function NAME() { [native code] }"` — exactly matching Chrome's native-code masking. BO at `window_bootstrap.js:5325-5360` does this for the registered list; if any property in the live `challenge.js` query falls outside our masked set, it leaks.

### 3.5 The full-pass capture protocol

If §3.4 individual-signal verification is too piecemeal, the full-pass alternative: capture *every* property AWS WAF reads in one script.

The trick: monkey-patch `Object.prototype.toString` / property getters to log every access:

```js
// In a fresh page BEFORE challenge.js loads, install a property-access logger
(() => {
  const log = [];
  window.__awsAccessLog = log;
  
  const wrap = (obj, name, prop) => {
    const desc = Object.getOwnPropertyDescriptor(obj, prop);
    if (!desc || !desc.get) return;
    const orig = desc.get;
    Object.defineProperty(obj, prop, {
      get() {
        log.push({ obj: name, prop, t: performance.now() });
        return orig.call(this);
      },
      configurable: true
    });
  };
  
  // Wrap every navigator property
  for (const k of Object.keys(Navigator.prototype).concat(Object.keys(Object.getPrototypeOf(Navigator.prototype)))) {
    try { wrap(Navigator.prototype, 'navigator', k); } catch {}
  }
  // Wrap every screen property
  for (const k of Object.keys(Screen.prototype)) {
    try { wrap(Screen.prototype, 'screen', k); } catch {}
  }
  // Wrap window.* high-value
  for (const k of ['localStorage','sessionStorage','indexedDB','crypto','performance','Worker']) {
    try { wrap(window, 'window', k); } catch {}
  }
})();
```

Then trigger `challenge.js` load. Inspect `window.__awsAccessLog` post-execution. The list of `(obj, prop)` pairs is **exactly** the AWS WAF read set for that page load. Cross-reference each pair with §3.2 to know which signals AWS cares about RIGHT NOW (not last quarter).

Caveat: the wrapping must happen before `challenge.js` parses; if `challenge.js` caches references at module-init, our wrap is bypassed. Mitigation: install the wrap before the navigate completes (via `--init-script`).

---

## 4. WebAssembly proof-of-work analysis

### 4.1 What public research says

Per the [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) README (per our WebFetch):

> Three PoW types are explicitly named:
> 1. **HashcashScrypt** — Memory-hard cryptographic hashing
> 2. **SHA256** — Standard cryptographic hashing
> 3. **NetworkBandwidth** — Resource consumption challenges measuring network throughput

The [xKiian/awswaf](https://github.com/xKiian/awswaf) solver claims "while a Playwright instance takes 5–10 seconds per token, the solver generates one in under 100 milliseconds" — confirming the PoW is computationally tractable in pure Python/Go without WASM, when you know the algorithm. So the PoW is not the bottleneck; the fingerprint gate is.

### 4.2 The WASM module's role

The WASM module inside `challenge.js` (the one we have evidence of via `head -c 200 /tmp/challenge.js | grep AGFzbQ` per chapter 06 §1.4) handles:
1. Decrypting the puzzle payload using `gokuProps.key` (AES-CBC with `gokuProps.iv`).
2. Running the chosen PoW algorithm (HashcashScrypt / SHA256 / NetworkBandwidth).
3. Encrypting the solution + collected fingerprint payload.
4. Returning the encrypted blob for the JS wrapper to POST.

Public research (per the same neiii/aws-waf-solver and xKiian/awswaf repos) has reimplemented the PoW in pure code — so the WASM is **not** computationally novel; it's standard scrypt/SHA256/bandwidth-timing wrapped in AES-CBC. The WASM's value to AWS is:
- **Obfuscation**: the algorithm choice + parameters are inside the WASM, not the JS, so static analysis of `challenge.js` doesn't reveal them.
- **Tamper-resistance**: if you patch the JS, the WASM still runs its own validation; the bound `gokuProps.context` carries an integrity check.
- **Pinning to host environment**: the WASM imports may include `js-host`-specific functions (`Math.random`, `getRandomValues`, `crypto.subtle.*`) that must be present to instantiate.

### 4.3 Computational cost

Per the [AWS Challenge & CAPTCHA blog](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/):

> [The Challenge action] requires a client to complete a **computationally expensive task (proof of work)** to validate their environment while raising operational costs for bot operators.

The phrase "computationally expensive" is relative. Public solvers report:
- Real Chrome on modern hardware: ~100-500 ms for the full `getToken()` workflow (per [xKiian/awswaf](https://github.com/xKiian/awswaf) baseline).
- Native solver (no JS): < 100 ms.
- Slow hardware (mobile, older laptops): can take 1-2 seconds; this is why the SDK's [`getToken()` documented 2-second wait](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html) exists.

Our 8-second drain at `crates/browser/src/page.rs:3400` is plenty even if the PoW takes the full 2-second SDK timeout.

### 4.4 Reimplementation paths

Per chapter 06 §3.B, three sub-strategies for a Rust-side solver:

**Path 1 — wasmtime execution** (recommended in chapter 06)

```rust
// pseudo, full code in chapter 06 §3.B
let engine = wasmtime::Engine::default();
let module = wasmtime::Module::new(&engine, wasm_bytes)?;
let instance = wasmtime::Instance::new(&mut store, &module, &shimmed_imports)?;
let solve_fn = instance.get_typed_func::<(i32, i32), i32>(&mut store, "solve")?;
let result = solve_fn.call(&mut store, (key_ptr, iv_ptr))?;
```

Pros: AWS-WASM-version agnostic. When AWS rotates the WASM, we just re-pull and re-run.
Cons: must shim every `js-host` import the WASM uses. Public reverse-eng suggests the imports include `env.memory`, `env.abort`, `Math.random`, `Date.now`, `performance.now`, and `crypto.getRandomValues`. Each must return values consistent with our profile.

**Path 2 — pure-Rust reimplementation** (xKiian/awswaf approach)

```rust
// scrypt + sha2 + ... crates from crates.io (all MIT/Apache; license-clean)
use scrypt::{scrypt, Params};
use sha2::{Sha256, Digest};

fn solve_hashcash_scrypt(challenge: &[u8], iv: &[u8], context: &[u8]) -> Vec<u8> {
    // decrypt challenge with AES-CBC using gokuProps.key + iv
    // run scrypt with documented N, r, p parameters
    // hash the result with SHA256 to verify difficulty bits
    // return the nonce that produces the required prefix
    todo!()
}
```

Pros: no WASM runtime overhead; fast. Cons: rotation-fragile — every PoW algorithm change requires a code update.

**Path 3 — hybrid (recommended)**

Wasmtime-driven by default (rotation-resilient); pure-Rust fast-path for the dominant algorithm (HashcashScrypt). Falls back to wasmtime if the WASM's PoW header doesn't match the known fast-path.

### 4.5 The PoW is NOT the gate — the fingerprint is

The single most important insight from §4.4 + chapter 06 §0.3: the chapter 06 observation that "`POST /report` fires, but `getToken()` never resolves" means **the PoW never even starts**. The fingerprint collection + validation (which happens BEFORE the PoW request) bails first.

So: chapter 06 Alt A (engine-side stealth) is the right starting investment. Alt B's WASM-PoW work is only worth doing AFTER we know we can get past the fingerprint gate; otherwise we'd build a solver that mints valid tokens for AWS to immediately reject as "this token was minted by a known-bot fingerprint class."

---

## 5. /verify endpoint deep dive

### 5.1 Endpoint family — what public research says

Per chapter 06 §0.2 + our WebSearch results:

> Endpoints are `/inputs`, `/verify`, `/report` on the tenant token host (e.g. `1c5c1ecf7303.d474e66d.us-west-2.token.awswaf.com`).

Three endpoints, three roles:

| Endpoint | Method | Purpose | Body shape |
|---|---|---|---|
| `/inputs?client=browser` | POST | Submit collected fingerprint; receive PoW challenge | Encrypted: `{ payload: base64(AES-CBC(fingerprint_json, key, iv)) }` |
| `/verify` | POST | Submit PoW solution + final payload | Encrypted: `{ solution: base64(...), context: <opaque> }` |
| `/report` | POST | Telemetry / error reporting | Free-form; mostly empty on success, includes error tags on bail |

### 5.2 Request shape

Per [roundproxies bypass](https://roundproxies.com/blog/bypass-aws-waf/):

> AWS WAF embeds encrypted parameters in a `window.gokuProps` object containing:
> - **`key`**: An AES encryption key used to encrypt the challenge response
> - **`iv`**: The initialization vector for the AES cipher
> - **`context`**: A session identifier tied to your specific challenge

Confirmed by [The Web Scraping Club's IMDB AWS-WAF post](https://substack.thewebscraping.club/p/bypassing-aws-waf-with-scrapling):

> The HTML response contains three key components:
> 1. **gokuProps data**: Three base64-encoded values (`key`, `iv`, `context`) embedded in `window.gokuProps`
> 2. **Remote script reference**: A `<script src>` pointing to a customer-specific URL on `*.token.awswaf.com`
> 3. **Inline execution**: The page calls `AwsWafIntegration` methods including `saveReferrer()`, `checkForceRefresh()`, and `getToken()`

So:
- `gokuProps.key` = AES-CBC key (base64) — encrypts the request payload.
- `gokuProps.iv` = AES-CBC initialization vector (base64).
- `gokuProps.context` = opaque session id, bound to (a) the tenant, (b) the token domain, (c) the issuance timestamp. The server uses this to look up which key the client should be using.

The request body to `/inputs` and `/verify` is therefore:
```
{ "context": "<gokuProps.context>", "payload": "<base64(AES-CBC-encrypted(json(...)))>" }
```

The plaintext `json(...)` for `/inputs` is the fingerprint collector output (every property from §3.2). For `/verify`, the plaintext is `{ "solution": "<nonce>", "fingerprint": "<hash>", ... }`.

### 5.3 Response shape

Per chapter 06 §1.5:

> Camoufox's run will include `POST .../verify` (or `.../inputs` for the input-collector flavor) and a `200 OK` setting `Set-Cookie: aws-waf-token=...; Path=/`.

Confirmed by [Web Scraping Club IMDB post](https://substack.thewebscraping.club/p/bypassing-aws-waf-with-scrapling):

> The remote challenge.js script tests the browser environment, sends a validation payload to AWS, and returns a `Set-Cookie: aws-waf-token=...` on success. Subsequently, the page reloads and the token permits access without further behavioral analysis.

So the `/verify` response is:
- HTTP 200
- `Set-Cookie: aws-waf-token=<base64-blob>; Path=/; Secure; SameSite=None`
- Body: `{ "token": "<base64-blob>" }` (same value as the cookie, for JS-side access)

If the fingerprint failed validation:
- HTTP 200 (NOT 4xx — AWS deliberately obscures the bail)
- No `Set-Cookie`
- Body: `{ "error": "<opaque-tag>" }` or empty `{}`
- And the JS-side `getToken()` promise either resolves to `undefined` or hangs (see chapter 06 §0.3).

This is the **silent-bail** behavior that makes debugging hard: a 4xx would tell us "rejected"; a 200 with no cookie tells us "we were going to reject anyway, no need to expose the bail reason to a potential bot."

### 5.4 Server-side validation

What the server validates (inferred from the documented token-encryption design + public research):

1. **Context lookup**: `gokuProps.context` must map to a recently-issued challenge for this tenant. TTL on the context is short (likely 30-60 seconds; if you delay between receiving the stub and POSTing /verify, the context expires).
2. **Decryption**: `AES-CBC(payload, key, iv)` must produce valid JSON.
3. **Solution verification**: the PoW solution must satisfy the challenge difficulty (the WASM-minted nonce must produce a hash with N leading zero bits).
4. **Fingerprint validation**: the collected fingerprint must NOT match any "known-bot" cluster in AWS's classifier. This is the part that publicly fails most often for engine-built fingerprints (chapter 06 thesis).
5. **Token-domain check**: the `Host` header on the POST + the bound domain in `context` must agree.

If all 5 pass → issue token. If any fail → silent 200 with no cookie.

### 5.5 The `/report` endpoint

`/report` is the **telemetry** endpoint. Per chapter 06 §0.3:

> BO does execute `challenge.js` — proof: a telemetry POST to `awswaf.com/.../report` fires (logged via `crates/browser/src/page.rs:1061`).

So our engine reaches the JS execution stage. But the `/report` body is opaque + minimal; it doesn't tell us *what* the SDK reported. Possible report contents (inferred from public reverse-eng):
- Script load timing.
- Error tags (if `try/catch` caught a fingerprint-collection error).
- Browser-class hash for AWS's analytics.

The fact that `/report` fires but `/verify` doesn't tells us the fingerprint-collection completed, the bail happens AFTER collection but BEFORE the `/verify` POST. This is consistent with the SDK doing a "is this fingerprint reasonable" pre-check inside JS before bothering to do the PoW + POST.

---

## 6. AWS-side variance / non-determinism — the 85 % ceiling

### 6.1 What chapter 06 found

Per chapter 06 §0.5:

> The corpus shows AWS-side risk-rolling: amazon-co-uk (same code, same IP, four profiles) → chrome 696 KB, pixel 2011 B, iphone 1 MB, firefox 694 KB. Some fraction of the 2011-B stubs are AWS rolling the dice against us, not our fingerprint.

The 85 % ceiling estimate (chapter 06 §7.4) comes from: even amazon-co-uk on chrome profile, after many runs, doesn't pass 10/10. Empirical maximum looks like ~8.5/10 = 85 %.

### 6.2 Why variance exists — three plausible models

1. **Random sampling for ML training**: AWS WAF's `TGT_ML_` rules need labeled training data. Some fraction of requests are deliberately challenged regardless of fingerprint, so the classifier can see how each fingerprint class responds to a Challenge (does it return a valid token or not?). The data goes back into the model.
2. **Adaptive thresholding**: per-tenant rule sensitivity changes based on traffic load. Amazon-co-uk gets less attack volume than Amazon-com; the threshold for "challenge this" floats. On a quiet hour we pass, on a busy hour we don't.
3. **Cookie / state lottery**: each `Set-Cookie: aws-waf-token=...` issuance has some chance of being a "shadow token" that AWS later replays through a different evaluator. If our first run gets a "shadow", the second run gets a real eval based on it.

We cannot distinguish these models from the outside. What we can do: **always report 3-run median**, never best-of-3. Chapter 06 §4.5 enforces this.

### 6.3 What this means for solver acceptance

| Pass rate observed | Interpretation |
|---|---|
| 10/10 | Suspicious — fingerprint is too good (real Chrome rarely hits this); something is off |
| 7-9/10 | Strong solver, normal AWS variance |
| 5-6/10 | Borderline; possibly working but AWS variance is the dominant signal |
| 3-4/10 | Marginal; we're partially fooling AWS but missing a major signal |
| 1-2/10 | Solver is making no measurable difference vs baseline |
| 0/10 | Solver is actively making things worse, or AWS has identified our class |

Per chapter 06 §8: acceptance is ≥ 7/10 on a 10-run isolated A/B. Anything below is "we got lucky on the bisect, need more signal isolation."

---

## 7. Bot Control vs Challenge — when each fires

### 7.1 The decision flow

Per [AWS WAF Bot Control](https://docs.aws.amazon.com/waf/latest/developerguide/waf-bot-control.html):

> The Bot Control managed rule group provides a basic, common protection level that adds labels to self-identifying bots, verifies generally desirable bots, and detects high confidence bot signatures.
>
> The Bot Control rule group also provides a targeted protection level that adds detection for sophisticated bots that do not self identify. Targeted protections use detection techniques such as **browser interrogation, fingerprinting, and behavior heuristics** to identify bad bot traffic.

Decision flow when both Bot Control and Challenge action are configured (the typical Amazon-class setup):

```
Request arrives
   ↓
Bot Control (common level)
   - Self-identifying bot? (UA contains "bot", "scraper", "Googlebot", etc.)
     → Label as bot:verified or bot:unverified
     → Block if unverified
   - IP reputation check (datacenter range, known-bad)
     → Block if dirty
   - High-confidence bot signature
     → Block
   ↓
Bot Control (targeted level, if enabled)
   - Run `TGT_` rules: browser interrogation, fingerprinting
   - If signal labels accumulate (automated_browser, browser_inconsistency)
     → Block via aggregated score
   - If session has < 5 requests without `accepted` token
     → Challenge action issued
   ↓
Application rule with `Action: Challenge`
   - Issue 2011-B stub
   - challenge.js fingerprints + PoW + /verify
   - If token issued → request continues
   - If not → request blocked
```

### 7.2 The "Bot Control returned block" path

If Bot Control common-level fires `Block` (the request is immediately classified as a bot via UA / IP / signature):
- Response is 403 with the WAF's configured block body (often a small generic page).
- **The 2011-B Challenge stub is NEVER served.**
- `x-amzn-waf-action` header is NOT set (this header is only on Challenge / CAPTCHA actions, not on plain Block).
- The block is final; no token can rescue.

Our 126-corpus does not hit this path because (per chapter 23) our TLS class is Chrome-148 and our UA is whitelisted. If we ever did, the recovery is "fix the IP class" — Bot Control common-level is not engine-addressable.

### 7.3 The "Bot Control returned challenge" path

If Bot Control returns `Challenge` (typical for "I'm suspicious of you" but not "I'm certain you're a bot"):
- Response is the 2011-B stub with `x-amzn-waf-action: challenge`.
- The Challenge action runs; `challenge.js` decides via fingerprint whether to issue a token.
- This is **our observed path** on Amazon variants.

### 7.4 Pure Bot Control deployments (no Challenge action)

Some tenants run **only** Bot Control, no Challenge action. In that case:
- Bot Control common-level either allows or blocks.
- No JavaScript interstitial; no fingerprint collection; no token mint.
- The HuggingFace observation (per `~/projects/browser_oxide_internal/docs/VERIFICATION_REPORT_2026_04_26.md:342` — BO passes cleanly) is consistent with a pure-Bot-Control deployment that simply doesn't flag our class.

Implication: a fix that beats Challenge action does NOT automatically beat Bot Control common-level if the tenant escalates from "challenge" to "block" — different rule action, different fix path. (For BO this matters when we add HuggingFace-class scraping targets in a future corpus.)

### 7.5 The Bot Control `TGT_` labels

Per [AWS WAF Bot Control rule group](https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-bot.html), Bot Control's targeted protections add labels like:

```
awswaf:managed:aws:bot-control:signal:automated_browser
awswaf:managed:aws:bot-control:targeted:signal:automated_browser
awswaf:managed:aws:bot-control:targeted:signal:browser_automation_extension
awswaf:managed:aws:bot-control:targeted:signal:browser_inconsistency
```

The `browser_inconsistency` label is the one to fear: if Bot Control's fingerprint engine looks at our `navigator.userAgent` ("Chrome 148 macOS") and our screen.width (the iPhone profile values) — but our UA is set to chrome_148_macos — that's an inconsistency. Per-profile coherence (chapter 11) is the defense.

Per [How to use AWS WAF Bot Control for Targeted Bots signals](https://aws.amazon.com/blogs/networking-and-content-delivery/how-to-use-aws-waf-bot-control-for-targeted-bots-signals-and-mitigate-evasive-bots-with-adaptive-user-experience/), recent label additions include `TGT_SignalAutomatedBrowser`, `TGT_SignalBrowserAutomationExtension`, and `TGT_SignalBrowserInconsistency`.

---

## 8. AWS WAF Fraud Control — ATP + ACFP (a different beast)

### 8.1 ATP overview

Per [AWS WAF Fraud Control account takeover prevention (ATP)](https://docs.aws.amazon.com/waf/latest/developerguide/waf-atp.html):

> Account takeover is an online illegal activity in which an attacker gains unauthorized access to a person's account. […] You can monitor and control account takeover attempts by implementing the ATP feature. AWS WAF offers this feature in the AWS Managed Rules rule group `AWSManagedRulesATPRuleSet` and companion application integration SDKs.

ATP is the AWS WAF product for **login endpoints**. It:
- Inspects `POST` requests to the configured login URI.
- Checks credentials against AWS's stolen-credential database.
- Tracks failed-login rate per session (via the SDK token).
- Inspects responses to detect successful vs failed login (CloudFront only).

### 8.2 Why ATP needs the SDK token

Per [Using application integration SDKs with ATP](https://docs.aws.amazon.com/waf/latest/developerguide/waf-atp-with-tokens.html):

> The ATP managed rule group requires the challenge tokens that the application integration SDKs generate. The tokens enable the full set of protections that the rule group offers. We highly recommend implementing the application integration SDKs, for the most effective use of the ATP rule group.

The SDK token (different from the Challenge-action token — same cookie name, different population) includes:

> When you implement a client application integration SDK, the token includes passively collected information about the end user's interactivity with the application page. **Interactivity includes mouse movements, key presses, and interactions with any HTML form that's present on the page.**

So ATP tokens contain mouse + keystroke + form-fill behavioral data that Challenge-action tokens do not. A scraper that gets past Challenge by minting a fingerprint-only token will FAIL the moment it tries to POST to a login endpoint, because the ATP rule group expects behavioral signals in the token.

### 8.3 ATP without a token

Per the same doc:

> When web requests don't have a token, the ATP managed rule group is capable of blocking the following types of traffic:
> * Single IP addresses that make a lot of login requests.
> * Single IP addresses that make a lot of failed login requests in a short amount of time.
> * Login attempts with password traversal, using the same username but changing passwords.

So ATP still works without a token, just less effectively. The token expands what it can detect — to: credential stuffing (repeated stolen-creds use), failed-challenge-via-SDK detection, session-level rate limits.

### 8.4 ACFP — the signup variant

ACFP (Account Creation Fraud Prevention) is ATP's sibling for signup endpoints. Same architecture, different target. The [waf-tokens-block-missing-tokens.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-block-missing-tokens.html) doc notes:

> The `AWSManagedRulesACFPRuleSet` `AllRequests` rule is configured to run the Challenge action against all requests, effectively blocking any that don't have the `accepted` token label.

ACFP is **stricter** than ATP — it auto-Challenges every request to the signup endpoint, so a scraper that bypasses Challenge by spoofing a token at the signup URL will still get blocked unless the spoofed token has the correct ACFP-style label.

### 8.5 What this means for BO

ATP + ACFP are **out of scope for v0.1.0**. BO is a scraping engine, not a login-bot framework. We do not currently hit any login endpoint in the 126-corpus.

If a future use case did require authenticated scraping:
- The Challenge-action solver from chapter 06 would get us through the initial page.
- The login form POST would then trip ATP because our token doesn't have behavioral data.
- The fix is **out of scope for the public engine**; it would require either:
  - A private `vendor_solvers::AtpBehavioralForger` that injects realistic mouse-move / keystroke events between page-load and login-submit.
  - Or a 3rd-party CAPTCHA-service token mint.

For documentation completeness: do not pursue ATP / ACFP without a clear use case demanding it. The infrastructure cost (behavioral simulation engine, residential proxy compatibility, CAPTCHA-fallback wiring) is multi-engineer-week.

---

## 9. Forward-looking — AWS WAF evolution

### 9.1 Bot Control v4 (released; we're on it)

Per the [AWS WAF Bot Control rule group docs](https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-bot.html):

> Version requirement: `AWSManagedRulesBotControlRuleSet` `Version_4.0` or later. (The static version must be explicitly selected.)

Version 4.0 added **Web Bot Authentication (WBA)** — cryptographic verification for legitimate AI crawler bots (Anthropic Claude, OpenAI GPT-Search, etc.). Bots that publish their public-key directories can be "verified" and bypass Challenge actions. This is the AWS-blessed alternative to scraping; we don't care for our use case, but it's relevant context: AWS is expanding the "verified bot" surface, which means the "unverified bot" classifier (us) gets stricter over time.

### 9.2 Future: encrypted TLS metadata (ECH)

Per the [Cloudflare ECH writeup](https://blog.cloudflare.com/encrypted-client-hello/) and [RFC 9849](https://datatracker.ietf.org/doc/rfc9849/), Encrypted Client Hello hides the SNI + extensions from middleboxes. AWS WAF's JA3/JA4 fingerprinting (per [API_JA3Fingerprint](https://docs.aws.amazon.com/waf/latest/APIReference/API_JA3Fingerprint.html)) reads the outer ClientHello, but with ECH the "real" ClientHello is encrypted. Implications:
- **Short-term (2026-2027)**: AWS WAF + CloudFront support is still rolling out. ECH adoption < 5 % of traffic; no immediate impact.
- **Medium-term (2028-2029)**: as more domains add HTTPS RR records for ECH, AWS WAF will need to support ECH server-side. The outer JA3 becomes the JA3 of the *fronted* CloudFront edge, not the client. This eliminates JA3-as-identity-signal for ECH-using clients.
- **Long-term**: TLS fingerprinting moves from ClientHello-time to post-handshake (TLS 1.3 application-layer cert, ALPN, etc.) or shifts entirely to JS-layer signals.

For BO: we already support ECH-clean ClientHello via boring2; we'd need to add HTTPS RR support to net/dns when our deployment targets sites that publish them. Not urgent.

### 9.3 Future: HTTP/3 + QUIC fingerprinting

AWS CloudFront supports HTTP/3 today. The QUIC handshake has its own fingerprintable surface (transport-parameter ordering, initial-packet padding, etc.). [API_JA3Fingerprint](https://docs.aws.amazon.com/waf/latest/APIReference/API_JA3Fingerprint.html) is HTTP/1+2 only; AWS has not published a public JA4Q (QUIC) match statement yet, but it's the natural extension.

For BO: we don't currently support HTTP/3 (per `crates/net/src/lib.rs` — H1.1 + H2 only). If a future target site forces HTTP/3-only, we'd need to add a QUIC client. The TLS-layer work (boring2 ClientHello) carries over; the transport-layer work (quinn or s2n-quic) is new.

### 9.4 Future: Server-side ML escalation

Per the Bot Control docs:

> We periodically update our machine learning (ML) models for the targeted protection level ML-based rules, to improve bot predictions. […] If you notice a sudden and substantial change in the bot predictions made by these rules, contact us through your account manager.

The `TGT_ML_*` rule subset is updated **by AWS, server-side**, with no client-side rotation marker we can detect from the outside. This means our pass-rate against AWS WAF targets can change overnight without any visible change in the protocol — the same `challenge.js`, the same `/verify` response, but a different ML scoring threshold. Cross-link to chapter 33 §3 for the cadence-tracking template that catches this.

### 9.5 Future: Browser identity (Privacy Sandbox / PAT)

Chrome's Privacy Sandbox includes "Private Access Tokens" — a cryptographic primitive for a browser to prove "I am running real Chrome" without exposing identifying info. AWS WAF has not announced PAT support, but it's plausible mid-2027. For BO: we cannot mint valid PAT tokens without Chrome's private signing key. PAT is the doom-day signal for the entire "spoof Chrome" engine class.

---

## 10. Acceptance for v0.1.0

This chapter is **done** when ALL of the following hold:

- [ ] §3 signal inventory complete + each signal flagged COVERED / GAP / INVESTIGATE / PROFILE-DEPENDENT.
- [ ] At least one full `challenge.js` capture committed under `~/projects/browser_oxide_internal/benchmarks/captures/aws_waf/` (private; per CLAUDE.md keep the AWS-specific reverse-eng out of public).
- [ ] Top-5 §3.3 gaps either fixed in the public engine (per chapter 06 Alt A) or filed as scoped follow-ups in `15_OPEN_QUESTIONS.md`.
- [ ] The §1, §2, §7, §8 product-family overview is read by every contributor before they touch any AWS-WAF-related code (link from `CONTRIBUTING.md` if needed).
- [ ] §9 forward-looking risks are mirrored as entries in `24_RISK_REGISTER.md`.
- [ ] The chapter is cross-linked from `06_AWS_WAF_SOLVER.md` §6 "References" + `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.1` + `33_QUARTERLY_PROBE_ROTATION_LOG.md`.

This chapter is **not** done when:

- The Top-5 gaps are merely listed; they must be triaged (open issue with reproduction recipe) at minimum.
- The captures live only on a single developer's laptop; commit to the private repo so the next developer can resume.

---

## 11. Files / references

### 11.1 Public engine — code that touches AWS WAF

- `crates/browser/src/page.rs:1061-1063` — `x-amzn-waf-action` detection logger. The ONLY AWS-aware code in the public engine.
- `crates/browser/src/page.rs:828-852` — `Page::with_solvers` / `default_solvers` — registration seam.
- `crates/browser/src/page.rs:961-1102` — `Page::navigate_with_solvers` — entry point; this is where `csp_headers`, `csp_headers_ro`, and `accept_ch_upgrade` are forwarded.
- `crates/browser/src/page.rs:3400` — 8-second drain ceiling (covers the documented 2-s `getToken` timeout per §1.3).
- `crates/browser/src/challenge.rs:1-204` — `ChallengeSolver` trait + lifecycle. Read this before designing a solver.
- `crates/browser/src/challenge.rs:148` — `relax_response_csp` hook.
- `crates/net/src/tls.rs:1-687` — ClientHello (boring2 + Chrome 147). Verified byte-perfect; §3.2 T1-T4 reference.
- `crates/net/src/headers.rs` — header builder for UA + sec-ch-ua + accept-language; T6 + J2 reference.
- `crates/net/src/cookie_jar.rs` — SharedSession cookie jar; the AWS-WAF solver must write `aws-waf-token` here per §1.5.
- `crates/js_runtime/src/js/window_bootstrap.js:991-995, 1648-1660` — navigator.webdriver (J1).
- `crates/js_runtime/src/js/window_bootstrap.js:5325-5360` — native-code masking (W6).
- `crates/js_runtime/src/js/window_bootstrap.js:974` — hardwareConcurrency (J5).
- `crates/js_runtime/src/js/window_bootstrap.js:1031` — deviceMemory (J6).
- `crates/js_runtime/src/js/window_bootstrap.js:17-27` — WASM streaming wrappers (W1-W3).
- `crates/js_runtime/src/js/window_bootstrap.js:1163, 5331` — Battery API stub (J10).
- `crates/js_runtime/src/js/cleanup_bootstrap.js:15` — Battery delete (J10).
- `crates/js_runtime/src/js/cleanup_bootstrap.js:261-279` — permissions.query (J4).
- `crates/js_runtime/src/js/canvas_bootstrap.js:443-485` — WebGL getParameter / getExtension (C3-C7).
- `crates/js_runtime/src/js/timer_bootstrap.js:169-188` — performance.now (P1-P3).
- `crates/js_runtime/src/js/worker_bootstrap.js:115-137` — Worker scope navigator (W4).
- `crates/stealth/profiles/chrome_148_macos.yaml:16, 35-41, 60-69` — profile data for J1-J10 / S1-S6 / C1-C9.
- `crates/browser/tests/holistic_sweep.rs` — 126-site corpus.
- `crates/browser/src/classify.rs:81-156` — vendor marker tables (does NOT include AWS WAF markers per `18_ANTI_BOT_VENDOR_COOKBOOK.md §1.3`; AWS detection is header-only).

### 11.2 Private — vendor_solvers (do not modify from public)

- `~/projects/browser_oxide_internal/crates/vendor_solvers/src/lib.rs:1-66` — solver factory; add `AwsWafSolver` here per chapter 06 §5.
- `~/projects/browser_oxide_internal/crates/vendor_solvers/src/kasada_solver.rs:1-53` — template for AWS-WAF-equivalent.
- `~/projects/browser_oxide_internal/benchmarks/captures/aws_waf/` (create) — destination for §10 acceptance captures.
- `~/projects/browser_oxide_internal/docs/DEEP_NEXT_STEPS_2026_04_28.md:19-37` — original April 2026 AWS-WAF capture with `/inputs?client=browser` URL shape and 795 ms Camoufox timing reference.
- `~/projects/browser_oxide_internal/docs/VERIFICATION_REPORT_2026_04_26.md:342` — "first time we have evidence of clean AWS WAF passage" via HuggingFace; per §0.3 + §7.4, HuggingFace likely runs pure-Bot-Control-no-Challenge so don't generalize.

### 11.3 Sibling release docs

- `06_AWS_WAF_SOLVER.md` — the solver design (Alts A/B/C). This chapter widens its scope.
- `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.1` — vendor identification + public solver state.
- `18_ANTI_BOT_VENDOR_COOKBOOK.md §2.10` — (planned slot for CAPTCHA-action specifics if extended).
- `27_VENDOR_COMPETITIVE_MATRIX.md §1` — per-engine pass rates (BO routed 4/8 AWS WAF vs Camoufox 5/8 vs PW family 7/8).
- `04_TOOLING_SPEC.md` — capture tooling for the §10 acceptance artifacts.
- `11_PER_PROFILE_STRATEGY.md` — coherence requirements that prevent §7.5 `browser_inconsistency` labels.
- `14_TESTING_VALIDATION.md §L5` — 3-run sweep aggregation; the median, not best-of-3, is the metric for §6.
- `15_OPEN_QUESTIONS.md` — open backlog where §3.3 gaps are tracked.
- `23_TLS_HTTP_FINGERPRINT_REFERENCE.md` — T1-T4 reference.
- `24_RISK_REGISTER.md` — §9 forward-looking risks land here.
- `33_QUARTERLY_PROBE_ROTATION_LOG.md` — cadence-tracking template; AWS WAF entry tracks `challenge.js` obfuscation rotations + Bot Control ML model updates.

### 11.4 External AWS documentation (with role)

| URL | What it documents |
|---|---|
| [waf-bot-control.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-bot-control.html) | Bot Control overview; common + targeted protection levels |
| [aws-managed-rule-groups-bot.html](https://docs.aws.amazon.com/waf/latest/developerguide/aws-managed-rule-groups-bot.html) | Bot Control rule group; `TGT_` rule names; labels (browser_inconsistency, automated_browser); WBA |
| [waf-atp.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-atp.html) | ATP overview |
| [waf-atp-components.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-atp-components.html) | ATP rule group + login-page config + response inspection |
| [waf-atp-with-tokens.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-atp-with-tokens.html) | Why ATP needs SDK tokens (behavioral data) |
| [waf-captcha-and-challenge.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge.html) | Challenge vs CAPTCHA action definitions |
| [waf-captcha-and-challenge-how-it-works.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-how-it-works.html) | Lifecycle; HTTPS-only requirement |
| [waf-captcha-and-challenge-best-practices.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-captcha-and-challenge-best-practices.html) | GET text/html requirement; immunity time tuning |
| [waf-tokens.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens.html) | Token system overview |
| [waf-tokens-details.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-details.html) | Token contents (the most informative public doc) |
| [waf-tokens-immunity-times.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-immunity-times.html) | 300 s default / 60 s CAPTCHA min / 3-day max |
| [waf-tokens-domains.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-domains.html) | Token domain matching rules |
| [waf-tokens-block-missing-tokens.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-tokens-block-missing-tokens.html) | Token labels (accepted/rejected/absent); ATP/ACFP/BotControl behavior on each |
| [waf-js-challenge-api-specification.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-specification.html) | Public API surface (fetch/getToken/hasToken — only three) |
| [waf-js-challenge-api-get-token.html](https://docs.aws.amazon.com/waf/latest/developerguide/waf-js-challenge-api-get-token.html) | 2-second timeout; x-aws-waf-token header alternative |
| [API_JA3Fingerprint.html](https://docs.aws.amazon.com/waf/latest/APIReference/API_JA3Fingerprint.html) | JA3 match statement; 32-char MD5; fallback behavior |

### 11.5 AWS blogs

- [Protect against bots with AWS WAF Challenge and CAPTCHA actions](https://aws.amazon.com/blogs/networking-and-content-delivery/protect-against-bots-with-aws-waf-challenge-and-captcha-actions/) — 4-element token (timestamp, fingerprint hash, puzzle type, domain); 5-minute default immunity quote.
- [Optimizing web application user experiences with AWS WAF JavaScript integrations](https://aws.amazon.com/blogs/networking-and-content-delivery/optimizing-web-application-user-experiences-with-aws-waf-javascript-integrations/) — PoW concept; SDK telemetry.
- [How to use AWS WAF Bot Control for Targeted Bots signals](https://aws.amazon.com/blogs/networking-and-content-delivery/how-to-use-aws-waf-bot-control-for-targeted-bots-signals-and-mitigate-evasive-bots-with-adaptive-user-experience/) — `TGT_Signal*` rule names.

### 11.6 Open-source solver research (hypotheses, not ground truth)

- [xKiian/awswaf](https://github.com/xKiian/awswaf) — Python + Go solver; supports "token / Invisible" + "Captcha"; under-100-ms native PoW.
- [neiii/aws-waf-solver](https://github.com/neiii/aws-waf-solver) — names HashcashScrypt / SHA256 / NetworkBandwidth PoW types; describes 4-step flow (discover → solve → fingerprint → submit).
- [Switch3301/Aws-Waf-Solver](https://github.com/Switch3301/Aws-Waf-Solver) — same 4-step; details on challenge.js deobfuscation passes.
- [jonathanyly/awswaf-solver-api](https://github.com/jonathanyly/awswaf-solver-api) — FastAPI wrapper exposing `POST /solve`; returns `{ "token": "..." }`.
- [aferapi/aws-waf-solver](https://github.com/aferapi/aws-waf-solver) — independent Python reimplementation.
- [aws-samples/aws-waf-bot-control-api-protection-with-captcha](https://github.com/aws-samples/aws-waf-bot-control-api-protection-with-captcha) — AWS's own sample showing the undocumented `saveReferrer` / `checkForceRefresh` / `forceRefreshToken` methods.

### 11.7 Third-party writeups

- [roundproxies — How to bypass AWS WAF in 2026: 5 working (free) methods](https://roundproxies.com/blog/bypass-aws-waf/) — fingerprint signal list (canvas, WebGL, fonts, screen); gokuProps {key, iv, context} description; HTTP 202 / 405 split.
- [ZenRows — How to Bypass AWS WAF](https://www.zenrows.com/blog/bypass-aws-waf) — webdriver flag importance; multi-layer detection narrative.
- [Scrapfly — Bypass AWS WAF](https://scrapfly.io/bypass/aws-waf) — detection chain (TLS → challenge.js → telemetry → aws-waf-token → /verify); 96% success claim.
- [The Web Scraping Club — Bypassing AWS WAF on IMDB with Scrapling](https://substack.thewebscraping.club/p/bypassing-aws-waf-with-scrapling) — gokuProps content confirmed; "AWS WAF relies primarily on IP reputation and rate limiting [after token issue], rather than continuous request scoring".
- [DevInsight — How to Bypass Amazon(AWS WAF) CAPTCHA With NodeJS](https://medium.com/@qzxlqvd9542i/how-to-bypass-amazon-aws-waf-captcha-with-nodejs-when-scraping-e2a2023fc4cc) — HTTP 202 / 405 status code split; awsKey / awsIv / awsContext / awsChallengeJS parameter set.
- [DEV Community — How to Solve AWS WAF Challenges with Node.js](https://dev.to/ren_joyce_cd41204d5cb261f/how-to-solve-aws-waf-challenges-with-nodejs-2obe) — same 202/405 + parameter-set; cap-solver-style outsource pattern.
- [doiT — JA3 and JA4 Fingerprints in AWS WAF](https://www.doit.com/blog/ja3-and-ja4-fingerprints-in-aws-waf-and-beyond) — JA3/JA4 as rate-limit aggregation key + blocklist; documented in separate match statement, NOT part of Bot Control.
- [Vahid Faraji — How AWS WAF Interacts with Browser Requests and Cookies](https://vahid-faraji-dev.medium.com/how-aws-waf-interacts-with-browser-requests-and-cookies-a78558a69f18) — token-as-reference-not-JWT clarification.

### 11.8 External non-AWS context

- [Cloudflare — Encrypted Client Hello announcement](https://blog.cloudflare.com/encrypted-client-hello/) — §9.2 ECH context.
- [RFC 9849 — TLS Encrypted Client Hello](https://datatracker.ietf.org/doc/rfc9849/) — §9.2 normative spec.
- [Cisco — ECH Defense Strategies](https://secure.cisco.com/secure-firewall/docs/encrypted-client-hello-defense-strategies-how-cisco-secure-firewall-tackles-ech) — middlebox response patterns; gives sense of what AWS WAF will need to do.

---

## 12. Honesty disclaimer

This chapter compiles publicly-available reverse-engineering + AWS-published docs. The third-party solver claims (PoW types, endpoint shapes, gokuProps content) are research hypotheses; we have not independently captured AND deobfuscated a current `challenge.js` to confirm every detail. The chapter 06 §10 acceptance ("`/tmp/challenge.deobf.js` exists, committed to `docs/research_2026_05_24/awswaf/captures/`") is the prerequisite for converting hypotheses into ground truth.

What is GROUND TRUTH (independently verified):
- §1.1-1.7 AWS docs are quoted verbatim where they say something specific.
- §2 CAPTCHA vs Challenge distinction.
- §7 Bot Control behavior per the public docs.
- §8 ATP / ACFP scope per the public docs.

What is HYPOTHESIS pending capture:
- §3.2 signal table entries marked INVESTIGATE.
- §4 WASM module's exact PoW type per tenant (probably HashcashScrypt for amazon-de, but not confirmed).
- §5.2-5.4 `/verify` endpoint body shapes (assembled from public solver repos, not from our own capture).
- §6.2 the three variance models (we cannot tell from the outside).

What is FORWARD-LOOKING SPECULATION:
- §9 — all of it. Use to seed `24_RISK_REGISTER.md` entries, not to drive code today.

Per `CLAUDE.md`: do NOT add AWS-WAF-specific reverse-engineered code to the public engine. The §3 signal-list fixes are engine-faithfulness-to-Chrome fixes, not AWS-WAF bypasses. If a §3 fix's commit message would have to say "this is to defeat AWS WAF", reframe it as "this aligns BO's $signal with Chrome 148's $signal per real-Chrome capture" — and put the AWS-aware variant of the fix into `vendor_solvers` instead.
