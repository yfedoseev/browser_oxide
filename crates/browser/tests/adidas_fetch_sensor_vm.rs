//! Fetches the current adidas Akamai sensor VM script using our rquest-based
//! net layer (which is NOT IP-blocked), then dumps it to /tmp/adidas_sensor_vm.js
//! for manual inspection. The script URL path rotates per request — we extract
//! it from the interstitial HTML first, then fetch it.
//!
//! Run:
//!   cargo test -p browser --test adidas_fetch_sensor_vm -- \
//!     --ignored --test-threads=1 --nocapture

#[tokio::test]
#[ignore]
async fn adidas_fetch_sensor_vm() {
    let profile = stealth::chrome_130_macos();
    let client = net::HttpClient::new(&profile).unwrap();

    let resp = client.get("https://www.adidas.com/us").await.unwrap();
    let html = resp.text();
    println!("[interstitial] status={} size={}", resp.status, html.len());

    // Extract the <script src="/..."> path from the interstitial.
    let script_path = extract_script_src(&html);
    let Some(script_path) = script_path else {
        println!("[error] could not find sensor script tag in interstitial");
        println!("{}", &html[..html.len().min(600)]);
        return;
    };
    println!("[script] {script_path}");

    // Fetch the script.
    let url = format!("https://www.adidas.com{script_path}");
    let script_resp = client.get(&url).await.unwrap();
    let script = script_resp.text();
    println!("[script] status={} size={}", script_resp.status, script.len());

    std::fs::write("/tmp/adidas_sensor_vm.js", &script).unwrap();
    println!("[dump] wrote /tmp/adidas_sensor_vm.js");
}

fn extract_script_src(html: &str) -> Option<String> {
    // Look for: <script ... src="/PATH?v=...">
    let start = html.find("<script")?;
    let rest = &html[start..];
    let src_idx = rest.find("src=\"")?;
    let tail = &rest[src_idx + 5..];
    let end = tail.find('"')?;
    let raw = &tail[..end];
    // Trim query string? We want the full URL including the v= param.
    Some(raw.replace("&amp;", "&"))
}
