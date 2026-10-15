use crate::Span;

/// Raw kind id. Each language maps its own enum to u16.
pub type SyntaxKind = u16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxToken {
    pub kind: SyntaxKind,
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxNode {
    pub kind: SyntaxKind,
    pub span: Span,
    pub children: Vec<SyntaxElement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxElement {
    Node(SyntaxNode),
    Token(SyntaxToken),
}

impl SyntaxNode {
    pub fn new(kind: SyntaxKind, span: Span) -> Self {
        Self {
            kind,
            span,
            children: Vec::new(),
        }
    }

    pub fn with_children(kind: SyntaxKind, span: Span, children: Vec<SyntaxElement>) -> Self {
        Self {
            kind,
            span,
            children,
        }
    }
}
