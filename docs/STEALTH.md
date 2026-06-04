# Stealth profiles

A `StealthProfile` is the browser identity BrowserOxide presents to a
site: User-Agent, screen geometry, GPU, locale, Client Hints, TLS
ClientHello label, fingerprint seeds, and more. It is a plain serde
struct, so you can use a built-in preset, clone-and-tweak one, or load
a fully custom profile from YAML/JSON at runtime — no recompile.

- **Struct (source of truth):** `crates/stealth/src/profile.rs`
- **Loader:** `crates/stealth/src/config.rs`
- **Worked example file:** `crates/stealth/profiles/chrome_148_macos.yaml`
- **Coherent references:** `crates/stealth/src/presets.rs`

> A profile must be **internally consistent** — every loader validates
> the profile and rejects incoherent combinations (e.g. a macOS UA with
> a `Win32` platform). An incoherent fingerprint is itself a strong bot
> signal, so this is enforced, not advisory. See
> [Consistency rules](#consistency-rules-validate).

## Three ways to get a profile

### 1. Built-in presets (recommended starting point)

The presets are hand-verified coherent identities. Use one directly, or
as a base to tweak.

```rust
use browser::Page;

let profile = stealth::presets::chrome_148_macos();
let page = Page::navigate_stealth("https://example.com", profile).await?;
```

Available constructors (`stealth::presets::*`):

| Constructor | Identity |
|---|---|
| `chrome_148_windows` / `_macos` / `_linux` | Chrome 148 desktop |
| `chrome_148_ru` / `_cn` / `_de` / `_jp` | Chrome 148, regional locale |
| `firefox_135_windows` / `_macos` / `_linux` | Firefox 135 desktop |
| `pixel_9_pro_chrome_148` | Chrome 148 on Android 15 (Pixel 9 Pro) |
| `iphone_15_pro_safari_18` | Safari 18 on iOS 18 (iPhone 15 Pro) |

### 2. Clone a preset and override fields

The simplest way to make a custom profile in code: start from a coherent
preset and change what you need. (There is no separate builder type —
`StealthProfile` is just a struct; mutate its fields directly.)

```rust
let mut profile = stealth::presets::chrome_148_macos();
profile.screen_width = 3840;
profile.screen_height = 2160;
profile.inner_width = 3840;
profile.inner_height = 2076;
profile.outer_width = 3840;
profile.outer_height = 2160;
profile.device_pixel_ratio = 2.0;
profile.timezone = "America/Los_Angeles".into();
profile.language = "en-US".into();
profile.languages = vec!["en-US".into(), "en".into()];

// Always re-validate after editing — catches combinations that no
// longer cohere (e.g. inner_width > screen_width).
profile.validate().expect("profile must stay coherent");

let page = Page::navigate_stealth(url, profile).await?;
```

### 3. Load from a file (YAML or JSON)

Profiles can live as data — ship them, edit them, reload them without a
rebuild. Format is chosen by extension (`.yaml` / `.yml` / `.json`).

```rust
use stealth::StealthProfile;

// load_from_file VALIDATES internally and returns Err on an incoherent
// or malformed profile — you do NOT need a separate validate() call.
let profile = StealthProfile::load_from_file("profiles/my_chrome.yaml")?;
let page = Page::navigate_stealth(url, profile).await?;
```

From an in-memory string:

```rust
let profile = StealthProfile::from_yaml_str(yaml_text)?;   // or from_json_str
```

Dump-and-edit workflow (export a preset, hand-tweak the YAML, reload):

```rust
let yaml = stealth::presets::chrome_148_macos().to_yaml_string()?;
std::fs::write("profiles/base.yaml", yaml)?;
// …edit profiles/base.yaml…
let profile = StealthProfile::load_from_file("profiles/base.yaml")?;
```

`crates/stealth/profiles/chrome_148_macos.yaml` is the canonical worked
example — copy it as your starting point.

## Field reference

Grouped as in `profile.rs`. Fields marked **(optional)** have a serde
default and may be omitted from YAML/JSON; everything else is required.

### Identity

| Field | Type | Notes |
|---|---|---|
| `user_agent` | String | Full UA string. Must contain the reduced major version (see rules). |
| `browser_name` | String | `"Chrome"`, `"Firefox"`, `"Safari"`. |
| `browser_version` | String | Full version, e.g. `"148.0.7778.168"`. Feeds `Sec-CH-UA-Full-Version-List`. |
| `os_name` | String | `"Windows"` / `"macOS"` / `"Linux"` / `"Android"` / etc. |
| `os_version` | String | e.g. `"15.2"`. |
| `platform` | String | `navigator.platform` — must match OS (see rules). |
| `vendor` | String | `navigator.vendor`, e.g. `"Google Inc."` (empty for Firefox). |
| `vendor_sub`, `product_sub` | String | `navigator.vendorSub` / `productSub`. |
| `app_version` | String | `navigator.appVersion` (UA minus the `Mozilla/` prefix). |

### Screen & window

| Field | Type | Notes |
|---|---|---|
| `screen_width`, `screen_height` | u32 | Physical screen, CSS px. |
| `screen_avail_width`, `screen_avail_height` | u32 | Minus OS chrome (taskbar/dock). |
| `screen_avail_top` | u32 | Top inset (e.g. macOS menu bar ≈ 25). |
| `screen_color_depth` | u32 | Usually 24 or 30. |
| `device_pixel_ratio` | f64 | 1.0 standard, 2.0 Retina, 3.0 high-DPI mobile. |
| `inner_width`, `inner_height` | u32 | Viewport. |
| `outer_width`, `outer_height` | u32 | Window incl. browser chrome. |

### Hardware

| Field | Type | Notes |
|---|---|---|
| `cpu_cores` | u8 | `navigator.hardwareConcurrency` (1–128). |
| `device_memory` | u8 | `navigator.deviceMemory` GiB, rounded to {0.25,0.5,1,2,4,8} (1–64). |
| `max_touch_points` | u8 | 0 on desktop, >0 on touch devices. |

### GPU / WebGL

| Field | Type | Notes |
|---|---|---|
| `webgl_vendor` | String | `UNMASKED_VENDOR_WEBGL`. |
| `webgl_renderer` | String | `UNMASKED_RENDERER_WEBGL` — must match vendor (see rules). |
| `gpu_profile` | `GpuProfile` | **(optional)** Full GPU catalog entry (extensions, getParameter values, shader precision). Defaults to an NVIDIA RTX 3060 profile. See `crates/stealth/src/gpu.rs` for catalog entries. |

### Locale

| Field | Type | Notes |
|---|---|---|
| `language` | String | Primary, e.g. `"en-US"`. Must appear in `languages`. |
| `languages` | Vec<String> | `navigator.languages`. |
| `timezone` | String | IANA name, e.g. `"America/New_York"`. |

### Client Hints (high-entropy)

| Field | Type | Notes |
|---|---|---|
| `cpu_architecture` | String | **(opt, default `"x86"`)** `"x86"` or `"arm"`. `arm` only on macOS/Android/ChromeOS. |
| `cpu_bitness` | String | **(opt, default `"64"`)** `"64"` or `"32"`. |
| `platform_version` | String | **(opt)** Zero-padded triple, e.g. `"15.0.0"`. Must be empty on Linux Chrome. |
| `ua_model` | String | **(opt)** Device model (empty on desktop). |
| `ua_wow64` | bool | **(opt)** Only valid with Windows + `cpu_bitness="32"`. |
| `device_class` | enum | **(opt, default `Desktop`)** `Desktop` / `MobileAndroid` / `MobileIOS`. Drives TLS curve set, `Sec-CH-UA-Mobile`, form-factors. |

### Network

| Field | Type | Notes |
|---|---|---|
| `tls_impersonate` | String | boring2 ClientHello codename, e.g. `"chrome_147"`. **Not** a display version — internal label, not on the wire. |
| `connection_effective_type` | String | `"4g"` etc. |
| `connection_rtt`, `connection_downlink` | u32 / f64 | `navigator.connection`. |
| `allow_http3` | bool | **(opt, default false)** Leave false unless you have a Chrome-matched QUIC stack — vanilla `quinn-proto` emits a worse fingerprint than no h3. |
| `proxy` | Option<String> | **(opt)** Proxy URL; `BROWSER_OXIDE_PROXY` env overrides. |

### Plugins / media / fingerprint

| Field | Type | Notes |
|---|---|---|
| `pdf_viewer_enabled` | bool | `navigator.pdfViewerEnabled`. |
| `plugins_count`, `mime_types_count` | u32 | Reported plugin/MIME counts. |
| `canvas_seed`, `audio_seed` | u64 | Deterministic per-profile FP noise seeds. |
| `prefers_color_scheme` | String | `"light"` / `"dark"`. |
| `pointer_type` | String | `"fine"` (mouse) / `"coarse"` (touch). |
| `hover_capability` | String | `"hover"` / `"none"`. |
| `color_gamut` | String | **(opt, default `"srgb"`)** `"p3"` on macOS/iPhone; mismatch vs UA is a FingerprintJS probe. |
| `media_devices` | Vec<MediaDeviceInfo> | **(opt)** Each has `device_id`, `kind`, `label`, `group_id`. |
| `has_platform_authenticator` | bool | **(opt)** WebAuthn UVPA available (true on modern Mac/Windows desktop). |
| `conditional_mediation` | bool | **(opt, default true)** WebAuthn conditional-mediation available. |

### Engine behaviour

| Field | Type | Notes |
|---|---|---|
| `enforce_csp` | bool | **(opt, default true)** Refuse sub-resource fetches that violate the page CSP (matches real Chrome). `BROWSER_OXIDE_CSP_BYPASS=1` overrides at runtime. |

## Consistency rules (`validate()`)

Every loader (`load_from_file`, `from_yaml_str`, `from_json_str`) runs
these and returns `Err(ConfigError::Invalid(Vec<String>))` listing every
violation. After editing a profile in code, call `validate()` yourself.

1. **UA contains the reduced major version** — `user_agent` must contain
   `"<major>.0.0.0"` (Chrome UA-reduction form) or `"<major>.0"`
   (Firefox form), where `<major>` is the first segment of
   `browser_version`.
2. **Platform matches OS** — `Windows`→`Win32`, `macOS`→`MacIntel`,
   `Linux`→platform starting with `Linux`.
3. **Touch vs pointer** — `max_touch_points > 0` with a desktop-sized
   screen (`> 1024` wide) and `pointer_type == "fine"` is rejected.
4. **GPU vendor matches renderer** — NVIDIA/Intel/Apple renderer requires
   the matching vendor string; an Apple GPU is only valid on macOS.
5. **Screen sanity** — non-zero dimensions; `inner_width ≤ screen_width`;
   `outer_width ≥ inner_width`.
6. **CPU/memory sanity** — `cpu_cores` in 1–128, `device_memory` in 1–64.
7. **Language in list** — `language` must appear in `languages`.
8. **Client Hints** — `cpu_architecture` ∈ {`x86`,`arm`};
   `cpu_bitness` ∈ {`64`,`32`}; `ua_wow64` only with Windows + 32-bit;
   Linux Chrome must report empty `platform_version`; `arm` only on
   macOS/Android/ChromeOS; a non-empty `ua_model` requires a touch
   device (`max_touch_points > 0`).

When in doubt, start from the preset closest to your target identity and
change one field at a time, re-running `validate()` — the presets in
`presets.rs` are the reference for every coherent combination.

## Minimal custom YAML

A trimmed profile (omitted optional fields fall back to serde defaults).
See `crates/stealth/profiles/chrome_148_macos.yaml` for the full example.

```yaml
# Chrome 148 on Windows 11, 1080p, en-GB
user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"
browser_name: "Chrome"
browser_version: "148.0.7778.168"
os_name: "Windows"
os_version: "10.0.0"
platform: "Win32"
vendor: "Google Inc."
vendor_sub: ""
product_sub: "20030107"
app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"

screen_width: 1920
screen_height: 1080
screen_avail_width: 1920
screen_avail_height: 1032
screen_avail_top: 0
screen_color_depth: 24
device_pixel_ratio: 1.0
cpu_cores: 16
device_memory: 8
max_touch_points: 0

webgl_vendor: "Google Inc. (NVIDIA)"
webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)"

language: "en-GB"
languages: ["en-GB", "en"]
timezone: "Europe/London"

cpu_architecture: "x86"
cpu_bitness: "64"
platform_version: "15.0.0"

tls_impersonate: "chrome_147"
connection_effective_type: "4g"
connection_rtt: 50
connection_downlink: 10.0

pdf_viewer_enabled: true
plugins_count: 5
mime_types_count: 2
canvas_seed: 123456789
audio_seed: 987654321

prefers_color_scheme: "light"
pointer_type: "fine"
hover_capability: "hover"

inner_width: 1920
inner_height: 953
outer_width: 1920
outer_height: 1032
```

## Error handling

`ConfigError` variants (`crates/stealth/src/config.rs`):

- `Io` — file read failed.
- `UnknownFormat` — extension wasn't `.yaml`/`.yml`/`.json`.
- `Yaml` / `Json` — parse error.
- `Invalid(Vec<String>)` — parsed fine but failed `validate()`; the vec
  lists every consistency violation so you can fix them in one pass.
