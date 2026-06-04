# Guide: browser profiles (identities)

A **profile** (`stealth::StealthProfile`) is the browser identity the engine
presents — coherently, across every layer: TLS ClientHello + HTTP/2 fingerprint,
request headers + Client Hints, `navigator`/`window` properties, screen/GPU,
canvas & audio fingerprint seeds, timezone & locale. You pick one per navigation.

> Coherence is the whole point: a profile sets *all* of these together so they
> agree (a Firefox UA ships a Firefox TLS handshake, an iOS profile ships
> Safari headers + Apple GPU strings, etc.). Mixing layers yourself is the
> classic detection tell — let the preset keep them consistent.

## Built-in presets

```rust
use stealth::presets;

// Desktop — Chrome 148 (primary; current stable as of mid-2026)
presets::chrome_148_macos();    presets::chrome_148_windows();   presets::chrome_148_linux();
presets::chrome_148_ru();  presets::chrome_148_cn();  presets::chrome_148_de();  presets::chrome_148_jp();

// Desktop — Firefox 135 (real NSS ClientHello, not Chrome-on-wire)
presets::firefox_135_macos();   presets::firefox_135_windows();  presets::firefox_135_linux();

// Mobile
presets::pixel_9_pro_chrome_148();      // Android + Chrome
presets::iphone_15_pro_safari_18();     // iOS + Safari

// Helpers
presets::with_locale(profile, "de-DE"); // override language/timezone coherently
presets::random_desktop();              // a random desktop preset
```

## Loading a custom profile

```rust
let profile = stealth::StealthProfile::load_from_file("my_profile.yaml")?; // .yaml or .json
profile.validate()?;   // catches incoherent field combinations
```

See `crates/stealth/profiles/chrome_148_macos.yaml` for the full schema, and
[../STEALTH.md](../STEALTH.md) for the per-field reference and consistency rules.

## Routing across profiles

Anti-bot tenants react differently to different identities — a site that walls
Chrome may wave through Firefox or a mobile profile, and vice-versa. Real
pipelines try the best profile per domain. In the 126-site benchmark this
"routed" strategy passes **118/126** vs ~111–118 for any single profile (see
[../BENCHMARK.md](../BENCHMARK.md)).

A minimal router:

```rust
for make in [presets::chrome_148_macos, presets::iphone_15_pro_safari_18,
             presets::firefox_135_macos] {
    let mut page = Page::navigate(url, make(), 5).await?;
    if page.challenge_verdict() == browser::ChallengeVerdict::Pass {
        return Ok(page);                // first profile that renders real content
    }
}
```

Real example: `adidas.com` renders 1.5 MB on `chrome_148_macos` and 1.38 MB on
`iphone_15_pro_safari_18`, but stays at a 2.5 KB interstitial on Firefox/Pixel —
routing wins it.

## Notes

- One `PagePool` reuses a single isolate's profile; if you need multiple profiles
  concurrently, run **one pool per profile**.
- Space out same-IP, same-vendor requests — bursts trip token-clustering
  heuristics and produce false failures.
