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
