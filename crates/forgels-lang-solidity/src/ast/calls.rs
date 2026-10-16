use super::{SolidityNode, cast_node, first_ident_text};
use forgels_syntax::{AstNode, SyntaxNode};

/// Plain call statement: `name(args);` at block top level.
///
/// Children hold the callee token plus one token per argument (the
/// first token of each top level arg). Empty calls hold only the
/// callee, so `arg_count` is children minus one.
#[derive(Debug, Clone)]
pub struct CallStmt {
    syntax: SyntaxNode,
}

impl CallStmt {
    pub fn callee(&self) -> Option<String> {
        first_ident_text(&self.syntax)
    }

    pub fn arg_count(&self) -> usize {
        self.syntax.children.len().saturating_sub(1)
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
