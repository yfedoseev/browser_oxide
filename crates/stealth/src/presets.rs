use crate::profile::{MediaDeviceInfo, StealthProfile};

fn default_media_devices(seed: &str) -> Vec<MediaDeviceInfo> {
    // Deterministic device IDs based on a seed string
    let hash = |s: &str| -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        format!(
            "{:016x}{:016x}",
            h.finish(),
            h.finish().wrapping_mul(0x9e3779b97f4a7c15)
        )
    };
    vec![
        MediaDeviceInfo {
            device_id: hash(&format!("{}-audio-in", seed)),
            kind: "audioinput".into(),
            label: "Default".into(),
            group_id: hash(&format!("{}-group-a", seed)),
        },
        MediaDeviceInfo {
            device_id: hash(&format!("{}-audio-out", seed)),
            kind: "audiooutput".into(),
            label: "Default".into(),
            group_id: hash(&format!("{}-group-a", seed)),
        },
        MediaDeviceInfo {
            device_id: hash(&format!("{}-video-in", seed)),
            kind: "videoinput".into(),
            label: "Integrated Camera".into(),
            group_id: hash(&format!("{}-group-v", seed)),
        },
    ]
}

/// Chrome 130 on Windows 10.
pub fn chrome_130_windows() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "147.0.7727.117".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),

        screen_width: 1920,
        screen_height: 1080,
        screen_avail_width: 1920,
        screen_avail_height: 1040,
        screen_avail_top: 0,
        screen_color_depth: 24,
        device_pixel_ratio: 1.0,
        cpu_cores: 8,
        device_memory: 8,
        max_touch_points: 0,

        webgl_vendor: "Google Inc. (NVIDIA)".into(),
        webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/New_York".into(),

        cpu_architecture: "x86".into(),
        cpu_bitness: "64".into(),
        platform_version: "15.0.0".into(),
        ua_model: "".into(),
        ua_wow64: false,

        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1234567890abcdef,
        audio_seed: 0xfedcba0987654321,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(),
        hover_capability: "hover".into(),

        inner_width: 1920,
        inner_height: 969,
        outer_width: 1920,
        outer_height: 1080,

        proxy: None,
        media_devices: default_media_devices("win10"),
        gpu_profile: crate::gpu::nvidia_rtx_3060_windows(),
    }
}

/// Chrome 147 on macOS 15. (UA bumped 2026-04-27 — anti-bot vendors flag old Chrome
/// versions. Real Chrome shipped 147 in mid-Apr 2026 per playwright's bundled chromium.
/// TLS impersonation is still chrome_130 — verified byte-identical to Chrome 147 via
/// tls.peet.ws JA4/Akamai-FP comparison (chrome_130 BoringSSL config tracks current).
///
/// **CRITICAL**: navigator.userAgent reports `Chrome/147.0.0.0` (FROZEN minor versions
/// per Chrome's User-Agent reduction since March 2023 / Chrome 110+). The full version
/// `147.0.7727.117` is ONLY exposed via sec-ch-ua-full-version-list. Sending the full
/// version in the UA string is a 100% reliable bot signal — verified 2026-04-27 by
/// comparing httpbin.org/headers from playwright vs our pipeline.)
pub fn chrome_130_macos() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        // browser_version stores the FULL version — used by sec-ch-ua-full-version-list
        // and by build_sec_ch_ua's major-version split. The UA string above uses
        // the reduced 147.0.0.0 form per Chrome's UA-reduction policy.
        browser_version: "147.0.7727.117".into(),
        os_name: "macOS".into(),
        os_version: "15.2".into(),
        platform: "MacIntel".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),

        screen_width: 1440,
        screen_height: 900,
        screen_avail_width: 1440,
        screen_avail_height: 875,
        screen_avail_top: 25,
        screen_color_depth: 30,
        device_pixel_ratio: 2.0,
        cpu_cores: 10,
        device_memory: 16,
        max_touch_points: 0,

        webgl_vendor: "Google Inc. (Apple)".into(),
        webgl_renderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/Los_Angeles".into(),

        cpu_architecture: "arm".into(),
        cpu_bitness: "64".into(),
        platform_version: "15.2.0".into(),
        ua_model: "".into(),
        ua_wow64: false,

        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0xabcdef1234567890,
        audio_seed: 0x0987654321fedcba,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(),
        hover_capability: "hover".into(),

        inner_width: 1440,
        inner_height: 789,
        outer_width: 1440,
        outer_height: 900,

        proxy: None,
        media_devices: default_media_devices("macos"),
        gpu_profile: crate::gpu::apple_m2_pro_macos(),
    }
}

