use crate::gpu::GpuProfile;
use serde::{Deserialize, Serialize};

/// A media device reported by navigator.mediaDevices.enumerateDevices().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDeviceInfo {
    pub device_id: String,
    pub kind: String,
    pub label: String,
    pub group_id: String,
}

/// A complete stealth fingerprint profile.
///
/// All fields must be internally consistent. Use pre-built profiles
/// from `presets` or build with `StealthProfileBuilder` and call `validate()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StealthProfile {
    // === Identity ===
    pub user_agent: String,
    pub browser_name: String,
    pub browser_version: String,
    pub os_name: String,
    pub os_version: String,
    pub platform: String,
    pub vendor: String,
    pub vendor_sub: String,
    pub product_sub: String,
    pub app_version: String,

    // === Hardware ===
    pub screen_width: u32,
    pub screen_height: u32,
    pub screen_avail_width: u32,
    pub screen_avail_height: u32,
    /// Pixels from the top of the screen that are not available (e.g. macOS menu bar = 25).
    pub screen_avail_top: u32,
    pub screen_color_depth: u32,
    pub device_pixel_ratio: f64,
    pub cpu_cores: u8,
    pub device_memory: u8,
    pub max_touch_points: u8,

    // === GPU / WebGL ===
    pub webgl_vendor: String,
    pub webgl_renderer: String,

    /// Full GPU catalog entry — extension list, getParameter values,
    /// shader precision formats. Drives `canvas_bootstrap.js::
    /// WebGLRenderingContext` so per-profile fingerprint diversity
    /// matches Chrome 131 exactly. Defaults to an NVIDIA RTX 3060
    /// profile if not set, so existing presets don't break.
    #[serde(default = "default_gpu_profile")]
    pub gpu_profile: GpuProfile,

    // === Locale ===
    pub language: String,
    pub languages: Vec<String>,
    pub timezone: String,

    // === Client Hints high-entropy values ===
    //
    // Historically these were template constants inside window_bootstrap.js
    // (architecture="x86", model="", platformVersion derived from os_version).
    // That produced incorrect values on macOS/arm Apple Silicon and offered
    // no way to claim 32-bit / wow64. Now profile-driven so HTTP Sec-CH-UA-*
    // headers and navigator.userAgentData stay consistent. #[serde(default)]
    // keeps old serialized profiles readable.
    #[serde(default = "default_cpu_architecture")]
    pub cpu_architecture: String, // "x86" | "arm"
    #[serde(default = "default_cpu_bitness")]
    pub cpu_bitness: String, // "64" | "32"
    /// Chrome-style zero-padded triple, e.g. "15.0.0". Empty string on
    /// Linux, matching real Chrome behavior.
    #[serde(default)]
    pub platform_version: String,
    /// Device model (empty for desktop).
    #[serde(default)]
    pub ua_model: String,
    /// True iff running 32-bit Chrome on 64-bit Windows. Rare; false for
    /// every desktop preset we ship.
    #[serde(default)]
    pub ua_wow64: bool,

    // === Network ===
    /// rquest Impersonate variant (e.g., "chrome_147")
    pub tls_impersonate: String,
    pub connection_effective_type: String,
    pub connection_rtt: u32,
    pub connection_downlink: f64,

    // === Plugins ===
    pub pdf_viewer_enabled: bool,
    pub plugins_count: u32,
    pub mime_types_count: u32,

    // === Fingerprint seeds ===
    pub canvas_seed: u64,
    pub audio_seed: u64,

    // === WebAuthn / FedCM probe shape ===
    //
    // Anti-bot vendors call PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable()
    // and isConditionalMediationAvailable(). True on modern Mac/Windows desktop (Touch ID,
    // Windows Hello, Passkeys), false on Linux desktop. These drive the JS shim's
    // resolved-promise values; no real authenticator is implemented.
    #[serde(default)]
    pub has_platform_authenticator: bool,
    #[serde(default = "default_true")]
    pub conditional_mediation: bool,

    // === HTTP/3 / QUIC ===
    //
    // Disabled by default (gap #33). `quinn-proto 0.11` emits transport
    // parameters in a *random* shuffled order with a *random* GREASE TP
    // per handshake — distinguishable from Chrome's deterministic ordering.
    // Until we vendor-fork quinn-proto with a Chrome-fixed-order patch
    // (deferred per docs/SOTA_ROADMAP_2026.md §1 / GAPS.md §33),
    // advertising `h3` is a *worse* fingerprint than not speaking it at all.
    //
    // Set to `true` only on profiles where you have a working Chrome-
    // matched QUIC stack (e.g., a forked quinn-proto). Default `false`.
    #[serde(default)]
    pub allow_http3: bool,

    // === Media features ===
    pub prefers_color_scheme: String,
    pub pointer_type: String,
    pub hover_capability: String,

    // === Window dimensions ===
    pub inner_width: u32,
    pub inner_height: u32,
    pub outer_width: u32,
    pub outer_height: u32,

    // === Proxy ===
    #[serde(default)]
    pub proxy: Option<String>,

    // === Media devices ===
    #[serde(default)]
    pub media_devices: Vec<MediaDeviceInfo>,

    /// Enforce Content Security Policy on sub-resource fetches and
    /// `<script src>` loads. When true, browser_oxide will refuse to
    /// fetch URLs that violate the page's CSP — matching real Chrome.
    /// When false (legacy behaviour), we issue every fetch the page
    /// requests, which can be a cross-vendor bot tell because real
    /// Chrome would have blocked some of them.
    /// Defaults via `serde(default)` to `true` so new profiles get the
    /// safer behaviour automatically; preset constructors can flip
    /// it for benchmarking purity. Override at runtime with
    /// `BOXIDE_CSP_BYPASS=1`.
    #[serde(default = "default_true")]
    pub enforce_csp: bool,
}

