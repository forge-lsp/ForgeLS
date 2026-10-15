use crate::{Span, SyntaxNode};

/// Marker trait for typed AST nodes built over [`SyntaxNode`].
pub trait AstNode: Sized {
    fn cast(node: SyntaxNode) -> Option<Self>;
    fn syntax(&self) -> &SyntaxNode;
}

/// Visitor reused by all languages for AST traversal.
pub trait Visitor {
    fn visit_node(&mut self, node: &SyntaxNode);
    fn visit_span(&mut self, _span: &Span) {}
}
