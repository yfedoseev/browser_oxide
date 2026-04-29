//! Global font database backed by `fontdb`.
//!
//! Loads the bundled TTF faces at startup (once per process) and
//! exposes a `query()` that maps a CSS font-family request + weight +
//! italic to a face ID plus the raw face bytes. Chrome-style family
//! aliases (Arial → Liberation Sans, Times → Liberation Serif, etc.)
//! are set up so a site asking for Arial gets plausible Liberation
//! Sans metrics rather than the DejaVu Sans fallback.
//!
//! The database is built from in-binary font data only — NEVER from
//! the host's system fonts — so output is reproducible across
//! machines and matches the StealthProfile's nominal Chrome-on-X
//! behaviour rather than the developer's laptop.

use fontdb::{Database, Family, Query, Stretch, Style, Weight, ID};
use std::sync::OnceLock;

pub struct FontDatabase {
    inner: Database,
}

/// Bundled font data. `include_bytes!` keeps these in the final binary
/// so the fingerprint is hermetic — no filesystem lookups at runtime.
///
/// The bundled set covers all four Latin style combinations (regular,
/// bold, italic, bold-italic) for sans, serif, and monospace — twelve
/// Liberation faces total, mirroring what Linux Chrome gets from
/// fontconfig. DejaVu Sans is kept as the ultimate-fallback face, and
/// Noto Sans Regular handles Cyrillic / Greek / extended Latin
/// coverage so measurements of Russian / Greek strings don't fall off
/// the Liberation glyph set.
const LIBERATION_SANS_REGULAR: &[u8] = include_bytes!("../../fonts/LiberationSans-Regular.ttf");
const LIBERATION_SANS_BOLD: &[u8] = include_bytes!("../../fonts/LiberationSans-Bold.ttf");
const LIBERATION_SANS_ITALIC: &[u8] = include_bytes!("../../fonts/LiberationSans-Italic.ttf");
const LIBERATION_SANS_BOLD_ITALIC: &[u8] =
    include_bytes!("../../fonts/LiberationSans-BoldItalic.ttf");
const LIBERATION_SERIF_REGULAR: &[u8] = include_bytes!("../../fonts/LiberationSerif-Regular.ttf");
const LIBERATION_SERIF_BOLD: &[u8] = include_bytes!("../../fonts/LiberationSerif-Bold.ttf");
const LIBERATION_SERIF_ITALIC: &[u8] = include_bytes!("../../fonts/LiberationSerif-Italic.ttf");
const LIBERATION_SERIF_BOLD_ITALIC: &[u8] =
    include_bytes!("../../fonts/LiberationSerif-BoldItalic.ttf");
const LIBERATION_MONO_REGULAR: &[u8] = include_bytes!("../../fonts/LiberationMono-Regular.ttf");
const LIBERATION_MONO_BOLD: &[u8] = include_bytes!("../../fonts/LiberationMono-Bold.ttf");
const LIBERATION_MONO_ITALIC: &[u8] = include_bytes!("../../fonts/LiberationMono-Italic.ttf");
const LIBERATION_MONO_BOLD_ITALIC: &[u8] =
    include_bytes!("../../fonts/LiberationMono-BoldItalic.ttf");
const DEJAVU_SANS: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
const NOTO_SANS_REGULAR: &[u8] = include_bytes!("../../fonts/NotoSans-Regular.ttf");

/// Number of faces we bundle and advertise via `document.fonts.size`.
/// Keep in sync with the `load_font_data` calls in `init_bundled`.
pub const BUNDLED_FACE_COUNT: usize = 14;

