# stealth — Fingerprint Profiles + Anti-Detection (SOTA 2026)

The stealth layer that makes browser_oxide undetectable. Designed against Cloudflare Turnstile, DataDome, Akamai, HUMAN/PerimeterX, and Kasada.

## Why Stealth Is Built-In

browser_oxide has no Chrome, no CDP client, no WebDriver, no ChromeDriver. The detection vectors that plague Chrome automation simply don't exist:

| Vector | Chrome + CDP | browser_oxide |
|---|---|---|
| `Runtime.enable` CDP leak | V8 serializes console args (Proxy trap detection) | No CDP client — we ARE the engine |
| `cdc_*` variables | ChromeDriver injects them | No ChromeDriver |
| CDP WebSocket detection | /json/version endpoint exposed | No debugging port |
| `navigator.webdriver` | Must be patched | Doesn't exist unless we add it |
| Headless mode detection | Missing features in headless Chrome | Not Chrome |
| Puppeteer/Playwright artifacts | `__puppeteer_evaluation_script__` etc. | Not used |

## StealthProfile — The Full Browser Identity

A `StealthProfile` is a coherent set of 100+ browser characteristics that must all be internally consistent:

```rust
pub struct StealthProfile {
    // === Identity ===
    pub browser: BrowserIdentity,     // Chrome 130, Firefox 133, Safari 18
    pub os: OsIdentity,               // Windows 10, macOS 15.2, Ubuntu 24.04
    pub user_agent: String,
    pub user_agent_data: UserAgentData, // sec-ch-ua-* Client Hints

    // === Hardware ===
    pub screen: ScreenProfile,        // width, height, colorDepth, pixelRatio, availWidth/Height
    pub gpu: GpuProfile,              // WebGL vendor, renderer, extensions, max params
    pub cpu_cores: u8,                // navigator.hardwareConcurrency
    pub memory_gb: u8,                // navigator.deviceMemory
    pub max_touch_points: u8,         // 0 for desktop, 5+ for mobile

    // === Locale ===
    pub languages: Vec<String>,       // ["en-US", "en"]
    pub timezone: String,             // "America/New_York"
    pub locale: String,               // "en-US"
    pub intl_config: IntlConfig,      // DateTimeFormat, NumberFormat behavior

    // === Network ===
    pub tls_profile: TlsProfile,      // JA4 config for rquest
    pub http2_profile: Http2Profile,  // SETTINGS, WINDOW_UPDATE, PRIORITY
    pub http3_profile: Http3Profile,  // QUIC transport parameters
    pub connection: ConnectionProfile, // effectiveType, rtt, downlink

    // === Rendering ===
    pub canvas_fonts: Vec<FontProfile>, // Available fonts + rendering config
    pub canvas_hinting: HintingMode,   // Full/light/none (OS-dependent)
    pub canvas_subpixel: SubpixelMode, // LCD/grayscale (OS-dependent)

    // === Fingerprint seeds ===
    pub canvas_seed: u64,             // Deterministic canvas rendering variance
    pub webgl_seed: u64,              // Deterministic WebGL parameter variance
    pub audio_seed: u64,              // Deterministic AudioContext output

    // === APIs ===
    pub plugins: Vec<PluginInfo>,     // navigator.plugins
    pub mime_types: Vec<MimeTypeInfo>,// navigator.mimeTypes
    pub permissions: PermissionSet,   // Notification, geolocation, etc. default states
    pub media_codecs: CodecSupport,   // MediaCapabilities
    pub media_devices: Vec<MediaDeviceInfo>, // Camera, mic, speaker stubs
    pub speech_voices: Vec<SpeechVoice>,    // speechSynthesis.getVoices()
    pub fonts_enumerated: Vec<String>,      // CSS font enumeration results

    // === Media features (CSS @media) ===
    pub prefers_color_scheme: ColorScheme,
    pub prefers_reduced_motion: ReducedMotion,
    pub pointer: PointerType,         // fine (desktop) / coarse (mobile)
    pub hover: HoverCapability,       // hover (desktop) / none (mobile)

    // === Behavioral ===
    pub typing_speed: TypingProfile,  // Inter-key timing distribution
    pub mouse_profile: MouseProfile,  // Velocity curves, precision
}
```

