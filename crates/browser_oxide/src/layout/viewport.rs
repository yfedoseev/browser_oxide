/// Virtual viewport configuration.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub device_pixel_ratio: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1080.0,
            device_pixel_ratio: 1.0,
        }
    }
}

impl Viewport {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            device_pixel_ratio: 1.0,
        }
    }
}
