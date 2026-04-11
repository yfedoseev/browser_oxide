use css_values::types::length::*;

/// Context for resolving relative CSS units to absolute pixels.
pub struct ResolveContext {
    pub font_size: f32,      // current element's font-size in px
    pub root_font_size: f32, // <html> font-size in px (for rem)
    pub viewport_w: f32,     // viewport width in px
    pub viewport_h: f32,     // viewport height in px
}

impl Default for ResolveContext {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            root_font_size: 16.0,
            viewport_w: 1920.0,
            viewport_h: 1080.0,
        }
    }
}

/// Resolve a Length to absolute pixels.
pub fn resolve_length(length: &Length, ctx: &ResolveContext) -> f32 {
    match length {
        Length::Px(v) => *v as f32,
        Length::Em(v) => *v as f32 * ctx.font_size,
        Length::Rem(v) => *v as f32 * ctx.root_font_size,
        Length::Vw(v) => *v as f32 * ctx.viewport_w / 100.0,
        Length::Vh(v) => *v as f32 * ctx.viewport_h / 100.0,
        Length::Vmin(v) => *v as f32 * ctx.viewport_w.min(ctx.viewport_h) / 100.0,
        Length::Vmax(v) => *v as f32 * ctx.viewport_w.max(ctx.viewport_h) / 100.0,
        Length::Cm(v) => *v as f32 * 96.0 / 2.54,
        Length::Mm(v) => *v as f32 * 96.0 / 25.4,
        Length::In(v) => *v as f32 * 96.0,
        Length::Pt(v) => *v as f32 * 4.0 / 3.0,
        Length::Pc(v) => *v as f32 * 16.0,
        Length::Ch(v) => *v as f32 * ctx.font_size * 0.5, // approximate
        Length::Ex(v) => *v as f32 * ctx.font_size * 0.5, // approximate
        Length::Cqw(v) => *v as f32 * ctx.viewport_w / 100.0, // approximate
        Length::Cqh(v) => *v as f32 * ctx.viewport_h / 100.0, // approximate
        Length::Calc(_) => 0.0,                           // TODO: evaluate calc expressions
        Length::Zero => 0.0,
    }
}

/// Resolve a LengthPercentage. Percentage resolves against `reference_size`.
pub fn resolve_length_percentage(
    lp: &LengthPercentage,
    ctx: &ResolveContext,
    reference_size: f32,
) -> f32 {
    match lp {
        LengthPercentage::Length(l) => resolve_length(l, ctx),
        LengthPercentage::Percentage(p) => *p as f32 / 100.0 * reference_size,
        LengthPercentage::Calc(_) => 0.0,
    }
}

/// Resolve LengthPercentageAuto. Auto returns None.
pub fn resolve_length_percentage_auto(
    lpa: &LengthPercentageAuto,
    ctx: &ResolveContext,
    reference_size: f32,
) -> Option<f32> {
    match lpa {
        LengthPercentageAuto::Length(l) => Some(resolve_length(l, ctx)),
        LengthPercentageAuto::Percentage(p) => Some(*p as f32 / 100.0 * reference_size),
        LengthPercentageAuto::Auto => None,
        LengthPercentageAuto::Calc(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ResolveContext {
        ResolveContext::default()
    }

    #[test]
    fn resolve_px() {
        assert_eq!(resolve_length(&Length::Px(100.0), &ctx()), 100.0);
    }

    #[test]
    fn resolve_em() {
        let mut c = ctx();
        c.font_size = 20.0;
        assert_eq!(resolve_length(&Length::Em(2.0), &c), 40.0);
    }

    #[test]
    fn resolve_rem() {
        let mut c = ctx();
        c.root_font_size = 18.0;
        assert_eq!(resolve_length(&Length::Rem(1.5), &c), 27.0);
    }

    #[test]
    fn resolve_vw() {
        assert_eq!(resolve_length(&Length::Vw(50.0), &ctx()), 960.0);
    }

    #[test]
    fn resolve_vh() {
        assert_eq!(resolve_length(&Length::Vh(100.0), &ctx()), 1080.0);
    }

    #[test]
    fn resolve_percentage() {
        let lp = LengthPercentage::Percentage(50.0);
        assert_eq!(resolve_length_percentage(&lp, &ctx(), 200.0), 100.0);
    }

    #[test]
    fn resolve_auto() {
        let lpa = LengthPercentageAuto::Auto;
        assert_eq!(resolve_length_percentage_auto(&lpa, &ctx(), 200.0), None);
    }

    #[test]
    fn resolve_zero() {
        assert_eq!(resolve_length(&Length::Zero, &ctx()), 0.0);
    }
}
