/// CSS `font-family` values.
#[derive(Debug, Clone, PartialEq)]
pub enum FontFamily {
    Named(String),
    Generic(GenericFamily),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    SystemUi,
    UiSerif,
    UiSansSerif,
    UiMonospace,
    UiRounded,
    Emoji,
    Math,
    Fangsong,
}

/// CSS `font-weight` values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontWeight {
    Numeric(f64),
    Normal, // 400
    Bold,   // 700
    Bolder,
    Lighter,
}

/// CSS `font-style` values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique(Option<f64>), // angle in degrees
}
