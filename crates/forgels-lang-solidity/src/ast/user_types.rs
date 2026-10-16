use super::members::Param;
use super::{SolidityNode, cast_node};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, Span, SyntaxElement, SyntaxNode};

/// Struct definition: name plus field list.
///
/// Children hold the name token first, then one Param node per field.
/// Param reuses type plus data location parsing from function params.
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

    pub fn fields(&self) -> Vec<Param> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Param.as_u16() => {
                    Param::cast(node.clone())
                }
                _ => None,
            })
            .collect()
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

/// Enum definition: name plus variant names.
///
/// Children hold the name token first, then one Ident token per variant.
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

    pub fn variants(&self) -> Vec<String> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.text.clone())
                }
                _ => None,
            })
            .skip(1)
            .collect()
    }

    pub fn variant_spans(&self) -> Vec<Span> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some(token.span)
                }
                _ => None,
            })
            .skip(1)
            .collect()
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

/// Custom error: name only, params skipped.
///
/// Children hold the name token. Arity lives in the source text,
/// not in the tree, so hover and goto use name plus span only.
#[derive(Debug, Clone)]
pub struct ErrorDef {
    syntax: SyntaxNode,
}

impl ErrorDef {
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

impl AstNode for ErrorDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Error) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// User defined value type: `type Name is uint`.
///
/// Children hold the name token first, then one Type node for the
/// underlying type. Only the name matters for symbol lookup.
#[derive(Debug, Clone)]
pub struct UserTypeDef {
    syntax: SyntaxNode,
}

impl UserTypeDef {
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

    pub fn underlying(&self) -> Option<super::types::TypeNode> {
        use super::types::TypeNode;
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Type.as_u16() => {
                    TypeNode::cast(node.clone())
                }
                _ => None,
            })
    }
}

impl AstNode for UserTypeDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::UserType) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
