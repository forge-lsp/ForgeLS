use super::{SolidityNode, cast_node};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, Span, SyntaxElement, SyntaxNode};

/// Using directive: `using L for T;`, `using L for *;`, `using L for T global;`.
///
/// Children hold the `using` token, the library ident, the `for` token,
/// then either a `Star` token or a `Type` node, plus optional `global`
/// and the closing `;`. Operator forms (`using {f as +}`) keep the full
/// span with no library so unknown stays silent while analysis skips them.
#[derive(Debug, Clone)]
pub struct UsingDirective {
    syntax: SyntaxNode,
}

impl UsingDirective {
    pub fn library(&self) -> Option<String> {
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

    pub fn library_span(&self) -> Option<Span> {
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

    fn type_text(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Type.as_u16() => {
                    super::types::TypeNode::cast(node.clone()).map(|typed| typed.text())
                }
                _ => None,
            })
    }

    pub fn target_text(&self) -> Option<String> {
        if self.is_wildcard() {
            return Some(String::from("*"));
        }
        self.type_text()
    }

    pub fn target_span(&self) -> Option<Span> {
        for element in &self.syntax.children {
            match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Star.as_u16() => {
                    return Some(token.span);
                }
                SyntaxElement::Node(node) if node.kind == SolidityNode::Type.as_u16() => {
                    return Some(node.span);
                }
                _ => {}
            }
        }
        None
    }

    pub fn is_wildcard(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token) if token.kind == SolidityKind::Star.as_u16()
            )
        })
    }

    pub fn is_global(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token) if token.kind == SolidityKind::Global.as_u16()
            )
        })
    }
}

impl AstNode for UsingDirective {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Using) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
