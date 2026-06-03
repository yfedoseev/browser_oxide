use crate::profile::{DeviceClass, MediaDeviceInfo, StealthProfile};

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

/// Chrome 148 on Windows 10.
pub fn chrome_148_windows() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "148.0.7778.168".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),

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

        device_class: DeviceClass::Desktop,
        tls_impersonate: "chrome_147".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1234567890abcdef,
        audio_seed: 0xfedcba0987654321,
        audio_sample_rate: 44100,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "srgb".into(),
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

/// Chrome 148 on macOS 15. Real Chrome shipped 148 in early May 2026 per
/// chromiumdash.appspot.com: current stable = 148.0.7778.168 (Mac/Windows),
/// 148.0.7778.167 (Linux). Tracking the current stable version matters because
/// an outdated Chrome version is itself a reliable signal. The TLS impersonation
/// label is still `chrome_147` — an internal codename; not on the wire.
///
/// **CRITICAL**: navigator.userAgent reports `Chrome/148.0.0.0` (FROZEN minor versions
/// per Chrome's User-Agent reduction since March 2023 / Chrome 110+). The full version
/// `148.0.7778.168` is ONLY exposed via sec-ch-ua-full-version-list. Sending the full
/// version in the UA string is a divergence from real Chrome behavior — confirmed by
/// comparing real-browser header captures against our pipeline.
pub fn chrome_148_macos() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        // browser_version stores the FULL version — used by sec-ch-ua-full-version-list
        // and by build_sec_ch_ua's major-version split. The UA string above uses
        // the reduced 148.0.0.0 form per Chrome's UA-reduction policy.
        browser_version: "148.0.7778.168".into(),
        os_name: "macOS".into(),
        os_version: "15.2".into(),
        platform: "MacIntel".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),

        // Phase 7 — match real Chrome 148 on macOS arm64 (M3 MacBook Pro):
        // 1512x982 viewport, availHeight 949 (982 - 33 top), colorDepth 30,
        // 8 cpu cores. Verified against a real-browser secure-context probe capture.
        screen_width: 1512,
        screen_height: 982,
        screen_avail_width: 1512,
        screen_avail_height: 949,
        screen_avail_top: 33,
        screen_color_depth: 30,
        device_pixel_ratio: 2.0,
        cpu_cores: 8,
        device_memory: 8,
        max_touch_points: 0,

        webgl_vendor: "Google Inc. (Apple)".into(),
        webgl_renderer: "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/Los_Angeles".into(),

        cpu_architecture: "arm".into(),
        cpu_bitness: "64".into(),
        platform_version: "15.2.0".into(),
        ua_model: "".into(),
        ua_wow64: false,

        device_class: DeviceClass::Desktop,
        tls_impersonate: "chrome_147".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0xabcdef1234567890,
        audio_seed: 0x0987654321fedcba,
        // Apple Silicon M3 reports 48000 Hz native; this is the value
        // real Chrome on M3 returns from `new AudioContext().sampleRate`.
        // Held constant across page loads for cross-load consistency.
        audio_sample_rate: 48000,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "p3".into(),
        pointer_type: "fine".into(),
        hover_capability: "hover".into(),

        // Phase 7 — match Chrome 148 macOS arm64 viewport.
        inner_width: 1512,
        inner_height: 871,
        outer_width: 1512,
        outer_height: 982,

        proxy: None,
        media_devices: default_media_devices("macos"),
        gpu_profile: crate::gpu::apple_m3_macos(),
    }
}

/// Chrome 148 on Linux.
pub fn chrome_148_linux() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "148.0.7778.168".into(),
        os_name: "Linux".into(),
        os_version: "6.1".into(),
        platform: "Linux x86_64".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),

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

        device_class: DeviceClass::Desktop,
        tls_impersonate: "chrome_147".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1111222233334444,
        audio_seed: 0x5555666677778888,
        audio_sample_rate: 44100,

        // Linux desktop has no platform authenticator (no Touch ID / Windows Hello).
        has_platform_authenticator: false,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "srgb".into(),
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

