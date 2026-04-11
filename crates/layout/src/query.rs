use serde::Serialize;

/// A DOMRect returned by getBoundingClientRect().
#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct DOMRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl DOMRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            top: y,
            right: x + width,
            bottom: y + height,
            left: x,
        }
    }

    pub fn from_taffy_layout(layout: &taffy::Layout) -> Self {
        let x = layout.location.x as f64;
        let y = layout.location.y as f64;
        let w = layout.size.width as f64;
        let h = layout.size.height as f64;
        Self::new(x, y, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dom_rect_new() {
        let r = DOMRect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 20.0);
        assert_eq!(r.width, 100.0);
        assert_eq!(r.height, 50.0);
        assert_eq!(r.top, 20.0);
        assert_eq!(r.right, 110.0);
        assert_eq!(r.bottom, 70.0);
        assert_eq!(r.left, 10.0);
    }
}
