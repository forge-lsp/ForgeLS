use super::emits::EmitStmt;
use super::members::{LocalVar, MemberExpr};
use super::{CallStmt, SolidityNode, cast_node};
use forgels_syntax::{AstNode, SyntaxElement, SyntaxNode};

/// Type reference: elementary, mapping, sized, array, or user type.
///
/// Children hold the raw tokens. Type text joins them with minimal
/// spacing so hover shows mapping(address => uint).
#[derive(Debug, Clone)]
pub struct TypeNode {
    syntax: SyntaxNode,
}

impl TypeNode {
    pub fn text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        collect_token_text(&self.syntax, &mut parts);
        join_type_text(&parts)
    }
}

fn collect_token_text<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a str>) {
    for element in &node.children {
        match element {
            SyntaxElement::Token(token) => out.push(token.text.as_str()),
            SyntaxElement::Node(child) => collect_token_text(child, out),
        }
    }
}

impl AstNode for TypeNode {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Type) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

fn join_type_text(parts: &[&str]) -> String {
    // Explicit spacing: no guesswork, no chained replaces.
    // A token takes no leading space when it clings to the previous one
    // (`(`, `[`, `)`, `]`, `,`, `;`, `.`), or when the previous token
    // opens a group (`(`, `[`, `.`). Everything else separates with one
    // space, which gives `=>` its surrounding spaces for free.
    const NO_SPACE_BEFORE: &[&str] = &["(", "[", ")", "]", ",", ";", "."];
    const NO_SPACE_AFTER: &[&str] = &["(", "[", "."];
    let mut out = String::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0
            && !NO_SPACE_BEFORE.contains(part)
            && !NO_SPACE_AFTER.contains(&parts[index - 1])
        {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

/// Function or lone block body. Children hold LocalVar, Call, Emit, and
/// Member nodes; other statements are skipped but stay inside the span.
#[derive(Debug, Clone)]
pub struct BlockNode {
    syntax: SyntaxNode,
}

impl BlockNode {
    pub fn locals(&self) -> Vec<LocalVar> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::LocalVar.as_u16() => {
                    LocalVar::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }

    pub fn calls(&self) -> Vec<CallStmt> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Call.as_u16() => {
                    CallStmt::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }

    pub fn members(&self) -> Vec<MemberExpr> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Member.as_u16() => {
                    MemberExpr::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }

    pub fn emits(&self) -> Vec<EmitStmt> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Emit.as_u16() => {
                    EmitStmt::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }
}

impl AstNode for BlockNode {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Block) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
