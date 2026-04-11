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

    // === Network ===
    /// rquest Impersonate variant (e.g., "chrome_130")
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
}

fn default_gpu_profile() -> GpuProfile {
    crate::gpu::nvidia_rtx_3060_windows()
}

impl StealthProfile {
    /// Validate that all fields are internally consistent.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // UA must contain browser name and version
        if !self.user_agent.contains(&self.browser_version) {
            errors.push(format!(
                "UA '{}' doesn't contain browser version '{}'",
                self.user_agent, self.browser_version
            ));
        }

        // Platform must match OS
        match self.os_name.as_str() {
            "Windows" => {
                if self.platform != "Win32" {
                    errors.push(format!("Windows OS but platform is '{}'", self.platform));
                }
            }
            "macOS" => {
                if self.platform != "MacIntel" {
                    errors.push(format!("macOS but platform is '{}'", self.platform));
                }
            }
            "Linux" => {
                if !self.platform.starts_with("Linux") {
                    errors.push(format!("Linux OS but platform is '{}'", self.platform));
                }
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

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