/// Chrome 148 on Windows — Russian locale (Moscow).
pub fn chrome_148_ru() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "148.0.7778.168".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
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
        device_class: DeviceClass::Desktop,
        tls_impersonate: "chrome_147".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 100, connection_downlink: 8.0,
        pdf_viewer_enabled: true, plugins_count: 5, mime_types_count: 2,
        canvas_seed: 0xaaaa_bbbb_cccc_dddd, audio_seed: 0xdddd_cccc_bbbb_aaaa,
        audio_sample_rate: 44100,
        has_platform_authenticator: true, conditional_mediation: true, allow_http3: false,
        prefers_color_scheme: "light".into(),
        color_gamut: "srgb".into(),
        pointer_type: "fine".into(), hover_capability: "hover".into(),
        inner_width: 1920, inner_height: 969,
        outer_width: 1920, outer_height: 1080,
        proxy: None,
        media_devices: default_media_devices("ru"),
        gpu_profile: crate::gpu::nvidia_rtx_3060_windows(),
    }
}

/// Chrome 148 on Windows — Chinese locale (Shanghai).
pub fn chrome_148_cn() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "148.0.7778.168".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36".into(),
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
        device_class: DeviceClass::Desktop,
        tls_impersonate: "chrome_147".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 150, connection_downlink: 6.0,
        pdf_viewer_enabled: true, plugins_count: 5, mime_types_count: 2,
        canvas_seed: 0x1122_3344_5566_7788, audio_seed: 0x8877_6655_4433_2211,
        audio_sample_rate: 44100,
        has_platform_authenticator: true, conditional_mediation: true, allow_http3: false,
        prefers_color_scheme: "light".into(),
        color_gamut: "srgb".into(),
        pointer_type: "fine".into(), hover_capability: "hover".into(),
        inner_width: 1920, inner_height: 969,
        outer_width: 1920, outer_height: 1080,
        proxy: None,
        media_devices: default_media_devices("cn"),
        gpu_profile: crate::gpu::nvidia_rtx_3060_windows(),
    }
}

/// Chrome 148 on Windows — German locale (Berlin).
pub fn chrome_148_de() -> StealthProfile {
    let mut p = chrome_148_windows();
    p.language = "de-DE".into();
    p.languages = vec!["de-DE".into(), "de".into(), "en-US".into(), "en".into()];
    p.timezone = "Europe/Berlin".into();
    p.canvas_seed = 0xdede_dede_dede_dede;
    p.audio_seed = 0xeded_eded_eded_eded;
    p
}

/// Chrome 148 on Windows — Japanese locale (Tokyo).
pub fn chrome_148_jp() -> StealthProfile {
    let mut p = chrome_148_windows();
    p.language = "ja-JP".into();
    p.languages = vec!["ja".into(), "en-US".into(), "en".into()];
    p.timezone = "Asia/Tokyo".into();
    p.canvas_seed = 0x0a00_0000_0000_0001;
    p.audio_seed = 0x0a00_0000_0000_0002;
    p
}

// ===== Firefox 135 presets =====
//
// Per a real Firefox network capture, real Firefox 135 sends a distinctly
// different request shape than Chrome — no `sec-ch-ua*` headers, different
// `accept` and `accept-language` quality values, no `priority` header. Some
// sites treat Firefox more leniently than Chrome because Chrome is the
// majority of traffic and receives disproportionate detection investment.
// Adding a Firefox profile lets callers swap to it for sites where the
// Chrome class is being detected.
//
// Firefox-specific spec details:
// - `navigator.vendor === ""` (Chrome reports "Google Inc.")
// - `navigator.productSub === "20100101"` (Firefox uses Gecko build date,
//   Chrome uses "20030107")
// - `webgl_vendor` / `webgl_renderer` masked to "Mozilla" / "Mozilla" by
//   default (Firefox 113+ enables this for non-Nightly to reduce passive
//   fingerprint surface)
// - `tls_impersonate` is set to `firefox_135` here as a forward-compatible
//   string; when the Firefox TLS-class swap is not active the network layer
//   falls back to the Chrome cipher suite — coherent for now since most
//   sites that flip on Firefox UA do so based on the UA + headers, not TLS.

