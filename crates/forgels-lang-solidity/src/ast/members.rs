use super::types::{BlockNode, TypeNode};
use super::{SolidityNode, cast_node, first_ident_text, is_word_text};
use crate::lexer::SolidityKind;
use forgels_syntax::{AstNode, SyntaxElement, SyntaxNode};

/// Function definition: name, params, return types, and body block.
///
/// Children hold the name token, Param nodes, return Type nodes, then one
/// Block node. Modifier keywords are consumed but not stored; the node
/// span still covers them.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    syntax: SyntaxNode,
}

impl FunctionDef {
    pub fn name(&self) -> Option<String> {
        first_ident_text(&self.syntax)
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

    pub fn return_types(&self) -> Vec<TypeNode> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Returns.as_u16() => {
                    Returns::cast(node.clone())
                }
                _ => None,
            })
            .flat_map(|returns| returns.types())
            .collect()
    }

    pub fn body(&self) -> Option<BlockNode> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Block.as_u16() => {
                    BlockNode::cast(node.clone())
                }
                _ => None,
            })
    }
}

impl AstNode for FunctionDef {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Function) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// One function or returns-clause parameter: type plus optional name.
#[derive(Debug, Clone)]
pub struct Param {
    syntax: SyntaxNode,
}

impl Param {
    pub fn type_node(&self) -> Option<TypeNode> {
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

    pub fn name(&self) -> Option<String> {
        first_ident_text(&self.syntax)
    }
}

impl AstNode for Param {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Param) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn var_name_and_type(node: &SyntaxNode) -> (Option<TypeNode>, Option<String>) {
    let mut type_node = None;
    let mut name = None;
    for element in &node.children {
        match element {
            SyntaxElement::Node(child) if child.kind == SolidityNode::Type.as_u16() => {
                if type_node.is_none() {
                    type_node = TypeNode::cast(child.clone());
                }
            }
            SyntaxElement::Token(token) if name.is_none() && is_word_text(&token.text) => {
                name = Some(token.text.clone());
            }
            _ => {}
        }
    }
    (type_node, name)
}

/// Returns clause: the parenthesized type list after `returns`.
/// Children hold `Param` nodes; bare types never appear here directly.
#[derive(Debug, Clone)]
pub struct Returns {
    syntax: SyntaxNode,
}

impl Returns {
    pub fn types(&self) -> Vec<TypeNode> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(child) if child.kind == SolidityNode::Param.as_u16() => {
                    Param::cast(child.clone()).and_then(|param| param.type_node())
                }
                _ => None,
            })
            .collect()
    }
}

impl AstNode for Returns {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Returns) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// State variable: type plus name. The node span covers any initializer.
#[derive(Debug, Clone)]
pub struct StateVar {
    syntax: SyntaxNode,
}

impl StateVar {
    pub fn type_node(&self) -> Option<TypeNode> {
        var_name_and_type(&self.syntax).0
    }

    pub fn name(&self) -> Option<String> {
        var_name_and_type(&self.syntax).1
    }
}

impl AstNode for StateVar {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::StateVar) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Local variable: same shape as StateVar, found inside blocks.
#[derive(Debug, Clone)]
pub struct LocalVar {
    syntax: SyntaxNode,
}

impl LocalVar {
    pub fn type_node(&self) -> Option<TypeNode> {
        var_name_and_type(&self.syntax).0
    }

    pub fn name(&self) -> Option<String> {
        var_name_and_type(&self.syntax).1
    }
}

impl AstNode for LocalVar {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::LocalVar) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Member access with single ident receiver: `p.age`, `this.total`.
///
/// Children hold receiver token, dot token, then optional member token.
/// Only one dot is stored; chained `a.b.c` keeps the first pair and the
/// rest is skipped as statement tail. Calls add markers after index 2:
/// `(` plus one `,` per extra arg for non-empty, `)` alone for empty.
/// Plain access holds no marker, so `is_call` separates reads from calls.
#[derive(Debug, Clone)]
pub struct MemberExpr {
    syntax: SyntaxNode,
}

impl MemberExpr {
    pub fn receiver(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => Some(token.text.clone()),
                _ => None,
            })
    }

    pub fn member(&self) -> Option<String> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token) => Some(token.text.clone()),
                _ => None,
            })
            .nth(2)
    }

    pub fn is_call(&self) -> bool {
        self.syntax.children.iter().skip(3).any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind == SolidityKind::LParen.as_u16()
                        || token.kind == SolidityKind::Comma.as_u16()
                        || token.kind == SolidityKind::RParen.as_u16()
            )
        })
    }

    pub fn arg_count(&self) -> usize {
        self.syntax
            .children
            .iter()
            .skip(3)
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
}

impl AstNode for MemberExpr {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Member) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
