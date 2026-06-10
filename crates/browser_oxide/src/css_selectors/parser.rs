use crate::css_parser::{resolve_escapes, Token, TokenKind, Tokenizer};
use crate::css_selectors::ast::*;
use crate::css_selectors::error::SelectorParseError;
use crate::css_selectors::nth::parse_nth;

/// Parse a comma-separated selector list.
pub fn parse_selector_list(input: &str) -> Result<SelectorList, SelectorParseError> {
    let mut parser = SelectorParser::new(input);
    parser.parse_selector_list()
}

/// Parse a forgiving selector list (invalid selectors are dropped).
/// Used for `:is()`, `:where()`.
pub fn parse_selector_list_forgiving(input: &str) -> SelectorList {
    let mut parser = SelectorParser::new(input);
    parser.parse_selector_list_forgiving()
}

struct SelectorParser<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
}

impl<'a> SelectorParser<'a> {
    fn new(input: &'a str) -> Self {
        let tokens: Vec<Token<'a>> = Tokenizer::new(input).collect();
        Self { tokens, pos: 0 }
    }

    fn from_tokens(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn current_kind(&self) -> TokenKind<'a> {
        if self.pos < self.tokens.len() {
            self.tokens[self.pos].kind.clone()
        } else {
            TokenKind::Eof
        }
    }

    fn current_token(&self) -> Option<&Token<'a>> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.current_kind(), TokenKind::Whitespace) {
            self.advance();
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn parse_selector_list(&mut self) -> Result<SelectorList, SelectorParseError> {
        let mut selectors = Vec::new();
        self.skip_whitespace();

        if self.is_eof() {
            return Err(SelectorParseError::EmptySelector);
        }

        selectors.push(self.parse_complex_selector()?);

        loop {
            self.skip_whitespace();
            if self.is_eof() {
                break;
            }
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance();
                self.skip_whitespace();
                selectors.push(self.parse_complex_selector()?);
            } else {
                break;
            }
        }