fn default_true() -> bool {
    true
}

fn default_gpu_profile() -> GpuProfile {
    crate::gpu::nvidia_rtx_3060_windows()
}

fn default_cpu_architecture() -> String {
    "x86".into()
}

fn default_cpu_bitness() -> String {
    "64".into()
}

impl StealthProfile {
    /// Validate that all fields are internally consistent.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // UA must contain the major version. Two formats are valid:
        //   - Chrome's UA-reduction policy (since Chrome 110) freezes the
        //     UA string at `<Major>.0.0.0` while browser_version holds the
        //     full version (e.g. "147.0.7727.117") for use in
        //     sec-ch-ua-full-version-list.
        //   - Firefox publishes the full short version `<Major>.0` in the
        //     UA (e.g. "Firefox/135.0"). It does NOT participate in
        //     sec-ch-ua, so there's no separate full version to track.
        let ua_major: String = self
            .browser_version
            .split('.')
            .next()
            .unwrap_or("")
            .to_string();
        let chrome_form = format!("{}.0.0.0", ua_major);
        let firefox_form = format!("{}.0", ua_major);
        let ua_ok = self.user_agent.contains(&chrome_form)
            || self.user_agent.contains(&firefox_form);
        if !ua_ok {
            errors.push(format!(
                "UA '{}' doesn't contain reduced major version '{}' or '{}'",
                self.user_agent, chrome_form, firefox_form
            ));
        }

        // Platform must match OS
        match self.os_name.as_str() {
            "Windows" if self.platform != "Win32" => {
                errors.push(format!("Windows OS but platform is '{}'", self.platform));
            }
            "macOS" if self.platform != "MacIntel" => {
                errors.push(format!("macOS but platform is '{}'", self.platform));
            }
            "Linux" if !self.platform.starts_with("Linux") => {
                errors.push(format!("Linux OS but platform is '{}'", self.platform));
            }
            _ => {}
        }

        // Touch points: desktop = 0, mobile > 0
        if self.max_touch_points > 0 && self.screen_width > 1024 && self.pointer_type == "fine" {
            errors.push("Touch points > 0 but desktop pointer type".into());
        }

        // GPU vendor must match renderer
        if self.webgl_renderer.contains("NVIDIA") && !self.webgl_vendor.contains("NVIDIA") {
            errors.push("WebGL renderer is NVIDIA but vendor doesn't match".into());
        }
        if self.webgl_renderer.contains("Intel") && !self.webgl_vendor.contains("Intel") {
            errors.push("WebGL renderer is Intel but vendor doesn't match".into());
        }
        if self.webgl_renderer.contains("Apple") && !self.webgl_vendor.contains("Apple") {
            errors.push("WebGL renderer is Apple but vendor doesn't match".into());
        }

        // Apple GPU only on macOS
        if self.webgl_renderer.contains("Apple") && self.os_name != "macOS" {
            errors.push("Apple GPU on non-macOS".into());
        }

        // Screen dimensions sanity
        if self.screen_width == 0 || self.screen_height == 0 {
            errors.push("Screen dimensions cannot be zero".into());
        }
        if self.inner_width > self.screen_width {
            errors.push("inner_width > screen_width".into());
        }
        if self.outer_width < self.inner_width {
            errors.push("outer_width < inner_width".into());
        }

        // CPU/memory sanity
        if self.cpu_cores == 0 || self.cpu_cores > 128 {
            errors.push(format!("Unrealistic cpu_cores: {}", self.cpu_cores));
        }
        if self.device_memory == 0 || self.device_memory > 64 {
            errors.push(format!("Unrealistic device_memory: {}", self.device_memory));
        }

        // Language must be in languages list
        if !self.languages.contains(&self.language) {
            errors.push(format!(
                "language '{}' not in languages {:?}",
                self.language, self.languages
            ));
        }

        // === Client Hints consistency ===
        // Architecture enum
        if !matches!(self.cpu_architecture.as_str(), "x86" | "arm") {
            errors.push(format!(
                "cpu_architecture must be 'x86' or 'arm' (got '{}')",
                self.cpu_architecture
            ));
        }
        // Bitness enum
        if !matches!(self.cpu_bitness.as_str(), "64" | "32") {
            errors.push(format!(
                "cpu_bitness must be '64' or '32' (got '{}')",
                self.cpu_bitness
            ));
        }
        // wow64 only makes sense on 32-bit Windows Chrome
        if self.ua_wow64 && (self.os_name != "Windows" || self.cpu_bitness != "32") {
            errors.push(format!(
                "ua_wow64=true requires os_name=Windows and cpu_bitness=32 (got {} / {})",
                self.os_name, self.cpu_bitness
            ));
        }
        // Chrome on Linux reports empty platform_version per real Chrome.
        if self.os_name == "Linux" && !self.platform_version.is_empty() {
            errors.push(format!(
                "Chrome on Linux must report empty platform_version (got '{}')",
                self.platform_version
            ));
        }
        // Apple Silicon (arm) only on macOS
        if self.cpu_architecture == "arm"
            && !matches!(self.os_name.as_str(), "macOS" | "Android" | "ChromeOS")
        {
            errors.push(format!(
                "cpu_architecture=arm only on macOS/Android/ChromeOS (got '{}')",
                self.os_name
            ));
        }
        // Desktop profiles shouldn't leak a ua_model
        if !self.ua_model.is_empty() && self.max_touch_points == 0 {
            errors.push(format!(
                "ua_model='{}' on a desktop (max_touch_points=0) profile",
                self.ua_model
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
