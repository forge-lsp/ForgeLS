use super::emits::EmitStmt;
use super::members::{LocalVar, MemberExpr};
use super::{CallStmt, NewStmt, RevertStmt, SolidityNode, cast_node};
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

/// Function or lone block body. Children hold LocalVar, Param (try
/// returns plus catch params in the `try` wrapper only), Call, Emit,
/// Revert, New, Member, and nested Block nodes; other statements are
/// skipped but stay inside the span. Nested blocks come from bare
/// `{ ... }` groups and from `if`, `for`, `while`, `unchecked`, and
/// `try` bodies, so scopes and diagnostics recurse with `all_*` while
/// `locals`, `calls`, `members`, `emits`, `reverts`, and `news` stay
/// direct children only.
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

    /// Direct `try` returns plus `catch` params in the wrapper only.
    ///
    /// Other blocks hold no params; `function_scope` owns function
    /// params. Used for scope only, never for outline.
    pub fn params(&self) -> Vec<crate::Param> {
        use forgels_syntax::AstNode;
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Param.as_u16() => {
                    crate::Param::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }

    /// All params in this block plus nested blocks, depth first.
    pub fn all_params(&self) -> Vec<crate::Param> {
        let mut out = self.params();
        for child in self.child_blocks() {
            out.extend(child.all_params());
        }
        out
    }

    /// Direct nested blocks: bare groups plus control bodies.
    pub fn child_blocks(&self) -> Vec<BlockNode> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Block.as_u16() => {
                    BlockNode::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }

    /// All locals in this block plus nested blocks, depth first.
    pub fn all_locals(&self) -> Vec<LocalVar> {
        let mut out = self.locals();
        for child in self.child_blocks() {
            out.extend(child.all_locals());
        }
        out
    }

    /// All plain calls in this block plus nested blocks, depth first.
    pub fn all_calls(&self) -> Vec<CallStmt> {
        let mut out = self.calls();
        for child in self.child_blocks() {
            out.extend(child.all_calls());
        }
        out
    }

    /// All member accesses in this block plus nested blocks, depth first.
    pub fn all_members(&self) -> Vec<MemberExpr> {
        let mut out = self.members();
        for child in self.child_blocks() {
            out.extend(child.all_members());
        }
        out
    }

    /// All emit statements in this block plus nested blocks, depth first.
    pub fn all_emits(&self) -> Vec<EmitStmt> {
        let mut out = self.emits();
        for child in self.child_blocks() {
            out.extend(child.all_emits());
        }
        out
    }

    /// All revert statements in this block plus nested blocks, depth first.
    pub fn all_reverts(&self) -> Vec<RevertStmt> {
        let mut out = self.reverts();
        for child in self.child_blocks() {
            out.extend(child.all_reverts());
        }
        out
    }

    /// All deploy statements in this block plus nested blocks, depth first.
    pub fn all_news(&self) -> Vec<NewStmt> {
        let mut out = self.news();
        for child in self.child_blocks() {
            out.extend(child.all_news());
        }
        out
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

    pub fn reverts(&self) -> Vec<RevertStmt> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::Revert.as_u16() => {
                    RevertStmt::cast(node.clone())
                }
                _ => None,
            })
            .collect()
    }

    pub fn news(&self) -> Vec<NewStmt> {
        self.syntax
            .children
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Node(node) if node.kind == SolidityNode::New.as_u16() => {
                    NewStmt::cast(node.clone())
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
