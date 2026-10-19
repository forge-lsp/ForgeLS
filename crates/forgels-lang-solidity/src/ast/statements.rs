use super::{SolidityNode, cast_node};
use forgels_syntax::{AstNode, SyntaxNode};

/// Return statement: `return [expr] ;` with verbatim tokens.
///
/// Children hold raw tokens from `return` through `;`. No expression
/// grammar in V1; the interior stays verbatim for span precision.
#[derive(Debug, Clone)]
pub struct ReturnStmt {
    syntax: SyntaxNode,
}

impl AstNode for ReturnStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Return) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Break statement: `break ;` with verbatim tokens.
#[derive(Debug, Clone)]
pub struct BreakStmt {
    syntax: SyntaxNode,
}

impl AstNode for BreakStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Break) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Continue statement: `continue ;` with verbatim tokens.
#[derive(Debug, Clone)]
pub struct ContinueStmt {
    syntax: SyntaxNode,
}

impl AstNode for ContinueStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Continue) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Delete statement: `delete <expr> ;` with verbatim tokens.
#[derive(Debug, Clone)]
pub struct DeleteStmt {
    syntax: SyntaxNode,
}

impl AstNode for DeleteStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::Delete) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Do-while statement: `do <body> while (cond) ;` with verbatim tokens.
///
/// The body stays verbatim (brace group or single statement); no nested
/// Block child, so `child_blocks` and `all_*` counts stay unchanged.
#[derive(Debug, Clone)]
pub struct DoWhileStmt {
    syntax: SyntaxNode,
}

impl AstNode for DoWhileStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::DoWhile) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// Bare expression statement: `<expr> ;` with verbatim tokens.
///
/// Covers assignments, increments, and other non-decl rows. Children hold
/// raw tokens from start through `;`. Lone `;` yields no node.
#[derive(Debug, Clone)]
pub struct ExprStmt {
    syntax: SyntaxNode,
}

impl AstNode for ExprStmt {
    fn cast(node: SyntaxNode) -> Option<Self> {
        if cast_node(&node, SolidityNode::ExprStmt) {
            Some(Self { syntax: node })
        } else {
            None
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

/// True for the six statement kinds only.
pub fn is_statement_kind(kind: u16) -> bool {
    kind == SolidityNode::Return.as_u16()
        || kind == SolidityNode::Break.as_u16()
        || kind == SolidityNode::Continue.as_u16()
        || kind == SolidityNode::Delete.as_u16()
        || kind == SolidityNode::DoWhile.as_u16()
        || kind == SolidityNode::ExprStmt.as_u16()
}
