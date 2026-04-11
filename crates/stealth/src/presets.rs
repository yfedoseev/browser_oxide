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
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "130.0.6723.91".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),

        screen_width: 1920,
        screen_height: 1080,
        screen_avail_width: 1920,
        screen_avail_height: 1040,
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

        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1234567890abcdef,
        audio_seed: 0xfedcba0987654321,

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

/// Chrome 130 on macOS 15.
pub fn chrome_130_macos() -> StealthProfile {
    StealthProfile {
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "130.0.6723.91".into(),
        os_name: "macOS".into(),
        os_version: "15.2".into(),
        platform: "MacIntel".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),

        screen_width: 1440,
        screen_height: 900,
        screen_avail_width: 1440,
        screen_avail_height: 875,
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

        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0xabcdef1234567890,
        audio_seed: 0x0987654321fedcba,

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
        user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "130.0.6723.91".into(),
        os_name: "Linux".into(),
        os_version: "6.1".into(),
        platform: "Linux x86_64".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),

        screen_width: 1920,
        screen_height: 1080,
        screen_avail_width: 1920,
        screen_avail_height: 1053,
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

        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1111222233334444,
        audio_seed: 0x5555666677778888,

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
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "130.0.6723.91".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),
        screen_width: 1920, screen_height: 1080,
        screen_avail_width: 1920, screen_avail_height: 1040,
        screen_color_depth: 24, device_pixel_ratio: 1.0,
        cpu_cores: 8, device_memory: 8, max_touch_points: 0,
        webgl_vendor: "Google Inc. (NVIDIA)".into(),
        webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
        language: "ru-RU".into(),
        languages: vec!["ru-RU".into(), "ru".into(), "en-US".into(), "en".into()],
        timezone: "Europe/Moscow".into(),
        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 100, connection_downlink: 8.0,
        pdf_viewer_enabled: true, plugins_count: 5, mime_types_count: 2,
        canvas_seed: 0xaaaa_bbbb_cccc_dddd, audio_seed: 0xdddd_cccc_bbbb_aaaa,
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
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "130.0.6723.91".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.91 Safari/537.36".into(),
        screen_width: 1920, screen_height: 1080,
        screen_avail_width: 1920, screen_avail_height: 1040,
        screen_color_depth: 24, device_pixel_ratio: 1.25,
        cpu_cores: 12, device_memory: 16, max_touch_points: 0,
        webgl_vendor: "Google Inc. (NVIDIA)".into(),
        webgl_renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
        language: "zh-CN".into(),
        languages: vec!["zh-CN".into(), "zh".into(), "en-US".into(), "en".into()],
        timezone: "Asia/Shanghai".into(),
        tls_impersonate: "chrome_130".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 150, connection_downlink: 6.0,
        pdf_viewer_enabled: true, plugins_count: 5, mime_types_count: 2,
        canvas_seed: 0x1122_3344_5566_7788, audio_seed: 0x8877_6655_4433_2211,
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
        assert!(profile.user_agent.contains("130.0.6723.91"));
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
