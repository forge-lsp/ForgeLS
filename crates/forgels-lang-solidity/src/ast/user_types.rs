use super::{SolidityNode, cast_node};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, SyntaxElement, SyntaxNode};

/// Struct definition: name only, body is skipped during parsing.
///
/// Children hold the name token so heritage lists never see it.
#[derive(Debug, Clone)]
pub struct StructDef {
    syntax: SyntaxNode,
}

impl StructDef {
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
}

impl AstNode for StructDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Struct) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Enum definition: name only, body is skipped during parsing.
///
/// Children hold the name token so heritage lists never see it.
#[derive(Debug, Clone)]
pub struct EnumDef {
    syntax: SyntaxNode,
}

impl EnumDef {
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
}

impl AstNode for EnumDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Enum) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
