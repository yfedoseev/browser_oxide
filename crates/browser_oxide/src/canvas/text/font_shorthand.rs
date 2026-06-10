//! Minimal CSS `font` shorthand parser for Canvas 2D `ctx.font = ...`.
//!
//! Supports the subset Canvas / CSS specs require in practice:
//!
//! ```text
//! [ <font-style> || <font-weight> ]?  <font-size>[/<line-height>]?  <font-family>[, <family>]*
//! ```
//!
//! Deliberately scoped to the things websites actually set on a canvas
//! context — no stretch, no variant, no system fonts. Missing features
//! fall back to default values rather than erroring so a malformed
//! `ctx.font` string still produces a usable ParsedFont (matching
//! Chrome's forgiving behaviour).

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFont {
    /// CSS font-weight in the 100..900 range. Bold = 700, normal = 400.
    pub weight: u16,
    pub italic: bool,
    pub size_px: f32,
    /// Family fallback chain in request order. Each entry is the raw
    /// family name as typed (unquoted) — the font database handles
    /// normalisation and aliasing.
    pub families: Vec<String>,
}

impl ParsedFont {
    /// Default matches Canvas 2D spec: `10px sans-serif`.
    pub fn default_font() -> Self {
        Self {
            weight: 400,
            italic: false,
            size_px: 10.0,
            families: vec!["sans-serif".to_string()],
        }
    }

    /// Parse a `ctx.font` string. Returns `None` only for inputs that
    /// lack any recognisable size token; everything else falls back to
    /// defaults for missing pieces.
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut parsed = Self::default_font();

        // Tokenise up to the first family-list segment. We walk characters
        // left-to-right, collecting space-separated tokens, but treat
        // quoted strings as single tokens and treat a comma as a
        // family-list terminator.
        let (prefix_tokens, family_segment) = split_family_list(trimmed);

        // The families segment is everything after the first recognised
        // size token — but we haven't found that yet. For now, treat
        // `family_segment` as the family list candidate and walk
        // `prefix_tokens` to find the size token and any style/weight.
        let mut size_found = false;
        for tok in &prefix_tokens {
            if size_found {
                // After size_found, everything else belongs to the family
                // segment and should have been captured already. Ignore.
                continue;
            }
            if let Some(size) = parse_size_token(tok) {
                parsed.size_px = size;
                size_found = true;
                continue;
            }
            // Style / weight tokens — case-insensitive.
            let lower = tok.to_ascii_lowercase();
            match lower.as_str() {
                "italic" | "oblique" => parsed.italic = true,
                "normal" => {} // no-op
                "bold" | "bolder" => parsed.weight = 700,
                "lighter" => parsed.weight = 300,
                _ => {
                    // Numeric weight (100..900 in 100 steps).
                    if let Ok(w) = lower.parse::<u16>() {
                        if (100..=900).contains(&w) {
                            parsed.weight = w;
                        }
                    }
                }
            }
        }

        if !size_found {
            return None;
        }

        let mut families = parse_family_list(&family_segment);
        if families.is_empty() {
            families.push("sans-serif".to_string());
        }
        parsed.families = families;
        Some(parsed)
    }
}

/// Parse a single size token. Returns the size in CSS pixels.
///
/// Supports: `14px`, `14.5px`, `12pt` (converted to 16px), `1em` (16px),
/// `150%` (15px). Absolute keywords and other units fall through to
/// `None`.
fn parse_size_token(tok: &str) -> Option<f32> {
    // Strip an optional `/line-height` suffix (e.g. `14px/1.4`).
    let tok = tok.split('/').next()?;
    if tok.is_empty() {
        return None;
    }
    // Find the boundary between number and unit.
    let unit_start = tok.find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')?;
    let (num_str, unit) = tok.split_at(unit_start);
    let num: f32 = num_str.parse().ok()?;
    match unit {
        "px" => Some(num),
        "pt" => Some(num * 96.0 / 72.0),
        "em" | "rem" => Some(num * 16.0), // CSS default root font size
        "%" => Some(num * 16.0 / 100.0),
        _ => None,
    }
}

