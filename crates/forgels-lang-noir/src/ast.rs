use forgels_syntax::{AstNode, SyntaxNode};

#[derive(Debug, Clone)]
pub struct NoirFile {
    syntax: SyntaxNode,
}

impl NoirFile {
    pub fn new(syntax: SyntaxNode) -> Self {
        Self { syntax }
    }
}

impl AstNode for NoirFile {
    fn cast(node: SyntaxNode) -> Option<Self> {
        Some(Self { syntax: node })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
