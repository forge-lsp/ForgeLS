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

    /// Variant names paired with their token spans, in order.
    pub fn variant_entries(&self) -> Vec<(String, Span)> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Ident.as_u16() => {
                    Some((token.text.clone(), token.span))
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

/// Custom error: name plus params.
///
/// Children hold the name token first, then one Param node per arg.
/// Hover and goto use name plus span, arity uses param count.
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

    pub fn params(&self) -> Vec<Param> {
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

/// Event definition: name plus params.
///
/// Children hold the name token first, then one Param node per arg, plus
/// the `anonymous` token when present. `indexed` lives inside each Param.
#[derive(Debug, Clone)]
pub struct EventDef {
    syntax: SyntaxNode,
}

impl EventDef {
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

    pub fn params(&self) -> Vec<Param> {
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

    pub fn is_anonymous(&self) -> bool {
        self.syntax.children.iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::Anonymous.as_u16()
            )
        })
    }

    pub fn anonymous_span(&self) -> Option<Span> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if token.kind == SolidityKind::Anonymous.as_u16() => {
                    Some(token.span)
                }
                _ => None,
            })
    }
}

impl AstNode for EventDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Event) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Modifier definition: name only, body skipped.
///
/// Children hold the name token. Params are skipped with paren balance
/// so malformed rows still yield the name for hover and goto.
#[derive(Debug, Clone)]
pub struct ModifierDef {
    syntax: SyntaxNode,
}

impl ModifierDef {
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
}

impl AstNode for ModifierDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Modifier) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