### Profile Consistency Rules (Validated at Construction)

| Rule | Example |
|---|---|
| UA string matches browser + OS | Chrome 130 + Windows 10 → specific UA format |
| sec-ch-ua-* matches UA | Client Hints must agree with UA string |
| GPU matches OS | Apple M2 only with macOS, not Windows |
| TLS fingerprint matches browser version | Chrome 130 has specific JA4 hash |
| HTTP/2 SETTINGS match browser | Chrome sends specific frame values |
| Plugins match browser | Chrome 130: "PDF Viewer", "Chrome PDF Viewer" |
| Touch points match device | Desktop = 0, mobile = 5+ |
| Platform matches OS | Windows → "Win32", macOS → "MacIntel" |
| Fonts match OS | Windows has Segoe UI, macOS has SF Pro |
| Speech voices match OS | Windows has Microsoft voices, macOS has Siri voices |
| Canvas hinting matches OS | Windows: full hinting, macOS: no hinting |
| Media query values match screen | pointer: fine for desktop, coarse for mobile |
| Intl formatting matches locale | Date/number formats must match claimed locale |

## Navigator Properties — Full Surface

Anti-bot scripts check 50+ navigator properties. All must be present and consistent:

```javascript
// === Identity (from profile) ===
navigator.userAgent        → profile.user_agent
navigator.userAgentData    → profile.user_agent_data  // getHighEntropyValues()
navigator.platform         → profile.os.platform_string()
navigator.vendor           → "Google Inc." (Chrome) / "" (Firefox)
navigator.vendorSub        → ""
navigator.productSub       → "20030107" (Chrome/Safari)
navigator.appVersion       → derived from UA
navigator.appCodeName      → "Mozilla"
navigator.appName          → "Netscape"
navigator.product          → "Gecko"

// === Hardware (from profile) ===
navigator.hardwareConcurrency → profile.cpu_cores
navigator.deviceMemory     → profile.memory_gb
navigator.maxTouchPoints   → profile.max_touch_points

// === Locale (from profile) ===
navigator.language         → profile.languages[0]
navigator.languages        → profile.languages

// === State ===
navigator.onLine           → true
navigator.cookieEnabled    → true
navigator.doNotTrack       → null
navigator.pdfViewerEnabled → true (Chrome 130)
navigator.webdriver        → undefined  // NOT false — undefined is what real Chrome returns

// === Plugins (from profile) ===
navigator.plugins          → PluginArray with profile.plugins
navigator.mimeTypes        → MimeTypeArray with profile.mime_types

// === Connection ===
navigator.connection       → { effectiveType: "4g", rtt: 50, downlink: 10, saveData: false }

// === APIs that must EXIST (anti-bot probes for presence) ===
navigator.bluetooth        → object (Chrome)
navigator.usb              → object (Chrome)
navigator.serial           → object (Chrome)
navigator.hid              → object (Chrome)
navigator.keyboard         → object (Chrome)
navigator.locks            → object
navigator.storage          → object
navigator.serviceWorker    → object
navigator.clipboard        → object
navigator.geolocation      → object
navigator.presentation     → object
navigator.wakeLock         → object
navigator.mediaDevices     → object with enumerateDevices()
navigator.credentials      → object
navigator.permissions      → object with query()
navigator.mediaSession     → object
navigator.gpu              → object (WebGPU, Chrome 130+)
navigator.login            → object (FedCM, Chrome)
navigator.ink              → object (Chrome)
navigator.managed          → object (Chrome enterprise)
navigator.scheduling       → object with isInputPending()
navigator.windowControlsOverlay → object (PWA)
navigator.userActivation   → object with isActive, hasBeenActive
```

## Window Properties

