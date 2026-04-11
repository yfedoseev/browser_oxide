//! A Path2D that accumulates drawing commands, then converts to `skia_safe::Path`.
//!
//! Kept as a command queue rather than mutating a Skia path directly so that
//! `Path2D` remains cheaply cloneable (JS `new Path2D(other)`) and so tests can
//! inspect the command list without depending on Skia.

use skia_safe::{Path as SkPath, PathBuilder as SkPathBuilder, Rect as SkRect};

#[derive(Debug, Clone, Default)]
pub struct Path2D {
    commands: Vec<PathCommand>,
}

#[derive(Debug, Clone)]
enum PathCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    BezierCurveTo(f32, f32, f32, f32, f32, f32),
    QuadraticCurveTo(f32, f32, f32, f32),
    Arc(f32, f32, f32, f32, f32, bool),
    Rect(f32, f32, f32, f32),
    ClosePath,
}

impl Path2D {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        self.commands.push(PathCommand::MoveTo(x, y));
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        self.commands.push(PathCommand::LineTo(x, y));
    }

    pub fn bezier_curve_to(&mut self, cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) {
        self.commands
            .push(PathCommand::BezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y));
    }

    pub fn quadratic_curve_to(&mut self, cpx: f32, cpy: f32, x: f32, y: f32) {
        self.commands
            .push(PathCommand::QuadraticCurveTo(cpx, cpy, x, y));
    }

    pub fn arc(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
        counter_clockwise: bool,
    ) {
        self.commands.push(PathCommand::Arc(
            x,
            y,
            radius,
            start_angle,
            end_angle,
            counter_clockwise,
        ));
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.commands.push(PathCommand::Rect(x, y, w, h));
    }

    pub fn close_path(&mut self) {
        self.commands.push(PathCommand::ClosePath);
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Build a `skia_safe::Path` from the accumulated commands.
    /// Returns `None` if the command list is empty.
    pub fn to_skia_path(&self) -> Option<SkPath> {
        if self.commands.is_empty() {
            return None;
        }

        let mut builder = SkPathBuilder::new();
        for cmd in &self.commands {
            match cmd {
                PathCommand::MoveTo(x, y) => {
                    builder.move_to((*x, *y));
                }
                PathCommand::LineTo(x, y) => {
                    builder.line_to((*x, *y));
                }
                PathCommand::BezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y) => {
                    builder.cubic_to((*cp1x, *cp1y), (*cp2x, *cp2y), (*x, *y));
                }
                PathCommand::QuadraticCurveTo(cpx, cpy, x, y) => {
                    builder.quad_to((*cpx, *cpy), (*x, *y));
                }
                PathCommand::Arc(cx, cy, r, start, end, ccw) => {
                    append_arc(&mut builder, *cx, *cy, *r, *start, *end, *ccw);
                }
                PathCommand::Rect(x, y, w, h) => {
                    builder.add_rect(SkRect::from_xywh(*x, *y, *w, *h), None, None);
                }
                PathCommand::ClosePath => {
                    builder.close();
                }
            }
        }
        Some(builder.detach())
    }
}

/// Append a circular arc to the path builder.
///
/// Canvas 2D's `arc(x, y, r, start, end, counterclockwise)` specifies angles
/// in radians measured clockwise from the positive x axis (y axis points
/// down in canvas space). Skia's `arc_to(oval, start, sweep, forceMoveTo)`
/// takes degrees, where positive sweep is also clockwise — so we just
/// convert and compute the sweep with the wrap-around semantics the HTML
/// spec requires.
fn append_arc(
    builder: &mut SkPathBuilder,
    cx: f32,
    cy: f32,
    r: f32,
    start: f32,
    end: f32,
    ccw: bool,
) {
    let oval = SkRect::from_ltrb(cx - r, cy - r, cx + r, cy + r);
    let tau = std::f32::consts::TAU;

    // Normalise start/end into a canonical sweep per the HTML Canvas spec.
    let start_deg = start.to_degrees();
    let mut sweep = end - start;
    if !ccw {
        // Clockwise: sweep must be positive and at most 2π.
        while sweep < 0.0 {
            sweep += tau;
        }
        if sweep > tau {
            sweep = tau;
        }
    } else {
        // Counter-clockwise: sweep must be negative.
        while sweep > 0.0 {
            sweep -= tau;
        }
        if sweep < -tau {
            sweep = -tau;
        }
    }
    let sweep_deg = sweep.to_degrees();

    // `false` force_move_to = continue the current contour from the implicit
    // start of the arc, matching Canvas 2D's behavior of connecting the
    // current path point to the arc start with a straight line.
    builder.arc_to(oval, start_deg, sweep_deg, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_path() {
        let mut p = Path2D::new();
        p.rect(0.0, 0.0, 100.0, 50.0);
        assert!(p.to_skia_path().is_some());
    }

    #[test]
    fn line_path() {
        let mut p = Path2D::new();
        p.move_to(0.0, 0.0);
        p.line_to(100.0, 100.0);
        assert!(p.to_skia_path().is_some());
    }

    #[test]
    fn arc_path() {
        let mut p = Path2D::new();
        p.arc(50.0, 50.0, 25.0, 0.0, std::f32::consts::PI, false);
        assert!(p.to_skia_path().is_some());
    }

    #[test]
    fn empty_path_returns_none() {
        let p = Path2D::new();
        assert!(p.to_skia_path().is_none());
    }
}
