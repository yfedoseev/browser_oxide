//! Drive browser_oxide's full challenge flow against adidas.com/us and dump
//! every POST body the solver issues. The env var BOXIDE_DUMP_POST_DIR is
//! honored by net::HttpClient::post_bytes_with_headers and writes one .body
//! plus one .meta.json per POST.
//!
//! Run:
//!   BOXIDE_DUMP_POST_DIR=/tmp/oxide-sensor \
//!     cargo test -p browser --test adidas_sensor_capture -- \
//!     --ignored --test-threads=1 --nocapture
//!
//! Purpose (GAPS §Q7 continuation): look at what our sensor VM actually posts,
//! diff against the documented Akamai sensor_data schema, and answer which of
//!   (a) specific fingerprint values diverge → Tier 1 is the right fix
//!   (b) sensor VM execution itself is broken → fix runtime first
//!   (c) payload is fine, gate is behavioral timing → different workstream
//! is the real reason adidas is blocking us.

#[tokio::test]
#[ignore]
async fn adidas_sensor_capture() {
    let dir = std::env::var("BOXIDE_DUMP_POST_DIR")
        .unwrap_or_else(|_| "/tmp/oxide-sensor".to_string());
    std::env::set_var("BOXIDE_DUMP_POST_DIR", &dir);
    let _ = std::fs::remove_dir_all(&dir);

    let profile = stealth::chrome_130_macos();
    let page = browser::Page::navigate(
        "https://www.adidas.com/us",
        profile,
        1,
    )
    .await;

    match page {
        Ok(_) => println!("[oxide] navigate returned Ok"),
        Err(e) => println!("[oxide] navigate error: {e}"),
    }

    // Dump summary of captured POSTs.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut bodies: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .map(|x| x == "body")
                    .unwrap_or(false)
            })
            .collect();
        bodies.sort();
        println!("[oxide] captured {} POST bodies in {dir}", bodies.len());
        for path in &bodies {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let meta_path = path.with_file_name(format!("{stem}.meta.json"));
            let meta = std::fs::read_to_string(&meta_path).unwrap_or_default();
            let url = meta
                .lines()
                .find(|l| l.contains("\"url\""))
                .unwrap_or("")
                .trim()
                .to_string();
            println!("  {} {} bytes {}", path.display(), size, url);
        }
    }
}