/// Firefox 135 on macOS.
pub fn firefox_135_macos() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent:
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:135.0) Gecko/20100101 Firefox/135.0"
                .into(),
        browser_name: "Firefox".into(),
        browser_version: "135.0".into(),
        os_name: "macOS".into(),
        os_version: "14.5".into(),
        platform: "MacIntel".into(),
        vendor: "".into(),
        vendor_sub: "".into(),
        product_sub: "20100101".into(),
        app_version: "5.0 (Macintosh; Intel Mac OS X 14.5; rv:135.0) Gecko/20100101 Firefox/135.0"
            .into(),

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
        // NSS — substantial work tracked as a future item. Many sites flip
        // on UA+headers alone, so this gap is acceptable for now.
        device_class: DeviceClass::Desktop,
        tls_impersonate: "firefox_135".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0xff0011_ff0022_ff0033_u128 as u64,
        audio_seed: 0x88aa_bbcc_ddee_ff00,
        audio_sample_rate: 44100,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "p3".into(),
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
        user_agent:
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0"
                .into(),
        browser_name: "Firefox".into(),
        browser_version: "135.0".into(),
        os_name: "Windows".into(),
        os_version: "10.0".into(),
        platform: "Win32".into(),
        vendor: "".into(),
        vendor_sub: "".into(),
        product_sub: "20100101".into(),
        app_version: "5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0"
            .into(),

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

        device_class: DeviceClass::Desktop,
        tls_impersonate: "firefox_135".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0x1122_3344_5566_7788,
        audio_seed: 0x99aa_bbcc_ddee_ff00,
        audio_sample_rate: 44100,

        has_platform_authenticator: true,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "srgb".into(),
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

        device_class: DeviceClass::Desktop,
        tls_impersonate: "firefox_135".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        pdf_viewer_enabled: true,
        plugins_count: 5,
        mime_types_count: 2,

        canvas_seed: 0xaaaa_bbbb_cccc_dddd,
        audio_seed: 0xdddd_cccc_bbbb_aaaa,
        audio_sample_rate: 44100,

        has_platform_authenticator: false,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "srgb".into(),
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
    use rand::RngExt;
    let mut rng = rand::rng();
    let mut profile = match rng.random_range(0..3) {
        0 => chrome_148_windows(),
        1 => chrome_148_macos(),
        _ => chrome_148_linux(),
    };
    // Randomize seeds
    profile.canvas_seed = rng.random();
    profile.audio_seed = rng.random();
    profile
}

/// Apple Silicon Chrome 148 profile sampler.
///
/// Returns one variant of `chrome_148_macos` with screen geometry, core
/// count, RAM, and fingerprint seeds independently sampled from
/// realistic Apple Silicon distributions. Use this in benchmarking
/// / sweep / production loops where issuing the SAME profile from the
/// SAME datacenter IP across many requests trips IP-based fingerprint
/// clustering. Real-browser tooling ships dozens of macOS variants;
/// sampling avoids presenting a single static device fingerprint.
///
/// Sampled axes (all common on shipping Apple Silicon Macs):
/// - **Screen** — `(width, height, avail_height)` from a 4-config pool
///   covering 13.6" MBA / 14" MBP base / 14" Pro / 16" Pro logical
///   resolutions. devicePixelRatio stays 2.0 (Retina is universal on
///   modern Macs).
/// - **CPU cores** — `{8, 10, 12, 14, 16}` covering M1/M2/M3 base
///   (8) through Pro (10/12) and Max (14/16).
/// - **RAM (GB)** — `{8, 16, 24, 36}` covering the most-common Apple
///   Silicon shipping configurations. Note: `Sec-CH-Device-Memory`
///   quantizes to ≤8 via FIX-F, so the value is largely cosmetic for
///   the HTTP header but matters for `navigator.deviceMemory` JS reads.
/// - **canvas_seed / audio_seed** — fully random per call so canvas
///   and AudioContext fingerprints differ across instances.
///
/// `inner_width / outer_width` track `screen_width`. `inner_height`
/// is `screen_height - 111` (the Chrome toolbar+tab-bar height
/// observed on Chrome 148 macOS with bookmarks bar visible);
/// `outer_height = screen_height`.
///
/// The GPU profile (`unmasked_renderer = "Apple M3"`) is held fixed.
/// Per-M-chip GPU strings would require parallel changes to
/// `apple_m3_macos()` / `apple_m2_pro_macos()` and a per-string
/// GpuProfile selector — deferred to a follow-up.
///
/// Profile validation is asserted before return; a panic here means a
/// new sampled value violated a `validate()` invariant introduced
/// elsewhere — fail loud.
pub fn chrome_148_macos_sampled() -> StealthProfile {
    chrome_148_macos_sampled_with_rng(&mut rand::rng())
}