impl FontDatabase {
    pub fn get() -> &'static FontDatabase {
        static INSTANCE: OnceLock<FontDatabase> = OnceLock::new();
        INSTANCE.get_or_init(Self::init_bundled)
    }

    fn init_bundled() -> FontDatabase {
        let mut db = Database::new();
        // Load bundled faces as binary sources so fontdb can hand back
        // the raw bytes later via `face()`. The order matters for
        // default resolution — the first matching face wins when
        // multiple families carry the same name.
        db.load_font_data(LIBERATION_SANS_REGULAR.to_vec());
        db.load_font_data(LIBERATION_SANS_BOLD.to_vec());
        db.load_font_data(LIBERATION_SANS_ITALIC.to_vec());
        db.load_font_data(LIBERATION_SANS_BOLD_ITALIC.to_vec());
        db.load_font_data(LIBERATION_SERIF_REGULAR.to_vec());
        db.load_font_data(LIBERATION_SERIF_BOLD.to_vec());
        db.load_font_data(LIBERATION_SERIF_ITALIC.to_vec());
        db.load_font_data(LIBERATION_SERIF_BOLD_ITALIC.to_vec());
        db.load_font_data(LIBERATION_MONO_REGULAR.to_vec());
        db.load_font_data(LIBERATION_MONO_BOLD.to_vec());
        db.load_font_data(LIBERATION_MONO_ITALIC.to_vec());
        db.load_font_data(LIBERATION_MONO_BOLD_ITALIC.to_vec());
        db.load_font_data(DEJAVU_SANS.to_vec());
        db.load_font_data(NOTO_SANS_REGULAR.to_vec());

        // Chrome-on-Linux family aliases. When a site asks for Arial
        // (which is a Microsoft-licensed face we don't bundle), Linux
        // Chrome falls back to Liberation Sans under the hood. Matching
        // that alias chain here means `measureText("W", "16px Arial")`
        // gets realistic widths out of the box.
        //
        // Chrome's `sans-serif` / `serif` / `monospace` generics also
        // route through fontconfig on Linux to Liberation. We mirror
        // that explicitly rather than trusting fontdb's defaults.
        db.set_sans_serif_family("Liberation Sans");
        db.set_serif_family("Liberation Serif");
        db.set_monospace_family("Liberation Mono");
        db.set_cursive_family("Liberation Sans");
        db.set_fantasy_family("Liberation Sans");

        FontDatabase { inner: db }
    }

    /// Look up a face by family name + weight + italic style. Uses
    /// fontdb's selection algorithm which includes weight-closeness and
    /// style-closeness fallbacks.
    pub fn query(&self, family: &str, weight: u16, italic: bool) -> Option<ID> {
        let style = if italic { Style::Italic } else { Style::Normal };
        let families = resolve_family(family);
        let query = Query {
            families: &families,
            weight: Weight(weight),
            stretch: Stretch::Normal,
            style,
        };
        if let Some(id) = self.inner.query(&query) {
            return Some(id);
        }
        // Final fallback: sans-serif generic. We configured this to
        // point at Liberation Sans above so a misspelled family or an
        // exotic one still renders something plausible.
        let fallback = [Family::SansSerif];
        self.inner.query(&Query {
            families: &fallback,
            weight: Weight(weight),
            stretch: Stretch::Normal,
            style,
        })
    }

    /// Strict per-family lookup that does NOT fall back to sans-serif.
    /// Used by `query_chain` so the user-supplied fallback chain is
    /// honoured — `("Wingdings", "serif")` must reach `serif` instead of
    /// short-circuiting on Wingdings's outer fallback.
    fn query_strict(&self, family: &str, weight: u16, italic: bool) -> Option<ID> {
        let style = if italic { Style::Italic } else { Style::Normal };
        let families = resolve_family(family);
        self.inner.query(&Query {
            families: &families,
            weight: Weight(weight),
            stretch: Stretch::Normal,
            style,
        })
    }

    /// First-match query across a family fallback chain. Tries each
    /// user-specified family in order; only after the entire chain is
    /// exhausted does the global sans-serif fallback fire.
    pub fn query_chain(&self, families: &[String], weight: u16, italic: bool) -> Option<ID> {
        for fam in families {
            if let Some(id) = self.query_strict(fam, weight, italic) {
                return Some(id);
            }
        }
        // Whole chain unresolvable → final fallback to sans-serif so
        // shaping still renders something plausible (e.g. all-emoji
        // strings on a build without an emoji face).
        let style = if italic { Style::Italic } else { Style::Normal };
        let fallback = [Family::SansSerif];
        self.inner.query(&Query {
            families: &fallback,
            weight: Weight(weight),
            stretch: Stretch::Normal,
            style,
        })
    }

    /// Return the raw face bytes + face index for a given face ID.
    /// Returns `None` for faces backed by a file source (we only ever
    /// load binary sources, so this is effectively infallible, but the
    /// `fontdb::Source` enum forces us to handle both).
    pub fn face_data(&self, id: ID) -> Option<(&[u8], u32)> {
        let face = self.inner.face(id)?;
        // `init_bundled` only loads binary sources, so `face.source`
        // is always `Binary`. The destructure is still necessary to
        // extract the Arc'd bytes.
        let fontdb::Source::Binary(data) = &face.source;
        // `data` is an `Arc<dyn AsRef<[u8]> + Send + Sync>`. The Arc's
        // contents are immutable for the process lifetime, so the
        // slice is safe to hand out with the database's lifetime.
        let slice: &[u8] = (**data).as_ref();
        Some((slice, face.index))
    }
}