        Ok(selectors)
    }

    fn parse_selector_list_forgiving(&mut self) -> SelectorList {
        let mut selectors = Vec::new();
        self.skip_whitespace();

        if self.is_eof() {
            return selectors;
        }

        if let Ok(sel) = self.try_parse_complex_selector() {
            selectors.push(sel);
        }

        loop {
            self.skip_whitespace();
            if self.is_eof() {
                break;
            }
            if matches!(self.current_kind(), TokenKind::Comma) {
                self.advance();
                self.skip_whitespace();
                if let Ok(sel) = self.try_parse_complex_selector() {
                    selectors.push(sel);
                } else {
                    // Skip tokens until next comma or end
                    while !self.is_eof() && !matches!(self.current_kind(), TokenKind::Comma) {
                        self.advance();
                    }
                }
            } else {
                break;
            }
        }

        selectors
    }

    fn try_parse_complex_selector(&mut self) -> Result<Selector, SelectorParseError> {
        let saved = self.pos;
        match self.parse_complex_selector() {
            Ok(s) => Ok(s),
            Err(e) => {
                self.pos = saved;
                Err(e)
            }
        }
    }

    fn parse_complex_selector(&mut self) -> Result<Selector, SelectorParseError> {
        // Parse left-to-right, then reverse for right-to-left storage
        let mut components = Vec::new();

        // First compound selector
        let compound = self.parse_compound_selector()?;
        components.extend(compound);

        // Followed by combinator + compound pairs
        loop {
            let had_whitespace = matches!(self.current_kind(), TokenKind::Whitespace);
            self.skip_whitespace();

            if self.is_eof()
                || matches!(
                    self.current_kind(),
                    TokenKind::Comma | TokenKind::CloseParen | TokenKind::CloseCurly
                )
            {
                break;
            }

            // Determine combinator
            let combinator = match self.current_kind() {
                TokenKind::Delim('>') => {
                    self.advance();
                    self.skip_whitespace();
                    Combinator::Child
                }
                TokenKind::Delim('+') => {
                    self.advance();
                    self.skip_whitespace();
                    Combinator::NextSibling
                }
                TokenKind::Delim('~') => {
                    self.advance();
                    self.skip_whitespace();
                    Combinator::SubsequentSibling
                }
                _ if had_whitespace => Combinator::Descendant,
                _ => break,
            };

            let compound = self.parse_compound_selector()?;
            components.push(Component::Combinator(combinator));
            components.extend(compound);
        }

        // Reverse for right-to-left matching
        components.reverse();

        let specificity = compute_specificity_from_components(&components);
        Ok(Selector::new(components, specificity))
    }

    fn parse_compound_selector(&mut self) -> Result<Vec<Component>, SelectorParseError> {
        let mut components = Vec::new();

        loop {
            match self.current_kind() {
                TokenKind::Ident(name) => {
                    let name = resolve_escapes(name).to_string();
                    components.push(Component::Simple(SimpleSelector::Type(name)));
                    self.advance();
                }
                TokenKind::Delim('*') => {
                    components.push(Component::Simple(SimpleSelector::Universal));
                    self.advance();
                }
                TokenKind::Delim('.') => {
                    self.advance();
                    match self.current_kind() {
                        TokenKind::Ident(name) => {
                            let name = resolve_escapes(name).to_string();
                            components.push(Component::Simple(SimpleSelector::Class(name)));
                            self.advance();
                        }
                        _ => {
                            return Err(SelectorParseError::UnexpectedToken {
                                loc: self.current_token().map(|t| t.loc).unwrap_or_default(),
                                message: "expected class name after '.'".into(),
                            });
                        }
                    }
                }
                TokenKind::Hash { value, is_id: true } => {
                    let value = resolve_escapes(value).to_string();
                    components.push(Component::Simple(SimpleSelector::Id(value)));
                    self.advance();
                }
                TokenKind::Hash { value, .. } => {
                    let value = resolve_escapes(value).to_string();
                    components.push(Component::Simple(SimpleSelector::Id(value)));
                    self.advance();
                }
                TokenKind::OpenSquare => {
                    components.push(Component::Simple(self.parse_attribute_selector()?));
                    // ] already consumed by parse_attribute_selector
                }
                TokenKind::Colon => {
                    self.advance();
                    if matches!(self.current_kind(), TokenKind::Colon) {
                        // Pseudo-element (::)
                        self.advance();
                        components.push(Component::Simple(self.parse_pseudo_element()?));
                    } else {
                        // Pseudo-class
                        components.push(Component::Simple(self.parse_pseudo_class()?));
                    }
                }
                TokenKind::Delim('&') => {
                    components.push(Component::Simple(SimpleSelector::Nesting));
                    self.advance();
                }
                _ => break,
            }
        }

        if components.is_empty() {
            return Err(SelectorParseError::EmptySelector);
        }

        Ok(components)
    }

    fn parse_attribute_selector(&mut self) -> Result<SimpleSelector, SelectorParseError> {
        self.advance(); // [
        self.skip_whitespace();

        let name = match self.current_kind() {
            TokenKind::Ident(n) => {
                let n = resolve_escapes(n).to_string();
                self.advance();
                n
            }
            _ => {
                return Err(SelectorParseError::UnexpectedToken {
                    loc: self.current_token().map(|t| t.loc).unwrap_or_default(),
                    message: "expected attribute name".into(),
                });
            }
        };

        self.skip_whitespace();

        // Check for ] (presence selector)
        if matches!(self.current_kind(), TokenKind::CloseSquare) {
            self.advance();
            return Ok(SimpleSelector::Attribute {
                name,
                operator: None,
                value: None,
                case_sensitivity: CaseSensitivity::Default,
            });
        }

        // Parse operator
        let operator = self.parse_attribute_operator()?;
        self.skip_whitespace();

        // Parse value
        let value = match self.current_kind() {
            TokenKind::String(s) => {
                let s = resolve_escapes(s).to_string();
                self.advance();
                s
            }
            TokenKind::Ident(s) => {
                let s = resolve_escapes(s).to_string();
                self.advance();
                s
            }
            _ => {
                return Err(SelectorParseError::UnexpectedToken {
                    loc: self.current_token().map(|t| t.loc).unwrap_or_default(),
                    message: "expected attribute value".into(),
                });
            }
        };

        self.skip_whitespace();

        // Check for case sensitivity flag (i or s)
        let case_sensitivity = match self.current_kind() {
            TokenKind::Ident(flag) if flag.eq_ignore_ascii_case("i") => {
                self.advance();
                self.skip_whitespace();
                CaseSensitivity::CaseInsensitive
            }
            TokenKind::Ident(flag) if flag.eq_ignore_ascii_case("s") => {
                self.advance();
                self.skip_whitespace();
                CaseSensitivity::CaseSensitive
            }
            _ => CaseSensitivity::Default,
        };

        // Consume ]
        if matches!(self.current_kind(), TokenKind::CloseSquare) {
            self.advance();
        }

        Ok(SimpleSelector::Attribute {
            name,
            operator: Some(operator),
            value: Some(value),
            case_sensitivity,
        })
    }

    fn parse_attribute_operator(&mut self) -> Result<AttributeOperator, SelectorParseError> {
        match self.current_kind() {
            TokenKind::Delim('=') => {
                self.advance();
                Ok(AttributeOperator::Exact)
            }
            TokenKind::Delim('~') => {
                self.advance();
                if matches!(self.current_kind(), TokenKind::Delim('=')) {
                    self.advance();
                }
                Ok(AttributeOperator::Includes)
            }
            TokenKind::Delim('|') => {
                self.advance();
                if matches!(self.current_kind(), TokenKind::Delim('=')) {
                    self.advance();
                }
                Ok(AttributeOperator::DashMatch)
            }
            TokenKind::Delim('^') => {
                self.advance();
                if matches!(self.current_kind(), TokenKind::Delim('=')) {
                    self.advance();
                }
                Ok(AttributeOperator::Prefix)
            }
            TokenKind::Delim('$') => {
                self.advance();
                if matches!(self.current_kind(), TokenKind::Delim('=')) {
                    self.advance();
                }
                Ok(AttributeOperator::Suffix)
            }
            TokenKind::Delim('*') => {
                self.advance();
                if matches!(self.current_kind(), TokenKind::Delim('=')) {
                    self.advance();
                }
                Ok(AttributeOperator::Substring)
            }
            _ => Err(SelectorParseError::UnexpectedToken {
                loc: self.current_token().map(|t| t.loc).unwrap_or_default(),
                message: "expected attribute operator".into(),
            }),
        }
    }

    fn parse_pseudo_class(&mut self) -> Result<SimpleSelector, SelectorParseError> {
        match self.current_kind() {
            TokenKind::Ident(name) => {
                let name_lower = name.to_ascii_lowercase();
                self.advance();
                let pc = match name_lower.as_str() {
                    "hover" => PseudoClass::Hover,
                    "active" => PseudoClass::Active,
                    "focus" => PseudoClass::Focus,
                    "focus-within" => PseudoClass::FocusWithin,
                    "focus-visible" => PseudoClass::FocusVisible,
                    "link" => PseudoClass::Link,
                    "visited" => PseudoClass::Visited,
                    "any-link" => PseudoClass::AnyLink,
                    "target" => PseudoClass::Target,
                    "enabled" => PseudoClass::Enabled,
                    "disabled" => PseudoClass::Disabled,
                    "checked" => PseudoClass::Checked,
                    "default" => PseudoClass::Default,
                    "indeterminate" => PseudoClass::Indeterminate,
                    "required" => PseudoClass::Required,
                    "optional" => PseudoClass::Optional,
                    "valid" => PseudoClass::Valid,
                    "invalid" => PseudoClass::Invalid,
                    "in-range" => PseudoClass::InRange,
                    "out-of-range" => PseudoClass::OutOfRange,
                    "read-write" => PseudoClass::ReadWrite,
                    "read-only" => PseudoClass::ReadOnly,
                    "placeholder-shown" => PseudoClass::PlaceholderShown,
                    "root" => PseudoClass::Root,
                    "empty" => PseudoClass::Empty,
                    "first-child" => PseudoClass::FirstChild,
                    "last-child" => PseudoClass::LastChild,
                    "only-child" => PseudoClass::OnlyChild,
                    "first-of-type" => PseudoClass::FirstOfType,
                    "last-of-type" => PseudoClass::LastOfType,
                    "only-of-type" => PseudoClass::OnlyOfType,
                    _ => {
                        return Err(SelectorParseError::UnsupportedPseudoClass(name_lower));
                    }
                };
                Ok(SimpleSelector::PseudoClass(pc))
            }
            TokenKind::Function(name) => {
                let name_lower = name.to_ascii_lowercase();
                self.advance(); // past function token (includes `(`)

                let result = match name_lower.as_str() {
                    "nth-child" => self.parse_nth_function(false, false),
                    "nth-last-child" => self.parse_nth_function(true, false),
                    "nth-of-type" => self.parse_nth_function(false, true),
                    "nth-last-of-type" => self.parse_nth_function(true, true),
                    "not" => self.parse_functional_pseudo(PseudoClass::Not),
                    "is" => self.parse_functional_pseudo_forgiving(PseudoClass::Is),
                    "where" => self.parse_functional_pseudo_forgiving(PseudoClass::Where),
                    "has" => self.parse_has_pseudo(),
                    "lang" => self.parse_lang_pseudo(),
                    _ => Err(SelectorParseError::UnsupportedPseudoClass(name_lower)),
                };

                // Consume closing )
                if matches!(self.current_kind(), TokenKind::CloseParen) {
                    self.advance();
                }

                Ok(SimpleSelector::PseudoClass(result?))
            }
            _ => Err(SelectorParseError::UnexpectedToken {
                loc: self.current_token().map(|t| t.loc).unwrap_or_default(),
                message: "expected pseudo-class name".into(),
            }),
        }
    }

    fn parse_nth_function(
        &mut self,
        from_end: bool,
        of_type: bool,
    ) -> Result<PseudoClass, SelectorParseError> {
        // Collect tokens until ) or "of"
        let mut nth_tokens = Vec::new();
        loop {
            match self.current_kind() {
                TokenKind::CloseParen | TokenKind::Eof => break,
                TokenKind::Ident(name) if name.eq_ignore_ascii_case("of") && !of_type => {
                    self.advance();
                    break;
                }
                _ => {
                    if let Some(t) = self.current_token() {
                        nth_tokens.push(t.clone());
                    }
                    self.advance();
                }
            }
        }

        let nth = parse_nth(&nth_tokens)?;

        if of_type {
            if from_end {
                Ok(PseudoClass::NthLastOfType(nth))
            } else {
                Ok(PseudoClass::NthOfType(nth))
            }
        } else {
            // Check for "of <selector-list>" (already consumed "of")
            self.skip_whitespace();
            let selector_list =
                if !matches!(self.current_kind(), TokenKind::CloseParen | TokenKind::Eof) {
                    // Parse remaining as selector list
                    let mut inner_tokens = Vec::new();
                    let mut depth = 0;
                    loop {
                        match self.current_kind() {
                            TokenKind::CloseParen if depth == 0 => break,
                            TokenKind::Eof => break,
                            TokenKind::OpenParen => {
                                depth += 1;
                                if let Some(t) = self.current_token() {
                                    inner_tokens.push(t.clone());
                                }
                                self.advance();
                            }
                            TokenKind::CloseParen => {
                                depth -= 1;
                                if let Some(t) = self.current_token() {
                                    inner_tokens.push(t.clone());
                                }
                                self.advance();
                            }
                            _ => {
                                if let Some(t) = self.current_token() {
                                    inner_tokens.push(t.clone());
                                }
                                self.advance();
                            }
                        }
                    }
                    let mut inner_parser = SelectorParser::from_tokens(inner_tokens);
                    Some(inner_parser.parse_selector_list().unwrap_or_default())
                } else {
                    None
                };

            if from_end {
                Ok(PseudoClass::NthLastChild(nth, selector_list))
            } else {
                Ok(PseudoClass::NthChild(nth, selector_list))
            }
        }
    }

    fn parse_functional_pseudo(
        &mut self,
        constructor: impl FnOnce(SelectorList) -> PseudoClass,
    ) -> Result<PseudoClass, SelectorParseError> {
        let inner_tokens = self.collect_until_close_paren();
        let mut inner_parser = SelectorParser::from_tokens(inner_tokens);
        let list = inner_parser.parse_selector_list()?;
        Ok(constructor(list))
    }

    fn parse_functional_pseudo_forgiving(
        &mut self,
        constructor: impl FnOnce(SelectorList) -> PseudoClass,
    ) -> Result<PseudoClass, SelectorParseError> {
        let inner_tokens = self.collect_until_close_paren();
        let mut inner_parser = SelectorParser::from_tokens(inner_tokens);
        let list = inner_parser.parse_selector_list_forgiving();
        Ok(constructor(list))
    }

    fn parse_has_pseudo(&mut self) -> Result<PseudoClass, SelectorParseError> {
        let inner_tokens = self.collect_until_close_paren();
        let mut inner_parser = SelectorParser::from_tokens(inner_tokens);
        let list = inner_parser.parse_selector_list()?;
        let relatives = list
            .into_iter()
            .map(|s| RelativeSelector {
                combinator: None,
                selector: s,
            })
            .collect();
        Ok(PseudoClass::Has(relatives))
    }

    fn parse_lang_pseudo(&mut self) -> Result<PseudoClass, SelectorParseError> {
        let mut langs = Vec::new();
        loop {
            self.skip_whitespace();
            match self.current_kind() {
                TokenKind::Ident(name) => {
                    langs.push(name.to_string());
                    self.advance();
                }
                TokenKind::String(s) => {
                    langs.push(s.to_string());
                    self.advance();
                }
                TokenKind::Comma => {
                    self.advance();
                }
                _ => break,
            }
        }
        Ok(PseudoClass::Lang(langs))
    }

    fn parse_pseudo_element(&mut self) -> Result<SimpleSelector, SelectorParseError> {
        match self.current_kind() {
            TokenKind::Ident(name) => {
                let name_lower = name.to_ascii_lowercase();
                self.advance();
                let pe = match name_lower.as_str() {
                    "before" => PseudoElement::Before,
                    "after" => PseudoElement::After,
                    "first-line" => PseudoElement::FirstLine,
                    "first-letter" => PseudoElement::FirstLetter,
                    "placeholder" => PseudoElement::Placeholder,
                    "selection" => PseudoElement::Selection,
                    _ => PseudoElement::Custom(name_lower),
                };
                Ok(SimpleSelector::PseudoElement(pe))
            }
            _ => Err(SelectorParseError::UnexpectedToken {
                loc: self.current_token().map(|t| t.loc).unwrap_or_default(),
                message: "expected pseudo-element name".into(),
            }),
        }
    }

    fn collect_until_close_paren(&mut self) -> Vec<Token<'a>> {
        let mut tokens = Vec::new();
        let mut depth = 0;
        loop {
            match self.current_kind() {
                TokenKind::CloseParen if depth == 0 => break,
                TokenKind::Eof => break,
                TokenKind::OpenParen => {
                    depth += 1;
                    if let Some(t) = self.current_token() {
                        tokens.push(t.clone());
                    }
                    self.advance();
                }
                TokenKind::CloseParen => {
                    depth -= 1;
                    if let Some(t) = self.current_token() {
                        tokens.push(t.clone());
                    }
                    self.advance();
                }
                _ => {
                    if let Some(t) = self.current_token() {
                        tokens.push(t.clone());
                    }
                    self.advance();
                }
            }
        }
        tokens
    }
}

