use deno_core::op2;
use stealth::StealthProfile;

/// Stealth profile stored in OpState. Optional — if absent, defaults apply.
///
/// `cross_origin_isolated` is a per-document flag derived from response
/// headers (COOP=same-origin AND COEP=require-corp|credentialless — see
/// `net::headers::is_cross_origin_isolated`). It drives `self.crossOriginIsolated`
/// and gates SAB postMessage transfer to workers (gap #30).
///
/// `is_secure_context` is a per-document flag derived from the document
/// URL scheme (https/wss/file or http://localhost — see
/// `crate::is_secure_url`). It drives `self.isSecureContext` and gates
/// the ~18 secure-context-only Web Platform APIs (mediaDevices,
/// serviceWorker, clipboard, credentials, usb, bluetooth, etc.) per
/// the IDL `[SecureContext]` extended attribute. Phase 7 fix.
pub struct StealthState {
    pub profile: Option<StealthProfile>,
    pub cross_origin_isolated: bool,
    pub is_secure_context: bool,
}

impl StealthState {
    pub fn new(profile: Option<StealthProfile>) -> Self {
        Self {
            profile,
            cross_origin_isolated: false,
            is_secure_context: false,
        }
    }
    pub fn new_with_coi(profile: Option<StealthProfile>, cross_origin_isolated: bool) -> Self {
        Self {
            profile,
            cross_origin_isolated,
            is_secure_context: false,
        }
    }
    pub fn new_with_flags(
        profile: Option<StealthProfile>,
        cross_origin_isolated: bool,
        is_secure_context: bool,
    ) -> Self {
        Self {
            profile,
            cross_origin_isolated,
            is_secure_context,
        }
    }
}

#[op2]
#[string]
pub fn op_get_profile_value(#[state] state: &StealthState, #[string] key: &str) -> String {
    match &state.profile {
        Some(p) => match key {
            "user_agent" => p.user_agent.clone(),
            "platform" => p.platform.clone(),
            "vendor" => p.vendor.clone(),
            "vendor_sub" => p.vendor_sub.clone(),
            "product_sub" => p.product_sub.clone(),
            "app_version" => p.app_version.clone(),
            "language" => p.language.clone(),
            "languages" => serde_json::to_string(&p.languages).unwrap_or_default(),
            "hardware_concurrency" => p.cpu_cores.to_string(),
            "device_memory" => p.device_memory.to_string(),
            "max_touch_points" => p.max_touch_points.to_string(),
            "screen_width" => p.screen_width.to_string(),
            "screen_height" => p.screen_height.to_string(),
            "screen_avail_width" => p.screen_avail_width.to_string(),
            "screen_avail_height" => p.screen_avail_height.to_string(),
            "screen_avail_top" => p.screen_avail_top.to_string(),
            "screen_color_depth" => p.screen_color_depth.to_string(),
            "device_pixel_ratio" => p.device_pixel_ratio.to_string(),
            "inner_width" => p.inner_width.to_string(),
            "inner_height" => p.inner_height.to_string(),
            "outer_width" => p.outer_width.to_string(),
            "outer_height" => p.outer_height.to_string(),
            "timezone" => p.timezone.clone(),
            "pdf_viewer_enabled" => p.pdf_viewer_enabled.to_string(),
            "plugins_count" => p.plugins_count.to_string(),
            "mime_types_count" => p.mime_types_count.to_string(),
            "connection_effective_type" => p.connection_effective_type.clone(),
            "connection_rtt" => p.connection_rtt.to_string(),
            "connection_downlink" => p.connection_downlink.to_string(),
            "prefers_color_scheme" => p.prefers_color_scheme.clone(),
            "pointer_type" => p.pointer_type.clone(),
            "hover_capability" => p.hover_capability.clone(),
            "webgl_vendor" => p.webgl_vendor.clone(),
            "webgl_renderer" => p.webgl_renderer.clone(),
            "webgl_unmasked_vendor" => p.gpu_profile.unmasked_vendor.clone(),
            "webgl_unmasked_renderer" => p.gpu_profile.unmasked_renderer.clone(),
            "webgl_version" => p.gpu_profile.version.clone(),
            "webgl_shading_language_version" => p.gpu_profile.shading_language_version.clone(),
            "webgl_extensions" => {
                serde_json::to_string(&p.gpu_profile.extensions).unwrap_or_default()
            }
            "webgl_params" => serde_json::to_string(&p.gpu_profile.params).unwrap_or_default(),
            "webgl_shader_precision" => {
                serde_json::to_string(&p.gpu_profile.shader_precision).unwrap_or_default()
            }
            "browser_version" => p.browser_version.clone(),
            "browser_name" => p.browser_name.clone(),
            "os_name" => p.os_name.clone(),
            "os_version" => p.os_version.clone(),
            "media_devices" => serde_json::to_string(&p.media_devices).unwrap_or_default(),
            "audio_seed" => p.audio_seed.to_string(),
            "canvas_seed" => p.canvas_seed.to_string(),
            "cpu_architecture" => p.cpu_architecture.clone(),
            "cpu_bitness" => p.cpu_bitness.clone(),
            "platform_version" => p.platform_version.clone(),
            "ua_model" => p.ua_model.clone(),
            "ua_wow64" => p.ua_wow64.to_string(),
            "has_platform_authenticator" => p.has_platform_authenticator.to_string(),
            "conditional_mediation" => p.conditional_mediation.to_string(),
            "device_class" => match p.device_class {
                stealth::DeviceClass::Desktop => "Desktop",
                stealth::DeviceClass::MobileAndroid => "MobileAndroid",
                stealth::DeviceClass::MobileIOS => "MobileIOS",
            }
            .to_string(),
            _ => String::new(),
        },
        None => String::new(), // No profile = empty = use JS defaults
    }
}