/// Split the input into `(prefix_tokens, family_segment)`.
///
/// The family segment starts at the first token that looks like a
/// family name (either quoted, or a bare word following the font-size
/// token). We don't know which token is the size until we scan, so we
/// take a permissive approach: the family segment is everything from
/// the first comma-or-space that appears AFTER a size-looking token.
///
/// For inputs with no commas (e.g. `"14px Arial"`) the last
/// space-separated token is the family. For inputs with commas (e.g.
/// `"14px Arial, Helvetica, sans-serif"`) the family segment starts
/// immediately after the size token.
fn split_family_list(input: &str) -> (Vec<String>, String) {
    // Single-pass split: find the size token, everything before = prefix,
    // everything after = family segment.
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    let mut token_start = 0usize;

    for (i, c) in input.char_indices() {
        if let Some(q) = in_quote {
            if c == q {
                in_quote = None;
            } else {
                current.push(c);
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                in_quote = Some(c);
            }
            ' ' | '\t' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                token_start = i + 1;
            }
            ',' => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                // A comma terminates the prefix scan — everything from
                // `token_start` onwards is the family list.
                let family_segment = input[token_start..].to_string();
                return (tokens, family_segment);
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // No comma. Walk tokens and split at the token AFTER the first size
    // token. Everything before (inclusive of size) = prefix; everything
    // after = family list (space-joined for parse_family_list).
    let mut prefix = Vec::new();
    let mut family_tokens: Vec<String> = Vec::new();
    let mut seen_size = false;
    for tok in tokens {
        if seen_size {
            family_tokens.push(tok);
        } else if parse_size_token(&tok).is_some() {
            prefix.push(tok);
            seen_size = true;
        } else {
            prefix.push(tok);
        }
    }
    let family_segment = family_tokens.join(" ");
    (prefix, family_segment)
}

/// Parse a comma-separated family list like
/// `"Arial, 'Liberation Sans', sans-serif"`.
fn parse_family_list(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;
    for c in s.chars() {
        match in_quote {
            Some(q) => {
                if c == q {
                    in_quote = None;
                } else {
                    current.push(c);
                }
            }
            None => match c {
                '"' | '\'' => in_quote = Some(c),
                ',' => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        out.push(trimmed);
                    }
                    current.clear();
                }
                _ => current.push(c),
            },
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple() {
        let p = ParsedFont::parse("14px Arial").unwrap();
        assert_eq!(p.size_px, 14.0);
        assert_eq!(p.weight, 400);
        assert!(!p.italic);
        assert_eq!(p.families, vec!["Arial".to_string()]);
    }

    #[test]
    fn parses_bold_italic() {
        let p = ParsedFont::parse("bold italic 16px 'Times New Roman'").unwrap();
        assert_eq!(p.weight, 700);
        assert!(p.italic);
        assert_eq!(p.size_px, 16.0);
        assert_eq!(p.families, vec!["Times New Roman".to_string()]);
    }

    #[test]
    fn parses_multi_family_with_commas() {
        let p = ParsedFont::parse("14px Arial, Helvetica, sans-serif").unwrap();
        assert_eq!(p.size_px, 14.0);
        assert_eq!(
            p.families,
            vec![
                "Arial".to_string(),
                "Helvetica".to_string(),
                "sans-serif".to_string()
            ]
        );
    }

    #[test]
    fn parses_numeric_weight() {
        let p = ParsedFont::parse("500 12px Arial").unwrap();
        assert_eq!(p.weight, 500);
    }

    #[test]
    fn parses_point_size_converted_to_px() {
        let p = ParsedFont::parse("12pt Arial").unwrap();
        assert!(
            (p.size_px - 16.0).abs() < 0.01,
            "12pt → 16px, got {}",
            p.size_px
        );
    }

    #[test]
    fn parses_quoted_family_with_spaces() {
        let p = ParsedFont::parse("14px \"Liberation Sans\"").unwrap();
        assert_eq!(p.families, vec!["Liberation Sans".to_string()]);
    }

    #[test]
    fn parses_line_height_suffix() {
        let p = ParsedFont::parse("14px/1.4 Arial").unwrap();
        assert_eq!(p.size_px, 14.0);
        assert_eq!(p.families, vec!["Arial".to_string()]);
    }

    #[test]
    fn missing_size_is_error() {
        assert!(ParsedFont::parse("bold italic Arial").is_none());
    }

    #[test]
    fn empty_is_error() {
        assert!(ParsedFont::parse("").is_none());
        assert!(ParsedFont::parse("   ").is_none());
    }

    #[test]
    fn default_values() {
        let d = ParsedFont::default_font();
        assert_eq!(d.size_px, 10.0);
        assert_eq!(d.weight, 400);
        assert!(!d.italic);
        assert_eq!(d.families, vec!["sans-serif".to_string()]);
    }
}
