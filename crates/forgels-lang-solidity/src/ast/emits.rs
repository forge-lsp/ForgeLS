use super::{SolidityNode, cast_node};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, Span, SyntaxElement, SyntaxNode};

/// Emit statement: `emit Name(args);` at block top level.
///
/// Children hold the `emit` token, the event name token, plus one
/// marker per argument (the `(` token for the first arg and one `,`
/// token per extra arg). Empty emits hold only `emit` plus the name,
/// so `arg_count` is the marker count. Partial rows keep what exists:
/// bare `emit` holds only the keyword, `emit Name` holds both.
#[derive(Debug, Clone)]
pub struct EmitStmt {
    syntax: SyntaxNode,
}

impl EmitStmt {
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

impl AstNode for EmitStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Emit) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
