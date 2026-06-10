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
    /// Canvas `arcTo(x1, y1, x2, y2, radius)` — tangent arc between
    /// the current point and (x2, y2) via control point (x1, y1).
    ArcTo(f32, f32, f32, f32, f32),
    /// Canvas `ellipse(x, y, rx, ry, rotation, startAngle, endAngle, ccw)`.
    Ellipse(f32, f32, f32, f32, f32, f32, f32, bool),
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

    pub fn arc_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) {
        self.commands
            .push(PathCommand::ArcTo(x1, y1, x2, y2, radius));
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "path builder takes many coordinate args; struct-wrapping adds churn without clarity"
    )]
    pub fn ellipse(
        &mut self,
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rotation: f32,
        start_angle: f32,
        end_angle: f32,
        counter_clockwise: bool,
    ) {
        self.commands.push(PathCommand::Ellipse(
            cx,
            cy,
            rx,
            ry,
            rotation,
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
                PathCommand::ArcTo(x1, y1, x2, y2, r) => {
                    builder.arc_to_tangent((*x1, *y1), (*x2, *y2), *r);
                }
                PathCommand::Ellipse(cx, cy, rx, ry, rot, start, end, ccw) => {
                    append_ellipse(&mut builder, *cx, *cy, *rx, *ry, *rot, *start, *end, *ccw);
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

/// Append a rotated ellipse arc using cubic-bezier approximation.
///
/// Canvas's `ellipse(cx, cy, rx, ry, rotation, startAngle, endAngle, ccw)`
/// is split into ⌈|sweep|/(π/2)⌉ bezier segments. For a unit circle, each
/// 90° quarter is approximated by a cubic with control-point magnitude
/// α = (4/3)·tan(θ/4) where θ is the segment sweep. Unit-circle endpoints
/// and tangents are then scaled by (rx, ry), rotated by `rotation`, and
/// translated by (cx, cy). This is the standard approximation Blink and
/// Servo use, and the output is byte-stable.
#[allow(
    clippy::too_many_arguments,
    reason = "path builder takes many coordinate args; struct-wrapping adds churn without clarity"
)]
fn append_ellipse(
    builder: &mut SkPathBuilder,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rotation: f32,
    start: f32,
    end: f32,
    ccw: bool,
) {
    let tau = std::f32::consts::TAU;
    let half_pi = std::f32::consts::FRAC_PI_2;

    let mut sweep = end - start;
    if !ccw {
        while sweep < 0.0 {
            sweep += tau;
        }
        if sweep > tau {
            sweep = tau;
        }
    } else {
        while sweep > 0.0 {
            sweep -= tau;
        }
        if sweep < -tau {
            sweep = -tau;
        }
    }

    let cos_rot = rotation.cos();
    let sin_rot = rotation.sin();
    // Map a unit-circle point (ux, uy) to ellipse coords.
    let map = |ux: f32, uy: f32| -> (f32, f32) {
        let sx = ux * rx;
        let sy = uy * ry;
        let rxp = sx * cos_rot - sy * sin_rot;
        let ryp = sx * sin_rot + sy * cos_rot;
        (cx + rxp, cy + ryp)
    };

    // First point: ensure the contour starts at the arc start.
    // Per Canvas spec, ellipse implicitly connects with a line from the
    // current point if there is one — Skia's default line_to-on-implicit-
    // start handles this when we line_to the first endpoint instead of
    // move_to. But conservatively emit line_to so we match spec semantics
    // even when this is the first command.
    let (sx0, sy0) = (start.cos(), start.sin());
    let (px0, py0) = map(sx0, sy0);
    builder.line_to((px0, py0));

    if sweep == 0.0 {
        return;
    }

    let n_segs = (sweep.abs() / half_pi).ceil().max(1.0) as i32;
    let seg_sweep = sweep / n_segs as f32;
    let alpha = (4.0_f32 / 3.0) * (seg_sweep / 4.0).tan();

    let mut a0 = start;
    for _ in 0..n_segs {
        let a1 = a0 + seg_sweep;
        let (cos0, sin0) = (a0.cos(), a0.sin());
        let (cos1, sin1) = (a1.cos(), a1.sin());
        // Unit-circle tangent at angle a is (-sin a, cos a).
        let cp0 = (cos0 - alpha * sin0, sin0 + alpha * cos0);
        let cp1 = (cos1 + alpha * sin1, sin1 - alpha * cos1);
        let p1 = (cos1, sin1);
        builder.cubic_to(map(cp0.0, cp0.1), map(cp1.0, cp1.1), map(p1.0, p1.1));
        a0 = a1;
    }
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

    #[test]
    fn arc_to_path() {
        let mut p = Path2D::new();
        p.move_to(20.0, 20.0);
        p.arc_to(80.0, 20.0, 80.0, 80.0, 20.0);
        assert!(p.to_skia_path().is_some());
    }

    #[test]
    fn ellipse_path() {
        let mut p = Path2D::new();
        p.ellipse(
            50.0,
            50.0,
            30.0,
            20.0,
            0.0,
            0.0,
            std::f32::consts::TAU,
            false,
        );
        assert!(p.to_skia_path().is_some());
    }

    #[test]
    fn ellipse_with_rotation() {
        let mut p = Path2D::new();
        p.ellipse(
            50.0,
            50.0,
            30.0,
            20.0,
            std::f32::consts::FRAC_PI_4,
            0.0,
            std::f32::consts::PI,
            false,
        );
        assert!(p.to_skia_path().is_some());
    }
}