/// Resolve a CSS family name to a list of fontdb `Family` entries,
/// handling generic keywords and Chrome-style aliases.
///
/// Unknown families return ONLY the literal name — no implicit
/// `Family::SansSerif` fallback. The outer fallback path (in `query`)
/// catches the case where the user-supplied chain is fully
/// unresolvable; `query_chain` relies on the per-family failure to
/// proceed to the user's next explicit fallback (e.g. `"Wingdings",
/// serif` should resolve to a serif face, not silently to sans-serif).
/// Without this, Akamai-style font detection probes
/// (`Math.abs(measure("X, serif") - measure("serif")) > 0`) would
/// register every unknown family as installed.
fn resolve_family(name: &str) -> Vec<Family<'_>> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "sans-serif" => vec![Family::SansSerif],
        "serif" => vec![Family::Serif],
        "monospace" => vec![Family::Monospace],
        "cursive" => vec![Family::Cursive],
        "fantasy" => vec![Family::Fantasy],
        // Chrome substitution table for families we don't bundle. The
        // SansSerif tail keeps the ALIASED name resolvable even if the
        // bundled face name changes; the unknown-name branch below has
        // no such fallback.
        "arial" | "helvetica" | "helvetica neue" => {
            vec![Family::Name("Liberation Sans"), Family::SansSerif]
        }
        "times" | "times new roman" => vec![Family::Name("Liberation Serif"), Family::Serif],
        "courier" | "courier new" => vec![Family::Name("Liberation Mono"), Family::Monospace],
        _ => vec![Family::Name(name)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_sans_serif_generic() {
        let db = FontDatabase::get();
        assert!(db.query("sans-serif", 400, false).is_some());
    }

    #[test]
    fn resolves_arial_alias_to_liberation_sans() {
        let db = FontDatabase::get();
        let arial_id = db.query("Arial", 400, false).expect("Arial should resolve");
        let libsans_id = db
            .query("Liberation Sans", 400, false)
            .expect("Liberation Sans should resolve");
        assert_eq!(
            arial_id, libsans_id,
            "Arial alias should map to the bundled Liberation Sans face"
        );
    }

    #[test]
    fn resolves_times_alias_to_liberation_serif() {
        let db = FontDatabase::get();
        let id = db
            .query("Times New Roman", 400, false)
            .expect("Times should resolve");
        let libserif_id = db
            .query("Liberation Serif", 400, false)
            .expect("Liberation Serif should resolve");
        assert_eq!(id, libserif_id);
    }

    #[test]
    fn resolves_bold_weight() {
        let db = FontDatabase::get();
        let reg = db.query("Arial", 400, false).unwrap();
        let bold = db.query("Arial", 700, false).unwrap();
        assert_ne!(
            reg, bold,
            "regular and bold Arial should map to different faces"
        );
    }

    #[test]
    fn fallback_when_family_unknown() {
        let db = FontDatabase::get();
        // Should still return something via the sans-serif fallback.
        assert!(db.query("NonexistentFamily42", 400, false).is_some());
    }

    #[test]
    fn face_data_returns_bytes() {
        let db = FontDatabase::get();
        let id = db.query("Arial", 400, false).unwrap();
        let (bytes, _idx) = db.face_data(id).expect("bundled face has binary source");
        assert!(
            bytes.len() > 1000,
            "face bytes suspiciously small: {}",
            bytes.len()
        );
        // TTF magic is either 0x00010000 (TrueType) or "OTTO" (CFF).
        assert!(
            matches!(&bytes[0..4], b"\x00\x01\x00\x00" | b"OTTO" | b"true"),
            "not a TTF: first 4 bytes = {:?}",
            &bytes[0..4]
        );
    }

    #[test]
    fn resolves_italic_sans() {
        let db = FontDatabase::get();
        let reg = db.query("Arial", 400, false).unwrap();
        let italic = db.query("Arial", 400, true).unwrap();
        assert_ne!(
            reg, italic,
            "italic Arial should map to Liberation Sans Italic"
        );
    }

    #[test]
    fn resolves_bold_italic_serif() {
        let db = FontDatabase::get();
        let reg = db.query("Times New Roman", 400, false).unwrap();
        let bi = db.query("Times New Roman", 700, true).unwrap();
        assert_ne!(
            reg, bi,
            "bold-italic Times should map to Liberation Serif Bold Italic"
        );
    }

    #[test]
    fn bundled_face_count_matches_advertised() {
        // Sanity check against the constant document.fonts.size reports.
        let db = FontDatabase::get();
        let loaded = db.inner.faces().count();
        assert_eq!(
            loaded, BUNDLED_FACE_COUNT,
            "fontdb loaded {} faces, advertised {}",
            loaded, BUNDLED_FACE_COUNT
        );
    }

    #[test]
    fn query_chain_walks_fallback() {
        let db = FontDatabase::get();
        let chain = vec![
            "NonexistentA".to_string(),
            "NonexistentB".to_string(),
            "Arial".to_string(),
        ];
        let id = db.query_chain(&chain, 400, false).unwrap();
        let arial_id = db.query("Arial", 400, false).unwrap();
        assert_eq!(id, arial_id);
    }
}
