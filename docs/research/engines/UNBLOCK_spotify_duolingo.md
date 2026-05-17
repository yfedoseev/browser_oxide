# UNBLOCK — open.spotify.com & duolingo.com ("captcha human-gate" edge-block)

**Created:** 2026-05-16 · **Baseline git HEAD:** `fd98bfa` · Method:
verify-don't-assume — every claim below is anchored to a captured
artifact (`/tmp/audit_failing_sites/{spotify,duolingo}.json`), the
classifier source (`crates/browser/src/classify.rs`), or a cited
2026 web source.

## TL;DR verdict

| Site | True block class | Fixable in pure-stealth scope? |
|---|---|---|
| **open.spotify.com** | **(A) Classifier false-positive** — bare-`captcha` substring matched the benign **invisible reCAPTCHA v3 badge** in a small SPA shell. No interactive captcha, no human gate. | **Yes — classifier-only.** The "captcha-CHL / edge-block" tag is wrong; this is a thin-shell render-completeness datapoint at worst. |
| **duolingo.com** | **(B) UA / browser-support redirect gate** *plus* the **same (A) classifier FP** on the redirected error page. Final URL `…/errors/not-supported.html` is a static "Browser not supported" page that **happens to embed the reCAPTCHA v3 SDK**, which our bare-`captcha` row matched. **Not a CAPTCHA, not a human gate.** | **Yes — engine/profile + classifier.** The fix is making our client clear Duolingo's browser-support check (UA / JS-feature) so it never redirects; the captcha tag is a downstream FP of the FP. |

**Neither site is a genuine interactive human CAPTCHA. Both are
mis-bucketed.** The "captcha-CHL" tag in the live audit is a weak
bare-substring artifact, exactly as the directive suspected.

---

## STEP 1 — our side: what actually triggered "captcha-CHL"

### The classifier rule [CODE]

`crates/browser/src/classify.rs`:

- `SMALL_BODY` table (only consulted when `body.len() <
  INTERSTITIAL_MAX_BYTES` = 30 KB) ends with two weak rows:
  `("px-captcha","PerimeterX-CHL")` then **`("captcha","captcha-CHL")`**
  (classify.rs:114–115).
- `("captcha","captcha-CHL")` is a **bare substring match**. Any body
  < 30 KB containing the 7 letters `captcha` anywhere — inline JS, a
  CSS class name, a reCAPTCHA SDK `<script src>`, the string
  `.grecaptcha-badge` — is tagged `captcha-CHL`.
- `verdict_for("captcha-CHL", len)` falls to the catch-all
  `_ if len < SENSOR_SPLIT_BYTES (50 KB) => ChallengeVerdict::EdgeBlock`
  (classify.rs:142). So a 9.6 KB / 13 KB body with the literal word
  `captcha` → **`edge-block`**, regardless of whether anything was
  actually challenged.
- This is precisely the FP class flagged in
  `docs/research/engines/99_CODE_FALSE_POSITIVES.md` **FP-B2** (weak
  literal markers mislabel rendered pages) and **FP-B3** (thin-shell
  band). FP-B2/B3 already relocated/size-gated `px-captcha` and added
  the `ThinShell` band, but **the bare `captcha` row itself was left
  un-co-signal-gated** — it still fires on any <30 KB body. Spotify and
  Duolingo are live proof that this row is still over-matching.

### The captured evidence — `marker_contexts` [MECH]

`/tmp/audit_failing_sites/spotify.json`
(`verdict: edge-block`, `body_len: 9586`, `final_url:
https://open.spotify.com/`, cookies `sp_new sp_landingref sp_t
sp_landing` — Spotify's own first-party landing cookies, **no
PerimeterX `_px*` / no DataDome / no Akamai cookie**):

```
g-recaptcha: …="grecaptcha-error"></div><textarea id="g-recaptcha-response-100000" name="g-rec…
captcha:     …ous" type="application/json"><style>.grecaptcha-badge { display: none !important…
```

