use deno_core::op2;
use stealth::StealthProfile;

/// Stealth profile stored in OpState. Optional — if absent, defaults apply.
pub struct StealthState {
    pub profile: Option<StealthProfile>,
}

impl StealthState {
    pub fn new(profile: Option<StealthProfile>) -> Self {
        Self { profile }
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
            "screen_color_depth" => p.screen_color_depth.to_string(),
            "device_pixel_ratio" => p.device_pixel_ratio.to_string(),
            "inner_width" => p.inner_width.to_string(),
            "inner_height" => p.inner_height.to_string(),
            "outer_width" => p.outer_width.to_string(),
            "outer_height" => p.outer_height.to_string(),
            "timezone" => p.timezone.clone(),
            "pdf_viewer_enabled" => p.pdf_viewer_enabled.to_string(),
            "plugins_count" => p.plugins_count.to_string(),
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
            _ => String::new(),
        },
        None => String::new(), // No profile = empty = use JS defaults
    }
}

#[op2(fast)]
pub fn op_has_stealth_profile(#[state] state: &StealthState) -> bool {
    state.profile.is_some()
}

deno_core::extension!(
    stealth_extension,
    ops = [op_get_profile_value, op_has_stealth_profile],
);