/// As [`chrome_148_macos_sampled`] but takes a caller-supplied RNG so
/// tests can pin determinism.
///
/// **Cross-API consistency.** Sampled values MUST stay self-consistent
/// because fingerprinters cross-check related surfaces:
/// `navigator.hardwareConcurrency` is expected to match the chip
/// claimed by `WEBGL_UNMASKED_RENDERER`. So the sampler picks ONE chip
/// per call (M3 base / M3 Pro / M3 Max) and constrains `cpu_cores`,
/// `device_memory`, screen geometry, and the GpuProfile to match that
/// chip's shipping configurations.
///
/// Verified empirically: an earlier sampler version that varied cores
/// independently of GPU regressed a multi-site sweep because the cross-API
/// surfaces no longer agreed. The narrower M3-only fix passed cross-API
/// checks. The pool is widened again by adding `apple_m3_pro_macos()` /
/// `apple_m3_max_macos()` GpuProfile variants that match the chip
/// variant being sampled.
///
/// Per-chip pool (5 screen × 6 core × 5 RAM × 64-bit seeds):
///
/// | Chip       | cpu_cores | RAM (GB) | Screens                                  |
/// |------------|-----------|----------|------------------------------------------|
/// | M3         | 8         | 8/16/24  | 13.6" MBA, 14" MBP base                  |
/// | M3 Pro     | 11/12     | 18/36    | 14" MBP Pro, 16" MBP Pro                 |
/// | M3 Max     | 14/16     | 36/48    | 14" MBP Max, 16" MBP Max                 |
pub fn chrome_148_macos_sampled_with_rng(rng: &mut impl rand::RngExt) -> StealthProfile {
    let mut p = chrome_148_macos();

    // Pick a chip variant first; everything else is constrained by it.
    type ChipConfig = (
        &'static [u8],              // cores_pool
        &'static [u8],              // ram_pool (GB)
        &'static [(u32, u32, u32)], // screens (width, height, avail_height)
        crate::gpu::GpuProfile,     // matching GpuProfile
    );
    let chip_idx = rng.random_range(0..3);
    let (cores_pool, ram_pool, screens, gpu): ChipConfig = match chip_idx {
        // M3 base — 8c, 8/16/24 GB, 13.6"-14" MBP base hardware.
        0 => (
            &[8],
            &[8, 16, 24],
            &[
                (1512, 982, 949),   // 13.6" MBA / 13.6" MBP base
                (1728, 1117, 1010), // 14" MBP M3 base
            ],
            crate::gpu::apple_m3_macos(),
        ),
        // M3 Pro — 11c (4P+6E binned) or 12c (6P+6E), 18/36 GB.
        // 14" and 16" MBP Pro both ship M3 Pro.
        1 => (
            &[11, 12],
            &[18, 36],
            &[
                (1800, 1169, 1100), // 14" MBP M3 Pro
                (2056, 1329, 1253), // 16" MBP M3 Pro
            ],
            crate::gpu::apple_m3_pro_macos(),
        ),
        // M3 Max — 14c (10P+4E) or 16c (12P+4E), 36/48 GB (lower
        // shipping configs; BTO goes up to 128 GB but rare).
        _ => (
            &[14, 16],
            &[36, 48],
            &[
                (1800, 1169, 1100), // 14" MBP M3 Max
                (2056, 1329, 1253), // 16" MBP M3 Max
            ],
            crate::gpu::apple_m3_max_macos(),
        ),
    };

    p.cpu_cores = cores_pool[rng.random_range(0..cores_pool.len())];
    p.device_memory = ram_pool[rng.random_range(0..ram_pool.len())];

    let (w, h, ah) = screens[rng.random_range(0..screens.len())];
    p.screen_width = w;
    p.screen_height = h;
    p.screen_avail_width = w;
    p.screen_avail_height = ah;
    p.inner_width = w;
    // 111 px = Chrome 148 macOS toolbar + tab bar + bookmarks bar.
    p.inner_height = h.saturating_sub(111);
    p.outer_width = w;
    p.outer_height = h;

    // Per-chip GPU profile so navigator.hardwareConcurrency,
    // WEBGL_UNMASKED_RENDERER, and RAM all describe one real device.
    p.gpu_profile = gpu;
    // Mirror the renderer onto the legacy single-field `webgl_renderer`
    // so JS-side reads via op_get_profile_value("webgl_renderer") agree.
    p.webgl_renderer = p.gpu_profile.unmasked_renderer.clone();

    // Fully randomize the canvas + audio fingerprint seeds so two
    // instances of this sampler produce distinct canvas / DynamicsCompressor
    // hashes, avoiding per-fingerprint clustering across instances.
    p.canvas_seed = rng.random();
    p.audio_seed = rng.random();

    debug_assert!(
        p.validate().is_ok(),
        "chrome_148_macos_sampled produced an invalid profile: {:?}",
        p.validate()
    );

    p
}

