//! Verify our PNG output uses minimal chunk set matching Chrome's libpng.
//! Chrome emits exactly: signature + IHDR + IDAT + IEND.
//! Extra chunks like pHYs / tIME / tEXt would betray the engine.

use canvas::Canvas2D;

#[test]
fn png_uses_minimal_chunks() {
    let mut c = Canvas2D::new(100, 30, "Linux".to_string()).unwrap();
    c.set_fill_color(255, 100, 0, 1.0);
    c.fill_rect(10.0, 5.0, 80.0, 20.0);
    let png = c.to_png_bytes();

    // Parse chunks
    assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
    let mut chunks = Vec::new();
    let mut i = 8;
    while i < png.len() {
        let len = u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]) as usize;
        let ct = String::from_utf8_lossy(&png[i + 4..i + 8]).to_string();
        chunks.push(ct.clone());
        i += 12 + len;
    }
    eprintln!("PNG chunks: {:?}", chunks);
    // Chrome libpng: IHDR, IDAT, IEND (single IDAT).
    // Allowed: any non-ancillary chunks, but never pHYs/tIME/tEXt/iTXt.
    for c in &chunks {
        assert!(
            !["pHYs", "tIME", "tEXt", "iTXt", "zTXt", "bKGD", "cHRM", "sRGB", "gAMA"]
                .contains(&c.as_str()),
            "PNG must not emit metadata chunk: {c}"
        );
    }
    assert_eq!(chunks.first().map(|s| s.as_str()), Some("IHDR"));
    assert_eq!(chunks.last().map(|s| s.as_str()), Some("IEND"));
    assert!(chunks.iter().any(|c| c == "IDAT"));
}
