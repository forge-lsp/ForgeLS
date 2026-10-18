use super::{SolidityNode, cast_node, first_ident_text};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, Span, SyntaxElement, SyntaxNode};

/// Plain call statement: `name(args);` at block top level.
///
/// Children hold the callee token, optional `{` and `}` option markers,
/// plus one marker per argument (the `(` token for the first arg and one
/// `,` token per extra arg). Empty calls hold only the callee, so
/// `arg_count` counts `(` and `,` markers only.
#[derive(Debug, Clone)]
pub struct CallStmt {
    syntax: SyntaxNode,
}

impl CallStmt {
    pub fn callee(&self) -> Option<String> {
        first_ident_text(&self.syntax)
    }

    pub fn arg_count(&self) -> usize {
        self.syntax
            .children
            .iter()
            .filter(|element| {
                matches!(
                    element,
                    SyntaxElement::Token(token)
                        if token.kind == SolidityKind::LParen.as_u16()
                            || token.kind == SolidityKind::Comma.as_u16()
                )
            })
            .count()
    }

    pub fn has_call_options(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::LBrace.as_u16()
            )
        })
    }

    pub fn options_span(&self) -> Option<Span> {
        let mut open = None;
        let mut close = None;
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::LBrace.as_u16() => {
                    open = Some(token.span);
                }
                SyntaxElement::Token(token) if token.kind == SolidityKind::RBrace.as_u16() => {
                    close = Some(token.span);
                }
                _ => {}
            }
        }
        match (open, close) {
            (Some(start), Some(end)) => Some(Span::new(start.start, end.end)),
            (Some(start), None) => Some(start),
            _ => None,
        }
    }
}

impl AstNode for CallStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Call) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Revert statement: `revert Name(args);` at block top level.
///
/// Children hold the `revert` token, the error name token, plus one
/// marker per argument (the `(` token for the first arg and one `,`
/// token per extra arg). Bare `revert` holds only the keyword, and
/// `revert Name` holds keyword plus name, so arity stays silent
/// without a name and unknown stays silent without args.
#[derive(Debug, Clone)]
pub struct RevertStmt {
    syntax: SyntaxNode,
}

impl RevertStmt {
    pub fn name(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.text.clone())
                }
                _ => None,
            })
    }

    pub fn name_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.span)
                }
                _ => None,
            })
    }

    pub fn arg_count(&self) -> usize {
        let mut markers = 0;
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::LParen.as_u16()
                        || token.kind == SolidityKind::Comma.as_u16() =>
                {
                    markers += 1;
                }
                _ => {}
            }
        }
        markers
    }
}

impl AstNode for RevertStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Revert) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Deploy statement: `new Contract(args);` at block top level.
///
/// Children hold the `new` token, the contract name token, optional
/// `{` and `}` option markers, plus one marker per argument (the `(`
/// token for the first arg and one `,` token per extra arg). Named
/// args (`new C({a: 1})`) never parse so they stay silent. Option rows
/// keep markers so unknown skips `value` and arity still checks.
#[derive(Debug, Clone)]
pub struct NewStmt {
    syntax: SyntaxNode,
}

impl NewStmt {
    pub fn contract_name(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.text.clone())
                }
                _ => None,
            })
    }

    pub fn name_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.span)
                }
                _ => None,
            })
    }

    pub fn arg_count(&self) -> usize {
        let mut markers = 0;
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::LParen.as_u16()
                        || token.kind == SolidityKind::Comma.as_u16() =>
                {
                    markers += 1;
                }
                _ => {}
            }
        }
        markers
    }

    pub fn options_span(&self) -> Option<Span> {
        let mut open = None;
        let mut close = None;
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::LBrace.as_u16() => {
                    open = Some(token.span);
                }
                SyntaxElement::Token(token) if token.kind == SolidityKind::RBrace.as_u16() => {
                    close = Some(token.span);
                }
                _ => {}
            }
        }
        match (open, close) {
            (Some(start), Some(end)) => Some(Span::new(start.start, end.end)),
            (Some(start), None) => Some(start),
            _ => None,
        }
    }
}

impl AstNode for NewStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::New) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