```javascript
// === Chrome-specific (MUST exist for Chrome profiles) ===
window.chrome = {
    app: { isInstalled: false, InstallState: {...}, RunningState: {...} },
    runtime: { OnInstalledReason: {...}, PlatformOs: {...}, ... },
    csi: function() { return { startE: ..., onloadT: ..., ... } },
    loadTimes: function() { return { commitLoadTime: ..., ... } },
}

// === Document state (anti-bot checks) ===
document.hasFocus()        → true  // CRITICAL — headless browsers return false
document.hidden            → false
document.visibilityState   → "visible"

// === APIs that must exist ===
window.speechSynthesis     → { getVoices(): [...], speak(), cancel(), ... }
window.Notification        → constructor with .permission = "default"
window.RTCPeerConnection   → constructor (WebRTC)
window.SharedArrayBuffer   → constructor (if cross-origin isolated)
window.isSecureContext     → true (for HTTPS)
window.crossOriginIsolated → false (default)
window.indexedDB           → object
window.caches              → object
window.crypto.subtle       → object (Web Crypto)
window.OffscreenCanvas     → constructor
window.IntersectionObserver → constructor
window.ResizeObserver      → constructor
window.MutationObserver    → constructor
window.requestIdleCallback → function
window.trustedTypes        → object
window.PaymentRequest      → constructor
window.PublicKeyCredential → constructor (WebAuthn)
window.Intl.*              → Full Intl API (V8 provides this)
```

## Performance API

```javascript
// Chrome resolution: 100μs (normal), 5μs (cross-origin isolated)
performance.now()          → monotonic timer with Chrome resolution

// Chrome-specific (anti-bot reads this)
performance.memory         → { jsHeapSizeLimit, totalJSHeapSize, usedJSHeapSize }

// Navigation timing
performance.timing         → PerformanceTiming (deprecated but checked)
performance.getEntriesByType('navigation') → PerformanceNavigationTiming

// Resource timing (anti-bot checks their own script appears here)
performance.getEntriesByType('resource') → PerformanceResourceTiming[]
```

## Prototype Integrity

Anti-bot systems (HUMAN/PerimeterX, DataDome) verify that native functions haven't been replaced:

```javascript
Function.prototype.toString.call(navigator.permissions.query)
// Must return: "function query() { [native code] }"

// iframe isolation test:
// Create iframe, check if API behavior is consistent between main + iframe
// Detects globally overridden prototypes
```

Since browser_oxide implements these APIs natively in Rust (not via JS monkey-patching), `Function.prototype.toString` naturally returns `"[native code]"` for our V8 bindings. This is a fundamental advantage over JS injection approaches.

## Canvas Fingerprint

See [CANVAS.md](CANVAS.md). Real rendering via tiny-skia, not fake hashes. Deterministic per profile.

## Behavioral Emulation

For interactive scraping:

- **Mouse**: Bezier curves with S-curve velocity profiles (matching human motor patterns per Fitts's Law). DataDome specifically detects linear/simple Bezier paths.
- **Keyboard**: Inter-key timing from Gaussian distribution. Faster for common bigrams, slower for uncommon ones.
- **Scroll**: Momentum-based with deceleration (matching touchpad/wheel physics).
- **Timing**: Human-like delays between actions (200-800ms, not 0ms or exact intervals).

## Pre-built Profiles

```rust
// Desktop
StealthProfile::chrome_130_windows()
StealthProfile::chrome_130_macos()
StealthProfile::chrome_130_linux()
StealthProfile::firefox_133_windows()
StealthProfile::safari_18_macos()

// Mobile
StealthProfile::chrome_130_android()
StealthProfile::safari_18_ios()

// Random (statistically realistic)
StealthProfile::random_desktop()
StealthProfile::random_mobile()

// Custom builder
StealthProfile::builder()
    .browser(Browser::Chrome, "130.0.6723.91")
    .os(Os::Windows, "10.0")
    .gpu(Gpu::nvidia_rtx_3080())
    .screen(1920, 1080, 24, 1.0)
    .locale("de-DE", "Europe/Berlin")
    .build()?  // validates ALL consistency rules
```