Both hits are the **invisible reCAPTCHA v3 enterprise badge** plumbing
that Spotify's login/auth widget ships on every page:
`grecaptcha-response` hidden `<textarea>` and the
`.grecaptcha-badge { display:none }` CSS that hides the v3 badge. **v3
is scoreless and invisible — there is no challenge to solve and nothing
for a user to click.** This is normal Spotify chrome, not an
interstitial. The body is a 9.6 KB SPA bootstrap shell (Spotify's web
player hydrates client-side), not a challenge page.

`/tmp/audit_failing_sites/duolingo.json`
(`verdict: edge-block`, `body_len: 12999`, `final_url:
https://www.duolingo.com/errors/not-supported.html`, cookies `tsl lu
initial_referrer lr` — Duolingo first-party, **no anti-bot vendor
cookie**):

```
g-recaptcha: …="grecaptcha-error"></div><textarea id="g-recaptcha-response-100000" name="g-rec…
captcha:     …async="" src="https://www.gstatic.com/recaptcha/releases/U5VsmTDhJM1iOJUyw4DEUTY…
```

Identical pattern: the second hit is literally the **Google reCAPTCHA
SDK `<script src="https://www.gstatic.com/recaptcha/releases/…">`**
tag that Duolingo's global template includes on every page (including
its static error pages). The decisive fact is the **`final_url`**:
the engine was **302-redirected to `/errors/not-supported.html`** — a
static "Sorry, your browser version is not supported" page (confirmed
by direct fetch, see Step 2). The "captcha" the classifier saw is the
reCAPTCHA SDK on that *error* page, not a gate.

**Conclusion of Step 1:** for *both* sites the `captcha-CHL` tag is a
bare-substring false positive on benign reCAPTCHA-v3-SDK / badge
markup. No `px-captcha`, no `iframe src=…captcha-delivery…`, no
`g-recaptcha` *interactive* sitekey widget bound to a visible
challenge, no vendor challenge cookie. The audit harness's own code
comment (audit_failing_sites.rs:308–311) anticipated exactly this:
*"tell whether it's a real challenge frame … or an inline-JS
reference (false positive)"* — here it is the FP case.

---

## STEP 2 — web research (2026)

### Duolingo `/errors/not-supported.html` [MECH]

- Direct fetch of `https://www.duolingo.com/errors/not-supported.html`
  (2026-05-16): a static page reading **"Sorry, your browser version
  is not supported. Please use the latest version of one of the
  following browsers: Google Chrome, Apple Safari, Mozilla Firefox,
  Microsoft Edge."** **No reCAPTCHA challenge, no Press & Hold, no
  interactive element** — it just embeds the site-wide reCAPTCHA SDK
  (the marker our classifier tripped on).