/// Chrome 130 on Linux.
pub fn chrome_130_linux() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "147.0.7727.117".into(),
        os_name: "Linux".into(),
        os_version: "6.1".into(),
        platform: "Linux x86_64".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),

        screen_width: 1920,
        screen_height: 1080,
        screen_avail_width: 1920,
        screen_avail_height: 1053,
        screen_avail_top: 0,
        screen_color_depth: 24,
        device_pixel_ratio: 1.0,
        cpu_cores: 8,
        device_memory: 8,
        max_touch_points: 0,

        webgl_vendor: "Google Inc. (Intel)".into(),
        webgl_renderer: "ANGLE (Intel, Mesa Intel(R) UHD Graphics 630 (CFL GT2), OpenGL 4.6)".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/Chicago".into(),

        cpu_architecture: "x86".into(),
        cpu_bitness: "64".into(),
        platform_version: "".into(),
        ua_model: "".into(),
        ua_wow64: false,

        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1111222233334444,
        audio_seed: 0x5555666677778888,

        // Linux desktop has no platform authenticator (no Touch ID / Windows Hello).
        has_platform_authenticator: false,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(),
        hover_capability: "hover".into(),

        inner_width: 1920,
        inner_height: 969,
        outer_width: 1920,
        outer_height: 1080,

        proxy: None,
        media_devices: default_media_devices("linux"),
        gpu_profile: crate::gpu::intel_uhd_630_linux(),
    }
}

/// Chrome 130 on Windows — Russian locale (Moscow).
pub fn chrome_130_ru() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "147.0.7727.117".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),
        screen_width: 1920, screen_height: 1080,
        screen_avail_width: 1920, screen_avail_height: 1040,
        screen_avail_top: 0,
        screen_color_depth: 24, device_pixel_ratio: 1.0,
        cpu_cores: 8, device_memory: 8, max_touch_points: 0,
        webgl_vendor: "Google Inc. (NVIDIA)".into(),
        webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
        language: "ru-RU".into(),
        languages: vec!["ru-RU".into(), "ru".into(), "en-US".into(), "en".into()],
        timezone: "Europe/Moscow".into(),
        cpu_architecture: "x86".into(),
        cpu_bitness: "64".into(),
        platform_version: "15.0.0".into(),
        ua_model: "".into(),
        ua_wow64: false,
        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 100, connection_downlink: 8.0,
        pdf_viewer_enabled: true, plugins_count: 5, mime_types_count: 2,
        canvas_seed: 0xaaaa_bbbb_cccc_dddd, audio_seed: 0xdddd_cccc_bbbb_aaaa,
        has_platform_authenticator: true, conditional_mediation: true, allow_http3: false,
        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(), hover_capability: "hover".into(),
        inner_width: 1920, inner_height: 969,
        outer_width: 1920, outer_height: 1080,
        proxy: None,
        media_devices: default_media_devices("ru"),
        gpu_profile: crate::gpu::nvidia_rtx_3060_windows(),
    }
}

/// Chrome 130 on Windows — Chinese locale (Shanghai).
pub fn chrome_130_cn() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "147.0.7727.117".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36".into(),
        screen_width: 1920, screen_height: 1080,
        screen_avail_width: 1920, screen_avail_height: 1040,
        screen_avail_top: 0,
        screen_color_depth: 24, device_pixel_ratio: 1.25,
        cpu_cores: 12, device_memory: 16, max_touch_points: 0,
        webgl_vendor: "Google Inc. (NVIDIA)".into(),
        webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
        language: "zh-CN".into(),
        languages: vec!["zh-CN".into(), "zh".into(), "en-US".into(), "en".into()],
        timezone: "Asia/Shanghai".into(),
        cpu_architecture: "x86".into(),
        cpu_bitness: "64".into(),
        platform_version: "15.0.0".into(),
        ua_model: "".into(),
        ua_wow64: false,
        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 150, connection_downlink: 6.0,
        pdf_viewer_enabled: true, plugins_count: 5, mime_types_count: 2,
        canvas_seed: 0x1122_3344_5566_7788, audio_seed: 0x8877_6655_4433_2211,
        has_platform_authenticator: true, conditional_mediation: true, allow_http3: false,
        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(), hover_capability: "hover".into(),
        inner_width: 1920, inner_height: 969,
        outer_width: 1920, outer_height: 1080,
        proxy: None,
        media_devices: default_media_devices("cn"),
        gpu_profile: crate::gpu::nvidia_rtx_3060_windows(),
    }
}

