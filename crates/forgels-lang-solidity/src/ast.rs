use forgels_syntax::{AstNode, SyntaxNode};

#[derive(Debug, Clone)]
pub struct SolidityFile {
    syntax: SyntaxNode,
}

impl SolidityFile {
    pub fn new(syntax: SyntaxNode) -> Self {
        Self { syntax }
    }
}

impl AstNode for SolidityFile {
    fn cast(node: SyntaxNode) -> Option<Self> {
        Some(Self { syntax: node })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