#[op2(fast)]
pub fn op_has_stealth_profile(#[state] state: &StealthState) -> bool {
    state.profile.is_some()
}

/// Returns whether the document is cross-origin-isolated. Drives
/// `self.crossOriginIsolated` and gates SAB transfer (gap #30).
#[op2(fast)]
pub fn op_cross_origin_isolated(#[state] state: &StealthState) -> bool {
    state.cross_origin_isolated
}

/// Returns whether the document is in a secure context. Drives
/// `self.isSecureContext` and gates the ~18 secure-context-only
/// Web Platform APIs per the IDL `[SecureContext]` extended attribute.
/// Phase 7 fix — see `docs/PHASE7_AB_PROBE_FINDINGS_2026_04_29.md`.
#[op2(fast)]
pub fn op_is_secure_context(#[state] state: &StealthState) -> bool {
    state.is_secure_context
}

/// Generate a humanlike sigma-lognormal mouse trajectory from
/// `(from_x, from_y)` to `(to_x, to_y)` with target width `target_w`
/// (drives Fitts's-Law movement time). Returns a JSON array of
/// `{t_ms, x, y}` points sampled at ~125 Hz (8 ms cadence).
///
/// Used by `humanize.js` to pre-populate the synthetic mouse event
/// buffer (`__akamai_events.mouse`) with statistically correct
/// trajectories before any anti-bot script reads it. The Rust
/// implementation in `crates/stealth/src/behavior.rs` is a real
/// Plamondon Sigma-Lognormal generator (2-7 strokes, per-stroke σ/μ
/// from BeCAPTCHA-Mouse-validated distributions, pink-tremor noise)
/// — strictly stronger than the JS-side triangular approximation it
/// replaces. Defeats the RF mouse classifier used downstream by
/// HUMAN/PerimeterX, Kasada (sensor VM), DataDome (slider scorer),
/// and Akamai (sensor_data field 65).
#[op2]
#[string]
pub fn op_behavior_mouse_trajectory(
    from_x: f32,
    from_y: f32,
    to_x: f32,
    to_y: f32,
    target_w: f32,
) -> String {
    let profile = stealth::behavior::BehaviorProfile::default();
    let points =
        stealth::behavior::mouse_trajectory((from_x, from_y), (to_x, to_y), target_w, &profile);
    serde_json::to_string(&points).unwrap_or_else(|_| "[]".to_string())
}

deno_core::extension!(
    stealth_extension,
    ops = [
        op_get_profile_value,
        op_has_stealth_profile,
        op_cross_origin_isolated,
        op_is_secure_context,
        op_behavior_mouse_trajectory,
    ],
);