/// Chrome 130 on Windows — German locale (Berlin).
pub fn chrome_130_de() -> StealthProfile {
    let mut p = chrome_130_windows();
    p.language = "de-DE".into();
    p.languages = vec!["de-DE".into(), "de".into(), "en-US".into(), "en".into()];
    p.timezone = "Europe/Berlin".into();
    p.canvas_seed = 0xdede_dede_dede_dede;
    p.audio_seed = 0xeded_eded_eded_eded;
    p
}

/// Chrome 130 on Windows — Japanese locale (Tokyo).
pub fn chrome_130_jp() -> StealthProfile {
    let mut p = chrome_130_windows();
    p.language = "ja-JP".into();
    p.languages = vec!["ja".into(), "en-US".into(), "en".into()];
    p.timezone = "Asia/Tokyo".into();
    p.canvas_seed = 0x0a00_0000_0000_0001;
    p.audio_seed = 0x0a00_0000_0000_0002;
    p
}

// ===== Firefox 135 presets =====
//
// Per the Camoufox network capture (`/tmp/cam_capture/summary.txt`), real
// Firefox 135 sends a distinctly different request shape than Chrome — no
// `sec-ch-ua*` headers, different `accept` and `accept-language` quality
// values, no `priority` header. Several anti-bot vendors (DataDome,
// Disney+, Akamai-protected adidas) treat Firefox more leniently than
// Chrome because Chrome is ~70% of bot traffic, so vendors invest
// disproportionately in Chrome detection. Adding a Firefox profile lets
// callers swap to it for sites where Chrome class is being detected.
//
// Firefox-specific spec details:
// - `navigator.vendor === ""` (Chrome reports "Google Inc.")
// - `navigator.productSub === "20100101"` (Firefox uses Gecko build date,
//   Chrome uses "20030107")
// - `webgl_vendor` / `webgl_renderer` masked to "Mozilla" / "Mozilla" by
//   default (Firefox 113+ enables this for non-Nightly to reduce passive
//   fingerprint surface)
// - `tls_impersonate` is set to `firefox_135` here as a forward-compatible
//   string; the actual TLS-class swap is gated by Phase B.3 (rquest's
//   `Impersonate::Firefox*` enum). Until B.3 lands, the network layer
//   falls back to the chrome_130 cipher suite — coherent for now since
//   most sites that flip on Firefox UA do so based on the UA + headers,
//   not TLS.

/// Firefox 135 on macOS.
pub fn firefox_135_macos() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:135.0) Gecko/20100101 Firefox/135.0".into(),
        browser_name: "Firefox".into(),
        browser_version: "135.0".into(),
        os_name: "macOS".into(),
        os_version: "14.5".into(),
        platform: "MacIntel".into(),
        vendor: "".into(),
        vendor_sub: "".into(),
        product_sub: "20100101".into(),
        app_version: "5.0 (Macintosh; Intel Mac OS X 14.5; rv:135.0) Gecko/20100101 Firefox/135.0".into(),

        screen_width: 1440,
        screen_height: 900,
        screen_avail_width: 1440,
        screen_avail_height: 875,
        screen_avail_top: 25,
        screen_color_depth: 30,
        device_pixel_ratio: 2.0,
        cpu_cores: 10,
        device_memory: 16,
        max_touch_points: 0,

        // Firefox masks WebGL by default since v113 — both vendor and
        // renderer report "Mozilla". Sites that fingerprint via WebGL get
        // less entropy, which is the point.
        webgl_vendor: "Mozilla".into(),
        webgl_renderer: "Mozilla".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/Los_Angeles".into(),

        cpu_architecture: "arm".into(),
        cpu_bitness: "64".into(),
        platform_version: "14.5.0".into(),
        ua_model: "".into(),
        ua_wow64: false,

        // String token — currently informational only. The actual TLS
        // bytes are emitted by `crates/net` via boring2/BoringSSL with a
        // Chrome-tuned ClientHello. A real Firefox JA4 swap requires
        // reconfiguring boring2's cipher list / extension order to match
        // NSS — substantial work tracked as a future item. Many sites
        // (including the Camoufox-passing leboncoin/disneyplus) flip on
        // UA+headers alone, so this gap is acceptable for Phase B.
        tls_impersonate: "firefox_135".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0xff0011_ff0022_ff0033_u128 as u64,
        audio_seed: 0x88aa_bbcc_ddee_ff00,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(),
        hover_capability: "hover".into(),

        inner_width: 1440,
        inner_height: 789,
        outer_width: 1440,
        outer_height: 900,

        proxy: None,
        media_devices: default_media_devices("macos"),
        gpu_profile: crate::gpu::apple_m2_pro_macos(),
    }
}