fn compute_specificity_from_components(components: &[Component]) -> Specificity {
    let mut spec = Specificity::default();
    for c in components {
        if let Component::Simple(s) = c {
            spec += crate::css_selectors::specificity::simple_specificity_pub(s);
        }
    }
    spec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_type_selector() {
        let list = parse_selector_list("div").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0].components(),
            &[Component::Simple(SimpleSelector::Type("div".into()))]
        );
    }

    #[test]
    fn parse_class_selector() {
        let list = parse_selector_list(".foo").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0].components(),
            &[Component::Simple(SimpleSelector::Class("foo".into()))]
        );
    }

    #[test]
    fn parse_id_selector() {
        let list = parse_selector_list("#bar").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0].components(),
            &[Component::Simple(SimpleSelector::Id("bar".into()))]
        );
    }

    #[test]
    fn parse_universal() {
        let list = parse_selector_list("*").unwrap();
        assert_eq!(
            list[0].components(),
            &[Component::Simple(SimpleSelector::Universal)]
        );
    }

    #[test]
    fn parse_compound() {
        let list = parse_selector_list("div.foo#bar").unwrap();
        // Stored right-to-left, so reversed from parse order
        let comps = list[0].components();
        assert_eq!(comps.len(), 3);
        // Right-to-left: #bar, .foo, div
        assert!(matches!(&comps[0], Component::Simple(SimpleSelector::Id(s)) if s == "bar"));
        assert!(matches!(&comps[1], Component::Simple(SimpleSelector::Class(s)) if s == "foo"));
        assert!(matches!(&comps[2], Component::Simple(SimpleSelector::Type(s)) if s == "div"));
    }

    #[test]
    fn parse_descendant() {
        let list = parse_selector_list("div span").unwrap();
        let comps = list[0].components();
        // RTL: span, Descendant, div
        assert_eq!(comps.len(), 3);
        assert!(matches!(&comps[0], Component::Simple(SimpleSelector::Type(s)) if s == "span"));
        assert_eq!(comps[1], Component::Combinator(Combinator::Descendant));
        assert!(matches!(&comps[2], Component::Simple(SimpleSelector::Type(s)) if s == "div"));
    }

    #[test]
    fn parse_child_combinator() {
        let list = parse_selector_list("div > span").unwrap();
        let comps = list[0].components();
        assert_eq!(comps.len(), 3);
        assert!(matches!(&comps[0], Component::Simple(SimpleSelector::Type(s)) if s == "span"));
        assert_eq!(comps[1], Component::Combinator(Combinator::Child));
    }

    #[test]
    fn parse_next_sibling() {
        let list = parse_selector_list("h1 + p").unwrap();
        let comps = list[0].components();
        assert_eq!(comps[1], Component::Combinator(Combinator::NextSibling));
    }

    #[test]
    fn parse_subsequent_sibling() {
        let list = parse_selector_list("h1 ~ p").unwrap();
        let comps = list[0].components();
        assert_eq!(
            comps[1],
            Component::Combinator(Combinator::SubsequentSibling)
        );
    }

    #[test]
    fn parse_selector_list_comma() {
        let list = parse_selector_list("h1, h2, h3").unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn parse_attribute_presence() {
        let list = parse_selector_list("[href]").unwrap();
        assert!(matches!(
            &list[0].components()[0],
            Component::Simple(SimpleSelector::Attribute { name, operator: None, .. }) if name == "href"
        ));
    }

    #[test]
    fn parse_attribute_exact() {
        let list = parse_selector_list("[type=\"text\"]").unwrap();
        assert!(matches!(
            &list[0].components()[0],
            Component::Simple(SimpleSelector::Attribute {
                name,
                operator: Some(AttributeOperator::Exact),
                value: Some(v),
                ..
            }) if name == "type" && v == "text"
        ));
    }

    #[test]
    fn parse_attribute_case_insensitive() {
        let list = parse_selector_list("[type=\"text\" i]").unwrap();
        assert!(matches!(
            &list[0].components()[0],
            Component::Simple(SimpleSelector::Attribute {
                case_sensitivity: CaseSensitivity::CaseInsensitive,
                ..
            })
        ));
    }

    #[test]
    fn parse_pseudo_class_hover() {
        let list = parse_selector_list("a:hover").unwrap();
        let comps = list[0].components();
        assert!(matches!(
            &comps[0],
            Component::Simple(SimpleSelector::PseudoClass(PseudoClass::Hover))
        ));
    }

    #[test]
    fn parse_pseudo_first_child() {
        let list = parse_selector_list(":first-child").unwrap();
        assert!(matches!(
            &list[0].components()[0],
            Component::Simple(SimpleSelector::PseudoClass(PseudoClass::FirstChild))
        ));
    }

    #[test]
    fn parse_nth_child() {
        let list = parse_selector_list(":nth-child(2n+1)").unwrap();
        assert!(matches!(
            &list[0].components()[0],
            Component::Simple(SimpleSelector::PseudoClass(PseudoClass::NthChild(
                NthExpr { a: 2, b: 1 },
                None
            )))
        ));
    }

    #[test]
    fn parse_not() {
        let list = parse_selector_list(":not(.hidden)").unwrap();
        assert!(matches!(
            &list[0].components()[0],
            Component::Simple(SimpleSelector::PseudoClass(PseudoClass::Not(_)))
        ));
    }

    #[test]
    fn parse_is() {
        let list = parse_selector_list(":is(h1, h2, h3)").unwrap();
        if let Component::Simple(SimpleSelector::PseudoClass(PseudoClass::Is(inner))) =
            &list[0].components()[0]
        {
            assert_eq!(inner.len(), 3);
        } else {
            panic!("Expected :is()");
        }
    }

    #[test]
    fn parse_pseudo_element_before() {
        let list = parse_selector_list("p::before").unwrap();
        let comps = list[0].components();
        assert!(matches!(
            &comps[0],
            Component::Simple(SimpleSelector::PseudoElement(PseudoElement::Before))
        ));
    }

    #[test]
    fn parse_nesting_selector() {
        let list = parse_selector_list("& .child").unwrap();
        let comps = list[0].components();
        // RTL: .child, Descendant, &
        assert!(matches!(
            &comps[2],
            Component::Simple(SimpleSelector::Nesting)
        ));
    }

    #[test]
    fn specificity_computed() {
        let list = parse_selector_list("#main .content > p:first-child").unwrap();
        let spec = list[0].specificity();
        // #main = (1,0,0), .content = (0,1,0), p = (0,0,1), :first-child = (0,1,0)
        assert_eq!(spec, Specificity::new(1, 2, 1));
    }

    #[test]
    fn parse_empty_selector_is_error() {
        assert!(parse_selector_list("").is_err());
    }
}