- Mechanism: Duolingo runs a **client/edge browser-support check** and
  **redirects unsupported clients to this static page**. The
  webcompat tracker corroborates a UA-class block: Firefox users
  historically hit "browser unsupported", and the issue is labelled
  *"requires a UA override for working"* — i.e. the gate keys off the
  **User-Agent string** (and/or a JS feature-detection probe), not a
  bot score. (webcompat/web-bugs #56109, #69764, #120827.)
- Implication for us: our headless client is presenting a
  UA/feature-detection profile Duolingo's support check rejects, so it
  is bounced to `not-supported.html` *before any content loads*. This
  is a **profile/engine gate, not an anti-bot CAPTCHA**. It is in
  pure-stealth scope (we control UA + JS surface).

### open.spotify.com to a headless/datacenter client [MECH/HYP]

- Public 2026 intel: `open.spotify.com` serves a **minimal JS
  bootstrap shell (~9 KB) that hydrates the web player client-side**;
  non-browser / under-rendered clients see only the stub. Spotify's
  web auth ships **invisible reCAPTCHA v3** (scoreless, no user
  interaction) as standard anti-abuse plumbing on the login surface —
  this is the `.grecaptcha-badge`/`grecaptcha-response` markup in our
  capture, **not** a v2 image/click challenge. (Spotify web-player
  support docs; general SPA-shell behaviour corroborated across
  scraping-vendor write-ups, 2026.)
- No evidence Spotify fronts `open.spotify.com` with PerimeterX/DataDome/
  Akamai/Cloudflare *interstitials* on the landing path: the captured
  cookie jar is **100% Spotify first-party** (`sp_*`), zero vendor
  challenge cookie, and the body carries **none** of our structural
  vendor tokens (`_px*`, `datadome`, `_abck`, `akam/13`,
  `cdn-cgi/challenge-platform`, `_cf_chl_opt`). [HYP] Spotify may
  apply server-side bot scoring on deeper API/playback endpoints, but
  **the landing document we classified is a benign hydration shell**,
  not a served challenge — so for the audited URL the verdict is a
  pure classifier FP, with a secondary thin-shell render-completeness
  note.

---

## STEP 3 — verdict + concrete next action

### open.spotify.com → **(A) classifier false-positive** [CODE]

- **True state:** benign Spotify SPA hydration shell (9.6 KB) with the
  standard invisible-reCAPTCHA-v3 badge; no challenge, no human gate,
  no vendor block, first-party cookies only.
- **Why mis-bucketed:** `("captcha","captcha-CHL")` bare-substring row
  matched `.grecaptcha-badge` / `g-recaptcha-response` in a <30 KB
  body; `verdict_for` then mapped it to `edge-block`.
- **Concrete fix (classifier, surgical):** the bare `captcha` row must
  not fire on the invisible-reCAPTCHA-v3 SDK/badge. Replace the naked
  substring with a co-signal requirement so it only tags a *real*
  interactive captcha shell, not the always-present v3 badge. Concrete
  change in `crates/browser/src/classify.rs` `SMALL_BODY`/`engine_classify`:
  treat a body as `captcha-CHL` **only** when `captcha` co-occurs with
  an *interactive-challenge* token AND **not** purely the v3 badge —
  i.e. require one of `data-sitekey` / `class="g-recaptcha"` *rendered
  visible* / `h-captcha` / `turnstile` / `px-captcha` / a captcha
  `<iframe src>`, **and** suppress when the only captcha evidence is
  `grecaptcha-badge` / `grecaptcha-response` / a `gstatic.com/recaptcha`
  `<script src>` (invisible v3 = no gate). Net effect: Spotify's
  shell → falls through to `L3-RENDERED`; at 9.6 KB (< `THIN_SHELL_MAX_BYTES`
  15 KB) `verdict_for` already classifies it as `ThinShell` (FP-B3),
  i.e. *not a challenge* — a render-completeness datapoint, the
  correct outcome.
- **Regression test to add:** feed the captured spotify shell shape
  (small body whose only captcha evidence is `.grecaptcha-badge` +
  `g-recaptcha-response` + a `gstatic.com/recaptcha` script) →
  assert `tag != "captcha-CHL"` and `verdict.is_challenge() == false`
  (expect `ThinShell`). Mirror the existing
  `fp_b2_literal_strong_markers_size_gated` style.
- **Out of pure-stealth scope?** No. Nothing to bypass — pure
  classifier correction (a Class-B/P0 measurement FP per `99_CODE_FALSE_POSITIVES.md`).

### duolingo.com → **(B) UA / browser-support redirect gate** + (A) [CODE]/[MECH]

- **True state:** the engine is **302-redirected to a static
  `/errors/not-supported.html`** browser-support page. That page
  embeds the site-wide reCAPTCHA-v3 SDK, which the bare `captcha` row
  matched → spurious `captcha-CHL / edge-block`. The captcha tag is a
  **false positive layered on top of a real UA/feature gate**. Not an
  interactive human CAPTCHA; no anti-bot vendor cookie.
- **Two independent fixes, both in scope:**
  1. **Primary (engine/profile) — stop the redirect.** Duolingo's
     support check is rejecting our client's UA / JS-feature surface
     (cf. webcompat: a UA override makes it work; Firefox was blocked
     on UA). **Concrete next action:** capture the *initial* response
     before the redirect (status + `Location` + the gating
     script/UA-sniff) — re-run a focused single-site nav with response
     logging — and diff our `chrome_130_macos` preset's UA string +
     Client Hints + the specific JS feature the page probes (the
     not-supported template typically tests a modern-Chrome API /
     parses the UA major version). Align the preset so the
     support-check passes and Duolingo serves the real app instead of
     redirecting. This is the same profile-alignment class as other
     UA-gate sites; **no captcha solving involved**.
  2. **Secondary (classifier)** — the *same* spotify fix above
     (invisible-v3 suppression) also removes the bogus `captcha-CHL`
     on the error page, so once (1) is diagnosed the verdict reflects
     reality (it would read as a redirect/`not-supported` outcome, not
     a fake captcha — consider also adding a `not-supported.html` /
     "browser version is not supported" → explicit `UA-GATE`/redirect
     verdict so it is never re-confused with a challenge).
- **Out of pure-stealth scope?** No. UA/feature-profile alignment is
  core engine work; the captcha tag is a classifier FP. **No
  third-party captcha service is needed or relevant here.**

### Mis-bucket summary

- **(C) genuine interactive human CAPTCHA: NEITHER site.** No
  evidence of an interactive widget the user must solve; both
  "captcha" hits are invisible reCAPTCHA v3 SDK/badge plumbing. The
  paid captcha-solving-service path is **not** applicable to either.
- **(D) different vendor we mis-bucketed:** effectively yes — neither
  is the implied generic "captcha" vendor. Spotify = first-party SPA
  shell (no vendor). Duolingo = first-party UA/browser-support
  redirect (no anti-bot vendor); the only third-party present is
  Google reCAPTCHA v3 in invisible/scoreless mode, which is not a
  blocking gate.

---

## Concrete next actions (ordered)

1. **[CODE, P0, classifier]** Add the invisible-reCAPTCHA-v3 suppression
   / interactive co-signal requirement to the bare `captcha` row in
   `crates/browser/src/classify.rs` + regression test. Flips Spotify
   from `edge-block` (false) → `ThinShell` (true) and removes the
   bogus captcha tag from Duolingo's error page. Extends the FP-B2/B3
   work; trustworthy-measurement fix, ship as its own gate-checked
   commit.
2. **[MECH, engine, Duolingo]** Single-site focused re-nav with
   pre-redirect response capture to identify the exact UA / JS-feature
   the `/errors/not-supported.html` gate keys off; align the
   `chrome_130_macos` preset so the support-check passes (real app
   served, no redirect). Add a `not-supported`/UA-gate verdict so the
   outcome is never re-mis-labelled a captcha.
3. **[HYP, Spotify, optional]** If deeper Spotify web-player content
   (post-hydration / API) is ever in scope, re-audit those endpoints
   separately — the landing-document audit here only proves the shell
   is benign, not that every Spotify endpoint is open. Out of scope
   for the current "edge-block" question.

## Sources

- Captured artifacts (authoritative for our side):
  `/tmp/audit_failing_sites/spotify.json`,
  `/tmp/audit_failing_sites/duolingo.json` (2026-05-16 live audit).
- `/home/yfedoseev/projects/browser_oxide/crates/browser/src/classify.rs`
  (SMALL_BODY bare-`captcha` row :115; `verdict_for` :124–145).
- `/home/yfedoseev/projects/browser_oxide/docs/research/engines/99_CODE_FALSE_POSITIVES.md`
  (FP-B2 weak literal markers, FP-B3 thin-shell band — the same FP class).
- `/home/yfedoseev/projects/browser_oxide/crates/browser/tests/audit_failing_sites.rs`
  (marker_contexts extraction; harness comment :308–311 anticipating this exact FP).
- Duolingo "Browser not supported" page (direct fetch 2026-05-16):
  <https://www.duolingo.com/errors/not-supported.html>
- webcompat/web-bugs Duolingo UA-gate issues:
  <https://github.com/webcompat/web-bugs/issues/56109>,
  <https://github.com/webcompat/web-bugs/issues/69764>,
  <https://github.com/webcompat/web-bugs/issues/120827>
- Spotify web-player help (SPA/web-player behaviour):
  <https://support.spotify.com/us/article/web-player-help/>
- Headless/UA-redirect detection background (2026):
  <https://www.scraperapi.com/web-scraping/best-user-agent-list-for-web-scraping/>,
  <https://blog.castle.io/how-to-detect-headless-chrome-bots-instrumented-with-playwright/>
- PerimeterX 2026 context (ruled out for the audited Spotify landing —
  no `_px*` cookie/token present, listed for completeness):
  <https://scrapfly.io/blog/posts/how-to-bypass-perimeterx-human-anti-scraping>
