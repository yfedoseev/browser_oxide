//! Capture the raw HTML a site returns to BO's net stack (faithful TLS +
//! nav headers for the given stealth profile). Used to grab the live AWS WAF
//! challenge stub for the awswaf_probe oracle without going through a full
//! navigate (so the stub is exactly what challenge.js would be served against).
//!
//! Usage:
//!   aws_capture <url> [out_file] [profile]
//!
//! profile defaults to chrome_148_macos. Prints the body to stdout and (if
//! given) writes it to out_file. Exit 0 even on WAF stub — that's the point.

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().unwrap_or_else(|| {
        eprintln!("usage: aws_capture <url> [out_file] [profile]");
        std::process::exit(2);
    });
    let out = args.next();
    let profile_name = args.next().unwrap_or_else(|| "chrome_148_macos".into());
    let profile = match profile_name.as_str() {
        "chrome_148_macos" => stealth::presets::chrome_148_macos(),
        "chrome_148_windows" => stealth::presets::chrome_148_windows(),
        "firefox_135_macos" => stealth::presets::firefox_135_macos(),
        "iphone_15_pro_safari_18" => stealth::presets::iphone_15_pro_safari_18(),
        other => {
            eprintln!("unknown profile {other}");
            std::process::exit(2);
        }
    };

    let client = net::HttpClient::new(&profile).expect("http client");
    let hdrs = net::headers::nav_headers_for_url(&profile, &url, false);
    match client.get_follow_with_headers(&url, &hdrs, 10).await {
        Ok(resp) => {
            let body = resp.text();
            eprintln!(
                "[aws_capture] {} -> {} bytes (profile {profile_name})",
                url,
                body.len()
            );
            if let Some(out) = out {
                std::fs::write(&out, &body).expect("write out");
                eprintln!("[aws_capture] wrote {out}");
            } else {
                println!("{body}");
            }
        }
        Err(e) => {
            eprintln!("[aws_capture] fetch error: {e}");
            std::process::exit(1);
        }
    }
}