/// Chrome 148 on Pixel 9 Pro (Android 15). Phase 2 mobile profile.
/// TLS deltas vs desktop:
///   - elliptic curves: X25519_KYBER768_DRAFT00 instead of MLKEM768
///     (Android Chrome lags desktop on PQ rollout; verify against fresh
///     M147 Pixel capture if a recent rollout is suspected)
/// Header / UA-CH deltas:
///   - UA: Pixel-flavored mobile string with "Mobile" token
///   - Sec-CH-UA-Mobile: ?1
///   - Sec-CH-UA-Platform: "Android"
///   - Sec-CH-UA-Model: "Pixel 9 Pro" (display name, not codename `tokay`)
///   - Sec-CH-UA-Form-Factors: "Mobile"
/// Hardware / JS-surface deltas (Pixel 9 Pro specs):
///   - 412×870 viewport, devicePixelRatio = 2.625 (fractional!)
///   - maxTouchPoints: 5
///   - platform: "Linux armv81"
///   - hardwareConcurrency: 8 (Tensor G4 reports 8 from 9 actual cores)
///   - deviceMemory: 8 (Chrome rounds to spec set {0.25, 0.5, 1, 2, 4, 8})
///   - WebGL renderer: "ANGLE (Google, Mali-G715 MP7, OpenGL ES 3.2)"
pub fn pixel_9_pro_chrome_148() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (Linux; Android 15; Pixel 9 Pro Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Mobile Safari/537.36".into(),
        browser_name: "Chrome".into(),
        browser_version: "148.0.7778.168".into(),
        os_name: "Android".into(),
        os_version: "15".into(),
        platform: "Linux armv81".into(),
        vendor: "Google Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (Linux; Android 15; Pixel 9 Pro Build/AP4A.250105.002) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Mobile Safari/537.36".into(),

        // Pixel 9 Pro: 412 × 870 CSS px, fractional DPR 2.625
        screen_width: 412,
        screen_height: 870,
        screen_avail_width: 412,
        screen_avail_height: 870,
        screen_avail_top: 0,
        screen_color_depth: 24,
        device_pixel_ratio: 2.625,
        cpu_cores: 8,
        device_memory: 8,
        // Real Pixel reports 5 simultaneous touch points
        max_touch_points: 5,

        // ANGLE-wrapped renderer string (Pixel 9 Tensor G4 has Mali-G715 MP7)
        webgl_vendor: "Google Inc. (Google)".into(),
        webgl_renderer: "ANGLE (Google, Mali-G715 MP7, OpenGL ES 3.2)".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/Los_Angeles".into(),

        // Empty cpu_architecture on Android per UA reduction
        cpu_architecture: "".into(),
        cpu_bitness: "64".into(),
        platform_version: "15.0.0".into(),
        ua_model: "Pixel 9 Pro".into(),
        ua_wow64: false,

        device_class: DeviceClass::MobileAndroid,
        tls_impersonate: "chrome_147_android".into(),
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        // Android Chrome has an EMPTY plugin array — not the 5-plugin
        // desktop set. This is the single biggest mobile-vs-desktop tell
        // on Chromium that anti-bot stacks key off.
        pdf_viewer_enabled: false,
        plugins_count: 0,
        mime_types_count: 0,

        canvas_seed: 0xa5a5_d5d5_3c3c_e6e6,
        audio_seed: 0x9c9c_5e5e_4040_b1b1,
        audio_sample_rate: 44100,

        // No Touch ID / Windows Hello on stock Android; Passkeys via Play
        // Services exist but isUserVerifyingPlatformAuthenticatorAvailable
        // returns false on a fresh profile.
        has_platform_authenticator: false,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "srgb".into(),
        // Touch pointer on phones, not fine mouse
        pointer_type: "coarse".into(),
        // Phones don't hover
        hover_capability: "none".into(),

        // Match screen dimensions for inner/outer (no browser chrome distinction)
        inner_width: 412,
        inner_height: 870,
        outer_width: 412,
        outer_height: 870,

        proxy: None,
        media_devices: default_media_devices("android"),
        gpu_profile: crate::gpu::apple_m3_macos(), // TODO: add android_mali_g715 GPU profile
    }
}

