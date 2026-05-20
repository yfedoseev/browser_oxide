//! YAML/JSON configuration loader for `StealthProfile`.
//!
//! Profiles are normally produced by the `presets::*` constructors, which
//! are the authoritative source for the values we ship. This module lets a
//! caller load a profile from a YAML or JSON file at runtime — useful for
//! integrators that want to ship the engine binary and a separate
//! `profile.yaml` describing the desired browser identity (UA, screen,
//! locale, GPU, etc.) without rebuilding.
//!
//! ```no_run
//! use stealth::StealthProfile;
//!
//! let profile = StealthProfile::load_from_file("profiles/chrome_148_macos.yaml")
//!     .expect("profile loads");
//! profile.validate().expect("internally consistent");
//! ```

use crate::profile::StealthProfile;
use std::fs;
use std::path::Path;

/// Errors returned by the config loader.
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Yaml(serde_yaml_ng::Error),
    Json(serde_json::Error),
    UnknownFormat(String),
    Invalid(Vec<String>),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "io error: {e}"),
            ConfigError::Yaml(e) => write!(f, "yaml parse error: {e}"),
            ConfigError::Json(e) => write!(f, "json parse error: {e}"),
            ConfigError::UnknownFormat(ext) => {
                write!(f, "unknown config format '{ext}' (expected .yaml/.yml/.json)")
            }
            ConfigError::Invalid(errs) => {
                write!(f, "profile failed validation: {}", errs.join("; "))
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Config file format. Selected by file extension when reading from disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Yaml,
    Json,
}

impl ConfigFormat {
    /// Pick a format from a path extension. `.yaml`/`.yml` → Yaml,
    /// `.json` → Json. Anything else returns `UnknownFormat`.
    pub fn from_path(path: &Path) -> Result<Self, ConfigError> {
        match path.extension().and_then(|s| s.to_str()) {
            Some("yaml") | Some("yml") => Ok(ConfigFormat::Yaml),
            Some("json") => Ok(ConfigFormat::Json),
            other => Err(ConfigError::UnknownFormat(other.unwrap_or("").into())),
        }
    }
}

impl StealthProfile {
    /// Parse a profile from a YAML string and validate it.
    pub fn from_yaml_str(s: &str) -> Result<Self, ConfigError> {
        let profile: StealthProfile = serde_yaml_ng::from_str(s).map_err(ConfigError::Yaml)?;
        profile.validate().map_err(ConfigError::Invalid)?;
        Ok(profile)
    }

    /// Parse a profile from a JSON string and validate it.
    pub fn from_json_str(s: &str) -> Result<Self, ConfigError> {
        let profile: StealthProfile = serde_json::from_str(s).map_err(ConfigError::Json)?;
        profile.validate().map_err(ConfigError::Invalid)?;
        Ok(profile)
    }

    /// Load a profile from a `.yaml`/`.yml`/`.json` file on disk.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let format = ConfigFormat::from_path(path)?;
        let text = fs::read_to_string(path).map_err(ConfigError::Io)?;
        match format {
            ConfigFormat::Yaml => Self::from_yaml_str(&text),
            ConfigFormat::Json => Self::from_json_str(&text),
        }
    }

    /// Serialise this profile to YAML. Pairs with `from_yaml_str` for
    /// dump-and-edit workflows.
    pub fn to_yaml_string(&self) -> Result<String, ConfigError> {
        serde_yaml_ng::to_string(self).map_err(ConfigError::Yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[test]
    fn round_trip_yaml() {
        let original = presets::chrome_148_macos();
        let yaml = original.to_yaml_string().expect("serialise to yaml");
        let parsed = StealthProfile::from_yaml_str(&yaml).expect("parse back");
        assert_eq!(parsed.user_agent, original.user_agent);
        assert_eq!(parsed.browser_version, original.browser_version);
        assert_eq!(parsed.screen_width, original.screen_width);
        assert_eq!(parsed.tls_impersonate, original.tls_impersonate);
    }

    #[test]
    fn round_trip_json_matches_yaml() {
        let original = presets::chrome_148_windows();
        let json = serde_json::to_string(&original).expect("serialise json");
        let parsed = StealthProfile::from_json_str(&json).expect("parse json");
        assert_eq!(parsed.user_agent, original.user_agent);
    }

    #[test]
    fn format_detection_by_extension() {
        assert_eq!(
            ConfigFormat::from_path(Path::new("p.yaml")).unwrap(),
            ConfigFormat::Yaml
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("p.yml")).unwrap(),
            ConfigFormat::Yaml
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("p.json")).unwrap(),
            ConfigFormat::Json
        );
        assert!(ConfigFormat::from_path(Path::new("p.txt")).is_err());
    }

    #[test]
    fn shipped_example_profile_loads() {
        // The chrome_148_macos.yaml shipped under crates/stealth/profiles/
        // is the example users edit. If a serde rename or required-field
        // addition breaks the example, this test catches it before users do.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("profiles")
            .join("chrome_148_macos.yaml");
        let profile = StealthProfile::load_from_file(&path)
            .unwrap_or_else(|e| panic!("load {:?}: {e}", path));
        assert_eq!(profile.browser_name, "Chrome");
        assert_eq!(profile.browser_version, "148.0.7778.168");
        assert_eq!(profile.os_name, "macOS");
        assert!(profile.user_agent.contains("Chrome/148.0.0.0"));
    }

    #[test]
    fn unknown_field_rejected() {
        // We do not set `deny_unknown_fields`, so extra keys are tolerated —
        // assert that current behavior, so a future change is a conscious one.
        let yaml = r#"
user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"
browser_name: Chrome
browser_version: "148.0.7778.168"
os_name: macOS
os_version: "15.2"
platform: MacIntel
vendor: "Google Inc."
vendor_sub: ""
product_sub: "20030107"
app_version: "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"
screen_width: 2560
screen_height: 1440
screen_avail_width: 2560
screen_avail_height: 1415
screen_avail_top: 25
screen_color_depth: 30
device_pixel_ratio: 2.0
cpu_cores: 10
device_memory: 16
max_touch_points: 0
webgl_vendor: "Google Inc. (Apple)"
webgl_renderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M2 Pro, Unspecified Version)"
language: "en-US"
languages: ["en-US", "en"]
timezone: "America/New_York"
tls_impersonate: "chrome_147"
connection_effective_type: "4g"
connection_rtt: 50
connection_downlink: 10.0
pdf_viewer_enabled: true
plugins_count: 5
mime_types_count: 2
canvas_seed: 1311768467294899695
audio_seed: 18364758544493064001
prefers_color_scheme: "light"
pointer_type: "fine"
hover_capability: "hover"
inner_width: 2560
inner_height: 1330
outer_width: 2560
outer_height: 1440
"#;
        let p = StealthProfile::from_yaml_str(yaml).expect("loads");
        assert!(p.user_agent.contains("148.0.0.0"));
        assert_eq!(p.os_name, "macOS");
    }
}
