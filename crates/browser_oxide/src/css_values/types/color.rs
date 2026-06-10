/// A CSS color value.
#[derive(Debug, Clone, PartialEq)]
pub enum Color {
    Rgba {
        r: u8,
        g: u8,
        b: u8,
        a: f32,
    },
    Hsl {
        h: f64,
        s: f64,
        l: f64,
        a: f32,
    },
    Oklch {
        l: f64,
        c: f64,
        h: f64,
        a: f32,
    },
    Oklab {
        l: f64,
        a_axis: f64,
        b_axis: f64,
        alpha: f32,
    },
    Lab {
        l: f64,
        a_axis: f64,
        b_axis: f64,
        alpha: f32,
    },
    Lch {
        l: f64,
        c: f64,
        h: f64,
        a: f32,
    },
    CurrentColor,
    Transparent,
}

impl Color {
    /// Get as RGBA (approximate conversion for non-sRGB colors).
    pub fn to_rgba(&self) -> (u8, u8, u8, f32) {
        match self {
            Color::Rgba { r, g, b, a } => (*r, *g, *b, *a),
            Color::Transparent => (0, 0, 0, 0.0),
            Color::CurrentColor => (0, 0, 0, 1.0), // placeholder
            _ => (0, 0, 0, 1.0),                   // TODO: implement color space conversions
        }
    }
}