/// Firefox 135 on Windows 10.
pub fn firefox_135_windows() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0".into(),
        browser_name: "Firefox".into(),
        browser_version: "135.0".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "".into(),
        vendor_sub: "".into(),
        product_sub: "20100101".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0".into(),

        screen_width: 1920,
        screen_height: 1080,
        screen_avail_width: 1920,
        screen_avail_height: 1040,
        screen_avail_top: 0,
        screen_color_depth: 24,
        device_pixel_ratio: 1.0,
        cpu_cores: 8,
        device_memory: 8,
        max_touch_points: 0,

        webgl_vendor: "Mozilla".into(),
        webgl_renderer: "Mozilla".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/New_York".into(),

        cpu_architecture: "x86".into(),
        cpu_bitness: "64".into(),
        platform_version: "15.0.0".into(),
        ua_model: "".into(),
        ua_wow64: false,

        tls_impersonate: "firefox_135".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1122_3344_5566_7788,
        audio_seed: 0x99aa_bbcc_ddee_ff00,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(),
        hover_capability: "hover".into(),

        inner_width: 1920,
        inner_height: 969,
        outer_width: 1920,
        outer_height: 1080,

        proxy: None,
        media_devices: default_media_devices("windows"),
        gpu_profile: crate::gpu::nvidia_rtx_3060_windows(),
    }
}

/// Firefox 135 on Linux.
pub fn firefox_135_linux() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (X11; Linux x86_64; rv:135.0) Gecko/20100101 Firefox/135.0".into(),
        browser_name: "Firefox".into(),
        browser_version: "135.0".into(),
        os_name: "Linux".into(),
        os_version: "6.1".into(),
        platform: "Linux x86_64".into(),
        vendor: "".into(),
        vendor_sub: "".into(),
        product_sub: "20100101".into(),
        app_version: "5.0 (X11; Linux x86_64; rv:135.0) Gecko/20100101 Firefox/135.0".into(),

        screen_width: 1920,
        screen_height: 1080,
        screen_avail_width: 1920,
        screen_avail_height: 1053,
        screen_avail_top: 0,
        screen_color_depth: 24,
        device_pixel_ratio: 1.0,
        cpu_cores: 8,
        device_memory: 8,
        max_touch_points: 0,

        webgl_vendor: "Mozilla".into(),
        webgl_renderer: "Mozilla".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/Chicago".into(),

        cpu_architecture: "x86".into(),
        cpu_bitness: "64".into(),
        platform_version: "".into(),
        ua_model: "".into(),
        ua_wow64: false,

        tls_impersonate: "firefox_135".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0xaaaa_bbbb_cccc_dddd,
        audio_seed: 0xdddd_cccc_bbbb_aaaa,

        has_platform_authenticator: false,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        pointer_type: "fine".into(),
        hover_capability: "hover".into(),

        inner_width: 1920,
        inner_height: 969,
        outer_width: 1920,
        outer_height: 1080,

        proxy: None,
        media_devices: default_media_devices("linux"),
        gpu_profile: crate::gpu::intel_uhd_630_linux(),
    }
}

/// Create a profile with custom locale/timezone from a base profile.
pub fn with_locale(
    mut base: StealthProfile,
    language: &str,
    languages: &[&str],
    timezone: &str,
) -> StealthProfile {
    base.language = language.into();
    base.languages = languages.iter().map(|s| s.to_string()).collect();
    base.timezone = timezone.into();
    base
}

