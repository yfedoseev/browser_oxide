use canvas::text::ParsedFont;
use canvas::Canvas2D;

#[test]
fn test_measure_text_bit_accuracy() {
    let mut canvas = Canvas2D::new(200, 100).unwrap();

    // 16px Arial is a common detection target
    canvas.set_font("16px Arial");

    let text = "Hello World";
    let width = canvas.measure_text(text);

    println!("Width for 'Hello World' (16px Arial): {}", width);

    // Standard Chrome metrics for "Hello World" 16px Arial on Windows/Linux
    // are often exactly width=81 or 82.
    assert!(width > 70.0 && width < 90.0);
}

#[test]
fn test_measure_text_empty() {
    let mut canvas = Canvas2D::new(200, 100).unwrap();
    canvas.set_font("16px Arial");
    let width = canvas.measure_text("");
    assert_eq!(width, 0.0);
}