/// Mobile Safari 18 on iPhone 15 Pro (iOS 18.0). Phase 3 mobile profile.
/// Requires Phase 3 TLS work (separate cipher list / sigalgs / no ECH-ALPS
/// / zlib cert) — without it the profile produces a Chrome-flavored
/// ClientHello + Safari UA, which is the #1 instant flag on every
/// anti-bot stack. Use only after Phase 3 lands.
///
/// JS surface deltas (per the audit + Apple's "16 declined APIs" list):
///   - UA: `Mozilla/5.0 (iPhone; CPU iPhone OS 18_0_1 ...) Version/18.0.1 Mobile/15E148 Safari/604.1`
///   - `navigator.platform: "iPhone"`, `maxTouchPoints: 5`
///   - `navigator.deviceMemory: undefined` (WebKit doesn't expose it)
///   - `navigator.hardwareConcurrency: 2` (Safari intentionally caps)
///   - `navigator.userAgentData: undefined` (Safari has no UA-CH at all)
///   - `navigator.connection: undefined` (no NetworkInformation)
///   - No Bluetooth/USB/Serial/HID/Sensor/Battery/MIDI/IdleDetector/WebGPU
///   - No `PaymentRequest.prototype.hasEnrolledInstrument` (Chrome/Edge-only)
///   - WebGL renderer: literal `"Apple GPU"` constant (Apple strips model info)
///   - `window.orientation`: 0 (legacy iOS-only — desktop browsers do NOT have it)
///   - `DeviceMotionEvent.requestPermission`: present static (iOS 13+)
///   - `'ontouchstart' in window`: true
///   - AudioContext sampleRate: 48000
///   - Screen: 393×852 @ DPR 3 (iPhone 15 Pro)
pub fn iphone_15_pro_safari_18() -> StealthProfile {
    StealthProfile {
        enforce_csp: true,
        user_agent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0.1 Mobile/15E148 Safari/604.1".into(),
        browser_name: "Safari".into(),
        browser_version: "18.0.1".into(),
        os_name: "iOS".into(),
        os_version: "18.0.1".into(),
        platform: "iPhone".into(),
        vendor: "Apple Computer, Inc.".into(),
        vendor_sub: "".into(),
        product_sub: "20030107".into(),
        app_version: "5.0 (iPhone; CPU iPhone OS 18_0_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0.1 Mobile/15E148 Safari/604.1".into(),

        // iPhone 15 Pro: 393 × 852 CSS px @ DPR 3 (integer)
        screen_width: 393,
        screen_height: 852,
        screen_avail_width: 393,
        screen_avail_height: 852,
        screen_avail_top: 0,
        screen_color_depth: 24,
        device_pixel_ratio: 3.0,
        // Safari intentionally caps reported cores to limit fingerprint entropy
        cpu_cores: 2,
        // iOS Safari does NOT expose deviceMemory at all — set to 0 here as
        // a sentinel; the JS bootstrap will return undefined for iOS profiles
        // regardless of this value.
        device_memory: 0,
        max_touch_points: 5,

        // Apple intentionally returns the literal string "Apple GPU" (no model)
        webgl_vendor: "Apple Inc.".into(),
        webgl_renderer: "Apple GPU".into(),

        language: "en-US".into(),
        languages: vec!["en-US".into(), "en".into()],
        timezone: "America/Los_Angeles".into(),

        // Safari does not send Sec-CH-UA-* at all; these fields are unused
        // for iOS profiles but kept non-empty for serde compatibility.
        cpu_architecture: "arm".into(),
        cpu_bitness: "64".into(),
        platform_version: "18.0.1".into(),
        ua_model: "iPhone".into(),
        ua_wow64: false,

        device_class: DeviceClass::MobileIOS,
        tls_impersonate: "safari_18_ios".into(), // Phase 3 will wire this up
        connection_effective_type: "4g".into(),
        connection_rtt: 50,
        connection_downlink: 10.0,

        // Mobile Safari has empty plugin array
        pdf_viewer_enabled: false,
        plugins_count: 0,
        mime_types_count: 0,

        canvas_seed: 0xa1b2_c3d4_e5f6_0708,
        audio_seed: 0x0807_0605_0403_0201,
        audio_sample_rate: 44100,

        // Touch ID / Face ID exists but isUserVerifyingPlatformAuthenticatorAvailable
        // returns false on a fresh iOS Safari profile (per Apple privacy default)
        has_platform_authenticator: false,
        conditional_mediation: true,
        allow_http3: false,

        prefers_color_scheme: "light".into(),
        color_gamut: "p3".into(),
        pointer_type: "coarse".into(),
        hover_capability: "none".into(),

        inner_width: 393,
        inner_height: 852,
        outer_width: 393,
        outer_height: 852,

        proxy: None,
        media_devices: default_media_devices("ios"),
        gpu_profile: crate::gpu::apple_m3_macos(), // TODO: ios_apple_a17_pro GPU profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_148_windows_validates() {
        let profile = chrome_148_windows();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
    }

    #[test]
    fn http3_disabled_by_default_on_all_presets() {
        // vanilla quinn-proto emits randomized transport_parameters
        // per handshake; advertising h3 is worse than not speaking it. All
        // shipped presets must default allow_http3 to false until we
        // vendor-fork quinn with a Chrome-fixed-order patch.
        for profile in [
            chrome_148_windows(),
            chrome_148_macos(),
            chrome_148_linux(),
            chrome_148_ru(),
            chrome_148_cn(),
            chrome_148_de(),
            chrome_148_jp(),
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
    fn chrome_148_macos_validates() {
        let profile = chrome_148_macos();
        assert!(profile.validate().is_ok(), "{:?}", profile.validate());
    }

    #[test]
    fn chrome_148_linux_validates() {
        let profile = chrome_148_linux();
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
        for profile in [
            firefox_135_macos(),
            firefox_135_windows(),
            firefox_135_linux(),
        ] {
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
        let mut profile = chrome_148_windows();
        profile.platform = "MacIntel".into(); // Mismatch: Windows + MacIntel
        assert!(profile.validate().is_err());
    }

    #[test]
    fn invalid_gpu_os_mismatch() {
        let mut profile = chrome_148_windows();
        profile.webgl_renderer =
            "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)".into();
        profile.webgl_vendor = "Google Inc. (Apple)".into();
        assert!(profile.validate().is_err());
    }

    #[test]
    fn webdriver_not_in_profile() {
        // StealthProfile has no webdriver field — it's undefined by design
        let profile = chrome_148_windows();
        // Just verify the profile doesn't accidentally contain "webdriver"
        assert!(!profile.user_agent.contains("webdriver"));
    }

    #[test]
    fn ua_contains_version() {
        let profile = chrome_148_windows();
        // Chrome UA-reduction freezes minor versions to 0; only major is in the UA string.
        // Full version lives in browser_version for sec-ch-ua-full-version-list.
        assert!(profile.user_agent.contains("148.0.0.0"));
        assert_eq!(profile.browser_version, "148.0.7778.168");
    }

    #[test]
    fn serialization_roundtrip() {
        let profile = chrome_148_windows();
        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: StealthProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile.user_agent, deserialized.user_agent);
        assert_eq!(profile.screen_width, deserialized.screen_width);
    }

    #[test]
    fn macos_sampler_produces_valid_profiles() {
        // 200 samples — every one must pass validate() and stay within the
        // declared Apple-Silicon M3-family pools.
        for _ in 0..200 {
            let p = chrome_148_macos_sampled();
            p.validate()
                .unwrap_or_else(|e| panic!("invalid sampled profile: {e:?}"));

            assert!(
                matches!(p.screen_width, 1512 | 1728 | 1800 | 2056),
                "screen_width {} not in M3-family pool",
                p.screen_width
            );
            assert!(
                matches!(p.cpu_cores, 8 | 11 | 12 | 14 | 16),
                "cpu_cores {} not in M3-family pool",
                p.cpu_cores
            );
            assert!(
                matches!(p.device_memory, 8 | 16 | 18 | 24 | 36 | 48),
                "device_memory {} not in M3-family pool",
                p.device_memory
            );
            // Apple Silicon Retina invariants
            assert_eq!(p.device_pixel_ratio, 2.0);
            assert_eq!(p.audio_sample_rate, 48000);
            assert_eq!(p.cpu_architecture, "arm");
            assert_eq!(p.platform, "MacIntel");
            // Arithmetic consistency: inner_height = screen_height - 111 (Chrome chrome).
            assert_eq!(p.inner_height + 111, p.screen_height);
        }
    }

    #[test]
    fn macos_sampler_produces_diverse_fingerprints() {
        // 30 samples should hit ≥3 chip variants × multiple configs.
        // Statistically: P(missing one chip in 30 trials) = (2/3)^30 ≈ 5e-6.
        use std::collections::HashSet;
        let mut chips = HashSet::new();
        let mut tuples = HashSet::new();
        let mut seeds = HashSet::new();
        for _ in 0..30 {
            let p = chrome_148_macos_sampled();
            chips.insert(p.cpu_cores);
            tuples.insert((p.screen_width, p.cpu_cores, p.device_memory));
            seeds.insert(p.canvas_seed);
            seeds.insert(p.audio_seed);
        }
        assert!(
            chips.len() >= 3,
            "expected ≥3 distinct cpu_cores values in 30 samples (M3/Pro/Max), got {chips:?}"
        );
        assert!(
            tuples.len() >= 6,
            "expected ≥6 distinct (screen, cores, mem) tuples in 30 samples, got {}: {tuples:?}",
            tuples.len()
        );
        // Seeds are u64 — 30 calls = 60 seed values; collisions should be ~impossible
        assert!(
            seeds.len() >= 58,
            "canvas/audio seeds should be near-fully-distinct, got {} unique of 60",
            seeds.len()
        );
    }

    #[test]
    fn macos_sampler_keeps_per_chip_cross_api_consistency() {
        // Cross-API invariant: every sample's (cpu_cores, gpu_renderer,
        // device_memory) tuple must describe ONE real Apple Silicon
        // device. An earlier sampler version that varied cores
        // independently of GPU regressed a multi-site sweep because the
        // cross-API surfaces no longer agreed.
        for _ in 0..50 {
            let p = chrome_148_macos_sampled();
            let r = &p.gpu_profile.unmasked_renderer;
            match p.cpu_cores {
                8 => {
                    assert!(
                        r.contains("Apple M3,"),
                        "8 cores must pair with 'Apple M3' renderer; got '{r}'"
                    );
                    assert!(matches!(p.device_memory, 8 | 16 | 24));
                }
                11 | 12 => {
                    assert!(
                        r.contains("Apple M3 Pro"),
                        "11-12 cores must pair with 'Apple M3 Pro' renderer; got '{r}'"
                    );
                    assert!(matches!(p.device_memory, 18 | 36));
                }
                14 | 16 => {
                    assert!(
                        r.contains("Apple M3 Max"),
                        "14-16 cores must pair with 'Apple M3 Max' renderer; got '{r}'"
                    );
                    assert!(matches!(p.device_memory, 36 | 48));
                }
                other => panic!("unexpected cpu_cores {other} from sampler"),
            }
            // webgl_renderer (legacy single-field) MUST mirror gpu_profile
            // so JS-side reads of either path agree.
            assert_eq!(p.webgl_renderer, *r);
        }
    }
}