/// Random desktop profile (picks randomly from presets).
pub fn random_desktop() -> StealthProfile {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut profile = match rng.gen_range(0..3) {
        0 => chrome_130_windows(),
        1 => chrome_130_macos(),
        _ => chrome_130_linux(),
    };
    // Randomize seeds
    profile.canvas_seed = rng.gen();
    profile.audio_seed = rng.gen();
    profile
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_130_windows_validates() {
        let profile = chrome_130_windows();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
    }

    #[test]
    fn http3_disabled_by_default_on_all_presets() {
        // Gap #33: vanilla quinn-proto emits randomized transport_parameters
        // per handshake; advertising h3 is worse than not speaking it. All
        // shipped presets must default allow_http3 to false until we
        // vendor-fork quinn with a Chrome-fixed-order patch.
        for profile in [
            chrome_130_windows(),
            chrome_130_macos(),
            chrome_130_linux(),
            chrome_130_ru(),
            chrome_130_cn(),
            chrome_130_de(),
            chrome_130_jp(),
            firefox_135_macos(),
            firefox_135_windows(),
            firefox_135_linux(),
        ] {
            assert!(
                !profile.allow_http3,
                "Profile {} has allow_http3=true; gap #33 forbids this",
                profile.user_agent
            );
        }
    }

    #[test]
    fn chrome_130_macos_validates() {
        let profile = chrome_130_macos();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
    }

    #[test]
    fn chrome_130_linux_validates() {
        let profile = chrome_130_linux();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
    }

    #[test]
    fn firefox_135_macos_validates() {
        let profile = firefox_135_macos();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
        assert_eq!(profile.browser_name, "Firefox");
        assert_eq!(profile.vendor, "");
        assert_eq!(profile.product_sub, "20100101");
        assert!(profile.user_agent.contains("rv:135.0"));
        assert!(profile.user_agent.contains("Firefox/135.0"));
        assert!(!profile.user_agent.contains("Chrome"));
        assert_eq!(profile.tls_impersonate, "firefox_135");
    }

    #[test]
    fn firefox_135_windows_validates() {
        let profile = firefox_135_windows();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
        assert!(profile.user_agent.contains("Windows NT 10.0"));
        assert!(profile.user_agent.contains("Firefox/135.0"));
    }

    #[test]
    fn firefox_135_linux_validates() {
        let profile = firefox_135_linux();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
        assert!(profile.user_agent.contains("X11; Linux x86_64"));
        assert!(profile.user_agent.contains("Firefox/135.0"));
    }

    #[test]
    fn firefox_webgl_is_masked() {
        // Firefox 113+ masks WebGL by default — both vendor and renderer
        // report "Mozilla". This is a deliberate fingerprint reduction.
        for profile in [firefox_135_macos(), firefox_135_windows(), firefox_135_linux()] {
            assert_eq!(profile.webgl_vendor, "Mozilla");
            assert_eq!(profile.webgl_renderer, "Mozilla");
        }
    }

    #[test]
    fn random_desktop_validates() {
        for _ in 0..10 {
            let profile = random_desktop();
            assert!(profile.validate().is_ok());
        }
    }

    #[test]
    fn invalid_profile_detected() {
        let mut profile = chrome_130_windows();
        profile.platform = "MacIntel".into(); // Mismatch: Windows + MacIntel
        assert!(profile.validate().is_err());
    }

    #[test]
    fn invalid_gpu_os_mismatch() {
        let mut profile = chrome_130_windows();
        profile.webgl_renderer =
            "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)".into();
        profile.webgl_vendor = "Google Inc. (Apple)".into();
        assert!(profile.validate().is_err());
    }

    #[test]
    fn webdriver_not_in_profile() {
        // StealthProfile has no webdriver field — it's undefined by design
        let profile = chrome_130_windows();
        // Just verify the profile doesn't accidentally contain "webdriver"
        assert!(!profile.user_agent.contains("webdriver"));
    }

    #[test]
    fn ua_contains_version() {
        let profile = chrome_130_windows();
        // Chrome UA-reduction freezes minor versions to 0; only major is in the UA string.
        // Full version lives in browser_version for sec-ch-ua-full-version-list.
        assert!(profile.user_agent.contains("147.0.0.0"));
        assert_eq!(profile.browser_version, "147.0.7727.117");
    }

    #[test]
    fn serialization_roundtrip() {
        let profile = chrome_130_windows();
        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: StealthProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile.user_agent, deserialized.user_agent);
        assert_eq!(profile.screen_width, deserialized.screen_width);
    }
}