/// Resolve a named CSS color. Returns None if not a valid name.
pub fn named_color(name: &str) -> Option<Color> {
    let rgba = |r, g, b| Some(Color::Rgba { r, g, b, a: 1.0 });

    match name.to_ascii_lowercase().as_str() {
        "transparent" => Some(Color::Transparent),
        "currentcolor" => Some(Color::CurrentColor),

        // CSS Level 1
        "black" => rgba(0, 0, 0),
        "silver" => rgba(192, 192, 192),
        "gray" | "grey" => rgba(128, 128, 128),
        "white" => rgba(255, 255, 255),
        "maroon" => rgba(128, 0, 0),
        "red" => rgba(255, 0, 0),
        "purple" => rgba(128, 0, 128),
        "fuchsia" | "magenta" => rgba(255, 0, 255),
        "green" => rgba(0, 128, 0),
        "lime" => rgba(0, 255, 0),
        "olive" => rgba(128, 128, 0),
        "yellow" => rgba(255, 255, 0),
        "navy" => rgba(0, 0, 128),
        "blue" => rgba(0, 0, 255),
        "teal" => rgba(0, 128, 128),
        "aqua" | "cyan" => rgba(0, 255, 255),

        // CSS Level 2+
        "orange" => rgba(255, 165, 0),
        "aliceblue" => rgba(240, 248, 255),
        "antiquewhite" => rgba(250, 235, 215),
        "aquamarine" => rgba(127, 255, 212),
        "azure" => rgba(240, 255, 255),
        "beige" => rgba(245, 245, 220),
        "bisque" => rgba(255, 228, 196),
        "blanchedalmond" => rgba(255, 235, 205),
        "blueviolet" => rgba(138, 43, 226),
        "brown" => rgba(165, 42, 42),
        "burlywood" => rgba(222, 184, 135),
        "cadetblue" => rgba(95, 158, 160),
        "chartreuse" => rgba(127, 255, 0),
        "chocolate" => rgba(210, 105, 30),
        "coral" => rgba(255, 127, 80),
        "cornflowerblue" => rgba(100, 149, 237),
        "cornsilk" => rgba(255, 248, 220),
        "crimson" => rgba(220, 20, 60),
        "darkblue" => rgba(0, 0, 139),
        "darkcyan" => rgba(0, 139, 139),
        "darkgoldenrod" => rgba(184, 134, 11),
        "darkgray" | "darkgrey" => rgba(169, 169, 169),
        "darkgreen" => rgba(0, 100, 0),
        "darkkhaki" => rgba(189, 183, 107),
        "darkmagenta" => rgba(139, 0, 139),
        "darkolivegreen" => rgba(85, 106, 47),
        "darkorange" => rgba(255, 140, 0),
        "darkorchid" => rgba(153, 50, 204),
        "darkred" => rgba(139, 0, 0),
        "darksalmon" => rgba(233, 150, 122),
        "darkseagreen" => rgba(143, 188, 143),
        "darkslateblue" => rgba(72, 61, 139),
        "darkslategray" | "darkslategrey" => rgba(47, 79, 79),
        "darkturquoise" => rgba(0, 206, 209),
        "darkviolet" => rgba(148, 0, 211),
        "deeppink" => rgba(255, 20, 147),
        "deepskyblue" => rgba(0, 191, 255),
        "dimgray" | "dimgrey" => rgba(105, 105, 105),
        "dodgerblue" => rgba(30, 144, 255),
        "firebrick" => rgba(178, 34, 34),
        "floralwhite" => rgba(255, 250, 240),
        "forestgreen" => rgba(34, 139, 34),
        "gainsboro" => rgba(220, 220, 220),
        "ghostwhite" => rgba(248, 248, 255),
        "gold" => rgba(255, 215, 0),
        "goldenrod" => rgba(218, 165, 32),
        "greenyellow" => rgba(173, 255, 47),
        "honeydew" => rgba(240, 255, 240),
        "hotpink" => rgba(255, 105, 180),
        "indianred" => rgba(205, 92, 92),
        "indigo" => rgba(75, 0, 130),
        "ivory" => rgba(255, 255, 240),
        "khaki" => rgba(240, 230, 140),
        "lavender" => rgba(230, 230, 250),
        "lavenderblush" => rgba(255, 240, 245),
        "lawngreen" => rgba(124, 252, 0),
        "lemonchiffon" => rgba(255, 250, 205),
        "lightblue" => rgba(173, 216, 230),
        "lightcoral" => rgba(240, 128, 128),
        "lightcyan" => rgba(224, 255, 255),
        "lightgoldenrodyellow" => rgba(250, 250, 210),
        "lightgray" | "lightgrey" => rgba(211, 211, 211),
        "lightgreen" => rgba(144, 238, 144),
        "lightpink" => rgba(255, 182, 193),
        "lightsalmon" => rgba(255, 160, 122),
        "lightseagreen" => rgba(32, 178, 170),
        "lightskyblue" => rgba(135, 206, 250),
        "lightslategray" | "lightslategrey" => rgba(119, 136, 153),
        "lightsteelblue" => rgba(176, 196, 222),
        "lightyellow" => rgba(255, 255, 224),
        "limegreen" => rgba(50, 205, 50),
        "linen" => rgba(250, 240, 230),
        "mediumaquamarine" => rgba(102, 205, 170),
        "mediumblue" => rgba(0, 0, 205),
        "mediumorchid" => rgba(186, 85, 211),
        "mediumpurple" => rgba(147, 111, 219),
        "mediumseagreen" => rgba(60, 179, 113),
        "mediumslateblue" => rgba(123, 104, 238),
        "mediumspringgreen" => rgba(0, 250, 154),
        "mediumturquoise" => rgba(72, 209, 204),
        "mediumvioletred" => rgba(199, 21, 133),
        "midnightblue" => rgba(25, 25, 112),
        "mintcream" => rgba(245, 255, 250),
        "mistyrose" => rgba(255, 228, 225),
        "moccasin" => rgba(255, 228, 181),
        "navajowhite" => rgba(255, 222, 173),
        "oldlace" => rgba(253, 245, 230),
        "olivedrab" => rgba(107, 142, 35),
        "orangered" => rgba(255, 69, 0),
        "orchid" => rgba(218, 112, 214),
        "palegoldenrod" => rgba(238, 232, 170),
        "palegreen" => rgba(152, 251, 152),
        "paleturquoise" => rgba(175, 238, 238),
        "palevioletred" => rgba(219, 112, 147),
        "papayawhip" => rgba(255, 239, 213),
        "peachpuff" => rgba(255, 218, 185),
        "peru" => rgba(205, 133, 63),
        "pink" => rgba(255, 192, 203),
        "plum" => rgba(221, 160, 221),
        "powderblue" => rgba(176, 224, 230),
        "rebeccapurple" => rgba(102, 51, 153),
        "rosybrown" => rgba(188, 143, 143),
        "royalblue" => rgba(65, 105, 225),
        "saddlebrown" => rgba(139, 69, 19),
        "salmon" => rgba(250, 128, 114),
        "sandybrown" => rgba(244, 164, 96),
        "seagreen" => rgba(46, 139, 87),
        "seashell" => rgba(255, 245, 238),
        "sienna" => rgba(160, 82, 45),
        "skyblue" => rgba(135, 206, 235),
        "slateblue" => rgba(106, 90, 205),
        "slategray" | "slategrey" => rgba(112, 128, 144),
        "snow" => rgba(255, 250, 250),
        "springgreen" => rgba(0, 255, 127),
        "steelblue" => rgba(70, 130, 180),
        "tan" => rgba(210, 180, 140),
        "thistle" => rgba(216, 191, 216),
        "tomato" => rgba(255, 99, 71),
        "turquoise" => rgba(64, 224, 208),
        "violet" => rgba(238, 130, 238),
        "wheat" => rgba(245, 222, 179),
        "whitesmoke" => rgba(245, 245, 245),
        "yellowgreen" => rgba(154, 205, 50),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_colors_work() {
        assert!(
            matches!(named_color("red"), Some(Color::Rgba { r: 255, g: 0, b: 0, a }) if a == 1.0)
        );
        assert!(matches!(
            named_color("RED"),
            Some(Color::Rgba {
                r: 255,
                g: 0,
                b: 0,
                ..
            })
        ));
        assert!(matches!(
            named_color("transparent"),
            Some(Color::Transparent)
        ));
        assert!(matches!(
            named_color("currentcolor"),
            Some(Color::CurrentColor)
        ));
        assert!(named_color("notacolor").is_none());
    }

    #[test]
    fn rebeccapurple() {
        assert!(matches!(
            named_color("rebeccapurple"),
            Some(Color::Rgba {
                r: 102,
                g: 51,
                b: 153,
                ..
            })
        ));
    }
}
